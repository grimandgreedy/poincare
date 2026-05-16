use std::collections::HashMap;

use eframe::egui;
use poincare_lib::{parse_csv_grid, parse_csv_points, parse_curve_expr, parse_expr_with_vars};

use crate::plot::kind::{DEFAULT_ISO_PALETTE, PlotKind, SeedMode};
use crate::plot::sweep::ParameterSweep;
use crate::ui::equation_editor::{EquationEditor, equation_row_ed};

/// Show parameter sliders (and sweep controls) for expression-type plots.
/// Returns `true` if any value changed that requires a scene rebuild.
pub(crate) fn show_expression_params(
    ui: &mut egui::Ui,
    kind: &mut PlotKind,
    slider_dragging: &mut bool,
    eq_ed: &mut EquationEditor,
    sweep_map: &mut HashMap<String, ParameterSweep>,
) -> bool {
    let mut dirty = false;

    match kind {
        PlotKind::ExprCartesian {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "z =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["x", "y"]),
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprCurve {
            expression,
            parameters,
            t_range,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                let row_id = egui::Id::new("prop_curve_expr");
                if let Some(committed) = eq_ed.take_committed_for(row_id) {
                    if let Ok(parsed) = parse_curve_expr(&committed) {
                        let old_params: std::collections::HashMap<String, f64> =
                            parameters.iter().cloned().collect();
                        let mut seen = std::collections::HashSet::new();
                        let mut new_params = Vec::new();
                        for (name, default) in parsed
                            .0
                            .parameters
                            .iter()
                            .chain(parsed.1.parameters.iter())
                            .chain(parsed.2.parameters.iter())
                        {
                            if seen.insert(name.clone()) {
                                let val = old_params.get(name).copied().unwrap_or(*default);
                                new_params.push((name.clone(), val));
                            }
                        }
                        *parameters = new_params;
                    }
                    *expression = committed;
                    dirty = true;
                }
                let resp = equation_row_ed(ui, row_id, "(x,y,z) =", expression, eq_ed, false);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    match parse_curve_expr(expression) {
                        Ok(parsed) => {
                            let old_params: std::collections::HashMap<String, f64> =
                                parameters.iter().cloned().collect();
                            let mut seen = std::collections::HashSet::new();
                            let mut new_params = Vec::new();
                            for (name, default) in parsed
                                .0
                                .parameters
                                .iter()
                                .chain(parsed.1.parameters.iter())
                                .chain(parsed.2.parameters.iter())
                            {
                                if seen.insert(name.clone()) {
                                    let val = old_params.get(name).copied().unwrap_or(*default);
                                    new_params.push((name.clone(), val));
                                }
                            }
                            *parameters = new_params;
                            dirty = true;
                        }
                        Err(err) => {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 110, 110),
                                format!("⚠ {err}"),
                            );
                        }
                    }
                }
            });
            ui.add_space(4.0);
            ui.label("t range");
            ui.horizontal(|ui| {
                let mut t0 = t_range.0;
                let mut t1 = t_range.1;
                let r0 = ui.add(egui::DragValue::new(&mut t0).speed(0.1).prefix("t_min "));
                let r1 = ui.add(egui::DragValue::new(&mut t1).speed(0.1).prefix("t_max "));
                if r0.changed() || r1.changed() {
                    t_range.0 = t0;
                    t_range.1 = t1;
                    dirty = true;
                }
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprCartesianLine {
            dep_var,
            ind_var,
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            let lhs = format!("{dep_var}({ind_var}) =");
            let ind = ind_var.clone();
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    // SAFETY: lhs is borrowed while update_single_expr runs; it doesn't escape
                    // the closure, so leaking a &str is fine here via a local borrow.
                    lhs.as_str(),
                    expression,
                    parameters,
                    move |s| parse_expr_with_vars(s, &[ind.as_str()]),
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprSpherical {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "r =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["theta", "phi"]),
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprCylindrical {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "r =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["theta", "z"]),
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprPolar {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "r =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["theta"]),
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprParametricSurface {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_triple_pipe_expr(
                    ui,
                    expression,
                    parameters,
                    &["u", "v"],
                    &["x(u,v) =", "y(u,v) =", "z(u,v) ="],
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprDataGrid {
            csv_text,
            parse_error,
        } => {
            ui.add_space(6.0);
            ui.label("Grid CSV (first row: ,x1,x2,… / each row: y,z1,z2,…)");
            let before = csv_text.clone();
            ui.add(egui::TextEdit::multiline(csv_text).desired_rows(5));
            if *csv_text != before {
                *parse_error = match parse_csv_grid(csv_text) {
                    Ok(_) => String::new(),
                    Err(e) => e,
                };
                dirty = true;
            }
            if !parse_error.is_empty() {
                ui.colored_label(egui::Color32::RED, parse_error.as_str());
            } else if !csv_text.is_empty() {
                if let Ok((xs, ys, _)) = parse_csv_grid(csv_text) {
                    ui.label(format!("Grid {}x{} loaded", xs.len(), ys.len()));
                }
            }
        }
        PlotKind::ExprCurvePoints {
            csv_text,
            parse_error,
        } => {
            ui.add_space(6.0);
            ui.label("Points CSV (x,y,z per line)");
            let before = csv_text.clone();
            ui.add(egui::TextEdit::multiline(csv_text).desired_rows(5));
            if *csv_text != before {
                *parse_error = match parse_csv_points(csv_text) {
                    Ok(_) => String::new(),
                    Err(errs) => format!("{} parse error(s)", errs.len()),
                };
                dirty = true;
            }
            if !parse_error.is_empty() {
                ui.colored_label(egui::Color32::RED, parse_error.as_str());
            } else if let Ok(pts) = parse_csv_points(csv_text) {
                ui.label(format!("{} points loaded", pts.len()));
            }
        }
        PlotKind::ExprScatter {
            csv_text,
            parse_error,
        } => {
            ui.add_space(6.0);
            ui.label("Scatter CSV (x,y,z or x,y,z,w per line)");
            let before = csv_text.clone();
            ui.add(egui::TextEdit::multiline(csv_text).desired_rows(5));
            if *csv_text != before {
                *parse_error = match parse_csv_points(csv_text) {
                    Ok(_) => String::new(),
                    Err(errs) => format!("{} parse error(s)", errs.len()),
                };
                dirty = true;
            }
            if !parse_error.is_empty() {
                ui.colored_label(egui::Color32::RED, parse_error.as_str());
            } else if let Ok(pts) = parse_csv_points(csv_text) {
                ui.label(format!("{} points loaded", pts.len()));
            }
        }
        PlotKind::ExprVectorField {
            expression,
            parameters,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_triple_pipe_expr(
                    ui,
                    expression,
                    parameters,
                    &["x", "y", "z"],
                    &["vx =", "vy =", "vz ="],
                    eq_ed,
                );
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprVolume {
            expression,
            parameters,
            vol_resolution,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "d =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["x", "y", "z"]),
                    eq_ed,
                );
            });
            ui.label("Volume Resolution");
            ui.horizontal(|ui| {
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[0])
                            .range(8..=256)
                            .prefix("X "),
                    )
                    .changed();
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[1])
                            .range(8..=256)
                            .prefix("Y "),
                    )
                    .changed();
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[2])
                            .range(8..=256)
                            .prefix("Z "),
                    )
                    .changed();
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprIsosurface {
            expression,
            parameters,
            isovalues,
            iso_colours,
            vol_resolution,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_single_expr(
                    ui,
                    "f =",
                    expression,
                    parameters,
                    |s| parse_expr_with_vars(s, &["x", "y", "z"]),
                    eq_ed,
                );
            });
            ui.label("Volume Resolution");
            ui.horizontal(|ui| {
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[0])
                            .range(8..=256)
                            .prefix("X "),
                    )
                    .changed();
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[1])
                            .range(8..=256)
                            .prefix("Y "),
                    )
                    .changed();
                dirty |= ui
                    .add(
                        egui::DragValue::new(&mut vol_resolution[2])
                            .range(8..=256)
                            .prefix("Z "),
                    )
                    .changed();
            });
            ui.add_space(4.0);
            ui.label("Isovalues");
            let mut remove_idx: Option<usize> = None;
            while iso_colours.len() < isovalues.len() {
                let palette = DEFAULT_ISO_PALETTE;
                let c = palette[iso_colours.len() % palette.len()];
                iso_colours.push(c);
            }
            for i in 0..isovalues.len() {
                ui.horizontal(|ui| {
                    dirty |= ui
                        .add(
                            egui::DragValue::new(&mut isovalues[i])
                                .speed(0.1)
                                .prefix("level "),
                        )
                        .changed();
                    let c = &mut iso_colours[i];
                    dirty |= ui.color_edit_button_rgba_unmultiplied(c).changed();
                    if ui.small_button("X").clicked() {
                        remove_idx = Some(i);
                        dirty = true;
                    }
                });
            }
            if let Some(i) = remove_idx {
                isovalues.remove(i);
                iso_colours.remove(i);
            }
            if ui.button("Add Level").clicked() {
                let next_val = isovalues.last().copied().unwrap_or(0.0) + 1.0;
                isovalues.push(next_val);
                let palette = DEFAULT_ISO_PALETTE;
                iso_colours.push(palette[isovalues.len() % palette.len()]);
                dirty = true;
            }
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        PlotKind::ExprStreamlines {
            expression,
            parameters,
            seed_mode,
            step_size,
            max_steps,
        } => {
            ui.add_space(4.0);
            ui.group(|ui| {
                dirty |= update_triple_pipe_expr(
                    ui,
                    expression,
                    parameters,
                    &["x", "y", "z"],
                    &["vx =", "vy =", "vz ="],
                    eq_ed,
                );
            });
            ui.add_space(4.0);
            ui.label("Seed Mode");
            let mode_id: usize = match seed_mode {
                SeedMode::Grid { .. } => 0,
                SeedMode::Plane { .. } => 1,
                SeedMode::ManualCsv { .. } => 2,
            };
            ui.horizontal(|ui| {
                if ui.selectable_label(mode_id == 0, "Grid").clicked() && mode_id != 0 {
                    *seed_mode = SeedMode::Grid {
                        nx: 3,
                        ny: 3,
                        nz: 3,
                    };
                    dirty = true;
                }
                if ui.selectable_label(mode_id == 1, "Plane").clicked() && mode_id != 1 {
                    *seed_mode = SeedMode::Plane {
                        axis: 0,
                        offset: 0.0,
                    };
                    dirty = true;
                }
                if ui.selectable_label(mode_id == 2, "Manual CSV").clicked() && mode_id != 2 {
                    *seed_mode = SeedMode::ManualCsv {
                        csv_text: String::new(),
                    };
                    dirty = true;
                }
            });
            match seed_mode {
                SeedMode::Grid { nx, ny, nz } => {
                    ui.horizontal(|ui| {
                        dirty |= ui
                            .add(egui::DragValue::new(nx).range(1..=20).prefix("nx "))
                            .changed();
                        dirty |= ui
                            .add(egui::DragValue::new(ny).range(1..=20).prefix("ny "))
                            .changed();
                        dirty |= ui
                            .add(egui::DragValue::new(nz).range(1..=20).prefix("nz "))
                            .changed();
                    });
                }
                SeedMode::Plane { axis, offset } => {
                    let axis_labels = ["X", "Y", "Z"];
                    egui::ComboBox::from_label("Axis")
                        .selected_text(axis_labels[*axis])
                        .show_ui(ui, |ui| {
                            for (i, lbl) in axis_labels.iter().enumerate() {
                                if ui.selectable_label(*axis == i, *lbl).clicked() {
                                    *axis = i;
                                    dirty = true;
                                }
                            }
                        });
                    dirty |= ui
                        .add(egui::DragValue::new(offset).speed(0.1).prefix("offset "))
                        .changed();
                }
                SeedMode::ManualCsv { csv_text } => {
                    let before = csv_text.clone();
                    ui.add(egui::TextEdit::multiline(csv_text).desired_rows(4));
                    if *csv_text != before {
                        dirty = true;
                    }
                }
            }
            ui.horizontal(|ui| {
                dirty |= ui
                    .add(
                        egui::DragValue::new(step_size)
                            .speed(0.001)
                            .range(0.001..=1.0)
                            .prefix("step "),
                    )
                    .changed();
                dirty |= ui
                    .add(
                        egui::DragValue::new(max_steps)
                            .range(10..=5000)
                            .prefix("max_steps "),
                    )
                    .changed();
            });
            dirty |= show_param_sliders(ui, parameters, slider_dragging, sweep_map);
        }
        _ => {}
    }

    dirty
}

