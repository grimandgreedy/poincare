//! Interpreter tests: normal execution and runtime errors.

use poincare_lang::{Interpreter, Limits, RunOutcome, parse};

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
    assert!(
        outcome.error.is_none(),
        "runtime error: {:?}",
        outcome.error
    );
    outcome.value_display.unwrap_or_else(|| "()".to_string())
}

fn output_of(src: &str) -> Vec<String> {
    let outcome = run(src);
    assert!(
        outcome.error.is_none(),
        "runtime error: {:?}",
        outcome.error
    );
    outcome.output
}

fn expect_error(src: &str) -> String {
    let outcome = run(src);
    outcome.error.expect("expected a runtime error").message
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(value_of("1 + 2 * 3"), "7");
    assert_eq!(value_of("(1 + 2) * 3"), "9");
    assert_eq!(value_of("2 ^ 3 ^ 2"), "512");
    assert_eq!(value_of("-2 ^ 2"), "-4");
    assert_eq!(value_of("7 % 3"), "1");
}

#[test]
fn comparisons_and_logic() {
    assert_eq!(value_of("3 > 2"), "true");
    assert_eq!(value_of("3 == 3"), "true");
    assert_eq!(value_of("3 != 3"), "false");
    assert_eq!(value_of("true and false"), "false");
    assert_eq!(value_of("true or false"), "true");
    assert_eq!(value_of("not true"), "false");
}

#[test]
fn bindings_persist_and_are_used() {
    assert_eq!(value_of("x = 3\nx * 2"), "6");
}

#[test]
fn strings_and_booleans() {
    assert_eq!(value_of("\"hello\""), "hello");
    assert_eq!(value_of("true"), "true");
}

#[test]
fn print_writes_to_output_stream() {
    let out = output_of("print(\"a =\", 1 + 2)");
    assert_eq!(out, vec!["a = 3".to_string()]);
}

#[test]
fn expression_function_call() {
    assert_eq!(value_of("f(x) = x * x\nf(5)"), "25");
}

#[test]
fn block_function_with_tail_value() {
    assert_eq!(value_of("fn g(a) {\n  b = a + 1\n  b * 2\n}\ng(4)"), "10");
}

#[test]
fn recursion() {
    let src = "fact(n) = if n <= 1 { 1 } else { n * fact(n - 1) }\nfact(5)";
    assert_eq!(value_of(src), "120");
}

#[test]
fn mutual_recursion_across_top_level() {
    let src = "\
is_even(n) = if n == 0 { true } else { is_odd(n - 1) }
is_odd(n) = if n == 0 { false } else { is_even(n - 1) }
is_even(10)";
    assert_eq!(value_of(src), "true");
}

#[test]
fn closures_capture_environment() {
    let src = "make_adder(n) = x => x + n\nadd3 = make_adder(3)\nadd3(4)";
    assert_eq!(value_of(src), "7");
}

#[test]
fn if_expression_returns_value() {
    assert_eq!(value_of("if 2 > 1 { 10 } else { 20 }"), "10");
    assert_eq!(value_of("if 2 < 1 { 10 } else { 20 }"), "20");
}

#[test]
fn for_loop_accumulates_via_print() {
    let out = output_of("for a in [1, 2, 3] {\n  print(a)\n}");
    assert_eq!(out, vec!["1", "2", "3"]);
}

#[test]
fn for_loop_over_range() {
    let out = output_of("for i in 1..3 {\n  print(i)\n}");
    assert_eq!(out, vec!["1", "2", "3"]);
}

#[test]
fn lists_and_indexing() {
    assert_eq!(value_of("xs = [10, 20, 30]\nxs[1]"), "20");
}

#[test]
fn math_builtins() {
    assert_eq!(value_of("sqrt(16)"), "4");
    assert_eq!(value_of("abs(-5)"), "5");
    assert_eq!(value_of("max(1, 7, 3)"), "7");
    assert_eq!(value_of("floor(2.9)"), "2");
}

#[test]
fn pi_is_a_constant_not_a_builtin() {
    let outcome = run("floor(pi)");
    assert!(outcome.error.is_none());
    assert_eq!(outcome.value_display.unwrap(), "3");
}

#[test]
fn pipe_desugars_to_application() {
    assert_eq!(value_of("f(x) = x + 1\n5 |> f"), "6");
}

#[test]
fn composition_desugars_to_lambda() {
    let src = "inc(x) = x + 1\ndouble(x) = x * 2\nboth = double . inc\nboth(3)";
    // double(inc(3)) = double(4) = 8
    assert_eq!(value_of(src), "8");
}

#[test]
fn session_persists_across_runs() {
    let mut interp = Interpreter::new();
    let cell1 = parse("a = 40");
    interp.run(&cell1.program);
    let cell2 = parse("a + 2");
    let outcome = interp.run(&cell2.program);
    assert_eq!(outcome.value_display.as_deref(), Some("42"));
}

// --- runtime errors ---

#[test]
fn type_error_on_bad_arithmetic() {
    let msg = expect_error("1 + true");
    assert!(msg.contains("expected a number"), "{msg}");
}

#[test]
fn calling_a_non_function_errors() {
    let msg = expect_error("x = 3\nx(1)");
    assert!(msg.contains("not callable"), "{msg}");
}

#[test]
fn index_out_of_bounds_errors() {
    let msg = expect_error("xs = [1, 2]\nxs[5]");
    assert!(msg.contains("out of bounds"), "{msg}");
}

#[test]
fn plot_is_not_executable_yet() {
    let msg = expect_error("plot surface {\n  z = 1\n  x = -6..6\n}");
    assert!(msg.contains("not executable yet"), "{msg}");
}

#[test]
fn error_stops_the_cell_but_keeps_prior_output() {
    let outcome = run("print(\"before\")\n1 + true\nprint(\"after\")");
    assert!(outcome.error.is_some());
    assert_eq!(outcome.output, vec!["before".to_string()]);
}

#[test]
fn loop_iteration_limit_is_enforced() {
    let mut interp = Interpreter::with_limits(Limits {
        max_loop_iterations: 100,
        max_call_depth: 256,
    });
    // A range that would iterate far past the limit.
    let program = parse("for i in 1..100000 {\n  i\n}");
    let outcome = interp.run(&program.program);
    let msg = outcome.error.expect("expected limit error").message;
    assert!(msg.contains("iteration limit"), "{msg}");
}

#[test]
fn call_depth_limit_catches_infinite_recursion() {
    let mut interp = Interpreter::with_limits(Limits {
        max_loop_iterations: 10_000_000,
        max_call_depth: 64,
    });
    let program = parse("loop(n) = loop(n + 1)\nloop(0)");
    let outcome = interp.run(&program.program);
    let msg = outcome.error.expect("expected depth error").message;
    assert!(msg.contains("call depth"), "{msg}");
}
