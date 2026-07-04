use std::collections::BTreeMap;

use poincare_evaluator::{
    AttachmentValue, EvalAttachmentId, EvalCellId, EvalContextDelta, EvalDiagnostic,
    EvalDiagnosticSeverity, EvalOutput, EvalRequest, EvalResponse, EvalStatus, EvalTextStream,
    EvalValue, Evaluator, EvaluatorMetadata, HostError, NumberValue, RuntimeHost, SessionSnapshot,
    SessionStatus, TableValue, TextValue, ValueKind, VariableSummary,
};

pub struct EmptyNotebookHost;

impl RuntimeHost for EmptyNotebookHost {
    fn resolve_attachment(&self, _name_or_id: &str) -> Result<AttachmentValue, HostError> {
        Err(HostError::not_found("attachments are not wired yet"))
    }

    fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
        Err(HostError::not_found("attachments are not wired yet"))
    }
}

pub struct ScratchEvaluator {
    snapshot: SessionSnapshot,
    variables: BTreeMap<String, VariableSummary>,
}

impl ScratchEvaluator {
    pub fn new() -> Self {
        Self {
            snapshot: SessionSnapshot::default(),
            variables: BTreeMap::new(),
        }
    }

    fn sync_variables(&mut self) {
        self.snapshot.variables = self.variables.values().cloned().collect();
    }

    fn output_text(cell_id: EvalCellId, text: impl Into<String>) -> EvalOutput {
        EvalOutput::display(
            EvalValue::Text(TextValue {
                stream: EvalTextStream::Stdout,
                text: text.into(),
            }),
            cell_id,
        )
    }

    fn output_value(cell_id: EvalCellId, source: &str) -> EvalOutput {
        let trimmed = source.trim();
        if let Ok(value) = trimmed.parse::<i64>() {
            return EvalOutput::display(EvalValue::Number(NumberValue::Int(value)), cell_id);
        }
        if let Ok(value) = trimmed.parse::<f64>() {
            return EvalOutput::display(EvalValue::Number(NumberValue::Float(value)), cell_id);
        }
        EvalOutput::display(EvalValue::String(trimmed.to_string()), cell_id)
    }

    fn variable_summary(
        name: impl Into<String>,
        value: &EvalValue,
        cell_id: EvalCellId,
        run_count: u64,
    ) -> VariableSummary {
        let name = name.into();
        VariableSummary {
            name,
            kind: value_kind(value),
            preview: value_preview(value),
            source_cell: Some(cell_id),
            updated_at_run: Some(run_count),
            stale: false,
            size_hint: None,
        }
    }
}

impl Evaluator for ScratchEvaluator {
    fn metadata(&self) -> EvaluatorMetadata {
        let mut metadata = EvaluatorMetadata::new("poincare-scratch", "Poincare Scratch");
        metadata.features.supports_shared_state = true;
        metadata.features.supports_table_outputs = true;
        metadata.features.supports_variable_delete = true;
        metadata
    }

    fn evaluate_cell(&mut self, request: EvalRequest, _host: &dyn RuntimeHost) -> EvalResponse {
        self.snapshot.run_count += 1;
        self.snapshot.status = SessionStatus::Idle;
        let run_count = self.snapshot.run_count;
        let cell_id = request.cell_id.clone();
        let source = request.source.trim();

        if source.contains("fail") || source.contains("error(") {
            return EvalResponse {
                status: EvalStatus::Failed,
                outputs: Vec::new(),
                diagnostics: vec![EvalDiagnostic {
                    severity: EvalDiagnosticSeverity::Error,
                    message: "scratch evaluator failure requested by source".to_string(),
                    span: None,
                    code: Some("SCRATCH_FAILURE".to_string()),
                }],
                context_delta: None,
                session: Some(self.snapshot.clone()),
            };
        }

        let mut outputs = Vec::new();
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if let Some(text) = parse_print(line) {
                outputs.push(Self::output_text(cell_id.clone(), text));
                continue;
            }

            if line == "table()" || line == "table" {
                outputs.push(EvalOutput::display(
                    EvalValue::Table(TableValue {
                        title: Some("Scratch table".to_string()),
                        columns: vec!["x".to_string(), "y".to_string()],
                        rows: vec![
                            vec!["1".to_string(), "2".to_string()],
                            vec!["3".to_string(), "4".to_string()],
                        ],
                        truncated: false,
                    }),
                    cell_id.clone(),
                ));
                continue;
            }

            if let Some((name, value_source)) = parse_assignment(line) {
                let value = parse_value(value_source);
                let summary = Self::variable_summary(name, &value, cell_id.clone(), run_count);
                self.variables.insert(summary.name.clone(), summary);
                outputs.push(EvalOutput::display(value, cell_id.clone()));
                continue;
            }

            outputs.push(Self::output_value(cell_id.clone(), line));
        }

