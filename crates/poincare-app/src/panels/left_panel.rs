use eframe::egui;
use poincare_lib::{ColormapSource, ColourMode, DetectedPlotType, auto_detect_plot_type};
use viewport_lib::BuiltinColourmap;

use crate::App;
use crate::color32_from_rgba;
use crate::plot::builder::build_plot_entry_from_inputs;
use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;
use crate::plot::selected_type::SelectedPlotType;
use crate::plot::table::TablePlotTarget;
use crate::ui::domain_editor::truncate_str;
use crate::ui::equation_editor::{equation_row, equation_row_ed, filter_auto_templates};
use crate::ui::table_editor::edit_table_import;

#[derive(Clone, Copy)]
pub(crate) enum PlotMarkerKind {
    Point,
    Curve,
    Streamline,
    Surface,
    Isosurface,
    Volume,
    VectorField,
}

impl PlotMarkerKind {
    pub(crate) fn from_plot_kind(kind: &PlotKind) -> Self {
        match kind {
            PlotKind::ScatterCloud | PlotKind::PointAnnotations { .. } => Self::Point,
            PlotKind::HelixCurve
            | PlotKind::ExprCurve { .. }
            | PlotKind::ExprCartesianLine { .. }
            | PlotKind::InterpolatedCurve { .. }
            | PlotKind::DerivedPolylineGroups { .. } => Self::Curve,
            PlotKind::Streamlines { .. } | PlotKind::ExprStreamlines { .. } => Self::Streamline,
            PlotKind::ContouredSurface { .. }
            | PlotKind::SphericalHarmonic
            | PlotKind::GridSurface
            | PlotKind::ExprCartesian { .. }
            | PlotKind::ExprSpherical { .. }
            | PlotKind::ExprCylindrical { .. }
            | PlotKind::ExprPolar { .. }
            | PlotKind::ExprParametricSurface { .. }
            | PlotKind::DerivedSurfaceMesh { .. }
            | PlotKind::ScalarSlice { .. } => Self::Surface,
            PlotKind::Isosurface { .. } | PlotKind::ExprIsosurface { .. } => Self::Isosurface,
            PlotKind::VolumeRender { .. }
            | PlotKind::ExprVolume { .. }
            | PlotKind::DivergenceField { .. } => Self::Volume,
            PlotKind::VectorField
            | PlotKind::ExprVectorField { .. }
            | PlotKind::VectorSlice { .. }
            | PlotKind::GradientField { .. }
            | PlotKind::CurlField { .. }
            | PlotKind::ArrowAnnotations { .. } => Self::VectorField,
            PlotKind::ImportedTable { definition } => match definition.target {
                TablePlotTarget::SurfaceGrid => Self::Surface,
                TablePlotTarget::Curve => Self::Curve,
                TablePlotTarget::Scatter => Self::Point,
                TablePlotTarget::VectorField => Self::VectorField,
            },
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Point => "Point Plot",
            Self::Curve => "Curve Plot",
            Self::Streamline => "Streamline Plot",
            Self::Surface => "Surface Plot",
            Self::Isosurface => "Isosurface Plot",
            Self::Volume => "Volume Plot",
            Self::VectorField => "Vector Field Plot",
        }
    }
}

