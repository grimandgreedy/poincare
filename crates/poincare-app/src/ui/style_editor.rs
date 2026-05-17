use eframe::egui;
use poincare_lib::{
    ColormapSource, ColourMode, GlyphType, MatcapSource, ParamVisSettings, PlotStyle,
    ShadingMode,
    SurfaceFaceQuantity, SurfaceLicSettings, SurfaceLicVectorField,
};
use viewport_lib::{AttributeKind, BuiltinColourmap, BuiltinMatcap, ParamVisMode};

use crate::plot::kind::StyleCaps;

const LIC_SHOWCASE_BASE_COLOUR: [f32; 4] = [0.35, 0.55, 0.75, 1.0];

pub(crate) fn edit_plot_style_basic(
    ui: &mut egui::Ui,
    style: &mut PlotStyle,
    caps: StyleCaps,
) -> bool {
    let mut changed = false;
    changed |= edit_colour_mode(ui, &mut style.colour_mode, caps);
    changed |= ui
        .add(egui::Slider::new(&mut style.opacity, 0.05..=1.0).text("Opacity"))
        .changed();

    if caps.line {
        changed |= ui
            .add(egui::Slider::new(&mut style.line_width, 1.0..=8.0).text("Line Width"))
            .changed();
    }
    if caps.point {
        changed |= ui
            .add(egui::Slider::new(&mut style.point_size, 1.0..=16.0).text("Point Size"))
            .changed();
    }
    if caps.glyph {
        changed |= ui
            .add(egui::Slider::new(&mut style.glyph_scale, 0.1..=3.0).text("Glyph Scale"))
            .changed();
        let mut glyph_type = style.glyph_type;
        egui::ComboBox::from_label("Glyph Shape")
            .selected_text(match glyph_type {
                GlyphType::Arrow => "Arrow",
                GlyphType::Sphere => "Sphere",
                GlyphType::Cube => "Cube",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut glyph_type, GlyphType::Arrow, "Arrow");
                ui.selectable_value(&mut glyph_type, GlyphType::Sphere, "Sphere");
                ui.selectable_value(&mut glyph_type, GlyphType::Cube, "Cube");
            });
        if glyph_type != style.glyph_type {
            style.glyph_type = glyph_type;
            changed = true;
        }
    }

    changed
}

pub(crate) fn edit_plot_surface_settings(
    ui: &mut egui::Ui,
    style: &mut PlotStyle,
    caps: StyleCaps,
) -> bool {
    if !caps.mesh {
        ui.label(
            egui::RichText::new("No surface-specific settings for this plot type.")
                .weak()
                .small(),
        );
        return false;
    }

    let mut changed = false;
    changed |= ui.checkbox(&mut style.two_sided, "Two-sided").changed();

    let mut shading_mode = style.matcap;
    egui::ComboBox::from_label("Surface Look")
        .selected_text(surface_look_label(style.matcap))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut shading_mode, None, "Standard");
            for preset in MATCAP_PRESETS {
                ui.selectable_value(
                    &mut shading_mode,
                    Some(MatcapSource::Builtin(preset)),
                    matcap_label(preset),
                );
            }
        });
    if shading_mode != style.matcap {
        style.matcap = shading_mode;
        changed = true;
    }

    let mut shading = style.shading;
    egui::ComboBox::from_label("Shading")
        .selected_text(match shading {
            ShadingMode::Flat => "Flat",
            ShadingMode::Smooth => "Smooth",
            ShadingMode::Unlit => "Unlit",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut shading, ShadingMode::Smooth, "Smooth");
            ui.selectable_value(&mut shading, ShadingMode::Flat, "Flat");
            ui.selectable_value(&mut shading, ShadingMode::Unlit, "Unlit");
        });
    if shading != style.shading {
        style.shading = shading;
        changed = true;
    }

    ui.separator();
    ui.label("UV Visualization");
    changed |= edit_param_vis(ui, &mut style.param_vis);

    ui.separator();
    ui.label("Surface Scalar");
    changed |= edit_surface_quantity(ui, style);

    ui.separator();
    ui.label("Surface LIC");
    changed |= edit_surface_lic(ui, &mut style.surface_lic);

    changed
}

