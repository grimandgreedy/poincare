//! Poincaré: standalone 3D graphing application.

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
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::{fs::OpenOptions, io::Write};

use eframe::egui;
use grimdock::{PanelStyle, PanelTree};
use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, CurveInterpolation, parse_curve_expr,
    parse_expr_with_vars,
};
use viewport_lib::BuiltinColourmap;
use viewport_lib::{
    CameraAnimator, CameraTarget, CameraTrack, Easing, GroundPlaneMode, OrbitCameraController,
    Projection, ViewPreset, ViewportRenderer, interpolate_camera,
};

use dock::{DockTab, build_panel_tree};
use document::{
    DEFAULT_VIEWPORT_BACKGROUND, Document, ExportFormat, FrameAttachment, FrameAttachmentKind,
    SavedCameraView, StoredFrameField, default_camera, default_export_dir, default_export_filename,
    export_mode_for_format, sample_frame_field,
};
use plot::entry::PlotEntry;
use plot::kind::{PlotKind, PlotKindExt};
use plot::selected_type::SelectedPlotType;
use plot::table::{TableImportDefinition, TablePlotTarget};
use ui::data_table::DataTableState;
use ui::equation_editor::EquationEditor;

static DEBUG_LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn default_panel_style() -> PanelStyle {
    PanelStyle {
        content_inset: 10.0,
        ..PanelStyle::default()
    }
}

fn app_icon() -> Arc<egui::IconData> {
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../../../assets/icon_macos.png");
    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../../../assets/icon.png");

    let image = image::load_from_memory(icon_bytes)
        .expect("embedded app icon png should decode")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Arc::new(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn install_app_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "fira_code_nerd_regular".to_string(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/FiraCodeNerdFont-Regular.ttf"
        ))
        .into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family.clone())
            .or_default()
            .insert(0, "fira_code_nerd_regular".to_string());
    }
    ctx.set_fonts(fonts);
}

fn debug_log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        return document::home_dir().join("Library/Logs/Poincare");
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("Poincare/logs");
        }
        return document::home_dir().join("AppData/Local/Poincare/logs");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(xdg_state) = std::env::var_os("XDG_STATE_HOME") {
            return PathBuf::from(xdg_state).join("poincare");
        }
        document::home_dir().join(".local/state/poincare")
    }
}

fn debug_log_path() -> PathBuf {
    debug_log_dir().join("poincare.log")
}

fn init_debug_logging() {
    let dir = debug_log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = debug_log_path();
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = DEBUG_LOG_FILE.set(Mutex::new(file));
    }
    debug_log(&format!(
        "===== session start pid={} log={} =====",
        std::process::id(),
        path.display()
    ));
}

fn debug_log(message: &str) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}.{:03}", d.as_secs(), d.subsec_millis()))
        .unwrap_or_else(|_| "time_error".to_string());
    let line = format!("[{timestamp}] {message}\n");
    if let Some(file) = DEBUG_LOG_FILE.get() {
        if let Ok(mut file) = file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
    eprintln!("{line}");
}

fn install_panic_logging() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        debug_log(&format!("panic at {location}: {payload}"));
        let bt = std::backtrace::Backtrace::force_capture();
        debug_log(&format!("backtrace:\n{bt}"));
    }));
}

fn main() -> eframe::Result {
    init_debug_logging();
    install_panic_logging();
    eframe::run_native(
        "Poincaré",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 780.0])
                .with_icon(app_icon()),
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
    pending_open: bool,
    pending_save: bool,
    pending_save_as: bool,
    confirm_close_idx: Option<usize>,
    confirm_delete_plot_idx: Option<usize>,
    confirm_quit: bool,
    force_quit: bool,
    orbit_controller: OrbitCameraController,
    last_axes_snap: Option<(usize, bool)>,
    last_viewport_size: [u32; 2],
    add_plot_type: SelectedPlotType,
    add_expr_fields: [String; 3],
    add_table_import: TableImportDefinition,
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
    add_plot_open: bool,
    add_plot_focus_pending: bool,
    shortcuts_open: bool,
    command_palette_open: bool,
    command_palette_focus_pending: bool,
    command_palette_query: String,
    command_palette_selected: usize,
    camera_animator: CameraAnimator,
    camera_animations_enabled: bool,
    camera_animation_duration: f32,
    camera_animation_easing: Easing,
    inspector_tab: InspectorTab,
    pending_focus_tab: Option<DockTab>,
    last_scrolled_plot_selection: Option<(usize, Option<usize>)>,
    selected_plot_eq_target: Option<(usize, usize)>,
    renaming_plot: Option<usize>,
    rename_buf: String,
    rename_needs_focus: bool,
    surface_intersection_target: Option<usize>,
    surface_intersection_tolerance: f32,
    surface_intersection_stitch_distance: f32,
    surface_intersection_make_points: bool,
    analysis_show_all: bool,
    interpolate_modal: Option<InterpolateModalState>,
    axis_derivative_modal: Option<AxisDerivativeModalState>,
    fit_curve_modal: Option<FitCurveModalState>,
    surface_normals_modal: Option<SurfaceNormalsModalState>,
    surface_curvature_modal: Option<SurfaceCurvatureModalState>,
    curve_surface_measurement_modal: Option<CurveSurfaceMeasurementModalState>,
    moving_frame_modal: Option<MovingFrameModalState>,
    data_editor_modal: Option<DataEditorModalState>,
    data_panel: Option<DataPanelState>,
    export_job: Option<ExportJob>,
}

#[derive(Clone)]
struct InterpolateModalState {
    source_plot_idx: usize,
    output_name: String,
    interpolation: CurveInterpolation,
    error: String,
}

