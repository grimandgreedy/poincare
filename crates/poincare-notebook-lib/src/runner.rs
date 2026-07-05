use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::{
    CancellableRuntimeHost, EvaluatorSession, ExecutableCell, ExecutionState, NotebookBlock,
    NotebookBlockKind, NotebookCellId, NotebookDocument, OutputReproContext,
    ReproducibilityMetadata, RuntimeCancellationToken, RuntimeDeleteVariableResult,
    RuntimeExecutionPolicy, RuntimeFailure, RuntimeInspectionSnapshot, RuntimeOutputCompletion,
    RuntimeOutputLimits, RuntimePartialExecution, RuntimeResourceLimits, RuntimeSessionSnapshot,
    RuntimeStopReason, StaleReason, classify_eval_response, delete_variable_status,
    notebook_outputs_from_eval_response_with, partial_execution_for_response, source_hash,
    stop_reason_for_failure,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeRunCommand {
    RunCurrentCell { cell_id: NotebookCellId },
    RunAll,
    Restart,
    RestartAndRunAll,
    MarkCellEdited { cell_id: NotebookCellId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCellRun {
    pub cell_id: NotebookCellId,
    pub status: ExecutionState,
    pub output_completion: RuntimeOutputCompletion,
    pub failure: Option<RuntimeFailure>,
    pub partial_execution: Option<RuntimePartialExecution>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeRunReport {
    pub command: RuntimeRunCommand,
    pub cells: Vec<RuntimeCellRun>,
    pub stopped_at: Option<NotebookCellId>,
    pub stop_reason: Option<RuntimeStopReason>,
    pub policy: RuntimeExecutionPolicy,
    pub reproducibility: ReproducibilityMetadata,
    pub snapshot: RuntimeSessionSnapshot,
}

pub struct NotebookRuntime {
    session: EvaluatorSession,
    output_limits: RuntimeOutputLimits,
    resource_limits: RuntimeResourceLimits,
    cancellation_token: RuntimeCancellationToken,
    policy: RuntimeExecutionPolicy,
}

impl NotebookRuntime {
    pub fn new(session: EvaluatorSession) -> Self {
        Self {
            session,
            resource_limits: RuntimeResourceLimits::default(),
            output_limits: RuntimeResourceLimits::default().output_limits(),
            cancellation_token: RuntimeCancellationToken::new(),
            policy: RuntimeExecutionPolicy::default(),
        }
    }

    pub fn with_output_limits(mut self, output_limits: RuntimeOutputLimits) -> Self {
        self.output_limits = output_limits;
        self.resource_limits.max_output_count = self.output_limits.max_outputs_per_cell;
        self.resource_limits.max_text_chars = self.output_limits.max_text_chars;
        self.resource_limits.max_table_rows = self.output_limits.max_table_rows;
        self.resource_limits.max_graph_outputs = self.output_limits.max_graph_outputs;
        self
    }

    pub fn with_resource_limits(mut self, resource_limits: RuntimeResourceLimits) -> Self {
        self.output_limits = resource_limits.output_limits();
        self.resource_limits = resource_limits;
        self
    }

    pub fn session(&self) -> &EvaluatorSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut EvaluatorSession {
        &mut self.session
    }

    pub fn output_limits(&self) -> &RuntimeOutputLimits {
        &self.output_limits
    }

    pub fn resource_limits(&self) -> &RuntimeResourceLimits {
        &self.resource_limits
    }

    pub fn cancellation_token(&self) -> RuntimeCancellationToken {
        self.cancellation_token.clone()
    }

    pub fn interrupt(&self) {
        self.cancellation_token.cancel();
    }

    pub fn clear_interrupt(&self) {
        self.cancellation_token.reset();
    }

    pub fn with_execution_policy(mut self, policy: RuntimeExecutionPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn execution_policy(&self) -> RuntimeExecutionPolicy {
        self.policy
    }

    pub fn inspect_session(&self) -> RuntimeInspectionSnapshot {
        RuntimeInspectionSnapshot::from_session(&self.session.snapshot())
    }

    pub fn delete_variable(&mut self, name: &str) -> RuntimeDeleteVariableResult {
        let evaluation = self.session.delete_variable(name);
        RuntimeDeleteVariableResult {
            name: name.to_string(),
            status: delete_variable_status(&evaluation),
            evaluation,
        }
    }

    pub fn run_current_cell(
        &mut self,
        document: &mut NotebookDocument,
        cell_id: &NotebookCellId,
        host: &dyn evaluator::RuntimeHost,
    ) -> RuntimeRunReport {
        self.clear_interrupt();
        let mut cells = Vec::new();
        let stopped_at = match self.run_cell(document, cell_id, host) {
            Some(cell_run) => {
                let failed = cell_run.failure.is_some();
                let stopped_at = failed.then(|| cell_run.cell_id.clone());
                cells.push(cell_run);
                stopped_at
            }
            None => None,
        };
        let stop_reason = cells.last().and_then(|cell_run| {
            cell_run
                .failure
                .clone()
                .map(|failure| stop_reason_for_failure(cell_run.cell_id.clone(), failure))
        });

        RuntimeRunReport {
            command: RuntimeRunCommand::RunCurrentCell {
                cell_id: cell_id.clone(),
            },
            cells,
            stopped_at,
            stop_reason,
            policy: self.policy,
            reproducibility: self.session.reproducibility(),
            snapshot: self.session.snapshot(),
        }
    }

    pub fn run_all(
        &mut self,
        document: &mut NotebookDocument,
        host: &dyn evaluator::RuntimeHost,
    ) -> RuntimeRunReport {
        self.clear_interrupt();
        self.run_cells_in_order(
            document,
            executable_cell_ids(document),
            host,
            RuntimeRunCommand::RunAll,
        )
    }

    pub fn restart_and_run_all(
        &mut self,
        document: &mut NotebookDocument,
        host: &dyn evaluator::RuntimeHost,
    ) -> RuntimeRunReport {
        self.clear_interrupt();
        self.session.restart();
        self.run_cells_in_order(
            document,
            executable_cell_ids(document),
            host,
            RuntimeRunCommand::RestartAndRunAll,
        )
    }

    /// Restart the evaluator, clearing live session state without re-running.
    ///
    /// Saved outputs remain in the document but are marked disconnected from the
    /// current session (`StaleReason::EvaluatorRestarted`), and previously run
    /// executable cells become stale. Use `restart_and_run_all` to also rebuild
    /// state from source.
    pub fn restart(&mut self, document: &mut NotebookDocument) -> RuntimeRunReport {
        self.clear_interrupt();
        self.session.restart();
        mark_document_session_disconnected(document);

        RuntimeRunReport {
            command: RuntimeRunCommand::Restart,
            cells: Vec::new(),
            stopped_at: None,
            stop_reason: None,
            policy: self.policy,
            reproducibility: self.session.reproducibility(),
            snapshot: self.session.snapshot(),
        }
    }

    pub fn mark_cell_source_edited(
        &self,
        document: &mut NotebookDocument,
        cell_id: &NotebookCellId,
        new_source: impl Into<String>,
    ) -> bool {
        let Some(edited_index) = document.blocks.iter().position(|block| {
            block.id == *cell_id && matches!(block.kind, NotebookBlockKind::Executable(_))
        }) else {
            return false;
        };

        if let NotebookBlockKind::Executable(cell) = &mut document.blocks[edited_index].kind {
            cell.source = new_source.into();
        }

        for block in document.blocks.iter_mut().skip(edited_index) {
            let reason = if block.id == *cell_id {
                StaleReason::SourceEdited
            } else {
                StaleReason::EarlierCellEdited {
                    cell_id: cell_id.clone(),
                }
            };
            mark_block_stale(block, reason);
        }

        true
    }

    fn run_cells_in_order(
        &mut self,
        document: &mut NotebookDocument,
        cell_ids: Vec<NotebookCellId>,
        host: &dyn evaluator::RuntimeHost,
        command: RuntimeRunCommand,
    ) -> RuntimeRunReport {
        let mut cells = Vec::new();
        let mut stopped_at = None;
        let mut stop_reason = None;

        for cell_id in cell_ids {
            let Some(cell_run) = self.run_cell(document, &cell_id, host) else {
                continue;
            };
            let failure = cell_run.failure.clone();
            let failed = failure.is_some();
            if failed {
                stopped_at = Some(cell_run.cell_id.clone());
                stop_reason = failure
                    .map(|failure| stop_reason_for_failure(cell_run.cell_id.clone(), failure));
            }
            cells.push(cell_run);
            if failed && self.policy.stop_on_first_error {
                break;
            }
        }

        RuntimeRunReport {
            command,
            cells,
            stopped_at,
            stop_reason,
            policy: self.policy,
            reproducibility: self.session.reproducibility(),
            snapshot: self.session.snapshot(),
        }
    }

    fn run_cell(
        &mut self,
        document: &mut NotebookDocument,
        cell_id: &NotebookCellId,
        host: &dyn evaluator::RuntimeHost,
    ) -> Option<RuntimeCellRun> {
        let block_index = document
            .blocks
            .iter()
            .position(|block| block.id == *cell_id)?;
        let source = match &mut document.blocks[block_index].kind {
            NotebookBlockKind::Executable(cell) => {
                cell.execution = ExecutionState::Running;
                cell.source.clone()
            }
            _ => return None,
        };

        let cell_source_hash = source_hash(&source);
        let host = CancellableRuntimeHost::new(host, self.cancellation_token.clone());
        let evaluation =
            self.session
                .evaluate_cell(cell_id.clone(), source, &host, &self.resource_limits);
        let reproducibility = OutputReproContext {
            source_hash: cell_source_hash,
            metadata: self.session.reproducibility(),
        };
        let output_batch = notebook_outputs_from_eval_response_with(
            cell_id,
            &evaluation.response,
            &self.output_limits,
            Some(&reproducibility),
        );
        let partial_output_count = evaluation.response.outputs.len();
        let failure = classify_eval_response(&evaluation.response);
        let partial_execution =
            partial_execution_for_response(&evaluation.response, partial_output_count, self.policy);

        let status =
            execution_state_from_status(evaluation.response.status, evaluation.snapshot.run_count);

        let block = &mut document.blocks[block_index];
        block.outputs = output_batch.outputs;
        if let NotebookBlockKind::Executable(cell) = &mut block.kind {
            cell.execution = status.clone();
        }

        Some(RuntimeCellRun {
            cell_id: cell_id.clone(),
            status,
            output_completion: output_batch.completion,
            failure,
            partial_execution,
        })
    }
}

fn executable_cell_ids(document: &NotebookDocument) -> Vec<NotebookCellId> {
    document
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, NotebookBlockKind::Executable(_)))
        .map(|block| block.id.clone())
        .collect()
}

fn execution_state_from_status(status: evaluator::EvalStatus, run_count: u64) -> ExecutionState {
    match status {
        evaluator::EvalStatus::Complete => ExecutionState::Complete { run_count },
        evaluator::EvalStatus::Failed
        | evaluator::EvalStatus::Cancelled
        | evaluator::EvalStatus::ResourceLimitExceeded => ExecutionState::Failed { run_count },
    }
}

/// Mark every executable cell and saved output as disconnected from the live
/// session after an evaluator restart. Cells that have run (or are already
/// stale) gain an `EvaluatorRestarted` reason; untouched idle cells are left
/// alone. All existing outputs are flagged so the UI can show they no longer
/// correspond to live session state.
fn mark_document_session_disconnected(document: &mut NotebookDocument) {
    for block in document.blocks.iter_mut() {
        if let NotebookBlockKind::Executable(cell) = &mut block.kind {
            match &mut cell.execution {
                ExecutionState::Complete { .. } | ExecutionState::Failed { .. } => {
                    cell.execution = ExecutionState::Stale {
                        reasons: vec![StaleReason::EvaluatorRestarted],
                    };
                }
                ExecutionState::Stale { reasons } => {
                    push_unique_reason(reasons, StaleReason::EvaluatorRestarted);
                }
                ExecutionState::Idle | ExecutionState::Queued | ExecutionState::Running => {}
            }
        }

        for output in &mut block.outputs {
            push_unique_reason(&mut output.stale, StaleReason::EvaluatorRestarted);
        }
    }
}

fn mark_block_stale(block: &mut NotebookBlock, reason: StaleReason) {
    let NotebookBlockKind::Executable(ExecutableCell { execution, .. }) = &mut block.kind else {
        return;
    };

    match execution {
        ExecutionState::Stale { reasons } => push_unique_reason(reasons, reason.clone()),
        _ => {
            *execution = ExecutionState::Stale {
                reasons: vec![reason.clone()],
            };
        }
    }

    for output in &mut block.outputs {
        push_unique_reason(&mut output.stale, reason.clone());
    }
}

fn push_unique_reason(reasons: &mut Vec<StaleReason>, reason: StaleReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NotebookId, RuntimeSessionId};
    use evaluator::{
        AttachmentValue, EvalAttachmentId, EvalContextDelta, EvalDiagnostic, EvalOutput,
        EvalRequest, EvalResponse, EvalStatus, EvalValue, Evaluator, EvaluatorMetadata, HostError,
        RuntimeHost, SessionSnapshot, SessionStatus, VariableSummary,
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

    struct CancellingRuntimeHost;

    impl RuntimeHost for CancellingRuntimeHost {
        fn resolve_attachment(&self, _name_or_id: &str) -> Result<AttachmentValue, HostError> {
            Err(HostError::not_found("no attachments"))
        }

        fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
            Err(HostError::not_found("no attachments"))
        }

        fn should_cancel(&self) -> bool {
            true
        }
    }

    struct OrderedEvaluator {
        snapshot: SessionSnapshot,
    }

    impl OrderedEvaluator {
        fn new() -> Self {
            Self {
                snapshot: SessionSnapshot::default(),
            }
        }
    }

    impl Evaluator for OrderedEvaluator {
        fn metadata(&self) -> EvaluatorMetadata {
            let mut metadata = EvaluatorMetadata::new("ordered", "Ordered");
            metadata.features.supports_shared_state = true;
            metadata.features.supports_variable_delete = true;
            metadata
        }

        fn evaluate_cell(&mut self, request: EvalRequest, host: &dyn RuntimeHost) -> EvalResponse {
            self.snapshot.run_count += 1;
            self.snapshot.status = SessionStatus::Idle;

            if request.source.contains("cancel") && host.should_cancel() {
                return EvalResponse {
                    status: EvalStatus::Cancelled,
                    outputs: Vec::new(),
                    diagnostics: vec![EvalDiagnostic {
                        severity: evaluator::EvalDiagnosticSeverity::Error,
                        message: "cancelled".to_string(),
                        span: None,
                        code: Some("CANCELLED".to_string()),
                    }],
                    context_delta: None,
                    session: Some(self.snapshot.clone()),
                };
            }

            if request.source.contains("limit")
                && request.context.resource_limits.max_loop_iterations == Some(1)
            {
                return EvalResponse {
                    status: EvalStatus::ResourceLimitExceeded,
                    outputs: Vec::new(),
                    diagnostics: vec![EvalDiagnostic {
                        severity: evaluator::EvalDiagnosticSeverity::Error,
                        message: "loop limit exceeded".to_string(),
                        span: None,
                        code: Some("LOOP_LIMIT".to_string()),
                    }],
                    context_delta: None,
                    session: Some(self.snapshot.clone()),
                };
            }

            if request.source.contains("fail") {
                return EvalResponse {
                    status: EvalStatus::Failed,
                    outputs: vec![EvalOutput::display(
                        EvalValue::Text(evaluator::TextValue {
                            stream: evaluator::EvalTextStream::Stdout,
                            text: "before failure".to_string(),
                        }),
                        request.cell_id,
                    )],
                    diagnostics: vec![EvalDiagnostic::error("failed")],
                    context_delta: None,
                    session: Some(self.snapshot.clone()),
                };
            }

            self.snapshot.variables.push(VariableSummary {
                name: format!("v{}", self.snapshot.run_count),
                kind: evaluator::ValueKind::Number,
                preview: self.snapshot.run_count.to_string(),
                source_cell: Some(request.cell_id.clone()),
                updated_at_run: Some(self.snapshot.run_count),
                stale: false,
                size_hint: None,
            });

            EvalResponse {
                status: EvalStatus::Complete,
                outputs: vec![EvalOutput::display(
                    EvalValue::Text(evaluator::TextValue {
                        stream: evaluator::EvalTextStream::Stdout,
                        text: format!("ran {}", request.source),
                    }),
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

        fn delete_variable(&mut self, name: &str) -> EvalResponse {
            self.snapshot
                .variables
                .retain(|variable| variable.name != name);
            EvalResponse {
                status: EvalStatus::Complete,
                outputs: Vec::new(),
                diagnostics: Vec::new(),
                context_delta: Some(EvalContextDelta::VariablesRemoved(vec![name.to_string()])),
                session: Some(self.snapshot.clone()),
            }
        }
    }

    fn runtime() -> NotebookRuntime {
        NotebookRuntime::new(EvaluatorSession::new(
            RuntimeSessionId::new("session-1"),
            NotebookId::new("notebook-1"),
            Box::new(OrderedEvaluator::new()),
        ))
    }

    fn document_with_sources(sources: &[&str]) -> NotebookDocument {
        let mut document = NotebookDocument::new(NotebookId::new("notebook-1"), "Runtime");
        for (index, source) in sources.iter().enumerate() {
            document.add_block(NotebookBlock::executable(
                NotebookCellId::new(format!("cell-{}", index + 1)),
                "ordered",
                *source,
            ));
        }
        document
    }

    #[test]
    fn run_current_cell_replaces_outputs_and_updates_execution_state() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        let report = runtime.run_current_cell(&mut document, &NotebookCellId::new("cell-1"), &host);

        assert_eq!(report.cells.len(), 1);
        assert!(matches!(
            document.blocks[0].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Complete { run_count: 1 },
                ..
            })
        ));
        assert_eq!(document.blocks[0].outputs.len(), 1);
    }

    #[test]
    fn run_all_stops_at_first_failed_cell() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1", "fail", "b := 2"]);

        let report = runtime.run_all(&mut document, &host);

        assert_eq!(report.cells.len(), 2);
        assert_eq!(report.stopped_at, Some(NotebookCellId::new("cell-2")));
        assert!(matches!(
            report.stop_reason,
            Some(RuntimeStopReason::FailedCell { ref cell_id, ref failure })
                if *cell_id == NotebookCellId::new("cell-2")
                    && failure.class == crate::RuntimeErrorClass::Runtime
        ));
        assert_eq!(
            report.cells[1]
                .partial_execution
                .as_ref()
                .expect("partial execution")
                .outputs_before_failure,
            1
        );
        assert!(matches!(
            document.blocks[1].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Failed { run_count: 2 },
                ..
            })
        ));
        assert!(matches!(
            document.blocks[2].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Idle,
                ..
            })
        ));
        assert_eq!(document.blocks[1].outputs.len(), 2);
    }

    #[test]
    fn execution_policy_can_continue_after_failed_cell() {
        let mut runtime = runtime().with_execution_policy(RuntimeExecutionPolicy {
            stop_on_first_error: false,
            transactional_cell_execution: false,
        });
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1", "fail", "b := 2"]);

        let report = runtime.run_all(&mut document, &host);

        assert_eq!(report.cells.len(), 3);
        assert_eq!(report.stopped_at, Some(NotebookCellId::new("cell-2")));
        assert!(matches!(
            document.blocks[2].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Complete { run_count: 3 },
                ..
            })
        ));
    }

    #[test]
    fn runtime_passes_resource_limits_to_evaluator() {
        let mut runtime = runtime().with_resource_limits(RuntimeResourceLimits {
            max_loop_iterations: Some(1),
            ..RuntimeResourceLimits::default()
        });
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["limit"]);

        let report = runtime.run_all(&mut document, &host);

        assert!(matches!(
            report.stop_reason,
            Some(RuntimeStopReason::ResourceLimitExceeded { ref failure, .. })
                if failure.class == crate::RuntimeErrorClass::ResourceLimitExceeded
        ));
        assert!(matches!(
            report.cells[0].status,
            ExecutionState::Failed { run_count: 1 }
        ));
    }

    #[test]
    fn runtime_host_cancellation_is_reported_as_cancelled() {
        let mut runtime = runtime();
        let host = CancellingRuntimeHost;
        let mut document = document_with_sources(&["cancel"]);

        let report = runtime.run_all(&mut document, &host);

        assert!(matches!(
            report.stop_reason,
            Some(RuntimeStopReason::Cancelled { ref cell_id })
                if *cell_id == NotebookCellId::new("cell-1")
        ));
        assert!(matches!(
            report.cells[0].failure.as_ref().expect("failure").class,
            crate::RuntimeErrorClass::Cancelled
        ));
    }

    #[test]
    fn restart_and_run_all_rebuilds_session_from_zero() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        runtime.run_all(&mut document, &host);
        let report = runtime.restart_and_run_all(&mut document, &host);

        assert_eq!(report.snapshot.run_count, 1);
        assert_eq!(report.snapshot.bindings.len(), 1);
        assert!(matches!(
            document.blocks[0].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Complete { run_count: 1 },
                ..
            })
        ));
    }

    #[test]
    fn restart_marks_outputs_and_cells_session_disconnected() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        runtime.run_all(&mut document, &host);
        assert_eq!(document.blocks[0].outputs.len(), 1);

        let report = runtime.restart(&mut document);

        assert_eq!(report.command, RuntimeRunCommand::Restart);
        assert_eq!(report.snapshot.run_count, 0);
        assert_eq!(report.snapshot.bindings.len(), 0);
        assert!(matches!(
            document.blocks[0].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Stale { .. },
                ..
            })
        ));
        if let NotebookBlockKind::Executable(cell) = &document.blocks[0].kind {
            assert!(matches!(
                &cell.execution,
                ExecutionState::Stale { reasons }
                    if reasons.contains(&StaleReason::EvaluatorRestarted)
            ));
        }
        assert!(
            document.blocks[0].outputs[0]
                .stale
                .contains(&StaleReason::EvaluatorRestarted)
        );
    }

    #[test]
    fn run_records_source_hash_and_reproducibility_in_provenance() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        let report = runtime.run_all(&mut document, &host);

        let provenance = &document.blocks[0].outputs[0].provenance;
        assert_eq!(provenance.input_hash, Some(source_hash("a := 1")));
        assert!(
            provenance
                .notes
                .iter()
                .any(|note| note.contains("runtime=") && note.contains("evaluator=ordered"))
        );
        assert_eq!(report.reproducibility.evaluator_language_id, "ordered");
        assert_eq!(report.reproducibility.run_count, 1);
    }

    #[test]
    fn source_edit_marks_edited_and_later_executable_cells_stale() {
        let runtime = runtime();
        let mut document = document_with_sources(&["a := 1", "b := a", "c := b"]);
        for (index, block) in document.blocks.iter_mut().enumerate() {
            if let NotebookBlockKind::Executable(cell) = &mut block.kind {
                cell.execution = ExecutionState::Complete {
                    run_count: index as u64 + 1,
                };
            }
        }

        assert!(runtime.mark_cell_source_edited(
            &mut document,
            &NotebookCellId::new("cell-2"),
            "b := a + 1",
        ));

        assert!(matches!(
            document.blocks[0].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Complete { .. },
                ..
            })
        ));
        assert!(matches!(
            document.blocks[1].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Stale { .. },
                ..
            })
        ));
        assert!(matches!(
            document.blocks[2].kind,
            NotebookBlockKind::Executable(ExecutableCell {
                execution: ExecutionState::Stale { .. },
                ..
            })
        ));
    }

    #[test]
    fn inspection_snapshot_reports_current_variables() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        runtime.run_all(&mut document, &host);
        let inspection = runtime.inspect_session();

        assert_eq!(inspection.variables.len(), 1);
        assert_eq!(inspection.variables[0].name, "v1");
        assert_eq!(
            inspection.variables[0].source_cell,
            Some(NotebookCellId::new("cell-1"))
        );
        assert_eq!(inspection.functions.len(), 0);
    }

    #[test]
    fn delete_variable_updates_inspection_snapshot_when_supported() {
        let mut runtime = runtime();
        let host = EmptyRuntimeHost;
        let mut document = document_with_sources(&["a := 1"]);

        runtime.run_all(&mut document, &host);
        let delete = runtime.delete_variable("v1");

        assert!(matches!(
            delete.status,
            crate::RuntimeDeleteVariableStatus::Deleted
        ));
        assert_eq!(runtime.inspect_session().variables.len(), 0);
    }
}
