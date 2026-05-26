use crate::graph_spec::{ArrowAnnotation, PointAnnotation};
use crate::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PlotComponent, PlotGeometry, PlotObject,
    PlotStyle, Resolution, Scatter3D,
};
use viewport_lib::LabelItem;

pub struct AnnotatedPointsPlot {
    pub points: Vec<PointAnnotation>,
    pub show_labels: bool,
    pub style: PlotStyle,
}

pub struct AnnotatedArrowsPlot {
    pub arrows: Vec<ArrowAnnotation>,
    pub show_labels: bool,
    pub style: PlotStyle,
}

impl PlotObject for AnnotatedPointsPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        bounds_for_points(
            self.points
                .iter()
                .map(|point| glam::Vec3::from_array(point.position)),
        )
    }

    fn generate(&self, _domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let positions: Vec<glam::Vec3> = self
            .points
            .iter()
            .map(|point| glam::Vec3::from_array(point.position))
            .collect();
        let mut components = Vec::new();
        if !positions.is_empty() {
            components.push(PlotComponent {
                geometry: Scatter3D::from_points(&positions)
                    .generate(&Domain::default(), Resolution::default()),
                style: self.style.clone(),
            });
        }
        if self.show_labels {
            let labels = self
                .points
                .iter()
                .filter(|point| !point.label.trim().is_empty())
                .map(|point| {
                    let mut label = LabelItem::default();
                    label.text = point.label.clone();
                    label.world_anchor = Some(point.position);
                    label
                })
                .collect::<Vec<_>>();
            if !labels.is_empty() {
                components.push(PlotComponent {
                    geometry: PlotGeometry::Labels(labels),
                    style: self.style.clone(),
                });
            }
        }
        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}

impl PlotObject for AnnotatedArrowsPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        bounds_for_points(self.arrows.iter().flat_map(|arrow| {
            let origin = glam::Vec3::from_array(arrow.origin);
            let tip = origin + glam::Vec3::from_array(arrow.vector);
            [origin, tip]
        }))
    }

    fn generate(&self, _domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let glyphs = self
            .arrows
            .iter()
            .map(|arrow| {
                let vector = glam::Vec3::from_array(arrow.vector);
                GlyphInstance {
                    position: glam::Vec3::from_array(arrow.origin),
                    vector,
                    raw_vector: vector,
                }
            })
            .collect::<Vec<_>>();
        let mut components = vec![PlotComponent {
            geometry: PlotGeometry::Glyphs(glyphs),
            style: self.style.clone(),
        }];
        if self.show_labels {
            let labels = self
                .arrows
                .iter()
                .filter(|arrow| !arrow.label.trim().is_empty())
                .map(|arrow| {
                    let mut label = LabelItem::default();
                    let origin = glam::Vec3::from_array(arrow.origin);
                    let tip = origin + glam::Vec3::from_array(arrow.vector);
                    label.text = arrow.label.clone();
                    label.world_anchor = Some(tip.to_array());
                    label
                })
                .collect::<Vec<_>>();
            if !labels.is_empty() {
                components.push(PlotComponent {
                    geometry: PlotGeometry::Labels(labels),
                    style: self.style.clone(),
                });
            }
        }
        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}

fn bounds_for_points(points: impl IntoIterator<Item = glam::Vec3>) -> Option<DataBounds> {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for point in points {
        min = min.min(point);
        max = max.max(point);
        any = true;
    }
    any.then_some(DataBounds {
        x: min.x as f64..=max.x as f64,
        y: min.y as f64..=max.y as f64,
        z: min.z as f64..=max.z as f64,
    })
}
