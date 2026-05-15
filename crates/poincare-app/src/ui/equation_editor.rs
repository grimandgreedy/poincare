use eframe::egui;

/// Colour used for the left-hand-side label of an equation row (e.g. "z =").
pub(crate) const EXPR_LHS_COLOR: egui::Color32 = egui::Color32::from_rgb(120, 175, 255);

/// Templates for Auto mode: (chip label, text inserted into the input field).
pub(crate) const AUTO_TEMPLATES: &[(&str, &str)] = &[
    ("y(x) =", "y(x) = "),
    ("z(x,y) =", "z(x,y) = "),
    ("r(θ,φ) =", "r(theta,phi) = "),
    ("r(θ) =", "r(theta) = "),
    ("r(θ,z) =", "r(theta,z) = "),
    ("f(x,y,z) =", "f(x,y,z) = "),
];

/// Returns the subset of `AUTO_TEMPLATES` that are compatible with `current` input.
/// Matches on the dep-var (identifier before `(` or `=` on the LHS).
pub(crate) fn filter_auto_templates(current: &str) -> &'static [(&'static str, &'static str)] {
    let dep_var = current
        .split('=')
        .next()
        .unwrap_or("")
        .split('(')
        .next()
        .unwrap_or("")
        .trim();

    match dep_var {
        "y" => &AUTO_TEMPLATES[0..1],
        "z" => &AUTO_TEMPLATES[1..2],
        "r" => &AUTO_TEMPLATES[2..5],
        "f" => &AUTO_TEMPLATES[5..6],
        _ => AUTO_TEMPLATES,
    }
}

/// State for the centred calculator-style equation editor dialog.
pub(crate) struct EquationEditor {
    pub(crate) open: bool,
    pub(crate) target_id: Option<egui::Id>,
    pub(crate) edit_buf: String,
    pub(crate) selected_tab: usize,
    pub(crate) committed: Option<(egui::Id, String)>,
    pub(crate) focus_input: bool,
    /// When true, show filtered Auto-mode template chips in the dialog.
    pub(crate) show_auto_templates: bool,
}

impl Default for EquationEditor {
    fn default() -> Self {
        Self {
            open: false,
            target_id: None,
            edit_buf: String::new(),
            selected_tab: 0,
            committed: None,
            focus_input: false,
            show_auto_templates: false,
        }
    }
}

impl EquationEditor {
    /// If a committed expression targets `id`, take and return it.
    pub(crate) fn take_committed_for(&mut self, id: egui::Id) -> Option<String> {
        if matches!(&self.committed, Some((cid, _)) if *cid == id) {
            self.committed.take().map(|(_, t)| t)
        } else {
            None
        }
    }
}

/// Draw a styled inline equation row: `lhs_label  [___monospace_input___]`.
pub(crate) fn equation_row(ui: &mut egui::Ui, lhs: &str, buf: &mut String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(lhs)
                .monospace()
                .strong()
                .color(EXPR_LHS_COLOR),
        );
        ui.add(
            egui::TextEdit::singleline(buf)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .hint_text("expression  (Enter to apply)"),
        )
    })
    .inner
}

/// Like `equation_row` but the LHS label opens the centred equation editor dialog.
///
/// `auto_templates`: when true, the editor dialog will show filtered Auto-mode template chips.
pub(crate) fn equation_row_ed(
    ui: &mut egui::Ui,
    row_id: egui::Id,
    lhs: &str,
    buf: &mut String,
    eq_ed: &mut EquationEditor,
    auto_templates: bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        let lhs_resp = ui
            .add(
                egui::Button::new(
                    egui::RichText::new(lhs)
                        .monospace()
                        .strong()
                        .color(EXPR_LHS_COLOR),
                )
                .frame(false),
            )
            .on_hover_text("Click to open equation editor");
        if lhs_resp.clicked() {
            if eq_ed.open && eq_ed.target_id == Some(row_id) {
                eq_ed.open = false;
            } else {
                eq_ed.open = true;
                eq_ed.target_id = Some(row_id);
                eq_ed.edit_buf = buf.clone();
                eq_ed.focus_input = true;
                eq_ed.show_auto_templates = auto_templates;
            }
        }
        ui.add(
            egui::TextEdit::singleline(buf)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .hint_text("expression  (Enter to apply)"),
        )
    })
    .inner
}

