use std::f64::consts::PI;

use poincare_lib::{ColourMode, Domain, MatcapSource, PlotStyle, Resolution, ShadingMode};
use viewport_lib::BuiltinMatcap;

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            name: "Clay Torus".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: 0.0..=(2.0 * PI),
                z: -1.6..=1.6,
            },
            resolution: Resolution { u: 96, v: 48 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.88, 0.63, 0.46, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Clay)),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(2.2+0.65*cos(v))*cos(u)|(2.2+0.65*cos(v))*sin(u)|0.65*sin(v)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Candy Mobius".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: -1.0..=1.0,
                z: -0.8..=0.8,
            },
            resolution: Resolution { u: 120, v: 28 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.30, 0.74, 0.98, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Candy)),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(1+v/2*cos(u/2))*cos(u)|(1+v/2*cos(u/2))*sin(u)|v/2*sin(u/2)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Jade Enneper".to_string(),
            visible: true,
            domain: Domain {
                x: -1.8..=1.8,
                y: -1.8..=1.8,
                z: -4.0..=4.0,
            },
            resolution: Resolution { u: 80, v: 80 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.7, 0.9, 0.8, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Jade)),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "u-u^3/3+u*v^2|v-v^3/3+u^2*v|u^2-v^2".to_string(),
                parameters: Vec::new(),
            },
        },
    ]
}
