use std::fmt;
use std::sync::Arc;

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::expressions::{
    ParametricCurveExpr, ParametricSurfaceExpr, ScalarFieldExpr, VectorFieldExpr,
};
use crate::graph_spec::{GraphSpec, PlotDefinition, PlotSpec};
use crate::{
    AnnotatedArrowsPlot, AnnotatedPointsPlot, ColourMode, ContourPlot3D, Curve3D, DensityPlot3D,
    Domain, FiniteDifferenceConfig, GraphScene, LevelSet3D, PiecewisePlot, PlaneVectorFieldPlot,
    PlotStyle, Resolution, ScalarSlicePlot, Scatter3D, StreamPlot3D, Surface3D, TableDataSet,
    TableVectorFieldPlot, VectorField3D, eval_curve_point, eval_surface, eval_with_vars,
    finite_curl, finite_divergence, finite_gradient, generate_seeds, parse_expr_with_vars,
    parse_surface_expr,
};

#[derive(Debug, Clone)]
pub struct GraphBuildError {
    pub plot_index: Option<usize>,
    pub plot_name: Option<String>,
    pub diagnostic: Diagnostic,
}

impl GraphBuildError {
    fn new(plot_index: Option<usize>, plot_name: Option<String>, diagnostic: Diagnostic) -> Self {
        Self {
            plot_index,
            plot_name,
            diagnostic,
        }
    }

    fn for_plot(plot_index: usize, plot: &PlotSpec, message: impl Into<String>) -> Self {
        Self::new(
            Some(plot_index),
            Some(plot.name.clone()),
            Diagnostic::error(DiagnosticKind::Build, message),
        )
    }

    fn parse(plot_index: usize, plot: &PlotSpec, diagnostic: impl Into<Diagnostic>) -> Self {
        Self::new(Some(plot_index), Some(plot.name.clone()), diagnostic.into())
    }

    fn validation(plot_index: usize, plot: &PlotSpec, diagnostic: impl Into<Diagnostic>) -> Self {
        Self::new(Some(plot_index), Some(plot.name.clone()), diagnostic.into())
    }
}

impl fmt::Display for GraphBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.plot_index, &self.plot_name) {
            (Some(index), Some(name)) => {
                write!(f, "plot {} ({}): {}", index + 1, name, self.diagnostic)
            }
            (Some(index), None) => write!(f, "plot {}: {}", index + 1, self.diagnostic),
            _ => write!(f, "{}", self.diagnostic),
        }
    }
}

impl std::error::Error for GraphBuildError {}

impl GraphSpec {
    pub fn build_scene(&self) -> Result<GraphScene, GraphBuildError> {
        let mut scene = GraphScene::new();
        scene.axis_config = self.axis_config.clone();
        for (plot_index, plot) in self.plots.iter().enumerate() {
            if !plot.visible {
                continue;
            }
            compile_plot_spec(plot_index, plot, &mut scene, (plot_index + 1) as u64)?;
        }
        Ok(scene)
    }
}

