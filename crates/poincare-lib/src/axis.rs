//! Axis box, tick stubs, and label generation for 3D graphs.
//!
//! # Overview
//!
//! The axis system consists of three visual layers:
//!
//! 1. **Box edges** — 12 line segments forming the wireframe bounding box of the domain.
//! 2. **Tick stubs** — short perpendicular stubs at nice-number positions along each axis.
//! 3. **Labels** — projected 2D text: one per tick + one axis-name label per axis.
//!
//! All polyline geometry is returned as [`PolylineItem`] values that the caller appends
//! to [`FrameData::polylines`] before submission.

use glam::{Mat4, Vec2, Vec3};
use viewport_lib::{LabelItem, PolylineItem};

use crate::domain::Domain;

// ---------------------------------------------------------------------------
// Axis3 — axis discriminant
// ---------------------------------------------------------------------------

/// Identifies one of the three Cartesian axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis3 {
    X,
    Y,
    Z,
}

// ---------------------------------------------------------------------------
// AxisConfig
// ---------------------------------------------------------------------------

/// Configuration for the labelled axis box rendered around every graph.
#[derive(Debug, Clone)]
pub struct AxisConfig {
    /// Draw the 12-edge wireframe bounding box.
    pub show_box: bool,

    /// Draw projected axis/tick labels.
    pub show_labels: bool,

    /// Draw short perpendicular tick stubs along each axis.
    pub show_ticks: bool,

    /// Draw grid planes (not yet implemented — reserved for future use).
    pub show_grid: bool,

    /// Axis name labels: `[x_label, y_label, z_label]`.
    /// `None` suppresses the label for that axis.
    pub labels: [Option<String>; 3],

    /// Target number of ticks per axis: `[nx, ny, nz]`.
    pub tick_count: [u32; 3],

    /// Per-axis RGBA colours (linear float): `[x_colour, y_colour, z_colour]`.
    /// Used for both axis lines and their tick stubs.
    pub axis_colours: [[f32; 4]; 3],

    /// RGBA colour for tick labels (linear float).
    pub tick_colour: [f32; 4],
}

impl Default for AxisConfig {
    fn default() -> Self {
        let dim = [0.7, 0.7, 0.7, 1.0];
        Self {
            show_box: true,
            show_labels: true,
            show_ticks: true,
            show_grid: false,
            labels: [
                Some("x".to_string()),
                Some("y".to_string()),
                Some("z".to_string()),
            ],
            tick_count: [5, 5, 5],
            axis_colours: [
                [0.9, 0.2, 0.2, 1.0], // X — red
                [0.2, 0.9, 0.2, 1.0], // Y — green
                [0.2, 0.2, 0.9, 1.0], // Z — blue
            ],
            tick_colour: dim,
        }
    }
}

// ---------------------------------------------------------------------------
// Axis line generation
// ---------------------------------------------------------------------------

/// Returns the 3 axis lines through the origin as `(start, end)` pairs.
///
/// Each line spans the full domain extent along its axis at the other two
/// coordinates equal to zero (i.e. the lines cross at the origin).
pub fn build_axis_lines(domain: &Domain) -> [([f32; 3], [f32; 3]); 3] {
    let x0 = *domain.x.start() as f32;
    let x1 = *domain.x.end() as f32;
    let y0 = *domain.y.start() as f32;
    let y1 = *domain.y.end() as f32;
    let z0 = *domain.z.start() as f32;
    let z1 = *domain.z.end() as f32;

    [
        ([x0, 0.0, 0.0], [x1, 0.0, 0.0]), // X axis
        ([0.0, y0, 0.0], [0.0, y1, 0.0]), // Y axis
        ([0.0, 0.0, z0], [0.0, 0.0, z1]), // Z axis
    ]
}

// ---------------------------------------------------------------------------
// Tick stub generation
// ---------------------------------------------------------------------------

/// Maximum stub length in world units.
const MAX_STUB_LENGTH: f32 = 0.3;
/// Minimum stub length in world units.
const MIN_STUB_LENGTH: f32 = 1e-4;
/// Maximum outward label offset in world units.
const MAX_LABEL_OFFSET: f32 = 0.6;
/// Minimum outward label offset in world units.
const MIN_LABEL_OFFSET: f32 = 5e-4;
/// Desired projected tick length in normalized device coordinates.
const TARGET_TICK_NDC: f32 = 0.025;
/// Desired projected label offset in normalized device coordinates.
const TARGET_LABEL_OFFSET_NDC: f32 = 0.035;

