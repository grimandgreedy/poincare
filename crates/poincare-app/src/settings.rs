use eframe::egui;
use viewport_lib::{BuiltinColourmap, GroundPlaneMode, Projection, ViewPreset};

use crate::App;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Section {
    General,
    Graph,
    Input,
    Session,
}

const SECTION_KEY: &str = "__poincare_settings_section__";
const SIDEBAR_W: f32 = 150.0;
const WINDOW_W: f32 = 640.0;
const WINDOW_H: f32 = 440.0;

pub(crate) fn show_settings_window(
    ctx: &egui::Context,
    open: &mut bool,
    app: &mut App,
    _frame: &mut eframe::Frame,
) {
    let mut still_open = true;
    let available = ctx.available_rect().size() - egui::vec2(24.0, 24.0);
    let window_size = egui::vec2(WINDOW_W, WINDOW_H).min(available.max(egui::vec2(420.0, 320.0)));

    egui::Window::new("Settings")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .fixed_size(window_size)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            show_inner(ui, app);
        });

    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if !still_open || escape {
        *open = false;
    }
}

fn show_inner(ui: &mut egui::Ui, app: &mut App) {
    let section_id = egui::Id::new(SECTION_KEY);
    let mut active = ui
        .ctx()
        .data(|d| d.get_temp::<Section>(section_id))
        .unwrap_or(Section::General);

    let total_width = ui.available_width();
    let total_height = ui.available_height().max(1.0);
    let content_width = (total_width - SIDEBAR_W - 14.0).max(220.0);

    ui.set_min_height(total_height);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(SIDEBAR_W);
            ui.set_max_width(SIDEBAR_W);
            for (label, section) in [
                ("General", Section::General),
                ("Visual", Section::Graph),
                ("Input", Section::Input),
                ("Session", Section::Session),
            ] {
                let is_selected = section == active;
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 26.0),
                    egui::Sense::click(),
                );
                if resp.clicked() {
                    active = section;
                }
                let visuals = ui.visuals().clone();
                let bg = if is_selected {
                    visuals.selection.bg_fill
                } else if resp.hovered() {
                    visuals.widgets.hovered.weak_bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };
                if bg != egui::Color32::TRANSPARENT {
                    ui.painter().rect_filled(rect, 4.0, bg);
                }
                let text_color = if is_selected {
                    egui::Color32::WHITE
                } else {
                    visuals.text_color()
                };
                ui.painter().text(
                    egui::pos2(rect.left() + 8.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::TextStyle::Body.resolve(ui.style()),
                    text_color,
                );
            }
        });

        ui.add(egui::Separator::default().vertical());

        ui.vertical(|ui| {
            ui.set_min_width(content_width);
            ui.set_max_width(content_width);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(total_height)
                .id_salt("poincare_settings_content")
                .show(ui, |ui| match active {
                    Section::General => show_general_settings(ui, app),
                    Section::Graph => show_graph_settings(ui, app),
                    Section::Input => show_input_settings(ui, app),
                    Section::Session => show_session_settings(ui, app),
                });
        });
    });

    ui.ctx().data_mut(|d| d.insert_temp(section_id, active));
}

fn field_row(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_width(180.0);
        ui.label(label);
        add_contents(ui);
    });
}

fn color32_row(ui: &mut egui::Ui, label: &str, color: &mut egui::Color32) {
    field_row(ui, label, |ui| {
        let mut srgba = color.to_srgba_unmultiplied();
        if ui
            .color_edit_button_srgba_unmultiplied(&mut srgba)
            .changed()
        {
            *color = egui::Color32::from_rgba_unmultiplied(srgba[0], srgba[1], srgba[2], srgba[3]);
        }
    });
}

fn show_general_settings(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("General");
    ui.separator();
    ui.add_space(8.0);

    if ui.button("Reset to Defaults").clicked() {
        app.reset_settings_to_defaults();
    }
    ui.label(
        "Reset graph, visual, default colormap, and session settings to their default values.",
    );
}

