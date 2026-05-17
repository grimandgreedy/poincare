use std::collections::HashMap;
use std::path::PathBuf;

use poincare_lib::{AxisConfig, GraphScene};
use viewport_lib::{Aabb, Camera, GroundPlaneMode};

use crate::picking::ProbeHit;
use crate::picking::segment_segment_closest;
use crate::plot::entry::PlotEntry;
use crate::plot::kind::DomainLabels;
use crate::plot::sweep::ParameterSweep;
use crate::plot::table::TableDataSet;

pub(crate) const VIEWPORT_BACKGROUND: [f32; 4] = [18.0 / 255.0, 18.0 / 255.0, 18.0 / 255.0, 1.0];
pub(crate) const DEFAULT_VIEWPORT_BACKGROUND: [f32; 4] = VIEWPORT_BACKGROUND;
const UNDO_LIMIT: usize = 100;

#[derive(Clone)]
pub(crate) struct SavedCameraView {
    pub name: String,
    pub camera: Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportFormat {
    Png,
    Gif,
    Mp4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportMode {
    Image,
    Video,
}

#[derive(Clone)]
pub(crate) struct DocumentUndoState {
    title: String,
    path: Option<std::path::PathBuf>,
    dirty: bool,
    plots: Vec<PlotEntry>,
    selected_plot: Option<usize>,
    camera: Camera,
    axis_config: AxisConfig,
    export_path: String,
    export_width: u32,
    export_height: u32,
    ground_plane_mode: GroundPlaneMode,
    ground_plane_height: f32,
    ground_plane_color: [f32; 4],
    ground_plane_tile_size: f32,
    viewport_background: [f32; 4],
    saved_views: Vec<SavedCameraView>,
    camera_track_segment_duration: f32,
    sweep_config: Vec<HashMap<String, ParameterSweep>>,
    export_format: ExportFormat,
    export_fps: u32,
}

pub(crate) struct Document {
    pub title: String,
    pub path: Option<std::path::PathBuf>,
    pub dirty: bool,

    pub plots: Vec<PlotEntry>,
    pub selected_plot: Option<usize>,

    pub scene: GraphScene,
    pub scene_dirty: bool,

    pub camera: Camera,
    pub axis_config: AxisConfig,

    pub export_path: String,
    pub export_width: u32,
    pub export_height: u32,
    pub export_status: String,
    pub export_progress: Option<f32>,

    pub probe_mode: bool,
    pub probe_hit: Option<ProbeHit>,
    pub last_probe_hit: Option<ProbeHit>,
    pub hovered_plot: Option<usize>,
    pub pinned_probes: Vec<ProbeHit>,
    pub intersection_cache: Vec<glam::Vec3>,
    pub probe_snap_point: Option<glam::Vec3>,
    pub probe_snap_locked: bool,

    pub ground_plane_mode: GroundPlaneMode,
    pub ground_plane_height: f32,
    pub ground_plane_color: [f32; 4],
    pub ground_plane_tile_size: f32,
    pub viewport_background: [f32; 4],
    pub saved_views: Vec<SavedCameraView>,
    pub camera_track_segment_duration: f32,
    pub camera_track_t: f64,
    pub camera_track_playing: bool,

    /// Per-plot parameter sweep config.  Parallel to `plots`; grown lazily.
    pub sweep_config: Vec<HashMap<String, ParameterSweep>>,
    pub export_format: ExportFormat,
    pub export_fps: u32,
    history_head: Option<DocumentUndoState>,
    undo_stack: Vec<DocumentUndoState>,
    redo_stack: Vec<DocumentUndoState>,
    history_pending_commit: bool,
}

impl Document {
    pub(crate) fn new_default() -> Self {
        Self {
            title: "Untitled".to_string(),
            path: None,
            dirty: false,
            plots: Vec::new(),
            selected_plot: None,
            scene: GraphScene::new(),
            scene_dirty: true,
            camera: default_camera(),
            axis_config: AxisConfig::default(),
            export_path: default_export_path_for_format(ExportFormat::Png)
                .to_string_lossy()
                .into_owned(),
            export_width: 1600,
            export_height: 1000,
            export_status: String::new(),
            export_progress: None,
            probe_mode: false,
            probe_hit: None,
            last_probe_hit: None,
            hovered_plot: None,
            pinned_probes: Vec::new(),
            intersection_cache: Vec::new(),
            probe_snap_point: None,
            probe_snap_locked: false,
            ground_plane_mode: GroundPlaneMode::None,
            ground_plane_height: 0.0,
            ground_plane_color: [0.3, 0.3, 0.3, 1.0],
            ground_plane_tile_size: 1.0,
            viewport_background: DEFAULT_VIEWPORT_BACKGROUND,
            saved_views: Vec::new(),
            camera_track_segment_duration: 2.5,
            camera_track_t: 0.0,
            camera_track_playing: false,
            sweep_config: Vec::new(),
            export_format: ExportFormat::Png,
            export_fps: 24,
            history_head: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            history_pending_commit: false,
        }
    }

    pub(crate) fn title_or_untitled(&self) -> &str {
        if self.title.is_empty() {
            "Untitled"
        } else {
            &self.title
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.scene_dirty = true;
        self.export_status.clear();
        self.export_progress = None;
    }

    pub(crate) fn mark_modified(&mut self) {
        self.dirty = true;
        self.export_status.clear();
        self.export_progress = None;
    }

    pub(crate) fn initialize_history(&mut self) {
        self.history_head = Some(self.snapshot_state());
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.history_pending_commit = false;
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub(crate) fn record_undo_point(&mut self) {
        if self.history_head.is_none() {
            self.initialize_history();
        }
        if self.history_pending_commit {
            return;
        }
        if let Some(head) = &self.history_head {
            self.undo_stack.push(head.clone());
            if self.undo_stack.len() > UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
            self.history_pending_commit = true;
        }
    }

    pub(crate) fn finalize_history_point(&mut self) {
        if self.history_pending_commit {
            self.history_head = Some(self.snapshot_state());
            self.history_pending_commit = false;
        } else if self.history_head.is_none() {
            self.history_head = Some(self.snapshot_state());
        }
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.snapshot_state());
        self.restore_state(previous.clone());
        self.history_head = Some(previous);
        self.history_pending_commit = false;
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.snapshot_state());
        self.restore_state(next.clone());
        self.history_head = Some(next);
        self.history_pending_commit = false;
        true
    }

    /// Build the CPU-side scene from the current plot list.
    /// Returns `None` if the scene is not dirty.
    /// The caller is responsible for GPU upload and clearing `scene_dirty`.
    pub(crate) fn build_scene_data(&self) -> Option<GraphScene> {
        if !self.scene_dirty {
            return None;
        }
        let mut scene = GraphScene::new();
        scene.axis_config = self.axis_config.clone();
        for (plot_idx, plot) in self.plots.iter().enumerate() {
            if !plot.visible {
                continue;
            }
            // Pick IDs are one-based so `0` remains the "not pickable" sentinel.
            plot.add_to_scene_with_pick_id(&mut scene, (plot_idx + 1) as u64);
        }
        Some(scene)
    }

    /// Approximate scene extent (half-diagonal) for adaptive snap radii.
    pub(crate) fn scene_extent(&self) -> f32 {
        let data = self.scene.probe_data();
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        let mut any = false;
        for p in &data.polylines {
            for &v in p.positions {
                min = min.min(v);
                max = max.max(v);
                any = true;
            }
        }
        for s in &data.surfaces {
            for &pos in s.positions {
                let v = glam::Vec3::from(pos);
                min = min.min(v);
                max = max.max(v);
                any = true;
            }
        }
        if any { (max - min).length() * 0.5 } else { 1.0 }
    }

    /// Rebuild the cache of curve-curve intersection points from the current scene.
    pub(crate) fn recompute_intersections(&mut self) {
        self.intersection_cache.clear();
        let data = self.scene.probe_data();
        let polylines = data.polylines;

        let mut all_strips: Vec<Vec<glam::Vec3>> = Vec::new();
        for poly in &polylines {
            let mut segs: Vec<glam::Vec3> = Vec::new();
            let mut offset = 0usize;
            for &len in poly.strip_lengths {
                let len = len as usize;
                for j in offset..offset + len {
                    segs.push(poly.positions[j]);
                }
                offset += len;
            }
            if !segs.is_empty() {
                all_strips.push(segs);
            }
        }

        let snap_world_radius = 0.05 * self.scene_extent();
        for i in 0..all_strips.len() {
            for j in (i + 1)..all_strips.len() {
                let a = &all_strips[i];
                let b = &all_strips[j];
                for ka in 0..a.len().saturating_sub(1) {
                    for kb in 0..b.len().saturating_sub(1) {
                        let (pa, pb, dist) =
                            segment_segment_closest(a[ka], a[ka + 1], b[kb], b[kb + 1]);
                        if dist < snap_world_radius {
                            let mid = (pa + pb) * 0.5;
                            let already = self
                                .intersection_cache
                                .iter()
                                .any(|&c: &glam::Vec3| c.distance(mid) < snap_world_radius);
                            if !already {
                                self.intersection_cache.push(mid);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn visible_scene_bounds(&self) -> Option<Aabb> {
        scene_bounds(&self.scene).or_else(|| {
            let mut min = glam::Vec3::splat(f32::INFINITY);
            let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
            let mut any = false;
            for plot in self.plots.iter().filter(|plot| plot.visible) {
                if let Some(bounds) = plot_bounds(plot) {
                    min = min.min(bounds.min);
                    max = max.max(bounds.max);
                    any = true;
                }
            }
            any.then_some(Aabb { min, max })
        })
    }

    pub(crate) fn selected_plot_bounds(&self) -> Option<Aabb> {
        let idx = self.selected_plot?;
        let plot = self.plots.get(idx)?;
        plot.visible.then(|| plot_bounds(plot)).flatten()
    }

    fn snapshot_state(&self) -> DocumentUndoState {
        DocumentUndoState {
            title: self.title.clone(),
            path: self.path.clone(),
            dirty: self.dirty,
            plots: self.plots.clone(),
            selected_plot: self.selected_plot,
            camera: self.camera.clone(),
            axis_config: self.axis_config.clone(),
            export_path: self.export_path.clone(),
            export_width: self.export_width,
            export_height: self.export_height,
            ground_plane_mode: self.ground_plane_mode,
            ground_plane_height: self.ground_plane_height,
            ground_plane_color: self.ground_plane_color,
            ground_plane_tile_size: self.ground_plane_tile_size,
            viewport_background: self.viewport_background,
            saved_views: self.saved_views.clone(),
            camera_track_segment_duration: self.camera_track_segment_duration,
            sweep_config: self.sweep_config.clone(),
            export_format: self.export_format,
            export_fps: self.export_fps,
        }
    }

    fn restore_state(&mut self, state: DocumentUndoState) {
        self.title = state.title;
        self.path = state.path;
        self.dirty = state.dirty;
        self.plots = state.plots;
        self.selected_plot = state.selected_plot.filter(|&i| i < self.plots.len());
        self.camera = state.camera;
        self.axis_config = state.axis_config;
        self.export_path = state.export_path;
        self.export_width = state.export_width;
        self.export_height = state.export_height;
        self.ground_plane_mode = state.ground_plane_mode;
        self.ground_plane_height = state.ground_plane_height;
        self.ground_plane_color = state.ground_plane_color;
        self.ground_plane_tile_size = state.ground_plane_tile_size;
        self.viewport_background = state.viewport_background;
        self.saved_views = state.saved_views;
        self.camera_track_segment_duration = state.camera_track_segment_duration;
        self.camera_track_t = 0.0;
        self.camera_track_playing = false;
        self.sweep_config = state.sweep_config;
        self.export_format = state.export_format;
        self.export_fps = state.export_fps;
        self.scene_dirty = true;
        self.export_status.clear();
        self.export_progress = None;
        self.probe_hit = None;
        self.last_probe_hit = None;
        self.hovered_plot = None;
        self.pinned_probes.clear();
        self.intersection_cache.clear();
        self.probe_snap_point = None;
        self.probe_snap_locked = false;
    }
}

pub(crate) fn export_mode_for_format(format: ExportFormat) -> ExportMode {
    match format {
        ExportFormat::Png => ExportMode::Image,
        ExportFormat::Gif | ExportFormat::Mp4 => ExportMode::Video,
    }
}

pub(crate) fn default_export_filename(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Png => "poincare-export.png",
        ExportFormat::Gif => "poincare-export.gif",
        ExportFormat::Mp4 => "poincare-export.mp4",
    }
}

pub(crate) fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home);
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(profile);
    }
    match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
        (Some(drive), Some(path)) => PathBuf::from(format!(
            "{}{}",
            PathBuf::from(drive).to_string_lossy(),
            PathBuf::from(path).to_string_lossy()
        )),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

pub(crate) fn default_export_dir(mode: ExportMode) -> PathBuf {
    let mut path = home_dir();
    path.push(match mode {
        ExportMode::Image => "Pictures",
        ExportMode::Video => "Videos",
    });
    path.push("Poincare");
    path
}

pub(crate) fn ensure_export_dir_exists(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| format!("Export failed: {err}"))
}

pub(crate) fn default_export_path_for_format(format: ExportFormat) -> PathBuf {
    let mode = export_mode_for_format(format);
    let dir = default_export_dir(mode);
    let _ = ensure_export_dir_exists(&dir);
    dir.join(default_export_filename(format))
}

pub(crate) fn default_camera() -> Camera {
    // Z-up convention: start from identity (top-down, eye at +Z), tilt ~63° from
    // zenith (≈ 27° elevation), then yaw 45° around Z for an isometric-ish angle.
    Camera {
        center: glam::Vec3::ZERO,
        distance: 35.0,
        orientation: glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
            * glam::Quat::from_rotation_x(1.1),
        ..Camera::default()
    }
}

fn scene_bounds(scene: &GraphScene) -> Option<Aabb> {
    let data = scene.probe_data();
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;

    for surface in &data.surfaces {
        for &pos in surface.positions {
            let pos = glam::Vec3::from(pos);
            min = min.min(pos);
            max = max.max(pos);
            any = true;
        }
    }
    for polyline in &data.polylines {
        for &pos in polyline.positions {
            let pos = glam::Vec3::from(pos);
            min = min.min(pos);
            max = max.max(pos);
            any = true;
        }
    }
    for points in &data.points {
        for &pos in points.positions {
            min = min.min(pos);
            max = max.max(pos);
            any = true;
        }
    }

    if !any {
        return None;
    }

    if (max - min).length_squared() < 1.0e-8 {
        let pad = glam::Vec3::splat(0.5);
        min -= pad;
        max += pad;
    }

    Some(Aabb { min, max })
}

pub(crate) fn plot_bounds(plot: &PlotEntry) -> Option<Aabb> {
    if let crate::plot::kind::PlotKind::ImportedTable { definition } = &plot.kind {
        if let Ok(dataset) = definition.validate() {
            let bounds = match dataset {
                TableDataSet::SurfaceGrid { xs, ys, zs } => {
                    let (Some(x_min), Some(x_max)) = (
                        xs.iter().cloned().reduce(f64::min),
                        xs.iter().cloned().reduce(f64::max),
                    ) else {
                        return None;
                    };
                    let (Some(y_min), Some(y_max)) = (
                        ys.iter().cloned().reduce(f64::min),
                        ys.iter().cloned().reduce(f64::max),
                    ) else {
                        return None;
                    };
                    let (Some(z_min), Some(z_max)) = (
                        zs.iter().cloned().reduce(f64::min),
                        zs.iter().cloned().reduce(f64::max),
                    ) else {
                        return None;
                    };
                    Aabb {
                        min: glam::vec3(x_min as f32, y_min as f32, z_min as f32),
                        max: glam::vec3(x_max as f32, y_max as f32, z_max as f32),
                    }
                }
                TableDataSet::Curve { bounds, .. }
                | TableDataSet::Scatter { bounds, .. }
                | TableDataSet::VectorField { bounds, .. } => Aabb {
                    min: glam::vec3(
                        *bounds.x.start() as f32,
                        *bounds.y.start() as f32,
                        *bounds.z.start() as f32,
                    ),
                    max: glam::vec3(
                        *bounds.x.end() as f32,
                        *bounds.y.end() as f32,
                        *bounds.z.end() as f32,
                    ),
                },
            };
            return Some(bounds);
        }
    }

    let x0 = *plot.domain.x.start() as f32;
    let x1 = *plot.domain.x.end() as f32;
    let y0 = *plot.domain.y.start() as f32;
    let y1 = *plot.domain.y.end() as f32;
    let z0 = *plot.domain.z.start() as f32;
    let z1 = *plot.domain.z.end() as f32;

    let (mut min, mut max) = match plot.kind.domain_labels() {
        DomainLabels::None => return None,
        DomainLabels::Xy | DomainLabels::Xyz | DomainLabels::Uv => {
            (glam::vec3(x0, y0, z0), glam::vec3(x1, y1, z1))
        }
        DomainLabels::ThetaPhi => (glam::vec3(-6.0, -6.0, -6.0), glam::vec3(6.0, 6.0, 6.0)),
        DomainLabels::ThetaZ => (glam::vec3(-6.0, -6.0, z0), glam::vec3(6.0, 6.0, z1)),
        DomainLabels::Theta => (glam::vec3(-6.0, -6.0, -1.0), glam::vec3(6.0, 6.0, 1.0)),
        DomainLabels::T | DomainLabels::SingleVar(_) => {
            (glam::vec3(x0, x0, x0), glam::vec3(x1, x1, x1))
        }
    };

    if (max - min).length_squared() < 1.0e-8 {
        let pad = glam::Vec3::splat(0.5);
        min -= pad;
        max += pad;
    }

    Some(Aabb { min, max })
}