fn compile_plot_spec(
    plot_index: usize,
    plot: &PlotSpec,
    scene: &mut GraphScene,
    pick_id: u64,
) -> Result<(), GraphBuildError> {
    let surface_style = || {
        let mut style = plot.style.clone();
        style.two_sided = true;
        style
    };

    match &plot.definition {
        PlotDefinition::ContouredSurface {
            contour_values,
            contour_style,
        } => {
            let surface = Arc::new(
                Surface3D::from_fn(|x, y| x.sin() * y.cos())
                    .with_domain(plot.domain.clone())
                    .with_style(surface_style())
                    .with_resolution(plot.resolution),
            );
            scene.add_with_pick_id(
                pick_id,
                LevelSet3D::new(surface, contour_values.clone())
                    .with_contour_style(contour_style.clone()),
            );
        }
        PlotDefinition::SphericalHarmonic => {
            scene.add_with_pick_id(
                pick_id,
                Surface3D::spherical(|theta, phi| {
                    5.0 * (1.0 + 0.3 * (3.0 * theta).sin() * (2.0 * phi).cos())
                })
                .with_style(surface_style())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::HelixCurve => {
            use std::f64::consts::PI;
            scene.add_with_pick_id(
                pick_id,
                Curve3D::parametric(0.0..=20.0 * PI, |t| {
                    glam::DVec3::new(t.cos() * 3.0, t.sin() * 3.0, t * 0.15)
                })
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ScatterCloud => {
            let points: Vec<glam::Vec3> = (0..200)
                .map(|i| {
                    glam::Vec3::new(
                        (i as f32 * 0.37).sin() * 5.0,
                        (i as f32 * 0.73).cos() * 5.0,
                        (i as f32 * 0.11).sin() * 5.0,
                    )
                })
                .collect();
            scene.add_with_pick_id(
                pick_id,
                Scatter3D::from_points(&points).with_style(plot.style.clone()),
            );
        }
        PlotDefinition::VectorField => {
            let seeds = seed_resolution_from_plot(plot.resolution);
            scene.add_with_pick_id(
                pick_id,
                VectorField3D::from_fn(|x, y, _z| glam::Vec3::new(-y as f32, x as f32, 0.3), seeds)
                    .with_domain(plot.domain.clone())
                    .with_style(plot.style.clone())
                    .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::GridSurface => {
            let n_u = plot.resolution.u.max(2) as usize;
            let n_v = plot.resolution.v.max(2) as usize;
            let x0 = *plot.domain.x.start();
            let x1 = *plot.domain.x.end();
            let y0 = *plot.domain.y.start();
            let y1 = *plot.domain.y.end();
            let xs: Vec<f64> = (0..n_u)
                .map(|i| x0 + (x1 - x0) * i as f64 / (n_u - 1) as f64)
                .collect();
            let ys: Vec<f64> = (0..n_v)
                .map(|j| y0 + (y1 - y0) * j as f64 / (n_v - 1) as f64)
                .collect();
            let zs: Vec<f64> = (0..n_u * n_v)
                .map(|idx| {
                    let i = idx % n_u;
                    let j = idx / n_u;
                    (xs[i] * 0.5).sin() * (ys[j] * 0.5).cos() * 3.0
                })
                .collect();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::from_grid(&xs, &ys, &zs).with_style(surface_style()),
            );
        }
        PlotDefinition::Streamlines { seeds } => {
            let seeds: Vec<glam::Vec3> =
                seeds.iter().copied().map(glam::Vec3::from_array).collect();
            scene.add_with_pick_id(
                pick_id,
                StreamPlot3D::from_field(
                    |p: glam::Vec3| {
                        glam::Vec3::new(
                            p.z.sin() + p.y.cos(),
                            p.x.sin() + p.z.cos(),
                            p.y.sin() + p.x.cos(),
                        )
                    },
                    &seeds,
                    0.05,
                    500,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone()),
            );
        }
        PlotDefinition::VolumeRender { resolution } => {
            scene.add_with_pick_id(
                pick_id,
                DensityPlot3D::from_fn(|x, y, z| (-(x * x + y * y + z * z)).exp(), *resolution)
                    .with_domain(plot.domain.clone())
                    .with_style(plot.style.clone()),
            );
        }
        PlotDefinition::Isosurface {
            isovalues,
            resolution,
        } => {
            let iso_styles = vec![
                PlotStyle {
                    colour_mode: ColourMode::Solid([0.2, 0.6, 1.0, 1.0]),
                    opacity: 0.5,
                    two_sided: true,
                    ..PlotStyle::default()
                },
                PlotStyle {
                    colour_mode: ColourMode::Solid([0.2, 0.9, 0.4, 1.0]),
                    opacity: 0.5,
                    two_sided: true,
                    ..PlotStyle::default()
                },
                PlotStyle {
                    colour_mode: ColourMode::Solid([1.0, 0.4, 0.2, 1.0]),
                    opacity: 0.5,
                    two_sided: true,
                    ..PlotStyle::default()
                },
            ];
            scene.add_with_pick_id(
                pick_id,
                ContourPlot3D::from_fn(|x, y, z| x * x + y * y + z * z, isovalues, *resolution)
                    .with_domain(plot.domain.clone())
                    .with_per_iso_styles(iso_styles),
            );
        }
        PlotDefinition::ExprCartesian {
            expression,
            parameters,
        } => {
            let parsed = parse_surface_expr(expression).map_err(|err| {
                GraphBuildError::for_plot(plot_index, plot, format!("parse error: {err}"))
            })?;
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::from_fn(move |x, y| eval_surface(&parsed, x, y, &params))
                    .with_domain(plot.domain.clone())
                    .with_style(surface_style())
                    .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprCurve {
            expression,
            parameters,
            t_range,
        } => {
            let parsed_triple = ParametricCurveExpr::parse(expression)
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let params = parameters.clone();
            let (t0, t1) = *t_range;
            scene.add_with_pick_id(
                pick_id,
                Curve3D::parametric(t0..=t1, move |t| {
                    eval_curve_point(parsed_triple.components(), t, &params)
                })
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprCartesianLine {
            dep_var,
            ind_var,
            expression,
            parameters,
        } => {
            let parsed = parse_expr_with_vars(expression, &[ind_var.as_str()]).map_err(|err| {
                GraphBuildError::for_plot(plot_index, plot, format!("parse error: {err}"))
            })?;
            let params = parameters.clone();
            let dep = dep_var.clone();
            let ind = ind_var.clone();
            let (t0, t1) = (*plot.domain.x.start(), *plot.domain.x.end());
            scene.add_with_pick_id(
                pick_id,
                Curve3D::parametric(t0..=t1, move |t| {
                    let vars: Vec<(&str, f64)> = params
                        .iter()
                        .map(|(n, v)| (n.as_str(), *v))
                        .chain(std::iter::once((ind.as_str(), t)))
                        .collect();
                    let val = eval_with_vars(&parsed, &vars);
                    match (dep.as_str(), ind.as_str()) {
                        ("y", "x") => glam::DVec3::new(t, val, 0.0),
                        ("z", "x") => glam::DVec3::new(t, 0.0, val),
                        ("z", "y") => glam::DVec3::new(0.0, t, val),
                        ("x", "y") => glam::DVec3::new(val, t, 0.0),
                        ("x", "z") => glam::DVec3::new(val, 0.0, t),
                        ("y", "z") => glam::DVec3::new(0.0, val, t),
                        _ => glam::DVec3::new(t, val, 0.0),
                    }
                })
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprSpherical {
            expression,
            parameters,
        } => {
            let parsed = parse_expr_with_vars(expression, &["theta", "phi"]).map_err(|err| {
                GraphBuildError::for_plot(plot_index, plot, format!("parse error: {err}"))
            })?;
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::spherical(move |theta, phi| {
                    let mut vars: Vec<(&str, f64)> = vec![("theta", theta), ("phi", phi)];
                    for (name, val) in &params {
                        vars.push((name.as_str(), *val));
                    }
                    eval_with_vars(&parsed, &vars)
                })
                .with_domain(plot.domain.clone())
                .with_style(surface_style())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprCylindrical {
            expression,
            parameters,
        } => {
            let parsed = parse_expr_with_vars(expression, &["theta", "z"]).map_err(|err| {
                GraphBuildError::for_plot(plot_index, plot, format!("parse error: {err}"))
            })?;
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::cylindrical(move |theta, z| {
                    let mut vars: Vec<(&str, f64)> = vec![("theta", theta), ("z", z)];
                    for (name, val) in &params {
                        vars.push((name.as_str(), *val));
                    }
                    eval_with_vars(&parsed, &vars)
                })
                .with_domain(plot.domain.clone())
                .with_style(surface_style())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprPolar {
            expression,
            parameters,
        } => {
            let parsed = parse_expr_with_vars(expression, &["theta"]).map_err(|err| {
                GraphBuildError::for_plot(plot_index, plot, format!("parse error: {err}"))
            })?;
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::polar(move |theta| {
                    let mut vars: Vec<(&str, f64)> = vec![("theta", theta)];
                    for (name, val) in &params {
                        vars.push((name.as_str(), *val));
                    }
                    eval_with_vars(&parsed, &vars)
                })
                .with_domain(plot.domain.clone())
                .with_style(surface_style())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprParametricSurface {
            expression,
            parameters,
        } => {
            let parsed = ParametricSurfaceExpr::parse(expression)
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            let u_range = plot.domain.x.clone();
            let v_range = plot.domain.y.clone();
            scene.add_with_pick_id(
                pick_id,
                Surface3D::parametric(u_range, v_range, move |u, v| {
                    let mut vars: Vec<(&str, f64)> = vec![("u", u), ("v", v)];
                    for (name, val) in &params {
                        vars.push((name.as_str(), *val));
                    }
                    glam::DVec3::new(
                        eval_with_vars(&px, &vars),
                        eval_with_vars(&py, &vars),
                        eval_with_vars(&pz, &vars),
                    )
                })
                .with_style(surface_style())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ScalarSlice {
            expression,
            parameters,
            axis,
            position,
            contour_values,
            contour_style,
        } => {
            let parsed = ScalarFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                ScalarSlicePlot {
                    axis: *axis,
                    position: *position,
                    value_fn: Box::new(move |x, y, z| {
                        let mut vars: Vec<(&str, f64)> = vec![("x", x), ("y", y), ("z", z)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        parsed.eval(&vars)
                    }),
                    contour_values: contour_values.clone(),
                    contour_style: contour_style.clone(),
                    style: surface_style(),
                },
            );
        }
        PlotDefinition::VectorSlice {
            expression,
            parameters,
            axis,
            position,
        } => {
            let parsed = VectorFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            scene.add_with_pick_id(
                pick_id,
                PlaneVectorFieldPlot {
                    axis: *axis,
                    position: *position,
                    vector_fn: Box::new(move |x, y, z| {
                        let mut vars: Vec<(&str, f64)> = vec![("x", x), ("y", y), ("z", z)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        glam::vec3(
                            eval_with_vars(&px, &vars) as f32,
                            eval_with_vars(&py, &vars) as f32,
                            eval_with_vars(&pz, &vars) as f32,
                        )
                    }),
                    style: plot.style.clone(),
                },
            );
        }
        PlotDefinition::GradientField {
            expression,
            parameters,
        } => {
            let parsed = ScalarFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let params = parameters.clone();
            let diff = FiniteDifferenceConfig::default();
            let seeds = seed_resolution_from_plot(plot.resolution);
            scene.add_with_pick_id(
                pick_id,
                VectorField3D::from_fn(
                    move |x, y, z| {
                        let h = diff.step_for_parameters(&params);
                        finite_gradient(
                            |sx, sy, sz| {
                                let mut vars = vec![("x", sx), ("y", sy), ("z", sz)];
                                for (name, val) in &params {
                                    vars.push((name.as_str(), *val));
                                }
                                parsed.eval(&vars)
                            },
                            x,
                            y,
                            z,
                            h,
                        )
                    },
                    seeds,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::DivergenceField {
            expression,
            parameters,
            vol_resolution,
        } => {
            let parsed = VectorFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            let diff = FiniteDifferenceConfig::default();
            let res = *vol_resolution;
            scene.add_with_pick_id(
                pick_id,
                DensityPlot3D::from_fn(
                    move |x, y, z| {
                        let h = diff.step_for_parameters(&params);
                        finite_divergence(
                            |sx, sy, sz| {
                                let mut vars = vec![("x", sx), ("y", sy), ("z", sz)];
                                for (name, val) in &params {
                                    vars.push((name.as_str(), *val));
                                }
                                glam::vec3(
                                    eval_with_vars(&px, &vars) as f32,
                                    eval_with_vars(&py, &vars) as f32,
                                    eval_with_vars(&pz, &vars) as f32,
                                )
                            },
                            x,
                            y,
                            z,
                            h,
                        ) as f64
                    },
                    res,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone()),
            );
        }
        PlotDefinition::CurlField {
            expression,
            parameters,
        } => {
            let parsed = VectorFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            let diff = FiniteDifferenceConfig::default();
            let seeds = seed_resolution_from_plot(plot.resolution);
            scene.add_with_pick_id(
                pick_id,
                VectorField3D::from_fn(
                    move |x, y, z| {
                        let h = diff.step_for_parameters(&params);
                        finite_curl(
                            |sx, sy, sz| {
                                let mut vars = vec![("x", sx), ("y", sy), ("z", sz)];
                                for (name, val) in &params {
                                    vars.push((name.as_str(), *val));
                                }
                                glam::vec3(
                                    eval_with_vars(&px, &vars) as f32,
                                    eval_with_vars(&py, &vars) as f32,
                                    eval_with_vars(&pz, &vars) as f32,
                                )
                            },
                            x,
                            y,
                            z,
                            h,
                        )
                    },
                    seeds,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::PointAnnotations {
            points,
            show_labels,
        } => {
            if !points.is_empty() {
                scene.add_with_pick_id(
                    pick_id,
                    AnnotatedPointsPlot {
                        points: points.clone(),
                        show_labels: *show_labels,
                        style: plot.style.clone(),
                    },
                );
            }
        }
        PlotDefinition::ArrowAnnotations {
            arrows,
            show_labels,
        } => {
            if !arrows.is_empty() {
                scene.add_with_pick_id(
                    pick_id,
                    AnnotatedArrowsPlot {
                        arrows: arrows.clone(),
                        show_labels: *show_labels,
                        style: plot.style.clone(),
                    },
                );
            }
        }
        PlotDefinition::DerivedPolylineGroups { groups } => {
            if !groups.is_empty() {
                let converted: Vec<Vec<glam::Vec3>> = groups
                    .iter()
                    .map(|group| {
                        group
                            .iter()
                            .map(|point| glam::Vec3::from_array(*point))
                            .collect()
                    })
                    .collect();
                scene.add_with_pick_id(
                    pick_id,
                    build_curve_piecewise(&converted, plot.style.clone()),
                );
            }
        }
        PlotDefinition::InterpolatedCurve {
            points,
            interpolation,
        } => {
            if !points.is_empty() {
                let converted = vec![
                    points
                        .iter()
                        .map(|point| glam::Vec3::from_array(*point))
                        .collect::<Vec<_>>(),
                ];
                scene.add_with_pick_id(
                    pick_id,
                    build_curve_piecewise_with_interpolation(
                        &converted,
                        plot.style.clone(),
                        *interpolation,
                    ),
                );
            }
        }
        PlotDefinition::ImportedTable { definition } => {
            let dataset = definition
                .validate()
                .map_err(|errs| match errs.into_iter().next() {
                    Some(err) => {
                        let mut diagnostic = crate::ValidationDiagnostic::new(err.message);
                        if let Some(row) = err.row {
                            diagnostic = diagnostic.with_row(row);
                        }
                        if let Some(column) = err.column {
                            diagnostic = diagnostic.with_column(column);
                        }
                        GraphBuildError::validation(plot_index, plot, diagnostic)
                    }
                    None => GraphBuildError::validation(
                        plot_index,
                        plot,
                        crate::ValidationDiagnostic::new("invalid table import"),
                    ),
                })?;
            match dataset {
                TableDataSet::SurfaceGrid { xs, ys, zs } => {
                    scene.add_with_pick_id(
                        pick_id,
                        Surface3D::from_grid(&xs, &ys, &zs).with_style(surface_style()),
                    );
                }
                TableDataSet::Curve { groups, .. } => {
                    if !groups.is_empty() {
                        scene.add_with_pick_id(
                            pick_id,
                            build_curve_piecewise(&groups, plot.style.clone()),
                        );
                    }
                }
                TableDataSet::Scatter {
                    points, scalars, ..
                } => {
                    if !points.is_empty() {
                        if let Some(scalars) = scalars {
                            scene.add_with_pick_id(
                                pick_id,
                                Scatter3D::from_points_with_scalars(&points, &scalars)
                                    .with_style(plot.style.clone()),
                            );
                        } else {
                            scene.add_with_pick_id(
                                pick_id,
                                Scatter3D::from_points(&points).with_style(plot.style.clone()),
                            );
                        }
                    }
                }
                TableDataSet::VectorField { samples, bounds } => {
                    if !samples.is_empty() {
                        scene.add_with_pick_id(
                            pick_id,
                            TableVectorFieldPlot::new(samples, bounds, plot.style.clone()),
                        );
                    }
                }
            }
        }
        PlotDefinition::ExprVectorField {
            expression,
            parameters,
        } => {
            let parsed = VectorFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            let seeds = seed_resolution_from_plot(plot.resolution);
            scene.add_with_pick_id(
                pick_id,
                VectorField3D::from_fn(
                    move |x, y, z| {
                        let mut vars: Vec<(&str, f64)> = vec![("x", x), ("y", y), ("z", z)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        glam::Vec3::new(
                            eval_with_vars(&px, &vars) as f32,
                            eval_with_vars(&py, &vars) as f32,
                            eval_with_vars(&pz, &vars) as f32,
                        )
                    },
                    seeds,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone())
                .with_resolution(plot.resolution),
            );
        }
        PlotDefinition::ExprVolume {
            expression,
            parameters,
            vol_resolution,
        } => {
            let parsed = ScalarFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let params = parameters.clone();
            let res = *vol_resolution;
            scene.add_with_pick_id(
                pick_id,
                DensityPlot3D::from_fn(
                    move |x, y, z| {
                        let mut vars: Vec<(&str, f64)> = vec![("x", x), ("y", y), ("z", z)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        parsed.eval(&vars)
                    },
                    res,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone()),
            );
        }
        PlotDefinition::ExprIsosurface {
            expression,
            parameters,
            isovalues,
            iso_colours,
            vol_resolution,
        } => {
            let parsed = ScalarFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let params = parameters.clone();
            let res = *vol_resolution;
            let iso_styles: Vec<PlotStyle> = iso_colours
                .iter()
                .map(|c| PlotStyle {
                    colour_mode: ColourMode::Solid(*c),
                    opacity: c[3],
                    two_sided: true,
                    ..PlotStyle::default()
                })
                .collect();
            scene.add_with_pick_id(
                pick_id,
                ContourPlot3D::from_fn(
                    move |x, y, z| {
                        let mut vars: Vec<(&str, f64)> = vec![("x", x), ("y", y), ("z", z)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        parsed.eval(&vars)
                    },
                    isovalues,
                    res,
                )
                .with_domain(plot.domain.clone())
                .with_per_iso_styles(iso_styles),
            );
        }
        PlotDefinition::ExprStreamlines {
            expression,
            parameters,
            seed_mode,
            step_size,
            max_steps,
        } => {
            let parsed = VectorFieldExpr::parse(expression, &["x", "y", "z"])
                .map_err(|err| GraphBuildError::parse(plot_index, plot, err))?;
            let [px, py, pz] = parsed.components().clone();
            let params = parameters.clone();
            let seeds = generate_seeds(seed_mode, &plot.domain);
            let ss = *step_size;
            let ms = *max_steps;
            scene.add_with_pick_id(
                pick_id,
                StreamPlot3D::from_field(
                    move |p: glam::Vec3| {
                        let mut vars: Vec<(&str, f64)> =
                            vec![("x", p.x as f64), ("y", p.y as f64), ("z", p.z as f64)];
                        for (name, val) in &params {
                            vars.push((name.as_str(), *val));
                        }
                        glam::Vec3::new(
                            eval_with_vars(&px, &vars) as f32,
                            eval_with_vars(&py, &vars) as f32,
                            eval_with_vars(&pz, &vars) as f32,
                        )
                    },
                    &seeds,
                    ss,
                    ms,
                )
                .with_domain(plot.domain.clone())
                .with_style(plot.style.clone()),
            );
        }
    }

    Ok(())
}

fn seed_resolution_from_plot(resolution: Resolution) -> [u32; 3] {
    [
        resolution.u.clamp(2, 12),
        resolution.v.clamp(2, 12),
        ((resolution.u + resolution.v) / 4).clamp(2, 8),
    ]
}

fn build_curve_piecewise(groups: &[Vec<glam::Vec3>], style: PlotStyle) -> PiecewisePlot {
    build_curve_piecewise_with_interpolation(groups, style, crate::CurveInterpolation::default())
}

fn build_curve_piecewise_with_interpolation(
    groups: &[Vec<glam::Vec3>],
    style: PlotStyle,
    interpolation: crate::CurveInterpolation,
) -> PiecewisePlot {
    let mut plot = PiecewisePlot::new();
    for points in groups {
        if points.is_empty() {
            continue;
        }
        let bounds = bounds_domain_for_points(points);
        plot.add_piece(
            bounds,
            Curve3D::from_points_interpolated(points, interpolation).with_style(style.clone()),
        );
    }
    plot
}

fn bounds_domain_for_points(points: &[glam::Vec3]) -> Domain {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for &point in points {
        min = min.min(point);
        max = max.max(point);
    }
    Domain {
        x: min.x as f64..=max.x as f64,
        y: min.y as f64..=max.y as f64,
        z: min.z as f64..=max.z as f64,
    }
}
