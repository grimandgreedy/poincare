use crate::{Domain, SeedMode};

/// Step-size configuration for finite difference operations.
///
/// The actual step used is `(scale * relative_step).clamp(min_step, max_step)`,
/// where `scale` is derived from the magnitude of any parameter values in play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteDifferenceConfig {
    pub relative_step: f64,
    pub min_step: f64,
    pub max_step: f64,
}

impl Default for FiniteDifferenceConfig {
    fn default() -> Self {
        Self {
            relative_step: 0.01,
            min_step: 1.0e-3,
            max_step: 0.25,
        }
    }
}

impl FiniteDifferenceConfig {
    pub fn step_for_parameters(self, parameters: &[(String, f64)]) -> f64 {
        let scale = parameters
            .iter()
            .map(|(_, value)| value.abs())
            .fold(1.0_f64, f64::max);
        (scale * self.relative_step).clamp(self.min_step, self.max_step)
    }
}

/// Estimate the gradient of a scalar field at `(x, y, z)` using central differences with step `h`.
pub fn finite_gradient(
    f: impl Fn(f64, f64, f64) -> f64,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> glam::Vec3 {
    let dx = (f(x + h, y, z) - f(x - h, y, z)) / (2.0 * h);
    let dy = (f(x, y + h, z) - f(x, y - h, z)) / (2.0 * h);
    let dz = (f(x, y, z + h) - f(x, y, z - h)) / (2.0 * h);
    glam::vec3(dx as f32, dy as f32, dz as f32)
}

/// Estimate the divergence of a vector field at `(x, y, z)` using central differences with step `h`.
pub fn finite_divergence(
    f: impl Fn(f64, f64, f64) -> glam::Vec3,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> f32 {
    let ddx = (f(x + h, y, z).x - f(x - h, y, z).x) / (2.0 * h as f32);
    let ddy = (f(x, y + h, z).y - f(x, y - h, z).y) / (2.0 * h as f32);
    let ddz = (f(x, y, z + h).z - f(x, y, z - h).z) / (2.0 * h as f32);
    ddx + ddy + ddz
}

/// Estimate the curl of a vector field at `(x, y, z)` using central differences with step `h`.
pub fn finite_curl(
    f: impl Fn(f64, f64, f64) -> glam::Vec3,
    x: f64,
    y: f64,
    z: f64,
    h: f64,
) -> glam::Vec3 {
    let inv = 1.0 / (2.0 * h as f32);
    let d_fz_dy = (f(x, y + h, z).z - f(x, y - h, z).z) * inv;
    let d_fy_dz = (f(x, y, z + h).y - f(x, y, z - h).y) * inv;
    let d_fx_dz = (f(x, y, z + h).x - f(x, y, z - h).x) * inv;
    let d_fz_dx = (f(x + h, y, z).z - f(x - h, y, z).z) * inv;
    let d_fy_dx = (f(x + h, y, z).y - f(x - h, y, z).y) * inv;
    let d_fx_dy = (f(x, y + h, z).x - f(x, y - h, z).x) * inv;
    glam::vec3(d_fz_dy - d_fy_dz, d_fx_dz - d_fz_dx, d_fy_dx - d_fx_dy)
}

/// Generate seed points for streamline integration according to the given mode and domain.
pub fn generate_seeds(mode: &SeedMode, domain: &Domain) -> Vec<glam::Vec3> {
    match mode {
        SeedMode::Grid { nx, ny, nz } => {
            let nx = (*nx).max(1) as usize;
            let ny = (*ny).max(1) as usize;
            let nz = (*nz).max(1) as usize;
            let x0 = *domain.x.start() as f32;
            let x1 = *domain.x.end() as f32;
            let y0 = *domain.y.start() as f32;
            let y1 = *domain.y.end() as f32;
            let z0 = *domain.z.start() as f32;
            let z1 = *domain.z.end() as f32;
            let mut seeds = Vec::with_capacity(nx * ny * nz);
            for iz in 0..nz {
                let z = if nz <= 1 {
                    (z0 + z1) * 0.5
                } else {
                    z0 + (z1 - z0) * iz as f32 / (nz - 1) as f32
                };
                for iy in 0..ny {
                    let y = if ny <= 1 {
                        (y0 + y1) * 0.5
                    } else {
                        y0 + (y1 - y0) * iy as f32 / (ny - 1) as f32
                    };
                    for ix in 0..nx {
                        let x = if nx <= 1 {
                            (x0 + x1) * 0.5
                        } else {
                            x0 + (x1 - x0) * ix as f32 / (nx - 1) as f32
                        };
                        seeds.push(glam::Vec3::new(x, y, z));
                    }
                }
            }
            seeds
        }
        SeedMode::Plane { axis, offset } => {
            let n = 5usize;
            let x0 = *domain.x.start() as f32;
            let x1 = *domain.x.end() as f32;
            let y0 = *domain.y.start() as f32;
            let y1 = *domain.y.end() as f32;
            let z0 = *domain.z.start() as f32;
            let z1 = *domain.z.end() as f32;
            let mut seeds = Vec::new();
            for j in 0..n {
                for i in 0..n {
                    let t0 = i as f32 / (n - 1) as f32;
                    let t1 = j as f32 / (n - 1) as f32;
                    let (x, y, z) = match axis {
                        0 => (*offset, y0 + (y1 - y0) * t0, z0 + (z1 - z0) * t1),
                        1 => (x0 + (x1 - x0) * t0, *offset, z0 + (z1 - z0) * t1),
                        _ => (x0 + (x1 - x0) * t0, y0 + (y1 - y0) * t1, *offset),
                    };
                    seeds.push(glam::Vec3::new(x, y, z));
                }
            }
            seeds
        }
        SeedMode::ManualCsv { csv_text } => match crate::parse_csv_points(csv_text) {
            Ok(pts) => pts
                .iter()
                .map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
                .collect(),
            Err(_) => Vec::new(),
        },
    }
}
