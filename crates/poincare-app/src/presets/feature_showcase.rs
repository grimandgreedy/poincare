use std::f64::consts::PI;

use poincare_lib::{
    ColormapSource, ColourMode, Domain, ParamVisSettings, PlotStyle, Resolution, ShadingMode,
    SurfaceFaceQuantity,
};
use viewport_lib::{BuiltinColourmap, ParamVisMode};

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            name: "UV Grid Torus".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: 0.0..=(2.0 * PI),
                z: -1.5..=1.5,
            },
            resolution: Resolution { u: 90, v: 48 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.9, 0.9, 0.9, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                param_vis: Some(ParamVisSettings {
                    mode: ParamVisMode::Grid,
                    scale: 12.0,
                }),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(2+0.7*cos(v))*cos(u)|(2+0.7*cos(v))*sin(u)|0.7*sin(v)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Angle Distortion Mobius".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: -1.0..=1.0,
                z: -0.8..=0.8,
            },
            resolution: Resolution { u: 120, v: 26 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                face_quantity: Some(SurfaceFaceQuantity::AngleDistortion),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(1+v/2*cos(u/2))*cos(u)|(1+v/2*cos(u/2))*sin(u)|v/2*sin(u/2)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Area Distortion Dini".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(4.0 * PI),
                y: 0.12..=2.0,
                z: -2.0..=6.0,
            },
            resolution: Resolution { u: 96, v: 64 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                face_quantity: Some(SurfaceFaceQuantity::AreaDistortion),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "cos(u)*sin(v)|sin(u)*sin(v)|cos(v)+ln(tan(v/2))+0.2*u".to_string(),
                parameters: Vec::new(),
            },
        },
    ]
}
