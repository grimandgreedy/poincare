use eframe::egui;
use viewport_lib::ViewPreset;

use crate::App;
use crate::PlotPreset;

impl App {
    /// Render the top menu bar and document tab strip.
    /// Must be called before `CentralPanel` in the update loop.
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("poincare_menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menu_file(ui, ctx);
                self.menu_edit(ui);
                self.menu_view(ui);
                // self.menu_examples(ui);
            });
        });

        egui::TopBottomPanel::top("poincare_doc_tabs").show(ctx, |ui| {
            self.document_tab_strip(ui);
        });
    }

    fn menu_file(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("File", |ui| {
            if ui.button("New").clicked() {
                self.new_document();
                ui.close();
            }
            if ui.button("Open\u{2026}").clicked() {
                self.pending_open = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Save").clicked() {
                self.pending_save = true;
                ui.close();
            }
            if ui.button("Save As\u{2026}").clicked() {
                self.pending_save_as = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Close Tab").clicked() {
                let idx = self.active_document_idx;
                if self.documents[idx].dirty {
                    self.confirm_close_idx = Some(idx);
                } else {
                    self.close_document(idx);
                }
                ui.close();
            }
            ui.separator();
            if ui.button("Export PNG").clicked() {
                self.pending_export = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Settings\u{2026}").clicked() {
                self.settings_open = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ui.close();
            }
        });
    }

    fn menu_edit(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            let selected = self.documents[self.active_document_idx].selected_plot;
            ui.add_enabled_ui(selected.is_some(), |ui| {
                if ui.button("Duplicate Plot").clicked() {
                    if let Some(idx) = selected {
                        let mut cloned =
                            self.documents[self.active_document_idx].plots[idx].clone();
                        cloned.name = format!("{} (copy)", cloned.name);
                        self.documents[self.active_document_idx]
                            .plots
                            .insert(idx + 1, cloned);
                        self.documents[self.active_document_idx].selected_plot = Some(idx + 1);
                        self.mark_dirty();
                    }
                    ui.close();
                }
                if ui.button("Delete Plot").clicked() {
                    if let Some(idx) = selected {
                        self.documents[self.active_document_idx].plots.remove(idx);
                        let n = self.documents[self.active_document_idx].plots.len();
                        self.documents[self.active_document_idx].selected_plot =
                            if n == 0 { None } else { Some(idx.saturating_sub(1).min(n - 1)) };
                        self.mark_dirty();
                    }
                    ui.close();
                }
            });
        });
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            if ui.button("Front").clicked() {
                self.set_view_preset(ViewPreset::Front);
                ui.close();
            }
            if ui.button("Top").clicked() {
                self.set_view_preset(ViewPreset::Top);
                ui.close();
            }
            if ui.button("Isometric").clicked() {
                self.set_view_preset(ViewPreset::Isometric);
                ui.close();
            }
        });
    }

    fn menu_examples(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Examples", |ui| {
            for &preset in PlotPreset::all() {
                if ui.button(preset.name()).clicked() {
                    self.load_preset(preset);
                    ui.close();
                }
            }
        });
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
                if ui.small_button("\u{00d7}").on_hover_text("Close tab").clicked() {
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
