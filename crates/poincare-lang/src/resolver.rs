//! Name resolution and scope checking.
//!
//! A static pass over the parsed AST that reports undefined names and invalid
//! redefinitions, and reports which top-level names a cell defines so the
//! notebook session can grow from cell to cell. It does not evaluate anything.
//!
//! Scope model:
//! - A cell resolves against a [`SessionScope`] of names bound by earlier cells,
//!   plus the builtins, plus its own top-level definitions.
//! - Top-level definitions (bindings and functions) are *hoisted*: they are all
//!   visible to every top-level statement and function body, so forward
//!   references and mutual recursion work (matching notebook cell semantics).
//!   Use-before-definition of a value is left to run time, not flagged here.
//! - Nested blocks, function/lambda parameters, and loop variables introduce
//!   ordinary ordered lexical scopes.
//! - Inside a `plot` statement, the field and `over` domain names (e.g. `x`,
//!   `y`) are in scope for the plot's expressions.
//! - Names inside signature type positions are a separate namespace and are not
//!   resolved as runtime names.

use std::collections::HashSet;

use crate::ast::*;
use crate::builtins;
use crate::diagnostic::{Diagnostic, Severity};

/// The set of names already defined by earlier cells in a session.
#[derive(Clone, Debug, Default)]
pub struct SessionScope {
    names: HashSet<String>,
}

impl SessionScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: impl Into<String>) {
        self.names.insert(name.into());
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Add the top-level definitions produced by a resolved cell.
    pub fn extend_from_defs<I, S>(&mut self, defs: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for d in defs {
            self.names.insert(d.into());
        }
    }
}

impl<S: Into<String>> FromIterator<S> for SessionScope {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        let mut scope = SessionScope::new();
        for name in iter {
            scope.define(name);
        }
        scope
    }
}

/// The result of resolving one cell.
#[derive(Clone, Debug)]
pub struct ResolveResult {
    pub diagnostics: Vec<Diagnostic>,
    /// Top-level names this cell binds (variables and functions), for growing
    /// the session scope.
    pub cell_defs: Vec<String>,
}

impl ResolveResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Resolve a parsed program against the given session scope.
pub fn resolve(program: &Program, session: &SessionScope) -> ResolveResult {
    let mut resolver = Resolver {
        session,
        cell_defs: HashSet::new(),
        scopes: Vec::new(),
        diags: Vec::new(),
    };
    resolver.run(program)
}

struct Resolver<'s> {
    session: &'s SessionScope,
    cell_defs: HashSet<String>,
    scopes: Vec<HashSet<String>>,
    diags: Vec<Diagnostic>,
}

