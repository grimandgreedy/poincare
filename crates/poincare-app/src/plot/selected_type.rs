/// All plot types available in the "Add Plot" selector.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedPlotType {
    Auto,
    CartesianSurface,
    SphericalSurface,
    CylindricalSurface,
    PolarSurface,
    ParametricSurface,
    DataGridSurface,
    ParametricCurve,
    CurvePoints,
    Scatter,
    VectorField,
    Volume,
    Isosurface,
    Streamlines,
}

impl SelectedPlotType {
    pub(crate) fn all() -> &'static [Self] {
        &[
            Self::Auto,
            Self::CartesianSurface,
            Self::SphericalSurface,
            Self::CylindricalSurface,
            Self::PolarSurface,
            Self::ParametricSurface,
            Self::DataGridSurface,
            Self::ParametricCurve,
            Self::CurvePoints,
            Self::Scatter,
            Self::VectorField,
            Self::Volume,
            Self::Isosurface,
            Self::Streamlines,
        ]
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (detect from expression)",
            Self::CartesianSurface => "Cartesian Surface z=f(x,y)",
            Self::SphericalSurface => "Spherical Surface r=f(theta,phi)",
            Self::CylindricalSurface => "Cylindrical Surface r=f(theta,z)",
            Self::PolarSurface => "Polar Surface r=f(theta)",
            Self::ParametricSurface => "Parametric Surface (x,y,z)(u,v)",
            Self::DataGridSurface => "Data Grid Surface (CSV)",
            Self::ParametricCurve => "Parametric Curve (x,y,z)(t)",
            Self::CurvePoints => "Curve Points (CSV)",
            Self::Scatter => "Scatter (CSV)",
            Self::VectorField => "Vector Field (vx,vy,vz)(x,y,z)",
            Self::Volume => "Volume Density f(x,y,z)",
            Self::Isosurface => "Isosurface f(x,y,z)=levels",
            Self::Streamlines => "Streamlines (vx,vy,vz)(x,y,z)",
        }
    }
}
