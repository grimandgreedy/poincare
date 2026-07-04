//! Untyped core IR and lowering from the surface AST.
//!
//! The interpreter runs this core, not the surface AST, so that a future
//! elaboration/typecheck pass can slot in between lowering and evaluation
//! without rewriting the interpreter (see the Forward-Compatibility section of
//! the language roadmap). Lowering desugars surface sugar:
//!
//! - expression and block function definitions both become `Lambda` bindings;
//! - `x |> f` becomes `f(x)`;
//! - `g . f` becomes `(v) => g(f(v))` with a fresh variable;
//! - `if`/`else` chains become nested `If` with a `Unit` default;
//! - signatures produce no runtime code;
//! - `plot` lowers to a `Todo` placeholder (executed once graph builtins land).

use crate::ast::{self, BinaryOp, ElseBranch, Expr, Stmt, UnaryOp};
use crate::span::Span;

#[derive(Clone, Debug)]
pub struct CoreProgram {
    pub stmts: Vec<CoreStmt>,
}

#[derive(Clone, Debug)]
pub enum CoreStmt {
    /// Bind or rebind a name in the current scope.
    Bind { name: String, value: Core, span: Span },
    /// Evaluate an expression for its value/effect.
    Expr(Core),
}

/// A (possibly named) call argument.
#[derive(Clone, Debug)]
pub struct CoreArg {
    pub name: Option<String>,
    pub value: Core,
}

#[derive(Clone, Debug)]
pub enum Core {
    Unit { span: Span },
    Num { value: f64, span: Span },
    Bool { value: bool, span: Span },
    Str { value: String, span: Span },
    Var { name: String, span: Span },
    List { items: Vec<Core>, span: Span },
    Range { lo: Box<Core>, hi: Box<Core>, span: Span },
    Unary { op: UnaryOp, expr: Box<Core>, span: Span },
    Binary { op: BinaryOp, lhs: Box<Core>, rhs: Box<Core>, span: Span },
    Lambda { params: Vec<String>, body: Box<Core>, span: Span },
    Apply { func: Box<Core>, args: Vec<CoreArg>, span: Span },
    Index { base: Box<Core>, index: Box<Core>, span: Span },
    If { cond: Box<Core>, then: Box<Core>, els: Box<Core>, span: Span },
    Block { stmts: Vec<CoreStmt>, tail: Option<Box<Core>>, span: Span },
    For { var: String, iter: Box<Core>, body: Box<Core>, span: Span },
    /// A construct not yet executable (currently `plot`).
    Todo { what: &'static str, span: Span },
}

impl Core {
    pub fn span(&self) -> Span {
        match self {
            Core::Unit { span }
            | Core::Num { span, .. }
            | Core::Bool { span, .. }
            | Core::Str { span, .. }
            | Core::Var { span, .. }
            | Core::List { span, .. }
            | Core::Range { span, .. }
            | Core::Unary { span, .. }
            | Core::Binary { span, .. }
            | Core::Lambda { span, .. }
            | Core::Apply { span, .. }
            | Core::Index { span, .. }
            | Core::If { span, .. }
            | Core::Block { span, .. }
            | Core::For { span, .. }
            | Core::Todo { span, .. } => *span,
        }
    }
}

/// Lower a parsed program into the core IR.
pub fn lower(program: &ast::Program) -> CoreProgram {
    let mut lowerer = Lowerer { fresh: 0 };
    CoreProgram {
        stmts: program.stmts.iter().filter_map(|s| lowerer.stmt(s)).collect(),
    }
}

struct Lowerer {
    fresh: u32,
}

impl Lowerer {
    fn fresh_name(&mut self) -> String {
        self.fresh += 1;
        // `$` cannot appear in a source identifier, so this cannot collide.
        format!("$c{}", self.fresh)
    }

    fn stmt(&mut self, stmt: &Stmt) -> Option<CoreStmt> {
        match stmt {
            // Signatures carry no runtime code.
            Stmt::Signature(_) => None,
            Stmt::Binding(b) => Some(CoreStmt::Bind {
                name: b.name.sym.as_str().to_string(),
                value: self.expr(&b.value),
                span: b.span,
            }),
            Stmt::Func(f) => Some(CoreStmt::Bind {
                name: f.name.sym.as_str().to_string(),
                value: Core::Lambda {
                    params: f.params.iter().map(param_name).collect(),
                    body: Box::new(self.expr(&f.body)),
                    span: f.span,
                },
                span: f.span,
            }),
            Stmt::For(f) => Some(CoreStmt::Expr(Core::For {
                var: f.var.sym.as_str().to_string(),
                iter: Box::new(self.expr(&f.iter)),
                body: Box::new(self.block(&f.body)),
                span: f.span,
            })),
            Stmt::Plot(p) => Some(CoreStmt::Expr(Core::Todo {
                what: "plot",
                span: p.span,
            })),
            Stmt::Expr(e) => Some(CoreStmt::Expr(self.expr(e))),
        }
    }