impl Resolver<'_> {
    fn run(&mut self, program: &Program) -> ResolveResult {
        // Hoist top-level definitions so forward references resolve.
        for stmt in &program.stmts {
            match stmt {
                Stmt::Binding(b) => self.hoist(&b.name),
                Stmt::Func(f) => self.hoist(&f.name),
                _ => {}
            }
        }

        for stmt in &program.stmts {
            self.resolve_top_stmt(stmt);
        }

        let mut cell_defs: Vec<String> = self.cell_defs.iter().cloned().collect();
        cell_defs.sort();
        ResolveResult {
            diagnostics: std::mem::take(&mut self.diags),
            cell_defs,
        }
    }

    fn hoist(&mut self, name: &Ident) {
        if builtins::is_builtin(name.sym.as_str()) {
            self.diags.push(Diagnostic::warning(
                format!("definition of `{}` shadows a builtin", name.sym),
                name.span,
            ));
        }
        self.cell_defs.insert(name.sym.as_str().to_string());
    }

    // --- name lookup ---

    fn is_in_scope(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.contains(name))
            || self.cell_defs.contains(name)
            || self.session.contains(name)
            || builtins::is_builtin(name)
    }

    fn resolve_ident(&mut self, id: &Ident) {
        if !self.is_in_scope(id.sym.as_str()) {
            self.diags.push(Diagnostic::error(
                format!("undefined name `{}`", id.sym),
                id.span,
            ));
        }
    }

    // --- statements ---

    fn resolve_top_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Signature(sig) => self.resolve_signature(sig),
            Stmt::Binding(b) => self.resolve_expr(&b.value),
            Stmt::Func(f) => self.resolve_func(&f.params, &f.body),
            Stmt::For(f) => self.resolve_for(f),
            Stmt::Plot(p) => self.resolve_plot(p),
            Stmt::Expr(e) => self.resolve_expr(e),
        }
    }

    /// A statement inside a block, where local definitions are added in order.
    fn resolve_block_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Signature(sig) => self.resolve_signature(sig),
            Stmt::Binding(b) => {
                // The initializer sees the outer binding; the new name becomes
                // visible to later statements.
                self.resolve_expr(&b.value);
                self.define_local(&b.name);
            }
            Stmt::Func(f) => {
                // Add the name first so the body may recurse.
                self.define_local(&f.name);
                self.resolve_func(&f.params, &f.body);
            }
            Stmt::For(f) => self.resolve_for(f),
            Stmt::Plot(p) => self.resolve_plot(p),
            Stmt::Expr(e) => self.resolve_expr(e),
        }
    }

    fn resolve_signature(&mut self, sig: &Signature) {
        // Type positions are a separate namespace; do not resolve them as
        // runtime names. Just check the subject has a definition somewhere.
        let name = sig.name.sym.as_str();
        if !self.cell_defs.contains(name)
            && !self.session.contains(name)
            && !builtins::is_builtin(name)
        {
            self.diags.push(Diagnostic::warning(
                format!(
                    "signature for `{name}` has no matching definition in this cell or session"
                ),
                sig.name.span,
            ));
        }
    }

    fn resolve_for(&mut self, stmt: &ForStmt) {
        self.resolve_expr(&stmt.iter);
        self.scopes.push(HashSet::new());
        self.define_local(&stmt.var);
        self.resolve_block_body(&stmt.body);
        self.scopes.pop();
    }

    fn resolve_func(&mut self, params: &[Ident], body: &Expr) {
        self.scopes.push(HashSet::new());
        self.bind_params(params);
        self.resolve_expr(body);
        self.scopes.pop();
    }

    fn resolve_plot(&mut self, plot: &PlotStmt) {
        // Field and `over` domain names are in scope for plot expressions.
        self.scopes.push(HashSet::new());
        for field in plot.over.iter().chain(plot.fields.iter()) {
            self.define_local(&field.name);
        }
        if let Some(target) = &plot.target {
            self.resolve_expr(target);
        }
        for field in plot.over.iter().chain(plot.fields.iter()) {
            self.resolve_expr(&field.value);
        }
        self.scopes.pop();
    }

    fn bind_params(&mut self, params: &[Ident]) {
        let mut seen = HashSet::new();
        for param in params {
            if !seen.insert(param.sym.as_str().to_string()) {
                self.diags.push(Diagnostic::error(
                    format!("parameter `{}` is bound more than once", param.sym),
                    param.span,
                ));
            }
            self.define_local(param);
        }
    }

    fn define_local(&mut self, name: &Ident) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.sym.as_str().to_string());
        }
    }

    // --- blocks ---

    fn resolve_block(&mut self, block: &Block) {
        self.scopes.push(HashSet::new());
        self.resolve_block_body(block);
        self.scopes.pop();
    }

    /// Resolve a block's statements and tail in the current scope (no new scope
    /// is pushed; the caller controls scoping).
    fn resolve_block_body(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.resolve_block_stmt(stmt);
        }
        if let Some(tail) = &block.tail {
            self.resolve_expr(tail);
        }
    }

    // --- expressions ---

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int { .. } | Expr::Float { .. } | Expr::Str { .. } | Expr::Bool { .. } => {}
            Expr::Ident(id) => self.resolve_ident(id),
            Expr::List { items, .. } => {
                for item in items {
                    self.resolve_expr(item);
                }
            }
            Expr::Range { lo, hi, .. } => {
                self.resolve_expr(lo);
                self.resolve_expr(hi);
            }
            Expr::Unary { expr, .. } => self.resolve_expr(expr),
            Expr::Binary { lhs, rhs, .. }
            | Expr::Compose { lhs, rhs, .. }
            | Expr::Pipe { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                // A plot constructor's formula argument (e.g. the `z` in
                // `surface(z = x^2 + y^2)`) ranges over coordinate variables,
                // which are in scope there even though nothing binds them.
                let coord_vars = plot_ctor_coord_vars(callee);
                if coord_vars.is_empty() {
                    for arg in args {
                        // Named-argument labels are not references.
                        self.resolve_expr(&arg.value);
                    }
                } else {
                    self.scopes.push(coord_vars.iter().map(|v| v.to_string()).collect());
                    for arg in args {
                        self.resolve_expr(&arg.value);
                    }
                    self.scopes.pop();
                }
            }
            Expr::Index { base, index, .. } => {
                self.resolve_expr(base);
                self.resolve_expr(index);
            }
            Expr::If(if_expr) => self.resolve_if(if_expr),
            Expr::Block(block) => self.resolve_block(block),
            Expr::Lambda { params, body, .. } => self.resolve_func(params, body),
        }
    }

    fn resolve_if(&mut self, if_expr: &IfExpr) {
        self.resolve_expr(&if_expr.cond);
        self.resolve_block(&if_expr.then_block);
        match if_expr.els.as_deref() {
            Some(ElseBranch::Block(block)) => self.resolve_block(block),
            Some(ElseBranch::If(nested)) => self.resolve_if(nested),
            None => {}
        }
    }
}

/// If `callee` names a plot constructor with a formula argument, the coordinate
/// variables that are in scope inside that call; otherwise empty.
fn plot_ctor_coord_vars(callee: &Expr) -> &'static [&'static str] {
    match callee {
        Expr::Ident(id) if builtins::plot_formula_field(id.sym.as_str()).is_some() => {
            builtins::plot_coord_vars(id.sym.as_str())
        }
        _ => &[],
    }
}
