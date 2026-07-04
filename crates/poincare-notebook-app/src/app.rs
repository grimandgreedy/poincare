use std::path::PathBuf;

use eframe::egui;
use poincare_notebook_lib::{
    ExecutionState, NotebookBlock, NotebookBlockKind, NotebookCellId, NotebookDocument, NotebookId,
    TextCell, TextFormat,
};

use crate::cells::{self, CellAction};
use crate::persistence;

pub struct NotebookApp {
    document: NotebookDocument,
    path: Option<PathBuf>,
    selected_cell: Option<NotebookCellId>,
    next_cell_id: u64,
    dirty: bool,
    last_error: Option<String>,
    undo_stack: Vec<NotebookDocument>,
    redo_stack: Vec<NotebookDocument>,
    show_side_panel: bool,
}

impl NotebookApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut document = NotebookDocument::new(NotebookId::new("notebook-1"), "Untitled");
        document.add_block(NotebookBlock::markdown(
            NotebookCellId::new("cell-1"),
            "# Untitled\n",
        ));
        document.add_block(NotebookBlock::executable(
            NotebookCellId::new("cell-2"),
            "poincare",
            "",
        ));

        Self {
            document,
            path: None,
            selected_cell: Some(NotebookCellId::new("cell-1")),
            next_cell_id: 3,
            dirty: false,
            last_error: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            show_side_panel: true,
        }
    }

    fn new_document(&mut self) {
        self.record_undo();
        self.document = NotebookDocument::new(self.next_notebook_id(), "Untitled");
        let first_cell = self.next_cell_id();
        self.document
            .add_block(NotebookBlock::markdown(first_cell.clone(), "# Untitled\n"));
        self.path = None;
        self.selected_cell = Some(first_cell);
        self.dirty = false;
        self.last_error = None;
    }

    fn open_document(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Poincare notebook JSON", &["json"])
            .pick_file()
        else {
            return;
        };

        match persistence::load_document(&path) {
            Ok(document) => {
                self.record_undo();
                self.next_cell_id = next_cell_counter(&document);
                self.selected_cell = document.blocks.first().map(|block| block.id.clone());
                self.document = document;
                self.path = Some(path);
                self.dirty = false;
                self.last_error = None;
            }
            Err(err) => self.last_error = Some(err),
        }
    }

    fn save_document(&mut self) {
        if let Some(path) = self.path.clone() {
            self.save_document_to(path);
        } else {
            self.save_document_as();
        }
    }

    fn save_document_as(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Poincare notebook JSON", &["json"])
            .set_file_name("notebook.json")
            .save_file()
        else {
            return;
        };
        self.save_document_to(path);
    }

    fn save_document_to(&mut self, path: PathBuf) {
        match persistence::save_document(&path, &self.document) {
            Ok(()) => {
                self.path = Some(path);
                self.dirty = false;
                self.last_error = None;
            }
            Err(err) => self.last_error = Some(err),
        }
    }

    fn record_undo(&mut self) {
        self.record_undo_document(self.document.clone());
    }

    fn record_undo_document(&mut self, document: NotebookDocument) {
        self.undo_stack.push(document);
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo_stack.pop() {
            self.redo_stack.push(self.document.clone());
            self.document = previous;
            self.selected_cell = self.document.blocks.first().map(|block| block.id.clone());
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.document.clone());
            self.document = next;
            self.selected_cell = self.document.blocks.first().map(|block| block.id.clone());
            self.dirty = true;
        }
    }

    fn mutate_document(&mut self, mutate: impl FnOnce(&mut Self)) {
        self.record_undo();
        mutate(self);
        self.dirty = true;
    }

    fn insert_block(&mut self, index: usize, block: NotebookBlock) {
        self.selected_cell = Some(block.id.clone());
        self.document.blocks.insert(index, block);
    }

    fn handle_cell_action(&mut self, index: usize, action: CellAction) {
        match action {
            CellAction::InsertMarkdownAbove => self.mutate_document(|this| {
                let block = NotebookBlock::markdown(this.next_cell_id(), "");
                this.insert_block(index, block);
            }),
            CellAction::InsertMarkdownBelow => self.mutate_document(|this| {
                let block = NotebookBlock::markdown(this.next_cell_id(), "");
                this.insert_block(index + 1, block);
            }),
            CellAction::InsertCodeAbove => self.mutate_document(|this| {
                let block = NotebookBlock::executable(this.next_cell_id(), "poincare", "");
                this.insert_block(index, block);
            }),
            CellAction::InsertCodeBelow => self.mutate_document(|this| {
                let block = NotebookBlock::executable(this.next_cell_id(), "poincare", "");
                this.insert_block(index + 1, block);
            }),
            CellAction::Delete => {
                if self.document.blocks.len() > 1 {
                    self.mutate_document(|this| {
                        this.document.blocks.remove(index);
                        this.selected_cell = this
                            .document
                            .blocks
                            .get(index)
                            .or_else(|| this.document.blocks.last())
                            .map(|block| block.id.clone());
                    });
                }
            }
            CellAction::Duplicate => self.mutate_document(|this| {
                let mut block = this.document.blocks[index].clone();
                block.id = this.next_cell_id();
                this.insert_block(index + 1, block);
            }),
            CellAction::MoveUp => {
                if index > 0 {
                    self.mutate_document(|this| this.document.blocks.swap(index, index - 1));
                }
            }
            CellAction::MoveDown => {
                if index + 1 < self.document.blocks.len() {
                    self.mutate_document(|this| this.document.blocks.swap(index, index + 1));
                }
            }
            CellAction::ToggleCollapsed => self.mutate_document(|this| {
                let state = &mut this.document.blocks[index].state;
                state.collapsed = !state.collapsed;
            }),
            CellAction::ToggleOutputs => self.mutate_document(|this| {
                let state = &mut this.document.blocks[index].state;
                state.outputs_collapsed = !state.outputs_collapsed;
            }),
            CellAction::ConvertToMarkdown => self.mutate_document(|this| {
                let source = block_source(&this.document.blocks[index]);
                this.document.blocks[index].kind = NotebookBlockKind::Text(TextCell {
                    format: TextFormat::Markdown,
                    source,
                });
            }),
            CellAction::ConvertToCode => self.mutate_document(|this| {
                let source = block_source(&this.document.blocks[index]);
                this.document.blocks[index].kind =
                    NotebookBlockKind::Executable(poincare_notebook_lib::ExecutableCell {
                        language_id: "poincare".to_string(),
                        source,
                        execution: ExecutionState::Idle,
                    });
            }),
        }
    }

    fn mark_cell_edited(&mut self, previous: NotebookDocument) {
        self.record_undo_document(previous);
        self.dirty = true;
    }

    fn next_cell_id(&mut self) -> NotebookCellId {
        let id = NotebookCellId::new(format!("cell-{}", self.next_cell_id));
        self.next_cell_id += 1;
        id
    }

    fn next_notebook_id(&mut self) -> NotebookId {
        NotebookId::new(format!("notebook-{}", self.next_cell_id))
    }
}

