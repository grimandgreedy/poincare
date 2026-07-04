use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::{
    AnalysisReportSnapshot, AnalysisSnapshot, AttachmentId, AttachmentRef, BundlePath,
    DiagnosticSeverity, GraphBlockId, GraphOutput, GraphOwnership, ImageOutput, NotebookCellId,
    NotebookDiagnostic, NotebookOutput, NotebookOutputId, NotebookOutputKind, OutputProvenance,
    SourcePosition, SourceSpan, TableOutput, TextOutput, TextStream, ValueKind, ValueOutput,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOutputLimits {
    pub max_outputs_per_cell: usize,
    pub max_text_chars: usize,
    pub max_table_rows: usize,
}

impl Default for RuntimeOutputLimits {
    fn default() -> Self {
        Self {
            max_outputs_per_cell: 64,
            max_text_chars: 64 * 1024,
            max_table_rows: 1_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeOutputCompletion {
    Complete,
    Truncated { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeOutputBatch {
    pub outputs: Vec<NotebookOutput>,
    pub completion: RuntimeOutputCompletion,
}

pub fn notebook_outputs_from_eval_response(
    cell_id: &NotebookCellId,
    response: &evaluator::EvalResponse,
    limits: &RuntimeOutputLimits,
) -> RuntimeOutputBatch {
    let mut mapper = OutputMapper::new(cell_id.clone(), response, limits);

    for output in &response.outputs {
        mapper.push_eval_output(output);
        if mapper.limit_reached {
            break;
        }
    }

    for diagnostic in &response.diagnostics {
        mapper.push_diagnostic(diagnostic);
        if mapper.limit_reached {
            break;
        }
    }

    mapper.finish()
}

struct OutputMapper<'a> {
    cell_id: NotebookCellId,
    response: &'a evaluator::EvalResponse,
    limits: &'a RuntimeOutputLimits,
    outputs: Vec<NotebookOutput>,
    next_index: usize,
    limit_reached: bool,
}

impl<'a> OutputMapper<'a> {
    fn new(
        cell_id: NotebookCellId,
        response: &'a evaluator::EvalResponse,
        limits: &'a RuntimeOutputLimits,
    ) -> Self {
        Self {
            cell_id,
            response,
            limits,
            outputs: Vec::new(),
            next_index: 0,
            limit_reached: false,
        }
    }

    fn push_eval_output(&mut self, output: &evaluator::EvalOutput) {
        if output.display == evaluator::EvalDisplayHint::Hidden {
            return;
        }

        if let Some(kind) = self.output_kind_from_value(&output.value, output.display) {
            let provenance = self.provenance_from_eval(&output.provenance);
            self.push(kind, provenance);
        }
    }

    fn push_diagnostic(&mut self, diagnostic: &evaluator::EvalDiagnostic) {
        let kind = NotebookOutputKind::Diagnostic(notebook_diagnostic(diagnostic));
        self.push(kind, OutputProvenance::source_cell(self.cell_id.clone()));
    }

    fn push(&mut self, kind: NotebookOutputKind, provenance: OutputProvenance) {
        if self.outputs.len() >= self.limits.max_outputs_per_cell {
            self.limit_reached = true;
            return;
        }

        let output_id = NotebookOutputId::new(format!(
            "{}-run-{}-output-{}",
            self.cell_id.0,
            self.response
                .session
                .as_ref()
                .map(|session| session.run_count)
                .unwrap_or(0),
            self.next_index
        ));
        self.next_index += 1;
        self.outputs.push(NotebookOutput {
            id: output_id,
            kind,
            provenance,
            stale: Vec::new(),
        });
    }

    fn finish(mut self) -> RuntimeOutputBatch {
        let completion = if self.limit_reached {
            RuntimeOutputCompletion::Truncated {
                reason: format!("output count exceeded {}", self.limits.max_outputs_per_cell),
            }
        } else {
            RuntimeOutputCompletion::Complete
        };

        if let RuntimeOutputCompletion::Truncated { ref reason } = completion {
            if self.outputs.len() < self.limits.max_outputs_per_cell {
                self.push(
                    NotebookOutputKind::Diagnostic(NotebookDiagnostic {
                        severity: DiagnosticSeverity::Warning,
                        message: reason.clone(),
                        span: None,
                        code: Some("OUTPUT_LIMIT".to_string()),
                    }),
                    OutputProvenance::source_cell(self.cell_id.clone()),
                );
            }
        }

        RuntimeOutputBatch {
            outputs: self.outputs,
            completion,
        }
    }

    fn output_kind_from_value(
        &self,
        value: &evaluator::EvalValue,
        display: evaluator::EvalDisplayHint,
    ) -> Option<NotebookOutputKind> {
        match value {
            evaluator::EvalValue::Unit => None,
            evaluator::EvalValue::String(value) if display == evaluator::EvalDisplayHint::Text => {
                Some(NotebookOutputKind::Text(TextOutput {
                    stream: TextStream::Display,
                    text: truncate_text(value, self.limits.max_text_chars),
                }))
            }
            evaluator::EvalValue::Text(value) => Some(NotebookOutputKind::Text(TextOutput {
                stream: match value.stream {
                    evaluator::EvalTextStream::Stdout => TextStream::Stdout,
                    evaluator::EvalTextStream::Stderr => TextStream::Stderr,
                    evaluator::EvalTextStream::Display => TextStream::Display,
                },
                text: truncate_text(&value.text, self.limits.max_text_chars),
            })),
            evaluator::EvalValue::Table(value) => Some(NotebookOutputKind::Table(TableOutput {
                title: value.title.clone(),
                columns: value.columns.clone(),
                rows: value
                    .rows
                    .iter()
                    .take(self.limits.max_table_rows)
                    .cloned()
                    .collect(),
                truncated: value.truncated || value.rows.len() > self.limits.max_table_rows,
            })),
            evaluator::EvalValue::Graph(_) => Some(NotebookOutputKind::Graph(GraphOutput {
                graph_id: GraphBlockId::new(format!(
                    "{}-run-{}-graph-{}",
                    self.cell_id.0,
                    self.response
                        .session
                        .as_ref()
                        .map(|session| session.run_count)
                        .unwrap_or(0),
                    self.next_index
                )),
                ownership: GraphOwnership::Computed {
                    source_cell: self.cell_id.clone(),
                },
                preview: None,
            })),
            evaluator::EvalValue::Analysis(value) => {
                Some(NotebookOutputKind::Analysis(AnalysisSnapshot {
                    title: value.title.clone(),
                    reports: value
                        .reports
                        .iter()
                        .map(|report| AnalysisReportSnapshot {
                            title: report.title.clone(),
                            values: report.values.clone(),
                        })
                        .collect(),
                    tables: value
                        .tables
                        .iter()
                        .map(|table| TableOutput {
                            title: table.title.clone(),
                            columns: table.columns.clone(),
                            rows: table
                                .rows
                                .iter()
                                .take(self.limits.max_table_rows)
                                .cloned()
                                .collect(),
                            truncated: table.truncated
                                || table.rows.len() > self.limits.max_table_rows,
                        })
                        .collect(),
                    plots: value.plots.clone(),
                    diagnostics: value.diagnostics.iter().map(notebook_diagnostic).collect(),
                }))
            }
            evaluator::EvalValue::Image(value) => Some(NotebookOutputKind::Image(ImageOutput {
                path: BundlePath::new(value.path.clone()),
                mime_type: value.mime_type.clone(),
                alt_text: value.alt_text.clone(),
            })),
            evaluator::EvalValue::Diagnostic(value) => {
                Some(NotebookOutputKind::Diagnostic(notebook_diagnostic(value)))
            }
            evaluator::EvalValue::Attachment(value) => {
                Some(NotebookOutputKind::Attachment(AttachmentRef {
                    id: AttachmentId(value.id.0.clone()),
                    path: BundlePath::new(format!("attachments/{}", value.id.0)),
                    media_type: value.media_type.clone(),
                }))
            }
            _ => Some(NotebookOutputKind::Value(ValueOutput {
                value_kind: value_kind(value),
                preview: value_preview(value, display),
            })),
        }
    }

    fn provenance_from_eval(
        &self,
        provenance: &evaluator::EvalOutputProvenance,
    ) -> OutputProvenance {
        let source_cell = provenance
            .source_cell
            .as_ref()
            .map(|cell_id| NotebookCellId(cell_id.0.clone()))
            .or_else(|| Some(self.cell_id.clone()));
        OutputProvenance {
            source_cell,
            produced_at: provenance.produced_at.clone(),
            run_count: provenance.run_count.or_else(|| {
                self.response
                    .session
                    .as_ref()
                    .map(|session| session.run_count)
            }),
            input_hash: provenance.input_hash.clone(),
            graph_dependencies: Vec::new(),
            attachment_dependencies: provenance
                .attachment_dependencies
                .iter()
                .map(|attachment_id| AttachmentId(attachment_id.0.clone()))
                .collect(),
            notes: provenance.notes.clone(),
        }
    }
}

pub fn value_kind(value: &evaluator::EvalValue) -> ValueKind {
    match value {
        evaluator::EvalValue::Unit => ValueKind::Unit,
        evaluator::EvalValue::Bool(_) => ValueKind::Bool,
        evaluator::EvalValue::Number(_) => ValueKind::Number,
        evaluator::EvalValue::String(_) => ValueKind::String,
        evaluator::EvalValue::Text(_) => ValueKind::String,
        evaluator::EvalValue::List(_) => ValueKind::List,
        evaluator::EvalValue::Function(_) => ValueKind::Function,
        evaluator::EvalValue::Expr(_) => ValueKind::Expression,
        evaluator::EvalValue::Attachment(_) => ValueKind::Attachment,
        evaluator::EvalValue::Bytes(_) => ValueKind::Bytes,
        evaluator::EvalValue::Table(_) => ValueKind::Table,
        evaluator::EvalValue::Array(_) => ValueKind::Array,
        evaluator::EvalValue::Plot(_) => ValueKind::Plot,
        evaluator::EvalValue::Graph(_) => ValueKind::Graph,
        evaluator::EvalValue::Analysis(_) => ValueKind::Analysis,
        evaluator::EvalValue::Image(_) => ValueKind::Image,
        evaluator::EvalValue::Diagnostic(_) => ValueKind::Diagnostic,
    }
}

pub fn value_preview(value: &evaluator::EvalValue, _display: evaluator::EvalDisplayHint) -> String {
    match value {
        evaluator::EvalValue::Unit => "()".to_string(),
        evaluator::EvalValue::Bool(value) => value.to_string(),
        evaluator::EvalValue::Number(value) => match value {
            evaluator::NumberValue::Int(value) => value.to_string(),
            evaluator::NumberValue::Float(value) => value.to_string(),
            evaluator::NumberValue::Rational { numer, denom } => format!("{numer}/{denom}"),
        },
        evaluator::EvalValue::String(value) => value.clone(),
        evaluator::EvalValue::Text(value) => value.text.clone(),
        evaluator::EvalValue::List(values) => format!("list[{}]", values.len()),
        evaluator::EvalValue::Function(value) => value
            .name
            .clone()
            .unwrap_or_else(|| format!("function({})", value.parameters.join(", "))),
        evaluator::EvalValue::Expr(value) => format!("{value:?}"),
        evaluator::EvalValue::Attachment(value) => value.display_name.clone(),
        evaluator::EvalValue::Bytes(value) => format!("{} bytes", value.len),
        evaluator::EvalValue::Table(value) => {
            format!(
                "{} rows x {} columns",
                value.rows.len(),
                value.columns.len()
            )
        }
        evaluator::EvalValue::Array(value) => format!("array {:?}", value.shape),
        evaluator::EvalValue::Plot(_) => "plot".to_string(),
        evaluator::EvalValue::Graph(_) => "graph".to_string(),
        evaluator::EvalValue::Analysis(value) => value.title.clone(),
        evaluator::EvalValue::Image(value) => value
            .alt_text
            .clone()
            .unwrap_or_else(|| format!("image {}", value.mime_type)),
        evaluator::EvalValue::Diagnostic(value) => value.message.clone(),
    }
}

pub fn notebook_diagnostic(value: &evaluator::EvalDiagnostic) -> NotebookDiagnostic {
    NotebookDiagnostic {
        severity: match value.severity {
            evaluator::EvalDiagnosticSeverity::Info => DiagnosticSeverity::Info,
            evaluator::EvalDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            evaluator::EvalDiagnosticSeverity::Error => DiagnosticSeverity::Error,
        },
        message: value.message.clone(),
        span: value.span.map(|span| SourceSpan {
            start: SourcePosition {
                line: span.start.line,
                column: span.start.column,
                offset: span.start.offset,
            },
            end: SourcePosition {
                line: span.end.line,
                column: span.end.column,
                offset: span.end.offset,
            },
        }),
        code: value.code.clone(),
    }
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push_str("\n[output truncated]");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use evaluator::{
        EvalCellId, EvalDiagnostic, EvalDiagnosticSeverity, EvalOutput, EvalResponse, EvalStatus,
        EvalValue, NumberValue, SessionSnapshot,
    };

    #[test]
    fn maps_text_table_value_and_diagnostic_outputs() {
        let cell_id = NotebookCellId::new("cell-1");
        let response = EvalResponse {
            status: EvalStatus::Complete,
            outputs: vec![
                EvalOutput::display(
                    EvalValue::Text(evaluator::TextValue {
                        stream: evaluator::EvalTextStream::Stdout,
                        text: "hello".to_string(),
                    }),
                    EvalCellId::new("cell-1"),
                ),
                EvalOutput::display(
                    EvalValue::Number(NumberValue::Int(3)),
                    EvalCellId::new("cell-1"),
                ),
                EvalOutput::display(
                    EvalValue::Table(evaluator::TableValue {
                        title: Some("data".to_string()),
                        columns: vec!["x".to_string()],
                        rows: vec![vec!["1".to_string()], vec!["2".to_string()]],
                        truncated: false,
                    }),
                    EvalCellId::new("cell-1"),
                ),
            ],
            diagnostics: vec![EvalDiagnostic {
                severity: EvalDiagnosticSeverity::Warning,
                message: "careful".to_string(),
                span: None,
                code: None,
            }],
            context_delta: None,
            session: Some(SessionSnapshot {
                run_count: 7,
                ..SessionSnapshot::default()
            }),
        };

        let batch = notebook_outputs_from_eval_response(
            &cell_id,
            &response,
            &RuntimeOutputLimits::default(),
        );

        assert_eq!(batch.outputs.len(), 4);
        assert!(matches!(batch.outputs[0].kind, NotebookOutputKind::Text(_)));
        assert!(matches!(
            batch.outputs[1].kind,
            NotebookOutputKind::Value(_)
        ));
        assert!(matches!(
            batch.outputs[2].kind,
            NotebookOutputKind::Table(_)
        ));
        assert!(matches!(
            batch.outputs[3].kind,
            NotebookOutputKind::Diagnostic(_)
        ));
        assert_eq!(batch.outputs[0].provenance.run_count, Some(7));
    }

    #[test]
    fn applies_output_limits() {
        let cell_id = NotebookCellId::new("cell-1");
        let response = EvalResponse::complete(vec![
            EvalOutput::display(
                EvalValue::Number(NumberValue::Int(1)),
                EvalCellId::new("cell-1"),
            ),
            EvalOutput::display(
                EvalValue::Number(NumberValue::Int(2)),
                EvalCellId::new("cell-1"),
            ),
        ]);
        let limits = RuntimeOutputLimits {
            max_outputs_per_cell: 1,
            ..RuntimeOutputLimits::default()
        };

        let batch = notebook_outputs_from_eval_response(&cell_id, &response, &limits);

        assert_eq!(batch.outputs.len(), 1);
        assert!(matches!(
            batch.completion,
            RuntimeOutputCompletion::Truncated { .. }
        ));
    }
}
