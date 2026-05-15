use std::collections::HashMap;

use eframe::Storage;
use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, Domain, MatcapSource, ParamVisSettings, PlotStyle,
    Resolution, ShadingMode, SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField,
    TransferFunction,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use viewport_lib::{
    AttributeKind, BuiltinColormap, BuiltinMatcap, GroundPlaneMode, ParamVisMode, Projection,
};

use crate::App;
use crate::document::Document;
use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, SeedMode};
use crate::plot::sweep::ParameterSweep;

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
            speed: self.speed,
            playing: self.playing,
            phase: self.phase,
            direction: self.direction,
        }
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
    // Plot content
    #[serde(default)]
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
    ExprDataGrid {
        csv_text: String,
    },
    ExprCurvePoints {
        csv_text: String,
    },
    ExprScatter {
        csv_text: String,
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
            version: 1,
            title: doc.title.clone(),
            plots: doc.plots.iter().map(PersistedPlotEntry::from_plot_entry).collect(),
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
        doc.plots = self.plots.iter().map(PersistedPlotEntry::to_plot_entry).collect();
        doc.selected_plot = self.selected_plot.filter(|&i| i < doc.plots.len());
        doc.axis_config = self.axis_config.to_axis_config();
        doc.ground_plane_mode = u8_to_ground_plane(self.ground_plane_mode);
        doc.ground_plane_height = self.ground_plane_height;
        doc.ground_plane_color = self.ground_plane_color;
        doc.ground_plane_tile_size = self.ground_plane_tile_size;
        doc.viewport_background = self.viewport_background;
        doc.camera.projection = u8_to_projection(self.projection);
        if !self.title.is_empty() { doc.title = self.title; }
        if !self.export_path.is_empty() { doc.export_path = self.export_path; }
        if self.export_width > 0 { doc.export_width = self.export_width; }
        if self.export_height > 0 { doc.export_height = self.export_height; }
        doc.sweep_config = self
            .sweep_config
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
        doc.plots = self.plots.iter().map(PersistedPlotEntry::to_plot_entry).collect();
        doc.selected_plot = self.selected_plot.filter(|&i| i < doc.plots.len());
        doc.axis_config = self.axis_config.to_axis_config();
        doc.ground_plane_mode = u8_to_ground_plane(self.ground_plane_mode);
        doc.ground_plane_height = self.ground_plane_height;
        doc.ground_plane_color = self.ground_plane_color;
        doc.ground_plane_tile_size = self.ground_plane_tile_size;
        doc.viewport_background = self.viewport_background;
        doc.camera.projection = u8_to_projection(self.projection);
        if !self.title.is_empty() { doc.title = self.title.clone(); }
        if !self.export_path.is_empty() { doc.export_path = self.export_path.clone(); }
        if self.export_width > 0 { doc.export_width = self.export_width; }
        if self.export_height > 0 { doc.export_height = self.export_height; }
        doc.sweep_config = self
            .sweep_config
            .iter()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.to_sweep())).collect())
            .collect();
        doc.scene_dirty = true;
        doc.export_status.clear();
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
    fn from_plot_entry(entry: &PlotEntry) -> Self {
        Self {
            name: entry.name.clone(),
            visible: entry.visible,
            domain: PersistedDomain::from_domain(&entry.domain),
            resolution: PersistedResolution::from_resolution(entry.resolution),
            style: PersistedPlotStyle::from_plot_style(&entry.style),
            kind: PersistedPlotKind::from_plot_kind(&entry.kind),
        }
    }

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
            shading: shading_to_u8(style.shading),
            tube_radius: style.tube_radius,
            transfer_function: style
                .transfer_function
                .as_ref()
                .map(PersistedTransferFunction::from_transfer_function),
            matcap: style.matcap.map(|m| {
                builtin_matcap_to_u8(match m {
                    MatcapSource::Builtin(preset) => preset,
                })
            }),
            param_vis: style.param_vis.map(PersistedParamVis::from_param_vis),
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
                colormap: match colormap {
                    ColormapSource::Builtin(preset) => builtin_colormap_to_u8(*preset),
                    ColormapSource::Uploaded(_) => builtin_colormap_to_u8(BuiltinColormap::Viridis),
                },
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
    fn from_transfer_function(tf: &TransferFunction) -> Self {
        Self {
            opacity_scale: tf.opacity_scale,
            threshold: tf.threshold,
        }
    }

    fn to_transfer_function(&self) -> TransferFunction {
        TransferFunction {
            opacity_scale: self.opacity_scale,
            threshold: self.threshold,
        }
    }
}