impl eframe::App for NotebookApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_top_bar(ctx);
        self.show_side_panel(ctx);
        self.show_document(ctx);
    }
}

impl NotebookApp {
    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top-bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("New").clicked() {
                    self.new_document();
                }
                if ui.button("Open").clicked() {
                    self.open_document();
                }
                if ui.button("Save").clicked() {
                    self.save_document();
                }
                if ui.button("Save As").clicked() {
                    self.save_document_as();
                }
                ui.separator();
                if ui
                    .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui.button("Markdown").clicked() {
                    let index = selected_or_end_index(&self.document, &self.selected_cell);
                    self.handle_cell_action(index, CellAction::InsertMarkdownBelow);
                }
                if ui.button("Code").clicked() {
                    let index = selected_or_end_index(&self.document, &self.selected_cell);
                    self.handle_cell_action(index, CellAction::InsertCodeBelow);
                }
                ui.separator();
                ui.checkbox(&mut self.show_side_panel, "Variables");
                ui.separator();
                ui.label(document_title(self));
            });
        });
    }

    fn show_side_panel(&mut self, ctx: &egui::Context) {
        if !self.show_side_panel {
            return;
        }

        egui::SidePanel::right("session-panel")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Session");
                ui.label("No evaluator session");
                ui.separator();
                ui.heading("Document");
                ui.label(format!("{} cells", self.document.blocks.len()));
                if let Some(path) = &self.path {
                    ui.label(path.display().to_string());
                }
                if let Some(error) = &self.last_error {
                    ui.separator();
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
    }

    fn show_document(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(self.document.title.as_str());
                    if self.dirty {
                        ui.label("unsaved");
                    }
                });
                ui.add_space(6.0);

                let mut pending_action = None;
                let mut edited_snapshot = None;
                egui::ScrollArea::vertical()
                    .id_salt("notebook-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, block) in self.document.blocks.iter_mut().enumerate() {
                            let previous_block = block.clone();
                            let selected = self
                                .selected_cell
                                .as_ref()
                                .map(|cell_id| *cell_id == block.id)
                                .unwrap_or(false);
                            let cell_response = cells::show_cell(ui, block, selected);
                            if cell_response.clicked {
                                self.selected_cell = Some(block.id.clone());
                            }
                            if cell_response.edited {
                                edited_snapshot = Some((index, previous_block));
                            }
                            if let Some(action) = cell_response.action {
                                pending_action = Some((index, action));
                            }
                            ui.add_space(8.0);
                        }
                    });

                if let Some((index, previous_block)) = edited_snapshot {
                    let mut previous = self.document.clone();
                    if let Some(block) = previous.blocks.get_mut(index) {
                        *block = previous_block;
                    }
                    self.selected_cell = self
                        .document
                        .blocks
                        .get(index)
                        .map(|block| block.id.clone());
                    self.mark_cell_edited(previous);
                }
                if let Some((index, action)) = pending_action {
                    self.handle_cell_action(index, action);
                }
            });
        });
    }
}

fn block_source(block: &NotebookBlock) -> String {
    match &block.kind {
        NotebookBlockKind::Text(cell) => cell.source.clone(),
        NotebookBlockKind::Executable(cell) => cell.source.clone(),
        _ => String::new(),
    }
}

fn selected_or_end_index(document: &NotebookDocument, selected: &Option<NotebookCellId>) -> usize {
    selected
        .as_ref()
        .and_then(|cell_id| {
            document
                .blocks
                .iter()
                .position(|block| block.id == *cell_id)
        })
        .unwrap_or_else(|| document.blocks.len().saturating_sub(1))
}

fn next_cell_counter(document: &NotebookDocument) -> u64 {
    document
        .blocks
        .iter()
        .filter_map(|block| block.id.0.strip_prefix("cell-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn document_title(app: &NotebookApp) -> String {
    let mut title = app.document.title.clone();
    if app.dirty {
        title.push_str(" *");
    }
    title
}