pub(crate) fn paint_plot_marker(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    kind: PlotMarkerKind,
) {
    let stroke = egui::Stroke::new(1.6, color);
    let fill = color.gamma_multiply(0.22);
    let c = rect.center();
    let w = rect.width();
    let h = rect.height();
    let left = rect.left() + w * 0.16;
    let right = rect.right() - w * 0.16;
    let top = rect.top() + h * 0.16;
    let bottom = rect.bottom() - h * 0.16;

    match kind {
        PlotMarkerKind::Point => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, top),
                    egui::pos2(right, c.y),
                    egui::pos2(c.x, bottom),
                    egui::pos2(left, c.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
        PlotMarkerKind::Curve => {
            let points = vec![
                egui::pos2(left, c.y + h * 0.18),
                egui::pos2(c.x - w * 0.18, c.y - h * 0.18),
                egui::pos2(c.x + w * 0.02, c.y + h * 0.08),
                egui::pos2(right, c.y - h * 0.16),
            ];
            painter.add(egui::Shape::line(points, stroke));
        }
        PlotMarkerKind::Streamline => {
            let points = vec![
                egui::pos2(left, c.y + h * 0.2),
                egui::pos2(c.x - w * 0.2, c.y - h * 0.2),
                egui::pos2(c.x + w * 0.02, c.y + h * 0.06),
                egui::pos2(right - w * 0.08, c.y - h * 0.14),
            ];
            painter.add(egui::Shape::line(points, stroke));
            painter.line_segment(
                [
                    egui::pos2(right - w * 0.2, c.y - h * 0.24),
                    egui::pos2(right - w * 0.08, c.y - h * 0.14),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(right - w * 0.15, c.y - h * 0.02),
                    egui::pos2(right - w * 0.08, c.y - h * 0.14),
                ],
                stroke,
            );
        }
        PlotMarkerKind::Surface => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(left + w * 0.08, bottom - h * 0.1),
                    egui::pos2(c.x - w * 0.08, top),
                    egui::pos2(right, top + h * 0.12),
                    egui::pos2(c.x + w * 0.08, bottom),
                ],
                fill,
                stroke,
            ));
        }
        PlotMarkerKind::Isosurface => {
            let ring = vec![
                egui::pos2(c.x, top),
                egui::pos2(right - w * 0.12, c.y - h * 0.18),
                egui::pos2(right, c.y + h * 0.08),
                egui::pos2(c.x + w * 0.18, bottom),
                egui::pos2(left + w * 0.1, c.y + h * 0.18),
                egui::pos2(left, c.y - h * 0.08),
            ];
            painter.add(egui::Shape::convex_polygon(ring, fill, stroke));
            painter.circle_stroke(c, w * 0.18, egui::Stroke::new(1.2, color));
        }
        PlotMarkerKind::Volume => {
            let dx = w * 0.12;
            let dy = h * 0.14;
            let back = [
                egui::pos2(left + dx, top),
                egui::pos2(right, top),
                egui::pos2(right, bottom - dy),
                egui::pos2(left + dx, bottom - dy),
            ];
            let front = [
                egui::pos2(left, top + dy),
                egui::pos2(right - dx, top + dy),
                egui::pos2(right - dx, bottom),
                egui::pos2(left, bottom),
            ];
            for edge in back.windows(2) {
                painter.line_segment([edge[0], edge[1]], stroke);
            }
            painter.line_segment([back[3], back[0]], stroke);
            for edge in front.windows(2) {
                painter.line_segment([edge[0], edge[1]], stroke);
            }
            painter.line_segment([front[3], front[0]], stroke);
            for i in 0..4 {
                painter.line_segment([front[i], back[i]], stroke);
            }
        }
        PlotMarkerKind::VectorField => {
            painter.line_segment(
                [
                    egui::pos2(left, bottom),
                    egui::pos2(right - w * 0.14, top + h * 0.14),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(right - w * 0.34, top + h * 0.14),
                    egui::pos2(right - w * 0.14, top + h * 0.14),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(right - w * 0.14, top + h * 0.14),
                    egui::pos2(right - w * 0.14, top + h * 0.34),
                ],
                stroke,
            );
        }
    }
}

fn expression_summary(kind: &PlotKind) -> Option<String> {
    match kind {
        PlotKind::ExprCartesian { expression, .. } => Some(format!("z = {expression}")),
        PlotKind::ExprCurve { .. } => None,
        PlotKind::ExprCartesianLine {
            dep_var,
            ind_var,
            expression,
            ..
        } => Some(format!("{dep_var}({ind_var}) = {expression}")),
        PlotKind::ExprSpherical { expression, .. } => Some(format!("r(θ, φ) = {expression}")),
        PlotKind::ExprCylindrical { expression, .. } => Some(format!("r(θ, z) = {expression}")),
        PlotKind::ExprPolar { expression, .. } => Some(format!("r(θ) = {expression}")),
        PlotKind::ExprParametricSurface { expression, .. } => {
            let parts: Vec<&str> = expression.splitn(3, '|').collect();
            if parts.len() == 3 {
                Some(format!(
                    "({}, {}, {})",
                    parts[0].trim(),
                    parts[1].trim(),
                    parts[2].trim()
                ))
            } else {
                Some(expression.clone())
            }
        }
        PlotKind::ExprVectorField { expression, .. }
        | PlotKind::ExprVolume { expression, .. }
        | PlotKind::ExprIsosurface { expression, .. }
        | PlotKind::ExprStreamlines { expression, .. } => Some(expression.clone()),
        PlotKind::ImportedTable { definition } => {
            Some(format!("Imported {}", definition.target.label()))
        }
        PlotKind::InterpolatedCurve { interpolation, .. } => Some(format!(
            "Interpolated curve ({})",
            interpolation_kind_label(interpolation.kind)
        )),
        _ => None,
    }
}

