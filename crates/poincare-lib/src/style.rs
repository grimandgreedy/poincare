use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

#[derive(Serialize, Deserialize)]
struct PersistedTransferFunction {
    opacity_scale: f32,
    threshold: Option<(f32, f32)>,
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

#[derive(Serialize, Deserialize)]
enum PersistedColourMode {
    Solid([f32; 4]),
    Colormap {
        colormap: u8,
        scalar_range: Option<(f32, f32)>,
    },
    ByAttribute {
        name: String,
        kind: u8,
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

#[derive(Serialize, Deserialize)]
struct PersistedParamVis {
    mode: u8,
    scale: f32,
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

#[derive(Serialize, Deserialize)]
struct PersistedSurfaceLic {
    vector_field: u8,
    steps: u32,
    step_size: f32,
    strength: f32,
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

#[derive(Serialize, Deserialize)]
struct PersistedPlotStyle {
    colour_mode: PersistedColourMode,
    opacity: f32,
    two_sided: bool,
    line_width: f32,
    point_size: f32,
    glyph_scale: f32,
    glyph_type: u8,
    shading: u8,
    tube_radius: Option<f32>,
    transfer_function: Option<PersistedTransferFunction>,
    matcap: Option<u8>,
    param_vis: Option<PersistedParamVis>,
    face_quantity: Option<u8>,
    surface_lic: Option<PersistedSurfaceLic>,
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

impl Serialize for TransferFunction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PersistedTransferFunction {
            opacity_scale: self.opacity_scale,
            threshold: self.threshold,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TransferFunction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persisted = PersistedTransferFunction::deserialize(deserializer)?;
        Ok(Self {
            opacity_scale: persisted.opacity_scale,
            threshold: persisted.threshold,
        })
    }
}

impl Serialize for ColourMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let persisted = match self {
            Self::Solid(rgba) => PersistedColourMode::Solid(*rgba),
            Self::Colormap {
                colormap,
                scalar_range,
            } => PersistedColourMode::Colormap {
                colormap: match colormap {
                    ColormapSource::Builtin(preset) => builtin_colormap_to_u8(*preset),
                    ColormapSource::Uploaded(_) => builtin_colormap_to_u8(BuiltinColourmap::Viridis),
                },
                scalar_range: *scalar_range,
            },
            Self::ByAttribute { name, kind } => PersistedColourMode::ByAttribute {
                name: name.clone(),
                kind: attribute_kind_to_u8(*kind),
            },
        };
        persisted.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ColourMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persisted = PersistedColourMode::deserialize(deserializer)?;
        Ok(match persisted {
            PersistedColourMode::Solid(rgba) => Self::Solid(rgba),
            PersistedColourMode::Colormap {
                colormap,
                scalar_range,
            } => Self::Colormap {
                colormap: ColormapSource::Builtin(u8_to_builtin_colormap(colormap)),
                scalar_range,
            },
            PersistedColourMode::ByAttribute { name, kind } => Self::ByAttribute {
                name,
                kind: u8_to_attribute_kind(kind),
            },
        })
    }
}

impl Serialize for ParamVisSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PersistedParamVis {
            mode: param_vis_mode_to_u8(self.mode),
            scale: self.scale,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ParamVisSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persisted = PersistedParamVis::deserialize(deserializer)?;
        Ok(Self {
            mode: u8_to_param_vis_mode(persisted.mode),
            scale: persisted.scale,
        })
    }
}

impl Serialize for SurfaceLicSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PersistedSurfaceLic {
            vector_field: surface_lic_vector_field_to_u8(self.vector_field),
            steps: self.steps,
            step_size: self.step_size,
            strength: self.strength,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SurfaceLicSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persisted = PersistedSurfaceLic::deserialize(deserializer)?;
        Ok(Self {
            vector_field: u8_to_surface_lic_vector_field(persisted.vector_field),
            steps: persisted.steps,
            step_size: persisted.step_size,
            strength: persisted.strength,
        })
    }
}

impl Serialize for PlotStyle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PersistedPlotStyle {
            colour_mode: match &self.colour_mode {
                ColourMode::Solid(rgba) => PersistedColourMode::Solid(*rgba),
                ColourMode::Colormap {
                    colormap,
                    scalar_range,
                } => PersistedColourMode::Colormap {
                    colormap: match colormap {
                        ColormapSource::Builtin(preset) => builtin_colormap_to_u8(*preset),
                        ColormapSource::Uploaded(_) => builtin_colormap_to_u8(BuiltinColourmap::Viridis),
                    },
                    scalar_range: *scalar_range,
                },
                ColourMode::ByAttribute { name, kind } => PersistedColourMode::ByAttribute {
                    name: name.clone(),
                    kind: attribute_kind_to_u8(*kind),
                },
            },
            opacity: self.opacity,
            two_sided: self.two_sided,
            line_width: self.line_width,
            point_size: self.point_size,
            glyph_scale: self.glyph_scale,
            glyph_type: glyph_type_to_u8(self.glyph_type),
            shading: shading_to_u8(self.shading),
            tube_radius: self.tube_radius,
            transfer_function: self
                .transfer_function
                .as_ref()
                .map(|tf| PersistedTransferFunction {
                    opacity_scale: tf.opacity_scale,
                    threshold: tf.threshold,
                }),
            matcap: self.matcap.map(|m| match m {
                MatcapSource::Builtin(preset) => builtin_matcap_to_u8(preset),
            }),
            param_vis: self.param_vis.map(|pv| PersistedParamVis {
                mode: param_vis_mode_to_u8(pv.mode),
                scale: pv.scale,
            }),
            face_quantity: self.face_quantity.map(surface_face_quantity_to_u8),
            surface_lic: self.surface_lic.as_ref().map(|lic| PersistedSurfaceLic {
                vector_field: surface_lic_vector_field_to_u8(lic.vector_field),
                steps: lic.steps,
                step_size: lic.step_size,
                strength: lic.strength,
            }),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PlotStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persisted = PersistedPlotStyle::deserialize(deserializer)?;
        Ok(Self {
            colour_mode: match persisted.colour_mode {
                PersistedColourMode::Solid(rgba) => ColourMode::Solid(rgba),
                PersistedColourMode::Colormap {
                    colormap,
                    scalar_range,
                } => ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(u8_to_builtin_colormap(colormap)),
                    scalar_range,
                },
                PersistedColourMode::ByAttribute { name, kind } => ColourMode::ByAttribute {
                    name,
                    kind: u8_to_attribute_kind(kind),
                },
            },
            opacity: persisted.opacity,
            two_sided: persisted.two_sided,
            line_width: persisted.line_width,
            point_size: persisted.point_size,
            glyph_scale: persisted.glyph_scale,
            glyph_type: u8_to_glyph_type(persisted.glyph_type),
            shading: u8_to_shading(persisted.shading),
            tube_radius: persisted.tube_radius,
            transfer_function: persisted.transfer_function.map(|tf| TransferFunction {
                opacity_scale: tf.opacity_scale,
                threshold: tf.threshold,
            }),
            matcap: persisted
                .matcap
                .map(|m| MatcapSource::Builtin(u8_to_builtin_matcap(m))),
            param_vis: persisted.param_vis.map(|pv| ParamVisSettings {
                mode: u8_to_param_vis_mode(pv.mode),
                scale: pv.scale,
            }),
            face_quantity: persisted.face_quantity.map(u8_to_surface_face_quantity),
            surface_lic: persisted.surface_lic.map(|lic| SurfaceLicSettings {
                vector_field: u8_to_surface_lic_vector_field(lic.vector_field),
                steps: lic.steps,
                step_size: lic.step_size,
                strength: lic.strength,
            }),
        })
    }
}

