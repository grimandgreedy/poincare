//! Tree-walking interpreter over the core IR.
//!
//! Holds a session environment that persists across `run` calls, so a
//! definition in one cell is visible to the next (notebook session semantics).
//! Closures capture their defining environment by shared reference, so
//! top-level functions can call one another and recurse. Runtime errors stop
//! the current cell; `print` appends to an output stream. Loop iterations are
//! bounded and a cancellation flag is checked so runaway cells cannot hang.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::builtins;
use crate::host::Host;
use crate::ir::{self, Core, CoreArg, CoreStmt};
use crate::span::Span;

/// A call argument paired with an optional name.
type CallArg = (Option<String>, Value);

/// A runtime error with the span of the offending construct.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Span,
}

impl RuntimeError {
    fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

type EvalResult = Result<Value, RuntimeError>;

/// A runtime value.
///
/// Graph/plot/table values are language-native structured values, not
/// `poincare-lib` types; the Phase 6 evaluator adapter translates them into
/// `poincare_lib::GraphSpec`/`PlotSpec` and evaluator table values, keeping
/// this crate decoupled from the graphing library.
#[derive(Clone, Debug)]
pub enum Value {
    Unit,
    Num(f64),
    Bool(bool),
    Str(Rc<str>),
    Bytes(Rc<Vec<u8>>),
    List(Rc<Vec<Value>>),
    /// An inclusive numeric range `lo..hi`.
    Range(f64, f64),
    Table(Rc<Table>),
    Plot(Rc<Plot>),
    Graph(Rc<Graph>),
    /// A handle to a notebook attachment, resolved through the host.
    Attachment(Rc<str>),
    Closure(Rc<Closure>),
    Builtin(&'static str),
    /// A captured, unevaluated plot formula (e.g. the `z` in
    /// `surface(z = x^2 + y^2)`). Coordinate variables such as `x`/`y` are left
    /// free; other free variables bound to numbers are recorded as parameters.
    /// The evaluator adapter lowers this into a `poincare-lib` expression plot.
    Expr(Rc<ExprValue>),
}

/// A captured plot formula: its source text plus the numeric parameters it
/// closed over at capture time.
#[derive(Clone, Debug)]
pub struct ExprValue {
    pub source: String,
    pub params: Vec<(String, f64)>,
}

#[derive(Debug)]
pub struct Closure {
    params: Vec<String>,
    body: Core,
    env: Environment,
}

/// A tabular value: named columns over rows of cells.
#[derive(Clone, Debug)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// A single plot: its kind (`surface`, `curve`, ...) plus the fields it was
/// built from. Interpreted into a `poincare-lib` plot by the evaluator adapter.
#[derive(Clone, Debug)]
pub struct Plot {
    pub kind: String,
    pub fields: Vec<(String, Value)>,
    pub positional: Vec<Value>,
}

/// A graph: an ordered collection of plots.
#[derive(Clone, Debug)]
pub struct Graph {
    pub plots: Vec<Plot>,
}

impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Unit => "unit",
            Value::Num(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::List(_) => "list",
            Value::Range(_, _) => "range",
            Value::Table(_) => "table",
            Value::Plot(_) => "plot",
            Value::Graph(_) => "graph",
            Value::Attachment(_) => "attachment",
            Value::Closure(_) => "function",
            Value::Builtin(_) => "builtin",
            Value::Expr(_) => "expression",
        }
    }

    /// The parameter names of a closure value, if this is a function.
    pub fn closure_params(&self) -> Option<Vec<String>> {
        match self {
            Value::Closure(c) => Some(c.params.clone()),
            _ => None,
        }
    }

    /// A human-readable rendering, used by `print` and value previews.
    pub fn display(&self) -> String {
        match self {
            Value::Unit => "()".to_string(),
            Value::Num(n) => format!("{n}"),
            Value::Bool(b) => b.to_string(),
            Value::Str(s) => s.to_string(),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(Value::display).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Range(lo, hi) => format!("{lo}..{hi}"),
            Value::Bytes(b) => format!("<bytes {}>", b.len()),
            Value::Table(t) => {
                format!("<table {} columns x {} rows>", t.columns.len(), t.rows.len())
            }
            Value::Plot(p) => format!("<plot {}>", p.kind),
            Value::Graph(g) => format!("<graph {} plots>", g.plots.len()),
            Value::Attachment(name) => format!("<attachment {name}>"),
            Value::Closure(_) => "<function>".to_string(),
            Value::Builtin(name) => format!("<builtin {name}>"),
            Value::Expr(e) => e.source.clone(),
        }
    }
}