fn show_graph_settings(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Visual");
    ui.separator();
    ui.add_space(8.0);

    let mut axis_changed = false;
    field_row(ui, "Show axes", |ui| {
        axis_changed |= ui
            .checkbox(&mut app.documents[app.active_document_idx].axis_config.show_box, "")
            .changed();
    });
    field_row(ui, "Show grid", |ui| {
        axis_changed |= ui
            .checkbox(&mut app.documents[app.active_document_idx].axis_config.show_grid, "")
            .changed();
    });
    field_row(ui, "Show labels", |ui| {
        axis_changed |= ui
            .checkbox(&mut app.documents[app.active_document_idx].axis_config.show_labels, "")
            .changed();
    });
    field_row(ui, "Show ticks", |ui| {
        axis_changed |= ui
            .checkbox(&mut app.documents[app.active_document_idx].axis_config.show_ticks, "")
            .changed();
    });

    ui.add_space(6.0);
    field_row(ui, "Tick count", |ui| {
        axis_changed |= ui
            .add(
                egui::DragValue::new(&mut app.documents[app.active_document_idx].axis_config.tick_count[0])
                    .range(2..=20)
                    .prefix("X "),
            )
            .changed();
        axis_changed |= ui
            .add(
                egui::DragValue::new(&mut app.documents[app.active_document_idx].axis_config.tick_count[1])
                    .range(2..=20)
                    .prefix("Y "),
            )
            .changed();
        axis_changed |= ui
            .add(
                egui::DragValue::new(&mut app.documents[app.active_document_idx].axis_config.tick_count[2])
                    .range(2..=20)
                    .prefix("Z "),
            )
            .changed();
    });

    if axis_changed {
        app.mark_dirty();
    }

    ui.add_space(12.0);
    ui.separator();
    ui.heading("View");
    ui.horizontal(|ui| {
        if ui.button("Front").clicked() {
            app.set_view_preset(ViewPreset::Front);
        }
        if ui.button("Top").clicked() {
            app.set_view_preset(ViewPreset::Top);
        }
        if ui.button("Iso").clicked() {
            app.set_view_preset(ViewPreset::Isometric);
        }
    });
    field_row(ui, "Projection", |ui| {
        ui.selectable_value(
            &mut app.documents[app.active_document_idx].camera.projection,
            Projection::Perspective,
            "Perspective",
        );
        ui.selectable_value(
            &mut app.documents[app.active_document_idx].camera.projection,
            Projection::Orthographic,
            "Orthographic",
        );
    });
    field_row(ui, "Background", |ui| {
        ui.color_edit_button_rgba_unmultiplied(
            &mut app.documents[app.active_document_idx].viewport_background,
        );
    });
    color32_row(ui, "Panel background", &mut app.panel_style.content.bg);
    color32_row(ui, "Panel header", &mut app.panel_style.header.bg);
    color32_row(ui, "Selected tab", &mut app.panel_style.tabs.active.bg);
    let mut tab_highlight = app.panel_style.tabs.active.accent_color;
    color32_row(ui, "Tab highlight", &mut tab_highlight);
    app.panel_style.tabs.active.accent_color = tab_highlight;
    app.panel_style.tabs.inactive.accent_color = tab_highlight;
    app.panel_style.tabs.hovered.accent_color = tab_highlight;

    ui.add_space(6.0);
    field_row(ui, "Ground plane", |ui| {
        egui::ComboBox::from_id_salt("poincare_settings_ground_plane")
            .selected_text(match app.documents[app.active_document_idx].ground_plane_mode {
                GroundPlaneMode::None => "Off",
                GroundPlaneMode::ShadowOnly => "Shadow Only",
                GroundPlaneMode::Tile => "Tile",
                GroundPlaneMode::SolidColour => "Solid",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut app.documents[app.active_document_idx].ground_plane_mode,
                    GroundPlaneMode::None,
                    "Off",
                );
                ui.selectable_value(
                    &mut app.documents[app.active_document_idx].ground_plane_mode,
                    GroundPlaneMode::ShadowOnly,
                    "Shadow Only",
                );
                ui.selectable_value(
                    &mut app.documents[app.active_document_idx].ground_plane_mode,
                    GroundPlaneMode::Tile,
                    "Tile",
                );
                ui.selectable_value(
                    &mut app.documents[app.active_document_idx].ground_plane_mode,
                    GroundPlaneMode::SolidColour,
                    "Solid",
                );
            });
    });
    field_row(ui, "Ground height", |ui| {
        ui.add(
            egui::DragValue::new(&mut app.documents[app.active_document_idx].ground_plane_height)
                .speed(0.05)
                .prefix("z "),
        );
    });
    if matches!(
        app.documents[app.active_document_idx].ground_plane_mode,
        GroundPlaneMode::Tile | GroundPlaneMode::SolidColour
    ) {
        field_row(ui, "Ground tile size", |ui| {
            ui.add(egui::Slider::new(
                &mut app.documents[app.active_document_idx].ground_plane_tile_size,
                0.1..=10.0,
            ));
        });
        field_row(ui, "Ground colour", |ui| {
            ui.color_edit_button_rgba_unmultiplied(
                &mut app.documents[app.active_document_idx].ground_plane_color,
            );
        });
    }

    ui.add_space(12.0);
    ui.separator();
    ui.heading("Defaults");
    field_row(ui, "Default colormap", |ui| {
        egui::ComboBox::from_id_salt("poincare_settings_default_colormap")
            .selected_text(colormap_name(app.default_colormap))
            .show_ui(ui, |ui| {
                for preset in colormap_presets() {
                    ui.selectable_value(&mut app.default_colormap, preset, colormap_name(preset));
                }
            });
    });
}

fn show_session_settings(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Session");
    ui.separator();
    ui.add_space(8.0);

    field_row(ui, "Save state on exit", |ui| {
        ui.checkbox(&mut app.save_state_on_exit, "");
    });
    ui.label("When enabled, the current plots and selected plot are restored on next launch.");
}

fn show_input_settings(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Input");
    ui.separator();
    ui.add_space(8.0);

    field_row(ui, "Invert scroll", |ui| {
        ui.checkbox(&mut app.invert_scroll, "");
    });
    ui.label("Reverses mouse-wheel zoom direction in the viewport.");
}

fn colormap_presets() -> [BuiltinColourmap; 10] {
    [
        BuiltinColourmap::Viridis,
        BuiltinColourmap::Plasma,
        BuiltinColourmap::Greyscale,
        BuiltinColourmap::Coolwarm,
        BuiltinColourmap::Rainbow,
        BuiltinColourmap::Magma,
        BuiltinColourmap::Inferno,
        BuiltinColourmap::Turbo,
        BuiltinColourmap::Jet,
        BuiltinColourmap::RdBu,
    ]
}

fn colormap_name(preset: BuiltinColourmap) -> &'static str {
    match preset {
        BuiltinColourmap::Viridis => "Viridis",
        BuiltinColourmap::Plasma => "Plasma",
        BuiltinColourmap::Greyscale => "Greyscale",
        BuiltinColourmap::Coolwarm => "Coolwarm",
        BuiltinColourmap::Rainbow => "Rainbow",
        BuiltinColourmap::Magma => "Magma",
        BuiltinColourmap::Inferno => "Inferno",
        BuiltinColourmap::Turbo => "Turbo",
        BuiltinColourmap::Jet => "Jet",
        BuiltinColourmap::RdBu => "RdBu",
    }
}
