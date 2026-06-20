use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, Domain, GraphSpec, PlotDefinition, PlotSpec, PlotStyle,
    Resolution, ShadingMode,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use viewport_lib::BuiltinColourmap;

use crate::{fmt_duration, mobile_log};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum UiCommand {
    ToggleMenu,
    CloseMenu,
    ToggleEditor,
    OpenEditor,
    CloseEditor,
    SetEquation(String),
    SubmitEquation,
    OpenSettings,
    CloseSettings,
    SetShowGrid(bool),
    SetShowGround(bool),
    SelectPlot(usize),
    OpenPlotProperties(usize),
    DeletePlot(usize),
    ClosePlotProperties,
    SetSelectedPlotEquation(String),
    SetSelectedPlotDomain(PlotDomainEdit),
    SetSelectedPlotResolution(PlotResolutionEdit),
    SetSelectedPlotOpacity(f32),
    SetSelectedPlotTwoSided(bool),
    SetSelectedPlotShading(MobileShadingMode),
    SetSelectedPlotColourMode(MobileColourMode),
    SetSelectedPlotSolidColour(MobileSolidColour),
    SetSelectedPlotColormap(MobileColormap),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UiSnapshot {
    pub plot_count: usize,
    pub plots: Vec<PlotListItem>,
    pub selected_plot_index: Option<usize>,
    pub sidebar_open: bool,
    pub editor_open: bool,
    pub settings_open: bool,
    pub plot_properties_open: bool,
    pub show_grid: bool,
    pub show_ground: bool,
    pub equation: String,
    pub selected_plot: Option<PlotEditorSnapshot>,
    pub scene_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PlotListItem {
    pub index: usize,
    pub name: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PlotEditorSnapshot {
    pub index: usize,
    pub name: String,
    pub equation: String,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub z_min: f64,
    pub z_max: f64,
    pub resolution_u: u32,
    pub resolution_v: u32,
    pub opacity: f32,
    pub two_sided: bool,
    pub shading: MobileShadingMode,
    pub colour_mode: MobileColourMode,
    pub solid_colour: MobileSolidColour,
    pub colormap: MobileColormap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum PlotAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct PlotDomainEdit {
    pub axis: PlotAxis,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlotResolutionEdit {
    pub u: u32,
    pub v: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum MobileShadingMode {
    Smooth,
    Flat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum MobileColourMode {
    Solid,
    Colormap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum MobileSolidColour {
    Red,
    Green,
    Blue,
    Yellow,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum MobileColormap {
    Rainbow,
    Viridis,
    Plasma,
    Coolwarm,
    Turbo,
    Jet,
    Greyscale,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiEffects {
    pub plot_changed: bool,
    pub redraw_requested: bool,
}

#[derive(Clone, Debug)]
pub struct MobileModel {
    plots: Vec<PlotSpec>,
    selected_plot: Option<usize>,
    scene_error: Option<String>,
    sidebar_open: bool,
    editor_open: bool,
    settings_open: bool,
    plot_properties_open: bool,
    show_grid: bool,
    show_ground: bool,
    equation: String,
}

impl MobileModel {
    pub fn new() -> Self {
        Self {
            plots: Vec::new(),
            selected_plot: None,
            scene_error: None,
            sidebar_open: false,
            editor_open: false,
            settings_open: false,
            plot_properties_open: false,
            show_grid: false,
            show_ground: false,
            equation: "sin(x)*cos(y)".to_string(),
        }
    }

    pub fn plots(&self) -> Vec<PlotSpec> {
        self.plots.clone()
    }

    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    pub fn show_ground(&self) -> bool {
        self.show_ground
    }

    pub fn set_scene_error(&mut self, error: impl Into<String>) {
        self.scene_error = Some(error.into());
    }

    pub fn clear_scene_error(&mut self) {
        self.scene_error = None;
    }

    pub fn snapshot(&self) -> UiSnapshot {
        UiSnapshot {
            plot_count: self.plots.len(),
            plots: self
                .plots
                .iter()
                .enumerate()
                .map(|(index, plot)| PlotListItem {
                    index,
                    name: plot.name.clone(),
                    selected: Some(index) == self.selected_plot,
                })
                .collect(),
            selected_plot_index: self.selected_plot,
            sidebar_open: self.sidebar_open,
            editor_open: self.editor_open,
            settings_open: self.settings_open,
            plot_properties_open: self.plot_properties_open,
            show_grid: self.show_grid,
            show_ground: self.show_ground,
            equation: self.equation.clone(),
            selected_plot: self.selected_plot.and_then(|idx| {
                self.plots
                    .get(idx)
                    .map(|plot| plot_editor_snapshot(idx, plot))
            }),
            scene_error: self.scene_error.clone(),
        }
    }

    pub fn apply_commands(&mut self, commands: impl IntoIterator<Item = UiCommand>) -> UiEffects {
        let mut effects = UiEffects::default();
        for command in commands {
            effects = effects.merge(self.apply_command(command));
        }
        effects
    }

    fn apply_command(&mut self, command: UiCommand) -> UiEffects {
        match command {
            UiCommand::ToggleMenu => {
                self.sidebar_open = !self.sidebar_open;
                UiEffects::redraw()
            }
            UiCommand::CloseMenu => {
                self.sidebar_open = false;
                UiEffects::redraw()
            }
            UiCommand::ToggleEditor => {
                self.editor_open = !self.editor_open;
                UiEffects::redraw()
            }
            UiCommand::OpenEditor => {
                self.editor_open = true;
                UiEffects::redraw()
            }
            UiCommand::CloseEditor => {
                self.editor_open = false;
                UiEffects::redraw()
            }
            UiCommand::SetEquation(equation) => {
                self.equation = equation;
                UiEffects::redraw()
            }
            UiCommand::SubmitEquation => {
                let equation = self.equation.trim().to_string();
                if equation.is_empty() {
                    self.scene_error = Some("Enter an equation before plotting.".to_string());
                    return UiEffects::redraw();
                }

                self.add_equation_plot(equation)
            }
            UiCommand::OpenSettings => {
                self.settings_open = true;
                self.sidebar_open = false;
                UiEffects::redraw()
            }
            UiCommand::CloseSettings => {
                self.settings_open = false;
                UiEffects::redraw()
            }
            UiCommand::SetShowGrid(show) => {
                self.show_grid = show;
                UiEffects::redraw()
            }
            UiCommand::SetShowGround(show) => {
                self.show_ground = show;
                UiEffects::redraw()
            }
            UiCommand::SelectPlot(idx) => {
                if idx < self.plots.len() {
                    self.selected_plot = Some(idx);
                }
                UiEffects::redraw()
            }
            UiCommand::OpenPlotProperties(idx) => {
                if idx < self.plots.len() {
                    self.selected_plot = Some(idx);
                    self.plot_properties_open = true;
                    self.sidebar_open = false;
                }
                UiEffects::redraw()
            }
            UiCommand::DeletePlot(idx) => self.delete_plot(idx),
            UiCommand::ClosePlotProperties => {
                self.plot_properties_open = false;
                UiEffects::redraw()
            }
            UiCommand::SetSelectedPlotEquation(equation) => self.update_selected_plot(|plot| {
                if let PlotDefinition::ExprCartesian { expression, .. } = &mut plot.definition {
                    *expression = equation.trim().to_string();
                    plot.name = format!("z = {expression}");
                    Ok(())
                } else {
                    Err("This plot type does not expose a mobile equation editor.".to_string())
                }
            }),
            UiCommand::SetSelectedPlotDomain(edit) => self.update_selected_plot(|plot| {
                if edit.min >= edit.max {
                    return Err("Domain minimum must be less than maximum.".to_string());
                }

                match edit.axis {
                    PlotAxis::X => plot.domain.x = edit.min..=edit.max,
                    PlotAxis::Y => plot.domain.y = edit.min..=edit.max,
                    PlotAxis::Z => plot.domain.z = edit.min..=edit.max,
                }
                Ok(())
            }),
            UiCommand::SetSelectedPlotResolution(edit) => self.update_selected_plot(|plot| {
                plot.resolution = Resolution {
                    u: edit.u.clamp(8, 256),
                    v: edit.v.clamp(8, 256),
                };
                Ok(())
            }),
            UiCommand::SetSelectedPlotOpacity(opacity) => self.update_selected_plot(|plot| {
                plot.style.opacity = opacity.clamp(0.05, 1.0);
                Ok(())
            }),
            UiCommand::SetSelectedPlotTwoSided(two_sided) => self.update_selected_plot(|plot| {
                plot.style.two_sided = two_sided;
                Ok(())
            }),
            UiCommand::SetSelectedPlotShading(shading) => self.update_selected_plot(|plot| {
                plot.style.shading = match shading {
                    MobileShadingMode::Smooth => ShadingMode::Smooth,
                    MobileShadingMode::Flat => ShadingMode::Flat,
                };
                Ok(())
            }),
            UiCommand::SetSelectedPlotColourMode(mode) => self.update_selected_plot(|plot| {
                plot.style.colour_mode = match mode {
                    MobileColourMode::Solid => {
                        ColourMode::Solid(mobile_solid_colour_rgba(MobileSolidColour::Red))
                    }
                    MobileColourMode::Colormap => ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColourmap::Rainbow),
                        scalar_range: None,
                    },
                };
                Ok(())
            }),
            UiCommand::SetSelectedPlotSolidColour(colour) => self.update_selected_plot(|plot| {
                plot.style.colour_mode = ColourMode::Solid(mobile_solid_colour_rgba(colour));
                Ok(())
            }),
            UiCommand::SetSelectedPlotColormap(colormap) => self.update_selected_plot(|plot| {
                plot.style.colour_mode = ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(mobile_colormap_builtin(colormap)),
                    scalar_range: None,
                };
                Ok(())
            }),
        }
    }

    fn add_equation_plot(&mut self, equation: String) -> UiEffects {
        let plot = cartesian_plot(equation);
        let mut plots = self.plots.clone();
        plots.push(plot.clone());

        let spec = GraphSpec {
            axis_config: AxisConfig::default(),
            plots,
        };

        let validate_start = Instant::now();
        mobile_log(format_args!(
            "model add_equation_plot validate start plots={} resolution={}x{}",
            spec.plots.len(),
            plot.resolution.u,
            plot.resolution.v,
        ));
        match spec.build_scene() {
            Ok(_) => {
                mobile_log(format_args!(
                    "model add_equation_plot validate ok elapsed={}",
                    fmt_duration(validate_start.elapsed()),
                ));
                self.plots.push(plot);
                self.selected_plot = Some(self.plots.len() - 1);
                self.scene_error = None;
                self.editor_open = false;
                UiEffects {
                    plot_changed: true,
                    redraw_requested: true,
                }
            }
            Err(err) => {
                mobile_log(format_args!(
                    "model add_equation_plot validate failed elapsed={} err={err}",
                    fmt_duration(validate_start.elapsed()),
                ));
                self.scene_error = Some(err.to_string());
                UiEffects::redraw()
            }
        }
    }

    fn delete_plot(&mut self, idx: usize) -> UiEffects {
        if idx >= self.plots.len() {
            return UiEffects::redraw();
        }

        self.plots.remove(idx);
        self.scene_error = None;

        self.selected_plot = match self.selected_plot {
            Some(selected) if selected == idx => None,
            Some(selected) if selected > idx => Some(selected - 1),
            selected => selected,
        };
        if self.selected_plot.is_none() {
            self.plot_properties_open = false;
        }

        UiEffects {
            plot_changed: true,
            redraw_requested: true,
        }
    }

    fn update_selected_plot(
        &mut self,
        update: impl FnOnce(&mut PlotSpec) -> Result<(), String>,
    ) -> UiEffects {
        let Some(idx) = self.selected_plot else {
            self.scene_error = Some("Select a plot first.".to_string());
            return UiEffects::redraw();
        };
        if idx >= self.plots.len() {
            self.selected_plot = None;
            self.scene_error = Some("Selected plot no longer exists.".to_string());
            return UiEffects::redraw();
        }

        let mut plots = self.plots.clone();
        if let Err(err) = update(&mut plots[idx]) {
            self.scene_error = Some(err);
            return UiEffects::redraw();
        }

        let spec = GraphSpec {
            axis_config: AxisConfig::default(),
            plots,
        };

        let validate_start = Instant::now();
        mobile_log(format_args!(
            "model update_selected_plot validate start plots={}",
            spec.plots.len(),
        ));
        match spec.build_scene() {
            Ok(_) => {
                mobile_log(format_args!(
                    "model update_selected_plot validate ok elapsed={}",
                    fmt_duration(validate_start.elapsed()),
                ));
                self.plots = spec.plots;
                self.scene_error = None;
                UiEffects {
                    plot_changed: true,
                    redraw_requested: true,
                }
            }
            Err(err) => {
                mobile_log(format_args!(
                    "model update_selected_plot validate failed elapsed={} err={err}",
                    fmt_duration(validate_start.elapsed()),
                ));
                self.scene_error = Some(err.to_string());
                UiEffects::redraw()
            }
        }
    }
}

impl Default for MobileModel {
    fn default() -> Self {
        Self::new()
    }
}

impl UiEffects {
    fn redraw() -> Self {
        Self {
            plot_changed: false,
            redraw_requested: true,
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            plot_changed: self.plot_changed || other.plot_changed,
            redraw_requested: self.redraw_requested || other.redraw_requested,
        }
    }
}

fn cartesian_plot(equation: String) -> PlotSpec {
    PlotSpec {
        name: format!("z = {equation}"),
        visible: true,
        domain: Domain {
            x: -5.0..=5.0,
            y: -5.0..=5.0,
            z: -5.0..=5.0,
        },
        resolution: Resolution { u: 80, v: 80 },
        style: PlotStyle {
            colour_mode: ColourMode::Colormap {
                colormap: ColormapSource::Builtin(BuiltinColourmap::Rainbow),
                scalar_range: None,
            },
            two_sided: true,
            ..PlotStyle::default()
        },
        definition: PlotDefinition::ExprCartesian {
            expression: equation,
            parameters: Vec::new(),
        },
    }
}

fn plot_editor_snapshot(index: usize, plot: &PlotSpec) -> PlotEditorSnapshot {
    let equation = match &plot.definition {
        PlotDefinition::ExprCartesian { expression, .. } => expression.clone(),
        _ => String::new(),
    };

    PlotEditorSnapshot {
        index,
        name: plot.name.clone(),
        equation,
        x_min: *plot.domain.x.start(),
        x_max: *plot.domain.x.end(),
        y_min: *plot.domain.y.start(),
        y_max: *plot.domain.y.end(),
        z_min: *plot.domain.z.start(),
        z_max: *plot.domain.z.end(),
        resolution_u: plot.resolution.u,
        resolution_v: plot.resolution.v,
        opacity: plot.style.opacity,
        two_sided: plot.style.two_sided,
        shading: match plot.style.shading {
            ShadingMode::Smooth => MobileShadingMode::Smooth,
            ShadingMode::Flat => MobileShadingMode::Flat,
            ShadingMode::Unlit => MobileShadingMode::Smooth,
        },
        colour_mode: mobile_colour_mode(&plot.style.colour_mode),
        solid_colour: mobile_solid_colour(&plot.style.colour_mode),
        colormap: mobile_colormap(&plot.style.colour_mode),
    }
}

fn mobile_colour_mode(colour_mode: &ColourMode) -> MobileColourMode {
    match colour_mode {
        ColourMode::Solid(_) => MobileColourMode::Solid,
        ColourMode::Colormap { .. } | ColourMode::ByAttribute { .. } => MobileColourMode::Colormap,
    }
}

fn mobile_solid_colour(colour_mode: &ColourMode) -> MobileSolidColour {
    let ColourMode::Solid(colour) = colour_mode else {
        return MobileSolidColour::Red;
    };

    const OPTIONS: [MobileSolidColour; 5] = [
        MobileSolidColour::Red,
        MobileSolidColour::Green,
        MobileSolidColour::Blue,
        MobileSolidColour::Yellow,
        MobileSolidColour::White,
    ];

    OPTIONS
        .into_iter()
        .min_by(|a, b| {
            colour_distance(*colour, mobile_solid_colour_rgba(*a))
                .total_cmp(&colour_distance(*colour, mobile_solid_colour_rgba(*b)))
        })
        .unwrap_or(MobileSolidColour::Red)
}

fn mobile_colormap(colour_mode: &ColourMode) -> MobileColormap {
    match colour_mode {
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
            ..
        } => MobileColormap::Viridis,
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
            ..
        } => MobileColormap::Plasma,
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
            ..
        } => MobileColormap::Coolwarm,
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Turbo),
            ..
        } => MobileColormap::Turbo,
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Jet),
            ..
        } => MobileColormap::Jet,
        ColourMode::Colormap {
            colormap: ColormapSource::Builtin(BuiltinColourmap::Greyscale),
            ..
        } => MobileColormap::Greyscale,
        _ => MobileColormap::Rainbow,
    }
}

fn mobile_colormap_builtin(colormap: MobileColormap) -> BuiltinColourmap {
    match colormap {
        MobileColormap::Rainbow => BuiltinColourmap::Rainbow,
        MobileColormap::Viridis => BuiltinColourmap::Viridis,
        MobileColormap::Plasma => BuiltinColourmap::Plasma,
        MobileColormap::Coolwarm => BuiltinColourmap::Coolwarm,
        MobileColormap::Turbo => BuiltinColourmap::Turbo,
        MobileColormap::Jet => BuiltinColourmap::Jet,
        MobileColormap::Greyscale => BuiltinColourmap::Greyscale,
    }
}

fn mobile_solid_colour_rgba(colour: MobileSolidColour) -> [f32; 4] {
    match colour {
        MobileSolidColour::Red => [1.0, 0.15, 0.12, 1.0],
        MobileSolidColour::Green => [0.1, 0.8, 0.35, 1.0],
        MobileSolidColour::Blue => [0.15, 0.45, 1.0, 1.0],
        MobileSolidColour::Yellow => [1.0, 0.85, 0.15, 1.0],
        MobileSolidColour::White => [0.92, 0.94, 0.98, 1.0],
    }
}

fn colour_distance(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}
