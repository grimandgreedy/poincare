use crate::graph_spec::SliceAxis;
use crate::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PlotComponent, PlotGeometry, PlotObject,
    PlotStyle, Resolution,
};
use viewport_lib::{AttributeData, IsolineItem, MeshData, extract_isolines};

pub struct ScalarSlicePlot {
    pub axis: SliceAxis,
    pub position: f64,
    pub value_fn: Box<dyn Fn(f64, f64, f64) -> f64 + Send + Sync>,
    pub contour_values: Vec<f32>,
    pub contour_style: PlotStyle,
    pub style: PlotStyle,
}

pub struct PlaneVectorFieldPlot {
    pub axis: SliceAxis,
    pub position: f64,
    pub vector_fn: Box<dyn Fn(f64, f64, f64) -> glam::Vec3 + Send + Sync>,
    pub style: PlotStyle,
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
                    .map(|(u, v)| {
                        (glam::Vec3::from(*u) + glam::Vec3::from(*v))
                            .normalize_or_zero()
                            .to_array()
                    })
                    .collect(),
            ),
        );
        mesh.attributes.insert(
            "tangent_saddle".to_string(),
            AttributeData::VertexVector(
                tangent_u
                    .iter()
                    .zip(&tangent_v)
                    .map(|(u, v)| {
                        (glam::Vec3::from(*u) - glam::Vec3::from(*v))
                            .normalize_or_zero()
                            .to_array()
                    })
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

pub fn default_slice_position(domain: &Domain, axis: SliceAxis) -> f64 {
    match axis {
        SliceAxis::X => (*domain.x.start() + *domain.x.end()) * 0.5,
        SliceAxis::Y => (*domain.y.start() + *domain.y.end()) * 0.5,
        SliceAxis::Z => (*domain.z.start() + *domain.z.end()) * 0.5,
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
