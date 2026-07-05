use eframe::egui;
use poincare_notebook_lib::{
    DiagnosticSeverity, ExecutionState, GraphBlockId, GraphOutput, GraphOwnership, NotebookBlock,
    NotebookBlockKind, NotebookOutputKind, TextFormat,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellAction {
    InsertMarkdownAbove,
    InsertMarkdownBelow,
    InsertCodeAbove,
    InsertCodeBelow,
    Delete,
    Duplicate,
    MoveUp,
    MoveDown,
    ToggleCollapsed,
    ToggleOutputs,
    ConvertToMarkdown,
    ConvertToCode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphOutputAction {
    Activate { graph_id: GraphBlockId },
    Deactivate { graph_id: GraphBlockId },
    ResetView { graph_id: GraphBlockId },
    OpenInPoincare { graph_id: GraphBlockId },
    RefreshPreview { graph_id: GraphBlockId },
}

#[derive(Clone, Debug, Default)]
pub struct CellUiResponse {
    pub action: Option<CellAction>,
    pub graph_action: Option<GraphOutputAction>,
    pub edited: bool,
    pub clicked: bool,
}

pub fn show_cell(
    ui: &mut egui::Ui,
    block: &mut NotebookBlock,
    selected: bool,
    active_graph: Option<&GraphBlockId>,
    graphs: &mut crate::graph_viewport::GraphRenderManager,
) -> CellUiResponse {
    let mut response = CellUiResponse::default();
    let frame = egui::Frame::default()
        .fill(if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.18)
        } else {
            ui.visuals().panel_fill
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke.color
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10));

    let inner = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(cell_kind_label(block));
            ui.separator();
            ui.label(block.id.0.as_str());
            ui.separator();
            ui.label(cell_status_label(block));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cell_menu(ui, &mut response);
                if ui
                    .small_button(if block.state.collapsed {
                        "Expand"
                    } else {
                        "Collapse"
                    })
                    .clicked()
                {
                    response.action = Some(CellAction::ToggleCollapsed);
                }
            });
        });

        if !block.state.collapsed {
            ui.add_space(6.0);
            match &mut block.kind {
                NotebookBlockKind::Text(cell) => {
                    let editor = egui::TextEdit::multiline(&mut cell.source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .hint_text("Markdown text");
                    if ui.add(editor).changed() {
                        response.edited = true;
                    }
                }
                NotebookBlockKind::Executable(cell) => {
                    let editor = egui::TextEdit::multiline(&mut cell.source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .hint_text("Poincare code");
                    if ui.add(editor).changed() {
                        response.edited = true;
                    }
                }
                NotebookBlockKind::Graph(_) => {
                    ui.label("Graph block");
                }
                NotebookBlockKind::Table(table) => {
                    ui.label(table.title.as_deref().unwrap_or("Table"));
                    egui::Grid::new(format!("table-{}", block.id.0))
                        .striped(true)
                        .show(ui, |ui| {
                            for column in &table.columns {
                                ui.strong(column);
                            }
                            ui.end_row();
                            for row in table.rows.iter().take(12) {
                                for value in row {
                                    ui.label(value);
                                }
                                ui.end_row();
                            }
                        });
                }
                NotebookBlockKind::Diagnostic(diagnostics) => {
                    for diagnostic in &diagnostics.diagnostics {
                        ui.colored_label(
                            diagnostic_color(ui, diagnostic.severity),
                            diagnostic.message.as_str(),
                        );
                    }
                }
            }

            if !block.outputs.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Outputs");
                    if ui
                        .small_button(if block.state.outputs_collapsed {
                            "Show"
                        } else {
                            "Hide"
                        })
                        .clicked()
                    {
                        response.action = Some(CellAction::ToggleOutputs);
                    }
                });
                if !block.state.outputs_collapsed {
                    show_outputs(ui, block, active_graph, graphs, &mut response);
                }
            }
        }
    });
    if inner.response.clicked() {
        response.clicked = true;
    }

    response
}

