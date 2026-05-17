use poincare_lib::{
    ColormapSource, ColourMode, Domain, PlotStyle, Resolution, ShadingMode, TransferFunction,
};
use viewport_lib::{AttributeKind, BuiltinColourmap};

use crate::plot::entry::PlotEntry;
use crate::plot::kind::PlotKind;

pub fn build() -> Vec<PlotEntry> {
    // Sample CSV scatter data: 50 pseudo-random points
    let scatter_csv: String = (0..50)
        .map(|i: usize| {
            let t = i as f32 * 0.37;
            let x = (t * 1.7).sin() * 4.0;
            let y = (t * 2.3).cos() * 4.0;
            let z = (t * 0.9).sin() * 4.0;
            format!("{:.3},{:.3},{:.3}\n", x, y, z)
        })
        .collect();

    vec![
        PlotEntry {
            name: "Spherical Harmonic (expr)".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 80, v: 80 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                    scalar_range: None,
                },
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprSpherical {
                expression: "cos(2*theta)^2 * sin(phi)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Torus (R,r params)".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution { u: 80, v: 40 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                two_sided: true,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprParametricSurface {
                expression: "(R+r*cos(v))*cos(u)|(R+r*cos(v))*sin(u)|r*sin(v)".to_string(),
                parameters: vec![("R".to_string(), 3.0), ("r".to_string(), 1.0)],
            },
        },
        PlotEntry {
            name: "CSV Scatter".to_string(),
            visible: true,
            domain: Domain::default(),
            resolution: Resolution::default(),
            style: PlotStyle {
                colour_mode: ColourMode::ByAttribute {
                    name: "z".to_string(),
                    kind: AttributeKind::Vertex,
                },
                point_size: 6.0,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprScatter {
                csv_text: scatter_csv,
                parse_error: String::new(),
            },
        },
        PlotEntry {
            name: "Vector Field (-y, x, 0)".to_string(),
            visible: true,
            domain: Domain {
                x: -5.0..=5.0,
                y: -5.0..=5.0,
                z: -4.0..=4.0,
            },
            resolution: Resolution { u: 5, v: 5 },
            style: PlotStyle {
                colour_mode: ColourMode::ByAttribute {
                    name: "magnitude".to_string(),
                    kind: AttributeKind::Vertex,
                },
                glyph_scale: 0.8,
                shading: ShadingMode::Unlit,
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprVectorField {
                expression: "-y|x|0".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotEntry {
            name: "Gaussian Volume".to_string(),
            visible: true,
            domain: Domain {
                x: -3.0..=3.0,
                y: -3.0..=3.0,
                z: -3.0..=3.0,
            },
            resolution: Resolution::default(),
            style: PlotStyle {
                opacity: 0.3,
                transfer_function: Some(TransferFunction {
                    opacity_scale: 0.4,
                    threshold: None,
                }),
                ..PlotStyle::default()
            },
            kind: PlotKind::ExprVolume {
                expression: "exp(-(x^2+y^2+z^2))".to_string(),
                parameters: Vec::new(),
                vol_resolution: [64, 64, 64],
            },
        },
        PlotEntry {
            name: "Concentric Spheres (expr)".to_string(),
            visible: true,
            domain: Domain {
                x: -4.0..=4.0,
                y: -4.0..=4.0,
                z: -4.0..=4.0,
            },
            resolution: Resolution::default(),
            style: PlotStyle::default(),
            kind: PlotKind::ExprIsosurface {
                expression: "x^2+y^2+z^2".to_string(),
                parameters: Vec::new(),
                isovalues: vec![1.0, 4.0, 9.0],
                iso_colours: vec![
                    [0.2, 0.6, 1.0, 0.5],
                    [0.2, 0.9, 0.4, 0.5],
                    [1.0, 0.4, 0.2, 0.5],
                ],
                vol_resolution: [64, 64, 64],
            },
        },
    ]
}