/// Show parameter sliders and per-parameter sweep controls.
/// Returns `true` if any value changed (debounced on release for manual sliders;
/// immediate for sweep play/reset/reset-value).
pub(crate) fn show_param_sliders(
    ui: &mut egui::Ui,
    parameters: &mut Vec<(String, f64)>,
    slider_dragging: &mut bool,
    sweep_map: &mut HashMap<String, ParameterSweep>,
) -> bool {
    let mut dirty = false;
    if parameters.is_empty() {
        return dirty;
    }

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Parameters").weak().small());
    ui.separator();

    let mut any_dragging = false;
    for (name, value) in parameters.iter_mut() {
        let is_playing = sweep_map.get(name).is_some_and(|s| s.playing);

        ui.horizontal(|ui| {
            // Slider — disabled while the sweep is driving the value.
            let r = ui.add_enabled(
                !is_playing,
                egui::Slider::new(value, -10.0..=10.0)
                    .text(egui::RichText::new(name.as_str()).monospace()),
            );
            if !is_playing {
                if r.dragged() {
                    any_dragging = true;
                }
                if r.drag_stopped() {
                    dirty = true;
                }
            }

            // Play / pause toggle.
            let btn = if is_playing { "⏸" } else { "▶" };
            if ui
                .small_button(btn)
                .on_hover_text("Sweep this parameter")
                .clicked()
            {
                let sweep = sweep_map
                    .entry(name.clone())
                    .or_insert_with(|| ParameterSweep::new_for_value(*value));
                sweep.playing = !sweep.playing;
                if sweep.playing {
                    dirty = true;
                }
            }
        });

        // Sweep settings row — always shown; entry created with sensible defaults on first render.
        {
            let sweep = sweep_map
                .entry(name.clone())
                .or_insert_with(|| ParameterSweep::new_for_value(*value));
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.add(
                    egui::DragValue::new(&mut sweep.min)
                        .speed(0.1)
                        .prefix("min "),
                );
                ui.add(
                    egui::DragValue::new(&mut sweep.max)
                        .speed(0.1)
                        .prefix("max "),
                );
                ui.add(
                    egui::DragValue::new(&mut sweep.speed)
                        .speed(0.01)
                        .range(0.01..=20.0)
                        .prefix("speed "),
                );
                if ui
                    .small_button("↺")
                    .on_hover_text("Reset to start")
                    .clicked()
                {
                    sweep.reset();
                    *value = sweep.current_value();
                    dirty = true;
                }
            });
        }
    }

    if *slider_dragging && !any_dragging {
        dirty = true;
    }
    *slider_dragging = any_dragging;
    dirty
}

