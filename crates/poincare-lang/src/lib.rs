//! Poincare notebook language: lexer, parser, AST, spans, and diagnostics.
//!
//! This crate is the Phase 2 deliverable of the language roadmap. It turns
//! Poincare source text into an AST with source spans and structured
//! diagnostics, and is independent of the notebook UI and evaluator runtime.
//! The frozen V1 grammar it implements lives in
//! `docs/plans/poincare-notebook/poincare-language-v1-spec.md`.

pub mod ast;
pub mod builtins;
pub mod diagnostic;
pub mod env;
pub mod interpreter;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod span;
pub mod token;

pub use ast::{Program, Stmt};
pub use diagnostic::{Diagnostic, Severity};
pub use env::Env;
pub use interpreter::{Interpreter, Limits, RunOutcome, RuntimeError, Value};
pub use lexer::lex;
pub use parser::{ParseResult, parse};
pub use resolver::{ResolveResult, SessionScope, resolve};
pub use span::{Location, SourceMap, Span};
pub use token::{Token, TokenKind};
