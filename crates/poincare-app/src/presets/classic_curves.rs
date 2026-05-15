use std::f64::consts::PI;

use poincare_lib::{ColormapSource, ColourMode, Domain, PlotStyle, Resolution};
use viewport_lib::BuiltinColourmap;

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            name: "Trefoil Knot".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 400, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
                    scalar_range: None,
                },
                line_width: 3.0,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "((2+cos(3*t/2))*cos(t), (2+cos(3*t/2))*sin(t), sin(3*t/2))"
                    .to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 4.0 * PI),
            },
        },
        PlotEntry {
            name: "Torus Knot (3,5)".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 500, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                line_width: 2.5,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "((2+cos(5*t))*cos(3*t), (2+cos(5*t))*sin(3*t), sin(5*t))".to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 2.0 * PI),
            },
        },
        PlotEntry {
            name: "Lissajous 3D".to_string(),
            visible: true,
            domain: Domain {
                x: -1.5..=1.5,
                y: -1.5..=1.5,
                z: -1.5..=1.5,
            },
            resolution: Resolution { u: 400, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.2, 0.9, 1.0, 1.0]),
                line_width: 2.5,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "(sin(3*t), sin(2*t), sin(5*t))".to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 2.0 * PI),
            },
        },
        PlotEntry {
            name: "Viviani Curve".to_string(),
            visible: true,
            domain: Domain {
                x: -2.5..=2.5,
                y: -2.5..=2.5,
                z: -2.5..=2.5,
            },
            resolution: Resolution { u: 400, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.6, 0.2, 1.0]),
                line_width: 2.5,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "(1+cos(t), sin(t), 2*sin(t/2))".to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 4.0 * PI),
            },
        },
        PlotEntry {
            name: "Logarithmic Spiral".to_string(),
            visible: true,
            domain: Domain {
                x: -8.0..=8.0,
                y: -8.0..=8.0,
                z: -8.0..=8.0,
            },
            resolution: Resolution { u: 500, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                line_width: 2.5,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "(exp(0.1*t)*cos(t), exp(0.1*t)*sin(t), t/5)".to_string(),
                parameters: Vec::new(),
                t_range: (-4.0 * PI, 4.0 * PI),
            },
        },
    ]
}
