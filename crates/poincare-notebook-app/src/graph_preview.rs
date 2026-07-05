//! CPU-sampled static previews of computed graphs.
//!
//! The interactive 3D viewport (GPU) is a separate, future path. For inline
//! notebook output we render a lightweight 2D thumbnail directly from the
//! [`GraphSpec`] using `poincare-lib`'s CPU expression evaluator: cartesian
//! surfaces become heatmaps over the x-y plane, `y = f(x)` lines become
//! polylines, and plot kinds without a 2D projection are labelled.

use poincare_lib::expr_parser::{eval_with_vars, parse_expr_with_vars};
use poincare_lib::{GraphSpec, PlotDefinition, PlotSpec};
use std::ops::RangeInclusive;

/// Paint a static preview of `spec` inside `rect`.
///
/// Visible plots are laid out left-to-right, each in its own cell with a label.
/// Returns nothing; unrenderable plots are annotated in place.
pub fn paint_graph_spec(ui: &egui::Ui, rect: egui::Rect, spec: &GraphSpec, is_active: bool) {
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let bg = if is_active {
        visuals.selection.bg_fill.gamma_multiply(0.28)
    } else {
        visuals.panel_fill
    };
    painter.rect_filled(rect, egui::CornerRadius::same(6), bg);

    let visible: Vec<&PlotSpec> = spec.plots.iter().filter(|plot| plot.visible).collect();
    if visible.is_empty() {
        centered_note(ui, &painter, rect, "empty graph");
        return;
    }

    // Lay the visible plots out as columns, capped so each cell stays legible.
    const MAX_CELLS: usize = 6;
    let shown = visible.len().min(MAX_CELLS);
    let content = rect.shrink(6.0);
    let cell_width = content.width() / shown as f32;

    for (index, plot) in visible.iter().take(shown).enumerate() {
        let cell = egui::Rect::from_min_size(
            egui::pos2(content.left() + index as f32 * cell_width, content.top()),
            egui::vec2(cell_width, content.height()),
        )
        .shrink(4.0);
        paint_plot(ui, &painter, cell, plot);
    }

    if visible.len() > shown {
        painter.text(
            rect.right_bottom() + egui::vec2(-8.0, -6.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("+{} more", visible.len() - shown),
            egui::TextStyle::Small.resolve(ui.style()),
            visuals.weak_text_color(),
        );
    }
}

fn paint_plot(ui: &egui::Ui, painter: &egui::Painter, cell: egui::Rect, plot: &PlotSpec) {
    let title_height = 14.0;
    let plot_rect = egui::Rect::from_min_max(
        egui::pos2(cell.left(), cell.top() + title_height),
        cell.max,
    );

    match &plot.definition {
        PlotDefinition::ExprCartesian {
            expression,
            parameters,
        } => paint_surface_heatmap(
            ui,
            painter,
            plot_rect,
            expression,
            parameters,
            plot.domain.x.clone(),
            plot.domain.y.clone(),
        ),
        PlotDefinition::ExprCartesianLine {
            ind_var,
            expression,
            parameters,
            ..
        } => paint_line(
            ui,
            painter,
            plot_rect,
            expression,
            ind_var,
            parameters,
            plot.domain.x.clone(),
        ),
        other => {
            centered_note(ui, painter, plot_rect, plot_kind_label(other));
        }
    }

    painter.text(
        cell.left_top(),
        egui::Align2::LEFT_TOP,
        &plot.name,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
}

/// Render `z = f(x, y)` as a colour-mapped heatmap over the x-y plane.
fn paint_surface_heatmap(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    expression: &str,
    parameters: &[(String, f64)],
    x_range: RangeInclusive<f64>,
    y_range: RangeInclusive<f64>,
) {
    let parsed = match parse_expr_with_vars(expression, &["x", "y"]) {
        Ok(parsed) => parsed,
        Err(error) => {
            centered_note(ui, painter, rect, &format!("parse error: {error}"));
            return;
        }
    };

    // Grid resolution scaled to the pixel size, capped to keep it cheap.
    let cols = (rect.width() / 6.0).round().clamp(8.0, 48.0) as usize;
    let rows = (rect.height() / 6.0).round().clamp(8.0, 48.0) as usize;

    let mut values = vec![f64::NAN; cols * rows];
    let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for row in 0..rows {
        // Screen y grows downward; sample the top of the plane first.
        let ty = row as f64 / (rows - 1).max(1) as f64;
        let y = lerp_range(&y_range, 1.0 - ty);
        for col in 0..cols {
            let tx = col as f64 / (cols - 1).max(1) as f64;
            let x = lerp_range(&x_range, tx);
            let vars = [("x", x), ("y", y)];
            let mut all = vars.to_vec();
            all.extend(parameters.iter().map(|(name, value)| (name.as_str(), *value)));
            let z = eval_with_vars(&parsed, &all);
            values[row * cols + col] = z;
            if z.is_finite() {
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
        }
    }

    if !zmin.is_finite() {
        centered_note(ui, painter, rect, "no finite values");
        return;
    }

    let span = (zmax - zmin).abs();
    let cell_w = rect.width() / cols as f32;
    let cell_h = rect.height() / rows as f32;
    for row in 0..rows {
        for col in 0..cols {
            let z = values[row * cols + col];
            if !z.is_finite() {
                continue;
            }
            let t = if span <= f64::EPSILON {
                0.5
            } else {
                ((z - zmin) / span).clamp(0.0, 1.0)
            };
            let color = colormap(t as f32);
            let cell = egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + col as f32 * cell_w,
                    rect.top() + row as f32 * cell_h,
                ),
                egui::vec2(cell_w + 1.0, cell_h + 1.0),
            );
            painter.rect_filled(cell, 0.0, color);
        }
    }

    let range_label = if span <= f64::EPSILON {
        format!("z = {zmin:.3}")
    } else {
        format!("z ∈ [{zmin:.3}, {zmax:.3}]")
    };
    painter.text(
        rect.left_bottom() + egui::vec2(2.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        range_label,
        egui::TextStyle::Small.resolve(ui.style()),
        egui::Color32::WHITE,
    );
}

/// Render `dep = f(ind)` as an autoscaled polyline with a baseline axis.
fn paint_line(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    expression: &str,
    ind_var: &str,
    parameters: &[(String, f64)],
    x_range: RangeInclusive<f64>,
) {
    let parsed = match parse_expr_with_vars(expression, &[ind_var]) {
        Ok(parsed) => parsed,
        Err(error) => {
            centered_note(ui, painter, rect, &format!("parse error: {error}"));
            return;
        }
    };

    let samples = (rect.width() / 2.0).round().clamp(16.0, 256.0) as usize;
    let mut points: Vec<(f64, f64)> = Vec::with_capacity(samples);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for index in 0..samples {
        let t = index as f64 / (samples - 1).max(1) as f64;
        let x = lerp_range(&x_range, t);
        let mut vars = vec![(ind_var, x)];
        vars.extend(parameters.iter().map(|(name, value)| (name.as_str(), *value)));
        let y = eval_with_vars(&parsed, &vars);
        if y.is_finite() {
            ymin = ymin.min(y);
            ymax = ymax.max(y);
        }
        points.push((x, y));
    }

    if !ymin.is_finite() {
        centered_note(ui, painter, rect, "no finite values");
        return;
    }

    // Pad a flat function so it draws mid-cell rather than on an edge.
    if (ymax - ymin).abs() <= f64::EPSILON {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let y_span = ymax - ymin;
    let visuals = ui.visuals();

    // Zero axis if it falls inside the sampled range.
    if ymin <= 0.0 && ymax >= 0.0 {
        let ty = 1.0 - ((0.0 - ymin) / y_span);
        let y = egui::lerp(rect.top()..=rect.bottom(), ty as f32);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        );
    }

    let x_span = x_range.end() - x_range.start();
    let screen: Vec<egui::Pos2> = points
        .iter()
        .filter(|(_, y)| y.is_finite())
        .map(|(x, y)| {
            let tx = if x_span.abs() <= f64::EPSILON {
                0.5
            } else {
                (x - x_range.start()) / x_span
            };
            let ty = 1.0 - ((y - ymin) / y_span);
            egui::pos2(
                egui::lerp(rect.left()..=rect.right(), tx as f32),
                egui::lerp(rect.top()..=rect.bottom(), ty as f32),
            )
        })
        .collect();

    if screen.len() >= 2 {
        painter.add(egui::Shape::line(
            screen,
            egui::Stroke::new(2.0, visuals.hyperlink_color),
        ));
    }

    painter.text(
        rect.left_bottom() + egui::vec2(2.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("y ∈ [{ymin:.3}, {ymax:.3}]"),
        egui::TextStyle::Small.resolve(ui.style()),
        visuals.weak_text_color(),
    );
}

fn centered_note(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, text: &str) {
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
}

fn lerp_range(range: &RangeInclusive<f64>, t: f64) -> f64 {
    range.start() + (range.end() - range.start()) * t
}

/// A compact blue → cyan → green → yellow → red ramp for scalar fields.
fn colormap(t: f32) -> egui::Color32 {
    const STOPS: [(f32, [u8; 3]); 5] = [
        (0.0, [40, 60, 160]),
        (0.25, [40, 160, 180]),
        (0.5, [60, 170, 90]),
        (0.75, [220, 200, 60]),
        (1.0, [210, 70, 55]),
    ];
    let t = t.clamp(0.0, 1.0);
    for window in STOPS.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if t <= t1 {
            let local = if (t1 - t0).abs() <= f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * local) as u8;
            return egui::Color32::from_rgb(
                mix(c0[0], c1[0]),
                mix(c0[1], c1[1]),
                mix(c0[2], c1[2]),
            );
        }
    }
    let last = STOPS[STOPS.len() - 1].1;
    egui::Color32::from_rgb(last[0], last[1], last[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_range_hits_endpoints_and_midpoint() {
        let range = -3.0..=3.0;
        assert_eq!(lerp_range(&range, 0.0), -3.0);
        assert_eq!(lerp_range(&range, 1.0), 3.0);
        assert_eq!(lerp_range(&range, 0.5), 0.0);
    }

    #[test]
    fn colormap_is_clamped_and_monotone_at_ends() {
        let low = colormap(-1.0);
        let high = colormap(2.0);
        assert_eq!(low, colormap(0.0));
        assert_eq!(high, colormap(1.0));
        // Blue-ish low end, red-ish high end.
        assert!(low.b() > low.r());
        assert!(high.r() > high.b());
    }

    #[test]
    fn constant_surface_expression_samples_to_its_value() {
        let parsed = parse_expr_with_vars("1", &["x", "y"]).unwrap();
        let z = eval_with_vars(&parsed, &[("x", 0.5), ("y", -2.0)]);
        assert_eq!(z, 1.0);
    }

    #[test]
    fn cartesian_line_expression_samples_along_x() {
        let parsed = parse_expr_with_vars("x * x", &["x"]).unwrap();
        assert_eq!(eval_with_vars(&parsed, &[("x", 3.0)]), 9.0);
        assert_eq!(eval_with_vars(&parsed, &[("x", -2.0)]), 4.0);
    }
}

fn plot_kind_label(definition: &PlotDefinition) -> &'static str {
    match definition {
        PlotDefinition::ExprCartesian { .. } => "surface",
        PlotDefinition::ExprCartesianLine { .. } => "line",
        PlotDefinition::ExprCurve { .. } => "curve (no 2D preview)",
        PlotDefinition::ExprPolar { .. } => "polar (no 2D preview)",
        PlotDefinition::ExprSpherical { .. } => "spherical (no 2D preview)",
        PlotDefinition::ExprCylindrical { .. } => "cylindrical (no 2D preview)",
        PlotDefinition::ExprParametricSurface { .. } => "parametric surface (no 2D preview)",
        PlotDefinition::ScatterCloud => "scatter (no 2D preview)",
        PlotDefinition::VectorField | PlotDefinition::ExprVectorField { .. } => {
            "vector field (no 2D preview)"
        }
        PlotDefinition::HelixCurve => "helix (no 2D preview)",
        PlotDefinition::SphericalHarmonic => "spherical harmonic (no 2D preview)",
        _ => "plot (no 2D preview)",
    }
}
