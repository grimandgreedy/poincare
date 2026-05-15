use eframe::egui;

use crate::App;
use crate::plot::kind::{PlotKind, StyleCaps, evenly_spaced_isovalues};
use crate::ui::domain_editor::{edit_domain, edit_resolution};
use crate::ui::expr_params::show_expression_params;
use crate::ui::style_editor::{align_surface_colour_for_lic, edit_plot_style};

impl App {
    pub(crate) fn plot_properties_panel(&mut self, ui: &mut egui::Ui) {
        let mut selected_dirty = false;
        let doc_idx = self.active_document_idx;

        let Some(index) = self.documents[doc_idx].selected_plot else {
            ui.label("Select a plot to edit its domain, resolution, and style.");
            return;
        };

        // Ensure sweep_config has an entry for every plot (grown lazily).
        {
            let n = self.documents[doc_idx].plots.len();
            self.documents[doc_idx].sweep_config.resize_with(n, Default::default);
        }

        // Borrow three disjoint fields of App simultaneously:
        //   self.slider_dragging, self.eq_editor  (App fields)
        //   self.documents[doc_idx]               (App field, different from the above)
        let slider_dragging = &mut self.slider_dragging;
        let eq_editor = &mut self.eq_editor;
        let doc = &mut self.documents[doc_idx];

        // Split doc into two disjoint field borrows so we can hold both
        // plot (&mut PlotEntry) and sweep_map (&mut SweepMap) at the same time.
        let plots = &mut doc.plots;
        let sweep_config = &mut doc.sweep_config;

        if let Some(plot) = plots.get_mut(index) {
            ui.label(format!("Selected: {}", plot.name));
            ui.add_space(6.0);

            selected_dirty |= edit_domain(ui, &mut plot.domain, plot.kind.domain_labels());

            ui.add_space(6.0);
            let resolution_label = if plot.kind.uses_seed_resolution() {
                "Seed Resolution"
            } else {
                "Resolution"
            };
            ui.label(resolution_label);
            selected_dirty |=
                edit_resolution(ui, &mut plot.resolution, plot.kind.uses_resolution());

            ui.add_space(6.0);
            ui.label("Style");
            selected_dirty |= ui
                .push_id("plot_style", |ui| {
                    edit_plot_style(ui, &mut plot.style, plot.kind.style_caps())
                })
                .inner;
            selected_dirty |= align_surface_colour_for_lic(&mut plot.style);

            if let PlotKind::ContouredSurface {
                contour_values,
                contour_style,
            } = &mut plot.kind
            {
                ui.add_space(6.0);
                ui.label("Contours");
                let mut contour_count = contour_values.len() as u32;
                if ui
                    .add(egui::Slider::new(&mut contour_count, 1..=20).text("Line Count"))
                    .changed()
                {
                    *contour_values = evenly_spaced_isovalues(contour_count as usize);
                    selected_dirty = true;
                }
                selected_dirty |= ui
                    .push_id("contour_style", |ui| {
                        edit_plot_style(
                            ui,
                            contour_style,
                            StyleCaps {
                                mesh: false,
                                line: true,
                                point: false,
                                glyph: false,
                            },
                        )
                    })
                    .inner;
            }

            let sweep_map = &mut sweep_config[index];

            let param_section_dirty = show_expression_params(
                ui,
                &mut plot.kind,
                slider_dragging,
                eq_editor,
                sweep_map,
            );
            selected_dirty |= param_section_dirty;
            // Last use of plot and sweep_map — NLL ends those borrows here.

            if selected_dirty {
                // plot/sweep_map borrows have ended; doc is still in scope but
                // plots/sweep_config sub-borrows are gone, so mark_dirty is valid.
                doc.mark_dirty();
                ui.colored_label(egui::Color32::YELLOW, "Pending scene rebuild");
            }
        }
    }

    pub(crate) fn export_panel(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        ui.heading("Export PNG");
        ui.text_edit_singleline(&mut self.documents[self.active_document_idx].export_path);
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.documents[self.active_document_idx].export_width)
                    .speed(1)
                    .range(256..=8192)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.documents[self.active_document_idx].export_height)
                    .speed(1)
                    .range(256..=8192)
                    .prefix("H "),
            );
        });
        if ui.button("Export PNG").clicked() {
            self.rebuild_scene(frame);
            self.export_png(frame);
        }
        if !self.documents[self.active_document_idx].export_status.is_empty() {
            ui.label(&self.documents[self.active_document_idx].export_status);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Shortcuts: F front, T top, I isometric, O projection toggle");
    }
}
