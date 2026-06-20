use egui::{
    Align, Align2, Area, Button, Color32, Context, Frame, Layout, Margin, Pos2, Rect, RichText,
    Stroke, TextEdit, TopBottomPanel, Ui, Vec2, pos2, vec2,
};
use poincare_mobile_core::{UiCommand, UiSnapshot};

const TOUCH_TARGET: f32 = 48.0;
const PANEL_RADIUS: f32 = 8.0;
const EDGE_MARGIN: f32 = 12.0;
const DIRECT_HIT_PAD: f32 = 12.0;
const QUICK_BUTTON_GAP: f32 = 8.0;
const QUICK_BUTTON_RIGHT_INSET: f32 = 20.0;
const NEXT_BUTTON_WIDTH: f32 = 64.0;

#[derive(Clone, Debug)]
pub(crate) struct HitRegion {
    pub rect: Rect,
    pub command: UiCommand,
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
        show_drawer(ctx, snapshot, &mut commands);
    }

    if snapshot.editor_open {
        show_equation_sheet(ctx, snapshot, &mut commands);
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
        return Some(UiCommand::ToggleEditor);
    }

    let next_right = plus.left() - QUICK_BUTTON_GAP;
    let next = Rect::from_min_size(
        pos2(next_right - NEXT_BUTTON_WIDTH, EDGE_MARGIN + 6.0),
        vec2(NEXT_BUTTON_WIDTH, TOUCH_TARGET),
    )
    .expand(DIRECT_HIT_PAD);
    if next.contains(pos) {
        return Some(UiCommand::NextPreset);
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
                        command: UiCommand::ToggleMenu,
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
                            RichText::new(snapshot.active_preset_name.as_str())
                                .size(15.0)
                                .color(Color32::from_rgb(235, 239, 246)),
                        );

                        let next = compact_button(ui, "Next");
                        hit_regions.push(HitRegion {
                            rect: next.rect.expand(DIRECT_HIT_PAD),
                            command: UiCommand::NextPreset,
                        });
                        if next.clicked() {
                            commands.push(UiCommand::NextPreset);
                        }

                        let add = icon_button(ui, "+");
                        hit_regions.push(HitRegion {
                            rect: add.rect.expand(DIRECT_HIT_PAD),
                            command: UiCommand::ToggleEditor,
                        });
                        if add.clicked() {
                            commands.push(UiCommand::ToggleEditor);
                        }
                    });
                });
        });
}

fn show_drawer(ctx: &Context, snapshot: &UiSnapshot, commands: &mut Vec<UiCommand>) {
    let screen = ctx.content_rect();
    let drawer_width = screen.width().mul_add(0.82, 0.0).min(340.0).max(280.0);

    Area::new("mobile_drawer_scrim".into())
        .order(egui::Order::Foreground)
        .fixed_pos(screen.left_top())
        .show(ctx, |ui| {
            let response = ui.allocate_rect(screen, egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(120));
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
                    if text_button(ui, "Done").clicked() {
                        commands.push(UiCommand::CloseMenu);
                    }
                });
            });

            ui.add_space(14.0);
            if primary_row(ui, "+ Add plot").clicked() {
                commands.push(UiCommand::OpenEditor);
                commands.push(UiCommand::CloseMenu);
            }
            if drawer_row(ui, "Next preset", false).clicked() {
                commands.push(UiCommand::NextPreset);
            }

            section_label(ui, "Presets");
            for (idx, name) in snapshot.preset_names.iter().enumerate() {
                let selected = idx == snapshot.active_preset_index;
                if drawer_row(ui, name, selected).clicked() {
                    commands.push(UiCommand::SelectPreset(idx));
                }
            }
        });
}

fn show_equation_sheet(ctx: &Context, snapshot: &UiSnapshot, commands: &mut Vec<UiCommand>) {
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
                    if text_button(ui, "Done").clicked() {
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
                if primary_compact(ui, "Plot").clicked() {
                    commands.push(UiCommand::SubmitEquation);
                }
                ui.label(
                    RichText::new("M2 will connect this to poincare-lib plotting.")
                        .size(13.0)
                        .color(Color32::from_white_alpha(150)),
                );
            });
        });
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

fn compact_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [NEXT_BUTTON_WIDTH, TOUCH_TARGET],
        Button::new(label)
            .fill(Color32::from_white_alpha(28))
            .stroke(Stroke::NONE),
    )
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

fn drawer_row(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
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
