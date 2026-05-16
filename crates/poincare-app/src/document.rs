use std::collections::HashMap;

use poincare_lib::{AxisConfig, GraphScene};
use viewport_lib::{Camera, GroundPlaneMode};

use crate::picking::segment_segment_closest;
use crate::picking::ProbeHit;
use crate::plot::entry::PlotEntry;
use crate::plot::sweep::ParameterSweep;

pub(crate) const VIEWPORT_BACKGROUND: [f32; 4] = [18.0 / 255.0, 18.0 / 255.0, 18.0 / 255.0, 1.0];
pub(crate) const DEFAULT_VIEWPORT_BACKGROUND: [f32; 4] = VIEWPORT_BACKGROUND;

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

    pub probe_mode: bool,
    pub probe_hit: Option<ProbeHit>,
    pub intersection_cache: Vec<glam::Vec3>,
    pub probe_snap_point: Option<glam::Vec3>,
    pub probe_snap_locked: bool,

    pub ground_plane_mode: GroundPlaneMode,
    pub ground_plane_height: f32,
    pub ground_plane_color: [f32; 4],
    pub ground_plane_tile_size: f32,
    pub viewport_background: [f32; 4],

    /// Per-plot parameter sweep config.  Parallel to `plots`; grown lazily.
    pub sweep_config: Vec<HashMap<String, ParameterSweep>>,
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
            export_path: "poincare-export.png".to_string(),
            export_width: 1600,
            export_height: 1000,
            export_status: String::new(),
            probe_mode: false,
            probe_hit: None,
            intersection_cache: Vec::new(),
            probe_snap_point: None,
            probe_snap_locked: false,
            ground_plane_mode: GroundPlaneMode::None,
            ground_plane_height: 0.0,
            ground_plane_color: [0.3, 0.3, 0.3, 1.0],
            ground_plane_tile_size: 1.0,
            viewport_background: DEFAULT_VIEWPORT_BACKGROUND,
            sweep_config: Vec::new(),
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
        for plot in self.plots.iter().filter(|p| p.visible) {
            plot.add_to_scene(&mut scene);
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
        if any {
            (max - min).length() * 0.5
        } else {
            1.0
        }
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