fn builtin_colormap_to_u8(value: BuiltinColourmap) -> u8 {
    match value {
        BuiltinColourmap::Viridis => 0,
        BuiltinColourmap::Plasma => 1,
        BuiltinColourmap::Greyscale => 2,
        BuiltinColourmap::Coolwarm => 3,
        BuiltinColourmap::Rainbow => 4,
        BuiltinColourmap::Magma => 5,
        BuiltinColourmap::Inferno => 6,
        BuiltinColourmap::Turbo => 7,
        BuiltinColourmap::Jet => 8,
        BuiltinColourmap::RdBu => 9,
    }
}

fn u8_to_builtin_colormap(value: u8) -> BuiltinColourmap {
    match value {
        1 => BuiltinColourmap::Plasma,
        2 => BuiltinColourmap::Greyscale,
        3 => BuiltinColourmap::Coolwarm,
        4 => BuiltinColourmap::Rainbow,
        5 => BuiltinColourmap::Magma,
        6 => BuiltinColourmap::Inferno,
        7 => BuiltinColourmap::Turbo,
        8 => BuiltinColourmap::Jet,
        9 => BuiltinColourmap::RdBu,
        _ => BuiltinColourmap::Viridis,
    }
}

fn attribute_kind_to_u8(value: AttributeKind) -> u8 {
    match value {
        AttributeKind::Vertex => 0,
        AttributeKind::Cell => 1,
        AttributeKind::Face => 2,
        AttributeKind::FaceColour => 3,
        AttributeKind::Edge => 4,
        AttributeKind::Halfedge => 5,
        AttributeKind::Corner => 6,
    }
}

