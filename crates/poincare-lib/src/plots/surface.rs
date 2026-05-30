use std::f64::consts::PI;
use std::ops::RangeInclusive;

use glam::Vec3;

use viewport_lib::{AttributeData, MeshData};

use crate::coordinate::{
    CoordinateSystem, ParametricDomain, cylindrical_to_cartesian, polar_to_cartesian,
    spherical_to_cartesian,
};
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::{PlotStyle, ShadingMode};

// ---------------------------------------------------------------------------
// Internal kind enum
// ---------------------------------------------------------------------------

enum SurfaceKind {
    /// z = f(x, y) over domain.x × domain.y
    Cartesian(Box<dyn Fn(f64, f64) -> f64 + Send + Sync>),
    /// r = f(theta, phi); theta ∈ [0, π], phi ∈ [0, 2π]
    Spherical(Box<dyn Fn(f64, f64) -> f64 + Send + Sync>),
    /// r = f(theta, z); theta ∈ [0, 2π], z from domain.z
    Cylindrical(Box<dyn Fn(f64, f64) -> f64 + Send + Sync>),
    /// r = f(theta); theta ∈ [0, 2π], fills a radial disk at z = 0
    Polar(Box<dyn Fn(f64) -> f64 + Send + Sync>),
    /// (x, y, z) = f(u, v) over explicit u × v domain
    Parametric {
        f: Box<dyn Fn(f64, f64) -> glam::DVec3 + Send + Sync>,
        domain: ParametricDomain,
    },
    /// Data-driven grid: positions read directly from xs/ys/zs arrays.
    ///
    /// Layout: `zs[j * xs.len() + i]` where `i` indexes `xs` and `j` indexes `ys`.
    Grid {
        xs: Vec<f64>,
        ys: Vec<f64>,
        zs: Vec<f64>,
    },
}

// ---------------------------------------------------------------------------
// Public struct
// ---------------------------------------------------------------------------

/// A 3D surface plot.
///
/// Supports six surface modes; all produce Cartesian mesh geometry.
pub struct Surface3D {
    kind: SurfaceKind,
    style: PlotStyle,
    resolution: Resolution,
    domain_override: Option<Domain>,
}

impl Surface3D {
    fn default_surface_style() -> PlotStyle {
        PlotStyle {
            two_sided: true,
            ..PlotStyle::default()
        }
    }

    // ------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------

