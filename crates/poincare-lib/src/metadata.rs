use crate::{Domain, PlotDefinition, PlotSpec, PlotStyle, Resolution, TablePlotTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleCapabilities {
    pub mesh: bool,
    pub line: bool,
    pub point: bool,
    pub glyph: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainEditorMetadata {
    Fixed,
    One { primary: String },
    Two { primary: String, secondary: String },
    Three { x: String, y: String, z: String },
}

impl DomainEditorMetadata {
    pub fn editable_axis_count(&self) -> usize {
        match self {
            Self::Fixed => 0,
            Self::One { .. } => 1,
            Self::Two { .. } => 2,
            Self::Three { .. } => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateSemantics {
    Fixed,
    CartesianSurface,
    CartesianVolume,
    ParametricSurface,
    ParametricCurve,
    SphericalSurface,
    CylindricalSurface,
    PolarCurve,
    ImportedData,
}

#[derive(Clone, Debug)]
pub struct PlotMetadata {
    pub style_caps: StyleCapabilities,
    pub domain_editor: DomainEditorMetadata,
    pub default_domain: Option<Domain>,
    pub default_resolution: Option<Resolution>,
    pub default_style: PlotStyle,
    pub uses_resolution: bool,
    pub uses_seed_resolution: bool,
    pub supports_surface_intersection: bool,
    pub coordinate_semantics: CoordinateSemantics,
    pub required_variables: Vec<String>,
}

impl PlotDefinition {
    pub fn metadata(&self) -> PlotMetadata {
        let default_style = PlotStyle::default();
        let default_domain = Some(Domain::default());
        let default_resolution = Some(Resolution::default());

        match self {
            Self::ContouredSurface { .. } | Self::GridSurface | Self::ExprCartesian { .. } => {
                PlotMetadata {
                    style_caps: StyleCapabilities {
                        mesh: true,
                        line: false,
                        point: false,
                        glyph: false,
                    },
                    domain_editor: DomainEditorMetadata::Two {
                        primary: "X".to_string(),
                        secondary: "Y".to_string(),
                    },
                    default_domain,
                    default_resolution,
                    default_style,
                    uses_resolution: true,
                    uses_seed_resolution: false,
                    supports_surface_intersection: true,
                    coordinate_semantics: CoordinateSemantics::CartesianSurface,
                    required_variables: vec!["x".to_string(), "y".to_string()],
                }
            }
            Self::SphericalHarmonic | Self::ExprSpherical { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: true,
                    line: false,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::Two {
                    primary: "theta".to_string(),
                    secondary: "phi".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: true,
                coordinate_semantics: CoordinateSemantics::SphericalSurface,
                required_variables: vec!["theta".to_string(), "phi".to_string()],
            },
            Self::ExprCylindrical { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: true,
                    line: false,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::Two {
                    primary: "theta".to_string(),
                    secondary: "z".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: true,
                coordinate_semantics: CoordinateSemantics::CylindricalSurface,
                required_variables: vec!["theta".to_string(), "z".to_string()],
            },
            Self::ExprPolar { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: true,
                    line: false,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::One {
                    primary: "theta".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: true,
                coordinate_semantics: CoordinateSemantics::PolarCurve,
                required_variables: vec!["theta".to_string()],
            },
            Self::ExprParametricSurface { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: true,
                    line: false,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::Two {
                    primary: "U".to_string(),
                    secondary: "V".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: true,
                coordinate_semantics: CoordinateSemantics::ParametricSurface,
                required_variables: vec!["u".to_string(), "v".to_string()],
            },
            Self::HelixCurve | Self::ExprCurve { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: false,
                    line: true,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::One {
                    primary: "T".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: false,
                coordinate_semantics: CoordinateSemantics::ParametricCurve,
                required_variables: vec!["t".to_string()],
            },
            Self::ExprCartesianLine { ind_var, .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: false,
                    line: true,
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::One {
                    primary: ind_var.to_uppercase(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: true,
                uses_seed_resolution: false,
                supports_surface_intersection: false,
                coordinate_semantics: CoordinateSemantics::ParametricCurve,
                required_variables: vec![ind_var.clone()],
            },
            Self::ScatterCloud | Self::PointAnnotations { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: false,
                    line: false,
                    point: true,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::Fixed,
                default_domain: None,
                default_resolution: None,
                default_style,
                uses_resolution: false,
                uses_seed_resolution: false,
                supports_surface_intersection: false,
                coordinate_semantics: CoordinateSemantics::Fixed,
                required_variables: Vec::new(),
            },
            Self::VectorField
            | Self::ExprVectorField { .. }
            | Self::VectorSlice { .. }
            | Self::GradientField { .. }
            | Self::CurlField { .. }
            | Self::ArrowAnnotations { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: false,
                    line: false,
                    point: false,
                    glyph: true,
                },
                domain_editor: match self {
                    Self::VectorSlice { .. } | Self::ArrowAnnotations { .. } => {
                        DomainEditorMetadata::Fixed
                    }
                    _ => DomainEditorMetadata::Three {
                        x: "X".to_string(),
                        y: "Y".to_string(),
                        z: "Z".to_string(),
                    },
                },
                default_domain: match self {
                    Self::VectorSlice { .. } | Self::ArrowAnnotations { .. } => None,
                    _ => default_domain,
                },
                default_resolution,
                default_style,
                uses_resolution: !matches!(self, Self::ArrowAnnotations { .. }),
                uses_seed_resolution: matches!(
                    self,
                    Self::VectorField
                        | Self::ExprVectorField { .. }
                        | Self::VectorSlice { .. }
                        | Self::GradientField { .. }
                        | Self::CurlField { .. }
                ),
                supports_surface_intersection: false,
                coordinate_semantics: match self {
                    Self::ArrowAnnotations { .. } => CoordinateSemantics::Fixed,
                    _ => CoordinateSemantics::CartesianVolume,
                },
                required_variables: match self {
                    Self::ArrowAnnotations { .. } => Vec::new(),
                    _ => vec!["x".to_string(), "y".to_string(), "z".to_string()],
                },
            },
            Self::Streamlines { .. } | Self::VolumeRender { .. } | Self::Isosurface { .. } => {
                PlotMetadata {
                    style_caps: StyleCapabilities {
                        mesh: matches!(self, Self::Isosurface { .. }),
                        line: matches!(self, Self::Streamlines { .. }),
                        point: false,
                        glyph: false,
                    },
                    domain_editor: DomainEditorMetadata::Three {
                        x: "X".to_string(),
                        y: "Y".to_string(),
                        z: "Z".to_string(),
                    },
                    default_domain,
                    default_resolution,
                    default_style,
                    uses_resolution: !matches!(self, Self::Streamlines { .. }),
                    uses_seed_resolution: false,
                    supports_surface_intersection: matches!(self, Self::Isosurface { .. }),
                    coordinate_semantics: CoordinateSemantics::CartesianVolume,
                    required_variables: Vec::new(),
                }
            }
            Self::ExprVolume { .. }
            | Self::ExprIsosurface { .. }
            | Self::ExprStreamlines { .. }
            | Self::DivergenceField { .. } => PlotMetadata {
                style_caps: StyleCapabilities {
                    mesh: matches!(self, Self::ExprIsosurface { .. }),
                    line: matches!(self, Self::ExprStreamlines { .. }),
                    point: false,
                    glyph: false,
                },
                domain_editor: DomainEditorMetadata::Three {
                    x: "X".to_string(),
                    y: "Y".to_string(),
                    z: "Z".to_string(),
                },
                default_domain,
                default_resolution,
                default_style,
                uses_resolution: !matches!(self, Self::ExprStreamlines { .. }),
                uses_seed_resolution: false,
                supports_surface_intersection: matches!(self, Self::ExprIsosurface { .. }),
                coordinate_semantics: CoordinateSemantics::CartesianVolume,
                required_variables: vec!["x".to_string(), "y".to_string(), "z".to_string()],
            },
            Self::ImportedTable { definition } => PlotMetadata {
                style_caps: match definition.target {
                    TablePlotTarget::SurfaceGrid => StyleCapabilities {
                        mesh: true,
                        line: false,
                        point: false,
                        glyph: false,
                    },
                    TablePlotTarget::Curve => StyleCapabilities {
                        mesh: false,
                        line: true,
                        point: false,
                        glyph: false,
                    },
                    TablePlotTarget::Scatter => StyleCapabilities {
                        mesh: false,
                        line: false,
                        point: true,
                        glyph: false,
                    },
                    TablePlotTarget::VectorField => StyleCapabilities {
                        mesh: false,
                        line: false,
                        point: false,
                        glyph: true,
                    },
                },
                domain_editor: DomainEditorMetadata::Fixed,
                default_domain: None,
                default_resolution: None,
                default_style,
                uses_resolution: false,
                uses_seed_resolution: false,
                supports_surface_intersection: matches!(
                    definition.target,
                    TablePlotTarget::SurfaceGrid
                ),
                coordinate_semantics: CoordinateSemantics::ImportedData,
                required_variables: Vec::new(),
            },
            Self::ScalarSlice { .. }
            | Self::DerivedPolylineGroups { .. }
            | Self::InterpolatedCurve { .. } => PlotMetadata {
                style_caps: match self {
                    Self::ScalarSlice { .. } => StyleCapabilities {
                        mesh: true,
                        line: false,
                        point: false,
                        glyph: false,
                    },
                    _ => StyleCapabilities {
                        mesh: false,
                        line: true,
                        point: false,
                        glyph: false,
                    },
                },
                domain_editor: DomainEditorMetadata::Fixed,
                default_domain: None,
                default_resolution: match self {
                    Self::ScalarSlice { .. } => default_resolution,
                    _ => None,
                },
                default_style,
                uses_resolution: matches!(self, Self::ScalarSlice { .. }),
                uses_seed_resolution: false,
                supports_surface_intersection: matches!(self, Self::ScalarSlice { .. }),
                coordinate_semantics: CoordinateSemantics::Fixed,
                required_variables: match self {
                    Self::ScalarSlice { .. } => {
                        vec!["x".to_string(), "y".to_string(), "z".to_string()]
                    }
                    _ => Vec::new(),
                },
            },
        }
    }
}

impl PlotSpec {
    pub fn metadata(&self) -> PlotMetadata {
        self.definition.metadata()
    }
}
