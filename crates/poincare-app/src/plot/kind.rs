use poincare_lib::{CurveInterpolation, DomainEditorMetadata, PlotMetadata, StyleCapabilities};

/// Default palette for isosurface per-level colours.
pub(crate) const DEFAULT_ISO_PALETTE: [[f32; 4]; 6] = [
    [0.2, 0.6, 1.0, 0.7],
    [1.0, 0.4, 0.2, 0.7],
    [0.2, 0.9, 0.4, 0.7],
    [0.9, 0.8, 0.1, 0.7],
    [0.7, 0.2, 0.9, 0.7],
    [0.1, 0.9, 0.9, 0.7],
];

pub(crate) use poincare_lib::{PlotDefinition as PlotKind, SeedMode};

pub(crate) trait PlotKindExt {
    fn style_caps(&self) -> StyleCapabilities;
    fn domain_editor(&self) -> DomainEditorMetadata;
    fn uses_resolution(&self) -> bool;
    fn uses_seed_resolution(&self) -> bool;
    fn supports_surface_intersection(&self) -> bool;
    fn parameters_mut(&mut self) -> Option<&mut Vec<(String, f64)>>;
}

impl PlotKindExt for PlotKind {
    fn style_caps(&self) -> StyleCapabilities {
        fn metadata(kind: &PlotKind) -> PlotMetadata {
            PlotKind::metadata(kind)
        }
        metadata(self).style_caps
    }

    fn domain_editor(&self) -> DomainEditorMetadata {
        fn metadata(kind: &PlotKind) -> PlotMetadata {
            PlotKind::metadata(kind)
        }
        metadata(self).domain_editor
    }

    fn uses_resolution(&self) -> bool {
        fn metadata(kind: &PlotKind) -> PlotMetadata {
            PlotKind::metadata(kind)
        }
        metadata(self).uses_resolution
    }

    fn uses_seed_resolution(&self) -> bool {
        fn metadata(kind: &PlotKind) -> PlotMetadata {
            PlotKind::metadata(kind)
        }
        metadata(self).uses_seed_resolution
    }

    fn supports_surface_intersection(&self) -> bool {
        fn metadata(kind: &PlotKind) -> PlotMetadata {
            PlotKind::metadata(kind)
        }
        metadata(self).supports_surface_intersection
    }

    fn parameters_mut(&mut self) -> Option<&mut Vec<(String, f64)>> {
        match self {
            Self::ExprCartesian { parameters, .. }
            | Self::ExprCurve { parameters, .. }
            | Self::ExprCartesianLine { parameters, .. }
            | Self::ExprSpherical { parameters, .. }
            | Self::ExprCylindrical { parameters, .. }
            | Self::ExprPolar { parameters, .. }
            | Self::ExprParametricSurface { parameters, .. }
            | Self::ScalarSlice { parameters, .. }
            | Self::VectorSlice { parameters, .. }
            | Self::GradientField { parameters, .. }
            | Self::DivergenceField { parameters, .. }
            | Self::CurlField { parameters, .. }
            | Self::ExprVectorField { parameters, .. }
            | Self::ExprVolume { parameters, .. }
            | Self::ExprIsosurface { parameters, .. }
            | Self::ExprStreamlines { parameters, .. } => Some(parameters),
            _ => None,
        }
    }
}

pub(crate) fn evenly_spaced_isovalues(count: usize) -> Vec<f32> {
    let count = count.max(1);
    if count == 1 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| -0.9 + 1.8 * i as f32 / (count - 1) as f32)
        .collect()
}

#[allow(dead_code)]
fn _keep_curve_interpolation(_value: CurveInterpolation) {}
