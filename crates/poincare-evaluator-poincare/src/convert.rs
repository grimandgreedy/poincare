//! Conversions from `poincare-lang` runtime values and diagnostics into the
//! backend-neutral `poincare-evaluator` types.
//!
//! Graph/plot translation is minimal in this phase: every language plot is
//! mapped to a `poincare-lib` `ExprCartesian` plot with a best-effort constant
//! expression and any range/resolution fields honored. Faithful
//! expression-domain plotting needs lazy/symbolic plot arguments and is
//! deferred; the structure still round-trips so graph outputs appear.

use poincare_evaluator::{
    AttachmentValue, BytesValue, EvalAttachmentId, EvalDiagnostic, EvalDiagnosticSeverity,
    EvalValue, FunctionValue, NumberValue, SourcePosition, SourceSpan, TableValue, ValueKind,
};
use poincare_lang::{Diagnostic, Graph, Plot, RuntimeError, Severity, SourceMap, Span, Table, Value};
use poincare_lib::{Domain, GraphSpec, PlotDefinition, PlotSpec, PlotStyle, Resolution};

/// Convert a language value into an evaluator value.
pub fn to_eval_value(value: &Value) -> EvalValue {
    match value {
        Value::Unit => EvalValue::Unit,
        Value::Num(n) => EvalValue::Number(to_number(*n)),
        Value::Bool(b) => EvalValue::Bool(*b),
        Value::Str(s) => EvalValue::String(s.to_string()),
        Value::Bytes(b) => EvalValue::Bytes(BytesValue {
            attachment: None,
            len: b.len(),
            preview_hex: None,
        }),
        Value::List(items) => EvalValue::List(items.iter().map(to_eval_value).collect()),
        Value::Range(lo, hi) => EvalValue::String(format!("{lo}..{hi}")),
        Value::Table(t) => EvalValue::Table(table_to_eval(t)),
        Value::Plot(p) => EvalValue::Plot(plot_to_spec(p)),
        Value::Graph(g) => EvalValue::Graph(graph_to_spec(g)),
        Value::Attachment(name) => EvalValue::Attachment(AttachmentValue {
            id: EvalAttachmentId::new(name.to_string()),
            display_name: name.to_string(),
            media_type: None,
            size_bytes: None,
            hash: None,
        }),
        Value::Closure(_) => EvalValue::Function(FunctionValue {
            name: None,
            parameters: value.closure_params().unwrap_or_default(),
        }),
        Value::Builtin(name) => EvalValue::Function(FunctionValue {
            name: Some((*name).to_string()),
            parameters: Vec::new(),
        }),
        Value::Expr(e) => EvalValue::String(e.source.clone()),
    }
}

/// Map a language value to its kind, for variable summaries.
pub fn value_kind(value: &Value) -> ValueKind {
    match value {
        Value::Unit => ValueKind::Unit,
        Value::Num(_) => ValueKind::Number,
        Value::Bool(_) => ValueKind::Bool,
        Value::Str(_) => ValueKind::String,
        Value::Bytes(_) => ValueKind::Bytes,
        Value::List(_) => ValueKind::List,
        Value::Range(_, _) => ValueKind::Unknown,
        Value::Table(_) => ValueKind::Table,
        Value::Plot(_) => ValueKind::Plot,
        Value::Graph(_) => ValueKind::Graph,
        Value::Attachment(_) => ValueKind::Attachment,
        Value::Closure(_) | Value::Builtin(_) => ValueKind::Function,
        Value::Expr(_) => ValueKind::Expression,
    }
}

fn to_number(n: f64) -> NumberValue {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
        NumberValue::Int(n as i64)
    } else {
        NumberValue::Float(n)
    }
}

fn table_to_eval(table: &Table) -> TableValue {
    TableValue {
        title: None,
        columns: table.columns.clone(),
        rows: table
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.display()).collect())
            .collect(),
        truncated: false,
    }
}

fn graph_to_spec(graph: &Graph) -> GraphSpec {
    let mut spec = GraphSpec::new();
    spec.plots = graph.plots.iter().map(plot_to_spec).collect();
    spec
}

fn plot_to_spec(plot: &Plot) -> PlotSpec {
    let mut domain = Domain::default();
    let mut resolution = Resolution::default();
    for (name, value) in &plot.fields {
        match (name.as_str(), value) {
            ("x", Value::Range(lo, hi)) => domain.x = *lo..=*hi,
            ("y", Value::Range(lo, hi)) => domain.y = *lo..=*hi,
            ("z", Value::Range(lo, hi)) => domain.z = *lo..=*hi,
            ("resolution", Value::List(items)) if items.len() == 2 => {
                if let (Value::Num(u), Value::Num(v)) = (&items[0], &items[1]) {
                    resolution = Resolution {
                        u: *u as u32,
                        v: *v as u32,
                    };
                }
            }
            _ => {}
        }
    }
    let (expression, parameters) = plot_expression(plot);
    // A `curve` is a 2D line `y = f(x)`; everything else is treated as a
    // cartesian surface `z = f(x, y)`.
    let definition = if plot.kind == "curve" {
        PlotDefinition::ExprCartesianLine {
            dep_var: "y".to_string(),
            ind_var: "x".to_string(),
            expression,
            parameters,
        }
    } else {
        PlotDefinition::ExprCartesian {
            expression,
            parameters,
        }
    };
    PlotSpec {
        name: plot.kind.clone(),
        visible: true,
        domain,
        resolution,
        style: PlotStyle::default(),
        definition,
    }
}

/// The expression string and numeric parameters for a plot's primary scalar
/// field. A captured formula (`Value::Expr`) supplies its source text and any
/// parameters it closed over; other values fall back to their display form.
fn plot_expression(plot: &Plot) -> (String, Vec<(String, f64)>) {
    for key in ["z", "y", "expr", "value"] {
        if let Some((_, value)) = plot.fields.iter().find(|(name, _)| name == key) {
            return match value {
                Value::Expr(e) => (e.source.clone(), e.params.clone()),
                other => (other.display(), Vec::new()),
            };
        }
    }
    ("0".to_string(), Vec::new())
}

// --- diagnostics ---

pub fn diagnostic_to_eval(diagnostic: &Diagnostic, map: &SourceMap) -> EvalDiagnostic {
    EvalDiagnostic {
        severity: match diagnostic.severity {
            Severity::Error => EvalDiagnosticSeverity::Error,
            Severity::Warning => EvalDiagnosticSeverity::Warning,
        },
        message: diagnostic.message.clone(),
        span: Some(span_to_source_span(diagnostic.span, map)),
        code: None,
    }
}

pub fn runtime_error_to_eval(error: &RuntimeError, map: &SourceMap) -> EvalDiagnostic {
    EvalDiagnostic {
        severity: EvalDiagnosticSeverity::Error,
        message: error.message.clone(),
        span: Some(span_to_source_span(error.span, map)),
        code: None,
    }
}

fn span_to_source_span(span: Span, map: &SourceMap) -> SourceSpan {
    let start = map.location(span.start);
    let end = map.location(span.end);
    SourceSpan {
        start: SourcePosition {
            line: start.line,
            column: start.column,
            offset: start.offset,
        },
        end: SourcePosition {
            line: end.line,
            column: end.column,
            offset: end.offset,
        },
    }
}