/// Capture a plot formula: render its source text and record any free variables
/// (other than coordinate variables) that are bound to numbers as parameters.
fn capture_formula(core: &Core, coord_vars: &[&str], env: &Environment) -> Value {
    let source = render_formula(core);
    let mut names = Vec::new();
    collect_free_vars(core, &mut names);
    let mut params: Vec<(String, f64)> = Vec::new();
    for name in names {
        if coord_vars.contains(&name.as_str()) {
            continue;
        }
        if params.iter().any(|(existing, _)| *existing == name) {
            continue;
        }
        if let Some(Value::Num(value)) = env.get(&name) {
            params.push((name, value));
        }
    }
    Value::Expr(Rc::new(ExprValue { source, params }))
}

/// Render a core expression back to a source string compatible with
/// `poincare-lib`'s expression grammar (`+ - * / ^`, function calls, unary `-`).
fn render_formula(core: &Core) -> String {
    match core {
        Core::Num { value, .. } => format_number(*value),
        Core::Bool { value, .. } => value.to_string(),
        Core::Str { value, .. } => value.clone(),
        Core::Var { name, .. } => name.clone(),
        Core::Unary { op, expr, .. } => {
            let inner = render_formula(expr);
            match op {
                UnaryOp::Neg => format!("(-{inner})"),
                UnaryOp::Not => format!("(!{inner})"),
            }
        }
        Core::Binary { op, lhs, rhs, .. } => {
            format!(
                "({} {} {})",
                render_formula(lhs),
                binary_op_symbol(*op),
                render_formula(rhs)
            )
        }
        Core::Apply { func, args, .. } => {
            let rendered: Vec<String> = args.iter().map(|a| render_formula(&a.value)).collect();
            format!("{}({})", render_formula(func), rendered.join(", "))
        }
        Core::List { items, .. } => {
            let rendered: Vec<String> = items.iter().map(render_formula).collect();
            format!("[{}]", rendered.join(", "))
        }
        Core::Range { lo, hi, .. } => {
            format!("{}..{}", render_formula(lo), render_formula(hi))
        }
        // Constructs with no expression-grammar equivalent fall back to a marker
        // that surfaces as a parse error in the preview rather than silently
        // rendering wrong data.
        _ => "<unsupported>".to_string(),
    }
}

fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Pow => "^",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

/// Format a number without a trailing `.0`, so `2.0` renders as `2`.
fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Collect the free variable names referenced in a core expression, in order of
/// first appearance. Variables bound by an inner lambda are excluded.
fn collect_free_vars(core: &Core, out: &mut Vec<String>) {
    match core {
        Core::Var { name, .. } => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Core::Unary { expr, .. } => collect_free_vars(expr, out),
        Core::Binary { lhs, rhs, .. } => {
            collect_free_vars(lhs, out);
            collect_free_vars(rhs, out);
        }
        Core::Apply { func, args, .. } => {
            collect_free_vars(func, out);
            for arg in args {
                collect_free_vars(&arg.value, out);
            }
        }
        Core::Index { base, index, .. } => {
            collect_free_vars(base, out);
            collect_free_vars(index, out);
        }
        Core::List { items, .. } => {
            for item in items {
                collect_free_vars(item, out);
            }
        }
        Core::Range { lo, hi, .. } => {
            collect_free_vars(lo, out);
            collect_free_vars(hi, out);
        }
        Core::If { cond, then, els, .. } => {
            collect_free_vars(cond, out);
            collect_free_vars(then, out);
            collect_free_vars(els, out);
        }
        Core::Lambda { params, body, .. } => {
            let mut inner = Vec::new();
            collect_free_vars(body, &mut inner);
            for name in inner {
                if !params.contains(&name) && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        _ => {}
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Unit, Value::Unit) => true,
        (Value::Num(x), Value::Num(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Range(a1, b1), Value::Range(a2, b2)) => a1 == a2 && b1 == b2,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        _ => false,
    }
}

// --- environment (shared, reference-counted scope chain) ---

type Scope = Rc<RefCell<HashMap<String, Value>>>;

/// The runtime realization of the scope-chain model: scopes are shared so
/// closures capturing an environment see later mutations to it (needed for
/// mutual recursion between top-level definitions).
#[derive(Clone, Debug)]
pub struct Environment {
    scopes: Vec<Scope>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Rc::new(RefCell::new(HashMap::new()))],
        }
    }

    /// A new environment sharing this one's scopes plus a fresh inner scope.
    fn child(&self) -> Environment {
        let mut scopes = self.scopes.clone();
        scopes.push(Rc::new(RefCell::new(HashMap::new())));
        Environment { scopes }
    }

    fn define(&self, name: impl Into<String>, value: Value) {
        self.scopes
            .last()
            .expect("environment always has a scope")
            .borrow_mut()
            .insert(name.into(), value);
    }

    fn get(&self, name: &str) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.borrow().get(name).cloned())
    }

    /// Bindings in the global (session) scope, name-sorted.
    fn globals(&self) -> Vec<(String, Value)> {
        let mut vars: Vec<(String, Value)> = self.scopes[0]
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        vars
    }
}

