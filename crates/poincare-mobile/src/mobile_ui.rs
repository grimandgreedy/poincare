use egui::{
    Align, Align2, Area, Button, Color32, Context, DragValue, Frame, Layout, Margin, Pos2, Rect,
    RichText, Stroke, TextEdit, TopBottomPanel, Ui, Vec2, pos2, vec2,
};
use poincare_mobile_core::{
    MobileShadingMode, PlotAxis, PlotDomainEdit, PlotEditorSnapshot, PlotResolutionEdit, UiCommand,
    UiSnapshot,
};

const TOUCH_TARGET: f32 = 48.0;
const PANEL_RADIUS: f32 = 8.0;
const EDGE_MARGIN: f32 = 12.0;
const DIRECT_HIT_PAD: f32 = 12.0;
const QUICK_BUTTON_RIGHT_INSET: f32 = 20.0;

#[derive(Clone, Debug)]
pub(crate) struct HitRegion {
    pub rect: Rect,
    pub commands: Vec<UiCommand>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderOutput {
    pub commands: Vec<UiCommand>,
    pub hit_regions: Vec<HitRegion>,
}

pub(crate) fn render(ctx: &Context, snapshot: &UiSnapshot) -> RenderOutput {
    apply_mobile_style(ctx);

    let mut commands = Vec::new();
    let mut hit_regions = Vec::new();
    show_hamburger(ctx, &mut commands, &mut hit_regions);
    show_quick_controls(ctx, snapshot, &mut commands, &mut hit_regions);

    if snapshot.sidebar_open {
        show_drawer(ctx, snapshot, &mut commands, &mut hit_regions);
    }

    if snapshot.editor_open {
        show_equation_sheet(ctx, snapshot, &mut commands, &mut hit_regions);
    }

    if snapshot.settings_open {
        show_settings_sheet(ctx, snapshot, &mut commands, &mut hit_regions);
    }

    if snapshot.plot_properties_open {
        show_plot_properties_sheet(ctx, snapshot, &mut commands, &mut hit_regions);
    }

    show_error(ctx, snapshot);
    RenderOutput {
        commands,
        hit_regions,
    }
}

pub(crate) fn hit_top_control(screen_size: Vec2, pos: Pos2) -> Option<UiCommand> {
    let hamburger = Rect::from_min_size(
        pos2(EDGE_MARGIN + 4.0, EDGE_MARGIN + 4.0),
        Vec2::splat(TOUCH_TARGET),
    )
    .expand(DIRECT_HIT_PAD);
    if hamburger.contains(pos) {
        return Some(UiCommand::ToggleMenu);
    }

    let plus_right = screen_size.x - QUICK_BUTTON_RIGHT_INSET;
    let plus = Rect::from_min_size(
        pos2(plus_right - TOUCH_TARGET, EDGE_MARGIN + 6.0),
        Vec2::splat(TOUCH_TARGET),
    )
    .expand(DIRECT_HIT_PAD);
    if plus.contains(pos) {
        return Some(UiCommand::OpenEditor);
    }

    None
}

fn apply_mobile_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = vec2(14.0, 10.0);
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.interact_size = vec2(TOUCH_TARGET, TOUCH_TARGET);
    style.visuals.widgets.inactive.corner_radius = PANEL_RADIUS.into();
    style.visuals.widgets.hovered.corner_radius = PANEL_RADIUS.into();
    style.visuals.widgets.active.corner_radius = PANEL_RADIUS.into();
    style.visuals.widgets.noninteractive.corner_radius = PANEL_RADIUS.into();
    ctx.set_style(style);
}

