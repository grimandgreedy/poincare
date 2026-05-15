use std::sync::Arc;

use viewport_lib::{extract_isolines, IsolineItem, MeshData};

use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotComponent, PlotGeometry, PlotObject};
use crate::plots::Surface3D;
use crate::resolution::Resolution;
use crate::style::{ColourMode, PlotStyle};

/// A surface with contour lines extracted at one or more isovalues.
pub struct LevelSet3D {
    surface: Arc<Surface3D>,
    isovalues: Vec<f32>,
    contour_style: PlotStyle,
}

impl LevelSet3D {
    pub fn new(surface: Arc<Surface3D>, isovalues: Vec<f32>) -> Self {
        Self {
            surface,
            isovalues,
            contour_style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.95, 0.25, 1.0]),
                line_width: 2.0,
                ..PlotStyle::default()
            },
        }
    }

    pub fn with_contour_style(mut self, contour_style: PlotStyle) -> Self {
        self.contour_style = contour_style;
        self
    }
}

impl PlotObject for LevelSet3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        self.surface.coordinate_system()
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        self.surface.natural_bounds()
    }

    fn generate(&self, domain: &Domain, resolution: Resolution) -> PlotGeometry {
        let surface_geometry = self.surface.generate(domain, resolution);
        let mut components = Vec::with_capacity(2);

        let isolines = match &surface_geometry {
            PlotGeometry::Surface(mesh) => {
                build_isoline_geometry(mesh, &self.isovalues, &self.contour_style)
            }
            _ => None,
        };

        components.push(PlotComponent {
            geometry: surface_geometry,
            style: self.surface.style().clone(),
        });

        if let Some(geometry) = isolines {
            components.push(PlotComponent {
                geometry,
                style: self.contour_style.clone(),
            });
        }

        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        self.surface.style()
    }

    fn resolution(&self) -> Resolution {
        self.surface.resolution()
    }

    fn domain_override(&self) -> Option<&Domain> {
        self.surface.domain_override()
    }
}

fn build_isoline_geometry(
    mesh: &MeshData,
    isovalues: &[f32],
    contour_style: &PlotStyle,
) -> Option<PlotGeometry> {
    let scalars = match mesh.attributes.get("value") {
        Some(viewport_lib::AttributeData::Vertex(values)) => values.clone(),
        _ => return None,
    };

    let mut isoline_item = IsolineItem::default();
    isoline_item.positions = mesh.positions.clone();
    isoline_item.indices = mesh.indices.clone();
    isoline_item.scalars = scalars;
    isoline_item.isovalues = isovalues.to_vec();
    isoline_item.color = [1.0, 1.0, 1.0, contour_style.opacity.clamp(0.0, 1.0)];
    isoline_item.line_width = contour_style.line_width;
    let (positions, strip_lengths) = extract_isolines(&isoline_item);

    if positions.is_empty() || strip_lengths.is_empty() {
        return None;
    }

    Some(PlotGeometry::Polyline {
        positions: positions.into_iter().map(glam::Vec3::from).collect(),
        strip_lengths,
        scalars: None,
    })
}
