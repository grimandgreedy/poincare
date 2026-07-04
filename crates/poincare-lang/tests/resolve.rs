//! Name-resolution and scope tests.

use poincare_lang::diagnostic::Severity;
use poincare_lang::{SessionScope, parse, resolve};

fn resolve_src(src: &str, session: &SessionScope) -> poincare_lang::ResolveResult {
    let parsed = parse(src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse errors in {src:?}: {:?}",
        parsed.diagnostics
    );
    resolve(&parsed.program, session)
}

fn resolve_ok(src: &str) -> poincare_lang::ResolveResult {
    let result = resolve_src(src, &SessionScope::new());
    assert!(
        !result.has_errors(),
        "unexpected resolve errors for {src:?}: {:?}",
        result.diagnostics
    );
    result
}

fn resolve_errs(src: &str) -> poincare_lang::ResolveResult {
    let result = resolve_src(src, &SessionScope::new());
    assert!(
        result.has_errors(),
        "expected a resolve error for {src:?} but got none"
    );
    result
}

#[test]
fn bindings_and_builtins_resolve() {
    resolve_ok("x = 3\ny = sin(x) + 1");
}

#[test]
fn undefined_name_is_an_error() {
    let r = resolve_errs("y = x + 1");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined name `x`"))
    );
}

#[test]
fn forward_function_reference_resolves() {
    // f references g defined later; hoisting makes this legal.
    resolve_ok("f(x) = g(x) + 1\ng(x) = x * 2");
}

#[test]
fn self_recursion_resolves() {
    resolve_ok("fact(n) = if n <= 1 { 1 } else { n * fact(n - 1) }");
}

#[test]
fn duplicate_parameter_is_an_error() {
    let r = resolve_errs("f(x, x) = x");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("bound more than once"))
    );
}

#[test]
fn shadowing_a_builtin_warns_but_is_allowed() {
    let r = resolve_src("sin = 3", &SessionScope::new());
    assert!(!r.has_errors());
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("shadows a builtin"))
    );
}

#[test]
fn session_names_are_visible() {
    let session = SessionScope::from_iter(["a"]);
    let result = resolve_src("y = a + 1", &session);
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
}

#[test]
fn cell_defs_are_reported() {
    let r = resolve_ok("a = 1\nfn g() {\n  1\n}");
    assert!(r.cell_defs.contains(&"a".to_string()));
    assert!(r.cell_defs.contains(&"g".to_string()));
}

#[test]
fn session_grows_from_cell_defs() {
    // Cell 1 defines `a`; cell 2 can then use it.
    let mut session = SessionScope::new();
    let cell1 = resolve_src("a = 3", &session);
    assert!(!cell1.has_errors());
    session.extend_from_defs(cell1.cell_defs);

    let cell2 = resolve_src("b = a + 1", &session);
    assert!(!cell2.has_errors(), "{:?}", cell2.diagnostics);
}

#[test]
fn block_locals_do_not_leak() {
    // `z` is local to the function body; using it at top level is undefined.
    let r = resolve_errs("fn h() {\n  z = 5\n  z\n}\nprint(z)");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined name `z`"))
    );
}

#[test]
fn loop_variable_is_in_scope() {
    resolve_ok("for i in [1, 2, 3] {\n  print(i)\n}");
}

#[test]
fn lambda_parameters_are_in_scope() {
    resolve_ok("f = x => x + 1");
    let r = resolve_errs("f = x => y");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined name `y`"))
    );
}

#[test]
fn plot_domain_variables_are_in_scope() {
    let src = "f(x, y) = x\nplot surface {\n  z = f(x, y)\n  x = -6..6\n  y = -6..6\n}";
    resolve_ok(src);
}

#[test]
fn undefined_name_in_plot_field_is_an_error() {
    let r = resolve_errs("plot surface {\n  z = missing\n  x = -6..6\n}");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined name `missing`"))
    );
}

#[test]
fn signature_without_definition_warns() {
    let r = resolve_src("f : R -> R", &SessionScope::new());
    assert!(!r.has_errors());
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("no matching definition"))
    );
}

#[test]
fn signature_with_definition_is_clean() {
    resolve_ok("f : R^2 -> R\nf(x, y) = x + y");
}

#[test]
fn attachment_pipeline_resolves() {
    let src = "\
data = csv(attachment(\"samples.csv\"))
points = scatter(data, x = \"x\", y = \"y\", z = \"z\")
plot points";
    resolve_ok(src);
}
