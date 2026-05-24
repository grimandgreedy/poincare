use std::collections::HashMap;

use eframe::Storage;
use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, CurveInterpolation, CurveInterpolationKind, Domain,
    GlyphType, GraphSpec, MatcapSource, ParamVisSettings, PlotSpec, PlotStyle, Resolution,
    ShadingMode, SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField,
    TransferFunction,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use viewport_lib::{
    AttributeKind, BuiltinColourmap, BuiltinMatcap, GroundPlaneMode, ParamVisMode, Projection,
};

use crate::App;
use crate::document::{Document, ExportFormat, SavedCameraView};
use crate::plot::analysis::{ArrowAnnotation, PointAnnotation, SliceAxis};
use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, SeedMode};
use crate::plot::sweep::ParameterSweep;
use crate::plot::table::TableImportDefinition;

const APP_STATE_KEY: &str = "poincare_app_v2_state";

// ---------------------------------------------------------------------------
// Top-level eframe storage blob
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct PersistedAppState {
    settings: PersistedAppSettings,
    session: Option<DocumentSnapshot>,
}

// ---------------------------------------------------------------------------
// App-level settings (panel colours, preferences — never written to project
// files, never read from them)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct PersistedAppSettings {
    panel_header_bg: [u8; 4],
    panel_content_bg: [u8; 4],
    tab_selected_bg: [u8; 4],
    tab_highlight: [u8; 4],
    default_colormap: u8,
    #[serde(default)]
    invert_scroll: bool,
    save_state_on_exit: bool,
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

fn default_true() -> bool {
    true
}

fn load_slice_axis(axis: u8) -> SliceAxis {
    match axis {
        0 => SliceAxis::X,
        1 => SliceAxis::Y,
        _ => SliceAxis::Z,
    }
}

// ---------------------------------------------------------------------------
// Document snapshot — serializable form of one Document.
// Used for:
//   • eframe session restore (stored inside PersistedAppState.session)
//   • project file save/load via save_document_to_path / load_document_from_path
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
    name: String,
    visible: bool,
    domain: PersistedDomain,
    resolution: PersistedResolution,
    style: PersistedPlotStyle,
    kind: PersistedPlotKind,
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
    fn to_curve_interpolation(self) -> CurveInterpolation {
        CurveInterpolation {
            kind: match self.kind {
                PersistedCurveInterpolationKind::Linear => CurveInterpolationKind::Linear,
                PersistedCurveInterpolationKind::CatmullRom => CurveInterpolationKind::CatmullRom,
                PersistedCurveInterpolationKind::CentripetalCatmullRom => {
                    CurveInterpolationKind::CentripetalCatmullRom
                }
                PersistedCurveInterpolationKind::MovingAverage => CurveInterpolationKind::MovingAverage,
                PersistedCurveInterpolationKind::SavitzkyGolay => CurveInterpolationKind::SavitzkyGolay,
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
            graph: Some(doc.graph_spec()),
            plots: Vec::new(),
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
        } = self;
        if let Some(graph) = graph {
            doc.plots = graph.plots.iter().map(plot_spec_to_plot_entry).collect();
            doc.axis_config = graph.axis_config;
        } else {
            doc.plots = plots.iter().map(PersistedPlotEntry::to_plot_entry).collect();
            doc.axis_config = axis_config.to_axis_config();
        }
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
        doc.scene_dirty = true;
        doc.export_status.clear();
        doc.export_progress = None;
    }
}

fn plot_spec_to_plot_entry(spec: &PlotSpec) -> PlotEntry {
    PlotEntry {
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

// ---------------------------------------------------------------------------
// PersistedPlotEntry conversions
// ---------------------------------------------------------------------------

impl PersistedPlotEntry {
    fn to_plot_entry(&self) -> PlotEntry {
        PlotEntry {
            name: self.name.clone(),
            visible: self.visible,
            domain: self.domain.to_domain(),
            resolution: self.resolution.to_resolution(),
            style: self.style.to_plot_style(),
            kind: self.kind.to_plot_kind(),
        }
    }
}

impl PersistedDomain {
    fn to_domain(&self) -> Domain {
        Domain {
            x: self.x[0]..=self.x[1],
            y: self.y[0]..=self.y[1],
            z: self.z[0]..=self.z[1],
        }
    }
}

impl PersistedResolution {
    fn to_resolution(&self) -> Resolution {
        Resolution {
            u: self.u,
            v: self.v,
        }
    }
}

impl PersistedPlotStyle {
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
    fn to_param_vis(&self) -> ParamVisSettings {
        ParamVisSettings {
            mode: u8_to_param_vis_mode(self.mode),
            scale: self.scale,
        }
    }
}

impl PersistedSurfaceLic {
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

// ---------------------------------------------------------------------------
// Enum ↔ u8 helpers (unchanged)
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

fn u8_to_shading(value: u8) -> ShadingMode {
    match value {
        0 => ShadingMode::Flat,
        2 => ShadingMode::Unlit,
        _ => ShadingMode::Smooth,
    }
}

fn u8_to_glyph_type(value: u8) -> GlyphType {
    match value {
        1 => GlyphType::Sphere,
        2 => GlyphType::Cube,
        _ => GlyphType::Arrow,
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

fn u8_to_param_vis_mode(value: u8) -> ParamVisMode {
    match value {
        1 => ParamVisMode::Grid,
        2 => ParamVisMode::LocalChecker,
        3 => ParamVisMode::LocalRadial,
        _ => ParamVisMode::Checker,
    }
}

fn u8_to_surface_face_quantity(value: u8) -> SurfaceFaceQuantity {
    match value {
        1 => SurfaceFaceQuantity::AreaDistortion,
        _ => SurfaceFaceQuantity::AngleDistortion,
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
