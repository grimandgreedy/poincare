use glam::Vec3;

use viewport_lib::MeshData;

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

/// A single glyph instance (position + direction vector) for vector field plots.
pub struct GlyphInstance {
    pub position: Vec3,
    /// Display vector used to orient and size the rendered glyph.
    pub vector: Vec3,
    /// Underlying field sample before any display scaling is applied.
    pub raw_vector: Vec3,
}

/// A geometry component with its own style.
pub struct PlotComponent {
    pub geometry: PlotGeometry,
    pub style: PlotStyle,
}

/// CPU-side geometry produced by `PlotObject::generate()`.
///
/// Each variant maps to a corresponding viewport renderer item:
/// - `Surface` → `SceneRenderItem` (uploaded mesh)
/// - `Polyline` → `PolylineItem`
/// - `Points` → `PointCloudItem`
/// - `Glyphs` → `GlyphItem`
/// - `Streamtube` → `StreamtubeItem` (GPU-instanced cylinder tubes)
/// - `Volume` → `VolumeItem` (GPU volume texture uploaded via `upload_volume`)
/// - `Composite` → recursively unpacked
pub enum PlotGeometry {
    Surface(MeshData),
    Polyline {
        positions: Vec<Vec3>,
        strip_lengths: Vec<u32>,
        scalars: Option<Vec<f32>>,
    },
    /// A point cloud with optional per-point scalar values for colormap coloring.
    Points {
        positions: Vec<Vec3>,
        scalars: Option<Vec<f32>>,
    },
    Glyphs(Vec<GlyphInstance>),
    /// Streamtube geometry: one item with all strips concatenated.
    Streamtube {
        /// All strip vertex positions concatenated in order.
        positions: Vec<Vec3>,
        /// Vertex count per individual strip.
        strip_lengths: Vec<u32>,
        /// Tube radius in world-space units.
        radius: f32,
    },
    /// Raw voxel data for GPU upload as a 3-D texture volume.
    Volume {
        /// Scalar values, x-fastest layout (`index = ix + iy*nx + iz*nx*ny`).
        data: Vec<f32>,
        /// Voxel dimensions `[nx, ny, nz]`.
        dims: [u32; 3],
        /// World-space origin (minimum corner).
        origin: [f32; 3],
        /// Voxel spacing `[dx, dy, dz]`.
        spacing: [f32; 3],
    },
    Composite(Vec<PlotComponent>),
}

/// Common interface for all plot types.
///
/// `generate()` always returns Cartesian-space geometry. Coordinate conversion
/// (spherical → Cartesian, etc.) happens inside `generate()`.
pub trait PlotObject: Send + Sync {
    fn coordinate_system(&self) -> CoordinateSystem;

    /// `None` means the caller must provide the domain; auto-fit is not meaningful
    /// for this type (e.g. analytical functions).
    fn natural_bounds(&self) -> Option<DataBounds>;

    fn generate(&self, domain: &Domain, resolution: Resolution) -> PlotGeometry;

    fn style(&self) -> &PlotStyle;

    fn resolution(&self) -> Resolution {
        Resolution::default()
    }

    fn domain_override(&self) -> Option<&Domain> {
        None
    }
}