/// Limits that keep evaluation bounded.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_loop_iterations: u64,
    pub max_call_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_loop_iterations: 10_000_000,
            max_call_depth: 256,
        }
    }
}

/// The result of running one cell.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    /// The value of the last expression statement, if not unit.
    pub value: Option<Value>,
    /// Display rendering of `value` (for quick previews).
    pub value_display: Option<String>,
    /// Text written by `print`, one entry per call.
    pub output: Vec<String>,
    /// Structured values emitted via `emit` (graphs, plots, tables).
    pub emitted: Vec<Value>,
    pub error: Option<RuntimeError>,
}

pub struct Interpreter {
    env: Environment,
    limits: Limits,
    cancel: Arc<AtomicBool>,
}

/// A host that has no attachments; used when a cell runs without one.
struct NoHost;

impl Host for NoHost {
    fn attachment_text(&self, _name_or_id: &str) -> Result<String, String> {
        Err("no attachment host is available".to_string())
    }
    fn attachment_bytes(&self, _name_or_id: &str) -> Result<Vec<u8>, String> {
        Err("no attachment host is available".to_string())
    }
}

/// Per-run execution state, holding the borrowed host for the duration of one
/// cell run plus the mutable output/counters. The session environment shares
/// scopes with the interpreter's, so top-level bindings persist after the run.
struct Run<'a> {
    session: Environment,
    host: &'a dyn Host,
    output: Vec<String>,
    emitted: Vec<Value>,
    limits: Limits,
    loop_iterations: u64,
    call_depth: usize,
    cancel: &'a AtomicBool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Self {
        Self {
            env: Environment::new(),
            limits,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A handle a host can set to request cancellation of a running cell.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Session variables: bindings in the global scope, name-sorted. Used to
    /// build a session snapshot for the notebook variables panel.
    pub fn variables(&self) -> Vec<(String, Value)> {
        self.env.globals()
    }

    /// Run a cell against the persistent session, with no attachment host.
    pub fn run(&mut self, program: &Program) -> RunOutcome {
        self.run_with_host(program, &NoHost)
    }

    /// Run a cell against the persistent session, resolving attachments through
    /// `host`.
    pub fn run_with_host(&mut self, program: &Program, host: &dyn Host) -> RunOutcome {
        let core = ir::lower(program);
        self.cancel.store(false, Ordering::Relaxed);
        let mut run = Run {
            session: self.env.clone(),
            host,
            output: Vec::new(),
            emitted: Vec::new(),
            limits: self.limits,
            loop_iterations: 0,
            call_depth: 0,
            cancel: &self.cancel,
        };
        run.exec(&core)
    }
}

