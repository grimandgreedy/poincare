use poincare_evaluator::{AttachmentValue, EvalAttachmentId, HostError, RuntimeHost};

/// A runtime host with no attachments wired up yet. Attachment-backed data
/// loading in cells will resolve once the notebook bundle host is connected.
pub struct EmptyNotebookHost;

impl RuntimeHost for EmptyNotebookHost {
    fn resolve_attachment(&self, _name_or_id: &str) -> Result<AttachmentValue, HostError> {
        Err(HostError::not_found("attachments are not wired yet"))
    }

    fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
        Err(HostError::not_found("attachments are not wired yet"))
    }
}
