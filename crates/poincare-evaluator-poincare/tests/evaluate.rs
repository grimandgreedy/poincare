//! Notebook-style cell execution through the evaluator API.

use poincare_evaluator::{
    AttachmentValue, EvalAttachmentId, EvalCellId, EvalDocumentId, EvalRequest, EvalResponse,
    EvalStatus, EvalValue, Evaluator, HostError, NumberValue, RuntimeHost, ValueKind,
};
use poincare_evaluator_poincare::PoincareEvaluator;

/// A host with in-memory attachments keyed by display name.
struct TestHost {
    csv: &'static str,
}

impl RuntimeHost for TestHost {
    fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentValue, HostError> {
        if name_or_id == "samples.csv" || name_or_id == "att-1" {
            Ok(AttachmentValue {
                id: EvalAttachmentId::new("att-1"),
                display_name: "samples.csv".to_string(),
                media_type: Some("text/csv".to_string()),
                size_bytes: None,
                hash: None,
            })
        } else {
            Err(HostError::not_found(format!("no attachment `{name_or_id}`")))
        }
    }

    fn attachment_bytes(&self, attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
        if attachment.0 == "att-1" {
            Ok(self.csv.as_bytes().to_vec())
        } else {
            Err(HostError::not_found("bytes not found"))
        }
    }
}

struct NoHost;
impl RuntimeHost for NoHost {
    fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentValue, HostError> {
        Err(HostError::not_found(name_or_id.to_string()))
    }
    fn attachment_bytes(&self, _attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
        Err(HostError::not_found("no attachments"))
    }
}

fn run(evaluator: &mut PoincareEvaluator, host: &dyn RuntimeHost, source: &str) -> EvalResponse {
    evaluator.evaluate_cell(
        EvalRequest::new(
            EvalDocumentId::new("nb"),
            EvalCellId::new("cell"),
            source,
        ),
        host,
    )
}

fn run_cell(
    evaluator: &mut PoincareEvaluator,
    host: &dyn RuntimeHost,
    cell: &str,
    source: &str,
) -> EvalResponse {
    evaluator.evaluate_cell(
        EvalRequest::new(EvalDocumentId::new("nb"), EvalCellId::new(cell), source),
        host,
    )
}

#[test]
fn metadata_declares_poincare_language() {
    let evaluator = PoincareEvaluator::new();
    let metadata = evaluator.metadata();
    assert_eq!(metadata.language_id, "poincare");
    assert!(metadata.features.supports_shared_state);
    assert!(metadata.features.supports_graph_outputs);
}

#[test]
fn define_in_one_cell_use_in_the_next() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;

    let first = run_cell(&mut evaluator, &host, "c1", "f(x) = x * x");
    assert_eq!(first.status, EvalStatus::Complete);

    let second = run_cell(&mut evaluator, &host, "c2", "f(6)");
    assert_eq!(second.status, EvalStatus::Complete);
    // The final expression value is the last output.
    let last = second.outputs.last().expect("an output");
    assert!(matches!(
        last.value,
        EvalValue::Number(NumberValue::Int(36))
    ));
}

#[test]
fn print_becomes_a_text_output() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    let response = run(&mut evaluator, &host, "print(\"hello\", 1 + 1)");
    assert_eq!(response.status, EvalStatus::Complete);
    let text = response
        .outputs
        .iter()
        .find_map(|o| match &o.value {
            EvalValue::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .expect("a text output");
    assert_eq!(text, "hello 2");
}

#[test]
fn emit_produces_a_graph_output() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    let response = run(
        &mut evaluator,
        &host,
        "g = graph()\ng = add_plot(g, surface(z = 1, x = -3..3, y = -3..3))\nemit(g)",
    );
    assert_eq!(response.status, EvalStatus::Complete, "{:?}", response.diagnostics);
    assert!(
        response
            .outputs
            .iter()
            .any(|o| matches!(o.value, EvalValue::Graph(_))),
        "expected a graph output"
    );
}

#[test]
fn surface_formula_reaches_the_graph_spec() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    let response = run(
        &mut evaluator,
        &host,
        "amp = 3\ng = add_plot(graph(), surface(z = amp * (x^2 + y^2), x = -3..3, y = -3..3))\nemit(g)",
    );
    assert_eq!(response.status, EvalStatus::Complete, "{:?}", response.diagnostics);
    let spec = response
        .outputs
        .iter()
        .find_map(|o| match &o.value {
            EvalValue::Graph(spec) => Some(spec),
            _ => None,
        })
        .expect("expected a graph output");
    let plot = spec.plots.first().expect("one plot");
    match &plot.definition {
        poincare_lib::PlotDefinition::ExprCartesian {
            expression,
            parameters,
        } => {
            assert_eq!(expression, "(amp * ((x ^ 2) + (y ^ 2)))");
            assert_eq!(parameters, &vec![("amp".to_string(), 3.0)]);
        }
        other => panic!("expected an ExprCartesian plot, got {other:?}"),
    }
    assert_eq!(*plot.domain.x.start(), -3.0);
    assert_eq!(*plot.domain.x.end(), 3.0);
}

#[test]
fn csv_attachment_becomes_a_table() {
    let mut evaluator = PoincareEvaluator::new();
    let host = TestHost {
        csv: "x,y\n1,2\n3,4",
    };
    let response = run(
        &mut evaluator,
        &host,
        "data = csv(attachment(\"samples.csv\"))\ndata",
    );
    assert_eq!(response.status, EvalStatus::Complete, "{:?}", response.diagnostics);
    let table = response
        .outputs
        .iter()
        .find_map(|o| match &o.value {
            EvalValue::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a table output");
    assert_eq!(table.columns, vec!["x", "y"]);
    assert_eq!(table.rows.len(), 2);

    // The session should show `data` as a table variable.
    let session = response.session.expect("session");
    let var = session
        .variables
        .iter()
        .find(|v| v.name == "data")
        .expect("data variable");
    assert_eq!(var.kind, ValueKind::Table);
    assert_eq!(var.source_cell.as_ref().unwrap().0, "cell");
}

#[test]
fn undefined_name_is_reported_as_a_diagnostic() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    let response = run(&mut evaluator, &host, "y = missing + 1");
    assert_eq!(response.status, EvalStatus::Failed);
    assert!(
        response
            .diagnostics
            .iter()
            .any(|d| d.message.contains("undefined name `missing`"))
    );
}

#[test]
fn runtime_error_fails_but_keeps_prior_output() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    let response = run(&mut evaluator, &host, "print(\"before\")\n1 + true");
    assert_eq!(response.status, EvalStatus::Failed);
    assert!(response.outputs.iter().any(|o| matches!(&o.value, EvalValue::Text(t) if t.text == "before")));
    assert!(response.diagnostics.iter().any(|d| d.message.contains("expected a number")));
}

#[test]
fn restart_clears_the_session() {
    let mut evaluator = PoincareEvaluator::new();
    let host = NoHost;
    run(&mut evaluator, &host, "a = 5");
    assert!(!evaluator.session_snapshot().variables.is_empty());

    evaluator.restart();
    assert!(evaluator.session_snapshot().variables.is_empty());

    // `a` is gone after restart.
    let response = run(&mut evaluator, &host, "a + 1");
    assert_eq!(response.status, EvalStatus::Failed);
}
