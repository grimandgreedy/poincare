//! Source spans and offset-to-line/column resolution.
//!
//! Spans are byte-offset ranges into the original source. Every AST and token
//! node carries a span; a `SourceMap` resolves offsets to 1-based line/column
//! for diagnostics. Keeping spans on every node from the start is a
//! forward-compatibility requirement for a future typechecker (see
//! `docs/plans/poincare-notebook/poincare-language-roadmap.md`).

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into the source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// A placeholder span, used for synthesized nodes with no source text.
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }
}

/// A 1-based source location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

/// Resolves byte offsets in a source string to line/column locations.
#[derive(Clone, Debug)]
pub struct SourceMap<'a> {
    src: &'a str,
    /// Byte offset of the start of each line (line 0 starts at offset 0).
    line_starts: Vec<u32>,
}

impl<'a> SourceMap<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i as u32) + 1);
            }
        }
        Self { src, line_starts }
    }

    /// The 1-based line/column for a byte offset. Columns count Unicode scalar
    /// values from the start of the line.
    pub fn location(&self, offset: u32) -> Location {
        let clamped = offset.min(self.src.len() as u32);
        let line_idx = match self.line_starts.binary_search(&clamped) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[line_idx];
        let column = self.src[line_start as usize..clamped as usize]
            .chars()
            .count() as u32;
        Location {
            line: line_idx as u32 + 1,
            column: column + 1,
            offset,
        }
    }

    /// The source text covered by a span.
    pub fn snippet(&self, span: Span) -> &'a str {
        let start = (span.start as usize).min(self.src.len());
        let end = (span.end as usize).min(self.src.len());
        &self.src[start..end]
    }
}