pub(crate) fn edit_colour_mode(
    ui: &mut egui::Ui,
    colour_mode: &mut ColourMode,
    caps: StyleCaps,
) -> bool {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Solid,
        Colormap,
        Attribute,
    }

    let mut changed = false;
    let mut mode = match colour_mode {
        ColourMode::Solid(_) => Mode::Solid,
        ColourMode::Colormap { .. } => Mode::Colormap,
        ColourMode::ByAttribute { .. } => Mode::Attribute,
    };

    egui::ComboBox::from_label("Colour")
        .selected_text(match mode {
            Mode::Solid => "Solid",
            Mode::Colormap => "Colormap",
            Mode::Attribute => "Attribute",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut mode, Mode::Solid, "Solid");
            ui.selectable_value(&mut mode, Mode::Colormap, "Colormap");
            ui.selectable_value(&mut mode, Mode::Attribute, "Attribute");
        });

    if !matches!(
        (mode, &*colour_mode),
        (Mode::Solid, ColourMode::Solid(_))
            | (Mode::Colormap, ColourMode::Colormap { .. })
            | (Mode::Attribute, ColourMode::ByAttribute { .. })
    ) {
        *colour_mode = match mode {
            Mode::Solid => ColourMode::Solid([0.55, 0.75, 1.0, 1.0]),
            Mode::Colormap => ColourMode::Colormap {
                colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                scalar_range: None,
            },
            Mode::Attribute => ColourMode::ByAttribute {
                name: default_attribute_name(caps).to_string(),
                kind: AttributeKind::Vertex,
            },
        };
        changed = true;
    }

    match colour_mode {
        ColourMode::Solid(rgba) => {
            ui.horizontal(|ui| {
                ui.label("Solid Colour");
                changed |= ui.color_edit_button_rgba_unmultiplied(rgba).changed();
            });
        }
        ColourMode::Colormap {
            colormap,
            scalar_range,
        } => {
            let mut preset = match *colormap {
                ColormapSource::Builtin(preset) => preset,
                ColormapSource::Uploaded(_) => BuiltinColourmap::Viridis,
            };
            egui::ComboBox::from_label("Preset")
                .selected_text(match preset {
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
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut preset, BuiltinColourmap::Viridis, "Viridis");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Plasma, "Plasma");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Greyscale, "Greyscale");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Coolwarm, "Coolwarm");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Rainbow, "Rainbow");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Magma, "Magma");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Inferno, "Inferno");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Turbo, "Turbo");
                    ui.selectable_value(&mut preset, BuiltinColourmap::Jet, "Jet");
                    ui.selectable_value(&mut preset, BuiltinColourmap::RdBu, "RdBu");
                });
            if *colormap != ColormapSource::Builtin(preset) {
                *colormap = ColormapSource::Builtin(preset);
                changed = true;
            }

            let mut use_explicit_range = scalar_range.is_some();
            if ui
                .checkbox(&mut use_explicit_range, "Explicit scalar range")
                .changed()
            {
                if use_explicit_range && scalar_range.is_none() {
                    *scalar_range = Some((0.0, 1.0));
                } else if !use_explicit_range {
                    *scalar_range = None;
                }
                changed = true;
            }
            if let Some((min, max)) = scalar_range.as_mut() {
                changed |= ui
                    .add(egui::DragValue::new(min).speed(0.1).prefix("min "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(max).speed(0.1).prefix("max "))
                    .changed();
                if *min > *max {
                    std::mem::swap(min, max);
                }
            }
        }
        ColourMode::ByAttribute { name, kind } => {
            let mut current_name = name.clone();
            egui::ComboBox::from_label("Attribute")
                .selected_text(current_name.clone())
                .show_ui(ui, |ui| {
                    for option in attribute_options(caps) {
                        ui.selectable_value(&mut current_name, option.to_string(), *option);
                    }
                });
            if current_name != *name {
                *name = current_name;
                changed = true;
            }
            if caps.mesh {
                let mut attr_kind = *kind;
                egui::ComboBox::from_label("Domain")
                    .selected_text(match attr_kind {
                        AttributeKind::Vertex => "Vertex",
                        AttributeKind::Face => "Face",
                        AttributeKind::Cell => "Cell",
                        AttributeKind::FaceColour => "Face Color",
                        _ => "Other",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut attr_kind, AttributeKind::Vertex, "Vertex");
                        ui.selectable_value(&mut attr_kind, AttributeKind::Face, "Face");
                    });
                if attr_kind != *kind {
                    *kind = attr_kind;
                    changed = true;
                }
            } else if *kind != AttributeKind::Vertex {
                *kind = AttributeKind::Vertex;
                changed = true;
            }
        }
    }

    changed
}