fn axis_span(domain: &Domain, axis: Axis3) -> f32 {
    match axis {
        Axis3::X => (*domain.x.end() - *domain.x.start()) as f32,
        Axis3::Y => (*domain.y.end() - *domain.y.start()) as f32,
        Axis3::Z => (*domain.z.end() - *domain.z.start()) as f32,
    }
    .abs()
}

fn tick_step_hint(domain: &Domain, axis: Axis3, tick_positions: &[(f64, String)]) -> f32 {
    let mut deltas: Vec<f32> = tick_positions
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).abs() as f32)
        .filter(|d| *d > 1e-6)
        .collect();
    if deltas.is_empty() {
        let span = axis_span(domain, axis);
        return (span / 5.0).max(MIN_STUB_LENGTH);
    }
    deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    deltas[deltas.len() / 2]
}

fn stub_length_for_axis(domain: &Domain, axis: Axis3, tick_positions: &[(f64, String)]) -> f32 {
    (tick_step_hint(domain, axis, tick_positions) * 0.18).clamp(MIN_STUB_LENGTH, MAX_STUB_LENGTH)
}

fn label_offset_for_axis(domain: &Domain, axis: Axis3, tick_positions: &[(f64, String)]) -> f32 {
    (stub_length_for_axis(domain, axis, tick_positions) * 1.4).clamp(MIN_LABEL_OFFSET, MAX_LABEL_OFFSET)
}

fn stub_direction(axis: Axis3) -> Vec3 {
    match axis {
        Axis3::X => Vec3::NEG_Y,
        Axis3::Y => Vec3::NEG_X,
        Axis3::Z => Vec3::NEG_Y,
    }
}

fn axis_point(axis: Axis3, pos: f64) -> Vec3 {
    let p = pos as f32;
    match axis {
        Axis3::X => Vec3::new(p, 0.0, 0.0),
        Axis3::Y => Vec3::new(0.0, p, 0.0),
        Axis3::Z => Vec3::new(0.0, 0.0, p),
    }
}

fn project_to_ndc_xy(vp: &Mat4, world: Vec3) -> Option<Vec2> {
    let clip = *vp * world.extend(1.0);
    if clip.w.abs() < 1e-6 || clip.z < -clip.w || clip.z > clip.w {
        return None;
    }
    let inv_w = 1.0 / clip.w;
    Some(Vec2::new(clip.x * inv_w, clip.y * inv_w))
}