#[derive(Clone)]
struct AxisDerivativeModalState {
    source_plot_idx: usize,
    numerator_axis: usize,
    denominator_axis: usize,
    output_name: String,
    error: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FitCurveMethodUi {
    Polynomial,
    RobustPolynomial,
    Spline,
    Fourier,
}

#[derive(Clone)]
struct FitCurveModalState {
    source_plot_idx: usize,
    method: FitCurveMethodUi,
    output_name: String,
    degree: u32,
    harmonics: u32,
    smoothing_window: u32,
    samples_per_segment: u32,
    show_control_points: bool,
    show_residual_plot: bool,
    error: String,
}

#[derive(Clone)]
struct SurfaceNormalsModalState {
    source_plot_idx: usize,
    max_samples: u32,
    vector_scale: f32,
    error: String,
}

#[derive(Clone)]
struct SurfaceCurvatureModalState {
    source_plot_idx: usize,
    quantity: SurfaceCurvatureQuantityUi,
    show_extrema: bool,
    error: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SurfaceCurvatureQuantityUi {
    Mean,
    Gaussian,
    PrincipalMax,
    PrincipalMin,
}

#[derive(Clone)]
struct CurveSurfaceMeasurementModalState {
    source_plot_idx: usize,
    target_surface_idx: Option<usize>,
    max_samples: u32,
    vector_scale: f32,
    error: String,
}

#[derive(Clone)]
struct MovingFrameModalState {
    source_plot_idx: usize,
    analysis_kind: poincare_lib::AnalysisKind,
    target_surface_idx: Option<usize>,
    max_samples: u32,
    vector_scale: f32,
    error: String,
}

#[derive(Clone)]
struct DataEditorModalState {
    doc_idx: usize,
    plot_idx: usize,
    payload: DataEditorPayload,
    original_payload: DataEditorPayload,
    confirm_close: bool,
    edit_mode: DataEditorMode,
    table_state: DataTableState,
}

#[derive(Clone)]
pub(crate) struct AnalysisPanelState {
    title: String,
    source_doc_idx: usize,
    source_plot_idx: usize,
    reports: Vec<poincare_lib::AnalysisReport>,
    tables: Vec<poincare_lib::AnalysisTable>,
    table_states: Vec<DataTableState>,
    diagnostics: Vec<poincare_lib::Diagnostic>,
    frame_fields: Vec<poincare_lib::FrameField>,
    provenance: poincare_lib::AnalysisProvenance,
}

#[derive(Clone)]
pub(crate) enum DataPanelState {
    Analysis(AnalysisPanelState),
}

#[derive(Clone)]
enum DataEditorPayload {
    ImportedTable(TableImportDefinition),
    PointAnnotations {
        raw_text: String,
        show_labels: bool,
        error: Option<String>,
    },
    ArrowAnnotations {
        raw_text: String,
        show_labels: bool,
        error: Option<String>,
    },
    DerivedPolylineGroups {
        raw_text: String,
        error: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataEditorMode {
    Raw,
    Cells,
}

struct ExportJob {
    doc_idx: usize,
    stage: ExportJobStage,
}

enum ExportJobStage {
    RenderingFrames {
        track: CameraTrack,
        output_path: PathBuf,
        temp_dir: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
        format: ExportFormat,
        frame_count: u32,
        next_frame: u32,
        first_projection: Projection,
        first_fov: f32,
    },
    Encoding {
        child: Child,
        output_path: PathBuf,
        temp_dir: PathBuf,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorTab {
    Domain,
    Style,
    Surface,
    Analysis,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameraCommand {
    ViewPreset(ViewPreset),
    FrameAll,
    FrameSelected,
    ResetView,
    SetProjection(Projection),
    ToggleProjection,
    SaveSlot(usize),
    RecallSlot(usize),
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_app_fonts(&cc.egui_ctx);
        let mut app = Self {
            documents: vec![Document::new_default()],
            active_document_idx: 0,
            pending_open: false,
            pending_save: false,
            pending_save_as: false,
            confirm_close_idx: None,
            confirm_delete_plot_idx: None,
            confirm_quit: false,
            force_quit: false,
            orbit_controller: OrbitCameraController::viewport_primitives(),
            last_axes_snap: None,
            last_viewport_size: [1000, 700],
            add_plot_type: SelectedPlotType::Auto,
            add_expr_fields: [String::new(), String::new(), String::new()],
            add_table_import: TableImportDefinition::empty(TablePlotTarget::Scatter),
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
            add_plot_open: false,
            add_plot_focus_pending: false,
            shortcuts_open: false,
            command_palette_open: false,
            command_palette_focus_pending: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            camera_animator: CameraAnimator::with_default_damping(),
            camera_animations_enabled: true,
            camera_animation_duration: 0.6,
            camera_animation_easing: Easing::EaseInOutCubic,
            inspector_tab: InspectorTab::Domain,
            pending_focus_tab: None,
            last_scrolled_plot_selection: None,
            selected_plot_eq_target: None,
            renaming_plot: None,
            rename_buf: String::new(),
            rename_needs_focus: false,
            surface_intersection_target: None,
            surface_intersection_tolerance: 0.01,
            surface_intersection_stitch_distance: 0.05,
            surface_intersection_make_points: true,
            analysis_show_all: false,
            interpolate_modal: None,
            axis_derivative_modal: None,
            fit_curve_modal: None,
            surface_normals_modal: None,
            surface_curvature_modal: None,
            curve_surface_measurement_modal: None,
            moving_frame_modal: None,
            data_editor_modal: None,
            data_panel: None,
            export_job: None,
        };
        persistence::load_persisted_state(cc.storage, &mut app);
        for doc in &mut app.documents {
            doc.initialize_history();
        }
        app
    }

    pub(crate) fn new_document(&mut self) {
        let mut doc = Document::new_default();
        doc.initialize_history();
        self.documents.push(doc);
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

    fn open_selected_plot_editor(&mut self) {
        let doc_idx = self.active_document_idx;
        let Some(plot_idx) = self.documents[doc_idx].selected_plot else {
            return;
        };
        let Some(plot) = self.documents[doc_idx].plots.get(plot_idx) else {
            return;
        };
        match &plot.kind {
            PlotKind::ImportedTable { .. }
            | PlotKind::PointAnnotations { .. }
            | PlotKind::ArrowAnnotations { .. }
            | PlotKind::DerivedPolylineGroups { .. } => {
                if let Some(payload) = data_editor_payload_from_plot_kind(&plot.kind) {
                    self.data_editor_modal = Some(DataEditorModalState {
                        doc_idx,
                        plot_idx,
                        payload: payload.clone(),
                        original_payload: payload,
                        confirm_close: false,
                        edit_mode: DataEditorMode::Cells,
                        table_state: DataTableState::default(),
                    });
                }
            }
            kind if is_equation_editable(kind) => {
                self.selected_plot_eq_target = Some((doc_idx, plot_idx));
                self.eq_editor.open = true;
                self.eq_editor.target_id = None;
                self.eq_editor.edit_buf = selected_plot_equation_text(kind);
                self.eq_editor.original_buf = self.eq_editor.edit_buf.clone();
                self.eq_editor.confirm_close = false;
                self.eq_editor.focus_input = true;
                self.eq_editor.show_auto_templates = false;
            }
            _ => {}
        }
    }

    fn apply_selected_plot_equation_edit(&mut self, text: String) {
        let Some((doc_idx, plot_idx)) = self.selected_plot_eq_target.take() else {
            return;
        };
        let Some(doc) = self.documents.get_mut(doc_idx) else {
            return;
        };
        let Some(plot) = doc.plots.get_mut(plot_idx) else {
            return;
        };
        if apply_expression_edit(&mut plot.kind, &text) {
            doc.mark_dirty();
        }
    }

    pub(crate) fn open_data_panel(&mut self, panel: DataPanelState) {
        self.data_panel = Some(panel);
        if let Some(tree) = self.panel_tree.as_mut() {
            tree.ensure_tab_in_leaf(6, dock::tab("Data", DockTab::DataPanel));
        }
        self.pending_focus_tab = Some(DockTab::DataPanel);
    }

    pub(crate) fn set_selected_plot(&mut self, doc_idx: usize, selected_plot: Option<usize>) {
        if let Some(DataPanelState::Analysis(state)) = &self.data_panel
            && (state.source_doc_idx != doc_idx || selected_plot != Some(state.source_plot_idx))
        {
            self.data_panel = None;
            self.pending_focus_tab = Some(DockTab::PlotProperties);
        }
        self.documents[doc_idx].selected_plot = selected_plot;
        let selected_plot_id = selected_plot
            .and_then(|index| self.documents[doc_idx].plots.get(index))
            .map(|plot| plot.plot_id);
        if let Some(plot_id) = selected_plot_id {
            let matching_frame = self.documents[doc_idx]
                .frame_fields
                .iter()
                .find(|field| field.source_plot_ids.contains(&plot_id))
                .map(|field| field.id);
            self.documents[doc_idx].frame_playback.selected_frame_field = matching_frame;
            self.documents[doc_idx].frame_playback.phase = 0.0;
            self.documents[doc_idx].frame_playback.playing = false;
        } else {
            self.documents[doc_idx].frame_playback.selected_frame_field = None;
            self.documents[doc_idx].frame_playback.playing = false;
        }
    }

    pub(crate) fn append_plot_entry(&mut self, doc_idx: usize, entry: PlotEntry) -> usize {
        let entry = self.documents[doc_idx].prepare_plot_entry(entry);
        self.documents[doc_idx].plots.push(entry);
        self.documents[doc_idx].plots.len() - 1
    }

    pub(crate) fn store_frame_fields(
        &mut self,
        doc_idx: usize,
        source_plot_ids: Vec<u64>,
        source_plot_names: Vec<String>,
        frame_fields: Vec<poincare_lib::FrameField>,
    ) {
        let doc = &mut self.documents[doc_idx];
        let mut last_id = None;
        for field in frame_fields {
            let stored = StoredFrameField {
                id: doc.next_frame_field_id(),
                title: field.title,
                source_plot_ids: source_plot_ids.clone(),
                source_plot_names: source_plot_names.clone(),
                frame_kind: field.frame_kind,
                samples: field.samples,
            };
            last_id = Some(stored.id);
            doc.frame_fields.push(stored);
        }
        if let Some(id) = last_id {
            doc.frame_playback.selected_frame_field = Some(id);
            doc.frame_playback.phase = 0.0;
            if !doc
                .frame_attachments
                .iter()
                .any(|attachment| attachment.frame_field_id == id)
            {
                doc.frame_attachments.extend([
                    FrameAttachment {
                        name: "Marker".to_string(),
                        frame_field_id: id,
                        kind: FrameAttachmentKind::Marker,
                        enabled: false,
                        scale: 1.0,
                        camera_distance: 3.0,
                    },
                    FrameAttachment {
                        name: "Triad".to_string(),
                        frame_field_id: id,
                        kind: FrameAttachmentKind::Triad,
                        enabled: true,
                        scale: 1.0,
                        camera_distance: 3.0,
                    },
                    FrameAttachment {
                        name: "Camera".to_string(),
                        frame_field_id: id,
                        kind: FrameAttachmentKind::Camera,
                        enabled: false,
                        scale: 1.0,
                        camera_distance: 3.0,
                    },
                    FrameAttachment {
                        name: "Profile Ring".to_string(),
                        frame_field_id: id,
                        kind: FrameAttachmentKind::ProfileRing,
                        enabled: false,
                        scale: 0.35,
                        camera_distance: 3.0,
                    },
                ]);
            }
        }
    }

    fn apply_frame_playback(&mut self, dt: f32) {
        let doc = &mut self.documents[self.active_document_idx];
        if !doc.frame_playback.playing {
            return;
        }
        doc.frame_playback.phase += dt * doc.frame_playback.speed.max(0.01);
        if doc.frame_playback.phase >= 1.0 {
            doc.frame_playback.phase = 1.0;
            doc.frame_playback.playing = false;
        }
    }

    fn apply_frame_camera_attachment(&mut self) {
        let doc = &mut self.documents[self.active_document_idx];
        let Some(field) = doc.active_frame_field().cloned() else {
            return;
        };
        let Some(sample) = sample_frame_field(&field, doc.frame_playback.phase).cloned() else {
            return;
        };
        let Some(attachment) = doc
            .frame_attachments
            .iter()
            .find(|attachment| {
                attachment.enabled
                    && attachment.frame_field_id == field.id
                    && attachment.kind == FrameAttachmentKind::Camera
            })
            .cloned()
        else {
            return;
        };
        doc.camera
            .set_center(glam::Vec3::from_array(sample.position));
        let _ = attachment;
    }

    pub(crate) fn inject_frame_attachments(
        &self,
        doc_idx: usize,
        frame_data: &mut viewport_lib::FrameData,
    ) {
        let doc = &self.documents[doc_idx];
        let Some(field) = doc.active_frame_field() else {
            return;
        };
        let Some(sample) = sample_frame_field(field, doc.frame_playback.phase) else {
            return;
        };
        let position = sample.position;
        let tangent = glam::Vec3::from_array(sample.tangent);
        let normal = glam::Vec3::from_array(sample.normal);
        let binormal = glam::Vec3::from_array(sample.binormal);

        for attachment in doc
            .frame_attachments
            .iter()
            .filter(|attachment| attachment.enabled && attachment.frame_field_id == field.id)
        {
            match attachment.kind {
                FrameAttachmentKind::Marker => {
                    let mut points = viewport_lib::PointCloudItem::default();
                    points.positions.push(position);
                    points.point_size = 12.0 * attachment.scale.max(0.2);
                    points.default_colour = [1.0, 0.82, 0.2, 1.0];
                    points.settings.pick_id = viewport_lib::PickId::NONE;
                    frame_data.scene.point_clouds.push(points);
                }
                FrameAttachmentKind::Triad => {
                    for (vector, colour) in [
                        (
                            (tangent * attachment.scale).to_array(),
                            [1.0, 0.2, 0.2, 1.0],
                        ),
                        (
                            (normal * attachment.scale).to_array(),
                            [0.2, 0.95, 0.25, 1.0],
                        ),
                        (
                            (binormal * attachment.scale).to_array(),
                            [0.25, 0.45, 1.0, 1.0],
                        ),
                    ] {
                        let mut glyphs = viewport_lib::GlyphItem::default();
                        glyphs.positions = vec![position];
                        glyphs.vectors = vec![
                            glam::Vec3::from_array(vector)
                                .normalize_or_zero()
                                .to_array(),
                        ];
                        glyphs.scalars = vec![0.0];
                        glyphs.use_default_colour = true;
                        glyphs.default_colour = colour;
                        glyphs.scale = attachment.scale.max(0.01);
                        glyphs.scale_by_magnitude = false;
                        glyphs.settings.pick_id = viewport_lib::PickId::NONE;
                        frame_data.scene.glyphs.push(glyphs);
                    }
                }
                FrameAttachmentKind::ProfileRing => {
                    let mut polyline = viewport_lib::PolylineItem::default();
                    let radius = attachment.scale.max(0.05);
                    let segments = 24usize;
                    for index in 0..=segments {
                        let theta = index as f32 / segments as f32 * std::f32::consts::TAU;
                        let offset =
                            normal * (theta.cos() * radius) + binormal * (theta.sin() * radius);
                        polyline
                            .positions
                            .push((glam::Vec3::from_array(position) + offset).to_array());
                    }
                    polyline.strip_lengths.push((segments + 1) as u32);
                    polyline.default_colour = [0.35, 0.9, 1.0, 1.0];
                    polyline.line_width = 2.0;
                    polyline.settings.pick_id = viewport_lib::PickId::NONE;
                    frame_data.scene.polylines.push(polyline);
                }
                FrameAttachmentKind::Camera => {}
            }
        }
    }

    pub(crate) fn insert_plot_entry(
        &mut self,
        doc_idx: usize,
        index: usize,
        entry: PlotEntry,
    ) -> usize {
        let entry = self.documents[doc_idx].prepare_plot_entry(entry);
        let insert_idx = index.min(self.documents[doc_idx].plots.len());
        self.documents[doc_idx].plots.insert(insert_idx, entry);
        insert_idx
    }

    pub(crate) fn replace_document_plots(&mut self, doc_idx: usize, plots: Vec<PlotEntry>) {
        self.documents[doc_idx].plots = plots;
        self.documents[doc_idx].normalize_plot_hierarchy();
    }

    fn show_data_editor_modal(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.data_editor_modal.clone() else {
            return;
        };

        let title = self
            .documents
            .get(state.doc_idx)
            .and_then(|doc| doc.plots.get(state.plot_idx))
            .map(|plot| format!("Data Editor: {}", plot.name))
            .unwrap_or_else(|| "Data Editor".to_string());

        let mut open = true;
        let mut save = false;
        let mut close_requested = false;
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(720.0)
            .default_height(640.0)
            .show(ctx, |ui| {
                let mut valid = false;
                ui.horizontal(|ui| {
                    ui.label("Mode");
                    ui.selectable_value(&mut state.edit_mode, DataEditorMode::Raw, "Raw");
                    ui.selectable_value(&mut state.edit_mode, DataEditorMode::Cells, "Cells");
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    valid = edit_data_payload(
                        ui,
                        &mut state.payload,
                        state.edit_mode,
                        &mut state.table_state,
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                    if ui.add_enabled(valid, egui::Button::new("Apply")).clicked() {
                        save = true;
                        close_requested = true;
                    }
                });
            });
        if escape_pressed && !state.confirm_close {
            close_requested = true;
        }
        if close_requested {
            if data_editor_payload_is_dirty(&state.payload, &state.original_payload) {
                state.confirm_close = true;
                open = true;
            } else {
                open = false;
            }
        }

        if save
            && let Some(doc) = self.documents.get_mut(state.doc_idx)
            && let Some(plot) = doc.plots.get_mut(state.plot_idx)
            && apply_data_editor_payload(&mut plot.kind, &state.payload)
        {
            doc.mark_dirty();
            state.confirm_close = false;
            open = false;
        }

        if state.confirm_close {
            let mut discard = false;
            let mut save_from_prompt = false;
            let mut cancel = false;
            egui::Window::new("Discard changes?")
                .id(egui::Id::new("data_editor_discard_confirm"))
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Discard unsaved changes?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let discard_button =
                            ui.add_sized([90.0, 30.0], egui::Button::new("discard"));
                        discard_button.request_focus();
                        if discard_button.clicked() || enter_pressed {
                            discard = true;
                        }
                        if ui
                            .add_sized([90.0, 30.0], egui::Button::new("save"))
                            .clicked()
                        {
                            save_from_prompt = true;
                        }
                        if ui
                            .add_sized([90.0, 30.0], egui::Button::new("cancel"))
                            .clicked()
                            || escape_pressed
                        {
                            cancel = true;
                        }
                    });
                });
            if discard {
                state.payload = state.original_payload.clone();
                state.confirm_close = false;
                open = false;
            } else if save_from_prompt {
                if let Some(doc) = self.documents.get_mut(state.doc_idx)
                    && let Some(plot) = doc.plots.get_mut(state.plot_idx)
                    && apply_data_editor_payload(&mut plot.kind, &state.payload)
                {
                    doc.mark_dirty();
                    state.confirm_close = false;
                    open = false;
                }
            } else if cancel {
                state.confirm_close = false;
                open = true;
            }
        }

        self.data_editor_modal = open.then_some(state);
    }

    pub(crate) fn open_analysis_results_panel(
        &mut self,
        title: String,
        source_doc_idx: usize,
        source_plot_idx: usize,
        reports: Vec<poincare_lib::AnalysisReport>,
        tables: Vec<poincare_lib::AnalysisTable>,
        diagnostics: Vec<poincare_lib::Diagnostic>,
        frame_fields: Vec<poincare_lib::FrameField>,
        provenance: poincare_lib::AnalysisProvenance,
    ) {
        let table_states = (0..tables.len())
            .map(|_| DataTableState::default())
            .collect();
        self.open_data_panel(DataPanelState::Analysis(AnalysisPanelState {
            title,
            source_doc_idx,
            source_plot_idx,
            reports,
            tables,
            table_states,
            diagnostics,
            frame_fields,
            provenance,
        }));
    }

    pub(crate) fn open_stored_frame_field_panel(&mut self, doc_idx: usize, frame_field_id: u64) {
        let Some(field) = self.documents[doc_idx]
            .frame_fields
            .iter()
            .find(|field| field.id == frame_field_id)
            .cloned()
        else {
            return;
        };
        let report = poincare_lib::AnalysisReport {
            title: field.title.clone(),
            values: vec![
                ("Frame Kind".to_string(), format!("{:?}", field.frame_kind)),
                ("Sample Count".to_string(), field.samples.len().to_string()),
            ],
        };
        let table = poincare_lib::AnalysisTable {
            title: format!("{} Samples", field.title),
            columns: vec![
                "row".to_string(),
                "s".to_string(),
                "x".to_string(),
                "y".to_string(),
                "z".to_string(),
                "tx".to_string(),
                "ty".to_string(),
                "tz".to_string(),
                "nx".to_string(),
                "ny".to_string(),
                "nz".to_string(),
                "bx".to_string(),
                "by".to_string(),
                "bz".to_string(),
            ],
            rows: field
                .samples
                .iter()
                .enumerate()
                .map(|(index, sample)| {
                    vec![
                        (index + 1).to_string(),
                        format!("{:.5}", sample.parameter),
                        format!("{:.5}", sample.position[0]),
                        format!("{:.5}", sample.position[1]),
                        format!("{:.5}", sample.position[2]),
                        format!("{:.5}", sample.tangent[0]),
                        format!("{:.5}", sample.tangent[1]),
                        format!("{:.5}", sample.tangent[2]),
                        format!("{:.5}", sample.normal[0]),
                        format!("{:.5}", sample.normal[1]),
                        format!("{:.5}", sample.normal[2]),
                        format!("{:.5}", sample.binormal[0]),
                        format!("{:.5}", sample.binormal[1]),
                        format!("{:.5}", sample.binormal[2]),
                    ]
                })
                .collect(),
        };
        self.open_analysis_results_panel(
            field.title.clone(),
            doc_idx,
            self.documents[doc_idx].selected_plot.unwrap_or_default(),
            vec![report],
            vec![table],
            Vec::new(),
            vec![poincare_lib::FrameField {
                title: field.title.clone(),
                source_plot: field.source_plot_names.join(", "),
                frame_kind: field.frame_kind,
                samples: field.samples.clone(),
            }],
            poincare_lib::AnalysisProvenance {
                kind: field.frame_kind,
                source_plots: field.source_plot_names,
                parameters: Vec::new(),
                notes: vec!["Loaded from the persisted document frame-field store.".to_string()],
            },
        );
    }

    pub(crate) fn data_panel_ui(&mut self, ui: &mut egui::Ui) {
        let Some(panel) = self.data_panel.take() else {
            ui.label(egui::RichText::new("No active data view.").weak());
            ui.label(
                egui::RichText::new(
                    "Open a plot data editor or run an analysis that produces reports or tables.",
                )
                .small()
                .weak(),
            );
            return;
        };

        match panel {
            DataPanelState::Analysis(mut state) => {
                let mut keep_open = true;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&state.title).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            keep_open = false;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Sources: {}",
                        state.provenance.source_plots.join(", ")
                    ))
                    .small()
                    .weak(),
                );
                if !state.provenance.parameters.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            state
                                .provenance
                                .parameters
                                .iter()
                                .map(|(k, v)| format!("{k}={v}"))
                                .collect::<Vec<_>>()
                                .join(", "),
                        )
                        .small()
                        .weak(),
                    );
                }
                if !state.frame_fields.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "Frame fields: {} sampled set(s)",
                            state.frame_fields.len()
                        ))
                        .small()
                        .weak(),
                    );
                }
                for report in &state.reports {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&report.title).strong());
                    egui::Grid::new(ui.id().with(&report.title))
                        .striped(true)
                        .show(ui, |ui| {
                            for (label, value) in &report.values {
                                ui.label(label);
                                ui.monospace(value);
                                ui.end_row();
                            }
                        });
                }
                if !state.diagnostics.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Diagnostics").strong());
                    for diagnostic in &state.diagnostics {
                        ui.label(diagnostic.to_string());
                    }
                }
                for (index, table) in state.tables.iter().enumerate() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&table.title).strong());
                    let mut rows = table.rows.clone();
                    let headers = table.columns.clone();
                    let table_id = format!("analysis_table_{index}");
                    ui::data_table::show_data_table(
                        ui,
                        state
                            .table_states
                            .get_mut(index)
                            .expect("table state per analysis table"),
                        &headers,
                        &mut rows,
                        ui::data_table::DataTableOptions::readonly(&table_id),
                    );
                }

                if keep_open {
                    self.data_panel = Some(DataPanelState::Analysis(state));
                }
            }
        }
    }

    pub(crate) fn switch_document(&mut self, idx: usize) {
        if idx < self.documents.len() {
            self.active_document_idx = idx;
        }
    }

    pub(crate) fn load_preset(&mut self, preset: PlotPreset) {
        self.record_undo_point();
        self.replace_document_plots(self.active_document_idx, preset.build());
        self.documents[self.active_document_idx].sweep_config = Vec::new();
        let selected = (!self.documents[self.active_document_idx].plots.is_empty()).then_some(0);
        self.set_selected_plot(self.active_document_idx, selected);
        self.documents[self.active_document_idx].viewport_selection_hidden_for = None;
        self.apply_preset_view_settings(preset);
        self.documents[self.active_document_idx].scene_dirty = true;
        self.documents[self.active_document_idx]
            .export_status
            .clear();
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.record_undo_point();
        self.documents[self.active_document_idx].mark_dirty();
    }

    pub(crate) fn mark_non_scene_dirty(&mut self) {
        self.record_undo_point();
        self.documents[self.active_document_idx].mark_modified();
    }

    pub(crate) fn record_undo_point(&mut self) {
        self.documents[self.active_document_idx].record_undo_point();
    }

    pub(crate) fn finalize_undo_point(&mut self) {
        self.documents[self.active_document_idx].finalize_history_point();
    }

    pub(crate) fn undo_active_document(&mut self) {
        let doc = &mut self.documents[self.active_document_idx];
        doc.finalize_history_point();
        let _ = doc.undo();
    }

    pub(crate) fn redo_active_document(&mut self) {
        let doc = &mut self.documents[self.active_document_idx];
        doc.finalize_history_point();
        let _ = doc.redo();
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
        let Some(scene_result) = self.documents[self.active_document_idx].build_scene_data() else {
            return;
        };
        let mut scene = match scene_result {
            Ok(scene) => scene,
            Err(err) => {
                self.documents[self.active_document_idx].export_status =
                    format!("Scene rebuild failed: {err}");
                return;
            }
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
        self.documents[self.active_document_idx]
            .scene
            .release_gpu_resources(viewport_renderer.resources_mut());

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
        self.documents[self.active_document_idx].export_progress = None;
        let mut export_camera = self.documents[self.active_document_idx].camera.clone();
        export_camera.set_aspect_ratio(
            self.documents[self.active_document_idx].export_width.max(1) as f32,
            self.documents[self.active_document_idx]
                .export_height
                .max(1) as f32,
        );
        let pixels = match self.render_export_pixels(
            frame,
            &export_camera,
            self.documents[self.active_document_idx].export_width.max(1),
            self.documents[self.active_document_idx]
                .export_height
                .max(1),
        ) {
            Ok(pixels) => pixels,
            Err(err) => {
                self.documents[self.active_document_idx].export_status = err;
                return;
            }
        };

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
            self.documents[self.active_document_idx]
                .export_height
                .max(1),
            image::ColorType::Rgba8,
        ) {
            Ok(()) => {
                self.documents[self.active_document_idx].export_status =
                    format!("Exported {}", path.display());
                debug_log(&format!("png export complete path={}", path.display()));
            }
            Err(err) => {
                self.documents[self.active_document_idx].export_status =
                    format!("Export failed: {err}");
                debug_log(&format!("png export failed: {err}"));
            }
        }
    }

    pub(crate) fn export_animation(&mut self, frame: &mut eframe::Frame) {
        let _ = frame;
        let doc_idx = self.active_document_idx;
        let track = self.build_saved_view_track();
        if track.len() < 2 {
            self.documents[doc_idx].export_status =
                "Animated export requires at least two saved views.".to_string();
            self.documents[doc_idx].export_progress = None;
            debug_log("animated export aborted: fewer than two saved views");
            return;
        }

        let width = self.documents[doc_idx].export_width.max(1);
        let height = self.documents[doc_idx].export_height.max(1);
        let fps = self.documents[doc_idx].export_fps.max(1);
        let format = self.documents[doc_idx].export_format;
        let duration = track.duration().max(0.0);
        let frame_count = ((duration * fps as f64).ceil() as u32).max(2);
        let output_path =
            normalized_export_path(self.documents[doc_idx].export_path.trim(), format);
        self.documents[doc_idx].export_path = output_path.to_string_lossy().into_owned();

        let temp_dir = std::env::temp_dir().join(format!(
            "poincare-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        if let Err(err) = std::fs::create_dir_all(&temp_dir) {
            self.documents[doc_idx].export_status = format!("Export failed: {err}");
            self.documents[doc_idx].export_progress = None;
            return;
        }

        let first_projection = self.documents[doc_idx]
            .saved_views
            .first()
            .map(|view| view.camera.projection)
            .unwrap_or(self.documents[doc_idx].camera.projection);
        let first_fov = self.documents[doc_idx]
            .saved_views
            .first()
            .map(|view| view.camera.fov_y)
            .unwrap_or(self.documents[doc_idx].camera.fov_y);

        self.documents[doc_idx].export_status =
            format!("Rendering animation frames... 0/{frame_count}");
        self.documents[doc_idx].export_progress = Some(0.0);
        self.export_job = Some(ExportJob {
            doc_idx,
            stage: ExportJobStage::RenderingFrames {
                track,
                output_path,
                temp_dir,
                width,
                height,
                fps,
                format,
                frame_count,
                next_frame: 0,
                first_projection,
                first_fov,
            },
        });
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.confirm_delete_plot_idx.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                self.confirm_delete_selected_plot();
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.confirm_delete_plot_idx = None;
            }
            return;
        }

        // File shortcuts: consume before checking wants_keyboard_input so they
        // work even when a text field has focus.
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            )
        }) {
            self.pending_save_as = true;
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.pending_save = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.pending_open = true;
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            ) || i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
        }) {
            self.redo_active_document();
        } else if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)
        }) {
            self.undo_active_document();
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::K)
        }) {
            self.command_palette_open = true;
            self.command_palette_focus_pending = true;
            self.command_palette_selected = 0;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::A)) {
            self.open_add_plot_modal();
        }

        if ctx.wants_keyboard_input() {
            return;
        }

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Questionmark)) {
            self.shortcuts_open = true;
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::X)) {
            self.request_delete_selected_plot();
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::V)) {
            let doc = &mut self.documents[self.active_document_idx];
            if let Some(plot_idx) = doc.selected_plot {
                if let Some(plot) = doc.plots.get_mut(plot_idx) {
                    plot.visible = !plot.visible;
                    doc.mark_dirty();
                }
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::J)) {
            let doc = &self.documents[self.active_document_idx];
            let display_order = panels::left_panel::plot_display_rows(&doc.plots)
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if !display_order.is_empty() {
                let next = doc
                    .selected_plot
                    .and_then(|selected| {
                        display_order
                            .iter()
                            .position(|index| *index == selected)
                            .map(|position| display_order[(position + 1) % display_order.len()])
                    })
                    .unwrap_or(display_order[0]);
                self.set_selected_plot(self.active_document_idx, Some(next));
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::K)) {
            let doc = &self.documents[self.active_document_idx];
            let display_order = panels::left_panel::plot_display_rows(&doc.plots)
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if !display_order.is_empty() {
                let next = doc
                    .selected_plot
                    .and_then(|selected| {
                        display_order
                            .iter()
                            .position(|index| *index == selected)
                            .map(|position| {
                                display_order
                                    [(position + display_order.len() - 1) % display_order.len()]
                            })
                    })
                    .unwrap_or_else(|| *display_order.last().expect("display order is non-empty"));
                self.set_selected_plot(self.active_document_idx, Some(next));
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::G)) {
            let doc = &self.documents[self.active_document_idx];
            let display_order = panels::left_panel::plot_display_rows(&doc.plots);
            if let Some((last, _)) = display_order.last() {
                self.set_selected_plot(self.active_document_idx, Some(*last));
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::G)) {
            let doc = &self.documents[self.active_document_idx];
            let display_order = panels::left_panel::plot_display_rows(&doc.plots);
            if let Some((first, _)) = display_order.first() {
                self.set_selected_plot(self.active_document_idx, Some(*first));
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::E)) {
            self.open_selected_plot_editor();
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.run_camera_command(CameraCommand::ViewPreset(ViewPreset::Front));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::T)) {
            self.run_camera_command(CameraCommand::ViewPreset(ViewPreset::Top));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::I)) {
            self.run_camera_command(CameraCommand::ViewPreset(ViewPreset::Isometric));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.run_camera_command(CameraCommand::ToggleProjection);
        }
    }

    pub(crate) fn run_camera_command(&mut self, command: CameraCommand) {
        match command {
            CameraCommand::ViewPreset(preset) => {
                let camera = &self.documents[self.active_document_idx].camera;
                self.apply_camera_view(
                    camera.center,
                    camera.distance,
                    preset.orientation(),
                    preset.preferred_projection(),
                );
            }
            CameraCommand::FrameAll => {
                if let Some(target) = self.documents[self.active_document_idx]
                    .visible_scene_bounds()
                    .map(|aabb| {
                        self.documents[self.active_document_idx]
                            .camera
                            .fit_aabb_target(&aabb)
                    })
                {
                    self.apply_camera_target(target, None);
                }
            }
            CameraCommand::FrameSelected => {
                let target = self.documents[self.active_document_idx]
                    .selected_plot_bounds()
                    .or_else(|| self.documents[self.active_document_idx].visible_scene_bounds())
                    .map(|aabb| {
                        self.documents[self.active_document_idx]
                            .camera
                            .fit_aabb_target(&aabb)
                    });
                if let Some(target) = target {
                    self.apply_camera_target(target, None);
                }
            }
            CameraCommand::ResetView => {
                let default = default_camera();
                self.documents[self.active_document_idx].camera.fov_y = default.fov_y;
                self.apply_camera_view(
                    default.center,
                    default.distance,
                    default.orientation,
                    Some(default.projection),
                );
            }
            CameraCommand::SetProjection(projection) => {
                self.camera_animator.cancel_flight();
                self.documents[self.active_document_idx].camera.projection = projection;
            }
            CameraCommand::ToggleProjection => {
                let projection = match self.documents[self.active_document_idx].camera.projection {
                    Projection::Perspective => Projection::Orthographic,
                    Projection::Orthographic => Projection::Perspective,
                    _ => Projection::Perspective,
                };
                self.run_camera_command(CameraCommand::SetProjection(projection));
            }
            CameraCommand::SaveSlot(slot) => {
                let saved = self.documents[self.active_document_idx].camera.clone();
                if let Some(target) = self.documents[self.active_document_idx]
                    .saved_views
                    .get_mut(slot)
                {
                    target.camera = saved;
                    self.documents[self.active_document_idx].mark_modified();
                }
            }
            CameraCommand::RecallSlot(slot) => {
                if let Some(saved) = self.documents[self.active_document_idx]
                    .saved_views
                    .get(slot)
                    .map(|view| view.camera.clone())
                {
                    self.documents[self.active_document_idx].camera.fov_y = saved.fov_y;
                    self.apply_camera_view(
                        saved.center,
                        saved.distance,
                        saved.orientation,
                        Some(saved.projection),
                    );
                }
            }
        }
    }

    pub(crate) fn set_view_preset(&mut self, preset: ViewPreset) {
        self.run_camera_command(CameraCommand::ViewPreset(preset));
    }

    pub(crate) fn cancel_camera_animation(&mut self) {
        self.camera_animator.cancel_flight();
    }

    fn apply_camera_target(
        &mut self,
        target: viewport_lib::CameraTarget,
        projection: Option<Projection>,
    ) {
        self.apply_camera_view(
            target.center,
            target.distance,
            target.orientation,
            projection,
        );
    }

    fn apply_camera_view(
        &mut self,
        center: glam::Vec3,
        distance: f32,
        orientation: glam::Quat,
        projection: Option<Projection>,
    ) {
        let camera = &mut self.documents[self.active_document_idx].camera;
        if self.camera_animations_enabled {
            self.camera_animator.fly_to_full(
                camera,
                center,
                distance,
                orientation,
                projection,
                self.camera_animation_duration,
                self.camera_animation_easing,
            );
        } else {
            self.camera_animator.cancel_flight();
            camera.set_center(center);
            camera.set_distance(distance);
            camera.set_orientation(orientation);
            if let Some(projection) = projection {
                camera.projection = projection;
            }
        }
    }

    pub(crate) fn add_saved_view(&mut self) {
        let next_index = self.documents[self.active_document_idx].saved_views.len() + 1;
        let camera = self.documents[self.active_document_idx].camera.clone();
        self.documents[self.active_document_idx]
            .saved_views
            .push(SavedCameraView {
                name: format!("View {next_index}"),
                camera,
            });
    }

    pub(crate) fn build_saved_view_track(&self) -> CameraTrack {
        let doc = &self.documents[self.active_document_idx];
        let segment = doc.camera_track_segment_duration.max(0.1) as f64;
        let mut track = CameraTrack::new();
        for (idx, view) in doc.saved_views.iter().enumerate() {
            track.push(
                idx as f64 * segment,
                CameraTarget {
                    center: view.camera.center,
                    distance: view.camera.distance,
                    orientation: view.camera.orientation,
                },
            );
        }
        track
    }

    fn apply_saved_view_track_sample(&mut self, t: f64) {
        let track = self.build_saved_view_track();
        if track.is_empty() {
            return;
        }
        let target = interpolate_camera(&track, t);
        self.cancel_camera_animation();
        let camera = &mut self.documents[self.active_document_idx].camera;
        camera.set_center(target.center);
        camera.set_distance(target.distance);
        camera.set_orientation(target.orientation);
    }

    fn render_export_pixels(
        &mut self,
        frame: &mut eframe::Frame,
        export_camera: &viewport_lib::Camera,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        let Some(render_state) = frame.wgpu_render_state() else {
            return Err("Export failed: no wgpu render state".to_string());
        };

        let mut renderer_guard = render_state.renderer.write();
        let Some(viewport_renderer) = renderer_guard
            .callback_resources
            .get_mut::<ViewportRenderer>()
        else {
            return Err("Export failed: viewport renderer missing".to_string());
        };

        let mut frame_data = self.documents[self.active_document_idx]
            .scene
            .build_frame(export_camera);
        frame_data.camera.viewport_size = [width as f32, height as f32];
        frame_data.viewport.show_grid = false;
        frame_data.viewport.background_colour =
            Some(self.documents[self.active_document_idx].viewport_background);
        frame_data.effects.ground_plane = viewport_lib::GroundPlane {
            mode: self.documents[self.active_document_idx].ground_plane_mode,
            height: self.documents[self.active_document_idx].ground_plane_height,
            colour: self.documents[self.active_document_idx].ground_plane_color,
            tile_colour2: [
                self.documents[self.active_document_idx].ground_plane_color[0] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[1] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[2] * 0.82,
                self.documents[self.active_document_idx].ground_plane_color[3],
            ],
            tile_size: self.documents[self.active_document_idx].ground_plane_tile_size,
            shadow_colour: [0.0, 0.0, 0.0, 1.0],
            shadow_opacity: 0.35,
        };
        self.inject_frame_attachments(self.active_document_idx, &mut frame_data);

        Ok(viewport_renderer.render_offscreen(
            &render_state.device,
            &render_state.queue,
            &frame_data,
            width,
            height,
        ))
    }

    fn spawn_ffmpeg_export(
        &self,
        temp_dir: &std::path::Path,
        output_path: &std::path::Path,
        fps: u32,
        format: ExportFormat,
    ) -> Result<Child, String> {
        if let Some(parent) = output_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|err| format!("Export failed: {err}"))?;
        }

        let input_pattern = temp_dir.join("frame_%05d.png");
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y")
            .arg("-nostdin")
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-i")
            .arg(&input_pattern);
        match format {
            ExportFormat::Gif => {
                cmd.arg("-vf").arg(format!(
                    "fps={fps},split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse"
                ));
            }
            ExportFormat::Mp4 => {
                cmd.args(["-vf", "format=yuv420p", "-pix_fmt", "yuv420p"]);
            }
            ExportFormat::Png => {}
        }
        cmd.arg(output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match cmd.spawn() {
            Ok(child) => Ok(child),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err("Export failed: ffmpeg was not found on PATH.".to_string())
            }
            Err(err) => Err(format!("Export failed: {err}")),
        }
    }

    fn tick_export_job(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Some(mut job) = self.export_job.take() else {
            return;
        };

        let keep_job = match &mut job.stage {
            ExportJobStage::RenderingFrames {
                track,
                output_path,
                temp_dir,
                width,
                height,
                fps,
                format,
                frame_count,
                next_frame,
                first_projection,
                first_fov,
            } => {
                if *next_frame >= *frame_count {
                    match self.spawn_ffmpeg_export(temp_dir, output_path, *fps, *format) {
                        Ok(child) => {
                            debug_log(&format!(
                                "animated export encoding start path={}",
                                output_path.display()
                            ));
                            self.documents[job.doc_idx].export_status =
                                "Encoding animation...".to_string();
                            self.documents[job.doc_idx].export_progress = None;
                            job.stage = ExportJobStage::Encoding {
                                child,
                                output_path: output_path.clone(),
                                temp_dir: temp_dir.clone(),
                            };
                            true
                        }
                        Err(err) => {
                            let _ = std::fs::remove_dir_all(temp_dir);
                            self.documents[job.doc_idx].export_status = err;
                            self.documents[job.doc_idx].export_progress = None;
                            false
                        }
                    }
                } else {
                    let t = if *frame_count <= 1 {
                        0.0
                    } else {
                        track.duration() * *next_frame as f64 / (*frame_count - 1) as f64
                    };
                    let target = interpolate_camera(track, t);
                    let mut export_camera = self.documents[job.doc_idx].camera.clone();
                    export_camera.set_center(target.center);
                    export_camera.set_distance(target.distance);
                    export_camera.set_orientation(target.orientation);
                    export_camera.projection = *first_projection;
                    export_camera.set_fov_y(*first_fov);
                    export_camera.set_aspect_ratio(*width as f32, *height as f32);

                    let pixels =
                        match self.render_export_pixels(frame, &export_camera, *width, *height) {
                            Ok(pixels) => pixels,
                            Err(err) => {
                                let _ = std::fs::remove_dir_all(temp_dir);
                                self.documents[job.doc_idx].export_status = err.clone();
                                self.documents[job.doc_idx].export_progress = None;
                                return;
                            }
                        };

                    let frame_path = temp_dir.join(format!("frame_{:05}.png", *next_frame));
                    if let Err(err) = image::save_buffer(
                        &frame_path,
                        &pixels,
                        *width,
                        *height,
                        image::ColorType::Rgba8,
                    ) {
                        let _ = std::fs::remove_dir_all(temp_dir);
                        self.documents[job.doc_idx].export_status = format!("Export failed: {err}");
                        self.documents[job.doc_idx].export_progress = None;
                        return;
                    }

                    *next_frame += 1;
                    self.documents[job.doc_idx].export_status = format!(
                        "Rendering animation frames... {}/{}",
                        *next_frame, *frame_count
                    );
                    self.documents[job.doc_idx].export_progress =
                        Some(*next_frame as f32 / *frame_count as f32);
                    ctx.request_repaint();
                    true
                }
            }
            ExportJobStage::Encoding {
                child,
                output_path,
                temp_dir,
            } => match child.try_wait() {
                Ok(Some(status)) => {
                    let _ = std::fs::remove_dir_all(temp_dir);
                    self.documents[job.doc_idx].export_progress = None;
                    if status.success() {
                        self.documents[job.doc_idx].export_status =
                            format!("Exported {}", output_path.display());
                    } else {
                        self.documents[job.doc_idx].export_status =
                            format!("Export failed: ffmpeg exited with status {status}");
                    }
                    false
                }
                Ok(None) => {
                    self.documents[job.doc_idx].export_status = "Encoding animation...".to_string();
                    self.documents[job.doc_idx].export_progress = None;
                    ctx.request_repaint();
                    true
                }
                Err(err) => {
                    let _ = std::fs::remove_dir_all(temp_dir);
                    self.documents[job.doc_idx].export_status = format!("Export failed: {err}");
                    self.documents[job.doc_idx].export_progress = None;
                    false
                }
            },
        };

        if keep_job {
            self.export_job = Some(job);
        }
    }

    fn do_save_active_document(&mut self) {
        let doc = &self.documents[self.active_document_idx];
        if let Some(path) = doc.path.clone() {
            match persistence::save_document_to_path(doc, &path) {
                Ok(()) => {
                    self.documents[self.active_document_idx].dirty = false;
                    self.documents[self.active_document_idx]
                        .export_status
                        .clear();
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
            // (different fields of Document) so the borrows cannot alias.
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

fn is_equation_editable(kind: &PlotKind) -> bool {
    matches!(
        kind,
        PlotKind::ExprCartesian { .. }
            | PlotKind::ExprCurve { .. }
            | PlotKind::ExprCartesianLine { .. }
            | PlotKind::ExprSpherical { .. }
            | PlotKind::ExprCylindrical { .. }
            | PlotKind::ExprPolar { .. }
            | PlotKind::ExprParametricSurface { .. }
            | PlotKind::ScalarSlice { .. }
            | PlotKind::VectorSlice { .. }
            | PlotKind::GradientField { .. }
            | PlotKind::DivergenceField { .. }
            | PlotKind::CurlField { .. }
            | PlotKind::ExprVectorField { .. }
            | PlotKind::ExprVolume { .. }
            | PlotKind::ExprIsosurface { .. }
            | PlotKind::ExprStreamlines { .. }
    )
}

fn selected_plot_equation_text(kind: &PlotKind) -> String {
    match kind {
        PlotKind::ExprCartesian { expression, .. }
        | PlotKind::ExprCurve { expression, .. }
        | PlotKind::ExprCartesianLine { expression, .. }
        | PlotKind::ExprSpherical { expression, .. }
        | PlotKind::ExprCylindrical { expression, .. }
        | PlotKind::ExprPolar { expression, .. }
        | PlotKind::ExprParametricSurface { expression, .. }
        | PlotKind::ScalarSlice { expression, .. }
        | PlotKind::VectorSlice { expression, .. }
        | PlotKind::GradientField { expression, .. }
        | PlotKind::DivergenceField { expression, .. }
        | PlotKind::CurlField { expression, .. }
        | PlotKind::ExprVectorField { expression, .. }
        | PlotKind::ExprVolume { expression, .. }
        | PlotKind::ExprIsosurface { expression, .. }
        | PlotKind::ExprStreamlines { expression, .. } => expression.clone(),
        _ => String::new(),
    }
}

fn apply_expression_edit(kind: &mut PlotKind, text: &str) -> bool {
    match kind {
        PlotKind::ExprCartesian {
            expression,
            parameters,
        } => apply_single_expression(expression, parameters, text, &["x", "y"]),
        PlotKind::ExprCurve {
            expression,
            parameters,
            ..
        } => {
            if let Ok(parsed) = parse_curve_expr(text) {
                let old: std::collections::HashMap<String, f64> =
                    parameters.iter().cloned().collect();
                let mut seen = std::collections::HashSet::new();
                *parameters = parsed
                    .0
                    .parameters
                    .iter()
                    .chain(parsed.1.parameters.iter())
                    .chain(parsed.2.parameters.iter())
                    .filter_map(|(name, default)| {
                        seen.insert(name.clone())
                            .then(|| (name.clone(), old.get(name).copied().unwrap_or(*default)))
                    })
                    .collect();
            }
            *expression = text.to_string();
            true
        }
        PlotKind::ExprCartesianLine {
            ind_var,
            expression,
            parameters,
            ..
        } => apply_single_expression(expression, parameters, text, &[ind_var.as_str()]),
        PlotKind::ExprSpherical {
            expression,
            parameters,
        } => apply_single_expression(expression, parameters, text, &["theta", "phi"]),
        PlotKind::ExprCylindrical {
            expression,
            parameters,
        } => apply_single_expression(expression, parameters, text, &["theta", "z"]),
        PlotKind::ExprPolar {
            expression,
            parameters,
        } => apply_single_expression(expression, parameters, text, &["theta"]),
        PlotKind::ExprParametricSurface {
            expression,
            parameters,
        } => apply_triple_expression(expression, parameters, text, &["u", "v"]),
        PlotKind::ScalarSlice {
            expression,
            parameters,
            ..
        } => apply_single_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::VectorSlice {
            expression,
            parameters,
            ..
        } => apply_triple_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::GradientField {
            expression,
            parameters,
        } => apply_single_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::DivergenceField {
            expression,
            parameters,
            ..
        } => apply_triple_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::CurlField {
            expression,
            parameters,
        } => apply_triple_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::ExprVectorField {
            expression,
            parameters,
        } => apply_triple_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::ExprVolume {
            expression,
            parameters,
            ..
        } => apply_single_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::ExprIsosurface {
            expression,
            parameters,
            ..
        } => apply_single_expression(expression, parameters, text, &["x", "y", "z"]),
        PlotKind::ExprStreamlines {
            expression,
            parameters,
            ..
        } => apply_triple_expression(expression, parameters, text, &["x", "y", "z"]),
        _ => false,
    }
}

fn apply_single_expression(
    expression: &mut String,
    parameters: &mut Vec<(String, f64)>,
    text: &str,
    coord_vars: &[&str],
) -> bool {
    if let Ok(parsed) = parse_expr_with_vars(text, coord_vars) {
        let old: std::collections::HashMap<String, f64> = parameters.iter().cloned().collect();
        *parameters = parsed
            .parameters
            .into_iter()
            .map(|(name, default)| (name.clone(), old.get(&name).copied().unwrap_or(default)))
            .collect();
    }
    *expression = text.to_string();
    true
}

fn apply_triple_expression(
    expression: &mut String,
    parameters: &mut Vec<(String, f64)>,
    text: &str,
    coord_vars: &[&str],
) -> bool {
    let parts: Vec<_> = text.splitn(3, '|').map(str::trim).collect();
    let p0 = parts.first().copied().unwrap_or_default();
    let p1 = parts.get(1).copied().unwrap_or_default();
    let p2 = parts.get(2).copied().unwrap_or_default();
    if let (Ok(px), Ok(py), Ok(pz)) = (
        parse_expr_with_vars(p0, coord_vars),
        parse_expr_with_vars(p1, coord_vars),
        parse_expr_with_vars(p2, coord_vars),
    ) {
        let old: std::collections::HashMap<String, f64> = parameters.iter().cloned().collect();
        let mut seen = std::collections::HashSet::new();
        *parameters = px
            .parameters
            .iter()
            .chain(py.parameters.iter())
            .chain(pz.parameters.iter())
            .filter_map(|(name, default)| {
                seen.insert(name.clone())
                    .then(|| (name.clone(), old.get(name).copied().unwrap_or(*default)))
            })
            .collect();
    }
    *expression = format!("{p0}|{p1}|{p2}");
    true
}

fn data_editor_payload_from_plot_kind(kind: &PlotKind) -> Option<DataEditorPayload> {
    match kind {
        PlotKind::ImportedTable { definition } => {
            Some(DataEditorPayload::ImportedTable(definition.clone()))
        }
        PlotKind::PointAnnotations {
            points,
            show_labels,
        } => Some(DataEditorPayload::PointAnnotations {
            raw_text: serialize_point_annotations(points),
            show_labels: *show_labels,
            error: None,
        }),
        PlotKind::ArrowAnnotations {
            arrows,
            show_labels,
        } => Some(DataEditorPayload::ArrowAnnotations {
            raw_text: serialize_arrow_annotations(arrows),
            show_labels: *show_labels,
            error: None,
        }),
        PlotKind::DerivedPolylineGroups { groups } => {
            Some(DataEditorPayload::DerivedPolylineGroups {
                raw_text: serialize_polyline_groups(groups),
                error: None,
            })
        }
        _ => None,
    }
}

fn data_editor_payload_is_dirty(current: &DataEditorPayload, original: &DataEditorPayload) -> bool {
    match (current, original) {
        (DataEditorPayload::ImportedTable(a), DataEditorPayload::ImportedTable(b)) => a != b,
        (
            DataEditorPayload::PointAnnotations {
                raw_text: a_text,
                show_labels: a_show,
                ..
            },
            DataEditorPayload::PointAnnotations {
                raw_text: b_text,
                show_labels: b_show,
                ..
            },
        ) => a_text != b_text || a_show != b_show,
        (
            DataEditorPayload::ArrowAnnotations {
                raw_text: a_text,
                show_labels: a_show,
                ..
            },
            DataEditorPayload::ArrowAnnotations {
                raw_text: b_text,
                show_labels: b_show,
                ..
            },
        ) => a_text != b_text || a_show != b_show,
        (
            DataEditorPayload::DerivedPolylineGroups {
                raw_text: a_text, ..
            },
            DataEditorPayload::DerivedPolylineGroups {
                raw_text: b_text, ..
            },
        ) => a_text != b_text,
        _ => true,
    }
}

fn edit_data_payload(
    ui: &mut egui::Ui,
    payload: &mut DataEditorPayload,
    edit_mode: DataEditorMode,
    table_state: &mut DataTableState,
) -> bool {
    match payload {
        DataEditorPayload::ImportedTable(definition) => {
            edit_imported_table_payload(ui, definition, edit_mode, table_state)
        }
        DataEditorPayload::PointAnnotations {
            raw_text,
            show_labels,
            error,
        } => {
            ui.label("Columns: `x<TAB>y<TAB>z<TAB>label`");
            ui.checkbox(show_labels, "Show labels");
            ui.add_space(6.0);
            edit_text_or_cells(
                ui,
                raw_text,
                edit_mode,
                &["x", "y", "z", "label"],
                '\t',
                table_state,
            );
            match parse_point_annotations(raw_text) {
                Ok(points) => {
                    *error = None;
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 210, 150),
                        format!("{} point annotation(s)", points.len()),
                    );
                    true
                }
                Err(message) => {
                    *error = Some(message.clone());
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), message);
                    false
                }
            }
        }
        DataEditorPayload::ArrowAnnotations {
            raw_text,
            show_labels,
            error,
        } => {
            ui.label("Columns: `ox<TAB>oy<TAB>oz<TAB>vx<TAB>vy<TAB>vz<TAB>label`");
            ui.checkbox(show_labels, "Show labels");
            ui.add_space(6.0);
            edit_text_or_cells(
                ui,
                raw_text,
                edit_mode,
                &["ox", "oy", "oz", "vx", "vy", "vz", "label"],
                '\t',
                table_state,
            );
            match parse_arrow_annotations(raw_text) {
                Ok(arrows) => {
                    *error = None;
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 210, 150),
                        format!("{} arrow annotation(s)", arrows.len()),
                    );
                    true
                }
                Err(message) => {
                    *error = Some(message.clone());
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), message);
                    false
                }
            }
        }
        DataEditorPayload::DerivedPolylineGroups { raw_text, error } => {
            ui.label("Columns: `group<TAB>x<TAB>y<TAB>z`");
            ui.add_space(6.0);
            edit_text_or_cells(
                ui,
                raw_text,
                edit_mode,
                &["group", "x", "y", "z"],
                '\t',
                table_state,
            );
            match parse_polyline_groups(raw_text) {
                Ok(groups) => {
                    let point_count: usize = groups.iter().map(Vec::len).sum();
                    *error = None;
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 210, 150),
                        format!("{} polyline(s), {} point(s)", groups.len(), point_count),
                    );
                    true
                }
                Err(message) => {
                    *error = Some(message.clone());
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), message);
                    false
                }
            }
        }
    }
}