fn interpolation_kind_label(kind: poincare_lib::CurveInterpolationKind) -> &'static str {
    match kind {
        poincare_lib::CurveInterpolationKind::Linear => "Polyline (Linear)",
        poincare_lib::CurveInterpolationKind::CatmullRom => "Interpolation (Catmull-Rom)",
        poincare_lib::CurveInterpolationKind::CentripetalCatmullRom => {
            "Interpolation (Centripetal Catmull-Rom)"
        }
        poincare_lib::CurveInterpolationKind::MovingAverage => "Smoothing (Moving Average)",
        poincare_lib::CurveInterpolationKind::SavitzkyGolay => "Smoothing (Savitzky-Golay)",
    }
}

#[derive(Clone, Copy)]
enum PlotAction {
    AddPlot,
    Rename(usize),
    Duplicate(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
}

impl App {
    pub(crate) fn open_add_plot_modal(&mut self) {
        self.add_plot_open = true;
        self.add_plot_focus_pending = true;
        if let Some(target) = self.add_plot_type.table_target() {
            self.add_table_import.set_target(target);
        }
    }

    fn add_plot_modal_is_empty(&self) -> bool {
        self.add_expr_fields
            .iter()
            .all(|field| field.trim().is_empty())
            && self.add_iso_values_text.trim() == "1.0, 2.0, 3.0"
            && self.add_table_import.raw_text.trim().is_empty()
    }

    fn plot_row_menu(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        plot_count: usize,
        pending_action: &mut Option<PlotAction>,
    ) {
        if ui.button("Add Plot").clicked() {
            *pending_action = Some(PlotAction::AddPlot);
            ui.close();
        }
        ui.separator();
        if ui.button("Rename").clicked() {
            *pending_action = Some(PlotAction::Rename(index));
            ui.close();
        }
        if ui.button("Duplicate").clicked() {
            *pending_action = Some(PlotAction::Duplicate(index));
            ui.close();
        }
        if index > 0 && ui.button("Move Up").clicked() {
            *pending_action = Some(PlotAction::MoveUp(index));
            ui.close();
        }
        if index + 1 < plot_count && ui.button("Move Down").clicked() {
            *pending_action = Some(PlotAction::MoveDown(index));
            ui.close();
        }
        if let Some(summary) =
            expression_summary(&self.documents[self.active_document_idx].plots[index].kind)
        {
            if ui.button("Copy Expression").clicked() {
                ui.ctx().copy_text(summary);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Remove").clicked() {
            *pending_action = Some(PlotAction::Remove(index));
            ui.close();
        }
    }

    pub(crate) fn representative_plot_color(&self, plot: &PlotEntry) -> egui::Color32 {
        match &plot.style.colour_mode {
            ColourMode::Solid(rgba) => color32_from_rgba(*rgba),
            ColourMode::Colormap { colormap, .. } => match colormap {
                ColormapSource::Builtin(preset) => builtin_colormap_color(*preset),
                ColormapSource::Uploaded(_) => egui::Color32::from_rgb(110, 180, 235),
            },
            ColourMode::ByAttribute { name, .. } => {
                let palette = [
                    egui::Color32::from_rgb(111, 203, 155),
                    egui::Color32::from_rgb(255, 176, 95),
                    egui::Color32::from_rgb(120, 176, 255),
                    egui::Color32::from_rgb(255, 118, 163),
                ];
                let index = name
                    .bytes()
                    .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
                    % palette.len();
                palette[index]
            }
        }
    }

    pub(crate) fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width().max(0.0) - 28.0);
            if ui
                .add_sized(
                    [24.0, 24.0],
                    egui::Button::new(egui::RichText::new("+").strong()),
                )
                .on_hover_text("Add plot")
                .clicked()
            {
                self.open_add_plot_modal();
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let viewport_width = ui.clip_rect().width().max(0.0);
                let plot_count = self.documents[self.active_document_idx].plots.len();
                let selection_key = (
                    self.active_document_idx,
                    self.documents[self.active_document_idx].selected_plot,
                );
                let should_scroll_to_selection =
                    self.last_scrolled_plot_selection != Some(selection_key);
                if plot_count == 0 {
                    self.last_scrolled_plot_selection = Some(selection_key);
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("No plots yet. Use + to add one.")
                            .weak()
                            .small(),
                    );
                    return;
                }