fn show_hamburger(ctx: &Context, commands: &mut Vec<UiCommand>, hit_regions: &mut Vec<HitRegion>) {
    Area::new("mobile_hamburger_button".into())
        .anchor(Align2::LEFT_TOP, [EDGE_MARGIN, EDGE_MARGIN])
        .show(ctx, |ui| {
            surface_frame(190)
                .inner_margin(Margin::same(4))
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        Vec2::splat(TOUCH_TARGET),
                        Button::new(RichText::new("☰").size(24.0))
                            .fill(Color32::from_black_alpha(0))
                            .stroke(Stroke::NONE),
                    );
                    hit_regions.push(HitRegion {
                        rect: response.rect.expand(DIRECT_HIT_PAD),
                        commands: vec![UiCommand::ToggleMenu],
                    });
                    if response.clicked() {
                        commands.push(UiCommand::ToggleMenu);
                    }
                });
        });
}

fn show_quick_controls(
    ctx: &Context,
    snapshot: &UiSnapshot,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    Area::new("mobile_quick_controls".into())
        .anchor(Align2::RIGHT_TOP, [-EDGE_MARGIN, EDGE_MARGIN])
        .show(ctx, |ui| {
            surface_frame(165)
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(plot_count_label(snapshot.plot_count))
                                .size(15.0)
                                .color(Color32::from_rgb(235, 239, 246)),
                        );

                        let add = icon_button(ui, "+");
                        hit_regions.push(HitRegion {
                            rect: add.rect.expand(DIRECT_HIT_PAD),
                            commands: vec![UiCommand::OpenEditor],
                        });
                        if add.clicked() {
                            commands.push(UiCommand::OpenEditor);
                        }
                    });
                });
        });
}

fn show_drawer(
    ctx: &Context,
    snapshot: &UiSnapshot,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    let screen = ctx.content_rect();
    let drawer_width = screen.width().mul_add(0.82, 0.0).min(340.0).max(280.0);

    Area::new("mobile_drawer_scrim".into())
        .order(egui::Order::Foreground)
        .fixed_pos(screen.left_top())
        .show(ctx, |ui| {
            let response = ui.allocate_rect(screen, egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(120));
            hit_regions.push(HitRegion {
                rect: response.rect,
                commands: vec![UiCommand::CloseMenu],
            });
            if response.clicked() {
                commands.push(UiCommand::CloseMenu);
            }
        });

    egui::SidePanel::left("mobile_drawer")
        .resizable(false)
        .exact_width(drawer_width)
        .frame(
            Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(11, 15, 22, 246))
                .inner_margin(Margin::symmetric(18, 18)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Poincare").size(22.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let done = text_button(ui, "Done");
                    hit_regions.push(HitRegion {
                        rect: done.rect.expand(DIRECT_HIT_PAD),
                        commands: vec![UiCommand::CloseMenu],
                    });
                    if done.clicked() {
                        commands.push(UiCommand::CloseMenu);
                    }
                });
            });

            ui.add_space(14.0);
            let settings = secondary_row(ui, "Settings");
            hit_regions.push(HitRegion {
                rect: settings.rect.expand(DIRECT_HIT_PAD),
                commands: vec![UiCommand::OpenSettings],
            });
            if settings.clicked() {
                commands.push(UiCommand::OpenSettings);
            }

            let add_plot = primary_row(ui, "+ Add plot");
            hit_regions.push(HitRegion {
                rect: add_plot.rect.expand(DIRECT_HIT_PAD),
                commands: vec![UiCommand::OpenEditor],
            });
            if add_plot.clicked() {
                commands.push(UiCommand::OpenEditor);
            }

            section_label(ui, "Plots");
            if snapshot.plots.is_empty() {
                ui.label(
                    RichText::new("No plots yet")
                        .size(15.0)
                        .color(Color32::from_white_alpha(150)),
                );
            }
            for plot in &snapshot.plots {
                let row = selectable_row(ui, plot.name.as_str(), plot.selected);
                hit_regions.push(HitRegion {
                    rect: row.rect.expand(DIRECT_HIT_PAD),
                    commands: vec![UiCommand::OpenPlotProperties(plot.index)],
                });
                if row.clicked() {
                    commands.push(UiCommand::OpenPlotProperties(plot.index));
                }
            }
        });
}