fn edit_imported_table_payload(
    ui: &mut egui::Ui,
    definition: &mut TableImportDefinition,
    edit_mode: DataEditorMode,
    table_state: &mut DataTableState,
) -> bool {
    match edit_mode {
        DataEditorMode::Raw => {
            crate::ui::table_editor::edit_table_import(ui, definition);
            definition.validate().is_ok()
        }
        DataEditorMode::Cells => {
            ui.horizontal(|ui| {
                if ui.button("Auto Detect").clicked() {
                    definition.auto_configure();
                }
                egui::ComboBox::from_label("Delimiter")
                    .selected_text(definition.delimiter.label())
                    .show_ui(ui, |ui| {
                        for delimiter in crate::plot::table::TableDelimiter::ALL {
                            ui.selectable_value(
                                &mut definition.delimiter,
                                delimiter,
                                delimiter.label(),
                            );
                        }
                    });
                ui.checkbox(&mut definition.header_row, "Header row");
            });
            ui.label(
                egui::RichText::new(definition.source_summary())
                    .small()
                    .weak(),
            );
            let delimiter = delimiter_char(definition.delimiter);
            edit_text_or_cells(
                ui,
                &mut definition.raw_text,
                DataEditorMode::Cells,
                &[],
                delimiter,
                table_state,
            );
            ui.separator();
            let preview = definition.preview();
            ui.label(format!(
                "{} columns, {} data rows",
                preview.column_count,
                preview.rows.len()
            ));
            match definition.validate() {
                Ok(_) => {
                    ui.colored_label(egui::Color32::from_rgb(120, 210, 150), "Import is valid");
                    true
                }
                Err(errors) => {
                    for error in errors.iter().take(5) {
                        ui.colored_label(egui::Color32::from_rgb(255, 110, 110), error.display());
                    }
                    false
                }
            }
        }
    }
}

