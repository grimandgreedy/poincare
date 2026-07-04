use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::{NotebookCellId, SourcePosition, SourceSpan};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeErrorClass {
    Parse,
    NameResolution,
    TypeValue,
    Runtime,
    Cancelled,
    ResourceLimitExceeded,
    AttachmentResolution,
    GraphBuildRender,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFailure {
    pub class: RuntimeErrorClass,
    pub message: String,
    pub code: Option<String>,
    pub source_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePartialExecution {
    pub outputs_before_failure: usize,
    pub state_may_have_changed: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeStopReason {
    FailedCell {
        cell_id: NotebookCellId,
        failure: RuntimeFailure,
    },
    Cancelled {
        cell_id: NotebookCellId,
    },
    ResourceLimitExceeded {
        cell_id: NotebookCellId,
        failure: RuntimeFailure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeExecutionPolicy {
    pub stop_on_first_error: bool,
    pub transactional_cell_execution: bool,
}

impl Default for RuntimeExecutionPolicy {
    fn default() -> Self {
        Self {
            stop_on_first_error: true,
            // Evaluators may implement transactionality internally later, but
            // the runtime cannot generally roll back opaque backend state.
            transactional_cell_execution: false,
        }
    }
}

pub fn classify_eval_response(response: &evaluator::EvalResponse) -> Option<RuntimeFailure> {
    match response.status {
        evaluator::EvalStatus::Complete => None,
        evaluator::EvalStatus::Cancelled => Some(RuntimeFailure {
            class: RuntimeErrorClass::Cancelled,
            message: diagnostic_message(response)
                .unwrap_or_else(|| "evaluation cancelled".to_string()),
            code: diagnostic_code(response),
            source_span: diagnostic_span(response),
        }),
        evaluator::EvalStatus::ResourceLimitExceeded => Some(RuntimeFailure {
            class: RuntimeErrorClass::ResourceLimitExceeded,
            message: diagnostic_message(response)
                .unwrap_or_else(|| "resource limit exceeded".to_string()),
            code: diagnostic_code(response),
            source_span: diagnostic_span(response),
        }),
        evaluator::EvalStatus::Failed => {
            let diagnostic = response.diagnostics.first();
            let code = diagnostic.and_then(|diagnostic| diagnostic.code.clone());
            Some(RuntimeFailure {
                class: classify_error_code(code.as_deref()),
                message: diagnostic
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "evaluation failed".to_string()),
                code,
                source_span: diagnostic.and_then(|diagnostic| diagnostic.span.map(source_span)),
            })
        }
    }
}

pub fn partial_execution_for_response(
    response: &evaluator::EvalResponse,
    outputs_before_failure: usize,
    policy: RuntimeExecutionPolicy,
) -> Option<RuntimePartialExecution> {
    if response.status == evaluator::EvalStatus::Complete {
        return None;
    }

    let has_visible_partial_outputs = outputs_before_failure > 0;
    let state_may_have_changed =
        !policy.transactional_cell_execution && response.context_delta.is_some();

    if !has_visible_partial_outputs && !state_may_have_changed {
        return None;
    }

    Some(RuntimePartialExecution {
        outputs_before_failure,
        state_may_have_changed,
        message: if state_may_have_changed {
            "cell failed after producing output or mutating session state".to_string()
        } else {
            "cell failed after producing partial output".to_string()
        },
    })
}

pub fn stop_reason_for_failure(
    cell_id: NotebookCellId,
    failure: RuntimeFailure,
) -> RuntimeStopReason {
    match failure.class {
        RuntimeErrorClass::Cancelled => RuntimeStopReason::Cancelled { cell_id },
        RuntimeErrorClass::ResourceLimitExceeded => {
            RuntimeStopReason::ResourceLimitExceeded { cell_id, failure }
        }
        _ => RuntimeStopReason::FailedCell { cell_id, failure },
    }
}

fn classify_error_code(code: Option<&str>) -> RuntimeErrorClass {
    let Some(code) = code.map(|code| code.to_ascii_uppercase()) else {
        return RuntimeErrorClass::Runtime;
    };

    if code.contains("PARSE") || code.contains("SYNTAX") {
        RuntimeErrorClass::Parse
    } else if code.contains("NAME") || code.contains("RESOLUTION") || code.contains("UNBOUND") {
        RuntimeErrorClass::NameResolution
    } else if code.contains("TYPE") || code.contains("VALUE") {
        RuntimeErrorClass::TypeValue
    } else if code.contains("CANCEL") || code.contains("INTERRUPT") {
        RuntimeErrorClass::Cancelled
    } else if code.contains("LIMIT") || code.contains("RESOURCE") {
        RuntimeErrorClass::ResourceLimitExceeded
    } else if code.contains("ATTACH") || code.contains("IO") {
        RuntimeErrorClass::AttachmentResolution
    } else if code.contains("GRAPH") || code.contains("PLOT") || code.contains("RENDER") {
        RuntimeErrorClass::GraphBuildRender
    } else if code.contains("UNSUPPORTED") {
        RuntimeErrorClass::Unsupported
    } else {
        RuntimeErrorClass::Runtime
    }
}

fn diagnostic_message(response: &evaluator::EvalResponse) -> Option<String> {
    response
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
}

fn diagnostic_code(response: &evaluator::EvalResponse) -> Option<String> {
    response
        .diagnostics
        .first()
        .and_then(|diagnostic| diagnostic.code.clone())
}

fn diagnostic_span(response: &evaluator::EvalResponse) -> Option<SourceSpan> {
    response
        .diagnostics
        .first()
        .and_then(|diagnostic| diagnostic.span.map(source_span))
}

fn source_span(span: evaluator::SourceSpan) -> SourceSpan {
    SourceSpan {
        start: SourcePosition {
            line: span.start.line,
            column: span.start.column,
            offset: span.start.offset,
        },
        end: SourcePosition {
            line: span.end.line,
            column: span.end.column,
            offset: span.end.offset,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evaluator::{
        EvalCellId, EvalDiagnostic, EvalDiagnosticSeverity, EvalOutput, EvalResponse, EvalStatus,
        EvalValue,
    };

    #[test]
    fn classifies_diagnostic_codes() {
        let response = EvalResponse {
            status: EvalStatus::Failed,
            outputs: Vec::new(),
            diagnostics: vec![EvalDiagnostic {
                severity: EvalDiagnosticSeverity::Error,
                message: "bad syntax".to_string(),
                span: None,
                code: Some("PARSE001".to_string()),
            }],
            context_delta: None,
            session: None,
        };

        let failure = classify_eval_response(&response).expect("failure");
        assert_eq!(failure.class, RuntimeErrorClass::Parse);
        assert_eq!(failure.message, "bad syntax");
    }

    #[test]
    fn marks_partial_execution_when_failed_response_has_outputs() {
        let response = EvalResponse {
            status: EvalStatus::Failed,
            outputs: vec![EvalOutput::display(
                EvalValue::String("partial".to_string()),
                EvalCellId::new("cell-1"),
            )],
            diagnostics: vec![EvalDiagnostic::error("failed")],
            context_delta: None,
            session: None,
        };

        let partial =
            partial_execution_for_response(&response, 1, RuntimeExecutionPolicy::default())
                .expect("partial");

        assert_eq!(partial.outputs_before_failure, 1);
        assert!(!partial.state_may_have_changed);
    }
}
