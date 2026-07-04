//! Poincare-language backend for the notebook evaluator API.
//!
//! `PoincareEvaluator` implements `poincare_evaluator::Evaluator` by driving the
//! `poincare-lang` interpreter: it parses and resolves each cell, runs it
//! against a persistent session, converts runtime values and diagnostics into
//! evaluator types, and bridges the evaluator's `RuntimeHost` to the language's
//! attachment host. This is the adapter that lets a notebook run Poincare cells.

mod convert;

use std::collections::HashMap;

use poincare_evaluator::{
    EvalCellId, EvalContextDelta, EvalDiagnostic, EvalDisplayHint, EvalOutput,
    EvalOutputProvenance, EvalRequest, EvalResponse, EvalStatus, EvalTextStream, EvalValue,
    Evaluator, EvaluatorCapability, EvaluatorMetadata, RuntimeHost, SessionSnapshot, SessionStatus,
    TextValue, VariableSummary,
};
use poincare_lang::{Host, Interpreter, SessionScope, Severity, SourceMap, parse, resolve};

/// A notebook evaluator backed by the Poincare language interpreter.
pub struct PoincareEvaluator {
    interp: Interpreter,
    run_count: u64,
    /// Cell and run that most recently defined each top-level name.
    var_sources: HashMap<String, (EvalCellId, u64)>,
}

impl Default for PoincareEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl PoincareEvaluator {
    pub fn new() -> Self {
        Self {
            interp: Interpreter::new(),
            run_count: 0,
            var_sources: HashMap::new(),
        }
    }

    fn build_variables(&self) -> Vec<VariableSummary> {
        self.interp
            .variables()
            .into_iter()
            .map(|(name, value)| {
                let (source_cell, updated_at_run) = match self.var_sources.get(&name) {
                    Some((cell, run)) => (Some(cell.clone()), Some(*run)),
                    None => (None, None),
                };
                VariableSummary {
                    kind: convert::value_kind(&value),
                    preview: preview(&value.display()),
                    name,
                    source_cell,
                    updated_at_run,
                    stale: false,
                    size_hint: None,
                }
            })
            .collect()
    }

    fn snapshot(&self, status: SessionStatus) -> SessionSnapshot {
        SessionSnapshot {
            run_count: self.run_count,
            status,
            variables: self.build_variables(),
        }
    }
}

impl Evaluator for PoincareEvaluator {
    fn metadata(&self) -> EvaluatorMetadata {
        let mut metadata = EvaluatorMetadata::new("poincare", "Poincare");
        metadata.version = Some(env!("CARGO_PKG_VERSION").to_string());
        metadata.features.supports_shared_state = true;
        metadata.features.supports_attachments = true;
        metadata.features.supports_graph_outputs = true;
        metadata.features.supports_table_outputs = true;
        metadata.features.supports_symbolic_expr = false;
        metadata.features.supports_interrupt = false;
        metadata.features.supports_variable_delete = false;
        metadata.safety.capabilities = vec![
            EvaluatorCapability::PureComputation,
            EvaluatorCapability::AttachmentRead,
        ];
        metadata
    }

    fn evaluate_cell(&mut self, request: EvalRequest, host: &dyn RuntimeHost) -> EvalResponse {
        let source = request.source;
        let cell_id = request.cell_id;
        let map = SourceMap::new(&source);

        // Parse.
        let parsed = parse(&source);
        let mut diagnostics: Vec<EvalDiagnostic> = parsed
            .diagnostics
            .iter()
            .map(|d| convert::diagnostic_to_eval(d, &map))
            .collect();
        if parsed
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            return self.failed(diagnostics);
        }

        // Resolve against the current session names.
        let session_scope: SessionScope =
            self.interp.variables().into_iter().map(|(n, _)| n).collect();
        let resolved = resolve(&parsed.program, &session_scope);
        for d in &resolved.diagnostics {
            diagnostics.push(convert::diagnostic_to_eval(d, &map));
        }
        if resolved.has_errors() {
            return self.failed(diagnostics);
        }

