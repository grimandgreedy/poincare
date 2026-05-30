//! DensityPlot3D: 3D scalar field volume renderer.

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

enum VolumeSource {
    Fn {
        f: Box<dyn Fn(f64, f64, f64) -> f64 + Send + Sync>,
        resolution: [u32; 3],
    },
    Voxels {
        data: Vec<f32>,
        dims: [u32; 3],
    },
}

/// A 3D volume density plot that evaluates a scalar field on a regular grid.
///
/// Two construction modes are supported:
/// - [`from_fn`](DensityPlot3D::from_fn): evaluates a closure on a regular grid.
/// - [`from_voxels`](DensityPlot3D::from_voxels): uses pre-computed voxel data directly.
///
/// The resulting geometry is uploaded as a GPU 3D texture and rendered with
/// direct volume rendering (DVR). Opacity is controlled via
/// [`PlotStyle::transfer_function`].
pub struct DensityPlot3D {
    source: VolumeSource,
    style: PlotStyle,
    domain_override: Option<Domain>,
}

impl DensityPlot3D {
    /// Create a density plot by evaluating `f(x, y, z)` on a regular 3D grid of
    /// `resolution = [nx, ny, nz]` samples spanning the scene domain.
    ///
    /// Data layout: x-fastest (`index = ix + iy*nx + iz*nx*ny`).
    pub fn from_fn(
        f: impl Fn(f64, f64, f64) -> f64 + Send + Sync + 'static,
        resolution: [u32; 3],
    ) -> Self {
        Self {
            source: VolumeSource::Fn {
                f: Box::new(f),
                resolution,
            },
            style: PlotStyle::default(),
            domain_override: None,
        }
    }

    /// Create a density plot from pre-computed voxel data.
    ///
    /// `data` must have length `dims[0] * dims[1] * dims[2]` in x-fastest order.
    /// Origin and spacing are computed from the scene domain at render time.
    pub fn from_voxels(data: &[f32], dims: [u32; 3]) -> Self {
        Self {
            source: VolumeSource::Voxels {
                data: data.to_vec(),
                dims,
            },
            style: PlotStyle::default(),
            domain_override: None,
        }
    }

    /// Override the default style (including transfer function).
    pub fn with_style(mut self, style: PlotStyle) -> Self {
        self.style = style;
        self
    }

    /// Override the domain for this plot.
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain_override = Some(domain);
        self
    }
}

impl PlotObject for DensityPlot3D {
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

        match &self.source {
            VolumeSource::Fn { f, resolution } => {
                let [nx, ny, nz] = *resolution;
                let nx = nx.max(2) as usize;
                let ny = ny.max(2) as usize;
                let nz = nz.max(2) as usize;

                let dx = (x1 - x0) / (nx - 1) as f64;
                let dy = (y1 - y0) / (ny - 1) as f64;
                let dz = (z1 - z0) / (nz - 1) as f64;

                let mut data = Vec::with_capacity(nx * ny * nz);
                for iz in 0..nz {
                    for iy in 0..ny {
                        for ix in 0..nx {
                            let x = x0 + ix as f64 * dx;
                            let y = y0 + iy as f64 * dy;
                            let z = z0 + iz as f64 * dz;
                            data.push(f(x, y, z) as f32);
                        }
                    }
                }

                PlotGeometry::Volume {
                    data,
                    dims: [nx as u32, ny as u32, nz as u32],
                    origin: [x0 as f32, y0 as f32, z0 as f32],
                    spacing: [dx as f32, dy as f32, dz as f32],
                }
            }
            VolumeSource::Voxels { data, dims } => {
                let [nx, ny, nz] = *dims;
                let nx = nx.max(1) as usize;
                let ny = ny.max(1) as usize;
                let nz = nz.max(1) as usize;

                let dx = if nx > 1 {
                    (x1 - x0) / (nx - 1) as f64
                } else {
                    1.0
                };
                let dy = if ny > 1 {
                    (y1 - y0) / (ny - 1) as f64
                } else {
                    1.0
                };
                let dz = if nz > 1 {
                    (z1 - z0) / (nz - 1) as f64
                } else {
                    1.0
                };

                PlotGeometry::Volume {
                    data: data.clone(),
                    dims: [nx as u32, ny as u32, nz as u32],
                    origin: [x0 as f32, y0 as f32, z0 as f32],
                    spacing: [dx as f32, dy as f32, dz as f32],
                }
            }
        }
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn domain_override(&self) -> Option<&Domain> {
        self.domain_override.as_ref()
    }
}
