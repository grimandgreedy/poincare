use std::f64::consts::PI;

use poincare_lib::{
    ColormapSource, ColourMode, Domain, MatcapSource, ParamVisSettings, PlotStyle, Resolution,
    ShadingMode,
};
use viewport_lib::{BuiltinColourmap, BuiltinMatcap, ParamVisMode};

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Torus".to_string(),
            visible: true,
            // domain.x = u range [0, 2π], domain.y = v range [0, 2π]
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: 0.0..=(2.0 * PI),
                z: -1.5..=1.5,
            },
            resolution: Resolution { u: 80, v: 40 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Clay)),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(2+cos(v))*cos(u)|(2+cos(v))*sin(u)|sin(v)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Möbius Strip".to_string(),
            visible: true,
            // u ∈ [0, 2π], v ∈ [-1, 1]
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: -1.0..=1.0,
                z: -0.5..=0.5,
            },
            resolution: Resolution { u: 100, v: 20 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.8, 0.5, 1.0, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                param_vis: Some(ParamVisSettings {
                    mode: ParamVisMode::Checker,
                    scale: 12.0,
                }),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(1+v/2*cos(u/2))*cos(u)|(1+v/2*cos(u/2))*sin(u)|v/2*sin(u/2)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Enneper Surface".to_string(),
            visible: true,
            // u, v ∈ [-2, 2]
            domain: Domain {
                x: -2.0..=2.0,
                y: -2.0..=2.0,
                z: -5.0..=5.0,
            },
            resolution: Resolution { u: 60, v: 60 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "u-u^3/3+u*v^2|v-v^3/3+u^2*v|u^2-v^2".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Monkey Saddle".to_string(),
            visible: true,
            domain: Domain {
                x: -2.0..=2.0,
                y: -2.0..=2.0,
                z: -8.0..=8.0,
            },
            resolution: Resolution { u: 80, v: 80 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
                    scalar_range: None,
                },
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCartesian {
                expression: "x^3-3*x*y^2".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Gaussian Bell".to_string(),
            visible: true,
            domain: Domain {
                x: -3.0..=3.0,
                y: -3.0..=3.0,
                z: -0.1..=1.1,
            },
            resolution: Resolution { u: 80, v: 80 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCartesian {
                expression: "exp(-(x^2+y^2))".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Dini's Surface".to_string(),
            visible: true,
            // u ∈ [0, 4π], v ∈ [0.1, 2]; avoid singularity at v = 0
            domain: Domain {
                x: 0.0..=(4.0 * PI),
                y: 0.1..=2.0,
                z: -2.0..=6.0,
            },
            resolution: Resolution { u: 80, v: 60 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "cos(u)*sin(v)|sin(u)*sin(v)|cos(v)+ln(tan(v/2))+0.2*u".to_string(),
                parameters: Vec::new(),
            },
        },
    ]
}
