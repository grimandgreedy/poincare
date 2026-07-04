//! A tiny runner for Poincare source files.
//!
//! Usage: `cargo run -p poincare-lang --example run -- path/to/file.pc`

use std::env;
use std::fs;
use std::process;

use poincare_lang::{Interpreter, SessionScope, Severity, SourceMap, parse, resolve};

fn main() {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: run <file.pc>");
            process::exit(2);
        }
    };
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            process::exit(2);
        }
    };

    let map = SourceMap::new(&source);

    // Parse.
    let parsed = parse(&source);
    for d in &parsed.diagnostics {
        eprintln!("{} {}", label(d.severity), d.render(&map));
    }
    if parsed.diagnostics.iter().any(|d| d.severity == Severity::Error) {
        process::exit(1);
    }

    // Resolve (fresh session).
    let resolved = resolve(&parsed.program, &SessionScope::new());
    for d in &resolved.diagnostics {
        eprintln!("{} {}", label(d.severity), d.render(&map));
    }
    if resolved.has_errors() {
        process::exit(1);
    }

    // Run.
    let mut interp = Interpreter::new();
    let outcome = interp.run(&parsed.program);

    for line in &outcome.output {
        println!("{line}");
    }
    for value in &outcome.emitted {
        println!("emit: {}", value.display());
    }
    if let Some(error) = &outcome.error {
        let loc = map.location(error.span.start);
        eprintln!("error {}:{}: {}", loc.line, loc.column, error.message);
        process::exit(1);
    }
    if let Some(display) = &outcome.value_display {
        println!("=> {display}");
    }
}

fn label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}
