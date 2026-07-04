//! Token definitions for the Poincare language.

use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Literals. Numeric literals keep their raw text so no precision is lost
    // at parse time (a forward-compatibility requirement for a future exact
    // number tower).
    Int(String),
    Float(String),
    Str(String),
    Ident(String),

    // Keywords in use in V1.
    Fn,
    For,
    In,
    If,
    Else,
    And,
    Or,
    Not,
    Plot,
    Over,
    True,
    False,

    // Reserved for the typed future: tokenized so they cannot be repurposed,
    // rejected by the parser if used. (`Type`, `match`, `forall`, `let`,
    // `fun`.) `data` is deliberately not reserved — too common a variable name.
    Reserved(&'static str),

    // Operators and punctuation.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Dot,      // `.` or `∘` — composition
    Pipe,     // `|>`
    Arrow,    // `->` — type/signature arrow only
    FatArrow, // `=>` — lambda
    Eq,       // `=` — binding / definition
    DotDot,   // `..` — range
    Colon,    // `:` — type ascription / field separator
    Comma,
    Semicolon,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,

    /// A significant newline. Suppressed inside `()`/`[]` and after
    /// continuation tokens; acts as a statement/field separator elsewhere.
    Newline,
    Eof,
}

impl TokenKind {
    /// A short human-readable description for diagnostics.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Int(_) => "integer literal".into(),
            TokenKind::Float(_) => "float literal".into(),
            TokenKind::Str(_) => "string literal".into(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Fn => "`fn`".into(),
            TokenKind::For => "`for`".into(),
            TokenKind::In => "`in`".into(),
            TokenKind::If => "`if`".into(),
            TokenKind::Else => "`else`".into(),
            TokenKind::And => "`and`".into(),
            TokenKind::Or => "`or`".into(),
            TokenKind::Not => "`not`".into(),
            TokenKind::Plot => "`plot`".into(),
            TokenKind::Over => "`over`".into(),
            TokenKind::True => "`true`".into(),
            TokenKind::False => "`false`".into(),
            TokenKind::Reserved(w) => format!("reserved keyword `{w}`"),
            TokenKind::Plus => "`+`".into(),
            TokenKind::Minus => "`-`".into(),
            TokenKind::Star => "`*`".into(),
            TokenKind::Slash => "`/`".into(),
            TokenKind::Percent => "`%`".into(),
            TokenKind::Caret => "`^`".into(),
            TokenKind::EqEq => "`==`".into(),
            TokenKind::NotEq => "`!=`".into(),
            TokenKind::Lt => "`<`".into(),
            TokenKind::Le => "`<=`".into(),
            TokenKind::Gt => "`>`".into(),
            TokenKind::Ge => "`>=`".into(),
            TokenKind::Dot => "`.`".into(),
            TokenKind::Pipe => "`|>`".into(),
            TokenKind::Arrow => "`->`".into(),
            TokenKind::FatArrow => "`=>`".into(),
            TokenKind::Eq => "`=`".into(),
            TokenKind::DotDot => "`..`".into(),
            TokenKind::Colon => "`:`".into(),
            TokenKind::Comma => "`,`".into(),
            TokenKind::Semicolon => "`;`".into(),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::LBracket => "`[`".into(),
            TokenKind::RBracket => "`]`".into(),
            TokenKind::LBrace => "`{`".into(),
            TokenKind::RBrace => "`}`".into(),
            TokenKind::Newline => "newline".into(),
            TokenKind::Eof => "end of input".into(),
        }
    }

    /// Whether a newline immediately after this token should be suppressed
    /// because the token cannot end a statement (line-continuation).
    pub fn is_continuation(&self) -> bool {
        matches!(
            self,
            TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::Caret
                | TokenKind::EqEq
                | TokenKind::NotEq
                | TokenKind::Lt
                | TokenKind::Le
                | TokenKind::Gt
                | TokenKind::Ge
                | TokenKind::Dot
                | TokenKind::Pipe
                | TokenKind::Arrow
                | TokenKind::FatArrow
                | TokenKind::Eq
                | TokenKind::DotDot
                | TokenKind::Colon
                | TokenKind::Comma
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::Not
                | TokenKind::LParen
                | TokenKind::LBracket
        )
    }
}