impl Run<'_> {
    fn exec(&mut self, program: &ir::CoreProgram) -> RunOutcome {
        let mut last: Option<Value> = None;
        for stmt in &program.stmts {
            match self.exec_stmt(stmt) {
                Ok(value) => last = value,
                Err(error) => {
                    return RunOutcome {
                        value: None,
                        value_display: last.map(|v| v.display()),
                        output: std::mem::take(&mut self.output),
                        emitted: std::mem::take(&mut self.emitted),
                        error: Some(error),
                    };
                }
            }
        }
        let final_value = last.filter(|v| !matches!(v, Value::Unit));
        RunOutcome {
            value_display: final_value.as_ref().map(|v| v.display()),
            value: final_value,
            output: std::mem::take(&mut self.output),
            emitted: std::mem::take(&mut self.emitted),
            error: None,
        }
    }

    fn exec_stmt(&mut self, stmt: &CoreStmt) -> Result<Option<Value>, RuntimeError> {
        match stmt {
            CoreStmt::Bind { name, value, .. } => {
                let env = self.session.clone();
                let v = self.eval(value, &env)?;
                self.session.define(name.clone(), v);
                Ok(None)
            }
            CoreStmt::Expr(core) => {
                let env = self.session.clone();
                let v = self.eval(core, &env)?;
                Ok(Some(v))
            }
        }
    }

    fn eval(&mut self, core: &Core, env: &Environment) -> EvalResult {
        match core {
            Core::Unit { .. } => Ok(Value::Unit),
            Core::Num { value, .. } => Ok(Value::Num(*value)),
            Core::Bool { value, .. } => Ok(Value::Bool(*value)),
            Core::Str { value, .. } => Ok(Value::Str(Rc::from(value.as_str()))),
            Core::Var { name, span } => self.eval_var(name, *span, env),
            Core::List { items, .. } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item, env)?);
                }
                Ok(Value::List(Rc::new(values)))
            }
            Core::Range { lo, hi, span } => {
                let lo = self.eval_number(lo, env)?;
                let hi = self.eval_number(hi, env)?;
                let _ = span;
                Ok(Value::Range(lo, hi))
            }
            Core::Unary { op, expr, span } => self.eval_unary(*op, expr, *span, env),
            Core::Binary { op, lhs, rhs, span } => self.eval_binary(*op, lhs, rhs, *span, env),
            Core::Lambda { params, body, .. } => Ok(Value::Closure(Rc::new(Closure {
                params: params.clone(),
                body: (**body).clone(),
                env: env.clone(),
            }))),
            Core::Apply { func, args, span } => self.eval_apply(func, args, *span, env),
            Core::Index { base, index, span } => self.eval_index(base, index, *span, env),
            Core::If {
                cond, then, els, ..
            } => {
                if self.eval_bool(cond, env)? {
                    self.eval(then, env)
                } else {
                    self.eval(els, env)
                }
            }
            Core::Block { stmts, tail, .. } => self.eval_block(stmts, tail.as_deref(), env),
            Core::For {
                var,
                iter,
                body,
                span,
            } => self.eval_for(var, iter, body, *span, env),
            Core::Todo { what, span } => Err(RuntimeError::new(
                format!("`{what}` is not executable yet (coming in a later phase)"),
                *span,
            )),
        }
    }

    fn eval_var(&mut self, name: &str, span: Span, env: &Environment) -> EvalResult {
        // Numeric constants live in the value namespace, not the builtin call
        // namespace, so `2 * pi` works.
        match name {
            "pi" => return Ok(Value::Num(std::f64::consts::PI)),
            "e" => return Ok(Value::Num(std::f64::consts::E)),
            _ => {}
        }
        if let Some(value) = env.get(name) {
            return Ok(value);
        }
        if let Some(canonical) = builtins::all().find(|b| *b == name) {
            return Ok(Value::Builtin(canonical));
        }
        Err(RuntimeError::new(format!("undefined name `{name}`"), span))
    }

    fn eval_block(
        &mut self,
        stmts: &[CoreStmt],
        tail: Option<&Core>,
        env: &Environment,
    ) -> EvalResult {
        let block_env = env.child();
        for stmt in stmts {
            match stmt {
                CoreStmt::Bind { name, value, .. } => {
                    let v = self.eval(value, &block_env)?;
                    block_env.define(name.clone(), v);
                }
                CoreStmt::Expr(core) => {
                    self.eval(core, &block_env)?;
                }
            }
        }
        match tail {
            Some(tail) => self.eval(tail, &block_env),
            None => Ok(Value::Unit),
        }
    }

    fn eval_for(
        &mut self,
        var: &str,
        iter: &Core,
        body: &Core,
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        let iterable = self.eval(iter, env)?;
        let items: Vec<Value> = match iterable {
            Value::List(items) => (*items).clone(),
            Value::Range(lo, hi) => {
                let mut values = Vec::new();
                let mut v = lo;
                while v <= hi {
                    values.push(Value::Num(v));
                    v += 1.0;
                }
                values
            }
            other => {
                return Err(RuntimeError::new(
                    format!("cannot iterate over a {}", other.kind()),
                    span,
                ));
            }
        };

        for item in items {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(RuntimeError::new("evaluation cancelled", span));
            }
            self.loop_iterations += 1;
            if self.loop_iterations > self.limits.max_loop_iterations {
                return Err(RuntimeError::new("loop iteration limit exceeded", span));
            }
            let loop_env = env.child();
            loop_env.define(var.to_string(), item);
            self.eval(body, &loop_env)?;
        }
        Ok(Value::Unit)
    }

    fn eval_apply(
        &mut self,
        func: &Core,
        args: &[CoreArg],
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        let callee = self.eval(func, env)?;

        // Plot constructors capture their formula argument unevaluated so it can
        // refer to unbound coordinate variables (`surface(z = x^2 + y^2)`).
        if let Value::Builtin(name) = callee {
            if let Some(formula_field) = builtins::plot_formula_field(name) {
                return self.eval_plot_ctor(name, formula_field, args, span, env);
            }
        }

        let mut arg_values: Vec<CallArg> = Vec::with_capacity(args.len());
        for arg in args {
            let value = self.eval(&arg.value, env)?;
            arg_values.push((arg.name.clone(), value));
        }

        match callee {
            Value::Closure(closure) => self.call_closure(&closure, arg_values, span),
            Value::Builtin(name) => self.call_builtin(name, arg_values, span),
            other => Err(RuntimeError::new(
                format!("a {} is not callable", other.kind()),
                span,
            )),
        }
    }

    /// Evaluate a plot constructor (`surface`, `curve`, ...), capturing its
    /// formula argument unevaluated while evaluating domain/resolution args
    /// normally. The formula is stored as a [`Value::Expr`] carrying its source
    /// text and any numeric parameters it references.
    fn eval_plot_ctor(
        &mut self,
        name: &'static str,
        formula_field: &str,
        args: &[CoreArg],
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        let coord_vars = builtins::plot_coord_vars(name);
        let mut arg_values: Vec<CallArg> = Vec::with_capacity(args.len());
        for arg in args {
            if arg.name.as_deref() == Some(formula_field) {
                let captured = capture_formula(&arg.value, coord_vars, env);
                arg_values.push((arg.name.clone(), captured));
            } else {
                let value = self.eval(&arg.value, env)?;
                arg_values.push((arg.name.clone(), value));
            }
        }
        self.call_builtin(name, arg_values, span)
    }

    /// Apply a callable value to positional arguments (used by higher-order
    /// builtins like `map`).
    fn apply_callable(&mut self, callable: &Value, args: Vec<Value>, span: Span) -> EvalResult {
        let named: Vec<CallArg> = args.into_iter().map(|v| (None, v)).collect();
        match callable {
            Value::Closure(closure) => self.call_closure(closure, named, span),
            Value::Builtin(name) => self.call_builtin(name, named, span),
            other => Err(RuntimeError::new(
                format!("a {} is not callable", other.kind()),
                span,
            )),
        }
    }

    fn call_closure(&mut self, closure: &Closure, args: Vec<CallArg>, span: Span) -> EvalResult {
        let mut slots: Vec<Option<Value>> = vec![None; closure.params.len()];
        let mut next_positional = 0;
        for (name, value) in args {
            match name {
                Some(name) => match closure.params.iter().position(|p| *p == name) {
                    Some(idx) => slots[idx] = Some(value),
                    None => {
                        return Err(RuntimeError::new(
                            format!("unknown argument `{name}`"),
                            span,
                        ));
                    }
                },
                None => {
                    if next_positional >= slots.len() {
                        return Err(RuntimeError::new(
                            format!(
                                "too many arguments: expected {}, got more",
                                closure.params.len()
                            ),
                            span,
                        ));
                    }
                    slots[next_positional] = Some(value);
                    next_positional += 1;
                }
            }
        }

        let call_env = closure.env.child();
        for (param, slot) in closure.params.iter().zip(slots.into_iter()) {
            match slot {
                Some(value) => call_env.define(param.clone(), value),
                None => {
                    return Err(RuntimeError::new(
                        format!("missing argument for parameter `{param}`"),
                        span,
                    ));
                }
            }
        }

        self.call_depth += 1;
        if self.call_depth > self.limits.max_call_depth {
            self.call_depth -= 1;
            return Err(RuntimeError::new("maximum call depth exceeded", span));
        }
        let result = self.eval(&closure.body, &call_env);
        self.call_depth -= 1;
        result
    }

    fn call_builtin(&mut self, name: &str, args: Vec<CallArg>, span: Span) -> EvalResult {
        match name {
            // --- output ---
            "print" => {
                let text: Vec<String> = positional(args).iter().map(Value::display).collect();
                self.output.push(text.join(" "));
                Ok(Value::Unit)
            }
            "emit" => {
                let values = positional(args);
                if values.is_empty() {
                    return Err(RuntimeError::new("`emit` needs a value", span));
                }
                for v in values {
                    self.emitted.push(v);
                }
                Ok(Value::Unit)
            }

            // --- unary math ---
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "log" | "log2"
            | "log10" | "sqrt" | "abs" | "sign" | "round" | "trunc" | "floor" | "ceil" => {
                let x = single_number_arg(name, &positional(args), span)?;
                let r = match name {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "asin" => x.asin(),
                    "acos" => x.acos(),
                    "atan" => x.atan(),
                    "exp" => x.exp(),
                    "log" => x.ln(),
                    "log2" => x.log2(),
                    "log10" => x.log10(),
                    "sqrt" => x.sqrt(),
                    "abs" => x.abs(),
                    "sign" => x.signum(),
                    "round" => x.round(),
                    "trunc" => x.trunc(),
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    _ => unreachable!(),
                };
                Ok(Value::Num(r))
            }
            "atan2" | "pow" => {
                let a = positional(args);
                if a.len() != 2 {
                    return Err(RuntimeError::new(
                        format!("`{name}` takes exactly two arguments"),
                        span,
                    ));
                }
                let x = number(&a[0], span)?;
                let y = number(&a[1], span)?;
                Ok(Value::Num(if name == "atan2" {
                    x.atan2(y)
                } else {
                    x.powf(y)
                }))
            }
            "min" | "max" => {
                let a = positional(args);
                if a.is_empty() {
                    return Err(RuntimeError::new(
                        format!("`{name}` needs at least one argument"),
                        span,
                    ));
                }
                let mut acc = number(&a[0], span)?;
                for arg in &a[1..] {
                    let v = number(arg, span)?;
                    acc = if name == "min" { acc.min(v) } else { acc.max(v) };
                }
                Ok(Value::Num(acc))
            }

            // --- list / data ---
            "len" => {
                let a = single_arg(name, positional(args), span)?;
                let n = match &a {
                    Value::List(items) => items.len(),
                    Value::Str(s) => s.chars().count(),
                    Value::Table(t) => t.rows.len(),
                    other => {
                        return Err(RuntimeError::new(
                            format!("cannot take the length of a {}", other.kind()),
                            span,
                        ));
                    }
                };
                Ok(Value::Num(n as f64))
            }
            "sum" | "mean" | "prod" => {
                let a = single_arg(name, positional(args), span)?;
                let items = expect_list(&a, span)?;
                let nums: Result<Vec<f64>, RuntimeError> =
                    items.iter().map(|v| number(v, span)).collect();
                let nums = nums?;
                match name {
                    "sum" => Ok(Value::Num(nums.iter().sum())),
                    "prod" => Ok(Value::Num(nums.iter().product())),
                    "mean" => {
                        if nums.is_empty() {
                            return Err(RuntimeError::new("`mean` of an empty list", span));
                        }
                        Ok(Value::Num(nums.iter().sum::<f64>() / nums.len() as f64))
                    }
                    _ => unreachable!(),
                }
            }
            "map" | "filter" => {
                let a = positional(args);
                if a.len() != 2 {
                    return Err(RuntimeError::new(
                        format!("`{name}` takes a list and a function"),
                        span,
                    ));
                }
                let items = expect_list(&a[0], span)?.to_vec();
                let func = a[1].clone();
                let mut out = Vec::new();
                for item in items {
                    let result = self.apply_callable(&func, vec![item.clone()], span)?;
                    if name == "map" {
                        out.push(result);
                    } else if bool_value(&result, span)? {
                        out.push(item);
                    }
                }
                Ok(Value::List(Rc::new(out)))
            }

            // --- graphs and plots ---
            "graph" => Ok(Value::Graph(Rc::new(Graph { plots: Vec::new() }))),
            "surface" | "curve" | "scatter" | "vector_field" | "volume" | "isosurface" => {
                let mut fields = Vec::new();
                let mut plot_positional = Vec::new();
                for (arg_name, value) in args {
                    match arg_name {
                        Some(n) => fields.push((n, value)),
                        None => plot_positional.push(value),
                    }
                }
                Ok(Value::Plot(Rc::new(Plot {
                    kind: name.to_string(),
                    fields,
                    positional: plot_positional,
                })))
            }
            "add_plot" => {
                let a = positional(args);
                if a.len() != 2 {
                    return Err(RuntimeError::new("`add_plot` takes a graph and a plot", span));
                }
                let graph = match &a[0] {
                    Value::Graph(g) => g,
                    other => {
                        return Err(RuntimeError::new(
                            format!("`add_plot` expected a graph, found a {}", other.kind()),
                            span,
                        ));
                    }
                };
                let plot = match &a[1] {
                    Value::Plot(p) => (**p).clone(),
                    other => {
                        return Err(RuntimeError::new(
                            format!("`add_plot` expected a plot, found a {}", other.kind()),
                            span,
                        ));
                    }
                };
                let mut plots = graph.plots.clone();
                plots.push(plot);
                Ok(Value::Graph(Rc::new(Graph { plots })))
            }

            // --- tables ---
            "column" => {
                let a = positional(args);
                if a.len() != 2 {
                    return Err(RuntimeError::new("`column` takes a table and a name", span));
                }
                let table = expect_table(&a[0], span)?;
                let col_name = expect_string(&a[1], span)?;
                let idx = table
                    .columns
                    .iter()
                    .position(|c| c == &col_name)
                    .ok_or_else(|| RuntimeError::new(format!("no column named `{col_name}`"), span))?;
                let cells = table
                    .rows
                    .iter()
                    .map(|row| row.get(idx).cloned().unwrap_or(Value::Unit))
                    .collect();
                Ok(Value::List(Rc::new(cells)))
            }
            "columns" => {
                let value = single_arg(name, positional(args), span)?;
                let table = expect_table(&value, span)?;
                let names = table
                    .columns
                    .iter()
                    .map(|c| Value::Str(Rc::from(c.as_str())))
                    .collect();
                Ok(Value::List(Rc::new(names)))
            }
            "rows" => {
                let value = single_arg(name, positional(args), span)?;
                let table = expect_table(&value, span)?;
                let rows = table
                    .rows
                    .iter()
                    .map(|row| Value::List(Rc::new(row.clone())))
                    .collect();
                Ok(Value::List(Rc::new(rows)))
            }
            "array2d" => {
                let value = single_arg(name, positional(args), span)?;
                let table = expect_table(&value, span)?;
                let mut matrix = Vec::with_capacity(table.rows.len());
                for row in &table.rows {
                    let nums: Result<Vec<Value>, RuntimeError> =
                        row.iter().map(|c| number(c, span).map(Value::Num)).collect();
                    matrix.push(Value::List(Rc::new(nums?)));
                }
                Ok(Value::List(Rc::new(matrix)))
            }

            // --- attachments and data loading ---
            "attachment" => {
                let name_value = single_arg(name, positional(args), span)?;
                let id = expect_string(&name_value, span)?;
                Ok(Value::Attachment(Rc::from(id.as_str())))
            }
            "text" => {
                let a = single_arg(name, positional(args), span)?;
                Ok(Value::Str(Rc::from(self.text_source(&a, span)?.as_str())))
            }
            "bytes" => {
                let a = single_arg(name, positional(args), span)?;
                let bytes = self.attachment_bytes(&a, span)?;
                Ok(Value::Bytes(Rc::new(bytes)))
            }
            "csv" => {
                let a = single_arg(name, positional(args), span)?;
                let text = self.text_source(&a, span)?;
                Ok(Value::Table(Rc::new(parse_csv(&text))))
            }
            "csv_matrix" => {
                let a = single_arg(name, positional(args), span)?;
                let text = self.text_source(&a, span)?;
                parse_csv_matrix(&text, span)
            }

            // --- analysis ---
            "derivative" => {
                let a = positional(args);
                if a.len() != 2 {
                    return Err(RuntimeError::new(
                        "`derivative` takes a function and a point",
                        span,
                    ));
                }
                let func = a[0].clone();
                let x = number(&a[1], span)?;
                let h = 1e-6;
                let hi = self.apply_callable(&func, vec![Value::Num(x + h)], span)?;
                let lo = self.apply_callable(&func, vec![Value::Num(x - h)], span)?;
                let d = (number(&hi, span)? - number(&lo, span)?) / (2.0 * h);
                Ok(Value::Num(d))
            }

            other => Err(RuntimeError::new(
                format!("builtin `{other}` is not implemented yet (coming in a later phase)"),
                span,
            )),
        }
    }

    fn text_source(&self, value: &Value, span: Span) -> Result<String, RuntimeError> {
        match value {
            Value::Str(s) => Ok(s.to_string()),
            Value::Attachment(id) => self
                .host
                .attachment_text(id)
                .map_err(|e| RuntimeError::new(e, span)),
            other => Err(RuntimeError::new(
                format!("expected text or an attachment, found a {}", other.kind()),
                span,
            )),
        }
    }

    fn attachment_bytes(&self, value: &Value, span: Span) -> Result<Vec<u8>, RuntimeError> {
        match value {
            Value::Attachment(id) => self
                .host
                .attachment_bytes(id)
                .map_err(|e| RuntimeError::new(e, span)),
            other => Err(RuntimeError::new(
                format!("`bytes` expected an attachment, found a {}", other.kind()),
                span,
            )),
        }
    }

    fn eval_index(
        &mut self,
        base: &Core,
        index: &Core,
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        let base_value = self.eval(base, env)?;
        let idx = self.eval_number(index, env)?;
        match base_value {
            Value::List(items) => {
                if idx < 0.0 || idx.fract() != 0.0 {
                    return Err(RuntimeError::new(
                        "list index must be a non-negative integer",
                        span,
                    ));
                }
                let i = idx as usize;
                items.get(i).cloned().ok_or_else(|| {
                    RuntimeError::new(
                        format!("list index {i} out of bounds (len {})", items.len()),
                        span,
                    )
                })
            }
            other => Err(RuntimeError::new(
                format!("cannot index a {}", other.kind()),
                span,
            )),
        }
    }

    fn eval_unary(
        &mut self,
        op: UnaryOp,
        expr: &Core,
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        match op {
            UnaryOp::Neg => Ok(Value::Num(-self.eval_number(expr, env)?)),
            UnaryOp::Not => Ok(Value::Bool(!self.eval_bool(expr, env)?)),
        }
        .map_err(|mut e: RuntimeError| {
            if e.span == Span::DUMMY {
                e.span = span;
            }
            e
        })
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        lhs: &Core,
        rhs: &Core,
        span: Span,
        env: &Environment,
    ) -> EvalResult {
        // Short-circuiting logical operators.
        match op {
            BinaryOp::And => {
                return if !self.eval_bool(lhs, env)? {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Bool(self.eval_bool(rhs, env)?))
                };
            }
            BinaryOp::Or => {
                return if self.eval_bool(lhs, env)? {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(self.eval_bool(rhs, env)?))
                };
            }
            _ => {}
        }

        let left = self.eval(lhs, env)?;
        let right = self.eval(rhs, env)?;

        match op {
            BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
            BinaryOp::Ne => Ok(Value::Bool(!values_equal(&left, &right))),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Pow => {
                let a = number(&left, span)?;
                let b = number(&right, span)?;
                let r = match op {
                    BinaryOp::Add => a + b,
                    BinaryOp::Sub => a - b,
                    BinaryOp::Mul => a * b,
                    BinaryOp::Div => a / b,
                    BinaryOp::Rem => a % b,
                    BinaryOp::Pow => a.powf(b),
                    _ => unreachable!(),
                };
                Ok(Value::Num(r))
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let a = number(&left, span)?;
                let b = number(&right, span)?;
                let r = match op {
                    BinaryOp::Lt => a < b,
                    BinaryOp::Le => a <= b,
                    BinaryOp::Gt => a > b,
                    BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),
        }
    }

    // --- typed-operand helpers ---

    fn eval_number(&mut self, core: &Core, env: &Environment) -> Result<f64, RuntimeError> {
        let value = self.eval(core, env)?;
        number(&value, core.span())
    }

    fn eval_bool(&mut self, core: &Core, env: &Environment) -> Result<bool, RuntimeError> {
        let value = self.eval(core, env)?;
        match value {
            Value::Bool(b) => Ok(b),
            other => Err(RuntimeError::new(
                format!("expected a bool, found a {}", other.kind()),
                core.span(),
            )),
        }
    }
}