    /// Create a Cartesian surface z = f(x, y).
    pub fn from_fn(f: impl Fn(f64, f64) -> f64 + Send + Sync + 'static) -> Self {
        Self {
            kind: SurfaceKind::Cartesian(Box::new(f)),
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Spherical surface r = f(theta, phi).
    ///
    /// `theta` ∈ [0, π], `phi` ∈ [0, 2π].
    pub fn spherical(f: impl Fn(f64, f64) -> f64 + Send + Sync + 'static) -> Self {
        Self {
            kind: SurfaceKind::Spherical(Box::new(f)),
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Cylindrical surface r = f(theta, z).
    ///
    /// `theta` ∈ [0, 2π], `z` sampled from `domain.z`.
    pub fn cylindrical(f: impl Fn(f64, f64) -> f64 + Send + Sync + 'static) -> Self {
        Self {
            kind: SurfaceKind::Cylindrical(Box::new(f)),
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Polar surface r = f(theta), rendered as a filled radial disk at z = 0.
    ///
    /// `theta` ∈ [0, 2π].
    pub fn polar(f: impl Fn(f64) -> f64 + Send + Sync + 'static) -> Self {
        Self {
            kind: SurfaceKind::Polar(Box::new(f)),
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Fully parametric surface (x, y, z) = f(u, v).
    pub fn parametric(
        u_range: RangeInclusive<f64>,
        v_range: RangeInclusive<f64>,
        f: impl Fn(f64, f64) -> glam::DVec3 + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind: SurfaceKind::Parametric {
                f: Box::new(f),
                domain: ParametricDomain::Surface {
                    u: u_range,
                    v: v_range,
                },
            },
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Create a surface from explicit grid data.
    ///
    /// `xs` and `ys` are the axis tick values; `zs` contains height values in
    /// row-major order: `zs[j * xs.len() + i]` gives the z at `(xs[i], ys[j])`.
    ///
    /// # Panics
    ///
    /// Panics if `zs.len() != xs.len() * ys.len()`.
    pub fn from_grid(xs: &[f64], ys: &[f64], zs: &[f64]) -> Self {
        assert_eq!(
            zs.len(),
            xs.len() * ys.len(),
            "Surface3D::from_grid: zs.len() must equal xs.len() * ys.len()"
        );
        Self {
            kind: SurfaceKind::Grid {
                xs: xs.to_vec(),
                ys: ys.to_vec(),
                zs: zs.to_vec(),
            },
            style: Self::default_surface_style(),
            resolution: Resolution::default(),
            domain_override: None,
        }
    }

    /// Override the default style.
    pub fn with_style(mut self, style: PlotStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = resolution;
        self
    }

    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain_override = Some(domain);
        self
    }
}

// ---------------------------------------------------------------------------
// PlotObject implementation
// ---------------------------------------------------------------------------

impl PlotObject for Surface3D {
    fn coordinate_system(&self) -> CoordinateSystem {
        match &self.kind {
            SurfaceKind::Cartesian(_) => CoordinateSystem::Cartesian,
            SurfaceKind::Spherical(_) => CoordinateSystem::Spherical,
            SurfaceKind::Cylindrical(_) => CoordinateSystem::Cylindrical,
            SurfaceKind::Polar(_) => CoordinateSystem::Polar,
            SurfaceKind::Grid { .. } => CoordinateSystem::Cartesian,
            SurfaceKind::Parametric { domain, .. } => {
                // Clone domain ranges for the return value.
                match domain {
                    ParametricDomain::Surface { u, v } => {
                        CoordinateSystem::Parametric(ParametricDomain::Surface {
                            u: u.clone(),
                            v: v.clone(),
                        })
                    }
                    ParametricDomain::Curve { t } => {
                        CoordinateSystem::Parametric(ParametricDomain::Curve { t: t.clone() })
                    }
                }
            }
        }
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        None
    }

    fn generate(&self, domain: &Domain, resolution: Resolution) -> PlotGeometry {
        let u_count = resolution.u.max(2) as usize;
        let v_count = resolution.v.max(2) as usize;

        // For Grid, the effective u/v grid dimensions come from the data arrays.
        // For all other kinds the resolution controls sampling density.
        let (positions, values, uvs, eff_u, eff_v): (
            Vec<[f32; 3]>,
            Vec<f32>,
            Vec<[f32; 2]>,
            usize,
            usize,
        ) = match &self.kind {
            SurfaceKind::Grid { xs, ys, zs } => {
                let u_c = xs.len();
                let v_c = ys.len();
                let mut pts = Vec::with_capacity(u_c * v_c);
                let mut vals = Vec::with_capacity(u_c * v_c);
                let mut uv_values = Vec::with_capacity(u_c * v_c);
                for j in 0..v_c {
                    for i in 0..u_c {
                        pts.push([xs[i] as f32, ys[j] as f32, zs[j * u_c + i] as f32]);
                        vals.push(zs[j * u_c + i] as f32);
                        uv_values.push(normalized_grid_uv(i, j, u_c, v_c));
                    }
                }
                (pts, vals, uv_values, u_c, v_c)
            }

            SurfaceKind::Cartesian(f) => {
                let x0 = *domain.x.start();
                let x1 = *domain.x.end();
                let y0 = *domain.y.start();
                let y1 = *domain.y.end();

                let mut pts = Vec::with_capacity(u_count * v_count);
                let mut vals = Vec::with_capacity(u_count * v_count);
                let mut uv_values = Vec::with_capacity(u_count * v_count);
                for j in 0..v_count {
                    for i in 0..u_count {
                        let tx = i as f64 / (u_count - 1) as f64;
                        let ty = j as f64 / (v_count - 1) as f64;
                        let x = x0 + tx * (x1 - x0);
                        let y = y0 + ty * (y1 - y0);
                        let z = f(x, y);
                        pts.push([x as f32, y as f32, z as f32]);
                        vals.push(z as f32);
                        uv_values.push([tx as f32, ty as f32]);
                    }
                }
                (pts, vals, uv_values, u_count, v_count)
            }

            SurfaceKind::Spherical(f) => {
                let mut pts = Vec::with_capacity(u_count * v_count);
                let mut vals = Vec::with_capacity(u_count * v_count);
                let mut uv_values = Vec::with_capacity(u_count * v_count);
                for j in 0..v_count {
                    for i in 0..u_count {
                        let theta = PI * j as f64 / (v_count - 1) as f64; // [0, π]
                        let phi = 2.0 * PI * i as f64 / (u_count - 1) as f64; // [0, 2π]
                        let r = f(theta, phi);
                        let (x, y, z) = spherical_to_cartesian(r, theta, phi);
                        pts.push([x as f32, y as f32, z as f32]);
                        vals.push(r as f32);
                        uv_values.push([(phi / (2.0 * PI)) as f32, (theta / PI) as f32]);
                    }
                }
                (pts, vals, uv_values, u_count, v_count)
            }

            SurfaceKind::Cylindrical(f) => {
                let z0 = *domain.z.start();
                let z1 = *domain.z.end();

                let mut pts = Vec::with_capacity(u_count * v_count);
                let mut vals = Vec::with_capacity(u_count * v_count);
                let mut uv_values = Vec::with_capacity(u_count * v_count);
                for j in 0..v_count {
                    for i in 0..u_count {
                        let theta = 2.0 * PI * i as f64 / (u_count - 1) as f64; // [0, 2π]
                        let tz = j as f64 / (v_count - 1) as f64;
                        let z = z0 + tz * (z1 - z0);
                        let r = f(theta, z);
                        let (x, y, z_out) = cylindrical_to_cartesian(r, theta, z);
                        pts.push([x as f32, y as f32, z_out as f32]);
                        vals.push(r as f32);
                        uv_values.push([(theta / (2.0 * PI)) as f32, tz as f32]);
                    }
                }
                (pts, vals, uv_values, u_count, v_count)
            }

            SurfaceKind::Polar(f) => {
                // Build a filled radial disk: u=angular samples, v=radial samples (0 to r).
                let mut pts = Vec::with_capacity(u_count * v_count);
                let mut vals = Vec::with_capacity(u_count * v_count);
                let mut uv_values = Vec::with_capacity(u_count * v_count);
                for j in 0..v_count {
                    for i in 0..u_count {
                        let theta = 2.0 * PI * i as f64 / (u_count - 1) as f64;
                        let r_edge = f(theta);
                        // v fraction scales from 0 at center to r_edge at edge.
                        let t_v = j as f64 / (v_count - 1) as f64;
                        let r = t_v * r_edge;
                        let (x, y, z) = polar_to_cartesian(r, theta);
                        pts.push([x as f32, y as f32, z as f32]);
                        vals.push(r_edge as f32);
                        uv_values.push([(theta / (2.0 * PI)) as f32, t_v as f32]);
                    }
                }
                (pts, vals, uv_values, u_count, v_count)
            }

            SurfaceKind::Parametric { f, domain } => match domain {
                ParametricDomain::Surface { u, v } => {
                    let u0 = *u.start();
                    let u1 = *u.end();
                    let v0 = *v.start();
                    let v1 = *v.end();

                    let mut pts = Vec::with_capacity(u_count * v_count);
                    let mut vals = Vec::with_capacity(u_count * v_count);
                    let mut uv_values = Vec::with_capacity(u_count * v_count);
                    for j in 0..v_count {
                        for i in 0..u_count {
                            let tu = i as f64 / (u_count - 1) as f64;
                            let tv = j as f64 / (v_count - 1) as f64;
                            let u_val = u0 + tu * (u1 - u0);
                            let v_val = v0 + tv * (v1 - v0);
                            let p = f(u_val, v_val);
                            pts.push([p.x as f32, p.y as f32, p.z as f32]);
                            vals.push(p.z as f32);
                            uv_values.push([tu as f32, tv as f32]);
                        }
                    }
                    (pts, vals, uv_values, u_count, v_count)
                }
                // Curve domain doesn't make sense for a surface; treat as degenerate.
                ParametricDomain::Curve { .. } => {
                    (Vec::new(), Vec::new(), Vec::new(), u_count, v_count)
                }
            },
        };

        if positions.is_empty() {
            return PlotGeometry::Surface(MeshData::default());
        }

        // Generate (eff_u-1)×(eff_v-1)×2 triangles (same topology for every variant).
        let mut indices: Vec<u32> = Vec::with_capacity((eff_u - 1) * (eff_v - 1) * 6);
        for j in 0..(eff_v - 1) {
            for i in 0..(eff_u - 1) {
                let tl = (j * eff_u + i) as u32;
                let tr = (j * eff_u + i + 1) as u32;
                let bl = ((j + 1) * eff_u + i) as u32;
                let br = ((j + 1) * eff_u + i + 1) as u32;
                indices.extend_from_slice(&[tl, tr, bl]);
                indices.extend_from_slice(&[tr, br, bl]);
            }
        }

        // Accumulate face normals.
        let mut normals: Vec<[f32; 3]> = vec![[0.0; 3]; positions.len()];
        let mut accum: Vec<Vec3> = vec![Vec3::ZERO; positions.len()];
        for tri in indices.chunks_exact(3) {
            let a = Vec3::from(positions[tri[0] as usize]);
            let b = Vec3::from(positions[tri[1] as usize]);
            let c = Vec3::from(positions[tri[2] as usize]);
            let n = (b - a).cross(c - a);
            accum[tri[0] as usize] += n;
            accum[tri[1] as usize] += n;
            accum[tri[2] as usize] += n;
        }
        for (i, n) in accum.iter().enumerate() {
            normals[i] = n.normalize_or_zero().to_array();
        }

        let mut tangent_u: Vec<[f32; 3]> = vec![[0.0; 3]; positions.len()];
        let mut tangent_v: Vec<[f32; 3]> = vec![[0.0; 3]; positions.len()];
        let mut tangent_diagonal: Vec<[f32; 3]> = vec![[0.0; 3]; positions.len()];
        let mut tangent_saddle: Vec<[f32; 3]> = vec![[0.0; 3]; positions.len()];
        for j in 0..eff_v {
            for i in 0..eff_u {
                let idx = j * eff_u + i;
                let i0 = i.saturating_sub(1);
                let i1 = (i + 1).min(eff_u - 1);
                let j0 = j.saturating_sub(1);
                let j1 = (j + 1).min(eff_v - 1);
                let n = Vec3::from(normals[idx]);

                let du = if i0 != i1 {
                    Vec3::from(positions[j * eff_u + i1]) - Vec3::from(positions[j * eff_u + i0])
                } else {
                    Vec3::ZERO
                };
                let dv = if j0 != j1 {
                    Vec3::from(positions[j1 * eff_u + i]) - Vec3::from(positions[j0 * eff_u + i])
                } else {
                    Vec3::ZERO
                };

                let tu = (du - n * du.dot(n)).normalize_or_zero();
                let tv = (dv - n * dv.dot(n)).normalize_or_zero();
                tangent_u[idx] = tu.to_array();
                tangent_v[idx] = tv.to_array();
                tangent_diagonal[idx] = (tu + tv).normalize_or_zero().to_array();
                tangent_saddle[idx] = (tu - tv).normalize_or_zero().to_array();
            }
        }

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
        mesh.attributes.insert(
            "radius".to_string(),
            AttributeData::Vertex(
                mesh.positions
                    .iter()
                    .map(|p| Vec3::from(*p).length())
                    .collect(),
            ),
        );
        mesh.attributes
            .insert("value".to_string(), AttributeData::Vertex(values));
        mesh.attributes.insert(
            "tangent_u".to_string(),
            AttributeData::VertexVector(tangent_u),
        );
        mesh.attributes.insert(
            "tangent_v".to_string(),
            AttributeData::VertexVector(tangent_v),
        );
        mesh.attributes.insert(
            "tangent_diagonal".to_string(),
            AttributeData::VertexVector(tangent_diagonal),
        );
        mesh.attributes.insert(
            "tangent_saddle".to_string(),
            AttributeData::VertexVector(tangent_saddle),
        );
        let (angle_distortion, area_distortion) =
            compute_face_surface_quantities(&mesh.positions, &mesh.indices, mesh.uvs.as_deref());
        mesh.attributes.insert(
            "angle_distortion".to_string(),
            AttributeData::Face(angle_distortion),
        );
        mesh.attributes.insert(
            "area_distortion".to_string(),
            AttributeData::Face(area_distortion),
        );

        if self.style.shading == ShadingMode::Flat {
            mesh = expand_flat_shaded_mesh(mesh);
        }

        PlotGeometry::Surface(mesh)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }

    fn domain_override(&self) -> Option<&Domain> {
        self.domain_override.as_ref()
    }
}

fn expand_flat_shaded_mesh(mesh: MeshData) -> MeshData {
    let MeshData {
        positions,
        uvs,
        attributes,
        indices,
        ..
    } = mesh;

    let mut vertex_attributes = std::collections::HashMap::new();
    let mut vertex_vector_attributes = std::collections::HashMap::new();
    let mut passthrough_attributes = std::collections::HashMap::new();
    for (name, data) in attributes {
        match data {
            AttributeData::Vertex(values) => {
                vertex_attributes.insert(name, values);
            }
            AttributeData::VertexVector(values) => {
                vertex_vector_attributes.insert(name, values);
            }
            AttributeData::Cell(values) => {
                passthrough_attributes.insert(name, AttributeData::Cell(values));
            }
            AttributeData::Face(values) => {
                passthrough_attributes.insert(name, AttributeData::Face(values));
            }
            AttributeData::FaceColour(values) => {
                passthrough_attributes.insert(name, AttributeData::FaceColour(values));
            }
            AttributeData::Edge(values)
            | AttributeData::Halfedge(values)
            | AttributeData::Corner(values) => {
                // [3*tri + corner] indexing matches flat vertex order after expansion.
                passthrough_attributes.insert(name, AttributeData::Vertex(values));
            }
        }
    }

    let mut flat_positions = Vec::with_capacity(indices.len());
    let mut flat_normals = Vec::with_capacity(indices.len());
    let mut flat_indices = Vec::with_capacity(indices.len());
    let mut flat_uvs = uvs.as_ref().map(|values| Vec::with_capacity(values.len()));
    let mut flat_attributes: std::collections::HashMap<String, Vec<f32>> = vertex_attributes
        .iter()
        .map(|(name, values)| (name.clone(), Vec::with_capacity(values.len())))
        .collect();
    let mut flat_vector_attributes: std::collections::HashMap<String, Vec<[f32; 3]>> =
        vertex_vector_attributes
            .iter()
            .map(|(name, values)| (name.clone(), Vec::with_capacity(values.len())))
            .collect();

    for tri in indices.chunks_exact(3) {
        let a = Vec3::from(positions[tri[0] as usize]);
        let b = Vec3::from(positions[tri[1] as usize]);
        let c = Vec3::from(positions[tri[2] as usize]);
        let n = (b - a).cross(c - a).normalize_or_zero().to_array();

        for &src in tri {
            let src_index = src as usize;
            flat_positions.push(positions[src_index]);
            flat_normals.push(n);
            flat_indices.push((flat_indices.len()) as u32);
            if let (Some(src_uvs), Some(dst_uvs)) = (uvs.as_ref(), flat_uvs.as_mut()) {
                dst_uvs.push(src_uvs[src_index]);
            }

            for (name, values) in &mut flat_attributes {
                values.push(vertex_attributes[name][src_index]);
            }
            for (name, values) in &mut flat_vector_attributes {
                values.push(vertex_vector_attributes[name][src_index]);
            }
        }
    }

    let mut attributes: std::collections::HashMap<String, AttributeData> = flat_attributes
        .into_iter()
        .map(|(name, values)| (name, AttributeData::Vertex(values)))
        .collect();
    attributes.extend(
        flat_vector_attributes
            .into_iter()
            .map(|(name, values)| (name, AttributeData::VertexVector(values))),
    );
    attributes.extend(passthrough_attributes);

    let mut mesh = MeshData::default();
    mesh.positions = flat_positions;
    mesh.normals = flat_normals;
    mesh.indices = flat_indices;
    mesh.uvs = flat_uvs;
    mesh.attributes = attributes;
    mesh
}

fn normalized_grid_uv(i: usize, j: usize, width: usize, height: usize) -> [f32; 2] {
    let u = if width > 1 {
        i as f32 / (width - 1) as f32
    } else {
        0.0
    };
    let v = if height > 1 {
        j as f32 / (height - 1) as f32
    } else {
        0.0
    };
    [u, v]
}

fn compute_face_surface_quantities(
    positions: &[[f32; 3]],
    indices: &[u32],
    uvs: Option<&[[f32; 2]]>,
) -> (Vec<f32>, Vec<f32>) {
    let Some(uvs) = uvs else {
        let tri_count = indices.len() / 3;
        return (vec![0.0; tri_count], vec![0.0; tri_count]);
    };

    let mut angle_distortion = Vec::with_capacity(indices.len() / 3);
    let mut area_distortion = Vec::with_capacity(indices.len() / 3);

    for tri in indices.chunks_exact(3) {
        let world = [
            Vec3::from(positions[tri[0] as usize]),
            Vec3::from(positions[tri[1] as usize]),
            Vec3::from(positions[tri[2] as usize]),
        ];
        let uv = [
            glam::Vec2::from(uvs[tri[0] as usize]),
            glam::Vec2::from(uvs[tri[1] as usize]),
            glam::Vec2::from(uvs[tri[2] as usize]),
        ];

        let world_angles = triangle_angles_3d(world);
        let uv_angles = triangle_angles_2d(uv);
        angle_distortion.push(
            world_angles
                .iter()
                .zip(uv_angles.iter())
                .map(|(a, b)| (a - b).abs())
                .sum(),
        );

        let world_area = 0.5 * (world[1] - world[0]).cross(world[2] - world[0]).length();
        let uv_area = 0.5 * (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]).abs();
        area_distortion.push(if uv_area > 1e-8 {
            world_area / uv_area
        } else {
            0.0
        });
    }

    (angle_distortion, area_distortion)
}

fn triangle_angles_3d(points: [Vec3; 3]) -> [f32; 3] {
    [
        angle_between_3d(points[1] - points[0], points[2] - points[0]),
        angle_between_3d(points[0] - points[1], points[2] - points[1]),
        angle_between_3d(points[0] - points[2], points[1] - points[2]),
    ]
}

fn triangle_angles_2d(points: [glam::Vec2; 3]) -> [f32; 3] {
    [
        angle_between_2d(points[1] - points[0], points[2] - points[0]),
        angle_between_2d(points[0] - points[1], points[2] - points[1]),
        angle_between_2d(points[0] - points[2], points[1] - points[2]),
    ]
}

fn angle_between_3d(a: Vec3, b: Vec3) -> f32 {
    let denom = a.length() * b.length();
    if denom <= 1e-8 {
        0.0
    } else {
        (a.dot(b) / denom).clamp(-1.0, 1.0).acos()
    }
}

fn angle_between_2d(a: glam::Vec2, b: glam::Vec2) -> f32 {
    let denom = a.length() * b.length();
    if denom <= 1e-8 {
        0.0
    } else {
        (a.dot(b) / denom).clamp(-1.0, 1.0).acos()
    }
}
