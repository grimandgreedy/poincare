//! Hand-rolled expression parser for math expressions used in interactive plotting.
//!
//! Supports: +, -, *, /, ^, parentheses, unary negation, and the functions
//! sin, cos, tan, exp, ln, sqrt, abs.  Built-in constants: pi, e.
//!
//! Coordinate variables: "x" and "y" for surfaces, "t" for curves.
//! Any other identifier (not a builtin constant or function) is treated as a
//! named parameter with a default value of 1.0.
//!
//! # Example
//! ```rust,ignore
//! use poincare_lib::expr_parser::{parse_surface_expr, eval_surface};
//!
//! let parsed = parse_surface_expr("a * sin(x^2 + y^2)").unwrap();
//! assert_eq!(parsed.parameters, vec![("a".to_string(), 1.0)]);
//! let z = eval_surface(&parsed, 1.0, 0.0, &parsed.parameters);
//! ```

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let mut num = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num.push(chars[i]);
                i += 1;
            }
            // Optional exponent
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                num.push(chars[i]);
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    num.push(chars[i]);
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    num.push(chars[i]);
                    i += 1;
                }
            }
            let v: f64 = num.parse().map_err(|_| format!("invalid number: {num}"))?;
            tokens.push(Token::Number(v));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut name = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                name.push(chars[i]);
                i += 1;
            }
            tokens.push(Token::Ident(name));
            continue;
        }
        let tok = match c {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '^' => Token::Caret,
            '(' => Token::LParen,
            ')' => Token::RParen,
            ',' => Token::Comma,
            _ => return Err(format!("unexpected character: {c}")),
        };
        tokens.push(tok);
        i += 1;
    }
    Ok(tokens)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Var(String),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    UnaryNeg(Box<Expr>),
    FnCall(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn consume(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// expr = additive
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_additive()
    }

    /// additive = multiplicative (('+' | '-') multiplicative)*
    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.consume();
                    let rhs = self.parse_multiplicative()?;
                    lhs = Expr::BinOp(BinOp::Add, Box::new(lhs), Box::new(rhs));
                }
                Some(Token::Minus) => {
                    self.consume();
                    let rhs = self.parse_multiplicative()?;
                    lhs = Expr::BinOp(BinOp::Sub, Box::new(lhs), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    /// multiplicative = unary (('*' | '/') unary)*
    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.consume();
                    let rhs = self.parse_unary()?;
                    lhs = Expr::BinOp(BinOp::Mul, Box::new(lhs), Box::new(rhs));
                }
                Some(Token::Slash) => {
                    self.consume();
                    let rhs = self.parse_unary()?;
                    lhs = Expr::BinOp(BinOp::Div, Box::new(lhs), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    /// unary = '-' unary | power
    fn parse_unary(&mut self) -> Result<Expr, String> {
        if let Some(Token::Minus) = self.peek() {
            self.consume();
            let inner = self.parse_unary()?;
            Ok(Expr::UnaryNeg(Box::new(inner)))
        } else {
            self.parse_power()
        }
    }

    /// power = primary ('^' unary)?   (right-associative)
    fn parse_power(&mut self) -> Result<Expr, String> {
        let base = self.parse_primary()?;
        if let Some(Token::Caret) = self.peek() {
            self.consume();
            let exp = self.parse_unary()?;
            Ok(Expr::BinOp(BinOp::Pow, Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    /// primary = Number | Ident '(' args ')' | Ident | '(' expr ')'
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Token::Number(v)) => {
                self.consume();
                Ok(Expr::Num(v))
            }
            Some(Token::Ident(name)) => {
                self.consume();
                if let Some(Token::LParen) = self.peek() {
                    // function call
                    self.consume(); // consume '('
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        args.push(self.parse_expr()?);
                        while let Some(Token::Comma) = self.peek() {
                            self.consume();
                            args.push(self.parse_expr()?);
                        }
                    }
                    match self.consume() {
                        Some(Token::RParen) => {}
                        _ => return Err(format!("expected ')' after arguments to {name}")),
                    }
                    Ok(Expr::FnCall(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Some(Token::LParen) => {
                self.consume();
                let inner = self.parse_expr()?;
                match self.consume() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err("expected ')'".to_string()),
                }
            }
            other => Err(format!("unexpected token: {other:?}")),
        }
    }
}

fn parse_one_expr(src: &str) -> Result<Expr, String> {
    let tokens = tokenize(src)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(format!(
            "unexpected token after expression: {:?}",
            parser.tokens[parser.pos]
        ));
    }
    Ok(expr)
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Evaluate an AST expression given a variable map.
///
/// Returns `f64::NAN` on any error (unknown variable, division by zero, etc.).
pub fn eval(expr: &Expr, vars: &HashMap<&str, f64>) -> f64 {
    match expr {
        Expr::Num(v) => *v,
        Expr::Var(name) => {
            if let Some(&v) = vars.get(name.as_str()) {
                v
            } else {
                f64::NAN
            }
        }
        Expr::BinOp(op, lhs, rhs) => {
            let l = eval(lhs, vars);
            let r = eval(rhs, vars);
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => l / r,
                BinOp::Pow => l.powf(r),
            }
        }
        Expr::UnaryNeg(inner) => -eval(inner, vars),
        Expr::FnCall(name, args) => {
            let arg0 = args.first().map(|a| eval(a, vars)).unwrap_or(f64::NAN);
            match name.as_str() {
                "sin" => arg0.sin(),
                "cos" => arg0.cos(),
                "tan" => arg0.tan(),
                "exp" => arg0.exp(),
                "ln" | "log" => arg0.ln(),
                "sqrt" => arg0.sqrt(),
                "abs" => arg0.abs(),
                "sinh" => arg0.sinh(),
                "cosh" => arg0.cosh(),
                "tanh" => arg0.tanh(),
                "asin" | "arcsin" => arg0.asin(),
                "acos" | "arccos" => arg0.acos(),
                "atan" | "arctan" => arg0.atan(),
                "atan2" | "arctan2" => {
                    let arg1 = args.get(1).map(|a| eval(a, vars)).unwrap_or(f64::NAN);
                    arg0.atan2(arg1)
                }
                "max" => {
                    let arg1 = args.get(1).map(|a| eval(a, vars)).unwrap_or(f64::NAN);
                    arg0.max(arg1)
                }
                "min" => {
                    let arg1 = args.get(1).map(|a| eval(a, vars)).unwrap_or(f64::NAN);
                    arg0.min(arg1)
                }
                "floor" => arg0.floor(),
                "ceil" => arg0.ceil(),
                "round" => arg0.round(),
                "sign" | "signum" => arg0.signum(),
                _ => f64::NAN, // unknown function
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Variable collection
// ---------------------------------------------------------------------------

/// The set of known function names that are NOT parameters.
fn known_functions() -> HashSet<&'static str> {
    [
        "sin", "cos", "tan", "exp", "ln", "log", "sqrt", "abs", "sinh", "cosh", "tanh", "asin",
        "acos", "atan", "atan2", "arcsin", "arccos", "arctan", "arctan2", "max", "min", "floor",
        "ceil", "round", "sign", "signum",
    ]
    .into_iter()
    .collect()
}

/// The set of known constant names that are NOT parameters.
fn known_constants() -> HashSet<&'static str> {
    ["pi", "e", "tau", "inf"].into_iter().collect()
}

/// Collect all identifiers referenced as variables (not as function names).
pub fn collect_variables(expr: &Expr) -> HashSet<String> {
    let mut vars = HashSet::new();
    collect_vars_inner(expr, &known_functions(), &mut vars);
    vars
}

fn collect_vars_inner(expr: &Expr, fns: &HashSet<&'static str>, out: &mut HashSet<String>) {
    match expr {
        Expr::Num(_) => {}
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::BinOp(_, l, r) => {
            collect_vars_inner(l, fns, out);
            collect_vars_inner(r, fns, out);
        }
        Expr::UnaryNeg(inner) => {
            collect_vars_inner(inner, fns, out);
        }
        Expr::FnCall(_, args) => {
            for a in args {
                collect_vars_inner(a, fns, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// A parsed mathematical expression ready for repeated evaluation.
#[derive(Clone)]
pub struct ParsedExpr {
    /// The original expression string as typed by the user.
    pub expression: String,
    /// AST for the expression.
    ast: Expr,
    /// Named parameters (not coordinate variables, not builtins) with default values.
    pub parameters: Vec<(String, f64)>,
}

/// Parse a surface expression `f(x, y)` and extract named parameters.
///
/// Coordinate variables: `x`, `y`.
/// Everything else (besides built-in functions/constants) is a named parameter.
pub fn parse_surface_expr(src: &str) -> Result<ParsedExpr, String> {
    let ast = parse_one_expr(src)?;
    let coord_vars: HashSet<&str> = ["x", "y"].into_iter().collect();
    let parameters = extract_parameters(&ast, &coord_vars);
    Ok(ParsedExpr {
        expression: src.to_string(),
        ast,
        parameters,
    })
}

/// Parse a parametric curve expression `(fx(t), fy(t), fz(t))`.
///
/// The outer parentheses are required. Components are separated by commas.
/// Coordinate variable: `t`.
/// Returns three `ParsedExpr` structs, one per component.
pub fn parse_curve_expr(src: &str) -> Result<(ParsedExpr, ParsedExpr, ParsedExpr), String> {
    let trimmed = src.trim();
    // Strip outer parens
    let inner = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // Split on commas respecting nesting depth.
    let parts = split_on_top_commas(inner)?;
    if parts.len() != 3 {
        return Err(format!(
            "parametric curve expects exactly 3 components, found {}",
            parts.len()
        ));
    }

    let coord_vars: HashSet<&str> = ["t"].into_iter().collect();

    let parse_component = |s: &str| -> Result<ParsedExpr, String> {
        let ast = parse_one_expr(s.trim())?;
        let parameters = extract_parameters(&ast, &coord_vars);
        Ok(ParsedExpr {
            expression: s.trim().to_string(),
            ast,
            parameters,
        })
    };

    let px = parse_component(&parts[0])?;
    let py = parse_component(&parts[1])?;
    let pz = parse_component(&parts[2])?;

    Ok((px, py, pz))
}

/// Evaluate a surface expression at `(x, y)` with the given parameter bindings.
pub fn eval_surface(parsed: &ParsedExpr, x: f64, y: f64, params: &[(String, f64)]) -> f64 {
    let mut vars: HashMap<&str, f64> = HashMap::new();
    vars.insert("x", x);
    vars.insert("y", y);
    vars.insert("pi", std::f64::consts::PI);
    vars.insert("e", std::f64::consts::E);
    vars.insert("tau", std::f64::consts::TAU);
    vars.insert("inf", f64::INFINITY);
    for (name, value) in params {
        vars.insert(name.as_str(), *value);
    }
    eval(&parsed.ast, &vars)
}

/// Evaluate a parametric curve at `t` with the given parameter bindings.
pub fn eval_curve_point(
    exprs: &(ParsedExpr, ParsedExpr, ParsedExpr),
    t: f64,
    params: &[(String, f64)],
) -> glam::DVec3 {
    let mut vars: HashMap<&str, f64> = HashMap::new();
    vars.insert("t", t);
    vars.insert("pi", std::f64::consts::PI);
    vars.insert("e", std::f64::consts::E);
    vars.insert("tau", std::f64::consts::TAU);
    vars.insert("inf", f64::INFINITY);
    for (name, value) in params {
        vars.insert(name.as_str(), *value);
    }
    glam::DVec3::new(
        eval(&exprs.0.ast, &vars),
        eval(&exprs.1.ast, &vars),
        eval(&exprs.2.ast, &vars),
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract parameters from `ast`: all variable identifiers that are not coordinate
/// variables and not known constants, deduplicated, in order of first appearance.
fn extract_parameters(ast: &Expr, coord_vars: &HashSet<&str>) -> Vec<(String, f64)> {
    let all_vars = collect_variables(ast);
    let constants = known_constants();

    let mut seen = HashSet::new();
    let mut params = Vec::new();

    // Walk the AST in order to produce a stable ordering.
    walk_vars_ordered(ast, &known_functions(), &mut |name: &str| {
        if coord_vars.contains(name) || constants.contains(name) || seen.contains(name) {
            return;
        }
        if all_vars.contains(name) {
            seen.insert(name.to_string());
            params.push((name.to_string(), 1.0));
        }
    });

    params
}

fn walk_vars_ordered(expr: &Expr, fns: &HashSet<&'static str>, visitor: &mut impl FnMut(&str)) {
    match expr {
        Expr::Num(_) => {}
        Expr::Var(name) => visitor(name),
        Expr::BinOp(_, l, r) => {
            walk_vars_ordered(l, fns, visitor);
            walk_vars_ordered(r, fns, visitor);
        }
        Expr::UnaryNeg(inner) => walk_vars_ordered(inner, fns, visitor),
        Expr::FnCall(_, args) => {
            for a in args {
                walk_vars_ordered(a, fns, visitor);
            }
        }
    }
}

/// Split a comma-separated string at top-level commas (not inside parentheses).
fn split_on_top_commas(s: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err("unmatched ')' in curve expression".to_string());
                }
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    Ok(parts)
}

// ---------------------------------------------------------------------------
// Generic public API (Phase 10)
// ---------------------------------------------------------------------------

/// Parse a math expression using an arbitrary set of coordinate variable names.
///
/// Any identifier that is not in `coord_vars`, not a known function, and not a known
/// constant is treated as a named parameter with default value 1.0.
///
/// # Example
/// ```rust,ignore
/// let parsed = parse_expr_with_vars("R + r * cos(v)", &["u", "v"]).unwrap();
/// // "R" and "r" become parameters; "v" is a coordinate variable.
/// ```
pub fn parse_expr_with_vars(src: &str, coord_vars: &[&str]) -> Result<ParsedExpr, String> {
    let ast = parse_one_expr(src)?;
    let coord_set: HashSet<&str> = coord_vars.iter().copied().collect();
    let parameters = extract_parameters(&ast, &coord_set);
    Ok(ParsedExpr {
        expression: src.to_string(),
        ast,
        parameters,
    })
}

/// Parse a triple expression `(expr1, expr2, expr3)` using an arbitrary variable set.
///
/// Accepts either `(a, b, c)` with outer parentheses or `a, b, c` without.
/// Each component is parsed independently with the given `coord_vars`.
pub fn parse_triple_expr(
    src: &str,
    coord_vars: &[&str],
) -> Result<(ParsedExpr, ParsedExpr, ParsedExpr), String> {
    let trimmed = src.trim();
    let inner = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let parts = split_on_top_commas(inner)?;
    if parts.len() != 3 {
        return Err(format!(
            "triple expression expects exactly 3 components, found {}",
            parts.len()
        ));
    }

    let coord_set: HashSet<&str> = coord_vars.iter().copied().collect();
    let parse_component = |s: &str| -> Result<ParsedExpr, String> {
        let ast = parse_one_expr(s.trim())?;
        let parameters = extract_parameters(&ast, &coord_set);
        Ok(ParsedExpr {
            expression: s.trim().to_string(),
            ast,
            parameters,
        })
    };

    let px = parse_component(&parts[0])?;
    let py = parse_component(&parts[1])?;
    let pz = parse_component(&parts[2])?;

    Ok((px, py, pz))
}

/// Evaluate a parsed expression with a generic set of (name, value) variable bindings.
///
/// Always injects `pi`, `e`, `tau`, and `inf` as built-in constants.
/// Returns `f64::NAN` for any unknown variable or function.
pub fn eval_with_vars(parsed: &ParsedExpr, vars: &[(&str, f64)]) -> f64 {
    let mut map: HashMap<&str, f64> = HashMap::new();
    map.insert("pi", std::f64::consts::PI);
    map.insert("e", std::f64::consts::E);
    map.insert("tau", std::f64::consts::TAU);
    map.insert("inf", f64::INFINITY);
    for (name, value) in vars {
        map.insert(name, *value);
    }
    eval(&parsed.ast, &map)
}

/// Parse CSV text where each non-blank line is `x,y,z` or `x,y,z,w`.
///
/// Returns `Ok(points)` on success, or `Err(errors)` containing `(line_number, message)` pairs.
/// The `w` component defaults to `0.0` if absent.
pub fn parse_csv_points(csv: &str) -> Result<Vec<[f64; 4]>, Vec<(usize, String)>> {
    let mut points = Vec::new();
    let mut errors = Vec::new();

    for (line_idx, raw) in csv.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 3 {
            errors.push((
                line_idx + 1,
                format!("expected at least 3 columns, got {}", fields.len()),
            ));
            continue;
        }
        let parse_f = |s: &str, col: &str| -> Result<f64, String> {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("invalid {col}: '{}'", s.trim()))
        };
        let x = match parse_f(fields[0], "x") {
            Ok(v) => v,
            Err(e) => {
                errors.push((line_idx + 1, e));
                continue;
            }
        };
        let y = match parse_f(fields[1], "y") {
            Ok(v) => v,
            Err(e) => {
                errors.push((line_idx + 1, e));
                continue;
            }
        };
        let z = match parse_f(fields[2], "z") {
            Ok(v) => v,
            Err(e) => {
                errors.push((line_idx + 1, e));
                continue;
            }
        };
        let w = if fields.len() >= 4 {
            match parse_f(fields[3], "w") {
                Ok(v) => v,
                Err(e) => {
                    errors.push((line_idx + 1, e));
                    continue;
                }
            }
        } else {
            0.0
        };
        points.push([x, y, z, w]);
    }

    if errors.is_empty() {
        Ok(points)
    } else {
        Err(errors)
    }
}

/// Parse an Excel-style grid paste.
///
/// Format: first row contains x-axis values (first cell is skipped).
/// Each remaining row: first cell = y value, then z values for each x column.
/// Returns `(xs, ys, zs)` where `zs[j * xs.len() + i]` = z at `(xs[i], ys[j])`.
pub fn parse_csv_grid(csv: &str) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let lines: Vec<Vec<&str>> = csv
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').collect())
        .collect();

    if lines.is_empty() {
        return Err("empty grid CSV".to_string());
    }

    // First row: skip first cell, rest are x values.
    let first_row = &lines[0];
    if first_row.len() < 2 {
        return Err("header row needs at least 2 columns (skip cell + x values)".to_string());
    }
    let xs: Result<Vec<f64>, _> = first_row[1..]
        .iter()
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("invalid x value: '{}'", s.trim()))
        })
        .collect();
    let xs = xs?;
    let n_x = xs.len();

    let data_rows = &lines[1..];
    if data_rows.is_empty() {
        return Err("grid CSV has no data rows".to_string());
    }

    let mut ys = Vec::new();
    let mut zs = Vec::new();

    for (row_idx, row) in data_rows.iter().enumerate() {
        if row.is_empty() {
            return Err(format!("row {} is empty", row_idx + 2));
        }
        let y: f64 = row[0]
            .trim()
            .parse()
            .map_err(|_| format!("invalid y value '{}' in row {}", row[0].trim(), row_idx + 2))?;
        ys.push(y);

        let z_cells = &row[1..];
        if z_cells.len() != n_x {
            return Err(format!(
                "row {} has {} z values but expected {}",
                row_idx + 2,
                z_cells.len(),
                n_x
            ));
        }
        for cell in z_cells {
            let z: f64 = cell
                .trim()
                .parse()
                .map_err(|_| format!("invalid z value '{}' in row {}", cell.trim(), row_idx + 2))?;
            zs.push(z);
        }
    }

    Ok((xs, ys, zs))
}

// ---------------------------------------------------------------------------
// Auto-detect
// ---------------------------------------------------------------------------

/// The coordinate system / plot type inferred from a free-form equation string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedPlotType {
    /// `y = f(x)`, `z = f(x)`, etc. — single independent variable → curve
    CartesianLine {
        dep: String,
        ind: String,
    },
    /// `z = f(x, y)` — standard Cartesian surface
    CartesianSurface,
    /// `y = f(x, z)` or `x = f(y, z)` — dep/ind stored as strings
    PermutedCartesian {
        dep: String,
        ind: (String, String),
    },
    /// `r = f(theta, phi)` — spherical surface
    SphericalSurface,
    /// `r = f(theta, z)` — cylindrical surface
    CylindricalSurface,
    /// `r = f(theta)` — polar curve/surface
    PolarSurface,
    /// Could not determine a unique type (no `=`, unknown variables, etc.)
    Unknown,
}

/// Result of auto-detecting the plot type from a free-form equation.
#[derive(Debug, Clone)]
pub struct AutoDetectResult {
    /// The most likely plot type.
    pub detected: DetectedPlotType,
    /// The dependent variable (left of `=`), e.g. `"z"`, `"r"`, `"y"`.
    pub dep_var: String,
    /// The independent variables used in the RHS (or specified explicitly).
    pub ind_vars: Vec<String>,
    /// The right-hand-side string (everything after `=`).
    pub rhs: String,
    /// A human-readable error if the input cannot be interpreted.
    pub error: Option<String>,
}

impl AutoDetectResult {
    fn unknown(error: impl Into<String>) -> Self {
        Self {
            detected: DetectedPlotType::Unknown,
            dep_var: String::new(),
            ind_vars: Vec::new(),
            rhs: String::new(),
            error: Some(error.into()),
        }
    }
}

/// Parse a free-form equation string and infer the plot type.
///
/// Accepted LHS forms:
/// - `z = rhs`              — infer ind vars from RHS
/// - `z(x, y) = rhs`       — explicit ind vars override RHS inference
///
/// Returns an [`AutoDetectResult`] with `detected == Unknown` (and an `error`) if
/// the input is missing `=`, the LHS is unparseable, or the variable set doesn't
/// match any known coordinate system.
pub fn auto_detect_plot_type(src: &str) -> AutoDetectResult {
    // --- split at first '=' ---
    let eq_pos = match src.find('=') {
        Some(p) => p,
        None => return AutoDetectResult::unknown("Missing '=' — enter an equation like z = x+y"),
    };
    let lhs_raw = src[..eq_pos].trim();
    let rhs = src[eq_pos + 1..].trim().to_string();

    if lhs_raw.is_empty() {
        return AutoDetectResult::unknown("Left-hand side is empty");
    }

    // --- parse LHS: `name` or `name(v1, v2, ...)` ---
    let (dep_var, explicit_ind_vars): (String, Option<Vec<String>>) =
        if let Some(paren_pos) = lhs_raw.find('(') {
            let dep = lhs_raw[..paren_pos].trim().to_string();
            let inner = lhs_raw[paren_pos + 1..].trim_end_matches(')').trim();
            let ind: Vec<String> = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (dep, Some(ind))
        } else {
            (lhs_raw.to_string(), None)
        };

    if dep_var.is_empty() {
        return AutoDetectResult::unknown("Could not parse left-hand side");
    }

    // --- extract identifiers from RHS (cheap tokenizer scan) ---
    let rhs_idents = extract_idents_from_rhs(&rhs);

    // --- determine independent vars ---
    let ind_vars: Vec<String> = if let Some(explicit) = explicit_ind_vars {
        explicit
    } else {
        // Infer from RHS, filtering known functions/constants and the dep var
        let fns = known_functions();
        let consts = known_constants();
        let mut vars: Vec<String> = rhs_idents
            .into_iter()
            .filter(|id| {
                id != &dep_var
                    && !fns.contains(id.as_str())
                    && !consts.contains(id.as_str())
            })
            .collect();
        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        vars.retain(|v| seen.insert(v.clone()));
        vars
    };

    // --- match dep_var + ind_var set → DetectedPlotType ---
    let ind_set: std::collections::HashSet<&str> =
        ind_vars.iter().map(|s| s.as_str()).collect();

    let detected = match dep_var.as_str() {
        "z" => {
            let has_x = ind_set.contains("x");
            let has_y = ind_set.contains("y");
            match (has_x, has_y) {
                // Both x and y present → surface
                (true, true) => DetectedPlotType::CartesianSurface,
                // Only one var → lowest-dimension line
                (true, false) => DetectedPlotType::CartesianLine {
                    dep: "z".to_string(),
                    ind: "x".to_string(),
                },
                (false, true) => DetectedPlotType::CartesianLine {
                    dep: "z".to_string(),
                    ind: "y".to_string(),
                },
                // No Cartesian vars (constant) → surface (z = c plane)
                (false, false) => DetectedPlotType::CartesianSurface,
            }
        }
        "y" => {
            let has_x = ind_set.contains("x");
            let has_z = ind_set.contains("z");
            match (has_x, has_z) {
                // Both x and z → permuted Cartesian surface
                (true, true) => DetectedPlotType::PermutedCartesian {
                    dep: "y".to_string(),
                    ind: ("x".to_string(), "z".to_string()),
                },
                // Only one var → lowest-dimension line
                (true, false) => DetectedPlotType::CartesianLine {
                    dep: "y".to_string(),
                    ind: "x".to_string(),
                },
                (false, true) => DetectedPlotType::CartesianLine {
                    dep: "y".to_string(),
                    ind: "z".to_string(),
                },
                // No vars (constant) → y(x) line by default
                (false, false) => DetectedPlotType::CartesianLine {
                    dep: "y".to_string(),
                    ind: "x".to_string(),
                },
            }
        }
        "x" => {
            let has_y = ind_set.contains("y");
            let has_z = ind_set.contains("z");
            match (has_y, has_z) {
                // Both y and z → permuted Cartesian surface
                (true, true) => DetectedPlotType::PermutedCartesian {
                    dep: "x".to_string(),
                    ind: ("y".to_string(), "z".to_string()),
                },
                // Only one var → lowest-dimension line
                (true, false) => DetectedPlotType::CartesianLine {
                    dep: "x".to_string(),
                    ind: "y".to_string(),
                },
                (false, true) => DetectedPlotType::CartesianLine {
                    dep: "x".to_string(),
                    ind: "z".to_string(),
                },
                // No vars (constant) → x(y) line by default
                (false, false) => DetectedPlotType::CartesianLine {
                    dep: "x".to_string(),
                    ind: "y".to_string(),
                },
            }
        }
        "r" => {
            // Spherical: r = f(theta, phi)
            if ind_set.contains("phi") {
                DetectedPlotType::SphericalSurface
            // Cylindrical: r = f(theta, z)
            } else if ind_set.contains("z") {
                DetectedPlotType::CylindricalSurface
            // Polar (lowest-dim default): r = f(theta) or r = constant
            } else {
                DetectedPlotType::PolarSurface
            }
        }
        _ => DetectedPlotType::Unknown,
    };

    let error = if detected == DetectedPlotType::Unknown {
        Some(format!(
            "Unknown equation type for '{dep_var} = ...' with variables {ind_vars:?}"
        ))
    } else {
        None
    };

    AutoDetectResult {
        detected,
        dep_var,
        ind_vars,
        rhs,
        error,
    }
}

/// Quick scan: extract all alphabetic identifiers from a raw expression string.
/// Does not parse — just tokenizes identifiers for variable inference.
fn extract_idents_from_rhs(rhs: &str) -> Vec<String> {
    let mut idents = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let chars: Vec<char> = rhs.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            if seen.insert(ident.clone()) {
                idents.push(ident);
            }
        } else {
            i += 1;
        }
    }
    idents
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_expression() {
        let p = parse_surface_expr("3.14").unwrap();
        assert!(p.parameters.is_empty());
        let z = eval_surface(&p, 0.0, 0.0, &[]);
        assert!((z - 3.14).abs() < 1e-10);
    }

    #[test]
    fn simple_surface() {
        let p = parse_surface_expr("x + y").unwrap();
        assert!(p.parameters.is_empty());
        let z = eval_surface(&p, 1.0, 2.0, &[]);
        assert!((z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn sinc_surface() {
        // sin(x^2+y^2)/(x^2+y^2)
        let p = parse_surface_expr("sin(x^2+y^2)/(x^2+y^2)").unwrap();
        assert!(p.parameters.is_empty());
        let z = eval_surface(&p, 1.0, 0.0, &[]);
        let expected = 1.0_f64.sin() / 1.0;
        assert!((z - expected).abs() < 1e-10);
    }

    #[test]
    fn parameter_extraction() {
        let p = parse_surface_expr("a * sin(x) + b * cos(y)").unwrap();
        assert_eq!(p.parameters.len(), 2);
        assert_eq!(p.parameters[0].0, "a");
        assert_eq!(p.parameters[1].0, "b");
    }

    #[test]
    fn parametric_curve() {
        let (px, py, pz) = parse_curve_expr("(cos(t), sin(t), t / 5)").unwrap();
        assert!(px.parameters.is_empty());
        let p = (px, py, pz);
        let point = eval_curve_point(&p, 0.0, &[]);
        assert!((point.x - 1.0).abs() < 1e-10);
        assert!((point.y - 0.0).abs() < 1e-10);
        assert!((point.z - 0.0).abs() < 1e-10);
    }

    #[test]
    fn pi_and_e_are_not_parameters() {
        let p = parse_surface_expr("sin(pi * x) + e").unwrap();
        assert!(p.parameters.is_empty());
    }

    #[test]
    fn power_operator() {
        let p = parse_surface_expr("x^2 + y^2").unwrap();
        let z = eval_surface(&p, 3.0, 4.0, &[]);
        assert!((z - 25.0).abs() < 1e-10);
    }

    #[test]
    fn unary_negation() {
        let p = parse_surface_expr("-x + y").unwrap();
        let z = eval_surface(&p, 3.0, 1.0, &[]);
        assert!((z - (-2.0)).abs() < 1e-10);
    }
}