fn edit_param_vis(ui: &mut egui::Ui, param_vis: &mut Option<ParamVisSettings>) -> bool {
    let mut changed = false;
    let mut enabled = param_vis.is_some();
    if ui.checkbox(&mut enabled, "Enable UV Pattern").changed() {
        if enabled {
            *param_vis = Some(ParamVisSettings::default());
        } else {
            *param_vis = None;
        }
        changed = true;
    }

    if let Some(settings) = param_vis.as_mut() {
        let mut mode = settings.mode;
        egui::ComboBox::from_label("Mode")
            .selected_text(param_vis_label(mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mode, ParamVisMode::Checker, "Checker");
                ui.selectable_value(&mut mode, ParamVisMode::Grid, "Grid");
                ui.selectable_value(&mut mode, ParamVisMode::LocalChecker, "Local Checker");
                ui.selectable_value(&mut mode, ParamVisMode::LocalRadial, "Local Radial");
            });
        if mode != settings.mode {
            settings.mode = mode;
            changed = true;
        }
        changed |= ui
            .add(egui::Slider::new(&mut settings.scale, 1.0..=32.0).text("Scale"))
            .changed();
    }

    changed
}

fn edit_surface_lic(ui: &mut egui::Ui, surface_lic: &mut Option<SurfaceLicSettings>) -> bool {
    let mut changed = false;
    let mut enabled = surface_lic.is_some();
    if ui.checkbox(&mut enabled, "Enable LIC").changed() {
        if enabled {
            *surface_lic = Some(SurfaceLicSettings::default());
        } else {
            *surface_lic = None;
        }
        changed = true;
    }

    if let Some(lic) = surface_lic.as_mut() {
        let mut field = lic.vector_field;
        egui::ComboBox::from_label("Flow Direction")
            .selected_text(match field {
                SurfaceLicVectorField::TangentU => "Tangent U",
                SurfaceLicVectorField::TangentV => "Tangent V",
                SurfaceLicVectorField::Diagonal => "Diagonal (U+V)",
                SurfaceLicVectorField::Saddle => "Saddle (U−V)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut field, SurfaceLicVectorField::TangentU, "Tangent U");
                ui.selectable_value(&mut field, SurfaceLicVectorField::TangentV, "Tangent V");
                ui.selectable_value(
                    &mut field,
                    SurfaceLicVectorField::Diagonal,
                    "Diagonal (U+V)",
                );
                ui.selectable_value(&mut field, SurfaceLicVectorField::Saddle, "Saddle (U−V)");
            });
        if field != lic.vector_field {
            lic.vector_field = field;
            changed = true;
        }
        changed |= ui
            .add(egui::Slider::new(&mut lic.steps, 4..=64).text("Steps"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut lic.step_size, 0.25..=4.0).text("Step Size"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut lic.strength, 0.0..=3.0).text("Strength"))
            .changed();
    }

    changed
}