        // Run.
        self.run_count += 1;
        let bridge = HostBridge { host };
        let outcome = self.interp.run_with_host(&parsed.program, &bridge);

        // Record which cell defined each new name.
        for name in &resolved.cell_defs {
            self.var_sources
                .insert(name.clone(), (cell_id.clone(), self.run_count));
        }

        // Assemble outputs: print stream, then emitted values, then the final
        // expression value.
        let mut outputs = Vec::new();
        for line in &outcome.output {
            outputs.push(EvalOutput {
                id: None,
                value: EvalValue::Text(TextValue {
                    stream: EvalTextStream::Stdout,
                    text: line.clone(),
                }),
                display: EvalDisplayHint::Text,
                provenance: EvalOutputProvenance::source_cell(cell_id.clone()),
            });
        }
        for value in &outcome.emitted {
            outputs.push(EvalOutput::display(
                convert::to_eval_value(value),
                cell_id.clone(),
            ));
        }
        if let Some(value) = &outcome.value {
            outputs.push(EvalOutput::display(
                convert::to_eval_value(value),
                cell_id.clone(),
            ));
        }

        match outcome.error {
            Some(error) => {
                diagnostics.push(convert::runtime_error_to_eval(&error, &map));
                EvalResponse {
                    status: EvalStatus::Failed,
                    outputs,
                    diagnostics,
                    context_delta: None,
                    session: Some(self.snapshot(SessionStatus::Failed)),
                }
            }
            None => {
                let snapshot = self.snapshot(SessionStatus::Idle);
                EvalResponse {
                    status: EvalStatus::Complete,
                    outputs,
                    diagnostics,
                    context_delta: Some(EvalContextDelta::VariablesUpdated(
                        snapshot.variables.clone(),
                    )),
                    session: Some(snapshot),
                }
            }
        }
    }

    fn session_snapshot(&self) -> SessionSnapshot {
        self.snapshot(SessionStatus::Idle)
    }

    fn restart(&mut self) -> EvalResponse {
        self.interp = Interpreter::new();
        self.var_sources.clear();
        self.run_count = 0;
        EvalResponse {
            status: EvalStatus::Complete,
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            context_delta: Some(EvalContextDelta::Restarted),
            session: Some(self.snapshot(SessionStatus::Restarted)),
        }
    }
}

impl PoincareEvaluator {
    fn failed(&self, diagnostics: Vec<EvalDiagnostic>) -> EvalResponse {
        EvalResponse {
            status: EvalStatus::Failed,
            outputs: Vec::new(),
            diagnostics,
            context_delta: None,
            session: Some(self.snapshot(SessionStatus::Failed)),
        }
    }
}

/// Truncate a preview to a reasonable length on a char boundary.
fn preview(text: &str) -> String {
    const MAX: usize = 80;
    if text.chars().count() > MAX {
        let mut s: String = text.chars().take(MAX - 1).collect();
        s.push('…');
        s
    } else {
        text.to_string()
    }
}

/// Bridges the evaluator's `RuntimeHost` (resolve-then-fetch by id) to the
/// language's name-based `Host`.
struct HostBridge<'a> {
    host: &'a dyn RuntimeHost,
}

impl Host for HostBridge<'_> {
    fn attachment_text(&self, name_or_id: &str) -> Result<String, String> {
        let attachment = self
            .host
            .resolve_attachment(name_or_id)
            .map_err(|e| e.message)?;
        self.host
            .attachment_text(&attachment.id)
            .map_err(|e| e.message)
    }

    fn attachment_bytes(&self, name_or_id: &str) -> Result<Vec<u8>, String> {
        let attachment = self
            .host
            .resolve_attachment(name_or_id)
            .map_err(|e| e.message)?;
        self.host
            .attachment_bytes(&attachment.id)
            .map_err(|e| e.message)
    }
}
