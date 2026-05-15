use std::f64::consts::PI;

use poincare_lib::{ColormapSource, ColourMode, Domain, PlotStyle, Resolution};
use viewport_lib::BuiltinColourmap;

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            name: "Mexican Hat".to_string(),
            visible: true,
            domain: Domain {
                x: -8.0..=8.0,
                y: -8.0..=8.0,
                z: -5.0..=5.0,
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
                expression: "sin(sqrt(x^2+y^2))/sqrt(x^2+y^2)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Helix (expr)".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 300, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.2, 1.0, 0.5, 1.0]),
                line_width: 2.5,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "(cos(t), sin(t), t/5)".to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 6.0 * PI),
            },
        },
    ]
}