fn number(value: &Value, span: Span) -> Result<f64, RuntimeError> {
    match value {
        Value::Num(n) => Ok(*n),
        other => Err(RuntimeError::new(
            format!("expected a number, found a {}", other.kind()),
            span,
        )),
    }
}

fn single_number_arg(name: &str, args: &[Value], span: Span) -> Result<f64, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::new(
            format!("`{name}` takes exactly one argument, got {}", args.len()),
            span,
        ));
    }
    number(&args[0], span)
}

/// Discard argument names, keeping values in order.
fn positional(args: Vec<CallArg>) -> Vec<Value> {
    args.into_iter().map(|(_, v)| v).collect()
}

/// Require exactly one positional argument.
fn single_arg(name: &str, mut args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::new(
            format!("`{name}` takes exactly one argument, got {}", args.len()),
            span,
        ));
    }
    Ok(args.pop().unwrap())
}

fn bool_value(value: &Value, span: Span) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(RuntimeError::new(
            format!("expected a bool, found a {}", other.kind()),
            span,
        )),
    }
}

fn expect_list(value: &Value, span: Span) -> Result<&[Value], RuntimeError> {
    match value {
        Value::List(items) => Ok(items),
        other => Err(RuntimeError::new(
            format!("expected a list, found a {}", other.kind()),
            span,
        )),
    }
}

