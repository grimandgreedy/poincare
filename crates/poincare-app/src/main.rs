//! Poincaré — standalone 3D graphing application.

mod dock;
mod document;
mod panels;
mod persistence;
mod picking;
mod plot;
mod presets;
mod settings;
mod topbar;
mod ui;

use std::path::PathBuf;

use eframe::egui;
use grimdock::{PanelStyle, PanelTree};
use poincare_lib::{AxisConfig, ColormapSource, ColourMode};
use viewport_lib::BuiltinColourmap;
use viewport_lib::{GroundPlaneMode, OrbitCameraController, Projection, ViewPreset, ViewportRenderer};

use dock::{DockTab, build_panel_tree};
use document::{DEFAULT_VIEWPORT_BACKGROUND, Document, default_camera};
use plot::entry::PlotEntry;
use plot::selected_type::SelectedPlotType;
use ui::equation_editor::EquationEditor;

fn default_panel_style() -> PanelStyle {
    PanelStyle {
        content_inset: 10.0,
        ..PanelStyle::default()
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Poincaré",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 780.0]),
            depth_buffer: 24,
            stencil_buffer: 8,
            ..Default::default()
        },
        Box::new(|cc| {
            let wgpu_state = cc
                .wgpu_render_state
                .as_ref()
                .expect("eframe wgpu backend required");

            let device = &wgpu_state.device;
            let format = wgpu_state.target_format;

            let renderer = ViewportRenderer::new(device, format);
            {
                let mut guard = wgpu_state.renderer.write();
                guard.callback_resources.insert(renderer);
            }
            Ok(Box::new(App::new(cc)))
        }),
    )
}

