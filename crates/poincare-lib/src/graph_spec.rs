use serde::{Deserialize, Serialize};

use crate::{AxisConfig, CurveInterpolation, Domain, PlotStyle, Resolution};

/// Canonical library-owned graph document model.
///
/// This is the declarative layer that higher-level clients should build and edit.
/// Later phases can compile this into concrete `PlotObject`s / `GraphScene`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphSpec {
    pub axis_config: AxisConfig,
    pub plots: Vec<PlotSpec>,
}

impl GraphSpec {
    pub fn new() -> Self {
        Self {
            axis_config: AxisConfig::default(),
            plots: Vec::new(),
        }
    }
}

impl Default for GraphSpec {
    fn default() -> Self {
        Self::new()
    }
}

/// Declarative plot entry owned by `poincare-lib`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlotSpec {
    pub name: String,
    pub visible: bool,
    pub domain: Domain,
    pub resolution: Resolution,
    pub style: PlotStyle,
    pub definition: PlotDefinition,
}

/// Canonical plot-definition enum for reusable graphing and analysis layers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlotDefinition {
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
        seeds: Vec<[f32; 3]>,
    },
    VolumeRender {
        resolution: [u32; 3],
    },
    Isosurface {
        isovalues: Vec<f64>,
        resolution: [u32; 3],
    },
    ExprCartesian {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprCurve {
        expression: String,
        parameters: Vec<(String, f64)>,
        t_range: (f64, f64),
    },
    ExprCartesianLine {
        dep_var: String,
        ind_var: String,
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprSpherical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprCylindrical {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprPolar {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprParametricSurface {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
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
    ExprVectorField {
        expression: String,
        parameters: Vec<(String, f64)>,
    },
    ExprVolume {
        expression: String,
        parameters: Vec<(String, f64)>,
        vol_resolution: [u32; 3],
    },
    ExprIsosurface {
        expression: String,
        parameters: Vec<(String, f64)>,
        isovalues: Vec<f64>,
        iso_colours: Vec<[f32; 4]>,
        vol_resolution: [u32; 3],
    },
    ExprStreamlines {
        expression: String,
        parameters: Vec<(String, f64)>,
        seed_mode: SeedMode,
        step_size: f32,
        max_steps: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SliceAxis {
    X,
    Y,
    Z,
}

impl SliceAxis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointAnnotation {
    pub position: [f32; 3],
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrowAnnotation {
    pub origin: [f32; 3],
    pub vector: [f32; 3],
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SeedMode {
    Grid {
        nx: u32,
        ny: u32,
        nz: u32,
    },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableDelimiter {
    Comma,
    Semicolon,
    Tab,
    Space,
    Pipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TablePlotTarget {
    SurfaceGrid,
    Curve,
    Scatter,
    VectorField,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableImportDefinition {
    pub source_path: Option<String>,
    pub raw_text: String,
    pub delimiter: TableDelimiter,
    pub header_row: bool,
    pub target: TablePlotTarget,
    pub mapping: TableColumnMapping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionalColumn {
    None,
    Column(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TableColumnMapping {
    SurfaceGrid {
        x: usize,
        y: usize,
        z: usize,
    },
    Curve {
        x: usize,
        y: usize,
        z: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
    Scatter {
        x: usize,
        y: usize,
        z: OptionalColumn,
        scalar: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
    VectorField {
        x: usize,
        y: usize,
        z: OptionalColumn,
        vx: usize,
        vy: usize,
        vz: OptionalColumn,
        scalar: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
}