/// Draw the centred calculator-style equation editor dialog.
pub(crate) fn show_eq_editor_window(ctx: &egui::Context, eq_ed: &mut EquationEditor) {
    if !eq_ed.open {
        return;
    }

    const TABS: &[(&str, &[(&str, &str)])] = &[
        (
            "Trig",
            &[
                ("sin(", "sin("),
                ("cos(", "cos("),
                ("tan(", "tan("),
                ("asin(", "asin("),
                ("acos(", "acos("),
                ("atan(", "atan("),
                ("atan2(", "atan2("),
                ("sinh(", "sinh("),
                ("cosh(", "cosh("),
                ("tanh(", "tanh("),
            ],
        ),
        (
            "Exp & Roots",
            &[
                ("exp(", "exp("),
                ("ln(", "ln("),
                ("log(", "log("),
                ("sqrt(", "sqrt("),
                ("cbrt(", "cbrt("),
                ("pow(", "pow("),
            ],
        ),
        (
            "Rounding",
            &[
                ("abs(", "abs("),
                ("floor(", "floor("),
                ("ceil(", "ceil("),
                ("round(", "round("),
                ("sign(", "sign("),
            ],
        ),
        ("Compare", &[("min(", "min("), ("max(", "max(")]),
        ("Constants", &[("pi", "pi"), ("e", "e"), ("tau", "tau")]),
    ];

    let te_id = egui::Id::new("eq_editor_input");

    let push = |buf: &mut String, s: &str| {
        let char_idx = egui::TextEdit::load_state(ctx, te_id)
            .and_then(|st| st.cursor.char_range())
            .map(|r| r.primary.index.min(r.secondary.index))
            .unwrap_or_else(|| buf.chars().count());
        let byte_pos = buf
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(buf.len());
        buf.insert_str(byte_pos, s);
        if let Some(mut st) = egui::TextEdit::load_state(ctx, te_id) {
            let new_idx = char_idx + s.chars().count();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(new_idx),
            )));
            st.store(ctx, te_id);
        }
    };

    let del_char = |buf: &mut String| {
        let char_idx = egui::TextEdit::load_state(ctx, te_id)
            .and_then(|st| st.cursor.char_range())
            .map(|r| r.primary.index.min(r.secondary.index))
            .unwrap_or_else(|| buf.chars().count());
        if char_idx > 0 {
            let byte_pos = buf
                .char_indices()
                .nth(char_idx)
                .map(|(i, _)| i)
                .unwrap_or(buf.len());
            let prev_byte = buf[..byte_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            buf.drain(prev_byte..byte_pos);
            if let Some(mut st) = egui::TextEdit::load_state(ctx, te_id) {
                st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(char_idx - 1),
                )));
                st.store(ctx, te_id);
            }
        }
    };

    let mut open = true;
    egui::Window::new("Equation Editor")
        .id(egui::Id::new("eq_editor_window"))
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .default_width(480.0)
        .show(ctx, |ui| {
            let mut refocus = false;

            let te_resp = ui.add(
                egui::TextEdit::singleline(&mut eq_ed.edit_buf)
                    .id(te_id)
                    .font(egui::FontId::monospace(16.0))
                    .desired_width(f32::INFINITY)
                    .hint_text("enter expression…"),
            );
            if te_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(target_id) = eq_ed.target_id {
                    eq_ed.committed = Some((target_id, eq_ed.edit_buf.clone()));
                }
                eq_ed.open = false;
            }
            if eq_ed.focus_input {
                ui.ctx().memory_mut(|m| m.request_focus(te_id));
                eq_ed.focus_input = false;
            }

            // Auto-mode template chips (filtered based on current buffer content)
            if eq_ed.show_auto_templates {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Templates:").weak().small());
                let visible = filter_auto_templates(&eq_ed.edit_buf);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for (chip_label, scaffold) in visible {
                        if ui.small_button(*chip_label).clicked() {
                            let rhs = if let Some(eq_pos) = eq_ed.edit_buf.find('=') {
                                eq_ed.edit_buf[eq_pos + 1..].trim().to_string()
                            } else {
                                String::new()
                            };
                            eq_ed.edit_buf = format!("{scaffold}{rhs}");
                            refocus = true;
                        }
                    }
                });
                ui.add_space(2.0);
                ui.separator();
            }

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                for (i, (tab_label, _)) in TABS.iter().enumerate() {
                    let active = eq_ed.selected_tab == i;
                    let resp = ui.add(
                        egui::Button::new(egui::RichText::new(*tab_label).small()).selected(active),
                    );
                    if resp.clicked() {
                        eq_ed.selected_tab = i;
                    }
                }
            });

            {
                let (_, funcs) = TABS[eq_ed.selected_tab.min(TABS.len() - 1)];
                ui.add_space(4.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        for (label, insert) in funcs.iter() {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(*label).monospace().size(12.5),
                                    )
                                    .min_size(egui::vec2(56.0, 26.0)),
                                )
                                .clicked()
                            {
                                push(&mut eq_ed.edit_buf, insert);
                                refocus = true;
                            }
                        }
                    });
                });
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            let d = egui::vec2(44.0, 34.0);
            let o = egui::vec2(44.0, 34.0);
            let w = egui::vec2(92.0, 34.0);
            let sp = egui::vec2(4.0, 4.0);

            let digit_btn =
                |ui: &mut egui::Ui, label: &str, insert: &str, buf: &mut String| -> bool {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(label).monospace().size(14.0))
                                .min_size(d),
                        )
                        .clicked()
                    {
                        push(buf, insert);
                        true
                    } else {
                        false
                    }
                };
            let op_btn = |ui: &mut egui::Ui, label: &str, insert: &str, buf: &mut String| -> bool {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(label).monospace().size(14.0))
                            .min_size(o),
                    )
                    .clicked()
                {
                    push(buf, insert);
                    true
                } else {
                    false
                }
            };

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = sp;
                refocus |= digit_btn(ui, "7", "7", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "8", "8", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "9", "9", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "/", " / ", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "^", "^", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "(", "(", &mut eq_ed.edit_buf);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = sp;
                refocus |= digit_btn(ui, "4", "4", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "5", "5", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "6", "6", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "*", " * ", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "+", " + ", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, ")", ")", &mut eq_ed.edit_buf);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = sp;
                refocus |= digit_btn(ui, "1", "1", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "2", "2", &mut eq_ed.edit_buf);
                refocus |= digit_btn(ui, "3", "3", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "-", " - ", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, ".", ".", &mut eq_ed.edit_buf);
                if ui
                    .add(egui::Button::new(egui::RichText::new("⌫").size(16.0)).min_size(o))
                    .clicked()
                {
                    del_char(&mut eq_ed.edit_buf);
                    refocus = true;
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = sp;
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("0").monospace().size(14.0))
                            .min_size(w),
                    )
                    .clicked()
                {
                    push(&mut eq_ed.edit_buf, "0");
                    refocus = true;
                }
                refocus |= op_btn(ui, ",", ", ", &mut eq_ed.edit_buf);
                refocus |= op_btn(ui, "π", "pi", &mut eq_ed.edit_buf);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("CLR").monospace().size(12.0))
                            .min_size(o),
                    )
                    .clicked()
                {
                    eq_ed.edit_buf.clear();
                    refocus = true;
                }
            });

            if refocus {
                ui.ctx().memory_mut(|m| m.request_focus(te_id));
            }

            ui.add_space(6.0);
            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .add_sized([90.0, 30.0], egui::Button::new("Cancel"))
                    .clicked()
                {
                    eq_ed.open = false;
                }
                ui.add_space(6.0);
                if ui
                    .add_sized(
                        [90.0, 30.0],
                        egui::Button::new(egui::RichText::new("Apply").strong()),
                    )
                    .clicked()
                {
                    if let Some(target_id) = eq_ed.target_id {
                        eq_ed.committed = Some((target_id, eq_ed.edit_buf.clone()));
                    }
                    eq_ed.open = false;
                }
            });
        });

    if !open {
        eq_ed.open = false;
    }
}
