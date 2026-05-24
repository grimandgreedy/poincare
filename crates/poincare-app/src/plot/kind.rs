use poincare_lib::{
    CurveInterpolation, DomainEditorMetadata, PlotMetadata, PlotStyle,
    StyleCapabilities as LibStyleCapabilities,
};

use crate::plot::analysis::{ArrowAnnotation, PointAnnotation, SliceAxis};
use crate::plot::table::TableImportDefinition;

/// Default palette for isosurface per-level colours.
pub(crate) const DEFAULT_ISO_PALETTE: [[f32; 4]; 6] = [
    [0.2, 0.6, 1.0, 0.7],
    [1.0, 0.4, 0.2, 0.7],
    [0.2, 0.9, 0.4, 0.7],
    [0.9, 0.8, 0.1, 0.7],
    [0.7, 0.2, 0.9, 0.7],
    [0.1, 0.9, 0.9, 0.7],
];

/// How streamline seed points are generated.
#[derive(Clone)]
pub(crate) enum SeedMode {
    Grid {
        nx: u32,
        ny: u32,
        nz: u32,
    },
    /// A regular grid on one axis-aligned plane at the given offset.
    Plane {
        axis: usize,
        offset: f32,
    },
    ManualCsv {
        csv_text: String,
    },
}

impl Default for SeedMode {
    fn default() -> Self {
        Self::Grid {
            nx: 3,
            ny: 3,
            nz: 3,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StyleCaps {
    pub(crate) mesh: bool,
    pub(crate) line: bool,
    pub(crate) point: bool,
    pub(crate) glyph: bool,
}

/// Which axes the domain panel should display for a given plot type.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum DomainLabels {
    /// Fixed construction data — no domain fields to edit.
    None,
    /// Cartesian surface: X, Y only (Z is the output).
    Xy,
    /// Full 3D domain: X, Y, Z.
    Xyz,
    /// Parametric surface: U, V.
    Uv,
    /// Spherical: θ (elevation), φ (azimuth).
    ThetaPhi,
    /// Cylindrical: θ (azimuth), Z.
    ThetaZ,
    /// Polar: θ only.
    Theta,
    /// Parametric curve: T only.
    T,
    /// Single axis with a custom label (e.g. the independent variable of a Cartesian line).
    /// The domain x field is used as the range.
    SingleVar(String),
}

impl From<LibStyleCapabilities> for StyleCaps {
    fn from(value: LibStyleCapabilities) -> Self {
        Self {
            mesh: value.mesh,
            line: value.line,
            point: value.point,
            glyph: value.glyph,
        }
    }
}

impl From<DomainEditorMetadata> for DomainLabels {
    fn from(value: DomainEditorMetadata) -> Self {
        match value {
            DomainEditorMetadata::Fixed => Self::None,
            DomainEditorMetadata::One { primary } => match primary.as_str() {
                "theta" => Self::Theta,
                "T" => Self::T,
                _ => Self::SingleVar(primary),
            },
            DomainEditorMetadata::Two { primary, secondary } => match (primary.as_str(), secondary.as_str()) {
                ("X", "Y") => Self::Xy,
                ("theta", "phi") => Self::ThetaPhi,
                ("theta", "z") => Self::ThetaZ,
                ("U", "V") => Self::Uv,
                _ => Self::None,
            },
            DomainEditorMetadata::Three { .. } => Self::Xyz,
        }
    }
}

#[derive(Clone)]
pub(crate) enum PlotKind {
    ContouredSurface {
        contour_values: Vec<f32>,
        contour_style: PlotStyle,
    },
    SphericalHarmonic,
    HelixCurve,
    ScatterCloud,
    VectorField,
    GridSurface,
    Streamlines {
        seeds: Vec<glam::Vec3>,
    },
    VolumeRender {
        resolution: [u32; 3],
    },
    Isosurface {
        isovalues: Vec<f64>,
        resolution: [u32; 3],
    },
    /// Cartesian surface z = f(x, y).
    ExprCartesian {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Parametric curve (x(t), y(t), z(t)).
    ExprCurve {
        expression: String,
        parameters: Vec<(String, f64)>,
        t_range: (f64, f64),
    },
    /// Cartesian line: dep_var(ind_var) = expression.
    /// e.g. y(x) = sin(x*c), z(x) = x^2, x(y) = cos(y).
    ExprCartesianLine {
        dep_var: String,
        ind_var: String,
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Spherical surface r = f(theta, phi).
    ExprSpherical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Cylindrical surface r = f(theta, z).
    ExprCylindrical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Polar surface r = f(theta).
    ExprPolar {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Parametric surface (x(u,v), y(u,v), z(u,v)) stored as "ex|ey|ez".
    ExprParametricSurface {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Imported table-backed plot.
    ImportedTable {
        definition: TableImportDefinition,
    },
    ScalarSlice {
        expression: String,
        parameters: Vec<(String, f64)>,
        axis: SliceAxis,
        position: f64,
        contour_values: Vec<f32>,
        contour_style: PlotStyle,
    },
    VectorSlice {
        expression: String,
        parameters: Vec<(String, f64)>,
        axis: SliceAxis,
        position: f64,
    },
    GradientField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    DivergenceField {
        expression: String,
        parameters: Vec<(String, f64)>,
        vol_resolution: [u32; 3],
    },
    CurlField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    PointAnnotations {
        points: Vec<PointAnnotation>,
        show_labels: bool,
    },
    ArrowAnnotations {
        arrows: Vec<ArrowAnnotation>,
        show_labels: bool,
    },
    DerivedPolylineGroups {
        groups: Vec<Vec<[f32; 3]>>,
    },
    InterpolatedCurve {
        points: Vec<[f32; 3]>,
        interpolation: CurveInterpolation,
    },
    /// Vector field (vx(x,y,z), vy(x,y,z), vz(x,y,z)).
    ExprVectorField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    /// Volume density = f(x, y, z).
    ExprVolume {
        expression: String,
        parameters: Vec<(String, f64)>,
        vol_resolution: [u32; 3],
    },
    /// Isosurface from expression f(x,y,z) with editable level list.
    ExprIsosurface {
        expression: String,
        parameters: Vec<(String, f64)>,
        isovalues: Vec<f64>,
        iso_colours: Vec<[f32; 4]>,
        vol_resolution: [u32; 3],
    },
    /// Streamlines from vector field expression.
    ExprStreamlines {
        expression: String,
        parameters: Vec<(String, f64)>,
        seed_mode: SeedMode,
        step_size: f32,
        max_steps: u32,
    },
}

impl PlotKind {
    fn metadata(&self) -> PlotMetadata {
        self.to_plot_definition().metadata()
    }

    pub(crate) fn style_caps(&self) -> StyleCaps {
        self.metadata().style_caps.into()
    }

    pub(crate) fn domain_labels(&self) -> DomainLabels {
        self.metadata().domain_editor.into()
    }

    pub(crate) fn uses_resolution(&self) -> bool {
        self.metadata().uses_resolution
    }

    pub(crate) fn uses_seed_resolution(&self) -> bool {
        self.metadata().uses_seed_resolution
    }

    pub(crate) fn supports_surface_intersection(&self) -> bool {
        self.metadata().supports_surface_intersection
    }

    /// Return a mutable reference to the parameter list for expression-based plot kinds.
    /// Returns `None` for built-in (non-expression) plot kinds.
    pub(crate) fn parameters_mut(&mut self) -> Option<&mut Vec<(String, f64)>> {
        match self {
            Self::ExprCartesian { parameters, .. }
            | Self::ExprCurve { parameters, .. }
            | Self::ExprCartesianLine { parameters, .. }
            | Self::ExprSpherical { parameters, .. }
            | Self::ExprCylindrical { parameters, .. }
            | Self::ExprPolar { parameters, .. }
            | Self::ExprParametricSurface { parameters, .. }
            | Self::ScalarSlice { parameters, .. }
            | Self::VectorSlice { parameters, .. }
            | Self::GradientField { parameters, .. }
            | Self::DivergenceField { parameters, .. }
            | Self::CurlField { parameters, .. }
            | Self::ExprVectorField { parameters, .. }
            | Self::ExprVolume { parameters, .. }
            | Self::ExprIsosurface { parameters, .. }
            | Self::ExprStreamlines { parameters, .. } => Some(parameters),
            _ => None,
        }
    }
}

pub(crate) fn evenly_spaced_isovalues(count: usize) -> Vec<f32> {
    let count = count.max(1);
    if count == 1 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| -0.9 + 1.8 * i as f32 / (count - 1) as f32)
        .collect()
}