fn show_equation_sheet(
    ctx: &Context,
    snapshot: &UiSnapshot,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    TopBottomPanel::bottom("mobile_equation_sheet")
        .resizable(false)
        .exact_height(190.0)
        .frame(
            Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(11, 15, 22, 248))
                .corner_radius(PANEL_RADIUS)
                .inner_margin(Margin::symmetric(18, 10)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let grabber = egui::Rect::from_center_size(ui.cursor().center(), vec2(42.0, 4.0));
                ui.painter()
                    .rect_filled(grabber, 2.0, Color32::from_white_alpha(70));
                ui.add_space(10.0);
            });

            ui.horizontal(|ui| {
                ui.heading(RichText::new("Equation").size(19.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let done = text_button(ui, "Done");
                    hit_regions.push(HitRegion {
                        rect: done.rect.expand(DIRECT_HIT_PAD),
                        commands: vec![UiCommand::CloseEditor],
                    });
                    if done.clicked() {
                        commands.push(UiCommand::CloseEditor);
                    }
                });
            });

            let mut equation = snapshot.equation.clone();
            ui.add_space(8.0);
            ui.add_sized(
                [ui.available_width(), TOUCH_TARGET],
                TextEdit::singleline(&mut equation).hint_text("z = f(x, y)"),
            );
            if equation != snapshot.equation {
                commands.push(UiCommand::SetEquation(equation));
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let plot = primary_compact(ui, "Plot");
                hit_regions.push(HitRegion {
                    rect: plot.rect.expand(DIRECT_HIT_PAD),
                    commands: vec![UiCommand::SubmitEquation],
                });
                if plot.clicked() {
                    commands.push(UiCommand::SubmitEquation);
                }
                ui.label(
                    RichText::new("Creates z = f(x, y).")
                        .size(13.0)
                        .color(Color32::from_white_alpha(150)),
                );
            });
        });
}

fn show_settings_sheet(
    ctx: &Context,
    snapshot: &UiSnapshot,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    TopBottomPanel::bottom("mobile_settings_sheet")
        .resizable(false)
        .exact_height(210.0)
        .frame(sheet_frame())
        .show(ctx, |ui| {
            sheet_grabber(ui);
            sheet_header(
                ui,
                "Settings",
                UiCommand::CloseSettings,
                commands,
                hit_regions,
            );

            let mut show_grid = snapshot.show_grid;
            if ui.checkbox(&mut show_grid, "Show grid").changed() {
                commands.push(UiCommand::SetShowGrid(show_grid));
            }

            let mut show_ground = snapshot.show_ground;
            if ui.checkbox(&mut show_ground, "Show ground plane").changed() {
                commands.push(UiCommand::SetShowGround(show_ground));
            }
        });
}

fn show_plot_properties_sheet(
    ctx: &Context,
    snapshot: &UiSnapshot,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    let Some(plot) = snapshot.selected_plot.as_ref() else {
        return;
    };

    TopBottomPanel::bottom("mobile_plot_properties_sheet")
        .resizable(false)
        .exact_height(460.0)
        .frame(sheet_frame())
        .show(ctx, |ui| {
            sheet_grabber(ui);
            sheet_header(
                ui,
                "Plot properties",
                UiCommand::ClosePlotProperties,
                commands,
                hit_regions,
            );

            egui::ScrollArea::vertical().show(ui, |ui| {
                edit_surface(ui, plot, commands);
                edit_domain(ui, plot, commands);
                edit_style(ui, plot, commands);
            });
        });
}