/// Update a single expression field; commits on Enter or via the equation editor dialog.
pub(crate) fn update_single_expr(
    ui: &mut egui::Ui,
    lhs: &str,
    expression: &mut String,
    parameters: &mut Vec<(String, f64)>,
    parse_fn: impl Fn(&str) -> Result<poincare_lib::ParsedExpr, String>,
    eq_ed: &mut EquationEditor,
) -> bool {
    let mut dirty = false;
    let row_id = egui::Id::new(("prop", lhs));

    if let Some(committed_text) = eq_ed.take_committed_for(row_id) {
        if let Ok(parsed) = parse_fn(&committed_text) {
            let old: std::collections::HashMap<String, f64> = parameters.iter().cloned().collect();
            *parameters = parsed
                .parameters
                .into_iter()
                .map(|(name, default)| {
                    let val = old.get(&name).copied().unwrap_or(default);
                    (name, val)
                })
                .collect();
        }
        *expression = committed_text;
        dirty = true;
    }

    let resp = equation_row_ed(ui, row_id, lhs, expression, eq_ed, false);
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        match parse_fn(expression) {
            Ok(parsed) => {
                let old: std::collections::HashMap<String, f64> =
                    parameters.iter().cloned().collect();
                *parameters = parsed
                    .parameters
                    .into_iter()
                    .map(|(name, default)| {
                        let val = old.get(&name).copied().unwrap_or(default);
                        (name, val)
                    })
                    .collect();
                dirty = true;
            }
            Err(err) => {
                ui.colored_label(egui::Color32::from_rgb(255, 110, 110), format!("⚠ {err}"));
            }
        }
    }
    dirty
}