fn cell_menu(ui: &mut egui::Ui, response: &mut CellUiResponse) {
    ui.menu_button("Cell", |ui| {
        if ui.button("Insert Markdown Above").clicked() {
            response.action = Some(CellAction::InsertMarkdownAbove);
            ui.close();
        }
        if ui.button("Insert Code Above").clicked() {
            response.action = Some(CellAction::InsertCodeAbove);
            ui.close();
        }
        if ui.button("Insert Markdown Below").clicked() {
            response.action = Some(CellAction::InsertMarkdownBelow);
            ui.close();
        }
        if ui.button("Insert Code Below").clicked() {
            response.action = Some(CellAction::InsertCodeBelow);
            ui.close();
        }
        ui.separator();
        if ui.button("Move Up").clicked() {
            response.action = Some(CellAction::MoveUp);
            ui.close();
        }
        if ui.button("Move Down").clicked() {
            response.action = Some(CellAction::MoveDown);
            ui.close();
        }
        if ui.button("Duplicate").clicked() {
            response.action = Some(CellAction::Duplicate);
            ui.close();
        }
        if ui.button("Delete").clicked() {
            response.action = Some(CellAction::Delete);
            ui.close();
        }
        ui.separator();
        if ui.button("Convert to Markdown").clicked() {
            response.action = Some(CellAction::ConvertToMarkdown);
            ui.close();
        }
        if ui.button("Convert to Code").clicked() {
            response.action = Some(CellAction::ConvertToCode);
            ui.close();
        }
    });
}

fn show_outputs(
    ui: &mut egui::Ui,
    block: &NotebookBlock,
    active_graph: Option<&GraphBlockId>,
    graphs: &mut crate::graph_viewport::GraphRenderManager,
    response: &mut CellUiResponse,
) {
    for output in &block.outputs {
        let stale = !output.stale.is_empty();
        egui::Frame::default()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(output_kind_label(&output.kind));
                    if stale {
                        ui.colored_label(ui.visuals().warn_fg_color, "stale");
                    }
                });
                match &output.kind {
                    NotebookOutputKind::Text(text) => {
                        ui.monospace(text.text.as_str());
                    }
                    NotebookOutputKind::Value(value) => {
                        ui.monospace(value.preview.as_str());
                    }
                    NotebookOutputKind::Table(table) => {
                        ui.label(table.title.as_deref().unwrap_or("Table"));
                        ui.label(format!(
                            "{} rows x {} columns",
                            table.rows.len(),
                            table.columns.len()
                        ));
                    }
                    NotebookOutputKind::Graph(graph) => {
                        show_graph_output(ui, graph, active_graph, graphs, response);
                    }
                    NotebookOutputKind::Image(image) => {
                        ui.label(format!("Image {}", image.path.0));
                    }
                    NotebookOutputKind::Diagnostic(diagnostic) => {
                        ui.colored_label(
                            diagnostic_color(ui, diagnostic.severity),
                            diagnostic.message.as_str(),
                        );
                    }
                    NotebookOutputKind::Analysis(analysis) => {
                        ui.label(analysis.title.as_str());
                    }
                    NotebookOutputKind::Attachment(attachment) => {
                        ui.label(format!("Attachment {}", attachment.id.0));
                    }
                }
            });
        ui.add_space(4.0);
    }
}