fn edit_text_or_cells(
    ui: &mut egui::Ui,
    raw_text: &mut String,
    edit_mode: DataEditorMode,
    fallback_headers: &[&str],
    delimiter: char,
    table_state: &mut DataTableState,
) {
    match edit_mode {
        DataEditorMode::Raw => {
            ui.add(
                egui::TextEdit::multiline(raw_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(16),
            );
        }
        DataEditorMode::Cells => {
            let mut rows = ui::data_table::parse_rows(raw_text, delimiter);
            if rows.is_empty() && !fallback_headers.is_empty() {
                rows.push(
                    fallback_headers
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                );
            }
            let headers = fallback_headers
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>();
            ui::data_table::show_data_table(
                ui,
                table_state,
                &headers,
                &mut rows,
                ui::data_table::DataTableOptions::editable("data_editor_cells"),
            );
            *raw_text = ui::data_table::serialize_rows(&rows, delimiter);
        }
    }
}

fn apply_data_editor_payload(kind: &mut PlotKind, payload: &DataEditorPayload) -> bool {
    match (kind, payload) {
        (PlotKind::ImportedTable { definition }, DataEditorPayload::ImportedTable(next)) => {
            *definition = next.clone();
            true
        }
        (
            PlotKind::PointAnnotations {
                points,
                show_labels,
            },
            DataEditorPayload::PointAnnotations {
                raw_text,
                show_labels: next_show_labels,
                ..
            },
        ) => match parse_point_annotations(raw_text) {
            Ok(next_points) => {
                *points = next_points;
                *show_labels = *next_show_labels;
                true
            }
            Err(_) => false,
        },
        (
            PlotKind::ArrowAnnotations {
                arrows,
                show_labels,
            },
            DataEditorPayload::ArrowAnnotations {
                raw_text,
                show_labels: next_show_labels,
                ..
            },
        ) => match parse_arrow_annotations(raw_text) {
            Ok(next_arrows) => {
                *arrows = next_arrows;
                *show_labels = *next_show_labels;
                true
            }
            Err(_) => false,
        },
        (
            PlotKind::DerivedPolylineGroups { groups },
            DataEditorPayload::DerivedPolylineGroups { raw_text, .. },
        ) => match parse_polyline_groups(raw_text) {
            Ok(next_groups) => {
                *groups = next_groups;
                true
            }
            Err(_) => false,
        },
        _ => false,
    }
}

fn serialize_point_annotations(points: &[poincare_lib::PointAnnotation]) -> String {
    points
        .iter()
        .map(|point| {
            format!(
                "{}\t{}\t{}\t{}",
                point.position[0], point.position[1], point.position[2], point.label
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_arrow_annotations(arrows: &[poincare_lib::ArrowAnnotation]) -> String {
    arrows
        .iter()
        .map(|arrow| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                arrow.origin[0],
                arrow.origin[1],
                arrow.origin[2],
                arrow.vector[0],
                arrow.vector[1],
                arrow.vector[2],
                arrow.label
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_polyline_groups(groups: &[Vec<[f32; 3]>]) -> String {
    groups
        .iter()
        .enumerate()
        .flat_map(|(group_idx, group)| {
            group
                .iter()
                .map(move |point| format!("{group_idx}\t{}\t{}\t{}", point[0], point[1], point[2]))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_point_annotations(raw_text: &str) -> Result<Vec<poincare_lib::PointAnnotation>, String> {
    let mut points = Vec::new();
    for (line_idx, line) in raw_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cells = split_editor_line(trimmed, 4);
        if cells.len() < 3 {
            return Err(format!("Line {} needs at least 3 columns", line_idx + 1));
        }
        points.push(poincare_lib::PointAnnotation {
            position: [
                parse_f32_cell(cells[0], line_idx, "x")?,
                parse_f32_cell(cells[1], line_idx, "y")?,
                parse_f32_cell(cells[2], line_idx, "z")?,
            ],
            label: cells.get(3).copied().unwrap_or_default().to_string(),
        });
    }
    Ok(points)
}

fn parse_arrow_annotations(raw_text: &str) -> Result<Vec<poincare_lib::ArrowAnnotation>, String> {
    let mut arrows = Vec::new();
    for (line_idx, line) in raw_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cells = split_editor_line(trimmed, 7);
        if cells.len() < 6 {
            return Err(format!("Line {} needs at least 6 columns", line_idx + 1));
        }
        arrows.push(poincare_lib::ArrowAnnotation {
            origin: [
                parse_f32_cell(cells[0], line_idx, "ox")?,
                parse_f32_cell(cells[1], line_idx, "oy")?,
                parse_f32_cell(cells[2], line_idx, "oz")?,
            ],
            vector: [
                parse_f32_cell(cells[3], line_idx, "vx")?,
                parse_f32_cell(cells[4], line_idx, "vy")?,
                parse_f32_cell(cells[5], line_idx, "vz")?,
            ],
            label: cells.get(6).copied().unwrap_or_default().to_string(),
        });
    }
    Ok(arrows)
}

fn parse_polyline_groups(raw_text: &str) -> Result<Vec<Vec<[f32; 3]>>, String> {
    let mut ordered: Vec<(String, Vec<[f32; 3]>)> = Vec::new();
    for (line_idx, line) in raw_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cells = split_editor_line(trimmed, 4);
        if cells.len() < 4 {
            return Err(format!("Line {} needs 4 columns", line_idx + 1));
        }
        let group_name = cells[0].to_string();
        let point = [
            parse_f32_cell(cells[1], line_idx, "x")?,
            parse_f32_cell(cells[2], line_idx, "y")?,
            parse_f32_cell(cells[3], line_idx, "z")?,
        ];
        if let Some((_, group)) = ordered.iter_mut().find(|(name, _)| *name == group_name) {
            group.push(point);
        } else {
            ordered.push((group_name, vec![point]));
        }
    }
    Ok(ordered.into_iter().map(|(_, group)| group).collect())
}

fn split_editor_line<'a>(line: &'a str, max_columns: usize) -> Vec<&'a str> {
    let delimiter = if line.contains('\t') { '\t' } else { ',' };
    line.splitn(max_columns, delimiter).map(str::trim).collect()
}

fn delimiter_char(delimiter: crate::plot::table::TableDelimiter) -> char {
    match delimiter {
        crate::plot::table::TableDelimiter::Comma => ',',
        crate::plot::table::TableDelimiter::Semicolon => ';',
        crate::plot::table::TableDelimiter::Tab => '\t',
        crate::plot::table::TableDelimiter::Space => ' ',
        crate::plot::table::TableDelimiter::Pipe => '|',
    }
}

fn parse_f32_cell(value: &str, line_idx: usize, label: &str) -> Result<f32, String> {
    value.parse::<f32>().map_err(|_| {
        format!(
            "Line {} has invalid {} value `{}`",
            line_idx + 1,
            label,
            value
        )
    })
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        persistence::save_persisted_state(storage, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Intercept OS close request; prompt if any document has unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) {
            let any_dirty = self.documents.iter().any(|d| d.dirty);
            if self.force_quit {
                self.force_quit = false;
            } else if any_dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_quit = true;
            }
        }

        // Handle pending file actions (rfd dialogs are blocking, so call them
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
                        if doc.title.is_empty() {
                            doc.title = stem;
                        }
                        doc.path = Some(path);
                        doc.dirty = false;
                        doc.initialize_history();
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
        if let Some(committed) = self.eq_editor.take_committed_selected_plot() {
            self.apply_selected_plot_equation_edit(committed);
        }
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
        let sweeps_active = self.tick_parameter_sweeps(dt);
        if sweeps_active {
            ctx.request_repaint();
        }
        self.apply_frame_playback(dt as f32);
        self.apply_frame_camera_attachment();
        if self.documents[self.active_document_idx]
            .frame_playback
            .playing
        {
            ctx.request_repaint();
        }
        {
            let track = self.build_saved_view_track();
            let mut apply_track_t = None;
            {
                let doc = &mut self.documents[self.active_document_idx];
                if doc.camera_track_playing && track.len() >= 2 {
                    doc.camera_track_t += dt;
                    if doc.camera_track_t >= track.duration() {
                        doc.camera_track_t = track.duration();
                        doc.camera_track_playing = false;
                    }
                    apply_track_t = Some(doc.camera_track_t);
                } else if doc.camera_track_playing {
                    doc.camera_track_playing = false;
                }
            }
            if let Some(t) = apply_track_t {
                self.apply_saved_view_track_sample(t);
                ctx.request_repaint();
            }
        }
        let camera_dt = ctx.input(|i| i.stable_dt).max(1.0 / 240.0);
        if self.camera_animator.update(
            camera_dt,
            &mut self.documents[self.active_document_idx].camera,
        ) {
            ctx.request_repaint();
        }

        self.rebuild_scene(frame);
        self.tick_export_job(ctx, frame);
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(color32_from_rgba(
                self.documents[self.active_document_idx].viewport_background,
            )))
            .show(ctx, |ui| {
                self.dock_ui(ui, frame);
            });

        ui::equation_editor::show_eq_editor_window(ctx, &mut self.eq_editor);
        self.show_add_plot_modal(ctx);
        self.show_interpolate_modal(ctx);
        self.show_axis_derivative_modal(ctx);
        self.show_fit_curve_modal(ctx);
        self.show_surface_normals_modal(ctx);
        self.show_surface_curvature_modal(ctx);
        self.show_curve_surface_measurement_modal(ctx);
        self.show_data_editor_modal(ctx);
        self.show_command_palette(ctx);
        self.show_shortcuts_modal(ctx);
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
                        if ui.button("Close Without Saving").clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancelled = true;
                        }
                    });
                });
            if confirmed {
                self.confirm_close_idx = None;
                self.close_document(close_idx);
            } else if cancelled {
                self.confirm_close_idx = None;
            }
        }

        if let Some(plot_idx) = self.confirm_delete_plot_idx {
            let plot_name = self.documents[self.active_document_idx]
                .plots
                .get(plot_idx)
                .map(|plot| plot.name.clone())
                .unwrap_or_else(|| "selected plot".to_string());
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Window::new("Delete Plot")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Delete plot \"{plot_name}\"?"));
                    ui.label(
                        egui::RichText::new("Press Enter to delete or Escape to cancel.")
                            .small()
                            .weak(),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let yes = ui.button("Yes");
                        yes.request_focus();
                        if yes.clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancelled = true;
                        }
                    });
                });
            if confirmed {
                self.confirm_delete_selected_plot();
            } else if cancelled {
                self.confirm_delete_plot_idx = None;
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
                    ui.label(format!(
                        "{n} document(s) have unsaved changes. Quit anyway?"
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Quit Without Saving").clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancelled = true;
                        }
                    });
                });
            if confirmed {
                self.confirm_quit = false;
                self.force_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if cancelled {
                self.confirm_quit = false;
            }
        }

        self.finalize_undo_point();
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