/// Update a triple expression stored as `"ex|ey|ez"` with three labeled equation rows.
pub(crate) fn update_triple_pipe_expr(
    ui: &mut egui::Ui,
    expression: &mut String,
    parameters: &mut Vec<(String, f64)>,
    coord_vars: &[&str],
    labels: &[&str; 3],
    eq_ed: &mut EquationEditor,
) -> bool {
    let mut dirty = false;
    let ids = [
        egui::Id::new(("triple", labels[0])),
        egui::Id::new(("triple", labels[1])),
        egui::Id::new(("triple", labels[2])),
    ];

    for (i, id) in ids.iter().enumerate() {
        if let Some(committed_text) = eq_ed.take_committed_for(*id) {
            let raw: Vec<String> = expression.splitn(3, '|').map(String::from).collect();
            let mut p = [
                raw.get(0).cloned().unwrap_or_default(),
                raw.get(1).cloned().unwrap_or_default(),
                raw.get(2).cloned().unwrap_or_default(),
            ];
            p[i] = committed_text;
            if let (Ok(px), Ok(py), Ok(pz)) = (
                parse_expr_with_vars(&p[0], coord_vars),
                parse_expr_with_vars(&p[1], coord_vars),
                parse_expr_with_vars(&p[2], coord_vars),
            ) {
                let old: std::collections::HashMap<String, f64> =
                    parameters.iter().cloned().collect();
                let mut seen = std::collections::HashSet::new();
                let mut new_params = Vec::new();
                for (name, default) in px
                    .parameters
                    .iter()
                    .chain(py.parameters.iter())
                    .chain(pz.parameters.iter())
                {
                    if seen.insert(name.clone()) {
                        new_params.push((name.clone(), old.get(name).copied().unwrap_or(*default)));
                    }
                }
                *parameters = new_params;
            }
            *expression = format!("{}|{}|{}", p[0].trim(), p[1].trim(), p[2].trim());
            dirty = true;
            break;
        }
    }

    let parts: Vec<String> = expression.splitn(3, '|').map(|s| s.to_string()).collect();
    let mut e0 = parts.get(0).cloned().unwrap_or_default();
    let mut e1 = parts.get(1).cloned().unwrap_or_default();
    let mut e2 = parts.get(2).cloned().unwrap_or_default();
    let r0 = equation_row_ed(ui, ids[0], labels[0], &mut e0, eq_ed, false);
    let r1 = equation_row_ed(ui, ids[1], labels[1], &mut e1, eq_ed, false);
    let r2 = equation_row_ed(ui, ids[2], labels[2], &mut e2, eq_ed, false);
    *expression = format!("{}|{}|{}", e0.trim(), e1.trim(), e2.trim());
    let any_enter = (r0.lost_focus() || r1.lost_focus() || r2.lost_focus())
        && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if ui.button("Apply").clicked() || any_enter {
        match (
            parse_expr_with_vars(&e0, coord_vars),
            parse_expr_with_vars(&e1, coord_vars),
            parse_expr_with_vars(&e2, coord_vars),
        ) {
            (Ok(px), Ok(py), Ok(pz)) => {
                let old: std::collections::HashMap<String, f64> =
                    parameters.iter().cloned().collect();
                let mut seen = std::collections::HashSet::new();
                let mut new_params = Vec::new();
                for (name, default) in px
                    .parameters
                    .iter()
                    .chain(py.parameters.iter())
                    .chain(pz.parameters.iter())
                {
                    if seen.insert(name.clone()) {
                        new_params.push((name.clone(), old.get(name).copied().unwrap_or(*default)));
                    }
                }
                *parameters = new_params;
                *expression = format!("{}|{}|{}", e0.trim(), e1.trim(), e2.trim());
                dirty = true;
            }
            _ => {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 110, 110),
                    "⚠ parse error in one or more components",
                );
            }
        }
    }
    dirty
}
