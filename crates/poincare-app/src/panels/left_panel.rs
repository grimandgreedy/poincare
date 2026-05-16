use eframe::egui;
use poincare_lib::{auto_detect_plot_type, ColormapSource, ColourMode, DetectedPlotType};
use viewport_lib::BuiltinColourmap;

use crate::color32_from_rgba;
use crate::plot::builder::build_plot_entry_from_inputs;
use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;
use crate::plot::selected_type::SelectedPlotType;
use crate::ui::domain_editor::truncate_str;
use crate::ui::equation_editor::{equation_row, equation_row_ed, filter_auto_templates};
use crate::App;

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
        _ => None,
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
                let plot_count = self.documents[self.active_document_idx].plots.len();
                if plot_count == 0 {
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

                for index in 0..plot_count {
                    let is_selected =
                        self.documents[self.active_document_idx].selected_plot == Some(index);
                    let is_renaming = self.renaming_plot == Some(index);
                    let (plot_name, label, dot_color) = {
                        let plot = &self.documents[self.active_document_idx].plots[index];
                        let label =
                            expression_summary(&plot.kind).unwrap_or_else(|| plot.name.clone());
                        (
                            plot.name.clone(),
                            truncate_str(&label, 28),
                            self.representative_plot_color(plot),
                        )
                    };

                    let row_response = egui::Frame::group(ui.style())
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
                            ui.horizontal(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(10.0, 10.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter()
                                    .circle_filled(dot_rect.center(), 4.5, dot_color);

                                if is_renaming {
                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut self.rename_buf)
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
                                        [ui.available_width() - 56.0, 22.0],
                                        egui::Button::new(label).selected(is_selected),
                                    );
                                    if response.clicked() {
                                        self.documents[self.active_document_idx].selected_plot =
                                            Some(index);
                                    }
                                    response.on_hover_text(plot_name);
                                }

                                let mut visible =
                                    self.documents[self.active_document_idx].plots[index].visible;
                                if ui.checkbox(&mut visible, "").changed() {
                                    self.documents[self.active_document_idx].plots[index].visible =
                                        visible;
                                    self.mark_dirty();
                                }

                                ui.menu_button("⋯", |ui| {
                                    self.plot_row_menu(ui, index, plot_count, &mut pending_action);
                                });
                            });
                        });
                    row_response.response.context_menu(|ui| {
                        self.plot_row_menu(ui, index, plot_count, &mut pending_action);
                    });

                    ui.add_space(6.0);
                }

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
                            self.documents[self.active_document_idx].selected_plot = Some(index);
                        }
                        PlotAction::Duplicate(index) => {
                            let mut cloned =
                                self.documents[self.active_document_idx].plots[index].clone();
                            cloned.name = format!("{} (copy)", cloned.name);
                            self.documents[self.active_document_idx]
                                .plots
                                .insert(index + 1, cloned);
                            self.documents[self.active_document_idx].selected_plot =
                                Some(index + 1);
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
                            self.documents[self.active_document_idx].plots.remove(index);
                            self.documents[self.active_document_idx].selected_plot =
                                match self.documents[self.active_document_idx].selected_plot {
                                    Some(_)
                                        if self.documents[self.active_document_idx]
                                            .plots
                                            .is_empty() =>
                                    {
                                        None
                                    }
                                    Some(sel) if sel == index => Some(index.saturating_sub(1))
                                        .filter(|_| {
                                            !self.documents[self.active_document_idx]
                                                .plots
                                                .is_empty()
                                        }),
                                    Some(sel) if sel > index => Some(sel - 1),
                                    other => other,
                                };
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
            self.add_csv_text.clear();
            self.add_iso_values_text = "1.0, 2.0, 3.0".to_string();
            self.add_error.clear();
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
                    ui.label(egui::RichText::new("CSV grid").weak().small());
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
                    if self.add_plot_focus_pending {
                        response.request_focus();
                        self.add_plot_focus_pending = false;
                    }
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
                    ui.label(egui::RichText::new("x,y,z per line").weak().small());
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
                    if self.add_plot_focus_pending {
                        response.request_focus();
                        self.add_plot_focus_pending = false;
                    }
                }
                SelectedPlotType::Scatter => {
                    ui.label(
                        egui::RichText::new("x,y,z or x,y,z,w per line")
                            .weak()
                            .small(),
                    );
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
                    if self.add_plot_focus_pending {
                        response.request_focus();
                        self.add_plot_focus_pending = false;
                    }
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
            &self.add_csv_text,
            &self.add_iso_values_text,
        );
        match result {
            Ok(mut entry) => {
                self.apply_default_colormap_to_entry(&mut entry);
                self.documents[self.active_document_idx].plots.push(entry);
                self.documents[self.active_document_idx].selected_plot =
                    Some(self.documents[self.active_document_idx].plots.len() - 1);
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
