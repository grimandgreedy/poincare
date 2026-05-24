use std::sync::Arc;

use poincare_lib::{
    ColourMode, ContourPlot3D, Curve3D, DensityPlot3D, Domain, GraphScene, LevelSet3D, PlotStyle,
    Resolution, Scatter3D, StreamPlot3D, Surface3D, VectorField3D, eval_curve_point, eval_surface,
    eval_with_vars,
    graph_spec::{
        ArrowAnnotation as LibArrowAnnotation, GraphSpec, OptionalColumn as LibOptionalColumn,
        PlotDefinition, PlotSpec, PointAnnotation as LibPointAnnotation,
        SeedMode as LibSeedMode, SliceAxis as LibSliceAxis,
        TableColumnMapping as LibTableColumnMapping, TableDelimiter as LibTableDelimiter,
        TableImportDefinition as LibTableImportDefinition, TablePlotTarget as LibTablePlotTarget,
    },
    parse_curve_expr, parse_expr_with_vars, parse_surface_expr,
};

use crate::plot::analysis::{
    AnnotatedArrowsPlot, AnnotatedPointsPlot, PlaneVectorFieldPlot, ScalarSlicePlot,
};
use crate::plot::kind::PlotKind;
use crate::plot::table::{
    TableDataSet, TableVectorFieldPlot, build_curve_piecewise,
    build_curve_piecewise_with_interpolation,
};

#[derive(Clone)]
pub(crate) struct PlotEntry {
    pub(crate) name: String,
    pub(crate) visible: bool,
    pub(crate) domain: Domain,
    pub(crate) resolution: Resolution,
    pub(crate) style: PlotStyle,
    pub(crate) kind: PlotKind,
}

impl PlotEntry {
    #[allow(dead_code)]
    fn surface_style(&self) -> PlotStyle {
        let mut style = self.style.clone();
        style.two_sided = true;
        style
    }

