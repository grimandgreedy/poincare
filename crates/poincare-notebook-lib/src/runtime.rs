use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::{NotebookCellId, NotebookId, ValueKind};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeSessionId(pub String);

impl RuntimeSessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRevision(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeBindingKind {
    Variable,
    Function,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBindingSummary {
    pub name: String,
    pub binding_kind: RuntimeBindingKind,
    pub value_kind: ValueKind,
    pub preview: String,
    pub source_cell: Option<NotebookCellId>,
    pub updated_at_run: Option<u64>,
    pub stale: bool,
    pub size_hint: Option<String>,
}

impl RuntimeBindingSummary {
    fn from_evaluator(value: evaluator::VariableSummary) -> Self {
        let value_kind = ValueKind::from(value.kind);
        let binding_kind = if value_kind == ValueKind::Function {
            RuntimeBindingKind::Function
        } else {
            RuntimeBindingKind::Variable
        };

        Self {
            name: value.name,
            binding_kind,
            value_kind,
            preview: value.preview,
            source_cell: value.source_cell.map(|cell_id| NotebookCellId(cell_id.0)),
            updated_at_run: value.updated_at_run,
            stale: value.stale,
            size_hint: value.size_hint,
        }
    }

    fn to_evaluator(&self) -> evaluator::VariableSummary {
        evaluator::VariableSummary {
            name: self.name.clone(),
            kind: self.value_kind.into(),
            preview: self.preview.clone(),
            source_cell: self
                .source_cell
                .as_ref()
                .map(|cell_id| evaluator::EvalCellId(cell_id.0.clone())),
            updated_at_run: self.updated_at_run,
            stale: self.stale,
            size_hint: self.size_hint.clone(),
        }
    }
}

impl From<evaluator::ValueKind> for ValueKind {
    fn from(value: evaluator::ValueKind) -> Self {
        match value {
            evaluator::ValueKind::Unit => Self::Unit,
            evaluator::ValueKind::Bool => Self::Bool,
            evaluator::ValueKind::Number => Self::Number,
            evaluator::ValueKind::String => Self::String,
            evaluator::ValueKind::List => Self::List,
            evaluator::ValueKind::Function => Self::Function,
            evaluator::ValueKind::Attachment => Self::Attachment,
            evaluator::ValueKind::Bytes => Self::Bytes,
            evaluator::ValueKind::Expression => Self::Expression,
            evaluator::ValueKind::Table => Self::Table,
            evaluator::ValueKind::Array => Self::Array,
            evaluator::ValueKind::Plot => Self::Plot,
            evaluator::ValueKind::Graph => Self::Graph,
            evaluator::ValueKind::Analysis => Self::Analysis,
            evaluator::ValueKind::Image => Self::Image,
            evaluator::ValueKind::Diagnostic => Self::Diagnostic,
            evaluator::ValueKind::Unknown => Self::Unknown,
        }
    }
}

impl From<ValueKind> for evaluator::ValueKind {
    fn from(value: ValueKind) -> Self {
        match value {
            ValueKind::Unit => Self::Unit,
            ValueKind::Bool => Self::Bool,
            ValueKind::Number => Self::Number,
            ValueKind::String => Self::String,
            ValueKind::List => Self::List,
            ValueKind::Function => Self::Function,
            ValueKind::Attachment => Self::Attachment,
            ValueKind::Bytes => Self::Bytes,
            ValueKind::Expression => Self::Expression,
            ValueKind::Table => Self::Table,
            ValueKind::Array => Self::Array,
            ValueKind::Plot => Self::Plot,
            ValueKind::Graph => Self::Graph,
            ValueKind::Analysis => Self::Analysis,
            ValueKind::Image => Self::Image,
            ValueKind::Diagnostic => Self::Diagnostic,
            ValueKind::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeSessionStatus {
    Idle,
    Running,
    Restarted,
    Failed,
    Cancelled,
    ResourceLimitExceeded,
}

impl From<evaluator::SessionStatus> for RuntimeSessionStatus {
    fn from(value: evaluator::SessionStatus) -> Self {
        match value {
            evaluator::SessionStatus::Idle => Self::Idle,
            evaluator::SessionStatus::Running => Self::Running,
            evaluator::SessionStatus::Restarted => Self::Restarted,
            evaluator::SessionStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEnvironment {
    pub session_id: RuntimeSessionId,
    pub document_id: NotebookId,
    pub evaluator: evaluator::EvaluatorMetadata,
    pub revision: RuntimeRevision,
    pub run_count: u64,
    pub status: RuntimeSessionStatus,
    pub bindings: Vec<RuntimeBindingSummary>,
}

impl RuntimeEnvironment {
    pub fn variables(&self) -> impl Iterator<Item = &RuntimeBindingSummary> {
        self.bindings
            .iter()
            .filter(|binding| binding.binding_kind == RuntimeBindingKind::Variable)
    }

    pub fn functions(&self) -> impl Iterator<Item = &RuntimeBindingSummary> {
        self.bindings
            .iter()
            .filter(|binding| binding.binding_kind == RuntimeBindingKind::Function)
    }

    fn eval_context(&self) -> evaluator::EvalContext {
        evaluator::EvalContext {
            run_count: self.run_count,
            variables: self
                .bindings
                .iter()
                .map(RuntimeBindingSummary::to_evaluator)
                .collect(),
            stale_reasons: Vec::new(),
            options: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSessionSnapshot {
    pub session_id: RuntimeSessionId,
    pub document_id: NotebookId,
    pub evaluator_language_id: String,
    pub evaluator_display_name: String,
    pub revision: RuntimeRevision,
    pub run_count: u64,
    pub status: RuntimeSessionStatus,
    pub bindings: Vec<RuntimeBindingSummary>,
}

impl RuntimeSessionSnapshot {
    pub fn variables(&self) -> impl Iterator<Item = &RuntimeBindingSummary> {
        self.bindings
            .iter()
            .filter(|binding| binding.binding_kind == RuntimeBindingKind::Variable)
    }

    pub fn functions(&self) -> impl Iterator<Item = &RuntimeBindingSummary> {
        self.bindings
            .iter()
            .filter(|binding| binding.binding_kind == RuntimeBindingKind::Function)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeEvaluation {
    pub response: evaluator::EvalResponse,
    pub snapshot: RuntimeSessionSnapshot,
}

pub struct EvaluatorSession {
    evaluator: Box<dyn evaluator::Evaluator>,
    environment: RuntimeEnvironment,
}

impl EvaluatorSession {
    pub fn new(
        session_id: RuntimeSessionId,
        document_id: NotebookId,
        evaluator: Box<dyn evaluator::Evaluator>,
    ) -> Self {
        let metadata = evaluator.metadata();
        let evaluator_snapshot = evaluator.session_snapshot();
        Self {
            evaluator,
            environment: RuntimeEnvironment {
                session_id,
                document_id,
                evaluator: metadata,
                revision: RuntimeRevision(0),
                run_count: evaluator_snapshot.run_count,
                status: evaluator_snapshot.status.into(),
                bindings: evaluator_snapshot
                    .variables
                    .into_iter()
                    .map(RuntimeBindingSummary::from_evaluator)
                    .collect(),
            },
        }
    }

    pub fn environment(&self) -> &RuntimeEnvironment {
        &self.environment
    }

    pub fn snapshot(&self) -> RuntimeSessionSnapshot {
        RuntimeSessionSnapshot {
            session_id: self.environment.session_id.clone(),
            document_id: self.environment.document_id.clone(),
            evaluator_language_id: self.environment.evaluator.language_id.clone(),
            evaluator_display_name: self.environment.evaluator.display_name.clone(),
            revision: self.environment.revision.clone(),
            run_count: self.environment.run_count,
            status: self.environment.status,
            bindings: self.environment.bindings.clone(),
        }
    }

    pub fn evaluate_cell(
        &mut self,
        cell_id: NotebookCellId,
        source: impl Into<String>,
        host: &dyn evaluator::RuntimeHost,
    ) -> RuntimeEvaluation {
        self.environment.status = RuntimeSessionStatus::Running;
        let request = evaluator::EvalRequest {
            document_id: evaluator::EvalDocumentId(self.environment.document_id.0.clone()),
            cell_id: evaluator::EvalCellId(cell_id.0),
            source: source.into(),
            context: self.environment.eval_context(),
        };

        let response = self.evaluator.evaluate_cell(request, host);
        self.apply_response_snapshot(&response);

        RuntimeEvaluation {
            response,
            snapshot: self.snapshot(),
        }
    }

    pub fn restart(&mut self) -> RuntimeEvaluation {
        let response = self.evaluator.restart();
        self.apply_response_snapshot(&response);

        RuntimeEvaluation {
            response,
            snapshot: self.snapshot(),
        }
    }

    pub fn delete_variable(&mut self, name: &str) -> RuntimeEvaluation {
        let response = self.evaluator.delete_variable(name);
        self.apply_response_snapshot(&response);

        RuntimeEvaluation {
            response,
            snapshot: self.snapshot(),
        }
    }

    fn apply_response_snapshot(&mut self, response: &evaluator::EvalResponse) {
        self.environment.revision.0 += 1;
        let snapshot = response
            .session
            .clone()
            .unwrap_or_else(|| self.evaluator.session_snapshot());
        self.environment.run_count = snapshot.run_count;
        self.environment.status = match response.status {
            evaluator::EvalStatus::Complete => snapshot.status.into(),
            evaluator::EvalStatus::Failed => RuntimeSessionStatus::Failed,
            evaluator::EvalStatus::Cancelled => RuntimeSessionStatus::Cancelled,
            evaluator::EvalStatus::ResourceLimitExceeded => {
                RuntimeSessionStatus::ResourceLimitExceeded
            }
        };
        self.environment.bindings = snapshot
            .variables
            .into_iter()
            .map(RuntimeBindingSummary::from_evaluator)
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        NotebookBlock, NotebookDocument, NotebookOutput, NotebookOutputId, NotebookOutputKind,
        OutputProvenance, ValueOutput,
    };
    use evaluator::{
        AttachmentValue, EvalAttachmentId, EvalContextDelta, EvalOutput, EvalRequest, EvalResponse,
        EvalStatus, EvalValue, Evaluator, EvaluatorMetadata, HostError, RuntimeHost,
        SessionSnapshot, SessionStatus, VariableSummary,
    };

    struct EmptyRuntimeHost;

    impl RuntimeHost for EmptyRuntimeHost {
        fn resolve_attachment(&self, _name_or_id: &str) -> Result<AttachmentValue, HostError> {
            Err(HostError::not_found("no attachments"))
        }

        fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
            Err(HostError::not_found("no attachments"))
        }
    }

    struct StatefulEvaluator {
        snapshot: SessionSnapshot,
    }

    impl StatefulEvaluator {
        fn new() -> Self {
            Self {
                snapshot: SessionSnapshot::default(),
            }
        }
    }

    impl Evaluator for StatefulEvaluator {
        fn metadata(&self) -> EvaluatorMetadata {
            let mut metadata = EvaluatorMetadata::new("fake-stateful", "Fake Stateful");
            metadata.features.supports_shared_state = true;
            metadata
        }

        fn evaluate_cell(&mut self, request: EvalRequest, _host: &dyn RuntimeHost) -> EvalResponse {
            self.snapshot.run_count += 1;
            self.snapshot.status = SessionStatus::Idle;

            if request.source.contains("define-f") {
                self.snapshot.variables.push(VariableSummary {
                    name: "f".to_string(),
                    kind: evaluator::ValueKind::Function,
                    preview: "f(x)".to_string(),
                    source_cell: Some(request.cell_id.clone()),
                    updated_at_run: Some(self.snapshot.run_count),
                    stale: false,
                    size_hint: None,
                });
            } else {
                self.snapshot.variables.push(VariableSummary {
                    name: "a".to_string(),
                    kind: evaluator::ValueKind::Number,
                    preview: "3".to_string(),
                    source_cell: Some(request.cell_id.clone()),
                    updated_at_run: Some(self.snapshot.run_count),
                    stale: false,
                    size_hint: None,
                });
            }

            EvalResponse {
                status: EvalStatus::Complete,
                outputs: vec![EvalOutput::display(
                    EvalValue::String("ok".to_string()),
                    request.cell_id,
                )],
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
    }

    #[test]
    fn runtime_session_tracks_live_bindings_without_mutating_document_outputs() {
        let mut document = NotebookDocument::new(NotebookId::new("notebook-1"), "Runtime");
        document.add_block(NotebookBlock::executable(
            NotebookCellId::new("cell-1"),
            "fake-stateful",
            "a := 3",
        ));

        let mut session = EvaluatorSession::new(
            RuntimeSessionId::new("session-1"),
            document.id.clone(),
            Box::new(StatefulEvaluator::new()),
        );
        let host = EmptyRuntimeHost;
        let result = session.evaluate_cell(NotebookCellId::new("cell-1"), "a := 3", &host);

        assert_eq!(result.response.status, EvalStatus::Complete);
        assert_eq!(result.snapshot.run_count, 1);
        assert_eq!(result.snapshot.revision, RuntimeRevision(1));
        assert_eq!(result.snapshot.bindings.len(), 1);
        assert_eq!(result.snapshot.bindings[0].name, "a");
        assert_eq!(result.snapshot.bindings[0].value_kind, ValueKind::Number);
        assert_eq!(
            result.snapshot.bindings[0].source_cell,
            Some(NotebookCellId::new("cell-1"))
        );
        assert_eq!(document.blocks[0].outputs.len(), 0);
    }

    #[test]
    fn runtime_session_separates_variable_and_function_summaries() {
        let mut session = EvaluatorSession::new(
            RuntimeSessionId::new("session-1"),
            NotebookId::new("notebook-1"),
            Box::new(StatefulEvaluator::new()),
        );
        let host = EmptyRuntimeHost;

        session.evaluate_cell(NotebookCellId::new("cell-1"), "a := 3", &host);
        let result =
            session.evaluate_cell(NotebookCellId::new("cell-2"), "define-f f(x) := x", &host);

        let variables: Vec<_> = result.snapshot.variables().collect();
        let functions: Vec<_> = result.snapshot.functions().collect();
        assert_eq!(variables.len(), 1);
        assert_eq!(variables[0].name, "a");
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "f");
        assert_eq!(result.snapshot.run_count, 2);
    }

    #[test]
    fn runtime_restart_clears_live_bindings_but_not_saved_outputs() {
        let cell_id = NotebookCellId::new("cell-1");
        let mut document = NotebookDocument::new(NotebookId::new("notebook-1"), "Runtime");
        let mut cell = NotebookBlock::executable(cell_id.clone(), "fake-stateful", "a := 3");
        cell.outputs.push(NotebookOutput {
            id: NotebookOutputId::new("output-1"),
            kind: NotebookOutputKind::Value(ValueOutput {
                value_kind: ValueKind::Number,
                preview: "3".to_string(),
            }),
            provenance: OutputProvenance::source_cell(cell_id.clone()),
            stale: Vec::new(),
        });
        document.add_block(cell);

        let mut session = EvaluatorSession::new(
            RuntimeSessionId::new("session-1"),
            document.id.clone(),
            Box::new(StatefulEvaluator::new()),
        );
        let host = EmptyRuntimeHost;
        session.evaluate_cell(cell_id, "a := 3", &host);

        let result = session.restart();

        assert_eq!(result.snapshot.status, RuntimeSessionStatus::Restarted);
        assert_eq!(result.snapshot.run_count, 0);
        assert_eq!(result.snapshot.bindings.len(), 0);
        assert_eq!(result.snapshot.revision, RuntimeRevision(2));
        assert_eq!(document.blocks[0].outputs.len(), 1);
    }
}
