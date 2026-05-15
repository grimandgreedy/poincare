use std::ops::RangeInclusive;

use glam::Vec3;

use crate::coordinate::{CoordinateSystem, ParametricDomain};
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

// ---------------------------------------------------------------------------
// Internal kind enum
// ---------------------------------------------------------------------------

enum CurveKind {
    Parametric {
        f: Box<dyn Fn(f64) -> glam::DVec3 + Send + Sync>,
        t_range: RangeInclusive<f64>,
    },
    Points(Vec<Vec3>),
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
        Self {
            kind: CurveKind::Points(points.to_vec()),
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
            CurveKind::Points(_) => CoordinateSystem::Cartesian,
        }
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        match &self.kind {
            CurveKind::Parametric { .. } => None,
            CurveKind::Points(pts) => {
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
            CurveKind::Points(pts) => PlotGeometry::Polyline {
                strip_lengths: vec![pts.len() as u32],
                positions: pts.clone(),
                scalars: None,
            },
        }
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }
}
