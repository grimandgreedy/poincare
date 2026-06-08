use std::collections::HashMap;

use eframe::Storage;
use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, CurveInterpolation, CurveInterpolationKind, Domain,
    GlyphType, GraphSpec, MatcapSource, ParamVisSettings, PlotSpec, PlotStyle, Resolution,
    ShadingMode, SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField, TransferFunction,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use viewport_lib::{
    AttributeKind, BuiltinColourmap, BuiltinMatcap, GroundPlaneMode, ParamVisMode, Projection,
};

use crate::App;
use crate::document::{
    Document, ExportFormat, FrameAttachment, FrameAttachmentKind, FramePlaybackState,
    SavedCameraView, StoredFrameField,
};
use crate::plot::analysis::{ArrowAnnotation, PointAnnotation, SliceAxis};
use crate::plot::entry::{PlotEntry, PlotId, PlotRelationship};
use crate::plot::kind::{PlotKind, SeedMode};
use crate::plot::sweep::ParameterSweep;
use crate::plot::table::TableImportDefinition;

const APP_STATE_KEY: &str = "poincare_app_state";

// ---------------------------------------------------------------------------
// Top-level eframe storage blob
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct PersistedAppState {
    #[serde(default)]
    settings: PersistedAppSettings,
    #[serde(default)]
    session: Option<DocumentSnapshot>,
}

// ---------------------------------------------------------------------------
// App-level settings (panel colours, preferences; never written to project
// files, never read from them)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct PersistedAppSettings {
    #[serde(default = "default_panel_header_bg")]
    panel_header_bg: [u8; 4],
    #[serde(default = "default_panel_content_bg")]
    panel_content_bg: [u8; 4],
    #[serde(default = "default_tab_selected_bg")]
    tab_selected_bg: [u8; 4],
    #[serde(default = "default_tab_highlight")]
    tab_highlight: [u8; 4],
    #[serde(default)]
    default_colormap: u8,
    #[serde(default)]
    invert_scroll: bool,
    #[serde(default)]
    save_state_on_exit: bool,
}