    #[allow(dead_code)]
    pub(crate) fn add_to_scene_with_pick_id(&self, scene: &mut GraphScene, pick_id: u64) {
        match &self.kind {
            PlotKind::ContouredSurface {
                contour_values,
                contour_style,
            } => {
                let surface = Arc::new(
                    Surface3D::from_fn(|x, y| x.sin() * y.cos())
                        .with_domain(self.domain.clone())
                        .with_style(self.surface_style())
                        .with_resolution(self.resolution),
                );
                scene.add_with_pick_id(
                    pick_id,
                    LevelSet3D::new(surface, contour_values.clone())
                        .with_contour_style(contour_style.clone()),
                );
            }
            PlotKind::SphericalHarmonic => {
                scene.add_with_pick_id(
                    pick_id,
                    Surface3D::spherical(|theta, phi| {
                        5.0 * (1.0 + 0.3 * (3.0 * theta).sin() * (2.0 * phi).cos())
                    })
                    .with_style(self.surface_style())
                    .with_resolution(self.resolution),
                );
            }
            PlotKind::HelixCurve => {
                use std::f64::consts::PI;
                scene.add_with_pick_id(
                    pick_id,
                    Curve3D::parametric(0.0..=20.0 * PI, |t| {
                        glam::DVec3::new(t.cos() * 3.0, t.sin() * 3.0, t * 0.15)
                    })
                    .with_style(self.style.clone())
                    .with_resolution(self.resolution),
                );
            }
            PlotKind::ScatterCloud => {
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
                    Scatter3D::from_points(&points).with_style(self.style.clone()),
                );
            }
            PlotKind::VectorField => {
                let seeds = [
                    self.resolution.u.clamp(2, 12),
                    self.resolution.v.clamp(2, 12),
                    ((self.resolution.u + self.resolution.v) / 4).clamp(2, 8),
                ];
                scene.add_with_pick_id(
                    pick_id,
                    VectorField3D::from_fn(
                        |x, y, _z| glam::Vec3::new(-y as f32, x as f32, 0.3),
                        seeds,
                    )
                    .with_domain(self.domain.clone())
                    .with_style(self.style.clone())
                    .with_resolution(self.resolution),
                );
            }
            PlotKind::GridSurface => {
                let n_u = self.resolution.u.max(2) as usize;
                let n_v = self.resolution.v.max(2) as usize;
                let x0 = *self.domain.x.start();
                let x1 = *self.domain.x.end();
                let y0 = *self.domain.y.start();
                let y1 = *self.domain.y.end();
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
                    Surface3D::from_grid(&xs, &ys, &zs).with_style(self.surface_style()),
                );
            }
            PlotKind::Streamlines { seeds } => {
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
                        seeds,
                        0.05,
                        500,
                    )
                    .with_domain(self.domain.clone())
                    .with_style(self.style.clone()),
                );
            }
            PlotKind::VolumeRender { resolution } => {
                scene.add_with_pick_id(
                    pick_id,
                    DensityPlot3D::from_fn(|x, y, z| (-(x * x + y * y + z * z)).exp(), *resolution)
                        .with_domain(self.domain.clone())
                        .with_style(self.style.clone()),
                );
            }
            PlotKind::Isosurface {
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
                        .with_domain(self.domain.clone())
                        .with_per_iso_styles(iso_styles),
                );
            }
            PlotKind::ExprCartesian {
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_surface_expr(expression) {
                    let params = parameters.clone();
                    scene.add_with_pick_id(
                        pick_id,
                        Surface3D::from_fn(move |x, y| eval_surface(&parsed, x, y, &params))
                            .with_domain(self.domain.clone())
                            .with_style(self.surface_style())
                            .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprCurve {
                expression,
                parameters,
                t_range,
            } => {
                if let Ok(parsed_triple) = parse_curve_expr(expression) {
                    let params = parameters.clone();
                    let (t0, t1) = *t_range;
                    scene.add_with_pick_id(
                        pick_id,
                        Curve3D::parametric(t0..=t1, move |t| {
                            eval_curve_point(&parsed_triple, t, &params)
                        })
                        .with_style(self.style.clone())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprCartesianLine {
                dep_var,
                ind_var,
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &[ind_var.as_str()]) {
                    let params = parameters.clone();
                    let dep = dep_var.clone();
                    let ind = ind_var.clone();
                    let (t0, t1) = (*self.domain.x.start(), *self.domain.x.end());
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
                        .with_style(self.style.clone())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprSpherical {
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["theta", "phi"]) {
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
                        .with_domain(self.domain.clone())
                        .with_style(self.surface_style())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprCylindrical {
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["theta", "z"]) {
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
                        .with_domain(self.domain.clone())
                        .with_style(self.surface_style())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprPolar {
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["theta"]) {
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
                        .with_domain(self.domain.clone())
                        .with_style(self.surface_style())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::ExprParametricSurface {
                expression,
                parameters,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["u", "v"]),
                        parse_expr_with_vars(parts[1], &["u", "v"]),
                        parse_expr_with_vars(parts[2], &["u", "v"]),
                    ) {
                        let params = parameters.clone();
                        let u_range = self.domain.x.clone();
                        let v_range = self.domain.y.clone();
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
                            .with_style(self.surface_style())
                            .with_resolution(self.resolution),
                        );
                    }
                }
            }
            PlotKind::ScalarSlice {
                expression,
                parameters,
                axis,
                position,
                contour_values,
                contour_style,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["x", "y", "z"]) {
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
                                eval_with_vars(&parsed, &vars)
                            }),
                            contour_values: contour_values.clone(),
                            contour_style: contour_style.clone(),
                            style: self.surface_style(),
                        },
                    );
                }
            }
            PlotKind::VectorSlice {
                expression,
                parameters,
                axis,
                position,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[1], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[2], &["x", "y", "z"]),
                    ) {
                        let params = parameters.clone();
                        scene.add_with_pick_id(
                            pick_id,
                            PlaneVectorFieldPlot {
                                axis: *axis,
                                position: *position,
                                vector_fn: Box::new(move |x, y, z| {
                                    let mut vars: Vec<(&str, f64)> =
                                        vec![("x", x), ("y", y), ("z", z)];
                                    for (name, val) in &params {
                                        vars.push((name.as_str(), *val));
                                    }
                                    glam::vec3(
                                        eval_with_vars(&px, &vars) as f32,
                                        eval_with_vars(&py, &vars) as f32,
                                        eval_with_vars(&pz, &vars) as f32,
                                    )
                                }),
                                style: self.style.clone(),
                            },
                        );
                    }
                }
            }
            PlotKind::GradientField {
                expression,
                parameters,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["x", "y", "z"]) {
                    let params = parameters.clone();
                    let seeds = [
                        self.resolution.u.clamp(2, 12),
                        self.resolution.v.clamp(2, 12),
                        ((self.resolution.u + self.resolution.v) / 4).clamp(2, 8),
                    ];
                    scene.add_with_pick_id(
                        pick_id,
                        VectorField3D::from_fn(
                            move |x, y, z| {
                                let h = gradient_step(&params);
                                finite_gradient(
                                    |sx, sy, sz| {
                                        let mut vars =
                                            vec![("x", sx), ("y", sy), ("z", sz)];
                                        for (name, val) in &params {
                                            vars.push((name.as_str(), *val));
                                        }
                                        eval_with_vars(&parsed, &vars)
                                    },
                                    x,
                                    y,
                                    z,
                                    h,
                                )
                            },
                            seeds,
                        )
                        .with_domain(self.domain.clone())
                        .with_style(self.style.clone())
                        .with_resolution(self.resolution),
                    );
                }
            }
            PlotKind::DivergenceField {
                expression,
                parameters,
                vol_resolution,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[1], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[2], &["x", "y", "z"]),
                    ) {
                        let params = parameters.clone();
                        let res = *vol_resolution;
                        scene.add_with_pick_id(
                            pick_id,
                            DensityPlot3D::from_fn(
                                move |x, y, z| {
                                    let h = gradient_step(&params);
                                    finite_divergence(
                                        |sx, sy, sz| {
                                            let mut vars =
                                                vec![("x", sx), ("y", sy), ("z", sz)];
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
                            .with_domain(self.domain.clone())
                            .with_style(self.style.clone()),
                        );
                    }
                }
            }
            PlotKind::CurlField {
                expression,
                parameters,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[1], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[2], &["x", "y", "z"]),
                    ) {
                        let params = parameters.clone();
                        let seeds = [
                            self.resolution.u.clamp(2, 12),
                            self.resolution.v.clamp(2, 12),
                            ((self.resolution.u + self.resolution.v) / 4).clamp(2, 8),
                        ];
                        scene.add_with_pick_id(
                            pick_id,
                            VectorField3D::from_fn(
                                move |x, y, z| {
                                    let h = gradient_step(&params);
                                    finite_curl(
                                        |sx, sy, sz| {
                                            let mut vars =
                                                vec![("x", sx), ("y", sy), ("z", sz)];
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
                            .with_domain(self.domain.clone())
                            .with_style(self.style.clone())
                            .with_resolution(self.resolution),
                        );
                    }
                }
            }
            PlotKind::PointAnnotations { points, show_labels } => {
                if !points.is_empty() {
                    scene.add_with_pick_id(
                        pick_id,
                        AnnotatedPointsPlot {
                            points: points.clone(),
                            show_labels: *show_labels,
                            style: self.style.clone(),
                        },
                    );
                }
            }
            PlotKind::ArrowAnnotations { arrows, show_labels } => {
                if !arrows.is_empty() {
                    scene.add_with_pick_id(
                        pick_id,
                        AnnotatedArrowsPlot {
                            arrows: arrows.clone(),
                            show_labels: *show_labels,
                            style: self.style.clone(),
                        },
                    );
                }
            }
            PlotKind::DerivedPolylineGroups { groups } => {
                if !groups.is_empty() {
                    let converted: Vec<Vec<glam::Vec3>> = groups
                        .iter()
                        .map(|group| group.iter().map(|point| glam::Vec3::from_array(*point)).collect())
                        .collect();
                    scene.add_with_pick_id(
                        pick_id,
                        build_curve_piecewise(&converted, self.style.clone()),
                    );
                }
            }
            PlotKind::InterpolatedCurve {
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
                            self.style.clone(),
                            *interpolation,
                        ),
                    );
                }
            }
            PlotKind::ImportedTable { definition } => {
                if let Ok(dataset) = definition.validate() {
                    match dataset {
                        TableDataSet::SurfaceGrid { xs, ys, zs } => {
                            scene.add_with_pick_id(
                                pick_id,
                                Surface3D::from_grid(&xs, &ys, &zs)
                                    .with_style(self.surface_style()),
                            );
                        }
                        TableDataSet::Curve { groups, .. } => {
                            if !groups.is_empty() {
                                scene.add_with_pick_id(
                                    pick_id,
                                    build_curve_piecewise(&groups, self.style.clone()),
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
                                            .with_style(self.style.clone()),
                                    );
                                } else {
                                    scene.add_with_pick_id(
                                        pick_id,
                                        Scatter3D::from_points(&points)
                                            .with_style(self.style.clone()),
                                    );
                                }
                            }
                        }
                        TableDataSet::VectorField { samples, bounds } => {
                            if !samples.is_empty() {
                                scene.add_with_pick_id(
                                    pick_id,
                                    TableVectorFieldPlot::new(samples, bounds, self.style.clone()),
                                );
                            }
                        }
                    }
                }
            }
            PlotKind::ExprVectorField {
                expression,
                parameters,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[1], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[2], &["x", "y", "z"]),
                    ) {
                        let params = parameters.clone();
                        let seeds = [
                            self.resolution.u.clamp(2, 12),
                            self.resolution.v.clamp(2, 12),
                            ((self.resolution.u + self.resolution.v) / 4).clamp(2, 8),
                        ];
                        scene.add_with_pick_id(
                            pick_id,
                            VectorField3D::from_fn(
                                move |x, y, z| {
                                    let mut vars: Vec<(&str, f64)> =
                                        vec![("x", x as f64), ("y", y as f64), ("z", z as f64)];
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
                            .with_domain(self.domain.clone())
                            .with_style(self.style.clone())
                            .with_resolution(self.resolution),
                        );
                    }
                }
            }
            PlotKind::ExprVolume {
                expression,
                parameters,
                vol_resolution,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["x", "y", "z"]) {
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
                                eval_with_vars(&parsed, &vars)
                            },
                            res,
                        )
                        .with_domain(self.domain.clone())
                        .with_style(self.style.clone()),
                    );
                }
            }
            PlotKind::ExprIsosurface {
                expression,
                parameters,
                isovalues,
                iso_colours,
                vol_resolution,
            } => {
                if let Ok(parsed) = parse_expr_with_vars(expression, &["x", "y", "z"]) {
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
                                eval_with_vars(&parsed, &vars)
                            },
                            isovalues,
                            res,
                        )
                        .with_domain(self.domain.clone())
                        .with_per_iso_styles(iso_styles),
                    );
                }
            }
            PlotKind::ExprStreamlines {
                expression,
                parameters,
                seed_mode,
                step_size,
                max_steps,
            } => {
                let parts: Vec<&str> = expression.splitn(3, '|').collect();
                if parts.len() == 3 {
                    if let (Ok(px), Ok(py), Ok(pz)) = (
                        parse_expr_with_vars(parts[0], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[1], &["x", "y", "z"]),
                        parse_expr_with_vars(parts[2], &["x", "y", "z"]),
                    ) {
                        let params = parameters.clone();
                        let seeds = crate::plot::builder::generate_seeds(seed_mode, &self.domain);
                        let ss = *step_size;
                        let ms = *max_steps;
                        scene.add_with_pick_id(
                            pick_id,
                            StreamPlot3D::from_field(
                                move |p: glam::Vec3| {
                                    let mut vars: Vec<(&str, f64)> = vec![
                                        ("x", p.x as f64),
                                        ("y", p.y as f64),
                                        ("z", p.z as f64),
                                    ];
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
                            .with_domain(self.domain.clone())
                            .with_style(self.style.clone()),
                        );
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn to_plot_spec(&self) -> PlotSpec {
        PlotSpec {
            name: self.name.clone(),
            visible: self.visible,
            domain: self.domain.clone(),
            resolution: self.resolution,
            style: self.style.clone(),
            definition: self.kind.to_plot_definition(),
        }
    }
}

impl PlotKind {
    #[allow(dead_code)]
    pub(crate) fn to_plot_definition(&self) -> PlotDefinition {
        match self {
            PlotKind::ContouredSurface {
                contour_values,
                contour_style,
            } => PlotDefinition::ContouredSurface {
                contour_values: contour_values.clone(),
                contour_style: contour_style.clone(),
            },
            PlotKind::SphericalHarmonic => PlotDefinition::SphericalHarmonic,
            PlotKind::HelixCurve => PlotDefinition::HelixCurve,
            PlotKind::ScatterCloud => PlotDefinition::ScatterCloud,
            PlotKind::VectorField => PlotDefinition::VectorField,
            PlotKind::GridSurface => PlotDefinition::GridSurface,
            PlotKind::Streamlines { seeds } => PlotDefinition::Streamlines {
                seeds: seeds.iter().map(|seed| seed.to_array()).collect(),
            },
            PlotKind::VolumeRender { resolution } => PlotDefinition::VolumeRender {
                resolution: *resolution,
            },
            PlotKind::Isosurface {
                isovalues,
                resolution,
            } => PlotDefinition::Isosurface {
                isovalues: isovalues.clone(),
                resolution: *resolution,
            },
            PlotKind::ExprCartesian {
                expression,
                parameters,
            } => PlotDefinition::ExprCartesian {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprCurve {
                expression,
                parameters,
                t_range,
            } => PlotDefinition::ExprCurve {
                expression: expression.clone(),
                parameters: parameters.clone(),
                t_range: *t_range,
            },
            PlotKind::ExprCartesianLine {
                dep_var,
                ind_var,
                expression,
                parameters,
            } => PlotDefinition::ExprCartesianLine {
                dep_var: dep_var.clone(),
                ind_var: ind_var.clone(),
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprSpherical {
                expression,
                parameters,
            } => PlotDefinition::ExprSpherical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprCylindrical {
                expression,
                parameters,
            } => PlotDefinition::ExprCylindrical {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprPolar {
                expression,
                parameters,
            } => PlotDefinition::ExprPolar {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprParametricSurface {
                expression,
                parameters,
            } => PlotDefinition::ExprParametricSurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ImportedTable { definition } => PlotDefinition::ImportedTable {
                definition: convert_table_import_definition(definition),
            },
            PlotKind::ScalarSlice {
                expression,
                parameters,
                axis,
                position,
                contour_values,
                contour_style,
            } => PlotDefinition::ScalarSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: convert_slice_axis(*axis),
                position: *position,
                contour_values: contour_values.clone(),
                contour_style: contour_style.clone(),
            },
            PlotKind::VectorSlice {
                expression,
                parameters,
                axis,
                position,
            } => PlotDefinition::VectorSlice {
                expression: expression.clone(),
                parameters: parameters.clone(),
                axis: convert_slice_axis(*axis),
                position: *position,
            },
            PlotKind::GradientField {
                expression,
                parameters,
            } => PlotDefinition::GradientField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::DivergenceField {
                expression,
                parameters,
                vol_resolution,
            } => PlotDefinition::DivergenceField {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::CurlField {
                expression,
                parameters,
            } => PlotDefinition::CurlField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::PointAnnotations { points, show_labels } => PlotDefinition::PointAnnotations {
                points: points
                    .iter()
                    .map(|point| LibPointAnnotation {
                        position: point.position,
                        label: point.label.clone(),
                    })
                    .collect(),
                show_labels: *show_labels,
            },
            PlotKind::ArrowAnnotations { arrows, show_labels } => PlotDefinition::ArrowAnnotations {
                arrows: arrows
                    .iter()
                    .map(|arrow| LibArrowAnnotation {
                        origin: arrow.origin,
                        vector: arrow.vector,
                        label: arrow.label.clone(),
                    })
                    .collect(),
                show_labels: *show_labels,
            },
            PlotKind::DerivedPolylineGroups { groups } => PlotDefinition::DerivedPolylineGroups {
                groups: groups.clone(),
            },
            PlotKind::InterpolatedCurve {
                points,
                interpolation,
            } => PlotDefinition::InterpolatedCurve {
                points: points.clone(),
                interpolation: *interpolation,
            },
            PlotKind::ExprVectorField {
                expression,
                parameters,
            } => PlotDefinition::ExprVectorField {
                expression: expression.clone(),
                parameters: parameters.clone(),
            },
            PlotKind::ExprVolume {
                expression,
                parameters,
                vol_resolution,
            } => PlotDefinition::ExprVolume {
                expression: expression.clone(),
                parameters: parameters.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::ExprIsosurface {
                expression,
                parameters,
                isovalues,
                iso_colours,
                vol_resolution,
            } => PlotDefinition::ExprIsosurface {
                expression: expression.clone(),
                parameters: parameters.clone(),
                isovalues: isovalues.clone(),
                iso_colours: iso_colours.clone(),
                vol_resolution: *vol_resolution,
            },
            PlotKind::ExprStreamlines {
                expression,
                parameters,
                seed_mode,
                step_size,
                max_steps,
            } => PlotDefinition::ExprStreamlines {
                expression: expression.clone(),
                parameters: parameters.clone(),
                seed_mode: convert_seed_mode(seed_mode),
                step_size: *step_size,
                max_steps: *max_steps,
            },
        }
    }
}

#[allow(dead_code)]
pub(crate) fn build_graph_spec(entries: &[PlotEntry], axis_config: poincare_lib::AxisConfig) -> GraphSpec {
    GraphSpec {
        axis_config,
        plots: entries.iter().map(PlotEntry::to_plot_spec).collect(),
    }
}

#[allow(dead_code)]
fn convert_slice_axis(axis: crate::plot::analysis::SliceAxis) -> LibSliceAxis {
    match axis {
        crate::plot::analysis::SliceAxis::X => LibSliceAxis::X,
        crate::plot::analysis::SliceAxis::Y => LibSliceAxis::Y,
        crate::plot::analysis::SliceAxis::Z => LibSliceAxis::Z,
    }
}

#[allow(dead_code)]
fn convert_seed_mode(seed_mode: &crate::plot::kind::SeedMode) -> LibSeedMode {
    match seed_mode {
        crate::plot::kind::SeedMode::Grid { nx, ny, nz } => LibSeedMode::Grid {
            nx: *nx,
            ny: *ny,
            nz: *nz,
        },
        crate::plot::kind::SeedMode::Plane { axis, offset } => LibSeedMode::Plane {
            axis: *axis,
            offset: *offset,
        },
        crate::plot::kind::SeedMode::ManualCsv { csv_text } => LibSeedMode::ManualCsv {
            csv_text: csv_text.clone(),
        },
    }
}

#[allow(dead_code)]
fn convert_table_import_definition(
    definition: &crate::plot::table::TableImportDefinition,
) -> LibTableImportDefinition {
    LibTableImportDefinition {
        source_path: definition.source_path.clone(),
        raw_text: definition.raw_text.clone(),
        delimiter: match definition.delimiter {
            crate::plot::table::TableDelimiter::Comma => LibTableDelimiter::Comma,
            crate::plot::table::TableDelimiter::Semicolon => LibTableDelimiter::Semicolon,
            crate::plot::table::TableDelimiter::Tab => LibTableDelimiter::Tab,
            crate::plot::table::TableDelimiter::Space => LibTableDelimiter::Space,
            crate::plot::table::TableDelimiter::Pipe => LibTableDelimiter::Pipe,
        },
        header_row: definition.header_row,
        target: match definition.target {
            crate::plot::table::TablePlotTarget::SurfaceGrid => LibTablePlotTarget::SurfaceGrid,
            crate::plot::table::TablePlotTarget::Curve => LibTablePlotTarget::Curve,
            crate::plot::table::TablePlotTarget::Scatter => LibTablePlotTarget::Scatter,
            crate::plot::table::TablePlotTarget::VectorField => LibTablePlotTarget::VectorField,
        },
        mapping: convert_table_column_mapping(&definition.mapping),
    }
}

#[allow(dead_code)]
fn convert_table_column_mapping(
    mapping: &crate::plot::table::TableColumnMapping,
) -> LibTableColumnMapping {
    match mapping {
        crate::plot::table::TableColumnMapping::SurfaceGrid { x, y, z } => {
            LibTableColumnMapping::SurfaceGrid {
                x: *x,
                y: *y,
                z: *z,
            }
        }
        crate::plot::table::TableColumnMapping::Curve {
            x,
            y,
            z,
            label,
            group,
        } => LibTableColumnMapping::Curve {
            x: *x,
            y: *y,
            z: convert_optional_column(*z),
            label: convert_optional_column(*label),
            group: convert_optional_column(*group),
        },
        crate::plot::table::TableColumnMapping::Scatter {
            x,
            y,
            z,
            scalar,
            label,
            group,
        } => LibTableColumnMapping::Scatter {
            x: *x,
            y: *y,
            z: convert_optional_column(*z),
            scalar: convert_optional_column(*scalar),
            label: convert_optional_column(*label),
            group: convert_optional_column(*group),
        },
        crate::plot::table::TableColumnMapping::VectorField {
            x,
            y,
            z,
            vx,
            vy,
            vz,
            scalar,
            label,
            group,
        } => LibTableColumnMapping::VectorField {
            x: *x,
            y: *y,
            z: convert_optional_column(*z),
            vx: *vx,
            vy: *vy,
            vz: convert_optional_column(*vz),
            scalar: convert_optional_column(*scalar),
            label: convert_optional_column(*label),
            group: convert_optional_column(*group),
        },
    }
}

#[allow(dead_code)]
fn convert_optional_column(column: crate::plot::table::OptionalColumn) -> LibOptionalColumn {
    match column {
        crate::plot::table::OptionalColumn::None => LibOptionalColumn::None,
        crate::plot::table::OptionalColumn::Column(index) => LibOptionalColumn::Column(index),
    }
}

#[allow(dead_code)]
fn gradient_step(parameters: &[(String, f64)]) -> f64 {
    let scale = parameters
        .iter()
        .map(|(_, value)| value.abs())
        .fold(1.0_f64, f64::max);
    (scale * 0.01).clamp(1.0e-3, 0.25)
}

#[allow(dead_code)]
fn finite_gradient(
    f: impl Fn(f64, f64, f64) -> f64,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> glam::Vec3 {
    let dx = (f(x + h, y, z) - f(x - h, y, z)) / (2.0 * h);
    let dy = (f(x, y + h, z) - f(x, y - h, z)) / (2.0 * h);
    let dz = (f(x, y, z + h) - f(x, y, z - h)) / (2.0 * h);
    glam::vec3(dx as f32, dy as f32, dz as f32)
}

#[allow(dead_code)]
fn finite_divergence(
    f: impl Fn(f64, f64, f64) -> glam::Vec3,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> f32 {
    let ddx = (f(x + h, y, z).x - f(x - h, y, z).x) / (2.0 * h as f32);
    let ddy = (f(x, y + h, z).y - f(x, y - h, z).y) / (2.0 * h as f32);
    let ddz = (f(x, y, z + h).z - f(x, y, z - h).z) / (2.0 * h as f32);
    ddx + ddy + ddz
}

#[allow(dead_code)]
fn finite_curl(
    f: impl Fn(f64, f64, f64) -> glam::Vec3,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> glam::Vec3 {
    let inv = 1.0 / (2.0 * h as f32);
    let d_fz_dy = (f(x, y + h, z).z - f(x, y - h, z).z) * inv;
    let d_fy_dz = (f(x, y, z + h).y - f(x, y, z - h).y) * inv;
    let d_fx_dz = (f(x, y, z + h).x - f(x, y, z - h).x) * inv;
    let d_fz_dx = (f(x + h, y, z).z - f(x - h, y, z).z) * inv;
    let d_fy_dx = (f(x + h, y, z).y - f(x - h, y, z).y) * inv;
    let d_fx_dy = (f(x, y + h, z).x - f(x, y - h, z).x) * inv;
    glam::vec3(d_fz_dy - d_fy_dz, d_fx_dz - d_fz_dx, d_fy_dx - d_fx_dy)
}
