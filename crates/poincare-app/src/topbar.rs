use eframe::egui;
use poincare_lib::{AnalysisKind, available_analyses};
use viewport_lib::{Projection, ViewPreset};

use crate::App;
use crate::CameraCommand;
use crate::PlotPreset;
use crate::dock::DockTab;
use crate::presets::example_plots::ExamplePlot;

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
    ShowShortcuts,
    Quit,
    Undo,
    Redo,
    EditSelectedPlot,
    DuplicatePlot,
    DeletePlot,
    SelectedPlotAnalysis(SelectedPlotAnalysisAction),
    Camera(CameraCommand),
    LoadPreset(PlotPreset),
    LoadExample(ExamplePlot),
}

#[derive(Clone, Copy)]
enum SelectedPlotAnalysisAction {
    PointCloudStatistics,
    DataQualityChecks,
    SurfaceNormals,
    SurfaceCurvature,
    SurfaceArea,
    SurfaceMeshQuality,
    ScalarSliceZ,
    GradientField,
    VectorSliceZ,
    DivergenceField,
    CurlField,
    DifferentiateCurve,
    IntegralCurve,
    TangentField,
    FrenetFrame,
    BishopFrame,
    DarbouxFrame,
    SurfaceAlignedFrame,
    AxisDerivativeCurve,
    ArcLengthCurve,
    CurvatureCurve,
    NormalField,
    BinormalField,
    FitCurve,
    InterpolateCurve,
    ExtractPoints,
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
                self.menu_help(ui, ctx);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        self.pending_focus_tab = Some(DockTab::ExportProperties);
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
            let can_undo = self.documents[self.active_document_idx].can_undo();
            let can_redo = self.documents[self.active_document_idx].can_redo();
            if ui
                .add_enabled(can_undo, egui::Button::new("Undo"))
                .clicked()
            {
                self.execute_palette_command(PaletteCommand::Undo, ui.ctx());
                ui.close();
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("Redo"))
                .clicked()
            {
                self.execute_palette_command(PaletteCommand::Redo, ui.ctx());
                ui.close();
            }
            ui.separator();
            let selected = self.documents[self.active_document_idx].selected_plot;
            ui.add_enabled_ui(selected.is_some(), |ui| {
                if ui.button("Edit Selected Plot").clicked() {
                    self.execute_palette_command(PaletteCommand::EditSelectedPlot, ui.ctx());
                    ui.close();
                }
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
            for (label, preset) in [
                ("Front", ViewPreset::Front),
                ("Back", ViewPreset::Back),
                ("Left", ViewPreset::Left),
                ("Right", ViewPreset::Right),
                ("Top", ViewPreset::Top),
                ("Bottom", ViewPreset::Bottom),
                ("Isometric", ViewPreset::Isometric),
            ] {
                if ui.button(label).clicked() {
                    self.execute_palette_command(
                        PaletteCommand::Camera(CameraCommand::ViewPreset(preset)),
                        ui.ctx(),
                    );
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Frame All").clicked() {
                self.execute_palette_command(
                    PaletteCommand::Camera(CameraCommand::FrameAll),
                    ui.ctx(),
                );
                ui.close();
            }
            if ui.button("Frame Selected").clicked() {
                self.execute_palette_command(
                    PaletteCommand::Camera(CameraCommand::FrameSelected),
                    ui.ctx(),
                );
                ui.close();
            }
            if ui.button("Reset View").clicked() {
                self.execute_palette_command(
                    PaletteCommand::Camera(CameraCommand::ResetView),
                    ui.ctx(),
                );
                ui.close();
            }
            ui.separator();
            if ui.button("Perspective").clicked() {
                self.execute_palette_command(
                    PaletteCommand::Camera(CameraCommand::SetProjection(Projection::Perspective)),
                    ui.ctx(),
                );
                ui.close();
            }
            if ui.button("Orthographic").clicked() {
                self.execute_palette_command(
                    PaletteCommand::Camera(CameraCommand::SetProjection(Projection::Orthographic)),
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

    fn menu_help(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("Help", |ui| {
            if ui.button("Keyboard Shortcuts").clicked() {
                self.execute_palette_command(PaletteCommand::ShowShortcuts, ctx);
                ui.close();
            }
        });
    }

    fn duplicate_selected_plot(&mut self) {
        if let Some(idx) = self.documents[self.active_document_idx].selected_plot {
            let mut cloned = self.documents[self.active_document_idx].plots[idx].clone();
            cloned.name = format!("{} (copy)", cloned.name);
            cloned.plot_id = 0;
            cloned.parent_plot_id = None;
            cloned.relationship = crate::plot::entry::PlotRelationship::Primary;
            let inserted_idx = self.insert_plot_entry(self.active_document_idx, idx + 1, cloned);
            self.set_selected_plot(self.active_document_idx, Some(inserted_idx));
            self.documents[self.active_document_idx].viewport_selection_hidden_for = None;
            self.mark_dirty();
        }
    }

    pub(crate) fn request_delete_selected_plot(&mut self) {
        if let Some(idx) = self.documents[self.active_document_idx].selected_plot {
            self.confirm_delete_plot_idx = Some(idx);
        }
    }

    pub(crate) fn confirm_delete_selected_plot(&mut self) {
        let Some(idx) = self.confirm_delete_plot_idx.take() else {
            return;
        };
        if idx >= self.documents[self.active_document_idx].plots.len() {
            return;
        }
        if self.documents[self.active_document_idx].selected_plot != Some(idx) {
            self.set_selected_plot(self.active_document_idx, Some(idx));
        }
        if let Some(idx) = self.documents[self.active_document_idx].selected_plot {
            self.documents[self.active_document_idx].remove_plot_family(idx);
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
        self.record_undo_point();
        let selected_idx = self.append_plot_entry(self.active_document_idx, example.build());
        let doc = &mut self.documents[self.active_document_idx];
        doc.sweep_config
            .resize_with(doc.plots.len(), Default::default);
        let _ = doc;
        self.set_selected_plot(self.active_document_idx, Some(selected_idx));
        let doc = &mut self.documents[self.active_document_idx];
        doc.viewport_selection_hidden_for = None;
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
            PaletteCommand::ExportPng => self.pending_focus_tab = Some(DockTab::ExportProperties),
            PaletteCommand::Settings => self.settings_open = true,
            PaletteCommand::ShowShortcuts => self.shortcuts_open = true,
            PaletteCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            PaletteCommand::Undo => self.undo_active_document(),
            PaletteCommand::Redo => self.redo_active_document(),
            PaletteCommand::EditSelectedPlot => self.open_selected_plot_editor(),
            PaletteCommand::DuplicatePlot => self.duplicate_selected_plot(),
            PaletteCommand::DeletePlot => self.request_delete_selected_plot(),
            PaletteCommand::SelectedPlotAnalysis(action) => {
                self.execute_selected_plot_analysis_command(action)
            }
            PaletteCommand::Camera(command) => self.run_camera_command(command),
            PaletteCommand::LoadPreset(preset) => self.load_preset(preset),
            PaletteCommand::LoadExample(example) => self.load_example_plot(example),
        }
    }

    fn execute_selected_plot_analysis_command(&mut self, action: SelectedPlotAnalysisAction) {
        let doc_idx = self.active_document_idx;
        let Some(plot_idx) = self.documents[doc_idx].selected_plot else {
            return;
        };
        let plot = self.documents[doc_idx].plots[plot_idx].clone();
        let plot_spec = plot.to_plot_spec();
        match action {
            SelectedPlotAnalysisAction::PointCloudStatistics => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::PointCloudStatistics,
                vec![],
            ),
            SelectedPlotAnalysisAction::DataQualityChecks => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::DataQualityChecks,
                vec![],
            ),
            SelectedPlotAnalysisAction::SurfaceNormals => {
                self.open_surface_normals_modal(plot_idx)
            }
            SelectedPlotAnalysisAction::SurfaceCurvature => {
                self.run_surface_plot_analysis(
                    doc_idx,
                    plot_idx,
                    AnalysisKind::SurfaceCurvature,
                    vec![],
                )
            }
            SelectedPlotAnalysisAction::SurfaceArea => {
                self.run_surface_plot_analysis(
                    doc_idx,
                    plot_idx,
                    AnalysisKind::SurfaceArea,
                    vec![],
                )
            }
            SelectedPlotAnalysisAction::SurfaceMeshQuality => self.run_surface_plot_analysis(
                doc_idx,
                plot_idx,
                AnalysisKind::SurfaceMeshQuality,
                vec![],
            ),
            SelectedPlotAnalysisAction::ScalarSliceZ => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::ScalarSlice,
                vec![("axis".to_string(), "z".to_string())],
            ),
            SelectedPlotAnalysisAction::GradientField => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::GradientField,
                vec![],
            ),
            SelectedPlotAnalysisAction::VectorSliceZ => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::VectorSlice,
                vec![("axis".to_string(), "z".to_string())],
            ),
            SelectedPlotAnalysisAction::DivergenceField => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::DivergenceField,
                vec![],
            ),
            SelectedPlotAnalysisAction::CurlField => {
                self.run_single_plot_analysis(doc_idx, &plot_spec, AnalysisKind::CurlField, vec![])
            }
            SelectedPlotAnalysisAction::DifferentiateCurve => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::DifferentiateCurve,
                vec![],
            ),
            SelectedPlotAnalysisAction::IntegralCurve => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::IntegralCurve,
                vec![],
            ),
            SelectedPlotAnalysisAction::TangentField => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::TangentField,
                vec![],
            ),
            SelectedPlotAnalysisAction::FrenetFrame => {
                self.open_moving_frame_modal(plot_idx, AnalysisKind::FrenetFrame, None)
            }
            SelectedPlotAnalysisAction::BishopFrame => {
                self.open_moving_frame_modal(plot_idx, AnalysisKind::BishopFrame, None)
            }
            SelectedPlotAnalysisAction::DarbouxFrame => {
                let target = self
                    .surface_frame_candidates(doc_idx, plot_idx)
                    .first()
                    .map(|(index, _)| *index);
                self.open_moving_frame_modal(plot_idx, AnalysisKind::DarbouxFrame, target)
            }
            SelectedPlotAnalysisAction::SurfaceAlignedFrame => {
                let target = self
                    .surface_frame_candidates(doc_idx, plot_idx)
                    .first()
                    .map(|(index, _)| *index);
                self.open_moving_frame_modal(plot_idx, AnalysisKind::SurfaceAlignedFrame, target)
            }
            SelectedPlotAnalysisAction::AxisDerivativeCurve => {
                self.open_axis_derivative_modal(plot_idx, &plot)
            }
            SelectedPlotAnalysisAction::ArcLengthCurve => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::ArcLengthCurve,
                vec![],
            ),
            SelectedPlotAnalysisAction::CurvatureCurve => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::CurvatureCurve,
                vec![],
            ),
            SelectedPlotAnalysisAction::NormalField => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::NormalField,
                vec![],
            ),
            SelectedPlotAnalysisAction::BinormalField => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::BinormalField,
                vec![],
            ),
            SelectedPlotAnalysisAction::FitCurve => self.open_fit_curve_modal(plot_idx, &plot),
            SelectedPlotAnalysisAction::InterpolateCurve => {
                self.open_interpolate_modal(plot_idx, &plot)
            }
            SelectedPlotAnalysisAction::ExtractPoints => self.run_single_plot_analysis(
                doc_idx,
                &plot_spec,
                AnalysisKind::ExtractPoints,
                vec![],
            ),
        }
    }

    fn selected_plot_analysis_items(&self) -> Vec<PaletteItem> {
        let doc_idx = self.active_document_idx;
        let Some(plot_idx) = self.documents[doc_idx].selected_plot else {
            return Vec::new();
        };
        let plot = &self.documents[doc_idx].plots[plot_idx];
        let plot_spec = plot.to_plot_spec();
        let capabilities = available_analyses(&plot_spec);
        let has_analysis = |kind| capabilities.iter().any(|cap| cap.kind == kind);
        let plot_name = plot.name.clone();
        let mut items = Vec::new();

        let mut push = |label: &str, action: SelectedPlotAnalysisAction, enabled: bool| {
            items.push(PaletteItem {
                label: format!("Analysis: {label} ({plot_name})"),
                command: PaletteCommand::SelectedPlotAnalysis(action),
                enabled,
            });
        };

        push(
            "Point Statistics",
            SelectedPlotAnalysisAction::PointCloudStatistics,
            has_analysis(AnalysisKind::PointCloudStatistics),
        );
        push(
            "Data Quality Checks",
            SelectedPlotAnalysisAction::DataQualityChecks,
            has_analysis(AnalysisKind::DataQualityChecks),
        );
        push(
            "Visualize Surface Normals",
            SelectedPlotAnalysisAction::SurfaceNormals,
            has_analysis(AnalysisKind::SurfaceNormals),
        );
        push(
            "Surface Curvature",
            SelectedPlotAnalysisAction::SurfaceCurvature,
            has_analysis(AnalysisKind::SurfaceCurvature),
        );
        push(
            "Surface Area",
            SelectedPlotAnalysisAction::SurfaceArea,
            has_analysis(AnalysisKind::SurfaceArea),
        );
        push(
            "Surface Mesh Quality",
            SelectedPlotAnalysisAction::SurfaceMeshQuality,
            has_analysis(AnalysisKind::SurfaceMeshQuality),
        );
        push(
            "Add Z Slice",
            SelectedPlotAnalysisAction::ScalarSliceZ,
            has_analysis(AnalysisKind::ScalarSlice),
        );
        push(
            "Add Gradient Field",
            SelectedPlotAnalysisAction::GradientField,
            has_analysis(AnalysisKind::GradientField),
        );
        push(
            "Add Z Vector Slice",
            SelectedPlotAnalysisAction::VectorSliceZ,
            has_analysis(AnalysisKind::VectorSlice),
        );
        push(
            "Add Divergence Volume",
            SelectedPlotAnalysisAction::DivergenceField,
            has_analysis(AnalysisKind::DivergenceField),
        );
        push(
            "Add Curl Field",
            SelectedPlotAnalysisAction::CurlField,
            has_analysis(AnalysisKind::CurlField),
        );
        push(
            "Create Derivative Curve",
            SelectedPlotAnalysisAction::DifferentiateCurve,
            has_analysis(AnalysisKind::DifferentiateCurve),
        );
        push(
            "Create Integral Curve",
            SelectedPlotAnalysisAction::IntegralCurve,
            has_analysis(AnalysisKind::IntegralCurve),
        );
        push(
            "Create Tangent Curve",
            SelectedPlotAnalysisAction::TangentField,
            has_analysis(AnalysisKind::TangentField),
        );
        push(
            "Frenet Frame...",
            SelectedPlotAnalysisAction::FrenetFrame,
            has_analysis(AnalysisKind::FrenetFrame),
        );
        push(
            "Bishop Frame...",
            SelectedPlotAnalysisAction::BishopFrame,
            has_analysis(AnalysisKind::BishopFrame),
        );
        push(
            "Darboux Frame...",
            SelectedPlotAnalysisAction::DarbouxFrame,
            !self.surface_frame_candidates(doc_idx, plot_idx).is_empty(),
        );
        push(
            "Surface-Aligned Frame...",
            SelectedPlotAnalysisAction::SurfaceAlignedFrame,
            !self.surface_frame_candidates(doc_idx, plot_idx).is_empty(),
        );
        push(
            "Differentiate by Axis…",
            SelectedPlotAnalysisAction::AxisDerivativeCurve,
            has_analysis(AnalysisKind::AxisDerivativeCurve),
        );
        push(
            "Create Arc Length Curve",
            SelectedPlotAnalysisAction::ArcLengthCurve,
            has_analysis(AnalysisKind::ArcLengthCurve),
        );
        push(
            "Create Curvature Curve",
            SelectedPlotAnalysisAction::CurvatureCurve,
            has_analysis(AnalysisKind::CurvatureCurve),
        );
        push(
            "Create Normal Curve",
            SelectedPlotAnalysisAction::NormalField,
            has_analysis(AnalysisKind::NormalField),
        );
        push(
            "Create Binormal Curve",
            SelectedPlotAnalysisAction::BinormalField,
            has_analysis(AnalysisKind::BinormalField),
        );
        push(
            "Fit Curve…",
            SelectedPlotAnalysisAction::FitCurve,
            has_analysis(AnalysisKind::FitCurve),
        );
        push(
            "Interpolate…",
            SelectedPlotAnalysisAction::InterpolateCurve,
            has_analysis(AnalysisKind::InterpolateCurve),
        );
        push(
            "Extract Points",
            SelectedPlotAnalysisAction::ExtractPoints,
            has_analysis(AnalysisKind::ExtractPoints),
        );

        items
    }

    fn command_palette_items(&self) -> Vec<PaletteItem> {
        let has_selected_plot = self.documents[self.active_document_idx]
            .selected_plot
            .is_some();
        let can_undo = self.documents[self.active_document_idx].can_undo();
        let can_redo = self.documents[self.active_document_idx].can_redo();
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
                label: "Help: Keyboard Shortcuts".to_string(),
                command: PaletteCommand::ShowShortcuts,
                enabled: true,
            },
            PaletteItem {
                label: "Edit: Undo".to_string(),
                command: PaletteCommand::Undo,
                enabled: can_undo,
            },
            PaletteItem {
                label: "Edit: Redo".to_string(),
                command: PaletteCommand::Redo,
                enabled: can_redo,
            },
            PaletteItem {
                label: "Edit: Edit Selected Plot".to_string(),
                command: PaletteCommand::EditSelectedPlot,
                enabled: has_selected_plot,
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
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Front)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Back".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Back)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Left".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Left)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Right".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Right)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Top".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Top)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Bottom".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Bottom)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Isometric".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ViewPreset(ViewPreset::Isometric)),
                enabled: true,
            },
            PaletteItem {
                label: "View: Frame All".to_string(),
                command: PaletteCommand::Camera(CameraCommand::FrameAll),
                enabled: true,
            },
            PaletteItem {
                label: "View: Frame Selected".to_string(),
                command: PaletteCommand::Camera(CameraCommand::FrameSelected),
                enabled: true,
            },
            PaletteItem {
                label: "View: Reset View".to_string(),
                command: PaletteCommand::Camera(CameraCommand::ResetView),
                enabled: true,
            },
            PaletteItem {
                label: "View: Perspective".to_string(),
                command: PaletteCommand::Camera(CameraCommand::SetProjection(
                    Projection::Perspective,
                )),
                enabled: true,
            },
            PaletteItem {
                label: "View: Orthographic".to_string(),
                command: PaletteCommand::Camera(CameraCommand::SetProjection(
                    Projection::Orthographic,
                )),
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
        items.extend(self.selected_plot_analysis_items());

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
        let mut items: Vec<PaletteItem> = self
            .command_palette_items()
            .into_iter()
            .filter(|item| item.label.to_lowercase().contains(&query))
            .collect();
        items.sort_by_key(|item| !item.enabled);

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

    pub(crate) fn show_shortcuts_modal(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            return;
        }

        let mut open = self.shortcuts_open;
        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                shortcut_section(
                    ui,
                    "File",
                    &[
                        ("Cmd/Ctrl+O", "Open document"),
                        ("Cmd/Ctrl+S", "Save document"),
                        ("Cmd/Ctrl+Shift+S", "Save document as"),
                        ("Cmd/Ctrl+,", "Open settings"),
                    ],
                );
                ui.separator();
                shortcut_section(
                    ui,
                    "Command",
                    &[
                        ("Cmd/Ctrl+K", "Open command palette"),
                        ("?", "Open keyboard shortcuts"),
                        ("Cmd/Ctrl+Z", "Undo"),
                        ("Cmd/Ctrl+Shift+Z", "Redo"),
                        ("E", "Edit selected plot"),
                        ("V", "Toggle selected plot visibility"),
                        ("J", "Select next plot"),
                        ("K", "Select previous plot"),
                        ("G", "Select first plot"),
                        ("Shift+G", "Select last plot"),
                    ],
                );
                ui.separator();
                shortcut_section(
                    ui,
                    "Camera",
                    &[
                        ("F", "Front view"),
                        ("T", "Top view"),
                        ("I", "Isometric view"),
                        ("O", "Toggle perspective / orthographic"),
                    ],
                );
                ui.separator();
                shortcut_section(
                    ui,
                    "Viewport",
                    &[
                        ("Left drag", "Orbit camera"),
                        ("Right drag", "Pan camera"),
                        ("Scroll", "Zoom"),
                        ("Axis indicator click", "Snap to axis view"),
                    ],
                );
            });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            open = false;
        }
        self.shortcuts_open = open;
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

fn shortcut_section(ui: &mut egui::Ui, title: &str, rows: &[(&str, &str)]) {
    ui.label(egui::RichText::new(title).strong());
    egui::Grid::new(format!("shortcuts_{title}"))
        .num_columns(2)
        .spacing([20.0, 6.0])
        .show(ui, |ui| {
            for (shortcut, description) in rows {
                ui.monospace(*shortcut);
                ui.label(*description);
                ui.end_row();
            }
        });
}
