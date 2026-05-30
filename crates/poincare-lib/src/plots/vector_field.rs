//! VectorField3D: arrow glyph vector field plot type.

use glam::Vec3;

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{GlyphInstance, PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::{PlotStyle, ShadingMode};

/// A 3D vector field rendered as arrow glyphs.
///
/// The field function is sampled at evenly-spaced seed points within the
/// plotting domain. Each sample becomes an arrow glyph pointing in the
/// field direction, scaled by [`PlotStyle::glyph_scale`].
pub struct VectorField3D {
    field_fn: Box<dyn Fn(f64, f64, f64) -> Vec3 + Send + Sync>,
    /// Number of seed points along each axis `[nx, ny, nz]`.
    seed_resolution: [u32; 3],
    style: PlotStyle,
    resolution: Resolution,
    domain_override: Option<Domain>,
}

impl VectorField3D {
    /// Create a vector field from a closure `f(x, y, z) -> Vec3`.
    ///
    /// `seed_resolution` controls how many sample points are placed along
    /// each axis: `[nx, ny, nz]`. Total glyphs = `nx * ny * nz`.
    pub fn from_fn(
        f: impl Fn(f64, f64, f64) -> Vec3 + Send + Sync + 'static,
        seed_resolution: [u32; 3],
    ) -> Self {
        Self {
            field_fn: Box::new(f),
            seed_resolution,
            style: PlotStyle {
                shading: ShadingMode::Unlit,
                ..PlotStyle::default()
            },
            resolution: Resolution::default(),
            domain_override: None,
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

    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain_override = Some(domain);
        self
    }
}

impl PlotObject for VectorField3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        // Analytical vector field with no natural bounds; caller supplies domain.
        None
    }

    fn generate(&self, domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let [nx, ny, nz] = self.seed_resolution;
        let nx = nx.max(1) as usize;
        let ny = ny.max(1) as usize;
        let nz = nz.max(1) as usize;

        let x0 = *domain.x.start();
        let x1 = *domain.x.end();
        let y0 = *domain.y.start();
        let y1 = *domain.y.end();
        let z0 = *domain.z.start();
        let z1 = *domain.z.end();

        let mut instances = Vec::with_capacity(nx * ny * nz);

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let tx = if nx > 1 {
                        i as f64 / (nx - 1) as f64
                    } else {
                        0.5
                    };
                    let ty = if ny > 1 {
                        j as f64 / (ny - 1) as f64
                    } else {
                        0.5
                    };
                    let tz = if nz > 1 {
                        k as f64 / (nz - 1) as f64
                    } else {
                        0.5
                    };

                    let x = x0 + tx * (x1 - x0);
                    let y = y0 + ty * (y1 - y0);
                    let z = z0 + tz * (z1 - z0);

                    let raw = (self.field_fn)(x, y, z);
                    // Normalize to unit length; glyph_scale is applied at render submission.
                    let display = raw.normalize_or_zero();

                    instances.push(GlyphInstance {
                        position: Vec3::new(x as f32, y as f32, z as f32),
                        vector: display,
                        raw_vector: raw,
                    });
                }
            }
        }

        PlotGeometry::Glyphs(instances)
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