                let mut pending_action: Option<PlotAction> = None;
                let mut apply_rename: Option<usize> = None;
                let mut cancel_rename = false;
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

                let display_rows =
                    plot_display_rows(&self.documents[self.active_document_idx].plots);
                for (index, depth) in display_rows {
                    let is_selected =
                        self.documents[self.active_document_idx].selected_plot == Some(index);
                    let is_renaming = self.renaming_plot == Some(index);
                    let (plot_name, hover_text, marker_color, marker_kind) = {
                        let plot = &self.documents[self.active_document_idx].plots[index];
                        (
                            plot.name.clone(),
                            expression_summary(&plot.kind).unwrap_or_else(|| plot.name.clone()),
                            self.representative_plot_color(plot),
                            PlotMarkerKind::from_plot_kind(&plot.kind),
                        )
                    };
                    let row_width = viewport_width;
                    let indent = depth as f32 * 18.0;
                    let content_width = (row_width - indent).max(64.0);
                    let title_width = (content_width - indent - 94.0).max(64.0);
                    let max_label_chars = ((title_width / 8.0).floor() as usize).max(6);
                    let label = truncate_str(&plot_name, max_label_chars);

                    let row_response = ui
                        .allocate_ui_with_layout(
                            egui::vec2(content_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::Frame::group(ui.style())
                                    .fill(if is_selected {
                                        ui.visuals().selection.bg_fill.gamma_multiply(0.22)
                                    } else {
                                        ui.visuals().faint_bg_color
                                    })
                                    .stroke(if is_selected {
                                        ui.visuals().selection.stroke
                                    } else {
                                        ui.visuals().widgets.noninteractive.bg_stroke
                                    })
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.set_width(content_width);
                                        ui.horizontal(|ui| {
                                            if indent > 0.0 {
                                                ui.add_space(indent);
                                            }
                                            let (marker_rect, marker_response) = ui
                                                .allocate_exact_size(
                                                    egui::vec2(14.0, 14.0),
                                                    egui::Sense::hover(),
                                                );
                                            paint_plot_marker(
                                                ui.painter(),
                                                marker_rect,
                                                marker_color,
                                                marker_kind,
                                            );
                                            marker_response.on_hover_text(marker_kind.label());

                                            if is_renaming {
                                                let response = ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.rename_buf,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                );
                                                if self.rename_needs_focus {
                                                    response.request_focus();
                                                    self.rename_needs_focus = false;
                                                }
                                                if response.lost_focus() && !escape_pressed {
                                                    apply_rename = Some(index);
                                                }
                                                if escape_pressed {
                                                    cancel_rename = true;
                                                }
                                            } else {
                                                let response = ui.add_sized(
                                                    [title_width, 22.0],
                                                    egui::Button::new(label).selected(is_selected),
                                                );
                                                if response.clicked() {
                                                    self.set_selected_plot(
                                                        self.active_document_idx,
                                                        Some(index),
                                                    );
                                                    self.documents[self.active_document_idx]
                                                        .viewport_selection_hidden_for = None;
                                                }
                                                response.on_hover_text(hover_text);
                                            }

                                            let mut visible = self.documents
                                                [self.active_document_idx]
                                                .plots[index]
                                                .visible;
                                            if ui.checkbox(&mut visible, "").changed() {
                                                self.documents[self.active_document_idx]
                                                    .set_plot_family_visibility(index, visible);
                                                self.mark_dirty();
                                            }

                                            ui.menu_button(egui::RichText::new("󰇙"), |ui| {
                                                self.plot_row_menu(
                                                    ui,
                                                    index,
                                                    plot_count,
                                                    &mut pending_action,
                                                );
                                            });
                                        });
                                    });
                            },
                        )
                        .response;
                    row_response.context_menu(|ui| {
                        self.plot_row_menu(ui, index, plot_count, &mut pending_action);
                    });
                    if should_scroll_to_selection && is_selected {
                        row_response.scroll_to_me(None);
                    }

                    ui.add_space(6.0);
                }
                self.last_scrolled_plot_selection = Some(selection_key);

                if let Some(index) = apply_rename {
                    let new_name = self.rename_buf.trim().to_string();
                    if !new_name.is_empty() {
                        self.documents[self.active_document_idx].plots[index].name = new_name;
                        self.mark_dirty();
                    }
                    self.renaming_plot = None;
                }

                if cancel_rename {
                    self.renaming_plot = None;
                }

                if let Some(action) = pending_action {
                    match action {
                        PlotAction::AddPlot => {
                            self.open_add_plot_modal();
                        }
                        PlotAction::Rename(index) => {
                            self.renaming_plot = Some(index);
                            self.rename_buf = self.documents[self.active_document_idx].plots[index]
                                .name
                                .clone();
                            self.rename_needs_focus = true;
                            self.set_selected_plot(self.active_document_idx, Some(index));
                            self.documents[self.active_document_idx]
                                .viewport_selection_hidden_for = None;
                        }
                        PlotAction::Duplicate(index) => {
                            let mut cloned =
                                self.documents[self.active_document_idx].plots[index].clone();
                            cloned.name = format!("{} (copy)", cloned.name);
                            cloned.plot_id = 0;
                            cloned.parent_plot_id = None;
                            cloned.relationship = crate::plot::entry::PlotRelationship::Primary;
                            let inserted_idx =
                                self.insert_plot_entry(self.active_document_idx, index + 1, cloned);
                            self.set_selected_plot(self.active_document_idx, Some(inserted_idx));
                            self.documents[self.active_document_idx]
                                .viewport_selection_hidden_for = None;
                            self.renaming_plot = None;
                            self.mark_dirty();
                        }
                        PlotAction::MoveUp(index) => {
                            self.documents[self.active_document_idx]
                                .plots
                                .swap(index, index - 1);
                            if self.documents[self.active_document_idx].selected_plot == Some(index)
                            {
                                self.documents[self.active_document_idx].selected_plot =
                                    Some(index - 1);
                            } else if self.documents[self.active_document_idx].selected_plot
                                == Some(index - 1)
                            {
                                self.documents[self.active_document_idx].selected_plot =
                                    Some(index);
                            }
                            self.mark_dirty();
                        }
                        PlotAction::MoveDown(index) => {
                            self.documents[self.active_document_idx]
                                .plots
                                .swap(index, index + 1);
                            if self.documents[self.active_document_idx].selected_plot == Some(index)
                            {
                                self.documents[self.active_document_idx].selected_plot =
                                    Some(index + 1);
                            } else if self.documents[self.active_document_idx].selected_plot
                                == Some(index + 1)
                            {
                                self.documents[self.active_document_idx].selected_plot =
                                    Some(index);
                            }
                            self.mark_dirty();
                        }
                        PlotAction::Remove(index) => {
                            self.documents[self.active_document_idx].remove_plot_family(index);
                            self.renaming_plot = match self.renaming_plot {
                                Some(r) if r == index => None,
                                Some(r) if r > index => Some(r - 1),
                                other => other,
                            };
                            self.mark_dirty();
                        }
                    }
                }
            });
    }

    pub(crate) fn show_add_plot_modal(&mut self, ctx: &egui::Context) {
        if !self.add_plot_open {
            return;
        }

        let mut open = self.add_plot_open;
        let mut close_after_submit = false;
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new("Add Plot")
            .open(&mut open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(520.0)
            .default_height(440.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    close_after_submit = self.render_add_plot_form(ui);
                });
            });
        if close_after_submit {
            open = false;
        }
        if escape_pressed && self.add_plot_modal_is_empty() {
            open = false;
        }
        self.add_plot_open = open;
    }

    fn render_add_plot_form(&mut self, ui: &mut egui::Ui) -> bool {
        let mut plot_type = self.add_plot_type;
        egui::ComboBox::from_label("Plot Type")
            .selected_text(self.add_plot_type.label())
            .show_ui(ui, |ui| {
                for pt in SelectedPlotType::all() {
                    ui.selectable_value(&mut plot_type, *pt, pt.label());
                }
            });
        if plot_type != self.add_plot_type {
            self.add_plot_type = plot_type;
            for field in &mut self.add_expr_fields {
                field.clear();
            }
            self.add_iso_values_text = "1.0, 2.0, 3.0".to_string();
            self.add_error.clear();
            if let Some(target) = self.add_plot_type.table_target() {
                self.add_table_import = crate::plot::table::TableImportDefinition::empty(target);
            }
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let mut submit_add = false;

        ui.group(|ui| {
            macro_rules! add_row {
                ($ui:expr, $key:expr, $lhs:expr, $buf:expr, $ed:expr) => {{
                    let id = egui::Id::new($key);
                    if let Some(committed) = $ed.take_committed_for(id) {
                        *$buf = committed;
                    }
                    let response = equation_row_ed($ui, id, $lhs, $buf, $ed, false);
                    if self.add_plot_focus_pending {
                        response.request_focus();
                        self.add_plot_focus_pending = false;
                    }
                    submit_add |= response.lost_focus() && enter_pressed;
                }};
            }

            match self.add_plot_type {
                SelectedPlotType::Auto => {
                    let id = egui::Id::new("add_auto");
                    if let Some(committed) = self.eq_editor.take_committed_for(id) {
                        self.add_expr_fields[0] = committed;
                    }
                    let response = equation_row_ed(
                        ui,
                        id,
                        "expr:",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor,
                        true,
                    );
                    if self.add_plot_focus_pending {
                        response.request_focus();
                        self.add_plot_focus_pending = false;
                    }
                    submit_add |= response.lost_focus() && enter_pressed;

                    let input = self.add_expr_fields[0].trim().to_string();
                    if input.is_empty() {
                        ui.label(
                            egui::RichText::new("Enter an equation, e.g. z = x+y")
                                .weak()
                                .small(),
                        );
                    } else {
                        let result = auto_detect_plot_type(&input);
                        match &result.detected {
                            DetectedPlotType::Unknown => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 110, 110),
                                    egui::RichText::new(
                                        result.error.as_deref().unwrap_or("Unknown type"),
                                    )
                                    .small(),
                                );
                            }
                            DetectedPlotType::CartesianLine { dep, ind } => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new(format!("Line {}({})", dep, ind)).small(),
                                );
                            }
                            DetectedPlotType::CartesianSurface => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new("Cartesian surface z(x,y)").small(),
                                );
                            }
                            DetectedPlotType::SphericalSurface => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new("Spherical surface r(θ,φ)").small(),
                                );
                            }
                            DetectedPlotType::CylindricalSurface => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new("Cylindrical surface r(θ,z)").small(),
                                );
                            }
                            DetectedPlotType::PolarSurface => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new("Polar surface r(θ)").small(),
                                );
                            }
                            DetectedPlotType::PermutedCartesian { dep, ind } => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    egui::RichText::new(format!(
                                        "Cartesian surface {}({},{})",
                                        dep, ind.0, ind.1
                                    ))
                                    .small(),
                                );
                            }
                        }
                    }

                    let visible = filter_auto_templates(&self.add_expr_fields[0]);
                    if !visible.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Templates:").weak().small());
                        ui.horizontal_wrapped(|ui| {
                            for (chip_label, scaffold) in visible {
                                if ui
                                    .small_button(*chip_label)
                                    .on_hover_text(format!("Insert: {scaffold}"))
                                    .clicked()
                                {
                                    let current = self.add_expr_fields[0].trim().to_string();
                                    self.add_expr_fields[0] =
                                        if let Some(eq_pos) = current.find('=') {
                                            let rhs = current[eq_pos + 1..].trim();
                                            format!("{scaffold}{rhs}")
                                        } else {
                                            scaffold.to_string()
                                        };
                                }
                            }
                        });
                    }
                }
                SelectedPlotType::CartesianSurface => {
                    add_row!(
                        ui,
                        "add_z",
                        "z =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::SphericalSurface => {
                    add_row!(
                        ui,
                        "add_r_sph",
                        "r =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::CylindricalSurface => {
                    add_row!(
                        ui,
                        "add_r_cyl",
                        "r =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::PolarSurface => {
                    add_row!(
                        ui,
                        "add_r_pol",
                        "r =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::ParametricSurface => {
                    add_row!(
                        ui,
                        "add_xu",
                        "x(u,v) =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_yu",
                        "y(u,v) =",
                        &mut self.add_expr_fields[1],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_zu",
                        "z(u,v) =",
                        &mut self.add_expr_fields[2],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::DataGridSurface => {
                    edit_table_import(ui, &mut self.add_table_import);
                }
                SelectedPlotType::ParametricCurve => {
                    add_row!(
                        ui,
                        "add_xt",
                        "x(t) =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_yt",
                        "y(t) =",
                        &mut self.add_expr_fields[1],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_zt",
                        "z(t) =",
                        &mut self.add_expr_fields[2],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::CurvePoints => {
                    edit_table_import(ui, &mut self.add_table_import);
                }
                SelectedPlotType::Scatter => {
                    edit_table_import(ui, &mut self.add_table_import);
                }
                SelectedPlotType::TableVectorField => {
                    edit_table_import(ui, &mut self.add_table_import);
                }
                SelectedPlotType::VectorField => {
                    add_row!(
                        ui,
                        "add_vx",
                        "vx =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_vy",
                        "vy =",
                        &mut self.add_expr_fields[1],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_vz",
                        "vz =",
                        &mut self.add_expr_fields[2],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::Volume => {
                    add_row!(
                        ui,
                        "add_d",
                        "d =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                }
                SelectedPlotType::Isosurface => {
                    add_row!(
                        ui,
                        "add_f",
                        "f =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                    ui.add_space(2.0);
                    equation_row(ui, "levels =", &mut self.add_iso_values_text);
                }
                SelectedPlotType::Streamlines => {
                    add_row!(
                        ui,
                        "add_svx",
                        "vx =",
                        &mut self.add_expr_fields[0],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_svy",
                        "vy =",
                        &mut self.add_expr_fields[1],
                        &mut self.eq_editor
                    );
                    add_row!(
                        ui,
                        "add_svz",
                        "vz =",
                        &mut self.add_expr_fields[2],
                        &mut self.eq_editor
                    );
                }
            }
        });

        if !self.add_error.is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 110, 110),
                format!("⚠ {}", self.add_error),
            );
        }

        let mut close = false;
        if ui
            .add_sized(
                [ui.available_width().max(0.0), 32.0],
                egui::Button::new(egui::RichText::new("+ Add Plot").strong()),
            )
            .clicked()
            || submit_add
        {
            close = self.try_add_plot_from_inputs();
        }

        close
    }

    fn try_add_plot_from_inputs(&mut self) -> bool {
        let result = build_plot_entry_from_inputs(
            self.add_plot_type,
            &self.add_expr_fields,
            &self.add_table_import,
            &self.add_iso_values_text,
        );
        match result {
            Ok(mut entry) => {
                self.apply_default_colormap_to_entry(&mut entry);
                let selected_idx = self.append_plot_entry(self.active_document_idx, entry);
                self.documents[self.active_document_idx].selected_plot = Some(selected_idx);
                self.add_error.clear();
                self.mark_dirty();
                true
            }
            Err(err) => {
                self.add_error = err;
                false
            }
        }
    }
}

fn builtin_colormap_color(preset: BuiltinColourmap) -> egui::Color32 {
    match preset {
        BuiltinColourmap::Viridis => egui::Color32::from_rgb(77, 190, 118),
        BuiltinColourmap::Plasma => egui::Color32::from_rgb(230, 126, 73),
        BuiltinColourmap::Greyscale => egui::Color32::from_rgb(168, 168, 168),
        BuiltinColourmap::Coolwarm => egui::Color32::from_rgb(205, 114, 130),
        BuiltinColourmap::Rainbow => egui::Color32::from_rgb(118, 180, 255),
        BuiltinColourmap::Magma => egui::Color32::from_rgb(209, 109, 84),
        BuiltinColourmap::Inferno => egui::Color32::from_rgb(245, 149, 62),
        BuiltinColourmap::Turbo => egui::Color32::from_rgb(78, 206, 181),
        BuiltinColourmap::Jet => egui::Color32::from_rgb(72, 155, 235),
        BuiltinColourmap::RdBu => egui::Color32::from_rgb(178, 118, 206),
    }
}

pub(crate) fn plot_display_rows(plots: &[PlotEntry]) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let valid_ids = plots
        .iter()
        .map(|plot| plot.plot_id)
        .collect::<std::collections::HashSet<_>>();
    for (index, plot) in plots.iter().enumerate() {
        if plot
            .parent_plot_id
            .is_some_and(|parent| valid_ids.contains(&parent))
        {
            continue;
        }
        rows.push((index, 0));
        rows.extend(
            plots
                .iter()
                .enumerate()
                .filter(|(_, child)| child.parent_plot_id == Some(plot.plot_id))
                .map(|(child_index, _)| (child_index, 1)),
        );
    }
    rows
}
