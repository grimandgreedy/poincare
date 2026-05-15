use poincare_lib::{ColourMode, Domain, PlotStyle, Resolution};

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            name: "Outer Sphere".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 96, v: 96 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.20, 0.62, 1.0, 1.0]),
                opacity: 0.34,
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprSpherical {
                expression: "2.6".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Inner Saddle".to_string(),
            visible: true,
            domain: Domain {
                x: -1.8..=1.8,
                y: -1.8..=1.8,
                z: -3.0..=3.0,
            },
            resolution: Resolution { u: 96, v: 96 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.55, 0.22, 1.0]),
                opacity: 0.46,
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCartesian {
                expression: "0.7*(x^2-y^2)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Concentric Iso".to_string(),
            visible: true,
            domain: Domain {
                x: -3.0..=3.0,
                y: -3.0..=3.0,
                z: -3.0..=3.0,
            },
            resolution: Resolution::default(),
            style: PlotStyle::default(),
            kind: PlotKind::ExprIsosurface {
                expression: "x^2+y^2+z^2".to_string(),
                parameters: Vec::new(),
                isovalues: vec![1.2, 3.0, 5.0],
                iso_colours: vec![
                    [0.95, 0.85, 0.25, 0.28],
                    [0.25, 0.95, 0.62, 0.22],
                    [0.95, 0.32, 0.44, 0.18],
                ],
                vol_resolution: [72, 72, 72],
            },
        },
    ]
}