pub(crate) fn align_surface_colour_for_lic(style: &mut PlotStyle) -> bool {
    if style.surface_lic.is_none() {
        return false;
    }
    if matches!(style.colour_mode, ColourMode::Solid(_)) {
        return false;
    }
    style.colour_mode = ColourMode::Solid(LIC_SHOWCASE_BASE_COLOUR);
    true
}

fn edit_surface_quantity(ui: &mut egui::Ui, style: &mut PlotStyle) -> bool {
    let mut changed = false;
    let mut selected = style.face_quantity;
    egui::ComboBox::from_label("Quantity")
        .selected_text(match selected {
            None => "Value",
            Some(SurfaceFaceQuantity::AngleDistortion) => "Angle Distortion",
            Some(SurfaceFaceQuantity::AreaDistortion) => "Area Distortion",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, None, "Value");
            ui.selectable_value(
                &mut selected,
                Some(SurfaceFaceQuantity::AngleDistortion),
                "Angle Distortion",
            );
            ui.selectable_value(
                &mut selected,
                Some(SurfaceFaceQuantity::AreaDistortion),
                "Area Distortion",
            );
        });
    if selected != style.face_quantity {
        style.face_quantity = selected;
        changed = true;
    }

    if style.face_quantity.is_some() {
        match &mut style.colour_mode {
            ColourMode::Solid(_) => {
                style.colour_mode = ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                };
                changed = true;
            }
            ColourMode::ByAttribute { name, kind } => {
                *name = style
                    .face_quantity
                    .expect("face_quantity checked above")
                    .attribute_name()
                    .to_string();
                *kind = AttributeKind::Face;
                changed = true;
            }
            ColourMode::Colormap { .. } => {}
        }
    }

    changed
}

fn default_attribute_name(caps: StyleCaps) -> &'static str {
    if caps.glyph { "magnitude" } else { "z" }
}

fn attribute_options(caps: StyleCaps) -> &'static [&'static str] {
    if caps.mesh {
        &[
            "x",
            "y",
            "z",
            "radius",
            "value",
            "angle_distortion",
            "area_distortion",
        ]
    } else if caps.glyph {
        &["x", "y", "z", "radius", "magnitude", "index"]
    } else if caps.point || caps.line {
        &["x", "y", "z", "radius", "scalar", "value", "index"]
    } else {
        &["z"]
    }
}

fn surface_look_label(matcap: Option<MatcapSource>) -> &'static str {
    match matcap {
        None => "Standard",
        Some(MatcapSource::Builtin(preset)) => matcap_label(preset),
    }
}

fn matcap_label(preset: BuiltinMatcap) -> &'static str {
    match preset {
        BuiltinMatcap::Clay => "Matcap: Clay",
        BuiltinMatcap::Wax => "Matcap: Wax",
        BuiltinMatcap::Candy => "Matcap: Candy",
        BuiltinMatcap::Flat => "Matcap: Flat",
        BuiltinMatcap::Ceramic => "Matcap: Ceramic",
        BuiltinMatcap::Jade => "Matcap: Jade",
        BuiltinMatcap::Mud => "Matcap: Mud",
        BuiltinMatcap::Normal => "Matcap: Normal",
    }
}

fn param_vis_label(mode: ParamVisMode) -> &'static str {
    match mode {
        ParamVisMode::Checker => "Checker",
        ParamVisMode::Grid => "Grid",
        ParamVisMode::LocalChecker => "Local Checker",
        ParamVisMode::LocalRadial => "Local Radial",
    }
}

const MATCAP_PRESETS: [BuiltinMatcap; 8] = [
    BuiltinMatcap::Clay,
    BuiltinMatcap::Wax,
    BuiltinMatcap::Candy,
    BuiltinMatcap::Flat,
    BuiltinMatcap::Ceramic,
    BuiltinMatcap::Jade,
    BuiltinMatcap::Mud,
    BuiltinMatcap::Normal,
];