        self.sync_variables();
        EvalResponse {
            status: EvalStatus::Complete,
            outputs,
            diagnostics: Vec::new(),
            context_delta: Some(EvalContextDelta::VariablesUpdated(
                self.snapshot.variables.clone(),
            )),
            session: Some(self.snapshot.clone()),
        }
    }

    fn session_snapshot(&self) -> SessionSnapshot {
        self.snapshot.clone()
    }

    fn restart(&mut self) -> EvalResponse {
        self.variables.clear();
        self.snapshot = SessionSnapshot {
            run_count: 0,
            status: SessionStatus::Restarted,
            variables: Vec::new(),
        };
        EvalResponse {
            status: EvalStatus::Complete,
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            context_delta: Some(EvalContextDelta::Restarted),
            session: Some(self.snapshot.clone()),
        }
    }

    fn delete_variable(&mut self, name: &str) -> EvalResponse {
        self.variables.remove(name);
        self.sync_variables();
        EvalResponse {
            status: EvalStatus::Complete,
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            context_delta: Some(EvalContextDelta::VariablesRemoved(vec![name.to_string()])),
            session: Some(self.snapshot.clone()),
        }
    }
}

fn parse_print(line: &str) -> Option<String> {
    let inner = line.strip_prefix("print(")?.strip_suffix(')')?.trim();
    Some(strip_quotes(inner).to_string())
}

fn parse_assignment(line: &str) -> Option<(&str, &str)> {
    for separator in [":=", "="] {
        if let Some((name, value)) = line.split_once(separator) {
            let name = name.trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                return Some((name, value.trim()));
            }
        }
    }
    None
}

fn parse_value(source: &str) -> EvalValue {
    let source = source.trim();
    if let Ok(value) = source.parse::<i64>() {
        EvalValue::Number(NumberValue::Int(value))
    } else if let Ok(value) = source.parse::<f64>() {
        EvalValue::Number(NumberValue::Float(value))
    } else {
        EvalValue::String(strip_quotes(source).to_string())
    }
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn value_kind(value: &EvalValue) -> ValueKind {
    match value {
        EvalValue::Unit => ValueKind::Unit,
        EvalValue::Bool(_) => ValueKind::Bool,
        EvalValue::Number(_) => ValueKind::Number,
        EvalValue::String(_) | EvalValue::Text(_) => ValueKind::String,
        EvalValue::List(_) => ValueKind::List,
        EvalValue::Function(_) => ValueKind::Function,
        EvalValue::Expr(_) => ValueKind::Expression,
        EvalValue::Attachment(_) => ValueKind::Attachment,
        EvalValue::Bytes(_) => ValueKind::Bytes,
        EvalValue::Table(_) => ValueKind::Table,
        EvalValue::Array(_) => ValueKind::Array,
        EvalValue::Plot(_) => ValueKind::Plot,
        EvalValue::Graph(_) => ValueKind::Graph,
        EvalValue::Analysis(_) => ValueKind::Analysis,
        EvalValue::Image(_) => ValueKind::Image,
        EvalValue::Diagnostic(_) => ValueKind::Diagnostic,
    }
}

fn value_preview(value: &EvalValue) -> String {
    match value {
        EvalValue::Number(NumberValue::Int(value)) => value.to_string(),
        EvalValue::Number(NumberValue::Float(value)) => value.to_string(),
        EvalValue::Number(NumberValue::Rational { numer, denom }) => format!("{numer}/{denom}"),
        EvalValue::String(value) => value.clone(),
        EvalValue::Text(value) => value.text.clone(),
        EvalValue::Table(value) => format!(
            "{} rows x {} columns",
            value.rows.len(),
            value.columns.len()
        ),
        _ => format!("{value:?}"),
    }
}
