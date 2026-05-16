use eframe::egui;
use viewport_lib::ViewPreset;

use crate::presets::example_plots::ExamplePlot;
use crate::App;
use crate::PlotPreset;

#[derive(Clone, Copy)]
enum PaletteCommand {
    AddPlot,
    NewDocument,
    OpenDocument,
    SaveDocument,
    SaveDocumentAs,
    CloseTab,
    ExportPng,
    Settings,
    Quit,
    DuplicatePlot,
    DeletePlot,
    ViewPreset(ViewPreset),
    LoadPreset(PlotPreset),
    LoadExample(ExamplePlot),
}

struct PaletteItem {
    label: String,
    command: PaletteCommand,
    enabled: bool,
}

impl App {
    /// Render the top menu bar and document tab strip.
    /// Must be called before `CentralPanel` in the update loop.
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("poincare_menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menu_file(ui, ctx);
                self.menu_edit(ui);
                self.menu_view(ui);
                self.menu_examples(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export PNG").clicked() {
                        self.export_open = true;
                    }
                });
            });
        });

        egui::TopBottomPanel::top("poincare_doc_tabs").show(ctx, |ui| {
            self.document_tab_strip(ui);
        });
    }

    fn menu_file(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("File", |ui| {
            if ui.button("New").clicked() {
                self.execute_palette_command(PaletteCommand::NewDocument, ctx);
                ui.close();
            }
            if ui.button("Open\u{2026}").clicked() {
                self.execute_palette_command(PaletteCommand::OpenDocument, ctx);
                ui.close();
            }
            ui.separator();
            if ui.button("Save").clicked() {
                self.execute_palette_command(PaletteCommand::SaveDocument, ctx);
                ui.close();
            }
            if ui.button("Save As\u{2026}").clicked() {
                self.execute_palette_command(PaletteCommand::SaveDocumentAs, ctx);
                ui.close();
            }
            ui.separator();
            if ui.button("Close Tab").clicked() {
                self.execute_palette_command(PaletteCommand::CloseTab, ctx);
                ui.close();
            }
            ui.separator();
            if ui.button("Settings\u{2026}").clicked() {
                self.execute_palette_command(PaletteCommand::Settings, ctx);
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                self.execute_palette_command(PaletteCommand::Quit, ctx);
                ui.close();
            }
        });
    }

    fn menu_edit(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            let selected = self.documents[self.active_document_idx].selected_plot;
            ui.add_enabled_ui(selected.is_some(), |ui| {
                if ui.button("Duplicate Plot").clicked() {
                    self.execute_palette_command(PaletteCommand::DuplicatePlot, ui.ctx());
                    ui.close();
                }
                if ui.button("Delete Plot").clicked() {
                    self.execute_palette_command(PaletteCommand::DeletePlot, ui.ctx());
                    ui.close();
                }
            });
        });
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            if ui.button("Front").clicked() {
                self.execute_palette_command(
                    PaletteCommand::ViewPreset(ViewPreset::Front),
                    ui.ctx(),
                );
                ui.close();
            }
            if ui.button("Top").clicked() {
                self.execute_palette_command(PaletteCommand::ViewPreset(ViewPreset::Top), ui.ctx());
                ui.close();
            }
            if ui.button("Isometric").clicked() {
                self.execute_palette_command(
                    PaletteCommand::ViewPreset(ViewPreset::Isometric),
                    ui.ctx(),
                );
                ui.close();
            }
        });
    }

    fn menu_examples(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Examples", |ui| {
            ui.label(egui::RichText::new("Presets").small().strong());
            for &preset in PlotPreset::all() {
                if ui.button(preset.name()).clicked() {
                    self.execute_palette_command(PaletteCommand::LoadPreset(preset), ui.ctx());
                    ui.close();
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("Single Plots").small().strong());
            for &example in ExamplePlot::all() {
                if ui.button(example.name()).clicked() {
                    self.execute_palette_command(PaletteCommand::LoadExample(example), ui.ctx());
                    ui.close();
                }
            }
        });
    }

    fn duplicate_selected_plot(&mut self) {
        if let Some(idx) = self.documents[self.active_document_idx].selected_plot {
            let mut cloned = self.documents[self.active_document_idx].plots[idx].clone();
            cloned.name = format!("{} (copy)", cloned.name);
            self.documents[self.active_document_idx]
                .plots
                .insert(idx + 1, cloned);
            self.documents[self.active_document_idx].selected_plot = Some(idx + 1);
            self.mark_dirty();
        }
    }

    fn delete_selected_plot(&mut self) {
        if let Some(idx) = self.documents[self.active_document_idx].selected_plot {
            self.documents[self.active_document_idx].plots.remove(idx);
            let n = self.documents[self.active_document_idx].plots.len();
            self.documents[self.active_document_idx].selected_plot = if n == 0 {
                None
            } else {
                Some(idx.saturating_sub(1).min(n - 1))
            };
            self.mark_dirty();
        }
    }

    fn load_example_plot(&mut self, example: ExamplePlot) {
        let doc = &mut self.documents[self.active_document_idx];
        doc.plots = vec![example.build()];
        doc.sweep_config.clear();
        doc.selected_plot = Some(0);
        doc.scene_dirty = true;
        doc.export_status.clear();
    }

    fn execute_palette_command(&mut self, command: PaletteCommand, ctx: &egui::Context) {
        match command {
            PaletteCommand::AddPlot => self.open_add_plot_modal(),
            PaletteCommand::NewDocument => self.new_document(),
            PaletteCommand::OpenDocument => self.pending_open = true,
            PaletteCommand::SaveDocument => self.pending_save = true,
            PaletteCommand::SaveDocumentAs => self.pending_save_as = true,
            PaletteCommand::CloseTab => {
                let idx = self.active_document_idx;
                if self.documents[idx].dirty {
                    self.confirm_close_idx = Some(idx);
                } else {
                    self.close_document(idx);
                }
            }
            PaletteCommand::ExportPng => self.export_open = true,
            PaletteCommand::Settings => self.settings_open = true,
            PaletteCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            PaletteCommand::DuplicatePlot => self.duplicate_selected_plot(),
            PaletteCommand::DeletePlot => self.delete_selected_plot(),
            PaletteCommand::ViewPreset(preset) => self.set_view_preset(preset),
            PaletteCommand::LoadPreset(preset) => self.load_preset(preset),
            PaletteCommand::LoadExample(example) => self.load_example_plot(example),
        }
    }

    fn command_palette_items(&self) -> Vec<PaletteItem> {
        let has_selected_plot = self.documents[self.active_document_idx]
            .selected_plot
            .is_some();
        let mut items = vec![
            PaletteItem {
                label: "File: Add Plot".to_string(),
                command: PaletteCommand::AddPlot,
                enabled: true,
            },
            PaletteItem {
                label: "File: New".to_string(),
                command: PaletteCommand::NewDocument,
                enabled: true,
            },
            PaletteItem {
                label: "File: Open…".to_string(),
                command: PaletteCommand::OpenDocument,
                enabled: true,
            },
            PaletteItem {
                label: "File: Save".to_string(),
                command: PaletteCommand::SaveDocument,
                enabled: true,
            },
            PaletteItem {
                label: "File: Save As…".to_string(),
                command: PaletteCommand::SaveDocumentAs,
                enabled: true,
            },
            PaletteItem {
                label: "File: Close Tab".to_string(),
                command: PaletteCommand::CloseTab,
                enabled: true,
            },
            PaletteItem {
                label: "File: Settings…".to_string(),
                command: PaletteCommand::Settings,
                enabled: true,
            },
            PaletteItem {
                label: "File: Quit".to_string(),
                command: PaletteCommand::Quit,
                enabled: true,
            },
            PaletteItem {
                label: "Export: Export PNG".to_string(),
                command: PaletteCommand::ExportPng,
                enabled: true,
            },
            PaletteItem {
                label: "Edit: Duplicate Plot".to_string(),
                command: PaletteCommand::DuplicatePlot,
                enabled: has_selected_plot,
            },
            PaletteItem {
                label: "Edit: Delete Plot".to_string(),
                command: PaletteCommand::DeletePlot,
                enabled: has_selected_plot,
            },
            PaletteItem {
                label: "View: Front".to_string(),
                command: PaletteCommand::ViewPreset(ViewPreset::Front),
                enabled: true,
            },
            PaletteItem {
                label: "View: Top".to_string(),
                command: PaletteCommand::ViewPreset(ViewPreset::Top),
                enabled: true,
            },
            PaletteItem {
                label: "View: Isometric".to_string(),
                command: PaletteCommand::ViewPreset(ViewPreset::Isometric),
                enabled: true,
            },
        ];

        items.extend(PlotPreset::all().iter().copied().map(|preset| PaletteItem {
            label: format!("Examples: {}", preset.name()),
            command: PaletteCommand::LoadPreset(preset),
            enabled: true,
        }));
        items.extend(
            ExamplePlot::all()
                .iter()
                .copied()
                .map(|example| PaletteItem {
                    label: format!("Examples: {}", example.name()),
                    command: PaletteCommand::LoadExample(example),
                    enabled: true,
                }),
        );

        items
    }

    pub(crate) fn show_command_palette(&mut self, ctx: &egui::Context) {
        if !self.command_palette_open {
            return;
        }

        let mut open = self.command_palette_open;
        let mut close_requested = false;
        let mut execute: Option<PaletteCommand> = None;
        let mut query_changed = false;
        let query = self.command_palette_query.to_lowercase();
        let items: Vec<PaletteItem> = self
            .command_palette_items()
            .into_iter()
            .filter(|item| item.label.to_lowercase().contains(&query))
            .collect();

        if self.command_palette_selected >= items.len() {
            self.command_palette_selected = 0;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !items.is_empty() {
            self.command_palette_selected = (self.command_palette_selected + 1) % items.len();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !items.is_empty() {
            self.command_palette_selected =
                (self.command_palette_selected + items.len() - 1) % items.len();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(item) = items.get(self.command_palette_selected) {
                if item.enabled {
                    execute = Some(item.command);
                    close_requested = true;
                }
            }
        }

        let mut window_open = open;
        egui::Window::new("Command Palette")
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 56.0])
            .open(&mut window_open)
            .show(ctx, |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command_palette_query)
                        .hint_text("Type a command…"),
                );
                if self.command_palette_focus_pending {
                    response.request_focus();
                    self.command_palette_focus_pending = false;
                }
                query_changed = response.changed();

                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.label(egui::RichText::new("No matching commands").weak());
                        } else {
                            for (idx, item) in items.iter().enumerate() {
                                let selected = idx == self.command_palette_selected;
                                let response = ui.add_enabled(
                                    item.enabled,
                                    egui::Button::new(&item.label).selected(selected),
                                );
                                if response.clicked() {
                                    execute = Some(item.command);
                                    close_requested = true;
                                }
                            }
                        }
                    });
            });
        open = window_open;

        if query_changed {
            self.command_palette_selected = 0;
        }
        if close_requested {
            open = false;
        }
        if let Some(command) = execute {
            self.execute_palette_command(command, ctx);
        }
        if !open {
            self.command_palette_query.clear();
            self.command_palette_selected = 0;
        }
        self.command_palette_open = open;
    }

    fn document_tab_strip(&mut self, ui: &mut egui::Ui) {
        // Collect display info before the UI loop to avoid holding borrows into self.documents.
        let tabs: Vec<(String, bool)> = self
            .documents
            .iter()
            .map(|doc| {
                let title = if doc.title.is_empty() {
                    "Untitled".to_string()
                } else {
                    doc.title.clone()
                };
                (title, doc.dirty)
            })
            .collect();

        let n = tabs.len();
        let active = self.active_document_idx;
        let mut new_active = active;
        let mut close_idx: Option<usize> = None;
        let mut add_new = false;

        ui.horizontal(|ui| {
            for (i, (title, dirty)) in tabs.iter().enumerate() {
                let is_active = i == active;
                let label = if *dirty {
                    format!("{title} *")
                } else {
                    title.clone()
                };

                if ui.selectable_label(is_active, &label).clicked() && !is_active {
                    new_active = i;
                }
                if ui
                    .small_button("\u{00d7}")
                    .on_hover_text("Close tab")
                    .clicked()
                {
                    close_idx = Some(i);
                }
                if i + 1 < n {
                    ui.separator();
                }
            }
            ui.add_space(4.0);
            if ui.button("+").on_hover_text("New document").clicked() {
                add_new = true;
            }
        });

        if add_new {
            self.new_document();
        } else if let Some(idx) = close_idx {
            if self.documents[idx].dirty {
                self.confirm_close_idx = Some(idx);
            } else {
                self.close_document(idx);
            }
        } else if new_active != active {
            self.switch_document(new_active);
        }
    }
}