struct App {
    documents: Vec<Document>,
    active_document_idx: usize,
    pending_export: bool,
    pending_open: bool,
    pending_save: bool,
    pending_save_as: bool,
    confirm_close_idx: Option<usize>,
    confirm_quit: bool,
    orbit_controller: OrbitCameraController,
    last_axes_snap: Option<(usize, bool)>,
    last_viewport_size: [u32; 2],
    add_plot_type: SelectedPlotType,
    add_expr_fields: [String; 3],
    add_csv_text: String,
    add_iso_values_text: String,
    add_error: String,
    slider_dragging: bool,
    eq_editor: EquationEditor,
    default_colormap: BuiltinColourmap,
    invert_scroll: bool,
    save_state_on_exit: bool,
    settings_open: bool,
    panel_tree: Option<PanelTree<DockTab>>,
    panel_style: PanelStyle,
    pending_focus_tab: Option<DockTab>,
    renaming_plot: Option<usize>,
    rename_buf: String,
    rename_needs_focus: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            documents: vec![Document::new_default()],
            active_document_idx: 0,
            pending_export: false,
            pending_open: false,
            pending_save: false,
            pending_save_as: false,
            confirm_close_idx: None,
            confirm_quit: false,
            orbit_controller: OrbitCameraController::viewport_primitives(),
            last_axes_snap: None,
            last_viewport_size: [1000, 700],
            add_plot_type: SelectedPlotType::Auto,
            add_expr_fields: [String::new(), String::new(), String::new()],
            add_csv_text: String::new(),
            add_iso_values_text: "1.0, 2.0, 3.0".to_string(),
            add_error: String::new(),
            slider_dragging: false,
            eq_editor: EquationEditor::default(),
            default_colormap: BuiltinColourmap::Viridis,
            invert_scroll: false,
            save_state_on_exit: false,
            settings_open: false,
            panel_tree: Some(build_panel_tree()),
            panel_style: default_panel_style(),
            pending_focus_tab: None,
            renaming_plot: None,
            rename_buf: String::new(),
            rename_needs_focus: false,
        };
        persistence::load_persisted_state(cc.storage, &mut app);
        app
    }

    pub(crate) fn active_document(&self) -> &Document {
        &self.documents[self.active_document_idx]
    }

    pub(crate) fn active_document_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active_document_idx]
    }

    pub(crate) fn new_document(&mut self) {
        self.documents.push(Document::new_default());
        self.active_document_idx = self.documents.len() - 1;
    }

    pub(crate) fn close_document(&mut self, idx: usize) {
        if self.documents.len() <= 1 {
            self.documents[0] = Document::new_default();
            self.active_document_idx = 0;
            return;
        }
        self.documents.remove(idx);
        if self.active_document_idx >= self.documents.len() {
            self.active_document_idx = self.documents.len() - 1;
        }
    }

    pub(crate) fn switch_document(&mut self, idx: usize) {
        if idx < self.documents.len() {
            self.active_document_idx = idx;
        }
    }

    pub(crate) fn load_preset(&mut self, preset: PlotPreset) {
        self.documents[self.active_document_idx].plots = preset.build();
        self.documents[self.active_document_idx].sweep_config = Vec::new();
        self.documents[self.active_document_idx].selected_plot =
            (!self.documents[self.active_document_idx].plots.is_empty()).then_some(0);
        self.apply_preset_view_settings(preset);
        self.documents[self.active_document_idx].scene_dirty = true;
        self.documents[self.active_document_idx].export_status.clear();
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.documents[self.active_document_idx].mark_dirty();
    }

    pub(crate) fn reset_settings_to_defaults(&mut self) {
        self.documents[self.active_document_idx].axis_config = AxisConfig::default();
        self.documents[self.active_document_idx].camera = default_camera();
        self.documents[self.active_document_idx].ground_plane_mode = GroundPlaneMode::None;
        self.documents[self.active_document_idx].ground_plane_height = 0.0;
        self.documents[self.active_document_idx].ground_plane_color = [0.3, 0.3, 0.3, 1.0];
        self.documents[self.active_document_idx].ground_plane_tile_size = 1.0;
        self.documents[self.active_document_idx].viewport_background = DEFAULT_VIEWPORT_BACKGROUND;
        self.default_colormap = BuiltinColourmap::Viridis;
        self.invert_scroll = false;
        self.save_state_on_exit = false;
        let default_style = default_panel_style();
        self.panel_style.header.bg = default_style.header.bg;
        self.panel_style.tabs.active.bg = default_style.tabs.active.bg;
        self.panel_style.content.bg = default_style.content.bg;
        self.panel_style.tabs.active.accent_color = default_style.tabs.active.accent_color;
        self.panel_style.tabs.inactive.accent_color = default_style.tabs.inactive.accent_color;
        self.panel_style.tabs.hovered.accent_color = default_style.tabs.hovered.accent_color;
        self.mark_dirty();
    }

    pub(crate) fn apply_default_colormap_to_entry(&self, entry: &mut PlotEntry) {
        if let ColourMode::Colormap {
            colormap: ColormapSource::Builtin(current),
            ..
        } = &mut entry.style.colour_mode
        {
            *current = self.default_colormap;
        }
    }

    pub(crate) fn rebuild_scene(&mut self, frame: &mut eframe::Frame) {
        let Some(mut scene) = self.documents[self.active_document_idx].build_scene_data() else {
            return;
        };

        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        let mut renderer_guard = render_state.renderer.write();
        let Some(viewport_renderer) = renderer_guard
            .callback_resources
            .get_mut::<ViewportRenderer>()
        else {
            return;
        };

        if let Err(err) = scene.upload_meshes(
            &render_state.device,
            &render_state.queue,
            viewport_renderer.resources_mut(),
        ) {
            self.documents[self.active_document_idx].export_status =
                format!("Scene rebuild failed: {err}");
            return;
        }

        self.documents[self.active_document_idx].scene = scene;
        self.documents[self.active_document_idx].scene_dirty = false;
        self.documents[self.active_document_idx].recompute_intersections();
    }

    pub(crate) fn export_png(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            self.documents[self.active_document_idx].export_status =
                "Export failed: no wgpu render state".to_string();
            return;
        };

        let mut renderer_guard = render_state.renderer.write();
        let Some(viewport_renderer) = renderer_guard
            .callback_resources
            .get_mut::<ViewportRenderer>()
        else {
            self.documents[self.active_document_idx].export_status =
                "Export failed: viewport renderer missing".to_string();
            return;
        };

        let mut export_camera = self.documents[self.active_document_idx].camera.clone();
        export_camera.set_aspect_ratio(
            self.documents[self.active_document_idx].export_width.max(1) as f32,
            self.documents[self.active_document_idx].export_height.max(1) as f32,
        );
        let mut frame_data = self.documents[self.active_document_idx]
            .scene
            .build_frame(&export_camera);
        frame_data.camera.viewport_size = [
            self.documents[self.active_document_idx].export_width as f32,
            self.documents[self.active_document_idx].export_height as f32,
        ];
        frame_data.viewport.show_grid = false;
        frame_data.viewport.background_colour =
            Some(self.documents[self.active_document_idx].viewport_background);
        frame_data.effects.ground_plane = viewport_lib::GroundPlane {
            mode: self.documents[self.active_document_idx].ground_plane_mode,
            height: self.documents[self.active_document_idx].ground_plane_height,
            colour: self.documents[self.active_document_idx].ground_plane_color,
            tile_size: self.documents[self.active_document_idx].ground_plane_tile_size,
            shadow_colour: [0.0, 0.0, 0.0, 1.0],
            shadow_opacity: 0.35,
        };

        let pixels = viewport_renderer.render_offscreen(
            &render_state.device,
            &render_state.queue,
            &frame_data,
            self.documents[self.active_document_idx].export_width.max(1),
            self.documents[self.active_document_idx].export_height.max(1),
        );

        let path = PathBuf::from(self.documents[self.active_document_idx].export_path.trim());
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            if let Err(err) = std::fs::create_dir_all(parent) {
                self.documents[self.active_document_idx].export_status =
                    format!("Export failed: {err}");
                return;
            }
        }

        match image::save_buffer(
            &path,
            &pixels,
            self.documents[self.active_document_idx].export_width.max(1),
            self.documents[self.active_document_idx].export_height.max(1),
            image::ColorType::Rgba8,
        ) {
            Ok(()) => {
                self.documents[self.active_document_idx].export_status =
                    format!("Exported {}", path.display());
            }
            Err(err) => {
                self.documents[self.active_document_idx].export_status =
                    format!("Export failed: {err}");
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // File shortcuts — consume before checking wants_keyboard_input so they
        // work even when a text field has focus.
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S)
        }) {
            self.pending_save_as = true;
        } else if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)
        }) {
            self.pending_save = true;
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)
        }) {
            self.pending_open = true;
        }

        if ctx.wants_keyboard_input() {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.set_view_preset(ViewPreset::Front);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::T)) {
            self.set_view_preset(ViewPreset::Top);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::I)) {
            self.set_view_preset(ViewPreset::Isometric);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.documents[self.active_document_idx].camera.projection =
                match self.documents[self.active_document_idx].camera.projection {
                    Projection::Perspective => Projection::Orthographic,
                    Projection::Orthographic => Projection::Perspective,
                    _ => Projection::Perspective,
                };
        }
    }

    pub(crate) fn set_view_preset(&mut self, preset: ViewPreset) {
        self.documents[self.active_document_idx].camera.orientation = preset.orientation();
        if let Some(projection) = preset.preferred_projection() {
            self.documents[self.active_document_idx].camera.projection = projection;
        }
    }

    fn do_save_active_document(&mut self) {
        let doc = &self.documents[self.active_document_idx];
        if let Some(path) = doc.path.clone() {
            match persistence::save_document_to_path(doc, &path) {
                Ok(()) => {
                    self.documents[self.active_document_idx].dirty = false;
                    self.documents[self.active_document_idx].export_status.clear();
                }
                Err(e) => {
                    self.documents[self.active_document_idx].export_status =
                        format!("Save failed: {e}");
                }
            }
        }
    }

    fn apply_preset_view_settings(&mut self, preset: PlotPreset) {
        let (mode, height, color, tile_size) = preset.ground_plane_settings();
        self.documents[self.active_document_idx].ground_plane_mode = mode;
        self.documents[self.active_document_idx].ground_plane_height = height;
        self.documents[self.active_document_idx].ground_plane_color = color;
        self.documents[self.active_document_idx].ground_plane_tile_size = tile_size;
    }

    /// Advance all playing parameter sweeps in the active document by `dt` seconds.
    /// Sets `scene_dirty` if any value changed.
    /// Returns `true` if at least one sweep is still playing (caller should request repaint).
    fn tick_parameter_sweeps(&mut self, dt: f64) -> bool {
        let doc_idx = self.active_document_idx;
        let doc = &mut self.documents[doc_idx];
        let n_plots = doc.plots.len();

        // Keep sweep_config parallel to plots.
        doc.sweep_config.resize_with(n_plots, Default::default);

        // Split the Document into two disjoint field borrows so we can mutate
        // plots[i].kind.parameters and sweep_config[i] simultaneously.
        let plots = &mut doc.plots;
        let sweep_config = &mut doc.sweep_config;

        let mut any_playing = false;
        let mut scene_needs_rebuild = false;

        for plot_idx in 0..n_plots {
            if !plots[plot_idx].visible {
                continue;
            }
            // sweep_config[plot_idx] and plots[plot_idx].kind are in different Vecs
            // (different fields of Document) → the borrows cannot alias.
            let sweep_map = &mut sweep_config[plot_idx];
            if let Some(parameters) = plots[plot_idx].kind.parameters_mut() {
                for (name, value) in parameters.iter_mut() {
                    if let Some(sweep) = sweep_map.get_mut(name) {
                        if sweep.playing {
                            any_playing = true;
                            *value = sweep.tick(dt);
                            scene_needs_rebuild = true;
                        }
                    }
                }
            }
            // NLL ends borrows of sweep_map and parameters here (loop body closes).
        }
        // NLL ends borrows of plots and sweep_config here.

        if scene_needs_rebuild {
            doc.scene_dirty = true;
        }
        any_playing
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        persistence::save_persisted_state(storage, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Intercept OS close request — prompt if any document has unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) {
            let any_dirty = self.documents.iter().any(|d| d.dirty);
            if any_dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_quit = true;
            }
        }

        // Handle pending file actions (rfd dialogs are blocking — call them
        // before UI rendering so the frame pauses cleanly during the dialog).
        if self.pending_open {
            self.pending_open = false;
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Poincaré project", &["poincare.json", "json"])
                .pick_file()
            {
                match persistence::load_document_from_path(&path) {
                    Ok(snapshot) => {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Untitled")
                            .to_string();
                        let mut doc = snapshot.into_document();
                        if doc.title.is_empty() { doc.title = stem; }
                        doc.path = Some(path);
                        doc.dirty = false;
                        self.documents.push(doc);
                        self.active_document_idx = self.documents.len() - 1;
                    }
                    Err(e) => {
                        self.documents[self.active_document_idx].export_status =
                            format!("Open failed: {e}");
                    }
                }
            }
        }

        if self.pending_save {
            self.pending_save = false;
            if self.documents[self.active_document_idx].path.is_some() {
                self.do_save_active_document();
            } else {
                self.pending_save_as = true;
            }
        }

        if self.pending_save_as {
            self.pending_save_as = false;
            let default_name = format!(
                "{}.poincare.json",
                self.documents[self.active_document_idx].title_or_untitled()
            );
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Poincaré project", &["poincare.json", "json"])
                .set_file_name(&default_name)
                .save_file()
            {
                let path = ensure_poincare_extension(path);
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string();
                self.documents[self.active_document_idx].path = Some(path);
                self.documents[self.active_document_idx].title = stem;
                self.do_save_active_document();
            }
        }

        self.handle_shortcuts(ctx);
        let settings_pressed = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::Comma)
        });
        if settings_pressed {
            self.settings_open = true;
        }
        self.top_bar(ctx);

        // Tick parameter sweeps before rebuilding the scene so the new values
        // are picked up by the same frame's rebuild.
        let dt = ctx.input(|i| i.stable_dt) as f64;
        if self.tick_parameter_sweeps(dt) {
            ctx.request_repaint();
        }

        self.rebuild_scene(frame);
        if self.pending_export {
            self.pending_export = false;
            self.export_png(frame);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(color32_from_rgba(
                self.documents[self.active_document_idx].viewport_background,
            )))
            .show(ctx, |ui| {
                self.dock_ui(ui, frame);
            });

        ui::equation_editor::show_eq_editor_window(ctx, &mut self.eq_editor);
        if self.settings_open {
            let mut open = self.settings_open;
            settings::show_settings_window(ctx, &mut open, self, frame);
            self.settings_open = open;
        }

        // Dirty-close confirmation dialog.
        if let Some(close_idx) = self.confirm_close_idx {
            let title = self.documents[close_idx].title_or_untitled().to_string();
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("\"{title}\" has unsaved changes. Close anyway?"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Close Without Saving").clicked() { confirmed = true; }
                        if ui.button("Cancel").clicked() { cancelled = true; }
                    });
                });
            if confirmed {
                self.confirm_close_idx = None;
                self.close_document(close_idx);
            } else if cancelled {
                self.confirm_close_idx = None;
            }
        }

        // Dirty-quit confirmation dialog.
        if self.confirm_quit {
            let n = self.documents.iter().filter(|d| d.dirty).count();
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Window::new("Quit With Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("{n} document(s) have unsaved changes. Quit anyway?"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Quit Without Saving").clicked() { confirmed = true; }
                        if ui.button("Cancel").clicked() { cancelled = true; }
                    });
                });
            if confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if cancelled {
                self.confirm_quit = false;
            }
        }
    }
}

