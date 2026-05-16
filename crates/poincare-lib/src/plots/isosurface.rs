//! ContourPlot3D — marching-cubes isosurface extraction plot type.

use viewport_lib::{VolumeData, extract_isosurface};

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotComponent, PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

/// A 3D isosurface plot that extracts contour surfaces via marching cubes.
///
/// The scalar field closure is evaluated on a regular 3D grid, then
/// [`viewport_lib::extract_isosurface`] is called once per isovalue to
/// produce a triangle mesh. All meshes are returned as a
/// [`PlotGeometry::Composite`] so each surface can carry its own style.
///
/// Colors cycle through a default palette when `per_iso_styles` is not set.
pub struct ContourPlot3D {
    field_fn: Box<dyn Fn(f64, f64, f64) -> f64 + Send + Sync>,
    isovalues: Vec<f64>,
    resolution: [u32; 3],
    style: PlotStyle,
    per_iso_styles: Option<Vec<PlotStyle>>,
    domain_override: Option<Domain>,
}

/// Default colors cycled across isosurfaces when `per_iso_styles` is absent.
const DEFAULT_ISO_COLORS: [[f32; 4]; 6] = [
    [0.2, 0.6, 1.0, 0.7],
    [1.0, 0.4, 0.2, 0.7],
    [0.2, 0.9, 0.4, 0.7],
    [0.9, 0.8, 0.1, 0.7],
    [0.7, 0.2, 0.9, 0.7],
    [0.1, 0.9, 0.9, 0.7],
];

impl ContourPlot3D {
    /// Create a contour plot from a scalar field closure, a set of iso-values, and grid resolution.
    ///
    /// * `f` — scalar field `f(x, y, z) -> f64`.
    /// * `isovalues` — one isosurface is extracted per value in this slice.
    /// * `resolution` — grid dimensions `[nx, ny, nz]` for field evaluation.
    pub fn from_fn(
        f: impl Fn(f64, f64, f64) -> f64 + Send + Sync + 'static,
        isovalues: &[f64],
        resolution: [u32; 3],
    ) -> Self {
        Self {
            field_fn: Box::new(f),
            isovalues: isovalues.to_vec(),
            resolution,
            style: PlotStyle::default(),
            per_iso_styles: None,
            domain_override: None,
        }
    }

    /// Override the default style (applied to all surfaces when `per_iso_styles` is `None`).
    pub fn with_style(mut self, style: PlotStyle) -> Self {
        self.style = style;
        self
    }

    /// Override the domain for this plot.
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain_override = Some(domain);
        self
    }

    /// Set per-isovalue styles. The slice is indexed cyclically if shorter than
    /// the number of isovalues.
    pub fn with_per_iso_styles(mut self, styles: Vec<PlotStyle>) -> Self {
        self.per_iso_styles = Some(styles);
        self
    }
}

impl PlotObject for ContourPlot3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        None
    }

    fn generate(&self, domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let x0 = *domain.x.start();
        let x1 = *domain.x.end();
        let y0 = *domain.y.start();
        let y1 = *domain.y.end();
        let z0 = *domain.z.start();
        let z1 = *domain.z.end();

        let [nx, ny, nz] = self.resolution;
        let nx = nx.max(2) as usize;
        let ny = ny.max(2) as usize;
        let nz = nz.max(2) as usize;

        let dx = (x1 - x0) / (nx - 1) as f64;
        let dy = (y1 - y0) / (ny - 1) as f64;
        let dz = (z1 - z0) / (nz - 1) as f64;

        // Evaluate the scalar field on a regular grid (x-fastest layout).
        let mut scalar_data = Vec::with_capacity(nx * ny * nz);
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let x = x0 + ix as f64 * dx;
                    let y = y0 + iy as f64 * dy;
                    let z = z0 + iz as f64 * dz;
                    scalar_data.push((self.field_fn)(x, y, z) as f32);
                }
            }
        }

        let volume_data = VolumeData {
            data: scalar_data,
            dims: [nx as u32, ny as u32, nz as u32],
            origin: [x0 as f32, y0 as f32, z0 as f32],
            spacing: [dx as f32, dy as f32, dz as f32],
        };

        let mut components = Vec::with_capacity(self.isovalues.len());

        for (i, &iso) in self.isovalues.iter().enumerate() {
            let mesh = extract_isosurface(&volume_data, iso as f32);

            let iso_style = if let Some(styles) = &self.per_iso_styles {
                if styles.is_empty() {
                    self.style.clone()
                } else {
                    styles[i % styles.len()].clone()
                }
            } else {
                // Cycle through the default color palette.
                let color = DEFAULT_ISO_COLORS[i % DEFAULT_ISO_COLORS.len()];
                PlotStyle {
                    colour_mode: crate::style::ColourMode::Solid(color),
                    opacity: self.style.opacity,
                    two_sided: true,
                    shading: self.style.shading,
                    ..PlotStyle::default()
                }
            };

            components.push(PlotComponent {
                geometry: PlotGeometry::Surface(mesh),
                style: iso_style,
            });
        }

        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn domain_override(&self) -> Option<&Domain> {
        self.domain_override.as_ref()
    }
}
