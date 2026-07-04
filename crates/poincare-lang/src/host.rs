//! Host interface for notebook-scoped resources.
//!
//! The interpreter reaches attachments only through a `Host`, never through the
//! filesystem directly. The notebook runtime provides a concrete host (bridged
//! from the evaluator's `RuntimeHost` in the Phase 6 adapter); tests can supply
//! a small in-memory host.

/// Access to notebook attachments, resolved by name or id.
pub trait Host {
    fn attachment_text(&self, name_or_id: &str) -> Result<String, String>;
    fn attachment_bytes(&self, name_or_id: &str) -> Result<Vec<u8>, String>;
}
