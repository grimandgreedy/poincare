//! Reproducibility metadata for notebook execution.
//!
//! Phase 8 of the runtime roadmap records enough provenance that a saved
//! notebook can describe *how* its outputs were produced: which evaluator and
//! version ran a cell, which runtime version drove it, the session run count,
//! and a content hash of the cell source. The source hash lets tooling compare
//! a saved output against the current cell source without re-running it.

use serde::{Deserialize, Serialize};

/// Version of the notebook runtime that produced an output. Tracks the
/// `poincare-notebook-lib` crate version so persisted outputs record the
/// runtime that generated them.
pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Deterministic content hash of a cell's source text.
///
/// Uses 64-bit FNV-1a so the value is stable across processes and platforms
/// (unlike `std::hash::DefaultHasher`, which is not guaranteed stable). The
/// `fnv1a64:` prefix documents the algorithm in persisted metadata.
pub fn source_hash(source: &str) -> String {
    format!("fnv1a64:{:016x}", fnv1a64(source.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Snapshot of the evaluator/runtime identity used to reconstruct or audit a
/// notebook run. Attached to run reports and, in condensed form, to output
/// provenance notes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityMetadata {
    pub evaluator_language_id: String,
    pub evaluator_display_name: String,
    pub evaluator_version: Option<String>,
    pub runtime_version: String,
    pub run_count: u64,
}

impl ReproducibilityMetadata {
    /// Condensed single-line description recorded in output provenance notes so
    /// saved outputs are self-describing without the surrounding run report.
    pub fn provenance_note(&self) -> String {
        let evaluator_version = self.evaluator_version.as_deref().unwrap_or("unknown");
        format!(
            "evaluator={} {}; runtime={}; run={}",
            self.evaluator_language_id, evaluator_version, self.runtime_version, self.run_count
        )
    }
}

/// Per-cell reproducibility context threaded into output mapping so each
/// produced output records the source hash and evaluator/runtime identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputReproContext {
    pub source_hash: String,
    pub metadata: ReproducibilityMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_hash_is_stable_and_sensitive_to_changes() {
        let a = source_hash("a := 1");
        let b = source_hash("a := 1");
        let c = source_hash("a := 2");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("fnv1a64:"));
    }

    #[test]
    fn provenance_note_describes_evaluator_and_runtime() {
        let metadata = ReproducibilityMetadata {
            evaluator_language_id: "poincare".to_string(),
            evaluator_display_name: "Poincare".to_string(),
            evaluator_version: Some("0.2.0".to_string()),
            runtime_version: "0.1.0".to_string(),
            run_count: 3,
        };

        assert_eq!(
            metadata.provenance_note(),
            "evaluator=poincare 0.2.0; runtime=0.1.0; run=3"
        );
    }
}
