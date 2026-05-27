#![allow(dead_code)]

pub(crate) use poincare_lib::{ArrowAnnotation, PointAnnotation, SliceAxis};
use poincare_lib::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PlotComponent, PlotGeometry, PlotObject,
    PlotStyle, Resolution, Scatter3D,
};
use viewport_lib::{AttributeData, IsolineItem, LabelItem, MeshData, extract_isolines};

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

pub(crate) struct SurfaceIntersectionResult {
    pub(crate) curves: Vec<Vec<glam::Vec3>>,
    pub(crate) isolated_points: Vec<glam::Vec3>,
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
                    vector: raw.normalize_or_zero(),
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
                    .generate(&_domain_default(), Resolution::default()),
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
                let raw_vector = glam::Vec3::from_array(arrow.vector);
                GlyphInstance {
                    position: glam::Vec3::from_array(arrow.origin),
                    vector: raw_vector,
                    raw_vector,
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

pub(crate) fn make_point_annotations(points: &[[f32; 3]], _prefix: &str) -> Vec<PointAnnotation> {
    points
        .iter()
        .map(|&position| PointAnnotation {
            position,
            label: String::new(),
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

pub(crate) fn intersect_surface_meshes(
    positions_a: &[[f32; 3]],
    indices_a: &[u32],
    positions_b: &[[f32; 3]],
    indices_b: &[u32],
    tolerance: f32,
    stitch_distance: f32,
) -> SurfaceIntersectionResult {
    let triangles_a = triangle_bounds(positions_a, indices_a, tolerance);
    let triangles_b = triangle_bounds(positions_b, indices_b, tolerance);
    let mut segments = Vec::new();
    let mut isolated_points = Vec::new();

    for (tri_a, bounds_a) in indices_a.chunks_exact(3).zip(triangles_a.iter()) {
        let a = [
            glam::Vec3::from(positions_a[tri_a[0] as usize]),
            glam::Vec3::from(positions_a[tri_a[1] as usize]),
            glam::Vec3::from(positions_a[tri_a[2] as usize]),
        ];
        for (tri_b, bounds_b) in indices_b.chunks_exact(3).zip(triangles_b.iter()) {
            if !aabb_overlap(bounds_a, bounds_b, tolerance) {
                continue;
            }
            let b = [
                glam::Vec3::from(positions_b[tri_b[0] as usize]),
                glam::Vec3::from(positions_b[tri_b[1] as usize]),
                glam::Vec3::from(positions_b[tri_b[2] as usize]),
            ];
            match intersect_triangles(a, b, tolerance) {
                TriangleIntersection::Segment(start, end) => segments.push((start, end)),
                TriangleIntersection::Point(point) => isolated_points.push(point),
                TriangleIntersection::None => {}
            }
        }
    }

    let curves = stitch_segments(&segments, stitch_distance.max(tolerance * 2.0));
    let isolated_points = dedup_points(&isolated_points, stitch_distance.max(tolerance * 2.0));
    SurfaceIntersectionResult {
        curves,
        isolated_points,
    }
}

#[derive(Clone, Copy)]
struct Bounds3 {
    min: glam::Vec3,
    max: glam::Vec3,
}

#[derive(Clone, Copy)]
enum TriangleIntersection {
    None,
    Point(glam::Vec3),
    Segment(glam::Vec3, glam::Vec3),
}

fn triangle_bounds(positions: &[[f32; 3]], indices: &[u32], tolerance: f32) -> Vec<Bounds3> {
    indices
        .chunks_exact(3)
        .map(|tri| {
            let mut min = glam::Vec3::splat(f32::INFINITY);
            let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
            for &index in tri {
                let point = glam::Vec3::from(positions[index as usize]);
                min = min.min(point);
                max = max.max(point);
            }
            let pad = glam::Vec3::splat(tolerance);
            Bounds3 {
                min: min - pad,
                max: max + pad,
            }
        })
        .collect()
}

fn aabb_overlap(a: &Bounds3, b: &Bounds3, tolerance: f32) -> bool {
    a.min.x <= b.max.x + tolerance
        && a.max.x + tolerance >= b.min.x
        && a.min.y <= b.max.y + tolerance
        && a.max.y + tolerance >= b.min.y
        && a.min.z <= b.max.z + tolerance
        && a.max.z + tolerance >= b.min.z
}

fn intersect_triangles(
    a: [glam::Vec3; 3],
    b: [glam::Vec3; 3],
    tolerance: f32,
) -> TriangleIntersection {
    let plane_a = triangle_plane(a);
    let plane_b = triangle_plane(b);
    let dir = plane_a.0.cross(plane_b.0);
    if dir.length_squared() <= tolerance * tolerance {
        return TriangleIntersection::None;
    }

    let seg_a = triangle_plane_clip_segment(a, plane_b.0, plane_b.1, tolerance);
    let seg_b = triangle_plane_clip_segment(b, plane_a.0, plane_a.1, tolerance);
    let (seg_a0, seg_a1) = match seg_a {
        Some(segment) => segment,
        None => return TriangleIntersection::None,
    };
    let (seg_b0, seg_b1) = match seg_b {
        Some(segment) => segment,
        None => return TriangleIntersection::None,
    };

    let dir_n = dir.normalize_or_zero();
    if dir_n.length_squared() <= 1.0e-12 {
        return TriangleIntersection::None;
    }
    let reference = seg_a0;
    let a0 = dir_n.dot(seg_a0 - reference);
    let a1 = dir_n.dot(seg_a1 - reference);
    let b0 = dir_n.dot(seg_b0 - reference);
    let b1 = dir_n.dot(seg_b1 - reference);
    let a_min = a0.min(a1);
    let a_max = a0.max(a1);
    let b_min = b0.min(b1);
    let b_max = b0.max(b1);
    let overlap_min = a_min.max(b_min);
    let overlap_max = a_max.min(b_max);
    if overlap_max < overlap_min - tolerance {
        return TriangleIntersection::None;
    }
    if (overlap_max - overlap_min).abs() <= tolerance {
        let point = reference + dir_n * ((overlap_min + overlap_max) * 0.5);
        return TriangleIntersection::Point(point);
    }
    TriangleIntersection::Segment(
        reference + dir_n * overlap_min,
        reference + dir_n * overlap_max,
    )
}

fn triangle_plane(tri: [glam::Vec3; 3]) -> (glam::Vec3, f32) {
    let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
    (normal, -normal.dot(tri[0]))
}

fn triangle_plane_clip_segment(
    tri: [glam::Vec3; 3],
    plane_normal: glam::Vec3,
    plane_offset: f32,
    tolerance: f32,
) -> Option<(glam::Vec3, glam::Vec3)> {
    let mut hits = Vec::new();
    let signed = tri.map(|point| plane_normal.dot(point) + plane_offset);
    for i in 0..3 {
        if signed[i].abs() <= tolerance {
            hits.push(tri[i]);
        }
    }
    for (i0, i1) in [(0, 1), (1, 2), (2, 0)] {
        let d0 = signed[i0];
        let d1 = signed[i1];
        if (d0 > tolerance && d1 > tolerance) || (d0 < -tolerance && d1 < -tolerance) {
            continue;
        }
        if (d0 - d1).abs() <= tolerance {
            continue;
        }
        if d0.abs() <= tolerance || d1.abs() <= tolerance || d0.signum() == d1.signum() {
            continue;
        }
        let t = d0 / (d0 - d1);
        hits.push(tri[i0].lerp(tri[i1], t));
    }
    let hits = dedup_points(&hits, tolerance * 2.0);
    match hits.as_slice() {
        [single] => Some((*single, *single)),
        [first, second, ..] => Some((*first, *second)),
        _ => None,
    }
}

fn dedup_points(points: &[glam::Vec3], tolerance: f32) -> Vec<glam::Vec3> {
    let mut unique: Vec<glam::Vec3> = Vec::new();
    'outer: for &point in points {
        for &existing in &unique {
            if existing.distance(point) <= tolerance {
                continue 'outer;
            }
        }
        unique.push(point);
    }
    unique
}

fn stitch_segments(segments: &[(glam::Vec3, glam::Vec3)], tolerance: f32) -> Vec<Vec<glam::Vec3>> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut nodes = Vec::<glam::Vec3>::new();
    let mut edges = Vec::<(usize, usize)>::new();
    for &(start, end) in segments {
        let a = find_or_insert_node(&mut nodes, start, tolerance);
        let b = find_or_insert_node(&mut nodes, end, tolerance);
        if a != b {
            edges.push((a, b));
        }
    }
    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    for (edge_index, &(a, b)) in edges.iter().enumerate() {
        adjacency[a].push(edge_index);
        adjacency[b].push(edge_index);
    }
    let mut visited = vec![false; edges.len()];
    let mut curves = Vec::new();

    for start in 0..nodes.len() {
        if adjacency[start].len() == 2 {
            continue;
        }
        for &edge_index in &adjacency[start] {
            if visited[edge_index] {
                continue;
            }
            curves.push(trace_curve(
                start,
                edge_index,
                &nodes,
                &edges,
                &adjacency,
                &mut visited,
            ));
        }
    }

    for edge_index in 0..edges.len() {
        if visited[edge_index] {
            continue;
        }
        let start = edges[edge_index].0;
        curves.push(trace_curve(
            start,
            edge_index,
            &nodes,
            &edges,
            &adjacency,
            &mut visited,
        ));
    }

    curves
        .into_iter()
        .filter(|curve| curve.len() >= 2)
        .collect()
}

fn trace_curve(
    start_node: usize,
    start_edge: usize,
    nodes: &[glam::Vec3],
    edges: &[(usize, usize)],
    adjacency: &[Vec<usize>],
    visited: &mut [bool],
) -> Vec<glam::Vec3> {
    let mut curve = vec![nodes[start_node]];
    let mut current_node = start_node;
    let mut current_edge = start_edge;
    loop {
        if visited[current_edge] {
            break;
        }
        visited[current_edge] = true;
        let (a, b) = edges[current_edge];
        let next_node = if a == current_node { b } else { a };
        curve.push(nodes[next_node]);
        let next_edge = adjacency[next_node]
            .iter()
            .copied()
            .find(|&edge| !visited[edge] && edge != current_edge);
        match next_edge {
            Some(edge) => {
                current_node = next_node;
                current_edge = edge;
            }
            None => break,
        }
    }
    curve
}

fn find_or_insert_node(nodes: &mut Vec<glam::Vec3>, point: glam::Vec3, tolerance: f32) -> usize {
    for (index, existing) in nodes.iter().enumerate() {
        if existing.distance(point) <= tolerance {
            return index;
        }
    }
    nodes.push(point);
    nodes.len() - 1
}