fn edit_surface(ui: &mut Ui, plot: &PlotEditorSnapshot, commands: &mut Vec<UiCommand>) {
    section_label(ui, "Surface");
    ui.label(
        RichText::new(plot.name.as_str())
            .size(14.0)
            .color(Color32::from_white_alpha(165)),
    );

    let mut equation = plot.equation.clone();
    ui.add_space(4.0);
    ui.add_sized(
        [ui.available_width(), TOUCH_TARGET],
        TextEdit::singleline(&mut equation).hint_text("z = f(x, y)"),
    );
    if equation != plot.equation {
        commands.push(UiCommand::SetSelectedPlotEquation(equation));
    }

    ui.horizontal(|ui| {
        let mut u = plot.resolution_u;
        let mut v = plot.resolution_v;
        ui.label("Resolution");
        let u_changed = ui.add(DragValue::new(&mut u).range(8..=256)).changed();
        ui.label("x");
        let v_changed = ui.add(DragValue::new(&mut v).range(8..=256)).changed();
        if u_changed || v_changed {
            commands.push(UiCommand::SetSelectedPlotResolution(PlotResolutionEdit {
                u,
                v,
            }));
        }
    });
}

fn edit_domain(ui: &mut Ui, plot: &PlotEditorSnapshot, commands: &mut Vec<UiCommand>) {
    section_label(ui, "Domain");
    edit_range(ui, "X", PlotAxis::X, plot.x_min, plot.x_max, commands);
    edit_range(ui, "Y", PlotAxis::Y, plot.y_min, plot.y_max, commands);
    edit_range(ui, "Z", PlotAxis::Z, plot.z_min, plot.z_max, commands);
}

fn edit_range(
    ui: &mut Ui,
    label: &str,
    axis: PlotAxis,
    min: f64,
    max: f64,
    commands: &mut Vec<UiCommand>,
) {
    let mut next_min = min;
    let mut next_max = max;
    ui.horizontal(|ui| {
        ui.label(label);
        let min_changed = ui
            .add(DragValue::new(&mut next_min).speed(0.1).prefix("min "))
            .changed();
        let max_changed = ui
            .add(DragValue::new(&mut next_max).speed(0.1).prefix("max "))
            .changed();
        if min_changed || max_changed {
            commands.push(UiCommand::SetSelectedPlotDomain(PlotDomainEdit {
                axis,
                min: next_min,
                max: next_max,
            }));
        }
    });
}

fn edit_style(ui: &mut Ui, plot: &PlotEditorSnapshot, commands: &mut Vec<UiCommand>) {
    section_label(ui, "Style");

    let mut opacity = plot.opacity;
    if ui
        .add(egui::Slider::new(&mut opacity, 0.05..=1.0).text("Opacity"))
        .changed()
    {
        commands.push(UiCommand::SetSelectedPlotOpacity(opacity));
    }

    let mut two_sided = plot.two_sided;
    if ui.checkbox(&mut two_sided, "Two-sided surface").changed() {
        commands.push(UiCommand::SetSelectedPlotTwoSided(two_sided));
    }

    ui.horizontal(|ui| {
        ui.label("Shading");
        if ui
            .selectable_label(plot.shading == MobileShadingMode::Smooth, "Smooth")
            .clicked()
        {
            commands.push(UiCommand::SetSelectedPlotShading(MobileShadingMode::Smooth));
        }
        if ui
            .selectable_label(plot.shading == MobileShadingMode::Flat, "Flat")
            .clicked()
        {
            commands.push(UiCommand::SetSelectedPlotShading(MobileShadingMode::Flat));
        }
    });

    let mut colour = plot.colour;
    let mut colour32 = Color32::from_rgba_unmultiplied(
        (colour[0].clamp(0.0, 1.0) * 255.0) as u8,
        (colour[1].clamp(0.0, 1.0) * 255.0) as u8,
        (colour[2].clamp(0.0, 1.0) * 255.0) as u8,
        (colour[3].clamp(0.0, 1.0) * 255.0) as u8,
    );
    if ui.color_edit_button_srgba(&mut colour32).changed() {
        colour = [
            f32::from(colour32.r()) / 255.0,
            f32::from(colour32.g()) / 255.0,
            f32::from(colour32.b()) / 255.0,
            f32::from(colour32.a()) / 255.0,
        ];
        commands.push(UiCommand::SetSelectedPlotColour(colour));
    }
}