impl PersistedParamVis {
    fn from_param_vis(param_vis: ParamVisSettings) -> Self {
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
    fn from_surface_lic(lic: &SurfaceLicSettings) -> Self {
        Self {
            vector_field: surface_lic_vector_field_to_u8(lic.vector_field),
            steps: lic.steps,
            step_size: lic.step_size,
            strength: lic.strength,
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
                seeds: seeds.iter().map(|s| s.to_array()).collect(),
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
            PlotKind::ExprDataGrid { csv_text, .. } => Self::ExprDataGrid {
                csv_text: csv_text.clone(),
            },
            PlotKind::ExprCurvePoints { csv_text, .. } => Self::ExprCurvePoints {
                csv_text: csv_text.clone(),
            },
            PlotKind::ExprScatter { csv_text, .. } => Self::ExprScatter {
                csv_text: csv_text.clone(),
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
                seeds: seeds.iter().map(|s| glam::Vec3::from_array(*s)).collect(),
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
            Self::ExprDataGrid { csv_text } => PlotKind::ExprDataGrid {
                csv_text: csv_text.clone(),
                parse_error: String::new(),
            },
            Self::ExprCurvePoints { csv_text } => PlotKind::ExprCurvePoints {
                csv_text: csv_text.clone(),
                parse_error: String::new(),
            },
            Self::ExprScatter { csv_text } => PlotKind::ExprScatter {
                csv_text: csv_text.clone(),
                parse_error: String::new(),
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
    "poincare-export.png".to_string()
}

fn default_export_width() -> u32 {
    1600
}

fn default_export_height() -> u32 {
    1000
}

// ---------------------------------------------------------------------------
// Enum ↔ u8 helpers (unchanged)
// ---------------------------------------------------------------------------

fn builtin_colormap_to_u8(value: BuiltinColormap) -> u8 {
    match value {
        BuiltinColormap::Viridis => 0,
        BuiltinColormap::Plasma => 1,
        BuiltinColormap::Greyscale => 2,
        BuiltinColormap::Coolwarm => 3,
        BuiltinColormap::Rainbow => 4,
        BuiltinColormap::Magma => 5,
        BuiltinColormap::Inferno => 6,
        BuiltinColormap::Turbo => 7,
        BuiltinColormap::Jet => 8,
        BuiltinColormap::RdBu => 9,
    }
}

fn u8_to_builtin_colormap(value: u8) -> BuiltinColormap {
    match value {
        0 => BuiltinColormap::Viridis,
        1 => BuiltinColormap::Plasma,
        2 => BuiltinColormap::Greyscale,
        3 => BuiltinColormap::Coolwarm,
        4 => BuiltinColormap::Rainbow,
        5 => BuiltinColormap::Magma,
        6 => BuiltinColormap::Inferno,
        7 => BuiltinColormap::Turbo,
        8 => BuiltinColormap::Jet,
        9 => BuiltinColormap::RdBu,
        _ => BuiltinColormap::Viridis,
    }
}

fn ground_plane_to_u8(value: GroundPlaneMode) -> u8 {
    match value {
        GroundPlaneMode::None => 0,
        GroundPlaneMode::ShadowOnly => 1,
        GroundPlaneMode::Tile => 2,
        GroundPlaneMode::SolidColor => 3,
    }
}

fn u8_to_ground_plane(value: u8) -> GroundPlaneMode {
    match value {
        1 => GroundPlaneMode::ShadowOnly,
        2 => GroundPlaneMode::Tile,
        3 => GroundPlaneMode::SolidColor,
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

fn attribute_kind_to_u8(value: AttributeKind) -> u8 {
    match value {
        AttributeKind::Vertex => 0,
        AttributeKind::Cell => 1,
        AttributeKind::Face => 2,
        AttributeKind::FaceColor => 3,
        AttributeKind::Edge => 4,
        AttributeKind::Halfedge => 5,
        AttributeKind::Corner => 6,
    }
}

fn u8_to_attribute_kind(value: u8) -> AttributeKind {
    match value {
        1 => AttributeKind::Cell,
        2 => AttributeKind::Face,
        3 => AttributeKind::FaceColor,
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

fn builtin_matcap_to_u8(value: BuiltinMatcap) -> u8 {
    value as u8
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
