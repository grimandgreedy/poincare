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
use crate::ir::{self, Core, CoreArg, CoreStmt};
use crate::span::Span;

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
#[derive(Clone)]
pub enum Value {
    Unit,
    Num(f64),
    Bool(bool),
    Str(Rc<str>),
    List(Rc<Vec<Value>>),
    /// An inclusive numeric range `lo..hi`.
    Range(f64, f64),
    Closure(Rc<Closure>),
    Builtin(&'static str),
}

pub struct Closure {
    params: Vec<String>,
    body: Core,
    env: Environment,
}

impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Unit => "unit",
            Value::Num(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Range(_, _) => "range",
            Value::Closure(_) => "function",
            Value::Builtin(_) => "builtin",
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
            Value::Closure(_) => "<function>".to_string(),
            Value::Builtin(name) => format!("<builtin {name}>"),
        }
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
#[derive(Clone)]
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
    /// The value of the last expression statement (for auto-display).
    pub value_display: Option<String>,
    /// Text written by `print`, one entry per call.
    pub output: Vec<String>,
    pub error: Option<RuntimeError>,
}

pub struct Interpreter {
    env: Environment,
    limits: Limits,
    output: Vec<String>,
    loop_iterations: u64,
    call_depth: usize,
    cancel: Arc<AtomicBool>,
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
            output: Vec::new(),
            loop_iterations: 0,
            call_depth: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A handle a host can set to request cancellation of a running cell.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Run a parsed program against the persistent session environment.
    pub fn run(&mut self, program: &Program) -> RunOutcome {
        let core = ir::lower(program);
        self.output.clear();
        self.loop_iterations = 0;
        self.call_depth = 0;
        self.cancel.store(false, Ordering::Relaxed);

        let mut last: Option<Value> = None;
        for stmt in &core.stmts {
            match self.exec_stmt(stmt) {
                Ok(value) => last = value,
                Err(error) => {
                    return RunOutcome {
                        value_display: last.map(|v| v.display()),
                        output: std::mem::take(&mut self.output),
                        error: Some(error),
                    };
                }
            }
        }
        RunOutcome {
            value_display: last.filter(|v| !matches!(v, Value::Unit)).map(|v| v.display()),
            output: std::mem::take(&mut self.output),
            error: None,
        }
    }

    fn exec_stmt(&mut self, stmt: &CoreStmt) -> Result<Option<Value>, RuntimeError> {
        match stmt {
            CoreStmt::Bind { name, value, .. } => {
                let env = self.env.clone();
                let v = self.eval(value, &env)?;
                self.env.define(name.clone(), v);
                Ok(None)
            }
            CoreStmt::Expr(core) => {
                let env = self.env.clone();
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
        let mut arg_values: Vec<(Option<&str>, Value)> = Vec::with_capacity(args.len());
        for arg in args {
            let value = self.eval(&arg.value, env)?;
            arg_values.push((arg.name.as_deref(), value));
        }

        match callee {
            Value::Closure(closure) => self.call_closure(&closure, arg_values, span),
            Value::Builtin(name) => {
                let positional: Vec<Value> = arg_values.into_iter().map(|(_, v)| v).collect();
                self.call_builtin(name, positional, span)
            }
            other => Err(RuntimeError::new(
                format!("a {} is not callable", other.kind()),
                span,
            )),
        }
    }

    fn call_closure(
        &mut self,
        closure: &Closure,
        args: Vec<(Option<&str>, Value)>,
        span: Span,
    ) -> EvalResult {
        let mut slots: Vec<Option<Value>> = vec![None; closure.params.len()];
        let mut next_positional = 0;
        for (name, value) in args {
            match name {
                Some(name) => match closure.params.iter().position(|p| p == name) {
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

    fn call_builtin(&mut self, name: &str, args: Vec<Value>, span: Span) -> EvalResult {
        match name {
            "print" => {
                let text: Vec<String> = args.iter().map(Value::display).collect();
                self.output.push(text.join(" "));
                Ok(Value::Unit)
            }
            "sin" | "cos" | "tan" | "exp" | "log" | "sqrt" | "abs" | "floor" | "ceil" => {
                let x = single_number_arg(name, &args, span)?;
                let r = match name {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "exp" => x.exp(),
                    "log" => x.ln(),
                    "sqrt" => x.sqrt(),
                    "abs" => x.abs(),
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    _ => unreachable!(),
                };
                Ok(Value::Num(r))
            }
            "min" | "max" => {
                if args.is_empty() {
                    return Err(RuntimeError::new(
                        format!("`{name}` needs at least one argument"),
                        span,
                    ));
                }
                let mut acc = number(&args[0], span)?;
                for arg in &args[1..] {
                    let v = number(arg, span)?;
                    acc = if name == "min" { acc.min(v) } else { acc.max(v) };
                }
                Ok(Value::Num(acc))
            }
            other => Err(RuntimeError::new(
                format!("builtin `{other}` is not implemented yet (coming in a later phase)"),
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
