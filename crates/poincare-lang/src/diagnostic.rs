//! Parse and lex diagnostics.

use crate::span::{SourceMap, Span};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

/// A single diagnostic with a message and the span it refers to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
        }
    }

    /// Render as `line:col: message` using a source map.
    pub fn render(&self, map: &SourceMap) -> String {
        let loc = map.location(self.span.start);
        format!("{}:{}: {}", loc.line, loc.column, self.message)
    }
}
