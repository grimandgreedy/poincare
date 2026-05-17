use viewport_lib::{
    AttributeKind, BuiltinColourmap, BuiltinMatcap, ColourmapId, GlyphType, ParamVis,
    ParamVisMode,
};

/// Transfer function for volume opacity control.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferFunction {
    /// Global opacity multiplier applied to all samples. Default: 0.5.
    pub opacity_scale: f32,
    /// Optional scalar threshold range `(min, max)`. Samples outside this range
    /// are discarded (set to zero opacity). `None` means no threshold clipping.
    pub threshold: Option<(f32, f32)>,
}

impl Default for TransferFunction {
    fn default() -> Self {
        Self {
            opacity_scale: 0.5,
            threshold: None,
        }
    }
}

/// Colour source for a plot.
#[derive(Debug, Clone, PartialEq)]
pub enum ColourMode {
    /// Fixed RGBA colour in linear 0..1.
    Solid([f32; 4]),
    /// Colour by the plot's default scalar field using a colormap.
    Colormap {
        /// Built-in or previously uploaded LUT.
        colormap: ColormapSource,
        /// Optional explicit scalar range. `None` lets the renderer auto-fit.
        scalar_range: Option<(f32, f32)>,
    },
    /// Colour by a named scalar attribute.
    ///
    /// Surfaces expose `x`, `y`, `z`, `radius`, and `value`.
    /// Non-surface plots also accept `x`, `y`, `z`, `radius`, `magnitude`, and `index`;
    /// scatter plots with explicit scalars additionally accept `scalar` and `value`.
    ByAttribute {
        /// Name of the attribute as stored on the generated geometry.
        name: String,
        /// Attribute interpolation domain.
        kind: AttributeKind,
    },
}

impl Default for ColourMode {
    fn default() -> Self {
        Self::Solid([0.4, 0.6, 1.0, 1.0])
    }
}

/// Colormap source used by [`ColourMode::Colormap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColormapSource {
    /// One of the viewport's built-in LUT presets.
    Builtin(BuiltinColourmap),
    /// A caller-uploaded LUT.
    Uploaded(ColourmapId),
}

impl Default for ColormapSource {
    fn default() -> Self {
        Self::Builtin(BuiltinColourmap::Viridis)
    }
}

/// Surface shading model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadingMode {
    /// One normal per triangle. Implemented for surfaces by expanding the mesh.
    Flat,
    /// Standard lit shading using per-vertex normals.
    #[default]
    Smooth,
    /// Ignore scene lights and render with ambient-only colour.
    Unlit,
}

/// Surface matcap source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatcapSource {
    /// One of the built-in viewport matcaps.
    Builtin(BuiltinMatcap),
}

/// Preset face quantities generated for analytical surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFaceQuantity {
    /// Sum of per-corner angle deviation between UV and world triangles.
    AngleDistortion,
    /// Triangle area ratio between world and UV domains.
    AreaDistortion,
}

impl SurfaceFaceQuantity {
    /// Stable attribute key used in `MeshData::attributes`.
    pub fn attribute_name(self) -> &'static str {
        match self {
            Self::AngleDistortion => "angle_distortion",
            Self::AreaDistortion => "area_distortion",
        }
    }
}

/// Surface UV visualization presets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamVisSettings {
    /// Which procedural UV pattern to show.
    pub mode: ParamVisMode,
    /// Tile frequency multiplier.
    pub scale: f32,
}

impl Default for ParamVisSettings {
    fn default() -> Self {
        Self {
            mode: ParamVisMode::Checker,
            scale: 8.0,
        }
    }
}

impl From<ParamVisSettings> for ParamVis {
    fn from(value: ParamVisSettings) -> Self {
        Self {
            mode: value.mode,
            scale: value.scale,
        }
    }
}

/// Built-in vector fields available for surface LIC rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLicVectorField {
    /// Follow the surface tangent in the increasing U direction.
    TangentU,
    /// Follow the surface tangent in the increasing V direction.
    TangentV,
    /// Diagonal flow: normalized sum of TangentU and TangentV.
    /// Produces helical / winding streaks across both parametric directions.
    Diagonal,
    /// Saddle flow: normalized difference of TangentU and TangentV.
    /// Creates saddle-point topology — flow converges along U and diverges along V.
    Saddle,
}

impl SurfaceLicVectorField {
    /// Stable mesh attribute name used by the viewport LIC pass.
    pub fn attribute_name(self) -> &'static str {
        match self {
            Self::TangentU => "tangent_u",
            Self::TangentV => "tangent_v",
            Self::Diagonal => "tangent_diagonal",
            Self::Saddle => "tangent_saddle",
        }
    }
}

/// Surface LIC configuration carried on plot styles.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceLicSettings {
    /// Which per-vertex vector field to advect along.
    pub vector_field: SurfaceLicVectorField,
    /// Number of forward/backward advection steps.
    pub steps: u32,
    /// Screen-space step size in pixels.
    pub step_size: f32,
    /// Contrast / modulation strength.
    pub strength: f32,
}

impl Default for SurfaceLicSettings {
    fn default() -> Self {
        Self {
            vector_field: SurfaceLicVectorField::TangentU,
            steps: 20,
            step_size: 1.5,
            strength: 2.0,
        }
    }
}

/// Visual appearance of a plot object.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotStyle {
    /// Colour source. Default: light blue solid fill.
    pub colour_mode: ColourMode,
    /// Global opacity multiplier. Default: 1.0.
    pub opacity: f32,
    /// Whether mesh surfaces should render without backface culling. Default: false.
    pub two_sided: bool,
    /// Hardware line width in pixels for curves. Default: 2.0.
    pub line_width: f32,
    /// Screen-space point size in pixels for Scatter3D. Default: 4.0.
    pub point_size: f32,
    /// Global scale applied to glyph arrows in VectorField3D. Default: 1.0.
    pub glyph_scale: f32,
    /// Glyph mesh used for vector field instances. Default: arrow.
    pub glyph_type: GlyphType,
    /// Surface shading mode. Default: smooth.
    pub shading: ShadingMode,
    /// Tube radius for StreamPlot3D. When `Some(r)`, streamlines are rendered as
    /// solid tubes with radius `r` instead of polylines. Default: `None`.
    pub tube_radius: Option<f32>,
    /// Transfer function for volume opacity control used by DensityPlot3D.
    /// `None` uses a default linear ramp with `opacity_scale = 0.5`.
    pub transfer_function: Option<TransferFunction>,
    /// Optional built-in matcap preset for surface rendering.
    pub matcap: Option<MatcapSource>,
    /// Optional procedural UV visualization for surfaces.
    pub param_vis: Option<ParamVisSettings>,
    /// Optional face quantity to color analytical surfaces by.
    pub face_quantity: Option<SurfaceFaceQuantity>,
    /// Optional surface line integral convolution overlay.
    pub surface_lic: Option<SurfaceLicSettings>,
}

impl Default for PlotStyle {
    fn default() -> Self {
        Self {
            colour_mode: ColourMode::default(),
            opacity: 1.0,
            two_sided: false,
            line_width: 2.0,
            point_size: 4.0,
            glyph_scale: 1.0,
            glyph_type: GlyphType::Arrow,
            shading: ShadingMode::Smooth,
            tube_radius: None,
            transfer_function: None,
            matcap: None,
            param_vis: None,
            face_quantity: None,
            surface_lic: None,
        }
    }
}