fn show_graph_output(
    ui: &mut egui::Ui,
    graph: &GraphOutput,
    active_graph: Option<&GraphBlockId>,
    graphs: &mut crate::graph_viewport::GraphRenderManager,
    response: &mut CellUiResponse,
) {
    let is_active = active_graph
        .map(|active_graph| *active_graph == graph.graph_id)
        .unwrap_or(false);

    ui.horizontal(|ui| {
        ui.monospace(graph.graph_id.0.as_str());
        ui.label(graph_ownership_label(&graph.ownership));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Open").clicked() {
                response.graph_action = Some(GraphOutputAction::OpenInPoincare {
                    graph_id: graph.graph_id.clone(),
                });
            }
            if ui.small_button("Refresh").clicked() {
                response.graph_action = Some(GraphOutputAction::RefreshPreview {
                    graph_id: graph.graph_id.clone(),
                });
            }
            if ui.small_button("Reset").clicked() {
                response.graph_action = Some(GraphOutputAction::ResetView {
                    graph_id: graph.graph_id.clone(),
                });
            }
            if is_active {
                if ui.small_button("Done").clicked() {
                    response.graph_action = Some(GraphOutputAction::Deactivate {
                        graph_id: graph.graph_id.clone(),
                    });
                }
            } else if ui.small_button("Activate").clicked() {
                response.graph_action = Some(GraphOutputAction::Activate {
                    graph_id: graph.graph_id.clone(),
                });
            }
        });
    });

    let width = ui.available_width().clamp(260.0, 720.0);
    // Keep the box at the render aspect so the GPU image is never stretched.
    let height = width / crate::graph_viewport::PREVIEW_ASPECT;
    let graph_id = graph.graph_id.0.as_str();

    // The active graph orbits on drag/scroll; others just register a click to
    // activate. Rendering is identical either way — a GPU texture drawn as an
    // image, which behaves correctly inside the scroll area.
    let sense = if is_active {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::click()
    };
    let (rect, viewport_response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
    graphs.interact(graph_id, ui, &viewport_response, rect, is_active);

    if let Some(texture_id) = graphs.image(graph_id) {
        egui::Image::new(egui::load::SizedTexture::new(texture_id, rect.size()))
            .paint_at(ui, rect);
        if is_active {
            ui.painter_at(rect).rect_stroke(
                rect,
                egui::CornerRadius::same(6),
                egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }
    } else {
        // GPU image not ready yet (first frame) or the scene failed to build:
        // fall back to the lightweight CPU-sampled thumbnail.
        paint_graph_preview(ui, rect, graph, is_active);
    }

    if is_active && viewport_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if viewport_response.clicked() && !is_active {
        response.graph_action = Some(GraphOutputAction::Activate {
            graph_id: graph.graph_id.clone(),
        });
    }
}

fn paint_graph_preview(ui: &egui::Ui, rect: egui::Rect, graph: &GraphOutput, is_active: bool) {
    // Render the computed graph directly from its spec. The interactive 3D
    // viewport (activation) is a separate, future path; this is a CPU-sampled
    // static thumbnail of the plots the cell produced.
    crate::graph_preview::paint_graph_spec(ui, rect, &graph.graph, is_active);

    let painter = ui.painter_at(rect);
    if is_active {
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}

fn graph_ownership_label(ownership: &GraphOwnership) -> String {
    match ownership {
        GraphOwnership::Snapshot => "snapshot".to_string(),
        GraphOwnership::Linked { source } => format!("linked {}", source.path),
        GraphOwnership::Computed { source_cell } => format!("computed from {}", source_cell.0),
    }
}

fn cell_kind_label(block: &NotebookBlock) -> &'static str {
    match &block.kind {
        NotebookBlockKind::Text(cell) => match cell.format {
            TextFormat::Markdown => "Markdown",
            TextFormat::PlainText => "Text",
        },
        NotebookBlockKind::Executable(_) => "Input",
        NotebookBlockKind::Graph(_) => "Graph",
        NotebookBlockKind::Table(_) => "Table",
        NotebookBlockKind::Diagnostic(_) => "Diagnostic",
    }
}

fn cell_status_label(block: &NotebookBlock) -> String {
    match &block.kind {
        NotebookBlockKind::Executable(cell) => match &cell.execution {
            ExecutionState::Idle => "idle".to_string(),
            ExecutionState::Queued => "queued".to_string(),
            ExecutionState::Running => "running".to_string(),
            ExecutionState::Complete { run_count } => format!("run {run_count}"),
            ExecutionState::Failed { run_count } => format!("failed {run_count}"),
            ExecutionState::Stale { reasons } => format!("stale {}", reasons.len()),
        },
        _ => "document".to_string(),
    }
}

fn output_kind_label(kind: &NotebookOutputKind) -> &'static str {
    match kind {
        NotebookOutputKind::Text(_) => "Text",
        NotebookOutputKind::Value(_) => "Value",
        NotebookOutputKind::Table(_) => "Table",
        NotebookOutputKind::Graph(_) => "Graph",
        NotebookOutputKind::Image(_) => "Image",
        NotebookOutputKind::Diagnostic(_) => "Diagnostic",
        NotebookOutputKind::Analysis(_) => "Analysis",
        NotebookOutputKind::Attachment(_) => "Attachment",
    }
}

fn diagnostic_color(ui: &egui::Ui, severity: DiagnosticSeverity) -> egui::Color32 {
    match severity {
        DiagnosticSeverity::Info => ui.visuals().text_color(),
        DiagnosticSeverity::Warning => ui.visuals().warn_fg_color,
        DiagnosticSeverity::Error => ui.visuals().error_fg_color,
    }
}
