use poincare_lib::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PlotComponent, PlotGeometry,
    PlotObject, PlotStyle, Resolution, Scatter3D,
};
use serde::{Deserialize, Serialize};
use viewport_lib::{AttributeData, IsolineItem, LabelItem, MeshData, extract_isolines};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SliceAxis {
    X,
    Y,
    Z,
}

impl SliceAxis {
    pub(crate) const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PointAnnotation {
    pub(crate) position: [f32; 3],
    pub(crate) label: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ArrowAnnotation {
    pub(crate) origin: [f32; 3],
    pub(crate) vector: [f32; 3],
    pub(crate) label: String,
}

pub(crate) struct ScalarSlicePlot {
    pub(crate) axis: SliceAxis,
    pub(crate) position: f64,
    pub(crate) value_fn: Box<dyn Fn(f64, f64, f64) -> f64 + Send + Sync>,
    pub(crate) contour_values: Vec<f32>,
    pub(crate) contour_style: PlotStyle,
    pub(crate) style: PlotStyle,
}

pub(crate) struct PlaneVectorFieldPlot {
    pub(crate) axis: SliceAxis,
    pub(crate) position: f64,
    pub(crate) vector_fn: Box<dyn Fn(f64, f64, f64) -> glam::Vec3 + Send + Sync>,
    pub(crate) style: PlotStyle,
}

pub(crate) struct AnnotatedPointsPlot {
    pub(crate) points: Vec<PointAnnotation>,
    pub(crate) show_labels: bool,
    pub(crate) style: PlotStyle,
}

pub(crate) struct AnnotatedArrowsPlot {
    pub(crate) arrows: Vec<ArrowAnnotation>,
    pub(crate) show_labels: bool,
    pub(crate) style: PlotStyle,
}

impl PlotObject for ScalarSlicePlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        None
    }

    fn generate(&self, domain: &Domain, resolution: Resolution) -> PlotGeometry {
        let u_count = resolution.u.max(2) as usize;
        let v_count = resolution.v.max(2) as usize;
        let mut positions = Vec::with_capacity(u_count * v_count);
        let mut values = Vec::with_capacity(u_count * v_count);
        let mut uvs = Vec::with_capacity(u_count * v_count);
        for j in 0..v_count {
            for i in 0..u_count {
                let tu = i as f64 / (u_count - 1) as f64;
                let tv = j as f64 / (v_count - 1) as f64;
                let (x, y, z) = plane_sample(domain, self.axis, self.position, tu, tv);
                let value = (self.value_fn)(x, y, z) as f32;
                positions.push([x as f32, y as f32, z as f32]);
                values.push(value);
                uvs.push([tu as f32, tv as f32]);
            }
        }

        let mut indices: Vec<u32> = Vec::with_capacity((u_count - 1) * (v_count - 1) * 6);
        for j in 0..(v_count - 1) {
            for i in 0..(u_count - 1) {
                let tl = (j * u_count + i) as u32;
                let tr = (j * u_count + i + 1) as u32;
                let bl = ((j + 1) * u_count + i) as u32;
                let br = ((j + 1) * u_count + i + 1) as u32;
                indices.extend_from_slice(&[tl, tr, bl]);
                indices.extend_from_slice(&[tr, br, bl]);
            }
        }

        let normal = match self.axis {
            SliceAxis::X => [1.0, 0.0, 0.0],
            SliceAxis::Y => [0.0, 1.0, 0.0],
            SliceAxis::Z => [0.0, 0.0, 1.0],
        };
        let normals = vec![normal; positions.len()];

        let tangent_u = vec![
            match self.axis {
                SliceAxis::X => [0.0, 1.0, 0.0],
                SliceAxis::Y => [1.0, 0.0, 0.0],
                SliceAxis::Z => [1.0, 0.0, 0.0],
            };
            positions.len()
        ];
        let tangent_v = vec![
            match self.axis {
                SliceAxis::X => [0.0, 0.0, 1.0],
                SliceAxis::Y => [0.0, 0.0, 1.0],
                SliceAxis::Z => [0.0, 1.0, 0.0],
            };
            positions.len()
        ];

        let mut mesh = MeshData::default();
        mesh.positions = positions;
        mesh.normals = normals;
        mesh.indices = indices;
        mesh.uvs = Some(uvs);
        mesh.attributes.insert(
            "x".to_string(),
            AttributeData::Vertex(mesh.positions.iter().map(|p| p[0]).collect()),
        );
        mesh.attributes.insert(
            "y".to_string(),
            AttributeData::Vertex(mesh.positions.iter().map(|p| p[1]).collect()),
        );
        mesh.attributes.insert(
            "z".to_string(),
            AttributeData::Vertex(mesh.positions.iter().map(|p| p[2]).collect()),
        );
        mesh.attributes
            .insert("value".to_string(), AttributeData::Vertex(values));
        mesh.attributes.insert(
            "tangent_u".to_string(),
            AttributeData::VertexVector(tangent_u.clone()),
        );
        mesh.attributes.insert(
            "tangent_v".to_string(),
            AttributeData::VertexVector(tangent_v.clone()),
        );
        mesh.attributes.insert(
            "tangent_diagonal".to_string(),
            AttributeData::VertexVector(
                tangent_u
                    .iter()
                    .zip(&tangent_v)
                    .map(|(u, v)| (glam::Vec3::from(*u) + glam::Vec3::from(*v)).normalize_or_zero().to_array())
                    .collect(),
            ),
        );
        mesh.attributes.insert(
            "tangent_saddle".to_string(),
            AttributeData::VertexVector(
                tangent_u
                    .iter()
                    .zip(&tangent_v)
                    .map(|(u, v)| (glam::Vec3::from(*u) - glam::Vec3::from(*v)).normalize_or_zero().to_array())
                    .collect(),
            ),
        );

        let isolines = build_isolines(&mesh, &self.contour_values);
        let mut components = vec![PlotComponent {
            geometry: PlotGeometry::Surface(mesh),
            style: self.style.clone(),
        }];
        if let Some(isolines) = isolines {
            components.push(PlotComponent {
                geometry: isolines,
                style: self.contour_style.clone(),
            });
        }
        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}

impl PlotObject for PlaneVectorFieldPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        None
    }

    fn generate(&self, domain: &Domain, resolution: Resolution) -> PlotGeometry {
        let u_count = resolution.u.max(2) as usize;
        let v_count = resolution.v.max(2) as usize;
        let mut glyphs = Vec::with_capacity(u_count * v_count);
        for j in 0..v_count {
            for i in 0..u_count {
                let tu = if u_count > 1 {
                    i as f64 / (u_count - 1) as f64
                } else {
                    0.5
                };
                let tv = if v_count > 1 {
                    j as f64 / (v_count - 1) as f64
                } else {
                    0.5
                };
                let (x, y, z) = plane_sample(domain, self.axis, self.position, tu, tv);
                let raw = (self.vector_fn)(x, y, z);
                glyphs.push(GlyphInstance {
                    position: glam::vec3(x as f32, y as f32, z as f32),
                    vector: raw.normalize_or_zero() * self.style.glyph_scale,
                    raw_vector: raw,
                });
            }
        }
        PlotGeometry::Glyphs(glyphs)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}