fn ensure_poincare_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        let mut name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();
        name.push_str(".poincare.json");
        path.set_file_name(name);
    }
    path
}

pub(crate) fn color32_from_rgba(rgba: [f32; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (rgba[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgba[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgba[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgba[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlotPreset {
    CompositeDemo,
    SciVisSampler,
    SurfaceGallery,
    AdvancedSciVis,
    ExpressionDemo,
    InputFormatDemo,
    ClassicCurves,
    ClassicSurfaces,
    MaterialShowcase,
    FeatureShowcase,
    TransparencyDemo,
    GroundPlaneDemo,
}

impl PlotPreset {
    fn all() -> &'static [Self] {
        &[
            Self::CompositeDemo,
            Self::SciVisSampler,
            Self::SurfaceGallery,
            Self::AdvancedSciVis,
            Self::ExpressionDemo,
            Self::InputFormatDemo,
            Self::ClassicCurves,
            Self::ClassicSurfaces,
            Self::MaterialShowcase,
            Self::FeatureShowcase,
            Self::TransparencyDemo,
            Self::GroundPlaneDemo,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            Self::CompositeDemo => "Composite Demo",
            Self::SciVisSampler => "SciVis Sampler",
            Self::SurfaceGallery => "Surface Gallery",
            Self::AdvancedSciVis => "Advanced SciVis",
            Self::ExpressionDemo => "Expression Demo",
            Self::InputFormatDemo => "Input Format Demo",
            Self::ClassicCurves => "Classic Curves",
            Self::ClassicSurfaces => "Classic Surfaces",
            Self::MaterialShowcase => "Material Showcase",
            Self::FeatureShowcase => "Feature Showcase",
            Self::TransparencyDemo => "Transparency Demo",
            Self::GroundPlaneDemo => "Ground Plane Demo",
        }
    }

    fn build(self) -> Vec<PlotEntry> {
        match self {
            Self::CompositeDemo => presets::composite_demo::build(),
            Self::SciVisSampler => presets::scivis_sampler::build(),
            Self::SurfaceGallery => presets::surface_gallery::build(),
            Self::AdvancedSciVis => presets::advanced_scivis::build(),
            Self::ExpressionDemo => presets::expression_demo::build(),
            Self::InputFormatDemo => presets::input_format_demo::build(),
            Self::ClassicCurves => presets::classic_curves::build(),
            Self::ClassicSurfaces => presets::classic_surfaces::build(),
            Self::MaterialShowcase => presets::material_showcase::build(),
            Self::FeatureShowcase => presets::feature_showcase::build(),
            Self::TransparencyDemo => presets::transparency_demo::build(),
            Self::GroundPlaneDemo => presets::ground_plane_demo::build(),
        }
    }

    fn ground_plane_settings(self) -> (GroundPlaneMode, f32, [f32; 4], f32) {
        match self {
            Self::GroundPlaneDemo => (GroundPlaneMode::Tile, -1.6, [0.26, 0.28, 0.30, 1.0], 1.5),
            _ => (GroundPlaneMode::None, 0.0, [0.3, 0.3, 0.3, 1.0], 1.0),
        }
    }
}
