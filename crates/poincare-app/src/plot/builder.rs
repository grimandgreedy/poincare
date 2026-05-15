use poincare_lib::{
    ColormapSource, ColourMode, DetectedPlotType, Domain, PlotStyle, Resolution, TransferFunction,
    auto_detect_plot_type, parse_csv_grid, parse_csv_points, parse_expr_with_vars,
};
use viewport_lib::{AttributeKind, BuiltinColormap};

/// Replace whole-identifier occurrences of `from` with `to` in an expression string.
/// Unlike a plain `str::replace`, this respects identifier boundaries so that e.g.
/// replacing `"x"` in `"exp(x)"` yields `"exp(t)"` rather than `"e_tp(t)"`.
fn replace_var(expr: &str, from: &str, to: &str) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let from_chars: Vec<char> = from.chars().collect();
    let flen = from_chars.len();
    let mut out = String::with_capacity(expr.len());
    let mut i = 0;
    while i < chars.len() {
        if i + flen <= chars.len() && chars[i..i + flen] == from_chars[..] {
            let before_ok = i == 0 || {
                let c = chars[i - 1];
                !c.is_alphanumeric() && c != '_'
            };
            let after_ok = i + flen >= chars.len() || {
                let c = chars[i + flen];
                !c.is_alphanumeric() && c != '_'
            };
            if before_ok && after_ok {
                out.push_str(to);
                i += flen;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

use crate::plot::entry::PlotEntry;
use crate::plot::kind::{DEFAULT_ISO_PALETTE, PlotKind, SeedMode};
use crate::plot::selected_type::SelectedPlotType;

/// Build a PlotEntry from the current "Add Plot" input state.
pub(crate) fn build_plot_entry_from_inputs(
    plot_type: SelectedPlotType,
    expr_fields: &[String; 3],
    csv_text: &str,
    iso_values_text: &str,
) -> Result<PlotEntry, String> {
    let default_domain = Domain {
        x: -5.0..=5.0,
        y: -5.0..=5.0,
        z: -5.0..=5.0,
    };
    let default_res = Resolution { u: 60, v: 60 };

    let entry = match plot_type {
        SelectedPlotType::Auto => {
            let full = expr_fields[0].trim().to_string();
            let result = auto_detect_plot_type(&full);
            if let Some(err) = &result.error {
                return Err(err.clone());
            }
            let rhs = &result.rhs;
            match result.detected {
                DetectedPlotType::CartesianSurface => {
                    let parsed = parse_expr_with_vars(rhs, &["x", "y"])
                        .map_err(|e| format!("Parse error: {e}"))?;
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: default_domain,
                        resolution: default_res,
                        style: PlotStyle {
                            colour_mode: ColourMode::Colormap {
                                colormap: ColormapSource::Builtin(
                                    viewport_lib::BuiltinColormap::Viridis,
                                ),
                                scalar_range: None,
                            },
                            two_sided: true,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprCartesian {
                            expression: rhs.clone(),
                            parameters: parsed.parameters,
                        },
                    }
                }
                DetectedPlotType::SphericalSurface => {
                    let parsed = parse_expr_with_vars(rhs, &["theta", "phi"])
                        .map_err(|e| format!("Parse error: {e}"))?;
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: Domain::default(),
                        resolution: default_res,
                        style: PlotStyle {
                            colour_mode: ColourMode::Colormap {
                                colormap: ColormapSource::Builtin(
                                    viewport_lib::BuiltinColormap::Viridis,
                                ),
                                scalar_range: None,
                            },
                            two_sided: true,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprSpherical {
                            expression: rhs.clone(),
                            parameters: parsed.parameters,
                        },
                    }
                }
                DetectedPlotType::CylindricalSurface => {
                    let parsed = parse_expr_with_vars(rhs, &["theta", "z"])
                        .map_err(|e| format!("Parse error: {e}"))?;
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: default_domain,
                        resolution: default_res,
                        style: PlotStyle {
                            colour_mode: ColourMode::Colormap {
                                colormap: ColormapSource::Builtin(
                                    viewport_lib::BuiltinColormap::Viridis,
                                ),
                                scalar_range: None,
                            },
                            two_sided: true,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprCylindrical {
                            expression: rhs.clone(),
                            parameters: parsed.parameters,
                        },
                    }
                }
                DetectedPlotType::PolarSurface => {
                    let parsed = parse_expr_with_vars(rhs, &["theta"])
                        .map_err(|e| format!("Parse error: {e}"))?;
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: Domain::default(),
                        resolution: default_res,
                        style: PlotStyle {
                            colour_mode: ColourMode::Colormap {
                                colormap: ColormapSource::Builtin(
                                    viewport_lib::BuiltinColormap::Viridis,
                                ),
                                scalar_range: None,
                            },
                            two_sided: true,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprPolar {
                            expression: rhs.clone(),
                            parameters: parsed.parameters,
                        },
                    }
                }
                DetectedPlotType::CartesianLine { dep, ind } => {
                    let parsed = parse_expr_with_vars(rhs, &[ind.as_str()])
                        .map_err(|e| format!("Parse error: {e}"))?;
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: default_domain,
                        resolution: Resolution { u: 200, v: 2 },
                        style: PlotStyle {
                            colour_mode: ColourMode::Solid([1.0, 0.85, 0.2, 1.0]),
                            line_width: 2.0,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprCartesianLine {
                            dep_var: dep,
                            ind_var: ind,
                            expression: rhs.clone(),
                            parameters: parsed.parameters,
                        },
                    }
                }
                DetectedPlotType::PermutedCartesian {
                    dep,
                    ind: (u_var, v_var),
                } => {
                    // Synthesise a parametric surface:
                    //   For dep=y, ind=(x,z): x(u,v)=u, y(u,v)=rhs(u,v), z(u,v)=v
                    //   For dep=x, ind=(y,z): x(u,v)=rhs(u,v), y(u,v)=u, z(u,v)=v
                    let coord_vars = &["u", "v"];
                    // Rewrite the rhs replacing ind vars with u, v (whole-identifier safe)
                    let rhs_uv = replace_var(&replace_var(rhs, &u_var, "u"), &v_var, "v");
                    let (ex, ey, ez) = match dep.as_str() {
                        "y" => ("u".to_string(), rhs_uv, "v".to_string()),
                        "x" => (rhs_uv, "u".to_string(), "v".to_string()),
                        _ => unreachable!(),
                    };
                    let px =
                        parse_expr_with_vars(&ex, coord_vars).map_err(|e| format!("x: {e}"))?;
                    let py =
                        parse_expr_with_vars(&ey, coord_vars).map_err(|e| format!("y: {e}"))?;
                    let pz =
                        parse_expr_with_vars(&ez, coord_vars).map_err(|e| format!("z: {e}"))?;
                    let mut seen = std::collections::HashSet::new();
                    let mut params = Vec::new();
                    for (name, val) in px
                        .parameters
                        .iter()
                        .chain(py.parameters.iter())
                        .chain(pz.parameters.iter())
                    {
                        if seen.insert(name.clone()) {
                            params.push((name.clone(), *val));
                        }
                    }
                    let expression = format!("{}|{}|{}", ex, ey, ez);
                    PlotEntry {
                        name: full.clone(),
                        visible: true,
                        domain: Domain::default(),
                        resolution: default_res,
                        style: PlotStyle {
                            colour_mode: ColourMode::Colormap {
                                colormap: ColormapSource::Builtin(
                                    viewport_lib::BuiltinColormap::Viridis,
                                ),
                                scalar_range: None,
                            },
                            two_sided: true,
                            ..PlotStyle::default()
                        },
                        kind: PlotKind::ExprParametricSurface {
                            expression,
                            parameters: params,
                        },
                    }
                }
                DetectedPlotType::Unknown => {
                    return Err(
                        "Could not determine plot type. Try using a template below.".to_string()
                    );
                }
            }
        }
        SelectedPlotType::CartesianSurface => {
            let expr = expr_fields[0].trim().to_string();
            let parsed = parse_expr_with_vars(&expr, &["x", "y"])
                .map_err(|e| format!("Parse error: {e}"))?;
            PlotEntry {
                name: format!("f(x,y) = {expr}"),
                visible: true,
                domain: default_domain,
                resolution: default_res,
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprCartesian {
                    expression: expr,
                    parameters: parsed.parameters,
                },
            }
        }
        SelectedPlotType::SphericalSurface => {
            let expr = expr_fields[0].trim().to_string();
            let parsed = parse_expr_with_vars(&expr, &["theta", "phi"])
                .map_err(|e| format!("Parse error: {e}"))?;
            PlotEntry {
                name: format!("r(theta,phi) = {expr}"),
                visible: true,
                domain: Domain::default(),
                resolution: default_res,
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprSpherical {
                    expression: expr,
                    parameters: parsed.parameters,
                },
            }
        }
        SelectedPlotType::CylindricalSurface => {
            let expr = expr_fields[0].trim().to_string();
            let parsed = parse_expr_with_vars(&expr, &["theta", "z"])
                .map_err(|e| format!("Parse error: {e}"))?;
            PlotEntry {
                name: format!("r(theta,z) = {expr}"),
                visible: true,
                domain: default_domain,
                resolution: default_res,
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprCylindrical {
                    expression: expr,
                    parameters: parsed.parameters,
                },
            }
        }
        SelectedPlotType::PolarSurface => {
            let expr = expr_fields[0].trim().to_string();
            let parsed =
                parse_expr_with_vars(&expr, &["theta"]).map_err(|e| format!("Parse error: {e}"))?;
            PlotEntry {
                name: format!("r(theta) = {expr}"),
                visible: true,
                domain: Domain::default(),
                resolution: default_res,
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprPolar {
                    expression: expr,
                    parameters: parsed.parameters,
                },
            }
        }
        SelectedPlotType::ParametricSurface => {
            let ex = expr_fields[0].trim();
            let ey = expr_fields[1].trim();
            let ez = expr_fields[2].trim();
            let coord_vars = &["u", "v"];
            let px = parse_expr_with_vars(ex, coord_vars).map_err(|e| format!("x: {e}"))?;
            let py = parse_expr_with_vars(ey, coord_vars).map_err(|e| format!("y: {e}"))?;
            let pz = parse_expr_with_vars(ez, coord_vars).map_err(|e| format!("z: {e}"))?;
            let mut seen = std::collections::HashSet::new();
            let mut params = Vec::new();
            for (name, val) in px
                .parameters
                .iter()
                .chain(py.parameters.iter())
                .chain(pz.parameters.iter())
            {
                if seen.insert(name.clone()) {
                    params.push((name.clone(), *val));
                }
            }
            let expression = format!("{}|{}|{}", ex, ey, ez);
            PlotEntry {
                name: format!("parametric({ex}, {ey}, {ez})"),
                visible: true,
                domain: Domain::default(),
                resolution: default_res,
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprParametricSurface {
                    expression,
                    parameters: params,
                },
            }
        }
        SelectedPlotType::DataGridSurface => {
            let (xs, ys, zs) = parse_csv_grid(csv_text).map_err(|e| format!("Grid CSV: {e}"))?;
            if xs.len() * ys.len() != zs.len() {
                return Err("Grid CSV: row/column mismatch".to_string());
            }
            PlotEntry {
                name: "Data Grid Surface".to_string(),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution::default(),
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Viridis),
                        scalar_range: None,
                    },
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprDataGrid {
                    csv_text: csv_text.to_string(),
                    parse_error: String::new(),
                },
            }
        }
        SelectedPlotType::ParametricCurve => {
            let ex = expr_fields[0].trim();
            let ey = expr_fields[1].trim();
            let ez = expr_fields[2].trim();
            let coord_vars = &["t"];
            let px = parse_expr_with_vars(ex, coord_vars).map_err(|e| format!("x: {e}"))?;
            let py = parse_expr_with_vars(ey, coord_vars).map_err(|e| format!("y: {e}"))?;
            let pz = parse_expr_with_vars(ez, coord_vars).map_err(|e| format!("z: {e}"))?;
            let mut seen = std::collections::HashSet::new();
            let mut params = Vec::new();
            for (name, val) in px
                .parameters
                .iter()
                .chain(py.parameters.iter())
                .chain(pz.parameters.iter())
            {
                if seen.insert(name.clone()) {
                    params.push((name.clone(), *val));
                }
            }
            let expression = format!("({ex}, {ey}, {ez})");
            PlotEntry {
                name: format!("({ex}, {ey}, {ez})"),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution { u: 200, v: 2 },
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([1.0, 0.85, 0.2, 1.0]),
                    line_width: 2.0,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprCurve {
                    expression,
                    parameters: params,
                    t_range: (0.0, std::f64::consts::TAU),
                },
            }
        }
        SelectedPlotType::CurvePoints => {
            parse_csv_points(csv_text).map_err(|errs| format!("{} parse error(s)", errs.len()))?;
            PlotEntry {
                name: "Curve from points".to_string(),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution::default(),
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([1.0, 0.85, 0.2, 1.0]),
                    line_width: 2.0,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprCurvePoints {
                    csv_text: csv_text.to_string(),
                    parse_error: String::new(),
                },
            }
        }
        SelectedPlotType::Scatter => {
            parse_csv_points(csv_text).map_err(|errs| format!("{} parse error(s)", errs.len()))?;
            PlotEntry {
                name: "Scatter".to_string(),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution::default(),
                style: PlotStyle {
                    colour_mode: ColourMode::ByAttribute {
                        name: "z".to_string(),
                        kind: AttributeKind::Vertex,
                    },
                    point_size: 6.0,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprScatter {
                    csv_text: csv_text.to_string(),
                    parse_error: String::new(),
                },
            }
        }
        SelectedPlotType::VectorField => {
            let ex = expr_fields[0].trim();
            let ey = expr_fields[1].trim();
            let ez = expr_fields[2].trim();
            let coord_vars = &["x", "y", "z"];
            let px = parse_expr_with_vars(ex, coord_vars).map_err(|e| format!("vx: {e}"))?;
            let py = parse_expr_with_vars(ey, coord_vars).map_err(|e| format!("vy: {e}"))?;
            let pz = parse_expr_with_vars(ez, coord_vars).map_err(|e| format!("vz: {e}"))?;
            let mut seen = std::collections::HashSet::new();
            let mut params = Vec::new();
            for (name, val) in px
                .parameters
                .iter()
                .chain(py.parameters.iter())
                .chain(pz.parameters.iter())
            {
                if seen.insert(name.clone()) {
                    params.push((name.clone(), *val));
                }
            }
            PlotEntry {
                name: format!("VF ({ex},{ey},{ez})"),
                visible: true,
                domain: default_domain,
                resolution: Resolution { u: 5, v: 5 },
                style: PlotStyle {
                    colour_mode: ColourMode::ByAttribute {
                        name: "magnitude".to_string(),
                        kind: AttributeKind::Vertex,
                    },
                    glyph_scale: 0.8,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprVectorField {
                    expression: format!("{}|{}|{}", ex, ey, ez),
                    parameters: params,
                },
            }
        }
        SelectedPlotType::Volume => {
            let expr = expr_fields[0].trim().to_string();
            let parsed = parse_expr_with_vars(&expr, &["x", "y", "z"])
                .map_err(|e| format!("Parse error: {e}"))?;
            PlotEntry {
                name: format!("volume f(x,y,z) = {expr}"),
                visible: true,
                domain: default_domain,
                resolution: Resolution::default(),
                style: PlotStyle {
                    opacity: 0.3,
                    transfer_function: Some(TransferFunction {
                        opacity_scale: 0.4,
                        threshold: None,
                    }),
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprVolume {
                    expression: expr,
                    parameters: parsed.parameters,
                    vol_resolution: [64, 64, 64],
                },
            }
        }
        SelectedPlotType::Isosurface => {
            let expr = expr_fields[0].trim().to_string();
            let parsed = parse_expr_with_vars(&expr, &["x", "y", "z"])
                .map_err(|e| format!("Parse error: {e}"))?;
            let isovalues: Result<Vec<f64>, _> = iso_values_text
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect();
            let isovalues = isovalues
                .map_err(|_| "Invalid isovalue — use comma-separated numbers".to_string())?;
            let iso_colours = isovalues
                .iter()
                .enumerate()
                .map(|(i, _)| DEFAULT_ISO_PALETTE[i % DEFAULT_ISO_PALETTE.len()])
                .collect();
            PlotEntry {
                name: format!("isosurface f(x,y,z) = {expr}"),
                visible: true,
                domain: default_domain,
                resolution: Resolution::default(),
                style: PlotStyle::default(),
                kind: PlotKind::ExprIsosurface {
                    expression: expr,
                    parameters: parsed.parameters,
                    isovalues,
                    iso_colours,
                    vol_resolution: [64, 64, 64],
                },
            }
        }
        SelectedPlotType::Streamlines => {
            let ex = expr_fields[0].trim();
            let ey = expr_fields[1].trim();
            let ez = expr_fields[2].trim();
            let coord_vars = &["x", "y", "z"];
            let px = parse_expr_with_vars(ex, coord_vars).map_err(|e| format!("vx: {e}"))?;
            let py = parse_expr_with_vars(ey, coord_vars).map_err(|e| format!("vy: {e}"))?;
            let pz = parse_expr_with_vars(ez, coord_vars).map_err(|e| format!("vz: {e}"))?;
            let mut seen = std::collections::HashSet::new();
            let mut params = Vec::new();
            for (name, val) in px
                .parameters
                .iter()
                .chain(py.parameters.iter())
                .chain(pz.parameters.iter())
            {
                if seen.insert(name.clone()) {
                    params.push((name.clone(), *val));
                }
            }
            PlotEntry {
                name: format!("streamlines ({ex},{ey},{ez})"),
                visible: true,
                domain: default_domain,
                resolution: Resolution::default(),
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColormap::Plasma),
                        scalar_range: None,
                    },
                    tube_radius: Some(0.06),
                    ..PlotStyle::default()
                },
                kind: PlotKind::ExprStreamlines {
                    expression: format!("{}|{}|{}", ex, ey, ez),
                    parameters: params,
                    seed_mode: SeedMode::default(),
                    step_size: 0.05,
                    max_steps: 500,
                },
            }
        }
    };

    Ok(entry)
}

/// Build seed points from a SeedMode given a domain.
pub(crate) fn generate_seeds(mode: &SeedMode, domain: &Domain) -> Vec<glam::Vec3> {
    match mode {
        SeedMode::Grid { nx, ny, nz } => {
            let nx = (*nx).max(1) as usize;
            let ny = (*ny).max(1) as usize;
            let nz = (*nz).max(1) as usize;
            let x0 = *domain.x.start() as f32;
            let x1 = *domain.x.end() as f32;
            let y0 = *domain.y.start() as f32;
            let y1 = *domain.y.end() as f32;
            let z0 = *domain.z.start() as f32;
            let z1 = *domain.z.end() as f32;
            let mut seeds = Vec::with_capacity(nx * ny * nz);
            for iz in 0..nz {
                let z = if nz <= 1 {
                    (z0 + z1) * 0.5
                } else {
                    z0 + (z1 - z0) * iz as f32 / (nz - 1) as f32
                };
                for iy in 0..ny {
                    let y = if ny <= 1 {
                        (y0 + y1) * 0.5
                    } else {
                        y0 + (y1 - y0) * iy as f32 / (ny - 1) as f32
                    };
                    for ix in 0..nx {
                        let x = if nx <= 1 {
                            (x0 + x1) * 0.5
                        } else {
                            x0 + (x1 - x0) * ix as f32 / (nx - 1) as f32
                        };
                        seeds.push(glam::Vec3::new(x, y, z));
                    }
                }
            }
            seeds
        }
        SeedMode::Plane { axis, offset } => {
            let n = 5usize;
            let x0 = *domain.x.start() as f32;
            let x1 = *domain.x.end() as f32;
            let y0 = *domain.y.start() as f32;
            let y1 = *domain.y.end() as f32;
            let z0 = *domain.z.start() as f32;
            let z1 = *domain.z.end() as f32;
            let mut seeds = Vec::new();
            for j in 0..n {
                for i in 0..n {
                    let t0 = i as f32 / (n - 1) as f32;
                    let t1 = j as f32 / (n - 1) as f32;
                    let (x, y, z) = match axis {
                        0 => (*offset, y0 + (y1 - y0) * t0, z0 + (z1 - z0) * t1),
                        1 => (x0 + (x1 - x0) * t0, *offset, z0 + (z1 - z0) * t1),
                        _ => (x0 + (x1 - x0) * t0, y0 + (y1 - y0) * t1, *offset),
                    };
                    seeds.push(glam::Vec3::new(x, y, z));
                }
            }
            seeds
        }
        SeedMode::ManualCsv { csv_text } => match parse_csv_points(csv_text) {
            Ok(pts) => pts
                .iter()
                .map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
                .collect(),
            Err(_) => Vec::new(),
        },
    }
}
