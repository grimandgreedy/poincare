//! StreamPlot3D — RK4 streamline/streamtube plot type.

use glam::Vec3;

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

/// A 3D streamline plot that integrates field lines using 4th-order Runge-Kutta.
///
/// Each seed point produces one trajectory. Integration stops when:
/// - `max_steps` is reached,
/// - the current position leaves the domain bounding box, or
/// - the velocity magnitude falls below `1e-6` (stagnation point).
///
/// If [`PlotStyle::tube_radius`] is `Some(r)`, returns
/// [`PlotGeometry::Streamtube`] (rendered as solid tubes).
/// Otherwise returns [`PlotGeometry::Polyline`] with per-vertex velocity
/// magnitude as scalars for colormap mapping.
pub struct StreamPlot3D {
    field_fn: Box<dyn Fn(Vec3) -> Vec3 + Send + Sync>,
    seeds: Vec<Vec3>,
    step_size: f32,
    max_steps: u32,
    style: PlotStyle,
    resolution: Resolution,
    domain_override: Option<Domain>,
}

impl StreamPlot3D {
    /// Create a streamline plot from a velocity field closure and a set of seed points.
    ///
    /// * `field` — closure `f(p: Vec3) -> Vec3` returning the velocity at position `p`.
    /// * `seeds` — starting positions for streamline integration.
    /// * `step_size` — RK4 step size in world units per step (e.g. `0.05`).
    /// * `max_steps` — maximum integration steps per seed before stopping.
    pub fn from_field(
        field: impl Fn(Vec3) -> Vec3 + Send + Sync + 'static,
        seeds: &[Vec3],
        step_size: f32,
        max_steps: u32,
    ) -> Self {
        Self {
            field_fn: Box::new(field),
            seeds: seeds.to_vec(),
            step_size,
            max_steps,
            style: PlotStyle::default(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Override the default style.
    pub fn with_style(mut self, style: PlotStyle) -> Self {
        self.style = style;
        self
    }

    /// Override the domain for this plot.
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain_override = Some(domain);
        self
    }

    /// Override the resolution (not used internally but stored for property panel).
    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = resolution;
        self
    }
}

/// Advance one RK4 step: returns `p_next`.
#[inline]
fn rk4_step(f: &dyn Fn(Vec3) -> Vec3, p: Vec3, h: f32) -> Vec3 {
    let k1 = h * f(p);
    let k2 = h * f(p + k1 * 0.5);
    let k3 = h * f(p + k2 * 0.5);
    let k4 = h * f(p + k3);
    p + (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0
}

impl PlotObject for StreamPlot3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        None
    }

    fn generate(&self, domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let x_min = *domain.x.start() as f32;
        let x_max = *domain.x.end() as f32;
        let y_min = *domain.y.start() as f32;
        let y_max = *domain.y.end() as f32;
        let z_min = *domain.z.start() as f32;
        let z_max = *domain.z.end() as f32;

        let in_domain = |p: Vec3| {
            p.x >= x_min
                && p.x <= x_max
                && p.y >= y_min
                && p.y <= y_max
                && p.z >= z_min
                && p.z <= z_max
        };

        let mut all_positions: Vec<Vec3> = Vec::new();
        let mut strip_lengths: Vec<u32> = Vec::new();
        let mut scalars: Vec<f32> = Vec::new();

        for &seed in &self.seeds {
            let mut p = seed;
            let mut strip_count: u32 = 0;

            // Always include the seed itself.
            if in_domain(p) {
                all_positions.push(p);
                scalars.push((self.field_fn)(p).length());
                strip_count += 1;
            }

            for _ in 0..self.max_steps {
                let velocity = (self.field_fn)(p);
                if velocity.length() < 1e-6 {
                    break;
                }

                let p_next = rk4_step(self.field_fn.as_ref(), p, self.step_size);

                if !in_domain(p_next) {
                    break;
                }

                p = p_next;
                let mag = (self.field_fn)(p).length();
                all_positions.push(p);
                scalars.push(mag);
                strip_count += 1;
            }

            if strip_count >= 2 {
                strip_lengths.push(strip_count);
            } else if strip_count == 1 {
                // Remove the single dangling point — it can't form a line.
                all_positions.pop();
                scalars.pop();
            }
        }

        if all_positions.is_empty() {
            return PlotGeometry::Polyline {
                positions: Vec::new(),
                strip_lengths: Vec::new(),
                scalars: None,
            };
        }

        match self.style.tube_radius {
            Some(radius) => PlotGeometry::Streamtube {
                positions: all_positions,
                strip_lengths,
                radius,
            },
            None => PlotGeometry::Polyline {
                positions: all_positions,
                strip_lengths,
                scalars: Some(scalars),
            },
        }
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }

    fn domain_override(&self) -> Option<&Domain> {
        self.domain_override.as_ref()
    }
}