fn normalized_export_path(path_text: &str, format: ExportFormat) -> PathBuf {
    let raw = path_text.trim();
    let mut path = if raw.is_empty() {
        default_export_dir(export_mode_for_format(format)).join(default_export_filename(format))
    } else {
        PathBuf::from(raw)
    };

    let expected_ext = match format {
        ExportFormat::Png => "png",
        ExportFormat::Gif => "gif",
        ExportFormat::Mp4 => "mp4",
    };

    if path.extension().and_then(|ext| ext.to_str()) != Some(expected_ext) {
        path.set_extension(expected_ext);
    }

    path
}

fn split_export_path(path_text: &str, format: ExportFormat) -> (PathBuf, String) {
    let path = normalized_export_path(path_text, format);
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(default_export_filename(format))
        .to_string();
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_export_dir(export_mode_for_format(format)));
    (dir, filename)
}

fn export_path_from_parts(dir: &std::path::Path, filename: &str, format: ExportFormat) -> PathBuf {
    let trimmed = filename.trim();
    let mut path = if trimmed.is_empty() {
        dir.join(default_export_filename(format))
    } else {
        dir.join(trimmed)
    };
    let expected_ext = match format {
        ExportFormat::Png => "png",
        ExportFormat::Gif => "gif",
        ExportFormat::Mp4 => "mp4",
    };
    if path.extension().and_then(|ext| ext.to_str()) != Some(expected_ext) {
        path.set_extension(expected_ext);
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
