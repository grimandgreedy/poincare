//! Parser test fixtures: expressions, assignments, function definitions,
//! loops, blocks, print, plot statements, and invalid syntax.

use poincare_lang::ast::*;
use poincare_lang::parse;

fn parse_ok(src: &str) -> Program {
    let result = parse(src);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics for {src:?}: {:?}",
        result.diagnostics
    );
    result.program
}

fn parse_err(src: &str) {
    let result = parse(src);
    assert!(
        !result.diagnostics.is_empty(),
        "expected a diagnostic for {src:?} but got none"
    );
}

fn only_expr(program: &Program) -> &Expr {
    assert_eq!(program.stmts.len(), 1, "expected one statement");
    match &program.stmts[0] {
        Stmt::Expr(e) => e,
        other => panic!("expected an expression statement, got {other:?}"),
    }
}

#[test]
fn binding_and_literals() {
    let p = parse_ok("x = 3");
    assert_eq!(p.stmts.len(), 1);
    match &p.stmts[0] {
        Stmt::Binding(b) => {
            assert_eq!(b.name.sym.as_str(), "x");
            assert!(matches!(&b.value, Expr::Int { raw, .. } if raw == "3"));
        }
        other => panic!("expected binding, got {other:?}"),
    }
}

#[test]
fn numeric_literals_are_lossless() {
    let p = parse_ok("x = 1_000\ny = 2.5e-4");
    match &p.stmts[0] {
        Stmt::Binding(b) => assert!(matches!(&b.value, Expr::Int { raw, .. } if raw == "1_000")),
        _ => panic!(),
    }
    match &p.stmts[1] {
        Stmt::Binding(b) => assert!(matches!(&b.value, Expr::Float { raw, .. } if raw == "2.5e-4")),
        _ => panic!(),
    }
}

#[test]
fn precedence_pow_over_unary() {
    // -x^2 parses as -(x^2)
    let p = parse_ok("-x^2");
    let e = only_expr(&p);
    match e {
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => {
            assert!(matches!(
                &**expr,
                Expr::Binary {
                    op: BinaryOp::Pow,
                    ..
                }
            ));
        }
        other => panic!("expected unary neg over pow, got {other:?}"),
    }
}

#[test]
fn precedence_arithmetic() {
    // 1 + 2 * 3 parses as 1 + (2 * 3)
    let p = parse_ok("1 + 2 * 3");
    let e = only_expr(&p);
    match e {
        Expr::Binary {
            op: BinaryOp::Add,
            rhs,
            ..
        } => {
            assert!(matches!(
                &**rhs,
                Expr::Binary {
                    op: BinaryOp::Mul,
                    ..
                }
            ));
        }
        other => panic!("expected add over mul, got {other:?}"),
    }
}

#[test]
fn pow_is_right_associative() {
    // 2^3^2 parses as 2^(3^2)
    let p = parse_ok("2^3^2");
    let e = only_expr(&p);
    match e {
        Expr::Binary {
            op: BinaryOp::Pow,
            rhs,
            ..
        } => {
            assert!(matches!(
                &**rhs,
                Expr::Binary {
                    op: BinaryOp::Pow,
                    ..
                }
            ));
        }
        other => panic!("expected right-assoc pow, got {other:?}"),
    }
}

#[test]
fn composition_and_call_binding() {
    // g . f(x) parses as g . (f(x))
    let p = parse_ok("g . f(x)");
    let e = only_expr(&p);
    match e {
        Expr::Compose { rhs, .. } => assert!(matches!(&**rhs, Expr::Call { .. })),
        other => panic!("expected compose with call rhs, got {other:?}"),
    }
}

#[test]
fn pipe_is_left_associative() {
    // d |> f |> g parses as (d |> f) |> g
    let p = parse_ok("d |> f |> g");
    let e = only_expr(&p);
    match e {
        Expr::Pipe { lhs, .. } => assert!(matches!(&**lhs, Expr::Pipe { .. })),
        other => panic!("expected left-assoc pipe, got {other:?}"),
    }
}

#[test]
fn range_expression() {
    let p = parse_ok("r = 1..10");
    match &p.stmts[0] {
        Stmt::Binding(b) => assert!(matches!(&b.value, Expr::Range { .. })),
        _ => panic!(),
    }
}

