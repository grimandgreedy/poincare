//! Phase 5 builtin tests: expanded math, list/higher-order, graphs, tables,
//! attachments, and analysis.

use std::collections::HashMap;

use poincare_lang::{Host, Interpreter, RunOutcome, Value, parse};

fn run(src: &str) -> RunOutcome {
    let parsed = parse(src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse errors in {src:?}: {:?}",
        parsed.diagnostics
    );
    let mut interp = Interpreter::new();
    interp.run(&parsed.program)
}

fn value_of(src: &str) -> String {
    let outcome = run(src);
    assert!(outcome.error.is_none(), "runtime error: {:?}", outcome.error);
    outcome.value_display.unwrap_or_else(|| "()".to_string())
}

fn expect_error(src: &str) -> String {
    run(src).error.expect("expected a runtime error").message
}

// An in-memory host for attachment tests.
struct MemoryHost {
    files: HashMap<String, String>,
}

impl Host for MemoryHost {
    fn attachment_text(&self, name_or_id: &str) -> Result<String, String> {
        self.files
            .get(name_or_id)
            .cloned()
            .ok_or_else(|| format!("no attachment `{name_or_id}`"))
    }
    fn attachment_bytes(&self, name_or_id: &str) -> Result<Vec<u8>, String> {
        self.attachment_text(name_or_id).map(|s| s.into_bytes())
    }
}

fn run_with_host(src: &str, files: &[(&str, &str)]) -> RunOutcome {
    let parsed = parse(src);
    assert!(parsed.diagnostics.is_empty(), "parse errors: {:?}", parsed.diagnostics);
    let host = MemoryHost {
        files: files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    };
    let mut interp = Interpreter::new();
    interp.run_with_host(&parsed.program, &host)
}

// --- expanded math ---

#[test]
fn expanded_math_builtins() {
    assert_eq!(value_of("pow(2, 10)"), "1024");
    assert_eq!(value_of("round(2.6)"), "3");
    assert_eq!(value_of("sign(-4)"), "-1");
    assert_eq!(value_of("log2(8)"), "3");
    assert_eq!(value_of("trunc(3.9)"), "3");
}

// --- list / higher-order ---

#[test]
fn list_reductions() {
    assert_eq!(value_of("sum([1, 2, 3, 4])"), "10");
    assert_eq!(value_of("mean([2, 4, 6])"), "4");
    assert_eq!(value_of("prod([1, 2, 3, 4])"), "24");
    assert_eq!(value_of("len([10, 20, 30])"), "3");
}

#[test]
fn map_applies_a_function() {
    assert_eq!(value_of("double(x) = x * 2\nmap([1, 2, 3], double)"), "[2, 4, 6]");
}

#[test]
fn map_with_lambda() {
    assert_eq!(value_of("map([1, 2, 3], x => x + 1)"), "[2, 3, 4]");
}

#[test]
fn filter_keeps_matching_items() {
    assert_eq!(value_of("filter([1, 2, 3, 4], x => x > 2)"), "[3, 4]");
}

// --- graphs and plots ---

#[test]
fn surface_builds_a_plot_value() {
    let outcome = run("surface(z = 1, x = -6..6, y = -6..6)");
    assert!(outcome.error.is_none());
    let display = outcome.value_display.unwrap();
    assert_eq!(display, "<plot surface>");
}

#[test]
fn graph_and_add_plot_accumulate() {
    let src = "\
g = graph()
g = add_plot(g, surface(z = 1, x = -3..3, y = -3..3))
g = add_plot(g, curve(y = 2))
g";
    assert_eq!(value_of(src), "<graph 2 plots>");
}

#[test]
fn emit_collects_structured_outputs() {
    let outcome = run("emit(surface(z = 1))");
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.emitted.len(), 1);
    assert!(matches!(&outcome.emitted[0], Value::Plot(p) if p.kind == "surface"));
}

#[test]
fn surface_captures_named_fields() {
    let outcome = run("emit(surface(z = 42, x = -1..1))");
    match &outcome.emitted[0] {
        Value::Plot(p) => {
            assert_eq!(p.kind, "surface");
            assert!(p.fields.iter().any(|(name, _)| name == "z"));
            assert!(p.fields.iter().any(|(name, _)| name == "x"));
        }
        other => panic!("expected a plot, got {other:?}"),
    }
}

