//! Abstract syntax tree for the Poincare language.
//!
//! Every node carries a `Span`. Names use the `Symbol` newtype so a future
//! move to interned, path-capable names is a drop-in change (see the
//! Forward-Compatibility section of the language roadmap).

use crate::span::Span;
use serde::{Deserialize, Serialize};

/// An identifier name. A newtype today over `String`; intended to become an
/// interned, path-capable symbol without changing the AST shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Self {
        Symbol(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol(s.to_string())
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Symbol(s)
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A spanned identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ident {
    pub sym: Symbol,
    pub span: Span,
}

/// A parsed program (or cell): a flat sequence of statements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Signature(Signature),
    Binding(Binding),
    Func(FuncDef),
    For(ForStmt),
    Plot(PlotStmt),
    Expr(Expr),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Signature(s) => s.span,
            Stmt::Binding(b) => b.span,
            Stmt::Func(f) => f.span,
            Stmt::For(f) => f.span,
            Stmt::Plot(p) => p.span,
            Stmt::Expr(e) => e.span(),
        }
    }
}

/// `name : T1 -> T2 -> ...` — an optional type ascription that drives
/// type-directed plotting. Types are stored as (restricted) expressions so the
/// type position can grow into full expressions later without a separate
/// type-AST.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    pub name: Ident,
    pub types: Vec<Expr>,
    pub span: Span,
}

/// `name = expr`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuncKind {
    /// `f(x, y) = expr`
    Expr,
    /// `fn f(x, y) { ... }`
    Block,
}

/// A function definition. Both `f(x) = e` and `fn f(x) { ... }` produce this;
/// the block form stores an `Expr::Block` body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: Ident,
    pub params: Vec<Ident>,
    pub body: Expr,
    pub kind: FuncKind,
    pub span: Span,
}

/// `for var in iter { body }`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForStmt {
    pub var: Ident,
    pub iter: Expr,
    pub body: Block,
    pub span: Span,
}

/// `plot kind? target? (over field, ...)? { field ... }?`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotStmt {
    pub kind: Option<Ident>,
    pub target: Option<Expr>,
    pub over: Vec<Field>,
    pub fields: Vec<Field>,
    pub span: Span,
}

/// A `name = value` field used in `over` clauses and plot config blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

/// `{ stmts...; tail? }` — the tail expression is the block's value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

/// A call argument: positional, or named (`name = value`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Integer literal, raw text preserved losslessly.
    Int {
        raw: String,
        span: Span,
    },
    /// Float literal, raw text preserved losslessly.
    Float {
        raw: String,
        span: Span,
    },
    Str {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Ident(Ident),
    List {
        items: Vec<Expr>,
        span: Span,
    },
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `g . f` / `g ∘ f`
    Compose {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `x |> f`
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        span: Span,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    If(IfExpr),
    Block(Block),
    Lambda {
        params: Vec<Ident>,
        body: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int { span, .. }
            | Expr::Float { span, .. }
            | Expr::Str { span, .. }
            | Expr::Bool { span, .. }
            | Expr::List { span, .. }
            | Expr::Range { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Compose { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::Call { span, .. }
            | Expr::Index { span, .. }
            | Expr::Lambda { span, .. } => *span,
            Expr::Ident(i) => i.span,
            Expr::If(i) => i.span,
            Expr::Block(b) => b.span,
        }
    }
}

/// `if cond { then } else { ... }` — an expression.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then_block: Block,
    pub els: Option<Box<ElseBranch>>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ElseBranch {
    Block(Block),
    If(IfExpr),
}