fn projected_world_extent(
    vp: &Mat4,
    origin: Vec3,
    direction: Vec3,
    target_ndc: f32,
) -> Option<f32> {
    let base = project_to_ndc_xy(vp, origin)?;
    let mut lo = MIN_STUB_LENGTH;
    let mut hi = MAX_STUB_LENGTH;
    let end_hi = project_to_ndc_xy(vp, origin + direction * hi)?;
    if (end_hi - base).length() <= target_ndc {
        return Some(hi);
    }
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        let end = project_to_ndc_xy(vp, origin + direction * mid)?;
        if (end - base).length() < target_ndc {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(hi)
}

fn projected_stub_length(
    domain: &Domain,
    vp: Option<&Mat4>,
    axis: Axis3,
    tick_positions: &[(f64, String)],
    pos: f64,
) -> f32 {
    let fallback = stub_length_for_axis(domain, axis, tick_positions);
    let Some(vp) = vp else { return fallback };
    projected_world_extent(vp, axis_point(axis, pos), stub_direction(axis), TARGET_TICK_NDC)
        .unwrap_or(fallback)
        .clamp(MIN_STUB_LENGTH, MAX_STUB_LENGTH)
}

fn projected_label_offset(
    domain: &Domain,
    vp: Option<&Mat4>,
    axis: Axis3,
    tick_positions: &[(f64, String)],
    pos: f64,
) -> f32 {
    let fallback = label_offset_for_axis(domain, axis, tick_positions);
    let Some(vp) = vp else { return fallback };
    projected_world_extent(vp, axis_point(axis, pos), stub_direction(axis), TARGET_LABEL_OFFSET_NDC)
        .unwrap_or(fallback)
        .clamp(MIN_LABEL_OFFSET, MAX_LABEL_OFFSET)
}

/// Returns short perpendicular line stubs at each tick position along `axis`.
///
/// Stubs are centred on the axis line through the origin:
///
/// * **X-axis**: stub at `(tick_x, 0, 0)` extending in the −Y direction.
/// * **Y-axis**: stub at `(0, tick_y, 0)` extending in the −X direction.
/// * **Z-axis**: stub at `(0, 0, tick_z)` extending in the −Y direction.
pub fn build_tick_stubs(
    domain: &Domain,
    axis: Axis3,
    tick_positions: &[(f64, String)],
) -> Vec<([f32; 3], [f32; 3])> {
    build_tick_stubs_projected(domain, None, axis, tick_positions)
}

pub fn build_tick_stubs_projected(
    domain: &Domain,
    vp: Option<&Mat4>,
    axis: Axis3,
    tick_positions: &[(f64, String)],
) -> Vec<([f32; 3], [f32; 3])> {
    tick_positions
        .iter()
        .map(|(pos, _)| {
            let p = *pos as f32;
            let stub_length = projected_stub_length(domain, vp, axis, tick_positions, *pos);
            match axis {
                Axis3::X => ([p, 0.0, 0.0], [p, -stub_length, 0.0]),
                Axis3::Y => ([0.0, p, 0.0], [-stub_length, p, 0.0]),
                Axis3::Z => ([0.0, 0.0, p], [0.0, -stub_length, p]),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Combined polyline assembly
// ---------------------------------------------------------------------------

/// Assemble the axis lines and tick stubs into [`PolylineItem`] values.
///
/// Returns:
/// * One `PolylineItem` for the 3 axis lines through the origin.
/// * One `PolylineItem` for all tick stubs across all three axes.
///
/// If `config.show_box` is false, the axis lines item is omitted.
/// If `config.show_ticks` is false, the tick-stub items are omitted.
pub fn build_axis_polyline(
    domain: &Domain,
    config: &AxisConfig,
    ticks_per_axis: &[Vec<(f64, String)>; 3],
) -> Vec<PolylineItem> {
    build_axis_polyline_projected(domain, config, ticks_per_axis, None)
}

pub fn build_axis_polyline_projected(
    domain: &Domain,
    config: &AxisConfig,
    ticks_per_axis: &[Vec<(f64, String)>; 3],
    vp: Option<&Mat4>,
) -> Vec<PolylineItem> {
    let mut items = Vec::new();
    let axes = [Axis3::X, Axis3::Y, Axis3::Z];
    let lines = build_axis_lines(domain);

    for (i, axis) in axes.iter().enumerate() {
        let colour = config.axis_colours[i];

        // --- Axis line through origin ---
        if config.show_box {
            let (a, b) = lines[i];
            let mut item = PolylineItem::default();
            item.positions = vec![a, b];
            item.scalars = Vec::new();
            item.strip_lengths = vec![2];
            item.scalar_range = None;
            item.colourmap_id = None;
            item.default_colour = colour;
            item.line_width = 1.5;
            items.push(item);
        }

        // --- Tick stubs for this axis ---
        if config.show_ticks {
            let stubs = build_tick_stubs_projected(domain, vp, *axis, &ticks_per_axis[i]);
            if !stubs.is_empty() {
                let mut positions: Vec<[f32; 3]> = Vec::with_capacity(stubs.len() * 2);
                let mut strip_lengths: Vec<u32> = Vec::with_capacity(stubs.len());
                for (a, b) in stubs {
                    positions.push(a);
                    positions.push(b);
                    strip_lengths.push(2);
                }
                let mut item = PolylineItem::default();
                item.positions = positions;
                item.scalars = Vec::new();
                item.strip_lengths = strip_lengths;
                item.scalar_range = None;
                item.colourmap_id = None;
                item.default_colour = colour;
                item.line_width = 1.0;
                items.push(item);
            }
        }
    }

    items
}

// ---------------------------------------------------------------------------
// Label generation
// ---------------------------------------------------------------------------

pub(crate) fn tick_label_anchor(
    domain: &Domain,
    vp: Option<&Mat4>,
    axis: Axis3,
    tick_positions: &[(f64, String)],
    pos: f64,
) -> Vec3 {
    let offset = projected_label_offset(domain, vp, axis, tick_positions, pos);
    let p = pos as f32;
    match axis {
        Axis3::X => Vec3::new(p, -offset, 0.0),
        Axis3::Y => Vec3::new(-offset, p, 0.0),
        Axis3::Z => Vec3::new(-offset, 0.0, p),
    }
}

/// Build [`LabelItem`] entries for tick values and axis names.
///
/// Tick labels are positioned at the tick stub start point, offset outward from the
/// box face.  Axis name labels sit at the midpoint of the bottom edge of each axis.
pub fn build_axis_labels(
    domain: &Domain,
    config: &AxisConfig,
    ticks_per_axis: &[Vec<(f64, String)>; 3],
) -> Vec<LabelItem> {
    build_axis_labels_projected(domain, config, ticks_per_axis, None)
}

pub fn build_axis_labels_projected(
    domain: &Domain,
    config: &AxisConfig,
    ticks_per_axis: &[Vec<(f64, String)>; 3],
    vp: Option<&Mat4>,
) -> Vec<LabelItem> {
    let x1 = *domain.x.end() as f32;
    let y1 = *domain.y.end() as f32;
    let z1 = *domain.z.end() as f32;

    let tc = config.tick_colour;
    let mut labels: Vec<LabelItem> = Vec::new();

    // --- Tick labels ---
    // Labels sit just below/beside the tick stub, offset from the origin axis.
    // The origin (pos == 0) is shared by all three axes, so we emit its label only once.
    if config.show_labels {
        let mut origin_labelled = false;

        for (pos, text) in &ticks_per_axis[0] {
            if *pos == 0.0 {
                if origin_labelled {
                    continue;
                }
                origin_labelled = true;
            }
            let mut lbl = LabelItem::default();
            lbl.world_anchor = Some(
                tick_label_anchor(domain, vp, Axis3::X, &ticks_per_axis[0], *pos).to_array(),
            );
            lbl.text = text.clone();
            lbl.colour = tc;
            lbl.font_size = 11.0;
            labels.push(lbl);
        }
        for (pos, text) in &ticks_per_axis[1] {
            if *pos == 0.0 {
                if origin_labelled {
                    continue;
                }
                origin_labelled = true;
            }
            let mut lbl = LabelItem::default();
            lbl.world_anchor = Some(
                tick_label_anchor(domain, vp, Axis3::Y, &ticks_per_axis[1], *pos).to_array(),
            );
            lbl.text = text.clone();
            lbl.colour = tc;
            lbl.font_size = 11.0;
            labels.push(lbl);
        }
        for (pos, text) in &ticks_per_axis[2] {
            if *pos == 0.0 {
                if origin_labelled {
                    continue;
                }
                origin_labelled = true;
            }
            let mut lbl = LabelItem::default();
            lbl.world_anchor = Some(
                tick_label_anchor(domain, vp, Axis3::Z, &ticks_per_axis[2], *pos).to_array(),
            );
            lbl.text = text.clone();
            lbl.colour = tc;
            lbl.font_size = 11.0;
            labels.push(lbl);
        }
    }

    // --- Axis name labels — placed just beyond the positive end of each axis ---
    let name_positions = [
        (
            config.labels[0].as_deref(),
            Vec3::new(
                x1 + projected_label_offset(domain, vp, Axis3::X, &ticks_per_axis[0], *domain.x.end()),
                0.0,
                0.0,
            ),
        ),
        (
            config.labels[1].as_deref(),
            Vec3::new(
                0.0,
                y1 + projected_label_offset(domain, vp, Axis3::Y, &ticks_per_axis[1], *domain.y.end()),
                0.0,
            ),
        ),
        (
            config.labels[2].as_deref(),
            Vec3::new(
                0.0,
                0.0,
                z1 + projected_label_offset(domain, vp, Axis3::Z, &ticks_per_axis[2], *domain.z.end()),
            ),
        ),
    ];

    if config.show_labels {
        for (name_opt, world_pos) in &name_positions {
            let Some(name) = name_opt else { continue };
            let mut lbl = LabelItem::default();
            lbl.world_anchor = Some(world_pos.to_array());
            lbl.text = name.to_string();
            lbl.colour = tc;
            lbl.font_size = 13.0;
            labels.push(lbl);
        }
    }

    labels
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Domain;

    fn default_domain() -> Domain {
        Domain::default() // [-10, 10] x [-10, 10] x [-10, 10]
    }

    #[test]
    fn axis_lines_has_three() {
        let lines = build_axis_lines(&default_domain());
        assert_eq!(lines.len(), 3, "must produce exactly 3 axis lines");
        // X axis passes through origin (y=0, z=0)
        assert_eq!(lines[0].0[1], 0.0);
        assert_eq!(lines[0].0[2], 0.0);
    }

    #[test]
    fn tick_stubs_match_count() {
        let domain = default_domain();
        let ticks: Vec<(f64, String)> = vec![
            (-10.0, "-10".to_string()),
            (-5.0, "-5".to_string()),
            (0.0, "0".to_string()),
            (5.0, "5".to_string()),
            (10.0, "10".to_string()),
        ];
        let stubs = build_tick_stubs(&domain, Axis3::X, &ticks);
        assert_eq!(stubs.len(), ticks.len());
    }

    #[test]
    fn tick_stubs_x_direction() {
        let domain = default_domain();
        let ticks = vec![(5.0_f64, "5".to_string())];
        let stubs = build_tick_stubs(&domain, Axis3::X, &ticks);
        // X stub: start = (5, 0, 0), end = (5, -0.3, 0)
        let (start, end) = stubs[0];
        assert!((start[0] - 5.0).abs() < 1e-6);
        assert!((start[1] - 0.0).abs() < 1e-6);
        assert!((end[1] - (-0.3)).abs() < 1e-4);
    }

    #[test]
    fn axis_config_defaults() {
        let cfg = AxisConfig::default();
        assert!(cfg.show_box);
        assert!(cfg.show_labels);
        assert!(cfg.show_ticks);
        assert!(!cfg.show_grid);
        assert_eq!(cfg.labels[0].as_deref(), Some("x"));
        assert_eq!(cfg.labels[1].as_deref(), Some("y"));
        assert_eq!(cfg.labels[2].as_deref(), Some("z"));
        assert_eq!(cfg.tick_count, [5, 5, 5]);
        // X=red, Y=green, Z=blue
        assert!(cfg.axis_colours[0][0] > cfg.axis_colours[0][1]); // red dominates X
        assert!(cfg.axis_colours[1][1] > cfg.axis_colours[1][0]); // green dominates Y
        assert!(cfg.axis_colours[2][2] > cfg.axis_colours[2][0]); // blue dominates Z
    }

    #[test]
    fn build_axis_polyline_returns_six_items_when_both_enabled() {
        let domain = default_domain();
        let cfg = AxisConfig::default();
        let ticks: Vec<(f64, String)> = vec![(0.0, "0".to_string())];
        let ticks_per_axis = [ticks.clone(), ticks.clone(), ticks.clone()];
        let items = build_axis_polyline(&domain, &cfg, &ticks_per_axis);
        assert_eq!(items.len(), 6, "should produce 3 axis lines + 3 tick sets");
    }

    #[test]
    fn build_axis_labels_returns_ticks_plus_axis_names() {
        let domain = default_domain();
        let cfg = AxisConfig::default();
        let ticks: Vec<(f64, String)> = vec![
            (-10.0, "-10".to_string()),
            (0.0, "0".to_string()),
            (10.0, "10".to_string()),
        ];
        let ticks_per_axis = [ticks.clone(), ticks.clone(), ticks.clone()];
        let labels = build_axis_labels(&domain, &cfg, &ticks_per_axis);
        // 3 ticks * 3 axes = 9, minus 2 duplicate origin labels + 3 axis names = 10
        assert_eq!(
            labels.len(),
            10,
            "expected 7 tick + 3 name labels (origin deduplicated)"
        );
        // Check axis name labels are present.
        let texts: Vec<&str> = labels.iter().map(|l| l.text.as_str()).collect();
        assert!(texts.contains(&"x"), "missing x label");
        assert!(texts.contains(&"y"), "missing y label");
        assert!(texts.contains(&"z"), "missing z label");
    }
}