fn show_error(ctx: &Context, snapshot: &UiSnapshot) {
    let Some(error) = &snapshot.scene_error else {
        return;
    };

    Area::new("mobile_scene_error".into())
        .anchor(Align2::CENTER_BOTTOM, [0.0, -18.0])
        .show(ctx, |ui| {
            Frame::NONE
                .fill(Color32::from_rgb(95, 30, 28))
                .corner_radius(PANEL_RADIUS)
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(RichText::new(error).color(Color32::from_rgb(255, 232, 226)));
                });
        });
}

fn surface_frame(alpha: u8) -> Frame {
    Frame::NONE
        .fill(Color32::from_black_alpha(alpha))
        .corner_radius(PANEL_RADIUS)
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(28)))
}

fn sheet_frame() -> Frame {
    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(11, 15, 22, 248))
        .corner_radius(PANEL_RADIUS)
        .inner_margin(Margin::symmetric(18, 10))
}

fn sheet_grabber(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        let grabber = egui::Rect::from_center_size(ui.cursor().center(), vec2(42.0, 4.0));
        ui.painter()
            .rect_filled(grabber, 2.0, Color32::from_white_alpha(70));
        ui.add_space(10.0);
    });
}

fn sheet_header(
    ui: &mut Ui,
    title: &str,
    close_command: UiCommand,
    commands: &mut Vec<UiCommand>,
    hit_regions: &mut Vec<HitRegion>,
) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new(title).size(19.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let done = text_button(ui, "Done");
            hit_regions.push(HitRegion {
                rect: done.rect.expand(DIRECT_HIT_PAD),
                commands: vec![close_command.clone()],
            });
            if done.clicked() {
                commands.push(close_command);
            }
        });
    });
}

fn icon_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [TOUCH_TARGET, TOUCH_TARGET],
        Button::new(RichText::new(label).size(20.0))
            .fill(Color32::from_white_alpha(28))
            .stroke(Stroke::NONE),
    )
}

fn text_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [68.0, 40.0],
        Button::new(RichText::new(label).color(Color32::from_rgb(127, 220, 198)))
            .fill(Color32::from_white_alpha(0))
            .stroke(Stroke::NONE),
    )
}

fn primary_compact(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [78.0, TOUCH_TARGET],
        Button::new(RichText::new(label).color(Color32::from_rgb(6, 16, 13)))
            .fill(Color32::from_rgb(127, 220, 198))
            .stroke(Stroke::NONE),
    )
}

fn primary_row(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), TOUCH_TARGET],
        Button::new(RichText::new(label).color(Color32::from_rgb(6, 16, 13)))
            .fill(Color32::from_rgb(127, 220, 198))
            .stroke(Stroke::NONE),
    )
}

fn secondary_row(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), TOUCH_TARGET],
        Button::new(RichText::new(label).color(Color32::from_rgb(235, 239, 246)))
            .fill(Color32::from_white_alpha(18))
            .stroke(Stroke::NONE),
    )
}

fn selectable_row(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        Color32::from_rgb(255, 209, 124)
    } else {
        Color32::from_white_alpha(18)
    };
    let color = if selected {
        Color32::from_rgb(6, 16, 13)
    } else {
        Color32::from_rgb(235, 239, 246)
    };

    ui.add_sized(
        [ui.available_width(), TOUCH_TARGET],
        Button::new(RichText::new(label).color(color))
            .fill(fill)
            .stroke(Stroke::NONE),
    )
}

fn section_label(ui: &mut Ui, label: &str) {
    ui.add_space(18.0);
    ui.label(
        RichText::new(label)
            .size(12.0)
            .color(Color32::from_white_alpha(145)),
    );
    ui.add_space(2.0);
}

fn plot_count_label(plot_count: usize) -> String {
    match plot_count {
        0 => "No plots".to_string(),
        1 => "1 plot".to_string(),
        count => format!("{count} plots"),
    }
}
