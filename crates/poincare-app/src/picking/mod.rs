use poincare_lib::ProbePickData;

/// A point picked by the coordinate probe tool.
pub(crate) struct ProbeHit {
    /// The position shown (live probe or locked snap point).
    pub(crate) world_pos: glam::Vec3,
    /// Local frame at the hit point.
    /// For surfaces: the face normal (pointing toward the camera).
    /// For polylines: the segment tangent direction.
    pub(crate) normal: glam::Vec3,
    /// Cursor is within snap radius of an intersection — hint only, not yet locked.
    pub(crate) near_snap: bool,
    /// User clicked to lock the probe to an intersection point.
    pub(crate) snapped: bool,
}

/// Cast a ray against all probe-pickable geometry.
///
/// Returns `(live_hit, snap_candidate)`:
/// - `live_hit`: `(world_pos, normal)` of the nearest hit.
/// - `snap_candidate`: midpoint of two surface hits within `snap_world_radius`, or `None`.
pub(crate) fn pick_probe(
    ray_orig: glam::Vec3,
    ray_dir: glam::Vec3,
    cursor_screen: glam::Vec2,
    data: &ProbePickData<'_>,
    view_proj: glam::Mat4,
    viewport_size: glam::Vec2,
    snap_world_radius: f32,
) -> (Option<(glam::Vec3, glam::Vec3)>, Option<glam::Vec3>) {
    let mut surface_hits: Vec<(f32, glam::Vec3, glam::Vec3)> = Vec::new();

    for surface in &data.surfaces {
        let positions = surface.positions;
        let mut best_t = f32::MAX;
        let mut best_pos = glam::Vec3::ZERO;
        let mut best_normal = glam::Vec3::Z;
        for tri in surface.indices.chunks_exact(3) {
            let v0 = glam::Vec3::from(positions[tri[0] as usize]);
            let v1 = glam::Vec3::from(positions[tri[1] as usize]);
            let v2 = glam::Vec3::from(positions[tri[2] as usize]);
            if let Some(t) = ray_triangle_t(ray_orig, ray_dir, v0, v1, v2) {
                if t < best_t {
                    best_t = t;
                    best_pos = ray_orig + ray_dir * t;
                    let n = (v1 - v0).cross(v2 - v0).normalize_or_zero();
                    best_normal = if n.dot(-ray_dir) >= 0.0 { n } else { -n };
                }
            }
        }
        if best_t < f32::MAX {
            surface_hits.push((best_t, best_pos, best_normal));
        }
    }
    surface_hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let snap_candidate = if surface_hits.len() >= 2 {
        let dist = surface_hits[0].1.distance(surface_hits[1].1);
        if dist <= snap_world_radius {
            Some((surface_hits[0].1 + surface_hits[1].1) * 0.5)
        } else {
            None
        }
    } else {
        None
    };

    let surface_live = surface_hits.first().map(|&(_, p, n)| (p, n));

    let polyline_screen_threshold = 12.0_f32;
    let mut best_poly_dist = polyline_screen_threshold;
    let mut best_poly_hit: Option<(glam::Vec3, glam::Vec3)> = None;

    for poly in &data.polylines {
        let positions = poly.positions;
        let mut offset = 0usize;
        for &len in poly.strip_lengths {
            let len = len as usize;
            for j in offset..offset + len.saturating_sub(1) {
                let a = positions[j];
                let b = positions[j + 1];
                let (world_pt, screen_dist) =
                    segment_screen_dist(a, b, cursor_screen, view_proj, viewport_size);
                if screen_dist < best_poly_dist {
                    best_poly_dist = screen_dist;
                    let tangent = (b - a).normalize_or_zero();
                    best_poly_hit = Some((world_pt, tangent));
                }
            }
            offset += len;
        }
    }

    let point_screen_threshold = 16.0_f32;
    let mut best_pt_dist = point_screen_threshold;
    let mut best_pt_pos: Option<glam::Vec3> = None;

    for pc in &data.points {
        for &pos in pc.positions {
            if let Some(sp) = world_to_screen(pos, view_proj, viewport_size) {
                let d = sp.distance(cursor_screen);
                if d < best_pt_dist {
                    best_pt_dist = d;
                    best_pt_pos = Some(pos);
                }
            }
        }
    }

    let live_hit = surface_live
        .or(best_poly_hit)
        .or_else(|| best_pt_pos.map(|p| (p, -ray_dir)));

    (live_hit, snap_candidate)
}

/// Möller–Trumbore ray-triangle intersection. Returns `t` along the ray, or `None`.
pub(crate) fn ray_triangle_t(
    orig: glam::Vec3,
    dir: glam::Vec3,
    v0: glam::Vec3,
    v1: glam::Vec3,
    v2: glam::Vec3,
) -> Option<f32> {
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let h = dir.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-8 {
        return None;
    }
    let f = 1.0 / a;
    let s = orig - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = f * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * e2.dot(q);
    if t > 1e-4 {
        Some(t)
    } else {
        None
    }
}

/// Find the closest point on segment `[a, b]` to `cursor_screen` (viewport-local pixels).
/// Returns `(world_point, screen_space_distance_in_pixels)`.
pub(crate) fn segment_screen_dist(
    a: glam::Vec3,
    b: glam::Vec3,
    cursor_screen: glam::Vec2,
    view_proj: glam::Mat4,
    viewport_size: glam::Vec2,
) -> (glam::Vec3, f32) {
    const SAMPLES: usize = 8;
    let mut best_t = 0.0_f32;
    let mut best_dist = f32::MAX;
    for k in 0..=SAMPLES {
        let t = k as f32 / SAMPLES as f32;
        let p = a + (b - a) * t;
        if let Some(sp) = world_to_screen(p, view_proj, viewport_size) {
            let d = sp.distance(cursor_screen);
            if d < best_dist {
                best_dist = d;
                best_t = t;
            }
        }
    }
    (a + (b - a) * best_t, best_dist)
}

/// Project a world-space point to viewport-local screen pixels.
/// Returns `None` if the point is outside the depth range.
pub(crate) fn world_to_screen(
    world: glam::Vec3,
    view_proj: glam::Mat4,
    viewport_size: glam::Vec2,
) -> Option<glam::Vec2> {
    let clip = view_proj.project_point3(world);
    if clip.z < 0.0 || clip.z > 1.0 {
        return None;
    }
    let x = (clip.x * 0.5 + 0.5) * viewport_size.x;
    let y = (1.0 - (clip.y * 0.5 + 0.5)) * viewport_size.y;
    Some(glam::Vec2::new(x, y))
}

/// Find the closest approach between two 3D line segments.
/// Returns `(point_on_a, point_on_b, distance)`.
pub(crate) fn segment_segment_closest(
    a0: glam::Vec3,
    a1: glam::Vec3,
    b0: glam::Vec3,
    b1: glam::Vec3,
) -> (glam::Vec3, glam::Vec3, f32) {
    let da = a1 - a0;
    let db = b1 - b0;
    let r = a0 - b0;

    let a = da.dot(da);
    let e = db.dot(db);
    let f = db.dot(r);

    let (s, t) = if a < 1e-8 && e < 1e-8 {
        (0.0_f32, 0.0_f32)
    } else if a < 1e-8 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = da.dot(r);
        if e < 1e-8 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = da.dot(db);
            let denom = a * e - b * b;
            let s = if denom.abs() > 1e-8 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t = (b * s + f) / e;
            let (s, t) = if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            };
            (s, t)
        }
    };

    let pa = a0 + da * s;
    let pb = b0 + db * t;
    (pa, pb, pa.distance(pb))
}
