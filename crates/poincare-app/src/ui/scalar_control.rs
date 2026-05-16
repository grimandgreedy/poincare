use eframe::egui;

pub(crate) struct ScalarControlResponse {
    pub changed: bool,
    pub dragging: bool,
    pub drag_stopped: bool,
    pub reset_clicked: bool,
    pub play_toggled: bool,
}

impl Default for ScalarControlResponse {
    fn default() -> Self {
        Self {
            changed: false,
            dragging: false,
            drag_stopped: false,
            reset_clicked: false,
            play_toggled: false,
        }
    }
}

pub(crate) struct ScalarControl<'a> {
    pub label: &'a str,
    pub framed: bool,
    pub value: Option<&'a mut f64>,
    pub min: &'a mut f64,
    pub max: &'a mut f64,
    pub step: Option<&'a mut f64>,
    pub speed: Option<&'a mut f64>,
    pub playing: Option<&'a mut bool>,
    pub reset_label: Option<&'a str>,
}

pub(crate) fn edit_scalar_control(
    ui: &mut egui::Ui,
    id_source: impl std::hash::Hash,
    mut control: ScalarControl<'_>,
) -> ScalarControlResponse {
    let mut out = ScalarControlResponse::default();
    let playing_now = control.playing.as_deref().copied().unwrap_or(false);
    let step_size = control
        .step
        .as_deref()
        .copied()
        .unwrap_or(0.1)
        .abs()
        .max(0.000_001);

    let render_body =
        |ui: &mut egui::Ui, out: &mut ScalarControlResponse, control: &mut ScalarControl<'_>| {
            let max_row_width = if control.framed { 440.0 } else { f32::INFINITY };
            if let Some(value) = control.value.as_deref_mut() {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [34.0, 0.0],
                        egui::Label::new(egui::RichText::new(control.label).monospace().strong()),
                    );
                    ui.add_enabled_ui(!playing_now, |ui| {
                        let current_resp = ui.add_sized(
                            [86.0, 0.0],
                            egui::DragValue::new(value)
                                .speed(step_size)
                                .fixed_decimals(3),
                        );
                        out.changed |= current_resp.changed();
                        out.dragging |= current_resp.dragged();
                        out.drag_stopped |= current_resp.drag_stopped();

                        let slider_width = if control.framed {
                            180.0
                        } else {
                            ui.available_width().clamp(120.0, max_row_width) - 104.0
                        };
                        let slider_resp = ui.add_sized(
                            [slider_width.clamp(120.0, 220.0), 0.0],
                            egui::Slider::new(value, *control.min..=*control.max)
                                .step_by(step_size)
                                .show_value(false),
                        );
                        out.changed |= slider_resp.changed();
                        out.dragging |= slider_resp.dragged();
                        out.drag_stopped |= slider_resp.drag_stopped();
                    });

                    if let Some(playing) = control.playing.as_deref_mut() {
                        let btn = if *playing { "Pause" } else { "Play" };
                        if ui.add_sized([50.0, 0.0], egui::Button::new(btn)).clicked() {
                            *playing = !*playing;
                            out.play_toggled = true;
                            out.changed = true;
                        }
                    }

                    if let Some(reset_label) = control.reset_label {
                        if ui
                            .add_sized([54.0, 0.0], egui::Button::new(reset_label))
                            .clicked()
                        {
                            out.reset_clicked = true;
                            out.changed = true;
                        }
                    }
                });
                ui.add_space(4.0);
            }

            egui::Grid::new(egui::Id::new(("scalar_fields", &id_source)))
                .num_columns(8)
                .spacing([8.0, 6.0])
                .min_col_width(0.0)
                .show(ui, |ui| {
                    field(ui, "min", control.min, step_size, false, out);
                    field(ui, "max", control.max, step_size, false, out);
                    if let Some(step) = control.step.as_deref_mut() {
                        field(ui, "step", step, 0.01, true, out);
                    }
                    if let Some(speed) = control.speed.as_deref_mut() {
                        field(ui, "speed", speed, 0.01, true, out);
                    }
                    ui.end_row();
                });
        };

    if control.framed {
        ui.scope(|ui| {
            ui.set_max_width(440.0);
            egui::Frame::group(ui.style()).show(ui, |ui| render_body(ui, &mut out, &mut control));
        });
    } else {
        render_body(ui, &mut out, &mut control);
    }

    if *control.min > *control.max {
        std::mem::swap(control.min, control.max);
    }
    if let Some(step) = control.step {
        *step = step.abs().max(0.000_001);
    }
    if let Some(speed) = control.speed {
        *speed = speed.max(0.01);
    }

    out
}

fn field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    speed: f64,
    positive_only: bool,
    out: &mut ScalarControlResponse,
) {
    ui.label(egui::RichText::new(label).small().weak());
    let drag = egui::DragValue::new(value).speed(speed).fixed_decimals(3);
    let drag = if positive_only {
        drag.range(0.000_001..=1000.0)
    } else {
        drag
    };
    let resp = ui.add_sized([84.0, 0.0], drag);
    out.changed |= resp.changed();
}
