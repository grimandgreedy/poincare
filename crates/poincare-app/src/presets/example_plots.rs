use poincare_lib::{
    ColormapSource, ColourMode, Domain, MatcapSource, ParamVisSettings, PlotStyle, Resolution,
    ShadingMode, SurfaceFaceQuantity, TransferFunction,
};
use viewport_lib::{AttributeKind, BuiltinColourmap, BuiltinMatcap, ParamVisMode};

use crate::plot::entry::PlotEntry;
use crate::plot::kind::{PlotKind, evenly_spaced_isovalues};

#[derive(Clone, Copy)]
pub(crate) enum ExamplePlot {
    ContouredSurface,
    SphericalHarmonic,
    HelixCurve,
    ScatterCloud,
    VectorField,
    GridSurface,
    Streamlines,
    VolumeRender,
    Isosurface,
}

impl ExamplePlot {
    pub(crate) fn all() -> &'static [Self] {
        &[
            Self::ContouredSurface,
            Self::SphericalHarmonic,
            Self::HelixCurve,
            Self::ScatterCloud,
            Self::VectorField,
            Self::GridSurface,
            Self::Streamlines,
            Self::VolumeRender,
            Self::Isosurface,
        ]
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ContouredSurface => "Contoured Surface",
            Self::SphericalHarmonic => "Spherical Harmonic",
            Self::HelixCurve => "Helix Curve",
            Self::ScatterCloud => "Scatter Cloud",
            Self::VectorField => "Vector Field",
            Self::GridSurface => "Grid Surface",
            Self::Streamlines => "Streamlines (ABC Flow)",
            Self::VolumeRender => "Volume Render (Gaussian)",
            Self::Isosurface => "Isosurface (Concentric Spheres)",
        }
    }

    pub(crate) fn build(self) -> PlotEntry {
        match self {
            Self::ContouredSurface => PlotEntry {
                name: "Cartesian Surface".to_string(),
                visible: true,
                domain: Domain {
                    x: -8.0..=8.0,
                    y: -8.0..=8.0,
                    z: -8.0..=8.0,
                },
                resolution: Resolution { u: 100, v: 100 },
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([0.10, 0.42, 0.74, 1.0]),
                    two_sided: true,
                    ..PlotStyle::default()
                },
                kind: PlotKind::ContouredSurface {
                    contour_values: evenly_spaced_isovalues(10),
                    contour_style: PlotStyle {
                        colour_mode: ColourMode::Solid([0.05, 0.05, 0.05, 1.0]),
                        line_width: 2.0,
                        ..PlotStyle::default()
                    },
                },
            },
            Self::SphericalHarmonic => PlotEntry {
                name: "Spherical Surface".to_string(),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution { u: 80, v: 80 },
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                        scalar_range: Some((3.5, 6.5)),
                    },
                    opacity: 0.72,
                    two_sided: true,
                    shading: ShadingMode::Smooth,
                    ..PlotStyle::default()
                },
                kind: PlotKind::SphericalHarmonic,
            },
            Self::HelixCurve => PlotEntry {
                name: "Helix Curve".to_string(),
                visible: true,
                domain: Domain::default(),
                resolution: Resolution { u: 220, v: 2 },
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([1.0, 0.85, 0.2, 1.0]),
                    line_width: 3.0,
                    ..PlotStyle::default()
                },
                kind: PlotKind::HelixCurve,
            },
            Self::ScatterCloud => PlotEntry {
                name: "Scatter Cloud".to_string(),
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
                kind: PlotKind::ScatterCloud,
            },
            Self::VectorField => PlotEntry {
                name: "Vector Field".to_string(),
                visible: true,
                domain: Domain {
                    x: -6.0..=6.0,
                    y: -6.0..=6.0,
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
                kind: PlotKind::VectorField,
            },
            Self::GridSurface => PlotEntry {
                name: "Grid Surface".to_string(),
                visible: true,
                domain: Domain {
                    x: -5.0..=5.0,
                    y: -5.0..=5.0,
                    z: -5.0..=5.0,
                },
                resolution: Resolution { u: 36, v: 36 },
                style: PlotStyle {
                    colour_mode: ColourMode::Colormap {
                        colormap: ColormapSource::Builtin(BuiltinColourmap::Coolwarm),
                        scalar_range: None,
                    },
                    two_sided: true,
                    shading: ShadingMode::Flat,
                    matcap: Some(MatcapSource::Builtin(BuiltinMatcap::Clay)),
                    param_vis: Some(ParamVisSettings {
                        mode: ParamVisMode::Grid,
                        scale: 10.0,
                    }),
                    face_quantity: Some(SurfaceFaceQuantity::AreaDistortion),
                    ..PlotStyle::default()
                },
                kind: PlotKind::GridSurface,
            },
            Self::Streamlines => {
                let mut seeds = Vec::new();
                for iy in 0..3 {
                    for ix in 0..3 {
                        let x = -2.0 + ix as f32 * 2.0;
                        let y = -2.0 + iy as f32 * 2.0;
                        seeds.push(glam::Vec3::new(x, y, 0.0));
                    }
                }
                PlotEntry {
                    name: "ABC Flow Streamlines".to_string(),
                    visible: true,
                    domain: Domain {
                        x: -4.0..=4.0,
                        y: -4.0..=4.0,
                        z: -4.0..=4.0,
                    },
                    resolution: Resolution::default(),
                    style: PlotStyle {
                        colour_mode: ColourMode::Colormap {
                            colormap: ColormapSource::Builtin(BuiltinColourmap::Plasma),
                            scalar_range: None,
                        },
                        tube_radius: Some(0.08),
                        ..PlotStyle::default()
                    },
                    kind: PlotKind::Streamlines { seeds },
                }
            }
            Self::VolumeRender => PlotEntry {
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
                kind: PlotKind::VolumeRender {
                    resolution: [64, 64, 64],
                },
            },
            Self::Isosurface => PlotEntry {
                name: "Concentric Spheres".to_string(),
                visible: true,
                domain: Domain {
                    x: -4.0..=4.0,
                    y: -4.0..=4.0,
                    z: -4.0..=4.0,
                },
                resolution: Resolution::default(),
                style: PlotStyle::default(),
                kind: PlotKind::Isosurface {
                    isovalues: vec![1.0, 4.0, 9.0],
                    resolution: [64, 64, 64],
                },
            },
        }
    }
}
