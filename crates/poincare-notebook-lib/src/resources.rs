use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::RuntimeOutputLimits;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeResourceLimits {
    pub max_loop_iterations: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
    pub max_output_count: usize,
    pub max_text_chars: usize,
    pub max_table_rows: usize,
    pub max_graph_outputs: usize,
}

impl Default for RuntimeResourceLimits {
    fn default() -> Self {
        Self {
            max_loop_iterations: Some(1_000_000),
            max_wall_time_ms: Some(5_000),
            max_output_count: 64,
            max_text_chars: 64 * 1024,
            max_table_rows: 1_000,
            max_graph_outputs: 16,
        }
    }
}

impl RuntimeResourceLimits {
    pub fn output_limits(&self) -> RuntimeOutputLimits {
        RuntimeOutputLimits {
            max_outputs_per_cell: self.max_output_count,
            max_text_chars: self.max_text_chars,
            max_table_rows: self.max_table_rows,
            max_graph_outputs: self.max_graph_outputs,
        }
    }
}

impl From<&RuntimeResourceLimits> for evaluator::EvalResourceLimits {
    fn from(value: &RuntimeResourceLimits) -> Self {
        Self {
            max_loop_iterations: value.max_loop_iterations,
            max_wall_time_ms: value.max_wall_time_ms,
            max_output_count: Some(value.max_output_count),
            max_text_chars: Some(value.max_text_chars),
            max_table_rows: Some(value.max_table_rows),
            max_graph_outputs: Some(value.max_graph_outputs),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl RuntimeCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub struct CancellableRuntimeHost<'a> {
    inner: &'a dyn evaluator::RuntimeHost,
    token: RuntimeCancellationToken,
}

impl<'a> CancellableRuntimeHost<'a> {
    pub fn new(inner: &'a dyn evaluator::RuntimeHost, token: RuntimeCancellationToken) -> Self {
        Self { inner, token }
    }
}

impl evaluator::RuntimeHost for CancellableRuntimeHost<'_> {
    fn resolve_attachment(
        &self,
        name_or_id: &str,
    ) -> Result<evaluator::AttachmentValue, evaluator::HostError> {
        self.inner.resolve_attachment(name_or_id)
    }

    fn attachment_bytes(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<Vec<u8>, evaluator::HostError> {
        self.inner.attachment_bytes(attachment)
    }

    fn attachment_text(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<String, evaluator::HostError> {
        self.inner.attachment_text(attachment)
    }

    fn attachment_table(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<evaluator::TableValue, evaluator::HostError> {
        self.inner.attachment_table(attachment)
    }

    fn attachment_array(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<evaluator::ArrayValue, evaluator::HostError> {
        self.inner.attachment_array(attachment)
    }

    fn should_cancel(&self) -> bool {
        self.token.is_cancelled() || self.inner.should_cancel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evaluator::{AttachmentValue, EvalAttachmentId, HostError, RuntimeHost};

    struct Host;

    impl RuntimeHost for Host {
        fn resolve_attachment(&self, _name_or_id: &str) -> Result<AttachmentValue, HostError> {
            Err(HostError::not_found("none"))
        }

        fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
            Err(HostError::not_found("none"))
        }
    }

    #[test]
    fn cancellation_token_flows_through_host_wrapper() {
        let token = RuntimeCancellationToken::new();
        let host = Host;
        let wrapper = CancellableRuntimeHost::new(&host, token.clone());
        assert!(!wrapper.should_cancel());

        token.cancel();
        assert!(wrapper.should_cancel());

        token.reset();
        assert!(!wrapper.should_cancel());
    }

    #[test]
    fn resource_limits_convert_to_eval_context_limits() {
        let limits = RuntimeResourceLimits {
            max_loop_iterations: Some(10),
            max_wall_time_ms: Some(20),
            max_output_count: 3,
            max_text_chars: 4,
            max_table_rows: 5,
            max_graph_outputs: 6,
        };
        let eval_limits = evaluator::EvalResourceLimits::from(&limits);

        assert_eq!(eval_limits.max_loop_iterations, Some(10));
        assert_eq!(eval_limits.max_wall_time_ms, Some(20));
        assert_eq!(eval_limits.max_output_count, Some(3));
        assert_eq!(eval_limits.max_graph_outputs, Some(6));
    }
}