#[test]
fn surface_formula_is_captured_unevaluated_over_coordinates() {
    // `x` and `y` are unbound coordinate variables; capturing the formula must
    // not try to evaluate them.
    let outcome = run("emit(surface(z = x^2 + y^2, x = -3..3, y = -3..3))");
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let plot = match &outcome.emitted[0] {
        Value::Plot(p) => p,
        other => panic!("expected a plot, got {other:?}"),
    };
    let (_, z) = plot
        .fields
        .iter()
        .find(|(name, _)| name == "z")
        .expect("z field");
    match z {
        Value::Expr(e) => {
            assert_eq!(e.source, "((x ^ 2) + (y ^ 2))");
            assert!(e.params.is_empty());
        }
        other => panic!("expected a captured expression, got {other:?}"),
    }
}

#[test]
fn curve_formula_supports_builtin_calls() {
    let outcome = run("emit(curve(y = sin(x), x = -6..6))");
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    match &outcome.emitted[0] {
        Value::Plot(p) => {
            let (_, y) = p.fields.iter().find(|(name, _)| name == "y").unwrap();
            assert!(matches!(y, Value::Expr(e) if e.source == "sin(x)"));
        }
        other => panic!("expected a plot, got {other:?}"),
    }
}

#[test]
fn formula_captures_bound_scalars_as_parameters() {
    // `amp` is bound to a number and is not a coordinate variable, so it is
    // recorded as a parameter while `x`/`y` stay free.
    let src = "\
amp = 2
emit(surface(z = amp * (x^2 + y^2), x = -3..3, y = -3..3))";
    let outcome = run(src);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    match &outcome.emitted[0] {
        Value::Plot(p) => {
            let (_, z) = p.fields.iter().find(|(name, _)| name == "z").unwrap();
            match z {
                Value::Expr(e) => {
                    assert_eq!(e.source, "(amp * ((x ^ 2) + (y ^ 2)))");
                    assert_eq!(e.params, vec![("amp".to_string(), 2.0)]);
                }
                other => panic!("expected a captured expression, got {other:?}"),
            }
        }
        other => panic!("expected a plot, got {other:?}"),
    }
}

// --- tables and CSV ---

#[test]
fn csv_string_parses_into_a_table() {
    let src = "\
data = csv(\"x,y\n1,2\n3,4\")
column(data, \"y\")";
    assert_eq!(value_of(src), "[2, 4]");
}

#[test]
fn table_columns_and_rows() {
    assert_eq!(value_of("columns(csv(\"a,b\n1,2\"))"), "[a, b]");
    assert_eq!(value_of("len(csv(\"a,b\n1,2\n3,4\"))"), "2");
}

#[test]
fn csv_matrix_parses_numeric_grid() {
    assert_eq!(value_of("csv_matrix(\"1,2\n3,4\")"), "[[1, 2], [3, 4]]");
}

#[test]
fn csv_matrix_rejects_non_numeric() {
    let msg = expect_error("csv_matrix(\"1,x\n3,4\")");
    assert!(msg.contains("non-numeric"), "{msg}");
}

// --- attachments via host ---

#[test]
fn attachment_text_via_host() {
    let outcome = run_with_host(
        "text(attachment(\"notes.txt\"))",
        &[("notes.txt", "hello world")],
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.value_display.as_deref(), Some("hello world"));
}

#[test]
fn csv_from_attachment() {
    let outcome = run_with_host(
        "data = csv(attachment(\"d.csv\"))\ncolumn(data, \"x\")",
        &[("d.csv", "x,y\n10,20\n30,40")],
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.value_display.as_deref(), Some("[10, 30]"));
}

#[test]
fn attachment_without_host_errors() {
    let msg = expect_error("text(attachment(\"missing.txt\"))");
    assert!(msg.contains("no attachment host"), "{msg}");
}

// --- analysis ---

#[test]
fn numeric_derivative() {
    // d/dx x^2 at x=3 is 6.
    let src = "sq(x) = x * x\nround(derivative(sq, 3))";
    assert_eq!(value_of(src), "6");
}

#[test]
fn unimplemented_analysis_builtin_errors() {
    let msg = expect_error("fit([1, 2, 3])");
    assert!(msg.contains("not implemented yet"), "{msg}");
}