impl PlotObject for AnnotatedPointsPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        bounds_for_points(self.points.iter().map(|point| glam::Vec3::from_array(point.position)))
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
                geometry: Scatter3D::from_points(&positions).generate(&_domain_default(), Resolution::default()),
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

fn build_isolines(mesh: &MeshData, contour_values: &[f32]) -> Option<PlotGeometry> {
    if contour_values.is_empty() {
        return None;
    }
    let scalars = match mesh.attributes.get("value") {
        Some(AttributeData::Vertex(values)) => values.clone(),
        _ => return None,
    };
    let mut item = IsolineItem::default();
    item.positions = mesh.positions.clone();
    item.indices = mesh.indices.clone();
    item.scalars = scalars;
    item.isovalues = contour_values.to_vec();
    let (positions, strip_lengths) = extract_isolines(&item);
    if positions.is_empty() {
        return None;
    }
    Some(PlotGeometry::Polyline {
        positions: positions.into_iter().map(glam::Vec3::from).collect(),
        strip_lengths,
        scalars: None,
    })
}

fn plane_sample(
    domain: &Domain,
    axis: SliceAxis,
    position: f64,
    tu: f64,
    tv: f64,
) -> (f64, f64, f64) {
    let lerp = |range: &std::ops::RangeInclusive<f64>, t: f64| {
        *range.start() + t * (*range.end() - *range.start())
    };
    match axis {
        SliceAxis::X => (position, lerp(&domain.y, tu), lerp(&domain.z, tv)),
        SliceAxis::Y => (lerp(&domain.x, tu), position, lerp(&domain.z, tv)),
        SliceAxis::Z => (lerp(&domain.x, tu), lerp(&domain.y, tv), position),
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

fn _domain_default() -> Domain {
    Domain::default()
}

pub(crate) fn default_slice_position(domain: &Domain, axis: SliceAxis) -> f64 {
    match axis {
        SliceAxis::X => (*domain.x.start() + *domain.x.end()) * 0.5,
        SliceAxis::Y => (*domain.y.start() + *domain.y.end()) * 0.5,
        SliceAxis::Z => (*domain.z.start() + *domain.z.end()) * 0.5,
    }
}

pub(crate) fn make_point_annotations(points: &[[f32; 3]], prefix: &str) -> Vec<PointAnnotation> {
    points
        .iter()
        .enumerate()
        .map(|(index, &position)| PointAnnotation {
            position,
            label: format!("{prefix} {}", index + 1),
        })
        .collect()
}

pub(crate) fn make_arrow_annotation(
    origin: glam::Vec3,
    vector: glam::Vec3,
    label: impl Into<String>,
) -> ArrowAnnotation {
    ArrowAnnotation {
        origin: origin.to_array(),
        vector: vector.to_array(),
        label: label.into(),
    }
}
