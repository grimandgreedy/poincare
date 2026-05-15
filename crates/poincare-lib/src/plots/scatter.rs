//! Scatter3D — a point-cloud plot type.

use glam::Vec3;

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

/// A 3D scatter (point-cloud) plot.
///
/// Each point is rendered as a screen-space circle in the viewport.
/// Optional per-point scalar values enable LUT (colormap) coloring.
pub struct Scatter3D {
    points: Vec<Vec3>,
    scalars: Option<Vec<f32>>,
    style: PlotStyle,
    resolution: Resolution,
}

impl Scatter3D {
    /// Create a scatter plot from a slice of 3D positions.
    pub fn from_points(points: &[Vec3]) -> Self {
        Self {
            points: points.to_vec(),
            scalars: None,
            style: PlotStyle::default(),
            resolution: Resolution::default(),
        }
    }

    /// Create a scatter plot with per-point scalar values for colormap coloring.
    ///
    /// # Panics
    ///
    /// Panics if `points.len() != scalars.len()`.
    pub fn from_points_with_scalars(points: &[Vec3], scalars: &[f32]) -> Self {
        assert_eq!(
            points.len(),
            scalars.len(),
            "Scatter3D: points and scalars must have the same length"
        );
        Self {
            points: points.to_vec(),
            scalars: Some(scalars.to_vec()),
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

    /// Access the optional per-point scalars.
    pub fn scalars(&self) -> Option<&[f32]> {
        self.scalars.as_deref()
    }
}

impl PlotObject for Scatter3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        if self.points.is_empty() {
            return None;
        }
        let mut x_min = f64::MAX;
        let mut x_max = f64::MIN;
        let mut y_min = f64::MAX;
        let mut y_max = f64::MIN;
        let mut z_min = f64::MAX;
        let mut z_max = f64::MIN;

        for p in &self.points {
            x_min = x_min.min(p.x as f64);
            x_max = x_max.max(p.x as f64);
            y_min = y_min.min(p.y as f64);
            y_max = y_max.max(p.y as f64);
            z_min = z_min.min(p.z as f64);
            z_max = z_max.max(p.z as f64);
        }

        Some(DataBounds {
            x: x_min..=x_max,
            y: y_min..=y_max,
            z: z_min..=z_max,
        })
    }

    fn generate(&self, _domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        PlotGeometry::Points {
            positions: self.points.clone(),
            scalars: self.scalars.clone(),
        }
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }
}
