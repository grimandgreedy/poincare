//! `poincare-lib` — a Mathematica-class 3D graphing library built on `viewport-lib`.
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
//! // Upload meshes once at startup (needs wgpu device + ViewportGpuResources).
//! scene.upload_meshes(device, renderer.resources_mut()).unwrap();
//!
//! // Each frame:
//! let mut frame = scene.build_frame(&camera);
//! frame.camera.viewport_size = [width as f32, height as f32];
//! ```

pub mod axis;
pub mod coordinate;
pub mod domain;
pub mod expr_parser;
pub mod label;
pub mod plot_object;
pub mod plots;
pub mod resolution;
pub mod scene;
pub mod style;
pub mod ticks;

pub use axis::AxisConfig;
pub use coordinate::{CoordinateSystem, ParametricDomain};
pub use domain::{DataBounds, Domain};
pub use expr_parser::{
    AutoDetectResult, DetectedPlotType, ParsedExpr, auto_detect_plot_type, eval_curve_point,
    eval_surface, eval_with_vars, parse_csv_grid, parse_csv_points, parse_curve_expr,
    parse_expr_with_vars, parse_surface_expr, parse_triple_expr,
};
pub use label::WorldLabel;
pub use plot_object::{GlyphInstance, PlotComponent, PlotGeometry, PlotObject};
pub use plots::{
    ContourPlot3D, Curve3D, CurveInterpolation, CurveInterpolationKind, DensityPlot3D,
    LevelSet3D, PiecewisePlot, Scatter3D, StreamPlot3D, Surface3D, VectorField3D,
    sample_curve_points,
};
pub use resolution::Resolution;
pub use scene::{GraphScene, PointPickData, PolylinePickData, ProbePickData, SurfacePickData};
pub use style::{
    ColormapSource, ColourMode, MatcapSource, ParamVisSettings, PlotStyle, ShadingMode,
    SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField, TransferFunction,
};
pub use viewport_lib::GlyphType;