fn expect_string(value: &Value, span: Span) -> Result<String, RuntimeError> {
    match value {
        Value::Str(s) => Ok(s.to_string()),
        other => Err(RuntimeError::new(
            format!("expected a string, found a {}", other.kind()),
            span,
        )),
    }
}

fn expect_table(value: &Value, span: Span) -> Result<&Table, RuntimeError> {
    match value {
        Value::Table(t) => Ok(t),
        other => Err(RuntimeError::new(
            format!("expected a table, found a {}", other.kind()),
            span,
        )),
    }
}

/// Parse CSV text into a table. The first non-empty line is the header. Cells
/// that parse as numbers become numbers; the rest stay strings. No quoting.
fn parse_csv(text: &str) -> Table {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let columns: Vec<String> = match lines.next() {
        Some(header) => header.split(',').map(|s| s.trim().to_string()).collect(),
        None => Vec::new(),
    };
    let rows = lines
        .map(|line| {
            line.split(',')
                .map(|cell| {
                    let cell = cell.trim();
                    match cell.parse::<f64>() {
                        Ok(n) => Value::Num(n),
                        Err(_) => Value::Str(Rc::from(cell)),
                    }
                })
                .collect()
        })
        .collect();
    Table { columns, rows }
}

/// Parse CSV text as a numeric matrix (no header): a list of lists of numbers.
fn parse_csv_matrix(text: &str, span: Span) -> EvalResult {
    let mut matrix = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut row = Vec::new();
        for cell in line.split(',') {
            let cell = cell.trim();
            match cell.parse::<f64>() {
                Ok(n) => row.push(Value::Num(n)),
                Err(_) => {
                    return Err(RuntimeError::new(
                        format!("non-numeric cell `{cell}` on line {}", line_idx + 1),
                        span,
                    ));
                }
            }
        }
        matrix.push(Value::List(Rc::new(row)));
    }
    Ok(Value::List(Rc::new(matrix)))
}