impl Default for PersistedAppSettings {
    fn default() -> Self {
        let panel_style = crate::default_panel_style();
        Self {
            panel_header_bg: color32_to_rgba(panel_style.header.bg),
            panel_content_bg: color32_to_rgba(panel_style.content.bg),
            tab_selected_bg: color32_to_rgba(panel_style.tabs.active.bg),
            tab_highlight: color32_to_rgba(panel_style.tabs.active.accent_color),
            default_colormap: builtin_colormap_to_u8(BuiltinColourmap::Viridis),
            invert_scroll: false,
            save_state_on_exit: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter sweep persistence
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct PersistedParameterSweep {
    min: f64,
    max: f64,
    #[serde(default = "default_parameter_step")]
    step: f64,
    speed: f64,
    playing: bool,
    phase: f64,
    direction: f64,
}

impl PersistedParameterSweep {
    fn from_sweep(s: &ParameterSweep) -> Self {
        Self {
            min: s.min,
            max: s.max,
            step: s.step,
            speed: s.speed,
            playing: s.playing,
            phase: s.phase,
            direction: s.direction,
        }
    }

    fn to_sweep(&self) -> ParameterSweep {
        ParameterSweep {
            min: self.min,
            max: self.max,
            step: self.step,
            speed: self.speed,
            playing: self.playing,
            phase: self.phase,
            direction: self.direction,
        }
    }
}

fn default_parameter_step() -> f64 {
    0.1
}

fn default_frame_scale() -> f32 {
    1.0
}

fn default_frame_camera_distance() -> f32 {
    3.0
}

fn default_frame_playback_speed() -> f32 {
    0.25
}

fn default_true() -> bool {
    true
}

fn default_plot_relationship() -> PersistedPlotRelationship {
    PersistedPlotRelationship::Primary
}

fn load_slice_axis(axis: u8) -> SliceAxis {
    match axis {
        0 => SliceAxis::X,
        1 => SliceAxis::Y,
        _ => SliceAxis::Z,
    }
}

// ---------------------------------------------------------------------------
// Document snapshot: serializable form of one Document.
// Used for:
//   - eframe session restore (stored inside PersistedAppState.session)
//   - project file save/load via save_document_to_path / load_document_from_path
//
// All fields are #[serde(default)] so that old eframe storage blobs (which
// only had `plots` + `selected_plot`) deserialize cleanly when new fields are
// added.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentSnapshot {
    #[serde(default)]
    pub graph: Option<GraphSpec>,

    // Plot content
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plots: Vec<PersistedPlotEntry>,
    #[serde(default)]
    pub selected_plot: Option<usize>,

    // View settings (moved here from PersistedSettings in Phase 3)
    #[serde(default)]
    pub axis_config: PersistedAxisConfig,
    #[serde(default)]
    pub ground_plane_mode: u8,
    #[serde(default)]
    pub ground_plane_height: f32,
    #[serde(default = "default_ground_plane_color")]
    pub ground_plane_color: [f32; 4],
    #[serde(default = "default_ground_plane_tile_size")]
    pub ground_plane_tile_size: f32,
    #[serde(default = "default_viewport_background")]
    pub viewport_background: [f32; 4],
    #[serde(default)]
    pub projection: u8,

    // Metadata / export defaults (Phase 4+)
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_export_path")]
    pub export_path: String,
    #[serde(default = "default_export_width")]
    pub export_width: u32,
    #[serde(default = "default_export_height")]
    pub export_height: u32,
    #[serde(default)]
    pub export_format: u8,
    #[serde(default = "default_export_fps")]
    pub export_fps: u32,
    #[serde(default = "default_camera_track_segment_duration")]
    pub camera_track_segment_duration: f32,
    #[serde(default)]
    pub saved_views: Vec<PersistedSavedCameraView>,

    /// Per-plot parameter sweep config (Phase 6).  `#[serde(default)]` keeps
    /// files written before Phase 6 loadable without error.
    #[serde(default)]
    pub sweep_config: Vec<HashMap<String, PersistedParameterSweep>>,
    #[serde(default)]
    pub frame_fields: Vec<PersistedFrameField>,
    #[serde(default)]
    pub frame_attachments: Vec<PersistedFrameAttachment>,
    #[serde(default)]
    pub frame_playback: PersistedFramePlaybackState,
}

// ---------------------------------------------------------------------------
// Serialization helpers for PersistedAxisConfig
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub(crate) struct PersistedAxisConfig {
    show_box: bool,
    show_labels: bool,
    show_ticks: bool,
    show_grid: bool,
    labels: [Option<String>; 3],
    tick_count: [u32; 3],
}

impl Default for PersistedAxisConfig {
    fn default() -> Self {
        Self::from_axis_config(&AxisConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Plot serialization types (unchanged from before)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub(crate) struct PersistedPlotEntry {
    #[serde(default)]
    plot_id: PlotId,
    #[serde(default)]
    parent_plot_id: Option<PlotId>,
    #[serde(default = "default_plot_relationship")]
    relationship: PersistedPlotRelationship,
    name: String,
    visible: bool,
    domain: PersistedDomain,
    resolution: PersistedResolution,
    style: PersistedPlotStyle,
    kind: PersistedPlotKind,
}

#[derive(Serialize, Deserialize)]
enum PersistedPlotRelationship {
    Primary,
    DerivedAnalysis,
}

#[derive(Serialize, Deserialize)]
struct PersistedDomain {
    x: [f64; 2],
    y: [f64; 2],
    z: [f64; 2],
}

#[derive(Serialize, Deserialize)]
struct PersistedResolution {
    u: u32,
    v: u32,
}

#[derive(Serialize, Deserialize)]
struct PersistedPlotStyle {
    colour_mode: PersistedColourMode,
    opacity: f32,
    two_sided: bool,
    line_width: f32,
    point_size: f32,
    glyph_scale: f32,
    #[serde(default)]
    glyph_type: u8,
    shading: u8,
    tube_radius: Option<f32>,
    transfer_function: Option<PersistedTransferFunction>,
    matcap: Option<u8>,
    param_vis: Option<PersistedParamVis>,
    face_quantity: Option<u8>,
    surface_lic: Option<PersistedSurfaceLic>,
}

#[derive(Serialize, Deserialize)]
enum PersistedColourMode {
    Solid([f32; 4]),
    Colormap {
        colormap: u8,
        scalar_range: Option<(f32, f32)>,
    },
    ByAttribute {
        name: String,
        kind: u8,
    },
}

#[derive(Serialize, Deserialize)]
struct PersistedTransferFunction {
    opacity_scale: f32,
    threshold: Option<(f32, f32)>,
}

#[derive(Serialize, Deserialize)]
struct PersistedParamVis {
    mode: u8,
    scale: f32,
}

#[derive(Serialize, Deserialize)]
struct PersistedSurfaceLic {
    vector_field: u8,
    steps: u32,
    step_size: f32,
    strength: f32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PersistedSavedCameraView {
    name: String,
    center: [f32; 3],
    distance: f32,
    orientation: [f32; 4],
    projection: u8,
    fov_y: f32,
}

#[derive(Serialize, Deserialize)]
enum PersistedSeedMode {
    Grid { nx: u32, ny: u32, nz: u32 },
    Plane { axis: usize, offset: f32 },
    ManualCsv { csv_text: String },
}

#[derive(Serialize, Deserialize)]
enum PersistedPlotKind {
    ContouredSurface {
        contour_values: Vec<f32>,
        contour_style: PersistedPlotStyle,
    },
    SphericalHarmonic,
    HelixCurve,
    ScatterCloud,
    VectorField,
    GridSurface,
    Streamlines {
        seeds: Vec<[f32; 3]>,
    },
    VolumeRender {
        resolution: [u32; 3],
    },
    Isosurface {
        isovalues: Vec<f64>,
        resolution: [u32; 3],
    },
    ExprCartesian {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprCurve {
        expression: String,
        parameters: Vec<(String, f64)>,
        t_range: (f64, f64),
    },
    ExprCartesianLine {
        dep_var: String,
        ind_var: String,
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprSpherical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprCylindrical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprPolar {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprParametricSurface {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ImportedTable {
        definition: TableImportDefinition,
    },
    ScalarSlice {
        expression: String,
        parameters: Vec<(String, f64)>,
        axis: u8,
        position: f64,
        contour_values: Vec<f32>,
        contour_style: PersistedPlotStyle,
    },
    VectorSlice {
        expression: String,
        parameters: Vec<(String, f64)>,
        axis: u8,
        position: f64,
    },
    GradientField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    DivergenceField {
        expression: String,
        parameters: Vec<(String, f64)>,
        vol_resolution: [u32; 3],
    },
    CurlField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    PointAnnotations {
        points: Vec<PointAnnotation>,
        #[serde(default = "default_true")]
        show_labels: bool,
    },
    ArrowAnnotations {
        arrows: Vec<ArrowAnnotation>,
        #[serde(default = "default_true")]
        show_labels: bool,
    },
    DerivedSurfaceMesh {
        positions: Vec<[f32; 3]>,
        indices: Vec<u32>,
        values: Vec<f32>,
        value_name: String,
    },
    DerivedPolylineGroups {
        groups: Vec<Vec<[f32; 3]>>,
    },
    InterpolatedCurve {
        points: Vec<[f32; 3]>,
        interpolation: PersistedCurveInterpolation,
    },
    ExprVectorField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprVolume {
        expression: String,
        parameters: Vec<(String, f64)>,
        vol_resolution: [u32; 3],
    },
    ExprIsosurface {
        expression: String,
        parameters: Vec<(String, f64)>,
        isovalues: Vec<f64>,
        iso_colours: Vec<[f32; 4]>,
        vol_resolution: [u32; 3],
    },
    ExprStreamlines {
        expression: String,
        parameters: Vec<(String, f64)>,
        seed_mode: PersistedSeedMode,
        step_size: f32,
        max_steps: u32,
    },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PersistedFrameField {
    id: u64,
    title: String,
    #[serde(default)]
    source_plot_ids: Vec<PlotId>,
    #[serde(default)]
    source_plot_names: Vec<String>,
    kind: String,
    samples: Vec<PersistedFrameSample>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PersistedFrameSample {
    parameter: f32,
    position: [f32; 3],
    tangent: [f32; 3],
    normal: [f32; 3],
    binormal: [f32; 3],
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct PersistedFrameAttachment {
    name: String,
    frame_field_id: u64,
    kind: String,
    enabled: bool,
    #[serde(default = "default_frame_scale")]
    scale: f32,
    #[serde(default = "default_frame_camera_distance")]
    camera_distance: f32,
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct PersistedFramePlaybackState {
    selected_frame_field: Option<u64>,
    #[serde(default)]
    phase: f32,
    #[serde(default)]
    playing: bool,
    #[serde(default = "default_frame_playback_speed")]
    speed: f32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PersistedCurveInterpolationKind {
    Linear,
    CatmullRom,
    CentripetalCatmullRom,
    MovingAverage,
    SavitzkyGolay,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PersistedCurveInterpolation {
    kind: PersistedCurveInterpolationKind,
    samples_per_segment: u32,
    closed: bool,
    #[serde(default = "default_curve_smoothing_window")]
    smoothing_window: u32,
}

impl PersistedCurveInterpolation {
    fn from_curve_interpolation(value: CurveInterpolation) -> Self {
        Self {
            kind: match value.kind {
                CurveInterpolationKind::Linear => PersistedCurveInterpolationKind::Linear,
                CurveInterpolationKind::CatmullRom => PersistedCurveInterpolationKind::CatmullRom,
                CurveInterpolationKind::CentripetalCatmullRom => {
                    PersistedCurveInterpolationKind::CentripetalCatmullRom
                }
                CurveInterpolationKind::MovingAverage => {
                    PersistedCurveInterpolationKind::MovingAverage
                }
                CurveInterpolationKind::SavitzkyGolay => {
                    PersistedCurveInterpolationKind::SavitzkyGolay
                }
            },
            samples_per_segment: value.samples_per_segment,
            closed: value.closed,
            smoothing_window: value.smoothing_window,
        }
    }

    fn to_curve_interpolation(self) -> CurveInterpolation {
        CurveInterpolation {
            kind: match self.kind {
                PersistedCurveInterpolationKind::Linear => CurveInterpolationKind::Linear,
                PersistedCurveInterpolationKind::CatmullRom => CurveInterpolationKind::CatmullRom,
                PersistedCurveInterpolationKind::CentripetalCatmullRom => {
                    CurveInterpolationKind::CentripetalCatmullRom
                }
                PersistedCurveInterpolationKind::MovingAverage => {
                    CurveInterpolationKind::MovingAverage
                }
                PersistedCurveInterpolationKind::SavitzkyGolay => {
                    CurveInterpolationKind::SavitzkyGolay
                }
            },
            samples_per_segment: self.samples_per_segment,
            closed: self.closed,
            smoothing_window: self.smoothing_window,
        }
    }
}

fn default_curve_smoothing_window() -> u32 {
    5
}

// ---------------------------------------------------------------------------
// eframe storage entry points
// ---------------------------------------------------------------------------

pub(crate) fn load_persisted_state(storage: Option<&dyn Storage>, app: &mut App) {
    let Some(storage) = storage else { return };
    let Some(saved) = eframe::get_value::<PersistedAppState>(storage, APP_STATE_KEY) else {
        return;
    };
    saved.settings.apply_to_app(app);
    if let Some(session) = saved.session {
        session.apply_to_app(app);
    }
}

pub(crate) fn save_persisted_state(storage: &mut dyn Storage, app: &App) {
    let state = PersistedAppState {
        settings: PersistedAppSettings::from_app(app),
        session: app
            .save_state_on_exit
            .then(|| DocumentSnapshot::from_app(app)),
    };
    eframe::set_value(storage, APP_STATE_KEY, &state);
}

// ---------------------------------------------------------------------------
// File-based project I/O
// ---------------------------------------------------------------------------

pub(crate) fn save_document_to_path(doc: &Document, path: &Path) -> Result<(), String> {
    let snapshot = DocumentSnapshot::from_document(doc);
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub(crate) fn load_document_from_path(path: &Path) -> Result<DocumentSnapshot, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// PersistedAppSettings conversions
// ---------------------------------------------------------------------------

impl PersistedAppSettings {
    fn from_app(app: &App) -> Self {
        Self {
            panel_header_bg: color32_to_rgba(app.panel_style.header.bg),
            panel_content_bg: color32_to_rgba(app.panel_style.content.bg),
            tab_selected_bg: color32_to_rgba(app.panel_style.tabs.active.bg),
            tab_highlight: color32_to_rgba(app.panel_style.tabs.active.accent_color),
            default_colormap: builtin_colormap_to_u8(app.default_colormap),
            invert_scroll: app.invert_scroll,
            save_state_on_exit: app.save_state_on_exit,
        }
    }

    fn apply_to_app(&self, app: &mut App) {
        app.panel_style.header.bg = rgba_to_color32(self.panel_header_bg);
        app.panel_style.content.bg = rgba_to_color32(self.panel_content_bg);
        app.panel_style.tabs.active.bg = rgba_to_color32(self.tab_selected_bg);
        let tab_highlight = rgba_to_color32(self.tab_highlight);
        app.panel_style.tabs.active.accent_color = tab_highlight;
        app.panel_style.tabs.inactive.accent_color = tab_highlight;
        app.panel_style.tabs.hovered.accent_color = tab_highlight;
        app.default_colormap = u8_to_builtin_colormap(self.default_colormap);
        app.invert_scroll = self.invert_scroll;
        app.save_state_on_exit = self.save_state_on_exit;
    }
}

// ---------------------------------------------------------------------------
// DocumentSnapshot conversions
// ---------------------------------------------------------------------------

impl DocumentSnapshot {
    /// Build a snapshot from a document (used for file save and session save).
    pub(crate) fn from_document(doc: &Document) -> Self {
        Self {
            version: 2,
            title: doc.title.clone(),
            graph: None,
            plots: doc
                .plots
                .iter()
                .map(PersistedPlotEntry::from_plot_entry)
                .collect(),
            selected_plot: doc.selected_plot,
            axis_config: PersistedAxisConfig::from_axis_config(&doc.axis_config),
            ground_plane_mode: ground_plane_to_u8(doc.ground_plane_mode),
            ground_plane_height: doc.ground_plane_height,
            ground_plane_color: doc.ground_plane_color,
            ground_plane_tile_size: doc.ground_plane_tile_size,
            viewport_background: doc.viewport_background,
            projection: projection_to_u8(doc.camera.projection),
            export_path: doc.export_path.clone(),
            export_width: doc.export_width,
            export_height: doc.export_height,
            export_format: export_format_to_u8(doc.export_format),
            export_fps: doc.export_fps,
            camera_track_segment_duration: doc.camera_track_segment_duration,
            saved_views: doc
                .saved_views
                .iter()
                .map(PersistedSavedCameraView::from_saved_view)
                .collect(),
            sweep_config: doc
                .sweep_config
                .iter()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), PersistedParameterSweep::from_sweep(v)))
                        .collect()
                })
                .collect(),
            frame_fields: doc
                .frame_fields
                .iter()
                .map(PersistedFrameField::from_frame_field)
                .collect(),
            frame_attachments: doc
                .frame_attachments
                .iter()
                .map(PersistedFrameAttachment::from_attachment)
                .collect(),
            frame_playback: PersistedFramePlaybackState::from_playback(&doc.frame_playback),
        }
    }

    /// Reconstruct a Document from a snapshot (used for file open).
    pub(crate) fn into_document(self) -> Document {
        let mut doc = Document::new_default();
        let DocumentSnapshot {
            graph,
            plots,
            selected_plot,
            axis_config,
            ground_plane_mode,
            ground_plane_height,
            ground_plane_color,
            ground_plane_tile_size,
            viewport_background,
            projection,
            version: _,
            title,
            export_path,
            export_width,
            export_height,
            export_format,
            export_fps,
            camera_track_segment_duration,
            saved_views,
            sweep_config,
            frame_fields,
            frame_attachments,
            frame_playback,
        } = self;
        if let Some(graph) = graph {
            doc.plots = graph.plots.iter().map(plot_spec_to_plot_entry).collect();
            doc.axis_config = graph.axis_config;
        } else {
            doc.plots = plots
                .iter()
                .map(PersistedPlotEntry::to_plot_entry)
                .collect();
            doc.axis_config = axis_config.to_axis_config();
        }
        doc.normalize_plot_hierarchy();
        doc.selected_plot = selected_plot.filter(|&i| i < doc.plots.len());
        doc.ground_plane_mode = u8_to_ground_plane(ground_plane_mode);
        doc.ground_plane_height = ground_plane_height;
        doc.ground_plane_color = ground_plane_color;
        doc.ground_plane_tile_size = ground_plane_tile_size;
        doc.viewport_background = viewport_background;
        doc.camera.projection = u8_to_projection(projection);
        if !title.is_empty() {
            doc.title = title;
        }
        if !export_path.is_empty() {
            doc.export_path = export_path;
        }
        if export_width > 0 {
            doc.export_width = export_width;
        }
        if export_height > 0 {
            doc.export_height = export_height;
        }
        doc.export_format = u8_to_export_format(export_format);
        doc.export_fps = export_fps.max(1);
        doc.camera_track_segment_duration = camera_track_segment_duration.max(0.1);
        doc.saved_views = saved_views
            .into_iter()
            .map(|view| view.to_saved_view())
            .collect();
        doc.sweep_config = sweep_config
            .into_iter()
            .map(|m| m.into_iter().map(|(k, v)| (k, v.to_sweep())).collect())
            .collect();
        doc.frame_fields = frame_fields
            .into_iter()
            .map(|field| field.to_frame_field())
            .collect();
        doc.frame_attachments = frame_attachments
            .into_iter()
            .map(|attachment| attachment.to_attachment())
            .collect();
        doc.frame_playback = frame_playback.to_playback();
        doc.scene_dirty = true;
        doc
    }

    /// Build a snapshot from the active document in App (used for session save).
    fn from_app(app: &App) -> Self {
        Self::from_document(&app.documents[app.active_document_idx])
    }

    /// Apply this snapshot to the active document in App (used for session restore).
    fn apply_to_app(&self, app: &mut App) {
        let doc = &mut app.documents[app.active_document_idx];
        if let Some(graph) = &self.graph {
            doc.plots = graph.plots.iter().map(plot_spec_to_plot_entry).collect();
            doc.axis_config = graph.axis_config.clone();
        } else {
            doc.plots = self
                .plots
                .iter()
                .map(PersistedPlotEntry::to_plot_entry)
                .collect();
            doc.axis_config = self.axis_config.to_axis_config();
        }
        doc.normalize_plot_hierarchy();
        doc.selected_plot = self.selected_plot.filter(|&i| i < doc.plots.len());
        doc.ground_plane_mode = u8_to_ground_plane(self.ground_plane_mode);
        doc.ground_plane_height = self.ground_plane_height;
        doc.ground_plane_color = self.ground_plane_color;
        doc.ground_plane_tile_size = self.ground_plane_tile_size;
        doc.viewport_background = self.viewport_background;
        doc.camera.projection = u8_to_projection(self.projection);
        if !self.title.is_empty() {
            doc.title = self.title.clone();
        }
        if !self.export_path.is_empty() {
            doc.export_path = self.export_path.clone();
        }
        if self.export_width > 0 {
            doc.export_width = self.export_width;
        }
        if self.export_height > 0 {
            doc.export_height = self.export_height;
        }
        doc.export_format = u8_to_export_format(self.export_format);
        doc.export_fps = self.export_fps.max(1);
        doc.camera_track_segment_duration = self.camera_track_segment_duration.max(0.1);
        doc.saved_views = self
            .saved_views
            .iter()
            .map(PersistedSavedCameraView::to_saved_view)
            .collect();
        doc.sweep_config = self
            .sweep_config
            .iter()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.to_sweep())).collect())
            .collect();
        doc.frame_fields = self
            .frame_fields
            .iter()
            .map(PersistedFrameField::to_frame_field)
            .collect();
        doc.frame_attachments = self
            .frame_attachments
            .iter()
            .map(PersistedFrameAttachment::to_attachment)
            .collect();
        doc.frame_playback = self.frame_playback.to_playback();
        doc.scene_dirty = true;
        doc.export_status.clear();
        doc.export_progress = None;
    }
}

fn plot_spec_to_plot_entry(spec: &PlotSpec) -> PlotEntry {
    PlotEntry {
        plot_id: 0,
        parent_plot_id: None,
        relationship: PlotRelationship::Primary,
        name: spec.name.clone(),
        visible: spec.visible,
        domain: spec.domain.clone(),
        resolution: spec.resolution,
        style: spec.style.clone(),
        kind: spec.definition.clone(),
    }
}

// ---------------------------------------------------------------------------
// PersistedAxisConfig conversions
// ---------------------------------------------------------------------------

impl PersistedAxisConfig {
    fn from_axis_config(config: &AxisConfig) -> Self {
        Self {
            show_box: config.show_box,
            show_labels: config.show_labels,
            show_ticks: config.show_ticks,
            show_grid: config.show_grid,
            labels: config.labels.clone(),
            tick_count: config.tick_count,
        }
    }

    fn to_axis_config(&self) -> AxisConfig {
        let mut config = AxisConfig::default();
        config.show_box = self.show_box;
        config.show_labels = self.show_labels;
        config.show_ticks = self.show_ticks;
        config.show_grid = self.show_grid;
        config.labels = self.labels.clone();
        config.tick_count = self.tick_count;
        config
    }
}

impl PersistedFrameField {
    fn from_frame_field(field: &StoredFrameField) -> Self {
        Self {
            id: field.id,
            title: field.title.clone(),
            source_plot_ids: field.source_plot_ids.clone(),
            source_plot_names: field.source_plot_names.clone(),
            kind: analysis_kind_to_key(field.frame_kind).to_string(),
            samples: field
                .samples
                .iter()
                .map(|sample| PersistedFrameSample {
                    parameter: sample.parameter,
                    position: sample.position,
                    tangent: sample.tangent,
                    normal: sample.normal,
                    binormal: sample.binormal,
                })
                .collect(),
        }
    }

    fn to_frame_field(&self) -> StoredFrameField {
        StoredFrameField {
            id: self.id,
            title: self.title.clone(),
            source_plot_ids: self.source_plot_ids.clone(),
            source_plot_names: self.source_plot_names.clone(),
            frame_kind: analysis_kind_from_key(&self.kind),
            samples: self
                .samples
                .iter()
                .map(|sample| poincare_lib::FrameSample {
                    parameter: sample.parameter,
                    position: sample.position,
                    tangent: sample.tangent,
                    normal: sample.normal,
                    binormal: sample.binormal,
                })
                .collect(),
        }
    }
}

impl PersistedFrameAttachment {
    fn from_attachment(attachment: &FrameAttachment) -> Self {
        Self {
            name: attachment.name.clone(),
            frame_field_id: attachment.frame_field_id,
            kind: frame_attachment_kind_to_key(attachment.kind).to_string(),
            enabled: attachment.enabled,
            scale: attachment.scale,
            camera_distance: attachment.camera_distance,
        }
    }

    fn to_attachment(&self) -> FrameAttachment {
        FrameAttachment {
            name: self.name.clone(),
            frame_field_id: self.frame_field_id,
            kind: frame_attachment_kind_from_key(&self.kind),
            enabled: self.enabled,
            scale: self.scale,
            camera_distance: self.camera_distance,
        }
    }
}

impl PersistedFramePlaybackState {
    fn from_playback(playback: &FramePlaybackState) -> Self {
        Self {
            selected_frame_field: playback.selected_frame_field,
            phase: playback.phase,
            playing: playback.playing,
            speed: playback.speed,
        }
    }

    fn to_playback(&self) -> FramePlaybackState {
        FramePlaybackState {
            selected_frame_field: self.selected_frame_field,
            phase: self.phase,
            playing: self.playing,
            speed: self.speed,
        }
    }
}

fn analysis_kind_to_key(kind: poincare_lib::AnalysisKind) -> &'static str {
    match kind {
        poincare_lib::AnalysisKind::FrenetFrame => "frenet",
        poincare_lib::AnalysisKind::BishopFrame => "bishop",
        poincare_lib::AnalysisKind::DarbouxFrame => "darboux",
        poincare_lib::AnalysisKind::SurfaceAlignedFrame => "surface_aligned",
        _ => "frenet",
    }
}

fn analysis_kind_from_key(key: &str) -> poincare_lib::AnalysisKind {
    match key {
        "bishop" => poincare_lib::AnalysisKind::BishopFrame,
        "darboux" => poincare_lib::AnalysisKind::DarbouxFrame,
        "surface_aligned" => poincare_lib::AnalysisKind::SurfaceAlignedFrame,
        _ => poincare_lib::AnalysisKind::FrenetFrame,
    }
}

fn frame_attachment_kind_to_key(kind: FrameAttachmentKind) -> &'static str {
    match kind {
        FrameAttachmentKind::Marker => "marker",
        FrameAttachmentKind::Triad => "triad",
        FrameAttachmentKind::Camera => "camera",
        FrameAttachmentKind::ProfileRing => "profile_ring",
    }
}

fn frame_attachment_kind_from_key(key: &str) -> FrameAttachmentKind {
    match key {
        "triad" => FrameAttachmentKind::Triad,
        "camera" => FrameAttachmentKind::Camera,
        "profile_ring" => FrameAttachmentKind::ProfileRing,
        _ => FrameAttachmentKind::Marker,
    }
}

// ---------------------------------------------------------------------------
// PersistedPlotEntry conversions
// ---------------------------------------------------------------------------

impl PersistedPlotEntry {
    fn to_plot_entry(&self) -> PlotEntry {
        PlotEntry {
            plot_id: self.plot_id,
            parent_plot_id: self.parent_plot_id,
            relationship: self.relationship.to_plot_relationship(),
            name: self.name.clone(),
            visible: self.visible,
            domain: self.domain.to_domain(),
            resolution: self.resolution.to_resolution(),
            style: self.style.to_plot_style(),
            kind: self.kind.to_plot_kind(),
        }
    }

    fn from_plot_entry(entry: &PlotEntry) -> Self {
        Self {
            plot_id: entry.plot_id,
            parent_plot_id: entry.parent_plot_id,
            relationship: PersistedPlotRelationship::from_plot_relationship(entry.relationship),
            name: entry.name.clone(),
            visible: entry.visible,
            domain: PersistedDomain::from_domain(&entry.domain),
            resolution: PersistedResolution::from_resolution(entry.resolution),
            style: PersistedPlotStyle::from_plot_style(&entry.style),
            kind: PersistedPlotKind::from_plot_kind(&entry.kind),
        }
    }
}

impl PersistedDomain {
    fn from_domain(domain: &Domain) -> Self {
        Self {
            x: [*domain.x.start(), *domain.x.end()],
            y: [*domain.y.start(), *domain.y.end()],
            z: [*domain.z.start(), *domain.z.end()],
        }
    }

    fn to_domain(&self) -> Domain {
        Domain {
            x: self.x[0]..=self.x[1],
            y: self.y[0]..=self.y[1],
            z: self.z[0]..=self.z[1],
        }
    }
}

impl PersistedResolution {
    fn from_resolution(resolution: Resolution) -> Self {
        Self {
            u: resolution.u,
            v: resolution.v,
        }
    }

    fn to_resolution(&self) -> Resolution {
        Resolution {
            u: self.u,
            v: self.v,
        }
    }
}

impl PersistedPlotStyle {
    fn from_plot_style(style: &PlotStyle) -> Self {
        Self {
            colour_mode: PersistedColourMode::from_colour_mode(&style.colour_mode),
            opacity: style.opacity,
            two_sided: style.two_sided,
            line_width: style.line_width,
            point_size: style.point_size,
            glyph_scale: style.glyph_scale,
            glyph_type: glyph_type_to_u8(style.glyph_type),
            shading: shading_to_u8(style.shading),
            tube_radius: style.tube_radius,
            transfer_function: style
                .transfer_function
                .as_ref()
                .map(PersistedTransferFunction::from_transfer_function),
            matcap: style.matcap.map(matcap_source_to_u8),
            param_vis: style
                .param_vis
                .as_ref()
                .map(PersistedParamVis::from_param_vis),
            face_quantity: style.face_quantity.map(surface_face_quantity_to_u8),
            surface_lic: style
                .surface_lic
                .as_ref()
                .map(PersistedSurfaceLic::from_surface_lic),
        }
    }

    fn to_plot_style(&self) -> PlotStyle {
        PlotStyle {
            colour_mode: self.colour_mode.to_colour_mode(),
            opacity: self.opacity,
            two_sided: self.two_sided,
            line_width: self.line_width,
            point_size: self.point_size,
            glyph_scale: self.glyph_scale,
            glyph_type: u8_to_glyph_type(self.glyph_type),
            shading: u8_to_shading(self.shading),
            tube_radius: self.tube_radius,
            transfer_function: self
                .transfer_function
                .as_ref()
                .map(PersistedTransferFunction::to_transfer_function),
            matcap: self
                .matcap
                .map(|m| MatcapSource::Builtin(u8_to_builtin_matcap(m))),
            param_vis: self.param_vis.as_ref().map(PersistedParamVis::to_param_vis),
            face_quantity: self.face_quantity.map(u8_to_surface_face_quantity),
            surface_lic: self
                .surface_lic
                .as_ref()
                .map(PersistedSurfaceLic::to_surface_lic),
        }
    }
}

impl PersistedColourMode {
    fn from_colour_mode(mode: &ColourMode) -> Self {
        match mode {
            ColourMode::Solid(rgba) => Self::Solid(*rgba),
            ColourMode::Colormap {
                colormap,
                scalar_range,
            } => Self::Colormap {
                colormap: colormap_source_to_u8(colormap),
                scalar_range: *scalar_range,
            },
            ColourMode::ByAttribute { name, kind } => Self::ByAttribute {
                name: name.clone(),
                kind: attribute_kind_to_u8(*kind),
            },
        }
    }

    fn to_colour_mode(&self) -> ColourMode {
        match self {
            Self::Solid(rgba) => ColourMode::Solid(*rgba),
            Self::Colormap {
                colormap,
                scalar_range,
            } => ColourMode::Colormap {
                colormap: ColormapSource::Builtin(u8_to_builtin_colormap(*colormap)),
                scalar_range: *scalar_range,
            },
            Self::ByAttribute { name, kind } => ColourMode::ByAttribute {
                name: name.clone(),
                kind: u8_to_attribute_kind(*kind),
            },
        }
    }
}

impl PersistedTransferFunction {
    fn from_transfer_function(transfer: &TransferFunction) -> Self {
        Self {
            opacity_scale: transfer.opacity_scale,
            threshold: transfer.threshold,
        }
    }

    fn to_transfer_function(&self) -> TransferFunction {
        TransferFunction {
            opacity_scale: self.opacity_scale,
            threshold: self.threshold,
        }
    }
}

impl PersistedSavedCameraView {
    fn from_saved_view(view: &SavedCameraView) -> Self {
        Self {
            name: view.name.clone(),
            center: view.camera.center.to_array(),
            distance: view.camera.distance,
            orientation: view.camera.orientation.to_array(),
            projection: projection_to_u8(view.camera.projection),
            fov_y: view.camera.fov_y,
        }
    }

    fn to_saved_view(&self) -> SavedCameraView {
        let mut camera = viewport_lib::Camera::default();
        camera.center = glam::Vec3::from_array(self.center);
        camera.set_distance(self.distance);
        camera.set_orientation(glam::Quat::from_array(self.orientation));
        camera.projection = u8_to_projection(self.projection);
        camera.fov_y = self.fov_y;
        SavedCameraView {
            name: self.name.clone(),
            camera,
        }
    }
}

impl PersistedParamVis {
    fn from_param_vis(param_vis: &ParamVisSettings) -> Self {
        Self {
            mode: param_vis_mode_to_u8(param_vis.mode),
            scale: param_vis.scale,
        }
    }

    fn to_param_vis(&self) -> ParamVisSettings {
        ParamVisSettings {
            mode: u8_to_param_vis_mode(self.mode),
            scale: self.scale,
        }
    }
}

impl PersistedSurfaceLic {
    fn from_surface_lic(surface_lic: &SurfaceLicSettings) -> Self {
        Self {
            vector_field: surface_lic_vector_field_to_u8(surface_lic.vector_field),
            steps: surface_lic.steps,
            step_size: surface_lic.step_size,
            strength: surface_lic.strength,
        }
    }

    fn to_surface_lic(&self) -> SurfaceLicSettings {
        SurfaceLicSettings {
            vector_field: u8_to_surface_lic_vector_field(self.vector_field),
            steps: self.steps,
            step_size: self.step_size,
            strength: self.strength,
        }
    }
}

impl PersistedSeedMode {
    fn from_seed_mode(mode: &SeedMode) -> Self {
        match mode {
            SeedMode::Grid { nx, ny, nz } => Self::Grid {
                nx: *nx,
                ny: *ny,
                nz: *nz,
            },
            SeedMode::Plane { axis, offset } => Self::Plane {
                axis: *axis,
                offset: *offset,
            },
            SeedMode::ManualCsv { csv_text } => Self::ManualCsv {
                csv_text: csv_text.clone(),
            },
        }
    }

    fn to_seed_mode(&self) -> SeedMode {
        match self {
            Self::Grid { nx, ny, nz } => SeedMode::Grid {
                nx: *nx,
                ny: *ny,
                nz: *nz,
            },
            Self::Plane { axis, offset } => SeedMode::Plane {
                axis: *axis,
                offset: *offset,
            },
            Self::ManualCsv { csv_text } => SeedMode::ManualCsv {
                csv_text: csv_text.clone(),
            },
        }
    }
}

impl PersistedPlotKind {
    fn from_plot_kind(kind: &PlotKind) -> Self {
        match kind {
            PlotKind::ContouredSurface {
                contour_values,
                contour_style,
            } => Self::ContouredSurface {
                contour_values: contour_values.clone(),
                contour_style: PersistedPlotStyle::from_plot_style(contour_style),
            },
            PlotKind::SphericalHarmonic => Self::SphericalHarmonic,
            PlotKind::HelixCurve => Self::HelixCurve,
            PlotKind::ScatterCloud => Self::ScatterCloud,
            PlotKind::VectorField => Self::VectorField,
            PlotKind::GridSurface => Self::GridSurface,
            PlotKind::Streamlines { seeds } => Self::Streamlines {
                seeds: seeds.clone(),
            },
            PlotKind::VolumeRender { resolution } => Self::VolumeRender {
                resolution: *resolution,
            },
            PlotKind::Isosurface {
                isovalues,
                resolution,
            } => Self::Isosurface {
                isovalues: isovalues.clone(),
                resolution: *resolution,
            },
            PlotKind::ExprCartesian {
                expression,
                parameters,
            } => Self::ExprCartesian {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprCurve {
                expression,
                parameters,
                t_range,
            } => Self::ExprCurve {
                expression: expression.clone(),
                parameters: parameters.clone(),
                t_range: *t_range,
            },
            PlotKind::ExprCartesianLine {
                dep_var,
                ind_var,
                expression,
                parameters,
            } => Self::ExprCartesianLine {
                dep_var: dep_var.clone(),
                ind_var: ind_var.clone(),
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprSpherical {
                expression,
                parameters,
            } => Self::ExprSpherical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprCylindrical {
                expression,
                parameters,
            } => Self::ExprCylindrical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprPolar {
                expression,
                parameters,
            } => Self::ExprPolar {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprParametricSurface {
                expression,
                parameters,
            } => Self::ExprParametricSurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ImportedTable { definition } => Self::ImportedTable {
                definition: definition.clone(),
            },
            PlotKind::ScalarSlice {
                expression,
                parameters,
                axis,
                position,
                contour_values,
                contour_style,
            } => Self::ScalarSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: slice_axis_to_u8(*axis),
                position: *position,
                contour_values: contour_values.clone(),
                contour_style: PersistedPlotStyle::from_plot_style(contour_style),
            },
            PlotKind::VectorSlice {
                expression,
                parameters,
                axis,
                position,
            } => Self::VectorSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: slice_axis_to_u8(*axis),
                position: *position,
            },
            PlotKind::GradientField {
                expression,
                parameters,
            } => Self::GradientField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::DivergenceField {
                expression,
                parameters,
                vol_resolution,
            } => Self::DivergenceField {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::CurlField {
                expression,
                parameters,
            } => Self::CurlField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::PointAnnotations {
                points,
                show_labels,
            } => Self::PointAnnotations {
                points: points.clone(),
                show_labels: *show_labels,
            },
            PlotKind::ArrowAnnotations {
                arrows,
                show_labels,
            } => Self::ArrowAnnotations {
                arrows: arrows.clone(),
                show_labels: *show_labels,
            },
            PlotKind::DerivedSurfaceMesh {
                positions,
                indices,
                values,
                value_name,
            } => Self::DerivedSurfaceMesh {
                positions: positions.clone(),
                indices: indices.clone(),
                values: values.clone(),
                value_name: value_name.clone(),
            },
            PlotKind::DerivedPolylineGroups { groups } => Self::DerivedPolylineGroups {
                groups: groups.clone(),
            },
            PlotKind::InterpolatedCurve {
                points,
                interpolation,
            } => Self::InterpolatedCurve {
                points: points.clone(),
                interpolation: PersistedCurveInterpolation::from_curve_interpolation(
                    *interpolation,
                ),
            },
            PlotKind::ExprVectorField {
                expression,
                parameters,
            } => Self::ExprVectorField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprVolume {
                expression,
                parameters,
                vol_resolution,
            } => Self::ExprVolume {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::ExprIsosurface {
                expression,
                parameters,
                isovalues,
                iso_colours,
                vol_resolution,
            } => Self::ExprIsosurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
                isovalues: isovalues.clone(),
                iso_colours: iso_colours.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::ExprStreamlines {
                expression,
                parameters,
                seed_mode,
                step_size,
                max_steps,
            } => Self::ExprStreamlines {
                expression: expression.clone(),
                parameters: parameters.clone(),
                seed_mode: PersistedSeedMode::from_seed_mode(seed_mode),
                step_size: *step_size,
                max_steps: *max_steps,
            },
        }
    }

    fn to_plot_kind(&self) -> PlotKind {
        match self {
            Self::ContouredSurface {
                contour_values,
                contour_style,
            } => PlotKind::ContouredSurface {
                contour_values: contour_values.clone(),
                contour_style: contour_style.to_plot_style(),
            },
            Self::SphericalHarmonic => PlotKind::SphericalHarmonic,
            Self::HelixCurve => PlotKind::HelixCurve,
            Self::ScatterCloud => PlotKind::ScatterCloud,
            Self::VectorField => PlotKind::VectorField,
            Self::GridSurface => PlotKind::GridSurface,
            Self::Streamlines { seeds } => PlotKind::Streamlines {
                seeds: seeds.clone(),
            },
            Self::VolumeRender { resolution } => PlotKind::VolumeRender {
                resolution: *resolution,
            },
            Self::Isosurface {
                isovalues,
                resolution,
            } => PlotKind::Isosurface {
                isovalues: isovalues.clone(),
                resolution: *resolution,
            },
            Self::ExprCartesian {
                expression,
                parameters,
            } => PlotKind::ExprCartesian {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprCurve {
                expression,
                parameters,
                t_range,
            } => PlotKind::ExprCurve {
                expression: expression.clone(),
                parameters: parameters.clone(),
                t_range: *t_range,
            },
            Self::ExprCartesianLine {
                dep_var,
                ind_var,
                expression,
                parameters,
            } => PlotKind::ExprCartesianLine {
                dep_var: dep_var.clone(),
                ind_var: ind_var.clone(),
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprSpherical {
                expression,
                parameters,
            } => PlotKind::ExprSpherical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprCylindrical {
                expression,
                parameters,
            } => PlotKind::ExprCylindrical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprPolar {
                expression,
                parameters,
            } => PlotKind::ExprPolar {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprParametricSurface {
                expression,
                parameters,
            } => PlotKind::ExprParametricSurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ImportedTable { definition } => PlotKind::ImportedTable {
                definition: definition.clone(),
            },
            Self::ScalarSlice {
                expression,
                parameters,
                axis,
                position,
                contour_values,
                contour_style,
            } => PlotKind::ScalarSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: load_slice_axis(*axis),
                position: *position,
                contour_values: contour_values.clone(),
                contour_style: contour_style.to_plot_style(),
            },
            Self::VectorSlice {
                expression,
                parameters,
                axis,
                position,
            } => PlotKind::VectorSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: load_slice_axis(*axis),
                position: *position,
            },
            Self::GradientField {
                expression,
                parameters,
            } => PlotKind::GradientField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::DivergenceField {
                expression,
                parameters,
                vol_resolution,
            } => PlotKind::DivergenceField {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            Self::CurlField {
                expression,
                parameters,
            } => PlotKind::CurlField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::PointAnnotations {
                points,
                show_labels,
            } => PlotKind::PointAnnotations {
                points: points.clone(),
                show_labels: *show_labels,
            },
            Self::ArrowAnnotations {
                arrows,
                show_labels,
            } => PlotKind::ArrowAnnotations {
                arrows: arrows.clone(),
                show_labels: *show_labels,
            },
            Self::DerivedSurfaceMesh {
                positions,
                indices,
                values,
                value_name,
            } => PlotKind::DerivedSurfaceMesh {
                positions: positions.clone(),
                indices: indices.clone(),
                values: values.clone(),
                value_name: value_name.clone(),
            },
            Self::DerivedPolylineGroups { groups } => PlotKind::DerivedPolylineGroups {
                groups: groups.clone(),
            },
            Self::InterpolatedCurve {
                points,
                interpolation,
            } => PlotKind::InterpolatedCurve {
                points: points.clone(),
                interpolation: interpolation.to_curve_interpolation(),
            },
            Self::ExprVectorField {
                expression,
                parameters,
            } => PlotKind::ExprVectorField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            Self::ExprVolume {
                expression,
                parameters,
                vol_resolution,
            } => PlotKind::ExprVolume {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            Self::ExprIsosurface {
                expression,
                parameters,
                isovalues,
                iso_colours,
                vol_resolution,
            } => PlotKind::ExprIsosurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
                isovalues: isovalues.clone(),
                iso_colours: iso_colours.clone(),
                vol_resolution: *vol_resolution,
            },
            Self::ExprStreamlines {
                expression,
                parameters,
                seed_mode,
                step_size,
                max_steps,
            } => PlotKind::ExprStreamlines {
                expression: expression.clone(),
                parameters: parameters.clone(),
                seed_mode: seed_mode.to_seed_mode(),
                step_size: *step_size,
                max_steps: *max_steps,
            },
        }
    }
}

impl PersistedPlotRelationship {
    fn from_plot_relationship(relationship: PlotRelationship) -> Self {
        match relationship {
            PlotRelationship::Primary => Self::Primary,
            PlotRelationship::DerivedAnalysis => Self::DerivedAnalysis,
        }
    }

    fn to_plot_relationship(&self) -> PlotRelationship {
        match self {
            Self::Primary => PlotRelationship::Primary,
            Self::DerivedAnalysis => PlotRelationship::DerivedAnalysis,
        }
    }
}

// ---------------------------------------------------------------------------
// serde default helpers for DocumentSnapshot fields
// ---------------------------------------------------------------------------

fn default_ground_plane_color() -> [f32; 4] {
    [0.3, 0.3, 0.3, 1.0]
}

fn default_ground_plane_tile_size() -> f32 {
    1.0
}

fn default_viewport_background() -> [f32; 4] {
    [18.0 / 255.0, 18.0 / 255.0, 18.0 / 255.0, 1.0]
}

fn default_export_path() -> String {
    crate::document::default_export_path_for_format(ExportFormat::Png)
        .to_string_lossy()
        .into_owned()
}

fn default_export_width() -> u32 {
    1600
}

fn default_export_height() -> u32 {
    1000
}

fn default_export_fps() -> u32 {
    24
}

fn default_camera_track_segment_duration() -> f32 {
    2.5
}

fn default_panel_header_bg() -> [u8; 4] {
    PersistedAppSettings::default().panel_header_bg
}

fn default_panel_content_bg() -> [u8; 4] {
    PersistedAppSettings::default().panel_content_bg
}

fn default_tab_selected_bg() -> [u8; 4] {
    PersistedAppSettings::default().tab_selected_bg
}

fn default_tab_highlight() -> [u8; 4] {
    PersistedAppSettings::default().tab_highlight
}

// ---------------------------------------------------------------------------
// Enum/u8 conversion helpers (unchanged)
// ---------------------------------------------------------------------------

fn builtin_colormap_to_u8(value: BuiltinColourmap) -> u8 {
    match value {
        BuiltinColourmap::Viridis => 0,
        BuiltinColourmap::Plasma => 1,
        BuiltinColourmap::Greyscale => 2,
        BuiltinColourmap::Coolwarm => 3,
        BuiltinColourmap::Rainbow => 4,
        BuiltinColourmap::Magma => 5,
        BuiltinColourmap::Inferno => 6,
        BuiltinColourmap::Turbo => 7,
        BuiltinColourmap::Jet => 8,
        BuiltinColourmap::RdBu => 9,
    }
}

fn u8_to_builtin_colormap(value: u8) -> BuiltinColourmap {
    match value {
        0 => BuiltinColourmap::Viridis,
        1 => BuiltinColourmap::Plasma,
        2 => BuiltinColourmap::Greyscale,
        3 => BuiltinColourmap::Coolwarm,
        4 => BuiltinColourmap::Rainbow,
        5 => BuiltinColourmap::Magma,
        6 => BuiltinColourmap::Inferno,
        7 => BuiltinColourmap::Turbo,
        8 => BuiltinColourmap::Jet,
        9 => BuiltinColourmap::RdBu,
        _ => BuiltinColourmap::Viridis,
    }
}

fn ground_plane_to_u8(value: GroundPlaneMode) -> u8 {
    match value {
        GroundPlaneMode::None => 0,
        GroundPlaneMode::ShadowOnly => 1,
        GroundPlaneMode::Tile => 2,
        GroundPlaneMode::SolidColour => 3,
    }
}

fn u8_to_ground_plane(value: u8) -> GroundPlaneMode {
    match value {
        1 => GroundPlaneMode::ShadowOnly,
        2 => GroundPlaneMode::Tile,
        3 => GroundPlaneMode::SolidColour,
        _ => GroundPlaneMode::None,
    }
}

fn projection_to_u8(value: Projection) -> u8 {
    match value {
        Projection::Perspective => 0,
        Projection::Orthographic => 1,
        _ => 0,
    }
}

fn u8_to_projection(value: u8) -> Projection {
    match value {
        1 => Projection::Orthographic,
        _ => Projection::Perspective,
    }
}

fn export_format_to_u8(value: ExportFormat) -> u8 {
    match value {
        ExportFormat::Png => 0,
        ExportFormat::Gif => 1,
        ExportFormat::Mp4 => 2,
    }
}

fn u8_to_export_format(value: u8) -> ExportFormat {
    match value {
        1 => ExportFormat::Gif,
        2 => ExportFormat::Mp4,
        _ => ExportFormat::Png,
    }
}

fn slice_axis_to_u8(axis: SliceAxis) -> u8 {
    match axis {
        SliceAxis::X => 0,
        SliceAxis::Y => 1,
        SliceAxis::Z => 2,
    }
}

fn colormap_source_to_u8(value: &ColormapSource) -> u8 {
    match value {
        ColormapSource::Builtin(value) => builtin_colormap_to_u8(*value),
        ColormapSource::Uploaded(_) => builtin_colormap_to_u8(BuiltinColourmap::Viridis),
    }
}

fn attribute_kind_to_u8(value: AttributeKind) -> u8 {
    match value {
        AttributeKind::Vertex => 0,
        AttributeKind::Cell => 1,
        AttributeKind::Face => 2,
        AttributeKind::FaceColour => 3,
        AttributeKind::Edge => 4,
        AttributeKind::Halfedge => 5,
        AttributeKind::Corner => 6,
    }
}

fn u8_to_attribute_kind(value: u8) -> AttributeKind {
    match value {
        1 => AttributeKind::Cell,
        2 => AttributeKind::Face,
        3 => AttributeKind::FaceColour,
        4 => AttributeKind::Edge,
        5 => AttributeKind::Halfedge,
        6 => AttributeKind::Corner,
        _ => AttributeKind::Vertex,
    }
}

fn shading_to_u8(value: ShadingMode) -> u8 {
    match value {
        ShadingMode::Flat => 0,
        ShadingMode::Smooth => 1,
        ShadingMode::Unlit => 2,
    }
}

fn u8_to_shading(value: u8) -> ShadingMode {
    match value {
        0 => ShadingMode::Flat,
        2 => ShadingMode::Unlit,
        _ => ShadingMode::Smooth,
    }
}

fn glyph_type_to_u8(value: GlyphType) -> u8 {
    match value {
        GlyphType::Arrow => 0,
        GlyphType::Sphere => 1,
        GlyphType::Cube => 2,
    }
}

fn u8_to_glyph_type(value: u8) -> GlyphType {
    match value {
        1 => GlyphType::Sphere,
        2 => GlyphType::Cube,
        _ => GlyphType::Arrow,
    }
}

fn matcap_source_to_u8(value: MatcapSource) -> u8 {
    match value {
        MatcapSource::Builtin(value) => builtin_matcap_to_u8(value),
    }
}

fn builtin_matcap_to_u8(value: BuiltinMatcap) -> u8 {
    match value {
        BuiltinMatcap::Clay => 0,
        BuiltinMatcap::Wax => 1,
        BuiltinMatcap::Candy => 2,
        BuiltinMatcap::Flat => 3,
        BuiltinMatcap::Ceramic => 4,
        BuiltinMatcap::Jade => 5,
        BuiltinMatcap::Mud => 6,
        BuiltinMatcap::Normal => 7,
    }
}

fn u8_to_builtin_matcap(value: u8) -> BuiltinMatcap {
    match value {
        0 => BuiltinMatcap::Clay,
        1 => BuiltinMatcap::Wax,
        2 => BuiltinMatcap::Candy,
        3 => BuiltinMatcap::Flat,
        4 => BuiltinMatcap::Ceramic,
        5 => BuiltinMatcap::Jade,
        6 => BuiltinMatcap::Mud,
        7 => BuiltinMatcap::Normal,
        _ => BuiltinMatcap::Clay,
    }
}

fn param_vis_mode_to_u8(value: ParamVisMode) -> u8 {
    match value {
        ParamVisMode::Checker => 0,
        ParamVisMode::Grid => 1,
        ParamVisMode::LocalChecker => 2,
        ParamVisMode::LocalRadial => 3,
    }
}

fn u8_to_param_vis_mode(value: u8) -> ParamVisMode {
    match value {
        1 => ParamVisMode::Grid,
        2 => ParamVisMode::LocalChecker,
        3 => ParamVisMode::LocalRadial,
        _ => ParamVisMode::Checker,
    }
}

fn surface_face_quantity_to_u8(value: SurfaceFaceQuantity) -> u8 {
    match value {
        SurfaceFaceQuantity::AngleDistortion => 0,
        SurfaceFaceQuantity::AreaDistortion => 1,
    }
}

fn u8_to_surface_face_quantity(value: u8) -> SurfaceFaceQuantity {
    match value {
        1 => SurfaceFaceQuantity::AreaDistortion,
        _ => SurfaceFaceQuantity::AngleDistortion,
    }
}

fn surface_lic_vector_field_to_u8(value: SurfaceLicVectorField) -> u8 {
    match value {
        SurfaceLicVectorField::TangentU => 0,
        SurfaceLicVectorField::TangentV => 1,
        SurfaceLicVectorField::Diagonal => 2,
        SurfaceLicVectorField::Saddle => 3,
    }
}

fn u8_to_surface_lic_vector_field(value: u8) -> SurfaceLicVectorField {
    match value {
        1 => SurfaceLicVectorField::TangentV,
        2 => SurfaceLicVectorField::Diagonal,
        3 => SurfaceLicVectorField::Saddle,
        _ => SurfaceLicVectorField::TangentU,
    }
}

fn color32_to_rgba(color: eframe::egui::Color32) -> [u8; 4] {
    color.to_srgba_unmultiplied()
}

fn rgba_to_color32(rgba: [u8; 4]) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::entry::{PlotEntry, PlotRelationship};
    use crate::plot::kind::PlotKind;
    use poincare_lib::{Domain, PlotStyle, Resolution};

    fn sample_document() -> Document {
        let mut doc = Document::new_default();
        doc.title = "Saved Project".to_string();
        doc.selected_plot = Some(0);
        doc.axis_config.show_grid = true;
        doc.camera.projection = Projection::Orthographic;
        doc.export_width = 1920;
        doc.export_height = 1080;
        doc.plots.push(PlotEntry {
            plot_id: 12,
            parent_plot_id: None,
            relationship: PlotRelationship::Primary,
            name: "wave".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 64, v: 48 },
            style: PlotStyle::default(),
            kind: PlotKind::ExprCartesian {
                expression: "sin(x * y)".to_string(),
                parameters: vec![("a".to_string(), 1.5)],
            },
        });
        doc.saved_views.push(SavedCameraView {
            name: "View 1".to_string(),
            camera: doc.camera.clone(),
        });
        doc
    }

    #[test]
    fn document_snapshot_round_trips_core_state() {
        let snapshot = DocumentSnapshot::from_document(&sample_document());
        let doc = snapshot.into_document();

        assert_eq!(doc.title, "Saved Project");
        assert_eq!(doc.selected_plot, Some(0));
        assert!(doc.axis_config.show_grid);
        assert_eq!(doc.camera.projection, Projection::Orthographic);
        assert_eq!(doc.export_width, 1920);
        assert_eq!(doc.export_height, 1080);
        assert_eq!(doc.saved_views.len(), 1);
        assert_eq!(doc.plots.len(), 1);
        assert_eq!(doc.plots[0].plot_id, 12);
        assert_eq!(doc.plots[0].resolution.u, 64);
        assert_eq!(doc.plots[0].resolution.v, 48);
        match &doc.plots[0].kind {
            PlotKind::ExprCartesian {
                expression,
                parameters,
            } => {
                assert_eq!(expression, "sin(x * y)");
                assert_eq!(parameters, &vec![("a".to_string(), 1.5)]);
            }
            _ => panic!("expected Cartesian expression plot"),
        }
    }

    #[test]
    fn project_file_save_load_round_trips_snapshot() {
        let path = std::env::temp_dir().join(format!(
            "poincare-save-load-test-{}.poincare.json",
            std::process::id()
        ));
        let doc = sample_document();

        save_document_to_path(&doc, &path).expect("save project");
        let loaded = load_document_from_path(&path)
            .expect("load project")
            .into_document();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.title, doc.title);
        assert_eq!(loaded.selected_plot, doc.selected_plot);
        assert_eq!(loaded.plots.len(), doc.plots.len());
        assert_eq!(loaded.plots[0].name, doc.plots[0].name);
    }

    #[test]
    fn app_state_accepts_empty_blob_defaults() {
        let saved: PersistedAppState =
            serde_json::from_str("{}").expect("empty app state should use defaults");

        assert!(saved.session.is_none());
        assert!(!saved.settings.save_state_on_exit);
        assert_eq!(
            u8_to_builtin_colormap(saved.settings.default_colormap),
            BuiltinColourmap::Viridis
        );
    }
}