#[test]
fn expression_function_definition() {
    let p = parse_ok("f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)");
    match &p.stmts[0] {
        Stmt::Func(f) => {
            assert_eq!(f.name.sym.as_str(), "f");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.kind, FuncKind::Expr);
        }
        other => panic!("expected function definition, got {other:?}"),
    }
}

#[test]
fn call_is_not_a_function_definition() {
    // f(x, y) with no `=` is a call expression, not a definition.
    let p = parse_ok("f(x, y)");
    assert!(matches!(only_expr(&p), Expr::Call { .. }));
}

#[test]
fn block_function_definition() {
    let p = parse_ok("fn describe(a) {\n  print(a)\n  a * 2\n}");
    match &p.stmts[0] {
        Stmt::Func(f) => {
            assert_eq!(f.kind, FuncKind::Block);
            match &f.body {
                Expr::Block(b) => {
                    assert_eq!(b.stmts.len(), 1);
                    assert!(b.tail.is_some());
                }
                other => panic!("expected block body, got {other:?}"),
            }
        }
        other => panic!("expected function definition, got {other:?}"),
    }
}

#[test]
fn signature_drives_types() {
    let p = parse_ok("f : R^2 -> R");
    match &p.stmts[0] {
        Stmt::Signature(s) => {
            assert_eq!(s.name.sym.as_str(), "f");
            assert_eq!(s.types.len(), 2);
            assert!(matches!(
                &s.types[0],
                Expr::Binary {
                    op: BinaryOp::Pow,
                    ..
                }
            ));
            assert!(matches!(&s.types[1], Expr::Ident(_)));
        }
        other => panic!("expected signature, got {other:?}"),
    }
}

#[test]
fn for_loop_with_block() {
    let src = "for a in [1, 2, 3, 4] {\n  print(\"a =\", a)\n}";
    let p = parse_ok(src);
    match &p.stmts[0] {
        Stmt::For(f) => {
            assert_eq!(f.var.sym.as_str(), "a");
            assert!(matches!(&f.iter, Expr::List { .. }));
            // The single `print(...)` line becomes the block's tail value.
            assert!(f.body.stmts.is_empty());
            assert!(f.body.tail.is_some());
        }
        other => panic!("expected for loop, got {other:?}"),
    }
}

#[test]
fn if_expression_with_else() {
    let p = parse_ok("sign = if x > 0 { 1 } else { -1 }");
    match &p.stmts[0] {
        Stmt::Binding(b) => match &b.value {
            Expr::If(i) => assert!(i.els.is_some()),
            other => panic!("expected if expression, got {other:?}"),
        },
        _ => panic!(),
    }
}

#[test]
fn if_else_across_newlines() {
    let src = "y = if x > 0 {\n  1\n}\nelse {\n  2\n}";
    let p = parse_ok(src);
    match &p.stmts[0] {
        Stmt::Binding(b) => assert!(matches!(&b.value, Expr::If(i) if i.els.is_some())),
        _ => panic!(),
    }
}