    fn block(&mut self, block: &ast::Block) -> Core {
        Core::Block {
            stmts: block.stmts.iter().filter_map(|s| self.stmt(s)).collect(),
            tail: block.tail.as_ref().map(|t| Box::new(self.expr(t))),
            span: block.span,
        }
    }

    fn expr(&mut self, expr: &Expr) -> Core {
        match expr {
            Expr::Int { raw, span } | Expr::Float { raw, span } => Core::Num {
                value: parse_number(raw),
                span: *span,
            },
            Expr::Str { value, span } => Core::Str {
                value: value.clone(),
                span: *span,
            },
            Expr::Bool { value, span } => Core::Bool {
                value: *value,
                span: *span,
            },
            Expr::Ident(id) => Core::Var {
                name: id.sym.as_str().to_string(),
                span: id.span,
            },
            Expr::List { items, span } => Core::List {
                items: items.iter().map(|i| self.expr(i)).collect(),
                span: *span,
            },
            Expr::Range { lo, hi, span } => Core::Range {
                lo: Box::new(self.expr(lo)),
                hi: Box::new(self.expr(hi)),
                span: *span,
            },
            Expr::Unary { op, expr, span } => Core::Unary {
                op: *op,
                expr: Box::new(self.expr(expr)),
                span: *span,
            },
            Expr::Binary { op, lhs, rhs, span } => Core::Binary {
                op: *op,
                lhs: Box::new(self.expr(lhs)),
                rhs: Box::new(self.expr(rhs)),
                span: *span,
            },
            // `g . f`  =>  (v) => g(f(v))
            Expr::Compose { lhs, rhs, span } => {
                let v = self.fresh_name();
                let inner = Core::Apply {
                    func: Box::new(self.expr(rhs)),
                    args: vec![CoreArg {
                        name: None,
                        value: Core::Var {
                            name: v.clone(),
                            span: *span,
                        },
                    }],
                    span: *span,
                };
                let outer = Core::Apply {
                    func: Box::new(self.expr(lhs)),
                    args: vec![CoreArg {
                        name: None,
                        value: inner,
                    }],
                    span: *span,
                };
                Core::Lambda {
                    params: vec![v],
                    body: Box::new(outer),
                    span: *span,
                }
            }
            // `x |> f`  =>  f(x)
            Expr::Pipe { lhs, rhs, span } => Core::Apply {
                func: Box::new(self.expr(rhs)),
                args: vec![CoreArg {
                    name: None,
                    value: self.expr(lhs),
                }],
                span: *span,
            },
            Expr::Call { callee, args, span } => Core::Apply {
                func: Box::new(self.expr(callee)),
                args: args
                    .iter()
                    .map(|a| CoreArg {
                        name: a.name.as_ref().map(|n| n.sym.as_str().to_string()),
                        value: self.expr(&a.value),
                    })
                    .collect(),
                span: *span,
            },
            Expr::Index { base, index, span } => Core::Index {
                base: Box::new(self.expr(base)),
                index: Box::new(self.expr(index)),
                span: *span,
            },
            Expr::If(if_expr) => self.lower_if(if_expr),
            Expr::Block(block) => self.block(block),
            Expr::Lambda { params, body, span } => Core::Lambda {
                params: params.iter().map(param_name).collect(),
                body: Box::new(self.expr(body)),
                span: *span,
            },
        }
    }

    fn lower_if(&mut self, if_expr: &ast::IfExpr) -> Core {
        let els = match if_expr.els.as_deref() {
            Some(ElseBranch::Block(b)) => self.block(b),
            Some(ElseBranch::If(nested)) => self.lower_if(nested),
            None => Core::Unit {
                span: if_expr.span,
            },
        };
        Core::If {
            cond: Box::new(self.expr(&if_expr.cond)),
            then: Box::new(self.block(&if_expr.then_block)),
            els: Box::new(els),
            span: if_expr.span,
        }
    }
}

fn param_name(id: &ast::Ident) -> String {
    id.sym.as_str().to_string()
}

/// Parse a numeric literal's raw text. Underscores are ignored. The lexer has
/// already validated the shape, so this does not fail in practice.
fn parse_number(raw: &str) -> f64 {
    raw.replace('_', "").parse().unwrap_or(f64::NAN)
}
