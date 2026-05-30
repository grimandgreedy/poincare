//! `poincare-lib` is a GPU-accelerated 3D graphing library built on [`viewport-lib`].
//!
//! # Quick start
//!
//! ```rust,ignore
//! use poincare_lib::{GraphScene, Surface3D};
//! use viewport_lib::Camera;
//!
//! let mut scene = GraphScene::new();
//! scene.add(Surface3D::from_fn(|x, y| x.sin() * y.cos()));
//!
//! // Upload meshes once at startup. Requires a wgpu Device, Queue, and a
//! // ViewportGpuResources handle from viewport-lib's ViewportRenderer.
//! scene.upload_meshes(&device, &queue, renderer.resources_mut()).unwrap();
//!
//! // Each frame:
//! let mut frame = scene.build_frame(&camera);
//! frame.camera.viewport_size = [width as f32, height as f32];
//! ```

pub mod analysis;
pub mod axis;
pub mod coordinate;
pub mod diagnostics;
pub mod domain;
pub mod expr_parser;
pub mod expressions;
pub mod graph_compile;
pub mod graph_spec;
pub mod label;
pub mod metadata;
pub mod plot_object;
pub mod plots;
pub mod resolution;
pub mod scene;
pub mod solvers;
pub mod style;
pub mod table_data;
pub mod ticks;

pub use analysis::{
    AnalysisCapability, AnalysisError, AnalysisKind, AnalysisOutput, AnalysisOutputKind,
    AnalysisProvenance, AnalysisReport, AnalysisRequest, AnalysisTable, AnalysisTarget,
    AnalysisTargetKind, FrameField, FrameSample, SampleGroupsKind, available_analyses, run_analysis,
    run_curve_surface_frame_analysis, run_surface_mesh_analysis, sample_groups,
};
pub use axis::AxisConfig;
pub use coordinate::{CoordinateSystem, ParametricDomain};
pub use diagnostics::{
    Diagnostic, DiagnosticKind, DiagnosticLocation, DiagnosticSeverity, ParseDiagnostic,
    ValidationDiagnostic,
};
pub use domain::{DataBounds, Domain};
pub use expr_parser::{
    AutoDetectResult, DetectedPlotType, ParsedExpr, auto_detect_plot_type, eval_curve_point,
    eval_surface, eval_with_vars, parse_csv_grid, parse_csv_points, parse_curve_expr,
    parse_expr_with_vars, parse_surface_expr, parse_triple_expr,
};
pub use expressions::{
    ParametricCurveExpr, ParametricSurfaceExpr, ScalarFieldExpr, VectorFieldExpr,
};
pub use graph_compile::GraphBuildError;
pub use graph_spec::{
    ArrowAnnotation, GraphSpec, OptionalColumn, PlotDefinition, PlotSpec, PointAnnotation,
    SeedMode, SliceAxis, TableColumnMapping, TableDelimiter, TableImportDefinition,
    TablePlotTarget,
};
pub use label::WorldLabel;
pub use metadata::{CoordinateSemantics, DomainEditorMetadata, PlotMetadata, StyleCapabilities};
pub use plot_object::{GlyphInstance, PlotComponent, PlotGeometry, PlotObject};
pub use plots::{
    AnnotatedArrowsPlot, AnnotatedPointsPlot, ContourPlot3D, Curve3D, CurveInterpolation,
    CurveInterpolationKind, DensityPlot3D, LevelSet3D, PiecewisePlot, PlaneVectorFieldPlot,
    ScalarSlicePlot, Scatter3D, StreamPlot3D, Surface3D, TableVectorFieldPlot, TableVectorSample,
    VectorField3D, default_slice_position, sample_curve_points,
};
pub use resolution::Resolution;
pub use scene::{GraphScene, PointPickData, PolylinePickData, ProbePickData, SurfacePickData};
pub use solvers::{
    FiniteDifferenceConfig, finite_curl, finite_divergence, finite_gradient, generate_seeds,
};
pub use style::{
    ColormapSource, ColourMode, MatcapSource, ParamVisSettings, PlotStyle, ShadingMode,
    SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField, TransferFunction,
};
pub use table_data::{
    TableDataSet, TablePreview, TableRow, TableValidationError, build_curve_piecewise,
    build_curve_piecewise_with_interpolation,
};
pub use viewport_lib::GlyphType;
