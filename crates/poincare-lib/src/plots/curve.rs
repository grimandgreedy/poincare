use std::ops::RangeInclusive;

use glam::Vec3;

use crate::coordinate::{CoordinateSystem, ParametricDomain};
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveInterpolationKind {
    Linear,
    CatmullRom,
    CentripetalCatmullRom,
    MovingAverage,
    SavitzkyGolay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveInterpolation {
    pub kind: CurveInterpolationKind,
    pub samples_per_segment: u32,
    pub closed: bool,
    pub smoothing_window: u32,
}

impl Default for CurveInterpolation {
    fn default() -> Self {
        Self {
            kind: CurveInterpolationKind::Linear,
            samples_per_segment: 1,
            closed: false,
            smoothing_window: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal kind enum
// ---------------------------------------------------------------------------

enum CurveKind {
    Parametric {
        f: Box<dyn Fn(f64) -> glam::DVec3 + Send + Sync>,
        t_range: RangeInclusive<f64>,
    },
    Points {
        points: Vec<Vec3>,
        interpolation: CurveInterpolation,
    },
}

// ---------------------------------------------------------------------------
// Public struct
// ---------------------------------------------------------------------------

/// A 3D parametric curve or arbitrary point sequence.
///
/// Produces [`PlotGeometry::Polyline`] — rendered as a [`PolylineItem`] in the viewport.
pub struct Curve3D {
    kind: CurveKind,
    style: PlotStyle,
    resolution: Resolution,
}

impl Curve3D {
    // ------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------

    /// Parametric curve (x, y, z) = f(t) over `t_range`.
    ///
    /// Sampled at `resolution.u` evenly-spaced t values.
    pub fn parametric(
        t_range: RangeInclusive<f64>,
        f: impl Fn(f64) -> glam::DVec3 + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind: CurveKind::Parametric {
                f: Box::new(f),
                t_range,
            },
            style: PlotStyle::default(),
            resolution: Resolution::default(),
        }
    }

    /// Arbitrary sequence of 3D points rendered as a connected polyline.
    pub fn from_points(points: &[Vec3]) -> Self {
        Self::from_points_interpolated(points, CurveInterpolation::default())
    }

    /// Arbitrary sequence of 3D points rendered with the requested interpolation mode.
    pub fn from_points_interpolated(points: &[Vec3], interpolation: CurveInterpolation) -> Self {
        Self {
            kind: CurveKind::Points {
                points: points.to_vec(),
                interpolation,
            },
            style: PlotStyle::default(),
            resolution: Resolution::default(),
        }
    }

    /// Override the default style.
    pub fn with_style(mut self, style: PlotStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = resolution;
        self
    }
}

// ---------------------------------------------------------------------------
// PlotObject implementation
// ---------------------------------------------------------------------------

impl PlotObject for Curve3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        match &self.kind {
            CurveKind::Parametric { t_range, .. } => {
                CoordinateSystem::Parametric(ParametricDomain::Curve { t: t_range.clone() })
            }
            CurveKind::Points { .. } => CoordinateSystem::Cartesian,
        }
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        match &self.kind {
            CurveKind::Parametric { .. } => None,
            CurveKind::Points { points: pts, .. } => {
                if pts.is_empty() {
                    return None;
                }
                let mut xmin = f32::MAX;
                let mut xmax = f32::MIN;
                let mut ymin = f32::MAX;
                let mut ymax = f32::MIN;
                let mut zmin = f32::MAX;
                let mut zmax = f32::MIN;
                for p in pts {
                    xmin = xmin.min(p.x);
                    xmax = xmax.max(p.x);
                    ymin = ymin.min(p.y);
                    ymax = ymax.max(p.y);
                    zmin = zmin.min(p.z);
                    zmax = zmax.max(p.z);
                }
                Some(DataBounds {
                    x: xmin as f64..=xmax as f64,
                    y: ymin as f64..=ymax as f64,
                    z: zmin as f64..=zmax as f64,
                })
            }
        }
    }

    fn generate(&self, _domain: &Domain, resolution: Resolution) -> PlotGeometry {
        match &self.kind {
            CurveKind::Parametric { f, t_range } => {
                let steps = resolution.u.max(2) as usize;
                let t0 = *t_range.start();
                let t1 = *t_range.end();
                let pts: Vec<Vec3> = (0..steps)
                    .map(|i| {
                        let t = t0 + (i as f64 / (steps - 1) as f64) * (t1 - t0);
                        let p = f(t);
                        Vec3::new(p.x as f32, p.y as f32, p.z as f32)
                    })
                    .collect();
                PlotGeometry::Polyline {
                    strip_lengths: vec![pts.len() as u32],
                    positions: pts,
                    scalars: None,
                }
            }
            CurveKind::Points {
                points,
                interpolation,
            } => {
                let sampled = sample_interpolated_points(points, *interpolation);
                PlotGeometry::Polyline {
                    strip_lengths: vec![sampled.len() as u32],
                    positions: sampled,
                    scalars: None,
                }
            }
        }
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }
}

fn sample_interpolated_points(points: &[Vec3], interpolation: CurveInterpolation) -> Vec<Vec3> {
    if points.len() <= 1 {
        return points.to_vec();
    }

    match interpolation.kind {
        CurveInterpolationKind::Linear => sample_linear(points, interpolation),
        CurveInterpolationKind::CatmullRom => sample_catmull_rom(points, interpolation, 0.0),
        CurveInterpolationKind::CentripetalCatmullRom => {
            sample_catmull_rom(points, interpolation, 0.5)
        }
        CurveInterpolationKind::MovingAverage => {
            let smoothed = moving_average_points(points, interpolation);
            sample_catmull_rom(&smoothed, interpolation, 0.5)
        }
        CurveInterpolationKind::SavitzkyGolay => {
            let smoothed = savitzky_golay_points(points, interpolation);
            sample_catmull_rom(&smoothed, interpolation, 0.5)
        }
    }
}

pub fn sample_curve_points(points: &[Vec3], interpolation: CurveInterpolation) -> Vec<Vec3> {
    sample_interpolated_points(points, interpolation)
}

fn sample_linear(points: &[Vec3], interpolation: CurveInterpolation) -> Vec<Vec3> {
    let segments = segment_count(points.len(), interpolation.closed);
    if segments == 0 {
        return points.to_vec();
    }

    let samples = interpolation.samples_per_segment.max(1) as usize;
    let mut out = Vec::with_capacity(segments * samples + 1);
    for i in 0..segments {
        let p1 = points[i];
        let p2 = points[(i + 1) % points.len()];
        if i == 0 {
            out.push(p1);
        }
        for step in 1..=samples {
            let t = step as f32 / samples as f32;
            if interpolation.closed || i + 1 < segments || step < samples {
                out.push(p1.lerp(p2, t));
            }
        }
    }
    if !interpolation.closed && out.last().copied() != points.last().copied() {
        out.push(*points.last().unwrap());
    }
    out
}

fn sample_catmull_rom(points: &[Vec3], interpolation: CurveInterpolation, alpha: f32) -> Vec<Vec3> {
    let segments = segment_count(points.len(), interpolation.closed);
    if segments == 0 {
        return points.to_vec();
    }

    let samples = interpolation.samples_per_segment.max(2) as usize;
    let mut out = Vec::with_capacity(segments * samples + 1);
    for i in 0..segments {
        let p0 = point_for_segment(points, i as isize - 1, interpolation.closed);
        let p1 = point_for_segment(points, i as isize, interpolation.closed);
        let p2 = point_for_segment(points, i as isize + 1, interpolation.closed);
        let p3 = point_for_segment(points, i as isize + 2, interpolation.closed);
        if i == 0 {
            out.push(p1);
        }
        for step in 1..=samples {
            let t = step as f32 / samples as f32;
            if interpolation.closed || i + 1 < segments || step < samples {
                let point = if alpha == 0.0 {
                    catmull_rom_uniform(p0, p1, p2, p3, t)
                } else {
                    catmull_rom_nonuniform(p0, p1, p2, p3, t, alpha)
                };
                out.push(point);
            }
        }
    }
    if !interpolation.closed && out.last().copied() != points.last().copied() {
        out.push(*points.last().unwrap());
    }
    out
}

fn segment_count(point_count: usize, closed: bool) -> usize {
    if point_count < 2 {
        0
    } else if closed {
        point_count
    } else {
        point_count - 1
    }
}

fn moving_average_points(points: &[Vec3], interpolation: CurveInterpolation) -> Vec<Vec3> {
    let radius = normalized_smoothing_window(interpolation, 3) / 2;
    (0..points.len())
        .map(|index| {
            let mut sum = Vec3::ZERO;
            let mut count = 0.0_f32;
            for offset in -(radius as isize)..=(radius as isize) {
                let sample = sample_point(points, index as isize + offset, interpolation.closed);
                sum += sample;
                count += 1.0;
            }
            sum / count
        })
        .collect()
}

fn savitzky_golay_points(points: &[Vec3], interpolation: CurveInterpolation) -> Vec<Vec3> {
    let window = normalized_smoothing_window(interpolation, 5);
    let radius = window / 2;
    let weights = savitzky_golay_weights(radius, 3);
    (0..points.len())
        .map(|index| {
            let mut sum = Vec3::ZERO;
            for (weight_index, weight) in weights.iter().enumerate() {
                let offset = weight_index as isize - radius as isize;
                let sample = sample_point(points, index as isize + offset, interpolation.closed);
                sum += sample * *weight;
            }
            sum
        })
        .collect()
}

fn normalized_smoothing_window(interpolation: CurveInterpolation, minimum: u32) -> usize {
    let mut window = interpolation.smoothing_window.max(minimum);
    if window % 2 == 0 {
        window += 1;
    }
    window as usize
}

fn sample_point(points: &[Vec3], index: isize, closed: bool) -> Vec3 {
    if closed {
        let len = points.len() as isize;
        points[index.rem_euclid(len) as usize]
    } else {
        let clamped = index.clamp(0, points.len() as isize - 1) as usize;
        points[clamped]
    }
}

fn point_for_segment(points: &[Vec3], index: isize, closed: bool) -> Vec3 {
    sample_point(points, index, closed)
}

fn catmull_rom_uniform(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    ((p1 * 2.0)
        + (-p0 + p2) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3)
        * 0.5
}

fn catmull_rom_nonuniform(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32, alpha: f32) -> Vec3 {
    let t0 = 0.0_f32;
    let t1 = next_knot(t0, p0, p1, alpha);
    let t2 = next_knot(t1, p1, p2, alpha);
    let t3 = next_knot(t2, p2, p3, alpha);
    let tau = t1 + (t2 - t1) * t;

    let a1 = interpolate_knot(p0, p1, t0, t1, tau);
    let a2 = interpolate_knot(p1, p2, t1, t2, tau);
    let a3 = interpolate_knot(p2, p3, t2, t3, tau);
    let b1 = interpolate_knot(a1, a2, t0, t2, tau);
    let b2 = interpolate_knot(a2, a3, t1, t3, tau);
    interpolate_knot(b1, b2, t1, t2, tau)
}

fn next_knot(previous: f32, a: Vec3, b: Vec3, alpha: f32) -> f32 {
    let distance = a.distance(b).max(1.0e-4);
    previous + distance.powf(alpha)
}

fn interpolate_knot(a: Vec3, b: Vec3, ta: f32, tb: f32, t: f32) -> Vec3 {
    if (tb - ta).abs() <= f32::EPSILON {
        return a;
    }
    let factor_a = (tb - t) / (tb - ta);
    let factor_b = (t - ta) / (tb - ta);
    a * factor_a + b * factor_b
}

fn savitzky_golay_weights(radius: usize, polynomial_order: usize) -> Vec<f32> {
    let size = radius * 2 + 1;
    let order = polynomial_order.min(size.saturating_sub(1));
    let cols = order + 1;

    let mut ata = vec![vec![0.0_f64; cols]; cols];
    let mut at = vec![vec![0.0_f64; size]; cols];
    for row in 0..size {
        let x = row as isize - radius as isize;
        let mut powers = vec![1.0_f64; cols];
        for col in 1..cols {
            powers[col] = powers[col - 1] * x as f64;
        }
        for i in 0..cols {
            at[i][row] = powers[i];
            for j in 0..cols {
                ata[i][j] += powers[i] * powers[j];
            }
        }
    }

    let ata_inv = invert_matrix(ata).unwrap_or_else(|| identity_matrix(cols));
    let mut weights = vec![0.0_f64; size];
    for row in 0..size {
        for k in 0..cols {
            weights[row] += ata_inv[0][k] * at[k][row];
        }
    }
    weights.into_iter().map(|value| value as f32).collect()
}

fn identity_matrix(size: usize) -> Vec<Vec<f64>> {
    let mut matrix = vec![vec![0.0; size]; size];
    for i in 0..size {
        matrix[i][i] = 1.0;
    }
    matrix
}

fn invert_matrix(mut matrix: Vec<Vec<f64>>) -> Option<Vec<Vec<f64>>> {
    let size = matrix.len();
    let mut inverse = identity_matrix(size);
    for pivot in 0..size {
        let mut best_row = pivot;
        let mut best_value = matrix[pivot][pivot].abs();
        for row in (pivot + 1)..size {
            let candidate = matrix[row][pivot].abs();
            if candidate > best_value {
                best_value = candidate;
                best_row = row;
            }
        }
        if best_value <= f64::EPSILON {
            return None;
        }
        if best_row != pivot {
            matrix.swap(pivot, best_row);
            inverse.swap(pivot, best_row);
        }
        let scale = matrix[pivot][pivot];
        for col in 0..size {
            matrix[pivot][col] /= scale;
            inverse[pivot][col] /= scale;
        }
        for row in 0..size {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            if factor.abs() <= f64::EPSILON {
                continue;
            }
            for col in 0..size {
                matrix[row][col] -= factor * matrix[pivot][col];
                inverse[row][col] -= factor * inverse[pivot][col];
            }
        }
    }
    Some(inverse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation_keeps_endpoints() {
        let points = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        let sampled = sample_interpolated_points(
            &points,
            CurveInterpolation {
                kind: CurveInterpolationKind::Linear,
                samples_per_segment: 3,
                closed: false,
                smoothing_window: 5,
            },
        );
        assert_eq!(sampled.first().copied(), Some(points[0]));
        assert_eq!(sampled.last().copied(), Some(points[2]));
        assert_eq!(sampled.len(), 7);
    }

    #[test]
    fn catmull_rom_keeps_endpoints() {
        let points = [
            Vec3::ZERO,
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
        ];
        let sampled = sample_interpolated_points(
            &points,
            CurveInterpolation {
                kind: CurveInterpolationKind::CatmullRom,
                samples_per_segment: 8,
                closed: false,
                smoothing_window: 5,
            },
        );
        assert_eq!(sampled.first().copied(), Some(points[0]));
        assert_eq!(sampled.last().copied(), Some(points[3]));
        assert!(sampled.len() > points.len());
    }

    #[test]
    fn centripetal_closed_curve_wraps() {
        let points = [
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ];
        let sampled = sample_interpolated_points(
            &points,
            CurveInterpolation {
                kind: CurveInterpolationKind::CentripetalCatmullRom,
                samples_per_segment: 6,
                closed: true,
                smoothing_window: 5,
            },
        );
        assert_eq!(sampled.first().copied(), Some(points[0]));
        assert!(sampled.len() > points.len());
    }

    #[test]
    fn moving_average_changes_noisy_points() {
        let points = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 10.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
        ];
        let sampled = sample_interpolated_points(
            &points,
            CurveInterpolation {
                kind: CurveInterpolationKind::MovingAverage,
                samples_per_segment: 4,
                closed: false,
                smoothing_window: 5,
            },
        );
        assert!(sampled.len() > points.len());
        assert!(sampled.iter().all(|point| point.y < 10.0));
    }

    #[test]
    fn savitzky_golay_changes_noisy_points() {
        let points = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.2, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 1.5, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(5.0, -0.1, 0.0),
            Vec3::new(6.0, 0.0, 0.0),
        ];
        let sampled = sample_interpolated_points(
            &points,
            CurveInterpolation {
                kind: CurveInterpolationKind::SavitzkyGolay,
                samples_per_segment: 4,
                closed: false,
                smoothing_window: 5,
            },
        );
        assert!(sampled.len() > points.len());
        assert!(sampled.iter().any(|point| point.y < 1.0));
    }
}
