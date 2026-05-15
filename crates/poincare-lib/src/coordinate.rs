use std::ops::RangeInclusive;

/// Coordinate system in which a plot's input parameters are expressed.
///
/// `generate()` always outputs Cartesian geometry — coordinate conversion
/// happens inside `generate()` and is invisible to the caller and the viewport.
pub enum CoordinateSystem {
    /// Standard x, y, z.
    Cartesian,
    /// Spherical: r = f(θ, φ) — θ elevation [0, π], φ azimuth [0, 2π].
    Spherical,
    /// Cylindrical: r = f(θ, z) — θ azimuth [0, 2π].
    Cylindrical,
    /// Polar: r = f(θ), embedded as z=0 surface in 3D.
    Polar,
    /// Parametric with an explicit domain.
    Parametric(ParametricDomain),
}

/// Domain type for parametric plots.
pub enum ParametricDomain {
    /// 1D curve parameterised by t.
    Curve { t: RangeInclusive<f64> },
    /// 2D surface parameterised by u and v.
    Surface {
        u: RangeInclusive<f64>,
        v: RangeInclusive<f64>,
    },
}

// ---------------------------------------------------------------------------
// Coordinate conversion functions
// ---------------------------------------------------------------------------

/// Convert spherical coordinates to Cartesian.
///
/// - `theta` — polar (elevation) angle in radians, measured from +Z axis, range [0, π].
/// - `phi`   — azimuthal angle in radians, range [0, 2π].
///
/// Returns `(x, y, z)`.
pub fn spherical_to_cartesian(r: f64, theta: f64, phi: f64) -> (f64, f64, f64) {
    (
        r * theta.sin() * phi.cos(),
        r * theta.sin() * phi.sin(),
        r * theta.cos(),
    )
}

/// Convert cylindrical coordinates to Cartesian.
///
/// - `theta` — azimuthal angle in radians, range [0, 2π].
/// - `z`     — height along the cylinder axis.
///
/// Returns `(x, y, z)`.
pub fn cylindrical_to_cartesian(r: f64, theta: f64, z: f64) -> (f64, f64, f64) {
    (r * theta.cos(), r * theta.sin(), z)
}

/// Convert polar coordinates to Cartesian, embedded at z=0.
///
/// - `theta` — azimuthal angle in radians, range [0, 2π].
///
/// Returns `(x, y, 0.0)`.
pub fn polar_to_cartesian(r: f64, theta: f64) -> (f64, f64, f64) {
    (r * theta.cos(), r * theta.sin(), 0.0)
}