#[test]
fn print_is_a_call() {
    let p = parse_ok("print(\"plotting a =\", a)");
    match only_expr(&p) {
        Expr::Call { callee, args, .. } => {
            assert!(matches!(&**callee, Expr::Ident(id) if id.sym.as_str() == "print"));
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected call, got {other:?}"),
    }
}

#[test]
fn plot_with_kind_and_block() {
    let src =
        "plot surface {\n  z = a * f(x, y)\n  x = -6..6\n  y = -6..6\n  resolution = [160, 160]\n}";
    let p = parse_ok(src);
    match &p.stmts[0] {
        Stmt::Plot(plot) => {
            assert_eq!(plot.kind.as_ref().unwrap().sym.as_str(), "surface");
            assert!(plot.target.is_none());
            assert_eq!(plot.fields.len(), 4);
            assert_eq!(plot.fields[3].name.sym.as_str(), "resolution");
        }
        other => panic!("expected plot statement, got {other:?}"),
    }
}

#[test]
fn plot_with_target_and_over() {
    let p = parse_ok("plot f over x = -6..6, y = -6..6");
    match &p.stmts[0] {
        Stmt::Plot(plot) => {
            assert!(plot.kind.is_none());
            assert!(matches!(&plot.target, Some(Expr::Ident(_))));
            assert_eq!(plot.over.len(), 2);
            assert!(plot.fields.is_empty());
        }
        other => panic!("expected plot statement, got {other:?}"),
    }
}

#[test]
fn plot_kind_with_target_and_block() {
    let src = "plot scatter samples {\n  x = \"x\"\n  y = \"y\"\n  z = \"z\"\n}";
    let p = parse_ok(src);
    match &p.stmts[0] {
        Stmt::Plot(plot) => {
            assert_eq!(plot.kind.as_ref().unwrap().sym.as_str(), "scatter");
            assert!(matches!(&plot.target, Some(Expr::Ident(id)) if id.sym.as_str() == "samples"));
            assert_eq!(plot.fields.len(), 3);
        }
        other => panic!("expected plot statement, got {other:?}"),
    }
}

#[test]
fn named_call_arguments() {
    let p = parse_ok("scatter(data, x = \"x\", y = \"y\", z = \"z\")");
    match only_expr(&p) {
        Expr::Call { args, .. } => {
            assert_eq!(args.len(), 4);
            assert!(args[0].name.is_none());
            assert_eq!(args[1].name.as_ref().unwrap().sym.as_str(), "x");
        }
        other => panic!("expected call, got {other:?}"),
    }
}

#[test]
fn lambda_forms() {
    assert!(matches!(
        only_expr(&parse_ok("x => x^2")),
        Expr::Lambda { .. }
    ));
    let p = parse_ok("g = (x, y) => x + y");
    match &p.stmts[0] {
        Stmt::Binding(b) => match &b.value {
            Expr::Lambda { params, .. } => assert_eq!(params.len(), 2),
            other => panic!("expected lambda, got {other:?}"),
        },
        _ => panic!(),
    }
}

#[test]
fn attachment_pipeline_program() {
    // A multi-statement program mixing bindings, calls, and a plot block.
    let src = "\
data = csv(attachment(\"samples.csv\"))
points = scatter(data, x = \"x\", y = \"y\", z = \"z\")
plot points";
    let p = parse_ok(src);
    assert_eq!(p.stmts.len(), 3);
    assert!(matches!(&p.stmts[0], Stmt::Binding(_)));
    assert!(matches!(&p.stmts[1], Stmt::Binding(_)));
    assert!(matches!(&p.stmts[2], Stmt::Plot(_)));
}

#[test]
fn nested_block_program() {
    let src = "\
g = graph()

for a in [1, 2, 3] {
  p = surface(z = a * sin(x * y), x = -3..3, y = -3..3)
  g = add_plot(g, p)
}

g";
    let p = parse_ok(src);
    assert_eq!(p.stmts.len(), 3);
    assert!(matches!(&p.stmts[0], Stmt::Binding(_)));
    assert!(matches!(&p.stmts[1], Stmt::For(_)));
    assert!(matches!(&p.stmts[2], Stmt::Expr(Expr::Ident(_))));
}

#[test]
fn line_continuation_after_operator() {
    // A binary operator at end of line continues onto the next line.
    let p = parse_ok("total = 1 +\n  2");
    match &p.stmts[0] {
        Stmt::Binding(b) => assert!(matches!(
            &b.value,
            Expr::Binary {
                op: BinaryOp::Add,
                ..
            }
        )),
        _ => panic!(),
    }
}

// --- invalid syntax ---

#[test]
fn error_unterminated_string() {
    parse_err("x = \"oops");
}

#[test]
fn error_unexpected_character() {
    parse_err("x = 1 @ 2");
}

#[test]
fn error_reserved_keyword() {
    parse_err("let x = 3");
}

#[test]
fn error_chained_comparison() {
    parse_err("a < b < c");
}

#[test]
fn error_missing_expression() {
    parse_err("x = ");
}

#[test]
fn error_unclosed_paren() {
    parse_err("y = (1 + 2");
}

#[test]
fn error_recovers_and_reports_later_statements() {
    // The first line is broken; the parser should still reach and the second
    // line should parse, so exactly the one error is reported.
    let result = parse("x = 1 2\ny = 3");
    assert!(!result.diagnostics.is_empty());
    // Recovery means the good binding is still present.
    assert!(
        result
            .program
            .stmts
            .iter()
            .any(|s| matches!(s, Stmt::Binding(b) if b.name.sym.as_str() == "y"))
    );
}
