use std::f64::consts::PI;

use poincare_lib::{ColourMode, Domain, MatcapSource, PlotStyle, Resolution, ShadingMode};
use viewport_lib::BuiltinMatcap;

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    vec![
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Studio Torus".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: 0.0..=(2.0 * PI),
                z: -1.4..=1.4,
            },
            resolution: Resolution { u: 100, v: 52 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.90, 0.78, 0.66, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Wax)),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(2.4+0.55*cos(v))*cos(u)|(2.4+0.55*cos(v))*sin(u)|0.55*sin(v)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            plot_id: 0,
            parent_plot_id: None,
            relationship: crate::plot::entry::PlotRelationship::Primary,
            name: "Reference Helix".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 320, v: 2 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.95, 0.95, 0.98, 1.0]),
                line_width: 2.0,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprCurve {
                expression: "(1.5*cos(t), 1.5*sin(t), -1.2 + t/(3*pi))".to_string(),
                parameters: Vec::new(),
                t_range: (0.0, 6.0 * PI),
            },
        },
    ]
}
