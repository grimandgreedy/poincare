use eframe::egui;
use poincare_lib::{DetectedPlotType, auto_detect_plot_type};

use crate::App;
use crate::plot::builder::build_plot_entry_from_inputs;
use crate::plot::kind::PlotKind;
use crate::plot::selected_type::SelectedPlotType;
use crate::ui::domain_editor::truncate_str;
use crate::ui::equation_editor::{equation_row, equation_row_ed, filter_auto_templates};

fn expression_summary(kind: &PlotKind) -> Option<String> {
    match kind {
        PlotKind::ExprCartesian { expression, .. } => Some(format!("z = {expression}")),
        PlotKind::ExprCurve { .. } => None,
        PlotKind::ExprCartesianLine { dep_var, ind_var, expression, .. } => {
            Some(format!("{dep_var}({ind_var}) = {expression}"))
        }
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

impl App {
    pub(crate) fn left_panel(&mut self, ui: &mut egui::Ui) {
        // ── Document header ─────────────────────────────────────────────
        {
            let doc = &self.documents[self.active_document_idx];
            let path_label = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unsaved".to_string());
            let plot_count = doc.plots.len();

            let mut title = doc.title.clone();
            let title_resp = ui.add(
                egui::TextEdit::singleline(&mut title)
                    .font(egui::TextStyle::Heading)
                    .hint_text("Untitled")
                    .desired_width(ui.available_width()),
            );
            if title_resp.changed() {
                self.documents[self.active_document_idx].title = title;
                self.mark_dirty();
            }

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&path_label).weak().small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} plot{}",
                            plot_count,
                            if plot_count == 1 { "" } else { "s" }
                        ))
                        .weak()
                        .small(),
                    );
                });
            });
        }

        ui.separator();

        // ── Add Plot ────────────────────────────────────────────────────
        ui.label("Add Plot");

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
            for f in self.add_expr_fields.iter_mut() {
                f.clear();
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
                    ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
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
                    ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
                }
                SelectedPlotType::Scatter => {
                    ui.label(
                        egui::RichText::new("x,y,z or x,y,z,w per line")
                            .weak()
                            .small(),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut self.add_csv_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(5),
                    );
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

        let add_button = ui.add_sized(
            [ui.available_width().max(0.0), 32.0],
            egui::Button::new(
                egui::RichText::new("+ Add Plot")
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(54, 100, 172))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(78, 130, 214),
            ))
            .corner_radius(6.0),
        );
        if add_button.clicked() || submit_add {
            self.try_add_plot_from_inputs();
        }

        ui.separator();

        // ── Plot list ───────────────────────────────────────────────────
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut remove_index = None;
            let mut dup_index = None;
            let mut swap_up = None;
            let mut swap_down = None;
            let mut toggled = false;
            let mut start_rename: Option<usize> = None;
            let mut apply_rename: Option<usize> = None;
            let mut cancel_rename = false;
            let n = self.documents[self.active_document_idx].plots.len();

            let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

            for index in 0..n {
                let plot_name =
                    self.documents[self.active_document_idx].plots[index].name.clone();
                let display_name = truncate_str(&plot_name, 34);
                let is_selected =
                    self.documents[self.active_document_idx].selected_plot == Some(index);
                let is_renaming = self.renaming_plot == Some(index);
                let domain = &self.documents[self.active_document_idx].plots[index].domain;
                let domain_summary = format!(
                    "x[{:.1}, {:.1}]  y[{:.1}, {:.1}]  z[{:.1}, {:.1}]",
                    *domain.x.start(),
                    *domain.x.end(),
                    *domain.y.start(),
                    *domain.y.end(),
                    *domain.z.start(),
                    *domain.z.end()
                );
                let copy_text = match &self.documents[self.active_document_idx].plots[index].kind {
                    PlotKind::ExprCartesian { expression, .. }
                    | PlotKind::ExprCurve { expression, .. }
                    | PlotKind::ExprSpherical { expression, .. }
                    | PlotKind::ExprCylindrical { expression, .. }
                    | PlotKind::ExprPolar { expression, .. }
                    | PlotKind::ExprParametricSurface { expression, .. }
                    | PlotKind::ExprVectorField { expression, .. }
                    | PlotKind::ExprVolume { expression, .. }
                    | PlotKind::ExprIsosurface { expression, .. }
                    | PlotKind::ExprStreamlines { expression, .. } => expression.clone(),
                    PlotKind::ExprCartesianLine { dep_var, ind_var, expression, .. } => {
                        format!("{dep_var}({ind_var}) = {expression}")
                    }
                    other => other.short_description().to_string(),
                };
                let equation_like = expression_summary(
                    &self.documents[self.active_document_idx].plots[index].kind,
                )
                .map(|summary| truncate_str(&summary, 42))
                .unwrap_or(display_name.clone());

                ui.group(|ui| {
                    // Extra height: 106px = name row (16) + gap (2) + expr+domain (42) + gap (2) + actions (22) + padding (22)
                    let card_size = egui::vec2(ui.available_width(), 106.0);
                    let (card_rect, response) =
                        ui.allocate_exact_size(card_size, egui::Sense::click());
                    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

                    if response.clicked() && !is_renaming {
                        self.documents[self.active_document_idx].selected_plot = Some(index);
                        self.focus_plot_tab();
                    }

                    let visuals = ui.visuals();
                    let bg_fill = if is_selected {
                        visuals.selection.bg_fill.gamma_multiply(0.35)
                    } else if response.hovered() {
                        visuals.widgets.hovered.bg_fill
                    } else {
                        visuals.widgets.noninteractive.bg_fill
                    };
                    let stroke = if is_selected {
                        visuals.selection.stroke
                    } else if response.hovered() {
                        visuals.widgets.hovered.bg_stroke
                    } else {
                        visuals.widgets.noninteractive.bg_stroke
                    };

                    ui.painter().rect(
                        card_rect,
                        egui::CornerRadius::same(6),
                        bg_fill,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let content_rect = card_rect.shrink2(egui::vec2(10.0, 8.0));

                    // Name row: 16px at top (minus checkbox width on right)
                    let name_rect = egui::Rect::from_min_size(
                        content_rect.min,
                        egui::vec2(content_rect.width() - 26.0, 16.0),
                    );
                    // Expression + domain: next 42px below name
                    let summary_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.left(), content_rect.top() + 18.0),
                        egui::pos2(content_rect.right() - 30.0, content_rect.top() + 60.0),
                    );
                    // Visibility checkbox: top-right corner
                    let checkbox_rect = egui::Rect::from_min_size(
                        egui::pos2(content_rect.right() - 22.0, content_rect.top() + 1.0),
                        egui::vec2(18.0, 18.0),
                    );
                    // Action buttons: bottom 22px
                    let actions_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.left(), content_rect.bottom() - 22.0),
                        egui::pos2(content_rect.right(), content_rect.bottom()),
                    );

                    if is_renaming {
                        // ── Rename mode ───────────────────────────────────────────
                        let rename_rect = egui::Rect::from_min_max(
                            content_rect.min,
                            egui::pos2(content_rect.right() - 30.0, content_rect.top() + 60.0),
                        );
                        let mut rename_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rename_rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        rename_ui.add_space(2.0);
                        rename_ui
                            .label(egui::RichText::new("Rename plot:").weak().size(10.0));
                        let text_resp = rename_ui.add(
                            egui::TextEdit::singleline(&mut self.rename_buf)
                                .desired_width(rename_rect.width())
                                .font(egui::TextStyle::Body),
                        );
                        if self.rename_needs_focus {
                            text_resp.request_focus();
                        }
                        // Apply when focus leaves (unless Escape was pressed)
                        if text_resp.lost_focus() {
                            if !escape_pressed {
                                apply_rename = Some(index);
                            } else {
                                cancel_rename = true;
                            }
                        }
                        if escape_pressed && !text_resp.lost_focus() {
                            cancel_rename = true;
                        }

                        let mut actions_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(actions_rect)
                                .layout(egui::Layout::right_to_left(egui::Align::Center)),
                        );
                        actions_ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("✗ Cancel").clicked() {
                                    cancel_rename = true;
                                }
                                if ui.small_button("✓ OK").clicked() {
                                    apply_rename = Some(index);
                                }
                            },
                        );
                    } else {
                        // ── Normal mode ───────────────────────────────────────────
                        // Name label
                        let mut name_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(name_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        let name_resp = name_ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(&display_name)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(180, 180, 200)),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Double-click to rename");
                        if name_resp.double_clicked() {
                            start_rename = Some(index);
                        } else if name_resp.clicked() {
                            self.documents[self.active_document_idx].selected_plot = Some(index);
                            self.focus_plot_tab();
                        }

                        // Expression + domain
                        let mut summary_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(summary_rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        let equation_response = summary_ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(&equation_like)
                                        .monospace()
                                        .size(13.0)
                                        .color(egui::Color32::WHITE),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        summary_ui.add_space(3.0);
                        let domain_response = summary_ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(&domain_summary)
                                        .small()
                                        .color(egui::Color32::from_rgb(220, 220, 230)),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if equation_response.clicked() || domain_response.clicked() {
                            self.documents[self.active_document_idx].selected_plot = Some(index);
                            self.focus_plot_tab();
                        }

                        // Action buttons
                        let mut actions_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(actions_rect)
                                .layout(egui::Layout::right_to_left(egui::Align::Center)),
                        );
                        actions_ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("Remove").clicked() {
                                    remove_index = Some(index);
                                }
                                if ui.small_button("Copy").clicked() {
                                    ui.ctx().copy_text(copy_text.clone());
                                }
                                if ui.small_button("Dup").clicked() {
                                    dup_index = Some(index);
                                }
                                if index + 1 < n && ui.small_button("Dn").clicked() {
                                    swap_down = Some(index);
                                }
                                if index > 0 && ui.small_button("Up").clicked() {
                                    swap_up = Some(index);
                                }
                                if ui.small_button("Rename").clicked() {
                                    start_rename = Some(index);
                                }
                            },
                        );
                    }

                    let mut visible =
                        self.documents[self.active_document_idx].plots[index].visible;
                    let checkbox_response =
                        ui.put(checkbox_rect, egui::Checkbox::without_text(&mut visible));
                    if checkbox_response.changed() {
                        self.documents[self.active_document_idx].plots[index].visible = visible;
                        toggled = true;
                    }

                    if !is_renaming {
                        response.on_hover_text(&plot_name);
                    }
                });
                ui.add_space(6.0);
            }

            // ── Deferred mutations ───────────────────────────────────────
            if let Some(index) = start_rename {
                self.rename_buf =
                    self.documents[self.active_document_idx].plots[index].name.clone();
                self.renaming_plot = Some(index);
                self.rename_needs_focus = true;
                self.documents[self.active_document_idx].selected_plot = Some(index);
            } else {
                self.rename_needs_focus = false;
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

            if toggled {
                self.mark_dirty();
            }

            if let Some(index) = swap_up {
                self.documents[self.active_document_idx]
                    .plots
                    .swap(index, index - 1);
                if self.documents[self.active_document_idx].selected_plot == Some(index) {
                    self.documents[self.active_document_idx].selected_plot = Some(index - 1);
                } else if self.documents[self.active_document_idx].selected_plot
                    == Some(index - 1)
                {
                    self.documents[self.active_document_idx].selected_plot = Some(index);
                }
                if self.renaming_plot == Some(index) {
                    self.renaming_plot = Some(index - 1);
                } else if self.renaming_plot == Some(index - 1) {
                    self.renaming_plot = Some(index);
                }
                self.mark_dirty();
            }

            if let Some(index) = swap_down {
                self.documents[self.active_document_idx]
                    .plots
                    .swap(index, index + 1);
                if self.documents[self.active_document_idx].selected_plot == Some(index) {
                    self.documents[self.active_document_idx].selected_plot = Some(index + 1);
                } else if self.documents[self.active_document_idx].selected_plot
                    == Some(index + 1)
                {
                    self.documents[self.active_document_idx].selected_plot = Some(index);
                }
                if self.renaming_plot == Some(index) {
                    self.renaming_plot = Some(index + 1);
                } else if self.renaming_plot == Some(index + 1) {
                    self.renaming_plot = Some(index);
                }
                self.mark_dirty();
            }

            if let Some(index) = dup_index {
                let mut cloned =
                    self.documents[self.active_document_idx].plots[index].clone();
                cloned.name = format!("{} (copy)", cloned.name);
                self.documents[self.active_document_idx]
                    .plots
                    .insert(index + 1, cloned);
                self.documents[self.active_document_idx].selected_plot = Some(index + 1);
                self.renaming_plot = None;
                self.focus_plot_tab();
                self.mark_dirty();
            }

            if let Some(index) = remove_index {
                self.documents[self.active_document_idx].plots.remove(index);
                self.documents[self.active_document_idx].selected_plot =
                    match self.documents[self.active_document_idx].selected_plot {
                        Some(_)
                            if self.documents[self.active_document_idx].plots.is_empty() =>
                        {
                            None
                        }
                        Some(sel) if sel == index => Some(index.saturating_sub(1)).filter(|_| {
                            !self.documents[self.active_document_idx].plots.is_empty()
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
        });
    }

    fn try_add_plot_from_inputs(&mut self) {
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
                self.focus_plot_tab();
                self.add_error.clear();
                self.mark_dirty();
            }
            Err(err) => {
                self.add_error = err;
            }
        }
    }
}