fn u8_to_attribute_kind(value: u8) -> AttributeKind {
    match value {
        1 => AttributeKind::Cell,
        2 => AttributeKind::Face,
        3 => AttributeKind::FaceColour,
        4 => AttributeKind::Edge,
        5 => AttributeKind::Halfedge,
        6 => AttributeKind::Corner,
        _ => AttributeKind::Vertex,
    }
}

fn shading_to_u8(value: ShadingMode) -> u8 {
    match value {
        ShadingMode::Flat => 0,
        ShadingMode::Smooth => 1,
        ShadingMode::Unlit => 2,
    }
}

fn u8_to_shading(value: u8) -> ShadingMode {
    match value {
        0 => ShadingMode::Flat,
        2 => ShadingMode::Unlit,
        _ => ShadingMode::Smooth,
    }
}

fn glyph_type_to_u8(value: GlyphType) -> u8 {
    match value {
        GlyphType::Arrow => 0,
        GlyphType::Sphere => 1,
        GlyphType::Cube => 2,
    }
}

fn u8_to_glyph_type(value: u8) -> GlyphType {
    match value {
        1 => GlyphType::Sphere,
        2 => GlyphType::Cube,
        _ => GlyphType::Arrow,
    }
}

fn builtin_matcap_to_u8(value: BuiltinMatcap) -> u8 {
    value as u8
}

fn u8_to_builtin_matcap(value: u8) -> BuiltinMatcap {
    match value {
        1 => BuiltinMatcap::Wax,
        2 => BuiltinMatcap::Candy,
        3 => BuiltinMatcap::Flat,
        4 => BuiltinMatcap::Ceramic,
        5 => BuiltinMatcap::Jade,
        6 => BuiltinMatcap::Mud,
        7 => BuiltinMatcap::Normal,
        _ => BuiltinMatcap::Clay,
    }
}

fn param_vis_mode_to_u8(value: ParamVisMode) -> u8 {
    match value {
        ParamVisMode::Checker => 0,
        ParamVisMode::Grid => 1,
        ParamVisMode::LocalChecker => 2,
        ParamVisMode::LocalRadial => 3,
    }
}

fn u8_to_param_vis_mode(value: u8) -> ParamVisMode {
    match value {
        1 => ParamVisMode::Grid,
        2 => ParamVisMode::LocalChecker,
        3 => ParamVisMode::LocalRadial,
        _ => ParamVisMode::Checker,
    }
}

fn surface_face_quantity_to_u8(value: SurfaceFaceQuantity) -> u8 {
    match value {
        SurfaceFaceQuantity::AngleDistortion => 0,
        SurfaceFaceQuantity::AreaDistortion => 1,
    }
}

fn u8_to_surface_face_quantity(value: u8) -> SurfaceFaceQuantity {
    match value {
        1 => SurfaceFaceQuantity::AreaDistortion,
        _ => SurfaceFaceQuantity::AngleDistortion,
    }
}

fn surface_lic_vector_field_to_u8(value: SurfaceLicVectorField) -> u8 {
    match value {
        SurfaceLicVectorField::TangentU => 0,
        SurfaceLicVectorField::TangentV => 1,
        SurfaceLicVectorField::Diagonal => 2,
        SurfaceLicVectorField::Saddle => 3,
    }
}

fn u8_to_surface_lic_vector_field(value: u8) -> SurfaceLicVectorField {
    match value {
        1 => SurfaceLicVectorField::TangentV,
        2 => SurfaceLicVectorField::Diagonal,
        3 => SurfaceLicVectorField::Saddle,
        _ => SurfaceLicVectorField::TangentU,
    }
}
