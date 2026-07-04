use std::f64::consts::PI;

use poincare_lib::{ColormapSource, ColourMode, Domain, PlotStyle, Resolution};
use viewport_lib::BuiltinColourmap;

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

fn curve_entry(
    name: &str,
    expression: &str,
    t_range: (f64, f64),
    domain: Domain,
    resolution: Resolution,
    style: PlotStyle,
) -> PlotEntry {
    PlotEntry::new(
        name,
        PlotKind::ExprCurve {
            expression: expression.to_string(),
            parameters: Vec::new(),
            t_range,
        },
    )
    .with_domain(domain)
    .with_resolution(resolution)
    .with_style(style)
}

pub fn build() -> Vec<PlotEntry> {
    vec![
        curve_entry(
            "Trefoil Knot",
            "((2+cos(3*t/2))*cos(t), (2+cos(3*t/2))*sin(t), sin(3*t/2))",
            (0.0, 4.0 * PI),
            Domain::default(),
            Resolution { u: 400, v: 2 },
            PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
                    scalar_range: None,
                },
                line_width: 3.0,
                ..PlotStyle::default()
            },
        ),
        curve_entry(
            "Torus Knot (3,5)",
            "((2+cos(5*t))*cos(3*t), (2+cos(5*t))*sin(3*t), sin(5*t))",
            (0.0, 2.0 * PI),
            Domain::default(),
            Resolution { u: 500, v: 2 },
            PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                line_width: 2.5,
                ..PlotStyle::default()
            },
        ),
        curve_entry(
            "Lissajous 3D",
            "(sin(3*t), sin(2*t), sin(5*t))",
            (0.0, 2.0 * PI),
            Domain {
                x: -1.5..=1.5,
                y: -1.5..=1.5,
                z: -1.5..=1.5,
            },
            Resolution { u: 400, v: 2 },
            PlotStyle {
                colour_mode: ColourMode::Solid([0.2, 0.9, 1.0, 1.0]),
                line_width: 2.5,
                ..PlotStyle::default()
            },
        ),
        curve_entry(
            "Viviani Curve",
            "(1+cos(t), sin(t), 2*sin(t/2))",
            (0.0, 4.0 * PI),
            Domain {
                x: -2.5..=2.5,
                y: -2.5..=2.5,
                z: -2.5..=2.5,
            },
            Resolution { u: 400, v: 2 },
            PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.6, 0.2, 1.0]),
                line_width: 2.5,
                ..PlotStyle::default()
            },
        ),
        curve_entry(
            "Logarithmic Spiral",
            "(exp(0.1*t)*cos(t), exp(0.1*t)*sin(t), t/5)",
            (-4.0 * PI, 4.0 * PI),
            Domain {
                x: -8.0..=8.0,
                y: -8.0..=8.0,
                z: -8.0..=8.0,
            },
            Resolution { u: 500, v: 2 },
            PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                line_width: 2.5,
                ..PlotStyle::default()
            },
        ),
    ]
}
