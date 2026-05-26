use std::collections::HashMap;
use std::f64::consts::PI;

use glam::Vec3;

use crate::{
    ArrowAnnotation, ColourMode, CurveInterpolation, CurveInterpolationKind, Diagnostic,
    DiagnosticKind, DiagnosticLocation, PlotMetadata, PlotSpec, PlotStyle, PointAnnotation,
    SliceAxis, TableDataSet, default_slice_position, eval_curve_point, eval_with_vars,
    parse_curve_expr, parse_expr_with_vars, sample_curve_points,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisKind {
    PointCloudStatistics,
    DataQualityChecks,
    InterpolateCurve,
    FitCurve,
    DifferentiateCurve,
    AxisDerivativeCurve,
    IntegralCurve,
    ArcLengthCurve,
    CurvatureCurve,
    TangentField,
    NormalField,
    BinormalField,
    ExtractPoints,
    ScalarSlice,
    VectorSlice,
    GradientField,
    DivergenceField,
    CurlField,
    SurfaceIntersection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisOutputKind {
    PlotSpec,
    NumericReport,
    Table,
    Composite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisTargetKind {
    Definition,
    SampledData,
    Geometry,
    PlotPair,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisCapability {
    pub kind: AnalysisKind,
    pub target_kind: AnalysisTargetKind,
    pub output_kind: AnalysisOutputKind,
    pub parameters: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnalysisTarget {
    Plot { index: usize, name: Option<String> },
    PlotPair { first: usize, second: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisRequest {
    pub kind: AnalysisKind,
    pub target: AnalysisTarget,
    pub parameters: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisProvenance {
    pub kind: AnalysisKind,
    pub source_plots: Vec<String>,
    pub parameters: Vec<(String, String)>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisReport {
    pub title: String,
    pub values: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug)]
pub enum AnalysisOutput {
    DerivedPlots {
        plots: Vec<PlotSpec>,
        provenance: AnalysisProvenance,
    },
    Report {
        report: AnalysisReport,
        provenance: AnalysisProvenance,
    },
    Table {
        table: AnalysisTable,
        provenance: AnalysisProvenance,
    },
    Composite {
        plots: Vec<PlotSpec>,
        reports: Vec<AnalysisReport>,
        tables: Vec<AnalysisTable>,
        diagnostics: Vec<Diagnostic>,
        provenance: AnalysisProvenance,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleGroupsKind {
    Curve,
    Polyline,
    InterpolationSource,
    SampleData,
}

#[derive(Clone, Debug)]
pub struct AnalysisError {
    pub diagnostic: Diagnostic,
}

impl AnalysisError {
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            diagnostic: Diagnostic::error(DiagnosticKind::Build, message),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            diagnostic: Diagnostic::error(DiagnosticKind::Validation, message),
        }
    }
}

pub fn available_analyses(plot: &PlotSpec) -> Vec<AnalysisCapability> {
    let metadata = plot.metadata();
    let mut capabilities = capabilities_for_metadata(&metadata);
    if supports_point_cloud_statistics(plot) {
        capabilities.push(AnalysisCapability {
            kind: AnalysisKind::PointCloudStatistics,
            target_kind: AnalysisTargetKind::SampledData,
            output_kind: AnalysisOutputKind::Composite,
            parameters: vec![],
        });
    }
    if supports_data_quality_analysis(plot) {
        capabilities.push(AnalysisCapability {
            kind: AnalysisKind::DataQualityChecks,
            target_kind: AnalysisTargetKind::SampledData,
            output_kind: AnalysisOutputKind::Composite,
            parameters: vec![],
        });
    }
    capabilities
}

pub fn sample_groups(
    plot: &PlotSpec,
    kind: SampleGroupsKind,
) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match kind {
        SampleGroupsKind::Curve => curve_sample_groups(plot),
        SampleGroupsKind::Polyline => polyline_sample_groups(plot),
        SampleGroupsKind::InterpolationSource => interpolation_source_groups(plot),
        SampleGroupsKind::SampleData => sample_data_groups(plot),
    }
}

pub fn run_analysis(
    plot: &PlotSpec,
    request: &AnalysisRequest,
) -> Result<AnalysisOutput, AnalysisError> {
    let params = parameter_map(&request.parameters);
    let provenance = AnalysisProvenance {
        kind: request.kind,
        source_plots: vec![plot.name.clone()],
        parameters: request.parameters.clone(),
        notes: Vec::new(),
    };

    let plots = match request.kind {
        AnalysisKind::PointCloudStatistics => return make_point_cloud_statistics_output(plot),
        AnalysisKind::DataQualityChecks => return make_data_quality_output(plot),
        AnalysisKind::ScalarSlice => vec![make_scalar_slice_plot(
            plot,
            parse_axis(params.get("axis").map(String::as_str)).unwrap_or(SliceAxis::Z),
            params
                .get("position")
                .and_then(|value| value.parse::<f64>().ok()),
            params
                .get("contours")
                .and_then(|value| value.parse::<usize>().ok()),
        )?],
        AnalysisKind::VectorSlice => vec![make_vector_slice_plot(
            plot,
            parse_axis(params.get("axis").map(String::as_str)).unwrap_or(SliceAxis::Z),
            params
                .get("position")
                .and_then(|value| value.parse::<f64>().ok()),
        )?],
        AnalysisKind::GradientField => vec![make_gradient_plot(plot)?],
        AnalysisKind::DivergenceField => vec![make_divergence_plot(plot)?],
        AnalysisKind::CurlField => vec![make_curl_plot(plot)?],
        AnalysisKind::DifferentiateCurve => vec![make_curve_derivative_plot(plot)?],
        AnalysisKind::FitCurve => {
            return make_curve_fit_output(plot, build_curve_fit_options(&params));
        }
        AnalysisKind::AxisDerivativeCurve => vec![make_axis_derivative_plot(
            plot,
            parse_axis_index(params.get("numerator_axis").map(String::as_str)).unwrap_or(1),
            parse_axis_index(params.get("denominator_axis").map(String::as_str)).unwrap_or(0),
            params.get("output_name").cloned(),
        )?],
        AnalysisKind::IntegralCurve => vec![make_curve_integral_plot(
            plot,
            params
                .get("normalize_integral")
                .is_none_or(|value| matches!(value.as_str(), "1" | "true" | "yes")),
        )?],
        AnalysisKind::ArcLengthCurve => vec![make_curve_arc_length_plot(plot)?],
        AnalysisKind::CurvatureCurve => vec![make_curve_curvature_plot(plot)?],
        AnalysisKind::TangentField => vec![make_curve_tangent_plot(plot)?],
        AnalysisKind::NormalField => vec![make_curve_normal_plot(plot)?],
        AnalysisKind::BinormalField => vec![make_curve_binormal_plot(plot)?],
        AnalysisKind::ExtractPoints => vec![make_extracted_points_plot(plot)?],
        AnalysisKind::InterpolateCurve => make_interpolated_plots(
            plot,
            build_interpolation(&params),
            params.get("output_name").cloned(),
        )?,
        AnalysisKind::SurfaceIntersection => {
            return Err(AnalysisError::unsupported(
                "Surface intersection remains a geometry-level app workflow.",
            ));
        }
    };

    Ok(AnalysisOutput::DerivedPlots { plots, provenance })
}

fn supports_point_cloud_statistics(plot: &PlotSpec) -> bool {
    sample_data_groups(plot)
        .map(|groups| groups.iter().map(Vec::len).sum::<usize>() >= 1)
        .unwrap_or(false)
}

fn supports_data_quality_analysis(plot: &PlotSpec) -> bool {
    sample_data_groups(plot)
        .map(|groups| groups.iter().map(Vec::len).sum::<usize>() >= 2)
        .unwrap_or(false)
}

fn capabilities_for_metadata(metadata: &PlotMetadata) -> Vec<AnalysisCapability> {
    let mut capabilities = Vec::new();

    if metadata.style_caps.line {
        capabilities.extend([
            AnalysisCapability {
                kind: AnalysisKind::InterpolateCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![
                    "output_name",
                    "interpolation_kind",
                    "samples_per_segment",
                    "closed",
                    "smoothing_window",
                ],
            },
            AnalysisCapability {
                kind: AnalysisKind::FitCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::Composite,
                parameters: vec![
                    "fit_method",
                    "output_name",
                    "degree",
                    "harmonics",
                    "smoothing_window",
                    "samples_per_segment",
                    "show_control_points",
                    "show_residual_plot",
                ],
            },
            AnalysisCapability {
                kind: AnalysisKind::DifferentiateCurve,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::AxisDerivativeCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["numerator_axis", "denominator_axis", "output_name"],
            },
            AnalysisCapability {
                kind: AnalysisKind::IntegralCurve,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["normalize_integral"],
            },
            AnalysisCapability {
                kind: AnalysisKind::ArcLengthCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::CurvatureCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::TangentField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::NormalField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::BinormalField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::ExtractPoints,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
        ]);
    }

    if metadata.coordinate_semantics == crate::CoordinateSemantics::CartesianVolume {
        capabilities.extend([
            AnalysisCapability {
                kind: AnalysisKind::ScalarSlice,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["axis", "position", "contours"],
            },
            AnalysisCapability {
                kind: AnalysisKind::VectorSlice,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["axis", "position"],
            },
        ]);
    }

    if metadata.required_variables == ["x".to_string(), "y".to_string(), "z".to_string()] {
        capabilities.extend([
            AnalysisCapability {
                kind: AnalysisKind::GradientField,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::DivergenceField,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
            AnalysisCapability {
                kind: AnalysisKind::CurlField,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec![],
            },
        ]);
    }

    if metadata.supports_surface_intersection {
        capabilities.push(AnalysisCapability {
            kind: AnalysisKind::SurfaceIntersection,
            target_kind: AnalysisTargetKind::PlotPair,
            output_kind: AnalysisOutputKind::PlotSpec,
            parameters: vec!["samples", "tolerance"],
        });
    }

    capabilities
}

fn make_scalar_slice_plot(
    source: &PlotSpec,
    axis: SliceAxis,
    position: Option<f64>,
    contour_count: Option<usize>,
) -> Result<PlotSpec, AnalysisError> {
    let (expression, parameters) = match &source.definition {
        crate::PlotDefinition::ExprVolume {
            expression,
            parameters,
            ..
        }
        | crate::PlotDefinition::ExprIsosurface {
            expression,
            parameters,
            ..
        } => (expression.clone(), parameters.clone()),
        _ => {
            return Err(AnalysisError::unsupported(
                "Scalar slices require a scalar volume or isosurface source.",
            ));
        }
    };
    Ok(PlotSpec {
        name: format!("{} Slice {}", axis.label(), source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::ByAttribute {
                name: "value".to_string(),
                kind: viewport_lib::AttributeKind::Vertex,
            },
            two_sided: true,
            ..source.style.clone()
        },
        definition: crate::PlotDefinition::ScalarSlice {
            expression,
            parameters,
            axis,
            position: position.unwrap_or_else(|| default_slice_position(&source.domain, axis)),
            contour_values: evenly_spaced_isovalues(contour_count.unwrap_or(8)),
            contour_style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.95, 0.35, 1.0]),
                line_width: 2.0,
                ..PlotStyle::default()
            },
        },
    })
}

fn make_vector_slice_plot(
    source: &PlotSpec,
    axis: SliceAxis,
    position: Option<f64>,
) -> Result<PlotSpec, AnalysisError> {
    let (expression, parameters) = match &source.definition {
        crate::PlotDefinition::ExprVectorField {
            expression,
            parameters,
        } => (expression.clone(), parameters.clone()),
        _ => {
            return Err(AnalysisError::unsupported(
                "Vector slices require a vector field source.",
            ));
        }
    };
    Ok(PlotSpec {
        name: format!("{} Slice {}", axis.label(), source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: source.style.clone(),
        definition: crate::PlotDefinition::VectorSlice {
            expression,
            parameters,
            axis,
            position: position.unwrap_or_else(|| default_slice_position(&source.domain, axis)),
        },
    })
}

fn make_gradient_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let (expression, parameters) = match &source.definition {
        crate::PlotDefinition::ExprVolume {
            expression,
            parameters,
            ..
        }
        | crate::PlotDefinition::ExprIsosurface {
            expression,
            parameters,
            ..
        } => (expression.clone(), parameters.clone()),
        _ => {
            return Err(AnalysisError::unsupported(
                "Gradient plots require a scalar volume or isosurface source.",
            ));
        }
    };
    Ok(PlotSpec {
        name: format!("Gradient {}", source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::ByAttribute {
                name: "magnitude".to_string(),
                kind: viewport_lib::AttributeKind::Vertex,
            },
            glyph_scale: 0.8,
            shading: crate::ShadingMode::Unlit,
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::GradientField {
            expression,
            parameters,
        },
    })
}

fn make_divergence_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let (expression, parameters) = match &source.definition {
        crate::PlotDefinition::ExprVectorField {
            expression,
            parameters,
        } => (expression.clone(), parameters.clone()),
        _ => {
            return Err(AnalysisError::unsupported(
                "Divergence plots require a vector field source.",
            ));
        }
    };
    Ok(PlotSpec {
        name: format!("Divergence {}", source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            opacity: 0.3,
            transfer_function: Some(crate::TransferFunction {
                opacity_scale: 0.4,
                threshold: None,
            }),
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::DivergenceField {
            expression,
            parameters,
            vol_resolution: [64, 64, 64],
        },
    })
}

fn make_curl_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let (expression, parameters) = match &source.definition {
        crate::PlotDefinition::ExprVectorField {
            expression,
            parameters,
        } => (expression.clone(), parameters.clone()),
        _ => {
            return Err(AnalysisError::unsupported(
                "Curl plots require a vector field source.",
            ));
        }
    };
    Ok(PlotSpec {
        name: format!("Curl {}", source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::ByAttribute {
                name: "magnitude".to_string(),
                kind: viewport_lib::AttributeKind::Vertex,
            },
            glyph_scale: 0.8,
            shading: crate::ShadingMode::Unlit,
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::CurlField {
            expression,
            parameters,
        },
    })
}

fn make_curve_derivative_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => groups
            .iter()
            .map(|group| derivative_cartesian_line_group(group, dep_var.as_str(), ind_var.as_str()))
            .filter(|group| group.len() >= 2)
            .collect(),
        _ => groups
            .iter()
            .map(|group| derivative_curve_group(group))
            .filter(|group| group.len() >= 2)
            .collect(),
    };
    derived_polyline_plot(
        source,
        format!("Derivative {}", source.name),
        [1.0, 0.55, 0.25, 1.0],
        derived_groups,
    )
}

fn make_curve_tangent_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = groups
        .iter()
        .map(|group| tangent_curve_group(group))
        .filter(|group| group.len() >= 2)
        .collect();
    derived_polyline_plot(
        source,
        format!("Tangent {}", source.name),
        [0.25, 0.85, 0.45, 1.0],
        derived_groups,
    )
}

fn make_curve_integral_plot(
    source: &PlotSpec,
    normalize_integral: bool,
) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => groups
            .iter()
            .map(|group| {
                integral_cartesian_line_group(
                    group,
                    dep_var.as_str(),
                    ind_var.as_str(),
                    normalize_integral,
                )
            })
            .filter(|group| group.len() >= 2)
            .collect(),
        _ => groups
            .iter()
            .map(|group| integral_curve_group(group, normalize_integral))
            .filter(|group| group.len() >= 2)
            .collect(),
    };
    derived_polyline_plot(
        source,
        format!("Integral {}", source.name),
        [0.45, 0.7, 1.0, 1.0],
        derived_groups,
    )
}

fn make_curve_arc_length_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => groups
            .iter()
            .map(|group| {
                scalar_plot_cartesian_line_group(
                    group,
                    dep_var.as_str(),
                    ind_var.as_str(),
                    &cumulative_arc_lengths(group),
                )
            })
            .filter(|group| group.len() >= 2)
            .collect(),
        _ => groups
            .iter()
            .map(|group| scalar_curve_group(group, &cumulative_arc_lengths(group)))
            .filter(|group| group.len() >= 2)
            .collect(),
    };
    derived_polyline_plot(
        source,
        format!("Arc Length {}", source.name),
        [0.95, 0.85, 0.3, 1.0],
        derived_groups,
    )
}

fn make_curve_curvature_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => groups
            .iter()
            .map(|group| {
                scalar_plot_cartesian_line_group(
                    group,
                    dep_var.as_str(),
                    ind_var.as_str(),
                    &curvature_values(group),
                )
            })
            .filter(|group| group.len() >= 2)
            .collect(),
        _ => groups
            .iter()
            .map(|group| scalar_curve_group(group, &curvature_values(group)))
            .filter(|group| group.len() >= 2)
            .collect(),
    };
    derived_polyline_plot(
        source,
        format!("Curvature {}", source.name),
        [0.8, 0.45, 1.0, 1.0],
        derived_groups,
    )
}

fn make_curve_normal_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = groups
        .iter()
        .map(|group| normal_curve_group(group))
        .filter(|group| group.len() >= 2)
        .collect();
    derived_polyline_plot(
        source,
        format!("Normal {}", source.name),
        [0.3, 0.8, 1.0, 1.0],
        derived_groups,
    )
}

fn make_curve_binormal_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = groups
        .iter()
        .map(|group| binormal_curve_group(group))
        .filter(|group| group.len() >= 2)
        .collect();
    derived_polyline_plot(
        source,
        format!("Binormal {}", source.name),
        [1.0, 0.45, 0.65, 1.0],
        derived_groups,
    )
}

fn make_axis_derivative_plot(
    source: &PlotSpec,
    numerator_axis: usize,
    denominator_axis: usize,
    output_name: Option<String>,
) -> Result<PlotSpec, AnalysisError> {
    if numerator_axis == denominator_axis {
        return Err(AnalysisError::invalid(
            "Numerator and denominator axes must be different.",
        ));
    }
    let groups = curve_sample_groups(source)?;
    let derived_groups: Vec<Vec<[f32; 3]>> = groups
        .iter()
        .map(|group| axis_derivative_group(group, numerator_axis, denominator_axis))
        .filter(|group| group.len() >= 2)
        .collect();
    if derived_groups.is_empty() {
        return Err(AnalysisError::invalid(
            "Could not compute an axis derivative from the selected curve.",
        ));
    }
    derived_polyline_plot(
        source,
        output_name.unwrap_or_else(|| {
            format!(
                "d{}/d{} {}",
                axis_name(numerator_axis),
                axis_name(denominator_axis),
                source.name
            )
        }),
        [1.0, 0.7, 0.25, 1.0],
        derived_groups,
    )
}

fn make_extracted_points_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let positions = polyline_sample_groups(source)?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(PlotSpec {
        name: format!("Points from {}", source.name),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::Solid([0.35, 0.85, 1.0, 1.0]),
            point_size: 8.0,
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::PointAnnotations {
            points: make_point_annotations(&positions, "Point"),
            show_labels: false,
        },
    })
}

fn make_interpolated_plots(
    source: &PlotSpec,
    interpolation: CurveInterpolation,
    output_name: Option<String>,
) -> Result<Vec<PlotSpec>, AnalysisError> {
    let groups = interpolation_source_groups(source)?;
    if groups.is_empty() || groups.iter().all(Vec::is_empty) {
        return Err(AnalysisError::invalid(
            "The selected plot does not have usable point samples.",
        ));
    }
    if groups.iter().all(|group| group.len() < 2) {
        return Err(AnalysisError::invalid(
            "At least two points are required to interpolate a curve.",
        ));
    }

    let base_name = output_name.unwrap_or_else(|| format!("Interpolated {}", source.name));
    let style = PlotStyle {
        colour_mode: ColourMode::Solid([0.95, 0.7, 0.2, 1.0]),
        line_width: 2.5,
        ..PlotStyle::default()
    };

    if groups.len() == 1 {
        let points = groups.into_iter().next().unwrap_or_default();
        return Ok(vec![PlotSpec {
            name: base_name,
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style,
            definition: crate::PlotDefinition::InterpolatedCurve {
                points,
                interpolation,
            },
        }]);
    }

    Ok(groups
        .into_iter()
        .enumerate()
        .filter(|(_, group)| group.len() >= 2)
        .map(|(index, group)| PlotSpec {
            name: format!("{base_name} {}", index + 1),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: style.clone(),
            definition: crate::PlotDefinition::InterpolatedCurve {
                points: group,
                interpolation,
            },
        })
        .collect())
}

fn interpolation_source_groups(plot: &PlotSpec) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match &plot.definition {
        crate::PlotDefinition::PointAnnotations { points, .. } => {
            Ok(vec![points.iter().map(|point| point.position).collect()])
        }
        crate::PlotDefinition::ExprCurve { .. }
        | crate::PlotDefinition::ExprCartesianLine { .. }
        | crate::PlotDefinition::HelixCurve => curve_sample_groups(plot),
        crate::PlotDefinition::ImportedTable { definition } => match definition.validate() {
            Ok(TableDataSet::Curve { groups, .. }) => Ok(groups
                .iter()
                .map(|group| group.iter().map(|point| point.to_array()).collect())
                .collect()),
            Ok(TableDataSet::Scatter { points, .. }) => {
                Ok(vec![points.iter().map(|point| point.to_array()).collect()])
            }
            Ok(_) => Err(AnalysisError::unsupported(
                "Interpolation is not available for this imported table target.",
            )),
            Err(errors) => Err(table_errors(errors)),
        },
        crate::PlotDefinition::DerivedPolylineGroups { groups } => Ok(groups.clone()),
        crate::PlotDefinition::InterpolatedCurve { points, .. } => Ok(vec![points.clone()]),
        _ => Err(AnalysisError::unsupported(
            "Interpolation is available for point and ordered sample plots.",
        )),
    }
}

fn polyline_sample_groups(plot: &PlotSpec) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match &plot.definition {
        crate::PlotDefinition::ExprCurve { .. }
        | crate::PlotDefinition::ExprCartesianLine { .. }
        | crate::PlotDefinition::HelixCurve => curve_sample_groups(plot),
        crate::PlotDefinition::ImportedTable { definition } => match definition.validate() {
            Ok(TableDataSet::Curve { groups, .. }) => Ok(groups
                .iter()
                .map(|group| group.iter().map(|point| point.to_array()).collect())
                .collect()),
            Ok(_) => Err(AnalysisError::unsupported(
                "Point extraction is only available for imported curve tables.",
            )),
            Err(errors) => Err(table_errors(errors)),
        },
        crate::PlotDefinition::DerivedPolylineGroups { groups } => Ok(groups.clone()),
        crate::PlotDefinition::InterpolatedCurve {
            points,
            interpolation,
        } => {
            let sampled = sample_curve_points(
                &points
                    .iter()
                    .map(|point| Vec3::from_array(*point))
                    .collect::<Vec<_>>(),
                *interpolation,
            );
            Ok(vec![
                sampled.into_iter().map(|point| point.to_array()).collect(),
            ])
        }
        _ => Err(AnalysisError::unsupported(
            "Point extraction is available for polyline and interpolated curve plots.",
        )),
    }
}

fn sample_data_groups(plot: &PlotSpec) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match &plot.definition {
        crate::PlotDefinition::ScatterCloud => Ok(vec![scatter_cloud_points()]),
        crate::PlotDefinition::PointAnnotations { points, .. } => {
            Ok(vec![points.iter().map(|point| point.position).collect()])
        }
        crate::PlotDefinition::ArrowAnnotations { arrows, .. } => {
            Ok(vec![arrows.iter().map(|arrow| arrow.origin).collect()])
        }
        crate::PlotDefinition::ExprCurve { .. }
        | crate::PlotDefinition::ExprCartesianLine { .. }
        | crate::PlotDefinition::HelixCurve => curve_sample_groups(plot),
        crate::PlotDefinition::ImportedTable { definition } => match definition.validate() {
            Ok(TableDataSet::Curve { groups, .. }) => Ok(groups
                .iter()
                .map(|group| group.iter().map(|point| point.to_array()).collect())
                .collect()),
            Ok(TableDataSet::Scatter { points, .. }) => {
                Ok(vec![points.iter().map(|point| point.to_array()).collect()])
            }
            Ok(TableDataSet::VectorField { samples, .. }) => Ok(vec![
                samples
                    .iter()
                    .map(|sample| sample.position.to_array())
                    .collect(),
            ]),
            Ok(TableDataSet::SurfaceGrid { xs, ys, zs }) => {
                if xs.is_empty() || ys.is_empty() || zs.is_empty() {
                    return Err(AnalysisError::invalid(
                        "Imported surface grid does not contain enough samples.",
                    ));
                }
                let ny = ys.len();
                let zs_ref = &zs;
                let points = xs
                    .iter()
                    .enumerate()
                    .flat_map(|(ix, x)| {
                        ys.iter().enumerate().filter_map(move |(iy, y)| {
                            let index = ix.checked_mul(ny)?.checked_add(iy)?;
                            let z = *zs_ref.get(index)?;
                            Some([*x as f32, *y as f32, z as f32])
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(vec![points])
            }
            Err(errors) => Err(table_errors(errors)),
        },
        crate::PlotDefinition::DerivedPolylineGroups { groups } => Ok(groups.clone()),
        crate::PlotDefinition::InterpolatedCurve {
            points,
            interpolation,
        } => {
            let sampled = sample_curve_points(
                &points
                    .iter()
                    .map(|point| Vec3::from_array(*point))
                    .collect::<Vec<_>>(),
                *interpolation,
            );
            Ok(vec![
                sampled.into_iter().map(|point| point.to_array()).collect(),
            ])
        }
        _ => Err(AnalysisError::unsupported(
            "Sample statistics require point-like or ordered sampled data.",
        )),
    }
}

fn curve_sample_groups(plot: &PlotSpec) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match &plot.definition {
        crate::PlotDefinition::HelixCurve => {
            let steps = plot.resolution.u.max(2) as usize;
            let points = (0..steps)
                .map(|i| {
                    let t = 20.0 * PI * i as f64 / (steps - 1) as f64;
                    Vec3::new(
                        (t.cos() * 3.0) as f32,
                        (t.sin() * 3.0) as f32,
                        (t * 0.15) as f32,
                    )
                    .to_array()
                })
                .collect();
            Ok(vec![points])
        }
        crate::PlotDefinition::ExprCurve {
            expression,
            parameters,
            t_range,
        } => {
            let parsed = parse_curve_expr(expression).map_err(parse_error)?;
            let steps = plot.resolution.u.max(2) as usize;
            let (t0, t1) = *t_range;
            let points = (0..steps)
                .map(|i| {
                    let t = t0 + (i as f64 / (steps - 1) as f64) * (t1 - t0);
                    let p = eval_curve_point(&parsed, t, parameters);
                    Vec3::new(p.x as f32, p.y as f32, p.z as f32).to_array()
                })
                .collect();
            Ok(vec![points])
        }
        crate::PlotDefinition::ExprCartesianLine {
            dep_var,
            ind_var,
            expression,
            parameters,
        } => {
            let parsed =
                parse_expr_with_vars(expression, &[ind_var.as_str()]).map_err(parse_error)?;
            let steps = plot.resolution.u.max(2) as usize;
            let (t0, t1) = (*plot.domain.x.start(), *plot.domain.x.end());
            let dep = dep_var.clone();
            let ind = ind_var.clone();
            let points = (0..steps)
                .map(|i| {
                    let t = t0 + (i as f64 / (steps - 1) as f64) * (t1 - t0);
                    let vars: Vec<(&str, f64)> = parameters
                        .iter()
                        .map(|(n, v)| (n.as_str(), *v))
                        .chain(std::iter::once((ind.as_str(), t)))
                        .collect();
                    let val = eval_with_vars(&parsed, &vars);
                    cartesian_line_point(dep.as_str(), ind.as_str(), t as f32, val as f32)
                        .to_array()
                })
                .collect();
            Ok(vec![points])
        }
        crate::PlotDefinition::ImportedTable { definition } => match definition.validate() {
            Ok(TableDataSet::Curve { groups, .. }) => Ok(groups
                .iter()
                .map(|group| group.iter().map(|point| point.to_array()).collect())
                .collect()),
            Ok(_) => Err(AnalysisError::unsupported(
                "Curve calculus tools require curve-like sample data.",
            )),
            Err(errors) => Err(table_errors(errors)),
        },
        crate::PlotDefinition::DerivedPolylineGroups { groups } => Ok(groups.clone()),
        crate::PlotDefinition::InterpolatedCurve {
            points,
            interpolation,
        } => {
            let sampled = sample_curve_points(
                &points
                    .iter()
                    .map(|point| Vec3::from_array(*point))
                    .collect::<Vec<_>>(),
                *interpolation,
            );
            Ok(vec![
                sampled.into_iter().map(|point| point.to_array()).collect(),
            ])
        }
        _ => Err(AnalysisError::unsupported(
            "Curve calculus tools are available for curve and polyline plots.",
        )),
    }
}

fn make_point_cloud_statistics_output(plot: &PlotSpec) -> Result<AnalysisOutput, AnalysisError> {
    let groups = sample_data_groups(plot)?;
    let samples = flatten_sample_groups(&groups);
    if samples.is_empty() {
        return Err(AnalysisError::invalid(
            "Point-cloud statistics require at least one sample.",
        ));
    }

    let centroid = samples.iter().copied().sum::<Vec3>() / samples.len() as f32;
    let (bbox_min, bbox_max) = bounds_for_points(&samples);
    let extent = bbox_max - bbox_min;
    let covariance = covariance_matrix(&samples, centroid);
    let variance = Vec3::new(
        covariance.x_axis.x,
        covariance.y_axis.y,
        covariance.z_axis.z,
    );
    let principal_components = principal_components(covariance);

    let reports = vec![AnalysisReport {
        title: format!("Point Statistics {}", plot.name),
        values: vec![
            ("Sample Count".to_string(), samples.len().to_string()),
            ("Sequence Count".to_string(), groups.len().to_string()),
            ("Centroid".to_string(), format_vec3(centroid)),
            ("Bounds Min".to_string(), format_vec3(bbox_min)),
            ("Bounds Max".to_string(), format_vec3(bbox_max)),
            ("Extent".to_string(), format_vec3(extent)),
            (
                "Variance".to_string(),
                format!("{:.5}, {:.5}, {:.5}", variance.x, variance.y, variance.z),
            ),
        ],
    }];
    let tables = vec![
        sample_positions_table(&groups),
        AnalysisTable {
            columns: vec![
                "axis".to_string(),
                "x".to_string(),
                "y".to_string(),
                "z".to_string(),
            ],
            rows: vec![
                vec![
                    "covariance row 1".to_string(),
                    format_float(covariance.x_axis.x),
                    format_float(covariance.y_axis.x),
                    format_float(covariance.z_axis.x),
                ],
                vec![
                    "covariance row 2".to_string(),
                    format_float(covariance.x_axis.y),
                    format_float(covariance.y_axis.y),
                    format_float(covariance.z_axis.y),
                ],
                vec![
                    "covariance row 3".to_string(),
                    format_float(covariance.x_axis.z),
                    format_float(covariance.y_axis.z),
                    format_float(covariance.z_axis.z),
                ],
            ],
        },
        AnalysisTable {
            columns: vec![
                "component".to_string(),
                "eigenvalue".to_string(),
                "direction".to_string(),
            ],
            rows: principal_components
                .iter()
                .enumerate()
                .map(|(index, (eigenvalue, direction))| {
                    vec![
                        format!("PC{}", index + 1),
                        format_float(*eigenvalue),
                        format_vec3(*direction),
                    ]
                })
                .collect(),
        },
    ];
    let plots = vec![
        PlotSpec {
            name: format!("Centroid {}", plot.name),
            visible: true,
            domain: plot.domain.clone(),
            resolution: plot.resolution,
            style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.86, 0.3, 1.0]),
                point_size: 9.0,
                ..PlotStyle::default()
            },
            definition: crate::PlotDefinition::PointAnnotations {
                points: vec![PointAnnotation {
                    position: centroid.to_array(),
                    label: "Centroid".to_string(),
                }],
                show_labels: true,
            },
        },
        PlotSpec {
            name: format!("PCA Axes {}", plot.name),
            visible: true,
            domain: plot.domain.clone(),
            resolution: plot.resolution,
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.5, 0.8, 1.0, 1.0]),
                glyph_scale: 1.0,
                shading: crate::ShadingMode::Unlit,
                ..PlotStyle::default()
            },
            definition: crate::PlotDefinition::ArrowAnnotations {
                arrows: principal_components
                    .iter()
                    .enumerate()
                    .map(|(index, (eigenvalue, direction))| ArrowAnnotation {
                        origin: centroid.to_array(),
                        vector: (*direction * (eigenvalue.max(0.0).sqrt() * 2.5)).to_array(),
                        label: format!("PC{}", index + 1),
                    })
                    .collect(),
                show_labels: true,
            },
        },
    ];
    Ok(AnalysisOutput::Composite {
        plots,
        reports,
        tables,
        diagnostics: Vec::new(),
        provenance: AnalysisProvenance {
            kind: AnalysisKind::PointCloudStatistics,
            source_plots: vec![plot.name.clone()],
            parameters: Vec::new(),
            notes: vec!["Computed from sampled point positions.".to_string()],
        },
    })
}

fn scatter_cloud_points() -> Vec<[f32; 3]> {
    (0..200)
        .map(|i| {
            glam::Vec3::new(
                (i as f32 * 0.37).sin() * 5.0,
                (i as f32 * 0.73).cos() * 5.0,
                (i as f32 * 0.11).sin() * 5.0,
            )
            .to_array()
        })
        .collect()
}

fn make_data_quality_output(plot: &PlotSpec) -> Result<AnalysisOutput, AnalysisError> {
    let groups = sample_data_groups(plot)?;
    let indexed = flatten_indexed_sample_groups(&groups);
    if indexed.len() < 2 {
        return Err(AnalysisError::invalid(
            "Data quality checks require at least two samples.",
        ));
    }
    let samples = indexed
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let centroid = samples.iter().copied().sum::<Vec3>() / samples.len() as f32;
    let (bbox_min, bbox_max) = bounds_for_points(&samples);
    let diag = bbox_max.distance(bbox_min).max(1.0e-4);
    let duplicate_rows = exact_duplicate_rows(&indexed);

    let heavy_checks = samples.len() <= 4_000;
    let nearest = if heavy_checks {
        nearest_neighbor_stats(&indexed)
    } else {
        None
    };
    let near_duplicate_epsilon = diag * 1.0e-3;
    let near_duplicate_count = nearest.as_ref().map_or(0, |stats| {
        stats
            .nearest_distances
            .iter()
            .filter(|distance| **distance > 0.0 && **distance <= near_duplicate_epsilon)
            .count()
    });
    let (distance_mean, distance_std) = mean_std(
        &samples
            .iter()
            .map(|point| point.distance(centroid))
            .collect::<Vec<_>>(),
    );
    let outlier_threshold = distance_mean + distance_std * 2.5;
    let outliers = indexed
        .iter()
        .filter(|sample| sample.position.distance(centroid) > outlier_threshold)
        .cloned()
        .collect::<Vec<_>>();
    let sparse_samples = nearest
        .as_ref()
        .map(|stats| {
            stats
                .nearest_distances
                .iter()
                .enumerate()
                .filter(|(_, distance)| **distance > stats.mean + stats.std * 2.0)
                .map(|(index, _)| indexed[index].clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let monotonicity_rows = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.len() >= 2)
        .map(|(index, group)| {
            vec![
                format!("sequence {}", index + 1),
                monotonicity_label(group, 0),
                monotonicity_label(group, 1),
                monotonicity_label(group, 2),
            ]
        })
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    if !duplicate_rows.is_empty() {
        diagnostics.push(Diagnostic::warning(
            DiagnosticKind::Validation,
            format!(
                "Detected {} exact duplicate sample(s).",
                duplicate_rows.len()
            ),
        ));
    }
    if near_duplicate_count > 0 {
        diagnostics.push(Diagnostic::warning(
            DiagnosticKind::Validation,
            format!(
                "Detected {} near-duplicate sample(s) within {:.4}.",
                near_duplicate_count, near_duplicate_epsilon
            ),
        ));
    }
    if !outliers.is_empty() {
        diagnostics.push(Diagnostic::warning(
            DiagnosticKind::Validation,
            format!("Detected {} positional outlier(s).", outliers.len()),
        ));
    }
    if !sparse_samples.is_empty() {
        diagnostics.push(Diagnostic::warning(
            DiagnosticKind::Validation,
            format!("Detected {} sparse-region sample(s).", sparse_samples.len()),
        ));
    }
    if !heavy_checks {
        diagnostics.push(Diagnostic::warning(
            DiagnosticKind::Build,
            "Skipped nearest-neighbor heavy checks for datasets above 4000 samples.",
        ));
    }
    diagnostics.extend(duplicate_rows.iter().take(12).map(|sample| {
        Diagnostic::warning(
            DiagnosticKind::Validation,
            "Exact duplicate sample.".to_string(),
        )
        .with_location(
            DiagnosticLocation::new()
                .with_component("sample")
                .with_row(sample.sequence_row),
        )
    }));
    diagnostics.extend(outliers.iter().take(12).map(|sample| {
        Diagnostic::warning(
            DiagnosticKind::Validation,
            format!(
                "Outlier distance {:.4} exceeds threshold {:.4}.",
                sample.position.distance(centroid),
                outlier_threshold
            ),
        )
        .with_location(
            DiagnosticLocation::new()
                .with_component("sample")
                .with_row(sample.sequence_row),
        )
    }));

    let mut reports = vec![AnalysisReport {
        title: format!("Data Quality {}", plot.name),
        values: vec![
            ("Sample Count".to_string(), samples.len().to_string()),
            (
                "Exact Duplicates".to_string(),
                duplicate_rows.len().to_string(),
            ),
            (
                "Near Duplicates".to_string(),
                near_duplicate_count.to_string(),
            ),
            ("Outliers".to_string(), outliers.len().to_string()),
            (
                "Sparse Samples".to_string(),
                sparse_samples.len().to_string(),
            ),
        ],
    }];
    if let Some(stats) = &nearest {
        reports.push(AnalysisReport {
            title: "Spacing Diagnostics".to_string(),
            values: vec![
                ("Nearest Min".to_string(), format_float(stats.min)),
                ("Nearest Mean".to_string(), format_float(stats.mean)),
                ("Nearest Median".to_string(), format_float(stats.median)),
                ("Nearest Max".to_string(), format_float(stats.max)),
                ("Nearest Std".to_string(), format_float(stats.std)),
            ],
        });
    }

    let mut tables = vec![
        sample_positions_table(&groups),
        AnalysisTable {
            columns: vec![
                "sequence".to_string(),
                "x".to_string(),
                "y".to_string(),
                "z".to_string(),
                "distance_from_centroid".to_string(),
            ],
            rows: outliers
                .iter()
                .take(250)
                .map(|sample| {
                    vec![
                        sample.sequence_row.to_string(),
                        format_float(sample.position.x),
                        format_float(sample.position.y),
                        format_float(sample.position.z),
                        format_float(sample.position.distance(centroid)),
                    ]
                })
                .collect(),
        },
    ];
    if !monotonicity_rows.is_empty() {
        tables.push(AnalysisTable {
            columns: vec![
                "sequence".to_string(),
                "x monotonic".to_string(),
                "y monotonic".to_string(),
                "z monotonic".to_string(),
            ],
            rows: monotonicity_rows,
        });
    }

    let mut plots = Vec::new();
    if !outliers.is_empty() {
        plots.push(PlotSpec {
            name: format!("Outliers {}", plot.name),
            visible: true,
            domain: plot.domain.clone(),
            resolution: plot.resolution,
            style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.35, 0.35, 1.0]),
                point_size: 8.0,
                ..PlotStyle::default()
            },
            definition: crate::PlotDefinition::PointAnnotations {
                points: outliers
                    .iter()
                    .enumerate()
                    .map(|(index, sample)| PointAnnotation {
                        position: sample.position.to_array(),
                        label: format!("Outlier {}", index + 1),
                    })
                    .collect(),
                show_labels: true,
            },
        });
    }

    Ok(AnalysisOutput::Composite {
        plots,
        reports,
        tables,
        diagnostics,
        provenance: AnalysisProvenance {
            kind: AnalysisKind::DataQualityChecks,
            source_plots: vec![plot.name.clone()],
            parameters: Vec::new(),
            notes: vec!["Ordered-sequence checks operate per sampled sequence.".to_string()],
        },
    })
}

#[derive(Clone)]
struct IndexedSample {
    sequence_row: usize,
    position: Vec3,
}

struct NearestNeighborSummary {
    nearest_distances: Vec<f32>,
    min: f32,
    mean: f32,
    median: f32,
    max: f32,
    std: f32,
}

fn flatten_sample_groups(groups: &[Vec<[f32; 3]>]) -> Vec<Vec3> {
    groups
        .iter()
        .flat_map(|group| group.iter().map(|point| Vec3::from_array(*point)))
        .collect()
}

fn flatten_indexed_sample_groups(groups: &[Vec<[f32; 3]>]) -> Vec<IndexedSample> {
    groups
        .iter()
        .enumerate()
        .flat_map(|(_, group)| {
            group
                .iter()
                .enumerate()
                .map(move |(row, point)| IndexedSample {
                    sequence_row: row + 1,
                    position: Vec3::from_array(*point),
                })
        })
        .collect()
}

fn bounds_for_points(points: &[Vec3]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for point in points {
        min = min.min(*point);
        max = max.max(*point);
    }
    (min, max)
}

fn covariance_matrix(points: &[Vec3], centroid: Vec3) -> glam::Mat3 {
    let mut xx = 0.0_f32;
    let mut xy = 0.0_f32;
    let mut xz = 0.0_f32;
    let mut yy = 0.0_f32;
    let mut yz = 0.0_f32;
    let mut zz = 0.0_f32;
    for point in points {
        let d = *point - centroid;
        xx += d.x * d.x;
        xy += d.x * d.y;
        xz += d.x * d.z;
        yy += d.y * d.y;
        yz += d.y * d.z;
        zz += d.z * d.z;
    }
    let scale = 1.0 / points.len().max(1) as f32;
    glam::Mat3::from_cols_array(&[
        xx * scale,
        xy * scale,
        xz * scale,
        xy * scale,
        yy * scale,
        yz * scale,
        xz * scale,
        yz * scale,
        zz * scale,
    ])
}

fn principal_components(covariance: glam::Mat3) -> [(f32, Vec3); 3] {
    let mut components = [(0.0_f32, Vec3::X), (0.0_f32, Vec3::Y), (0.0_f32, Vec3::Z)];
    let mut matrix = covariance;
    for component in &mut components {
        let direction = power_iteration(matrix);
        let eigenvalue = direction.dot(matrix * direction).max(0.0);
        *component = (eigenvalue, direction);
        matrix -= eigenvalue * outer_product(direction);
    }
    components.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    if components[2].1.length_squared() < 1.0e-6 {
        components[2].1 = components[0].1.cross(components[1].1).normalize_or_zero();
    }
    components
}

fn power_iteration(matrix: glam::Mat3) -> Vec3 {
    let mut v = Vec3::new(0.71, 0.53, 0.47).normalize();
    for _ in 0..16 {
        let next = matrix * v;
        if next.length_squared() <= 1.0e-8 {
            break;
        }
        v = next.normalize();
    }
    v.normalize_or_zero()
}

fn outer_product(v: Vec3) -> glam::Mat3 {
    glam::Mat3::from_cols(
        Vec3::new(v.x * v.x, v.x * v.y, v.x * v.z),
        Vec3::new(v.y * v.x, v.y * v.y, v.y * v.z),
        Vec3::new(v.z * v.x, v.z * v.y, v.z * v.z),
    )
}

fn exact_duplicate_rows(samples: &[IndexedSample]) -> Vec<IndexedSample> {
    let mut seen = std::collections::HashMap::new();
    let mut duplicates = Vec::new();
    for sample in samples {
        let key = (
            sample.position.x.to_bits(),
            sample.position.y.to_bits(),
            sample.position.z.to_bits(),
        );
        if seen.insert(key, sample.sequence_row).is_some() {
            duplicates.push(sample.clone());
        }
    }
    duplicates
}

fn nearest_neighbor_stats(samples: &[IndexedSample]) -> Option<NearestNeighborSummary> {
    if samples.len() < 2 {
        return None;
    }
    let mut nearest_distances = vec![f32::INFINITY; samples.len()];
    for i in 0..samples.len() {
        for j in (i + 1)..samples.len() {
            let distance = samples[i].position.distance(samples[j].position);
            nearest_distances[i] = nearest_distances[i].min(distance);
            nearest_distances[j] = nearest_distances[j].min(distance);
        }
    }
    let filtered = nearest_distances
        .iter()
        .copied()
        .filter(|distance| distance.is_finite())
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return None;
    }
    let mut sorted = filtered.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let (mean, std) = mean_std(&filtered);
    Some(NearestNeighborSummary {
        min: *sorted.first().unwrap_or(&0.0),
        mean,
        median: sorted[sorted.len() / 2],
        max: *sorted.last().unwrap_or(&0.0),
        std,
        nearest_distances,
    })
}

fn mean_std(values: &[f32]) -> (f32, f32) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values
        .iter()
        .map(|value| {
            let d = *value - mean;
            d * d
        })
        .sum::<f32>()
        / values.len() as f32;
    (mean, variance.sqrt())
}

fn monotonicity_label(group: &[[f32; 3]], axis: usize) -> String {
    if axis_is_monotonic(group, axis) {
        let first = axis_value(Vec3::from_array(group[0]), axis);
        let last = axis_value(Vec3::from_array(*group.last().unwrap_or(&group[0])), axis);
        if last >= first {
            "increasing".to_string()
        } else {
            "decreasing".to_string()
        }
    } else {
        "mixed".to_string()
    }
}

fn format_float(value: f32) -> String {
    format!("{value:.5}")
}

fn format_vec3(value: Vec3) -> String {
    format!("{:.5}, {:.5}, {:.5}", value.x, value.y, value.z)
}

fn sample_positions_table(groups: &[Vec<[f32; 3]>]) -> AnalysisTable {
    AnalysisTable {
        columns: vec![
            "sequence".to_string(),
            "row".to_string(),
            "x".to_string(),
            "y".to_string(),
            "z".to_string(),
        ],
        rows: groups
            .iter()
            .enumerate()
            .flat_map(|(sequence_idx, group)| {
                group.iter().enumerate().map(move |(row_idx, point)| {
                    vec![
                        (sequence_idx + 1).to_string(),
                        (row_idx + 1).to_string(),
                        format_float(point[0]),
                        format_float(point[1]),
                        format_float(point[2]),
                    ]
                })
            })
            .collect(),
    }
}

fn fit_curve_group(
    source: &PlotSpec,
    group: &[[f32; 3]],
    options: &CurveFitOptions,
) -> Result<CurveFitResult, AnalysisError> {
    match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => fit_cartesian_line_group(group, dep_var.as_str(), ind_var.as_str(), options),
        _ => fit_parametric_curve_group(group, options),
    }
}

fn fit_cartesian_line_group(
    group: &[[f32; 3]],
    dep_var: &str,
    ind_var: &str,
    options: &CurveFitOptions,
) -> Result<CurveFitResult, AnalysisError> {
    if group.len() < 2 {
        return Err(AnalysisError::invalid(
            "Need at least two points to fit a curve.",
        ));
    }
    let xs: Vec<f64> = group
        .iter()
        .filter_map(|point| {
            cartesian_axis_value(Vec3::from_array(*point), ind_var).map(|v| v as f64)
        })
        .collect();
    let ys: Vec<f64> = group
        .iter()
        .filter_map(|point| {
            cartesian_axis_value(Vec3::from_array(*point), dep_var).map(|v| v as f64)
        })
        .collect();
    if xs.len() != group.len() || ys.len() != group.len() {
        return Err(AnalysisError::invalid(
            "Could not read curve axes for fitting.",
        ));
    }
    let evaluation_count = resampled_point_count(group.len(), options.samples_per_segment);
    let x_eval = evenly_spaced_range(*xs.first().unwrap(), *xs.last().unwrap(), evaluation_count);

    match options.method {
        CurveFitMethod::Polynomial => {
            let fit = polynomial_fit(&xs, &ys, options.degree, None)?;
            let fitted_group = x_eval
                .iter()
                .map(|x| {
                    cartesian_line_point(dep_var, ind_var, *x as f32, fit.evaluate(*x) as f32)
                        .to_array()
                })
                .collect();
            let residual_values = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| (fit.evaluate(*x) - *y) as f32)
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::RobustPolynomial => {
            let fit = robust_polynomial_fit_scalar(&xs, &ys, options.degree)?;
            let fitted_group = x_eval
                .iter()
                .map(|x| {
                    cartesian_line_point(dep_var, ind_var, *x as f32, fit.evaluate(*x) as f32)
                        .to_array()
                })
                .collect();
            let residual_values = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| (fit.evaluate(*x) - *y) as f32)
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::Fourier => {
            let fit = fourier_fit(&xs, &ys, options.harmonics)?;
            let fitted_group = x_eval
                .iter()
                .map(|x| {
                    cartesian_line_point(dep_var, ind_var, *x as f32, fit.evaluate(*x) as f32)
                        .to_array()
                })
                .collect();
            let residual_values = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| (fit.evaluate(*x) - *y) as f32)
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::Spline => {
            let smoothed = smooth_scalar_values(&ys, options.smoothing_window);
            let control_points: Vec<[f32; 3]> = xs
                .iter()
                .zip(smoothed.iter())
                .map(|(x, y)| {
                    cartesian_line_point(dep_var, ind_var, *x as f32, *y as f32).to_array()
                })
                .collect();
            let fitted_group = sample_curve_points(
                &control_points
                    .iter()
                    .map(|point| Vec3::from_array(*point))
                    .collect::<Vec<_>>(),
                CurveInterpolation {
                    kind: CurveInterpolationKind::CentripetalCatmullRom,
                    samples_per_segment: options.samples_per_segment,
                    closed: false,
                    smoothing_window: options.smoothing_window,
                },
            )
            .into_iter()
            .map(|point| point.to_array())
            .collect();
            let residual_values = ys
                .iter()
                .zip(smoothed.iter())
                .map(|(observed, fitted)| (*fitted - *observed) as f32)
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
    }
}

fn fit_parametric_curve_group(
    group: &[[f32; 3]],
    options: &CurveFitOptions,
) -> Result<CurveFitResult, AnalysisError> {
    if group.len() < 2 {
        return Err(AnalysisError::invalid(
            "Need at least two points to fit a curve.",
        ));
    }
    let ts = evenly_spaced_parameter_values(group.len());
    let evaluation_ts = evenly_spaced_parameter_values(resampled_point_count(
        group.len(),
        options.samples_per_segment,
    ));
    let positions: Vec<Vec3> = group.iter().map(|point| Vec3::from_array(*point)).collect();

    match options.method {
        CurveFitMethod::Polynomial => {
            let fit = polynomial_fit_vector(&ts, &positions, options.degree, None)?;
            let fitted_group = evaluation_ts
                .iter()
                .map(|t| fit.evaluate(*t).to_array())
                .collect();
            let residual_values = ts
                .iter()
                .zip(positions.iter())
                .map(|(t, observed)| fit.evaluate(*t).distance(*observed))
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::RobustPolynomial => {
            let fit = robust_polynomial_fit_vector(&ts, &positions, options.degree)?;
            let fitted_group = evaluation_ts
                .iter()
                .map(|t| fit.evaluate(*t).to_array())
                .collect();
            let residual_values = ts
                .iter()
                .zip(positions.iter())
                .map(|(t, observed)| fit.evaluate(*t).distance(*observed))
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::Fourier => {
            let fit = fourier_fit_vector(&ts, &positions, options.harmonics)?;
            let fitted_group = evaluation_ts
                .iter()
                .map(|t| fit.evaluate(*t).to_array())
                .collect();
            let residual_values = ts
                .iter()
                .zip(positions.iter())
                .map(|(t, observed)| fit.evaluate(*t).distance(*observed))
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
        CurveFitMethod::Spline => {
            let smoothed = smooth_vec3_values(&positions, options.smoothing_window);
            let fitted_group = sample_curve_points(
                &smoothed,
                CurveInterpolation {
                    kind: CurveInterpolationKind::CentripetalCatmullRom,
                    samples_per_segment: options.samples_per_segment,
                    closed: false,
                    smoothing_window: options.smoothing_window,
                },
            )
            .into_iter()
            .map(|point| point.to_array())
            .collect();
            let residual_values = positions
                .iter()
                .zip(smoothed.iter())
                .map(|(observed, fitted)| fitted.distance(*observed))
                .collect();
            Ok(CurveFitResult {
                fitted_group,
                residual_values,
            })
        }
    }
}

fn derived_polyline_plot(
    source: &PlotSpec,
    name: String,
    color: [f32; 4],
    groups: Vec<Vec<[f32; 3]>>,
) -> Result<PlotSpec, AnalysisError> {
    if groups.is_empty() {
        return Err(AnalysisError::invalid(
            "The selected plot did not produce enough samples for this analysis.",
        ));
    }
    Ok(PlotSpec {
        name,
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::Solid(color),
            line_width: 2.25,
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::DerivedPolylineGroups { groups },
    })
}

fn make_curve_fit_output(
    source: &PlotSpec,
    options: CurveFitOptions,
) -> Result<AnalysisOutput, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    if groups.is_empty() || groups.iter().all(|group| group.len() < 2) {
        return Err(AnalysisError::invalid(
            "Curve fitting requires at least one sampled curve with two or more points.",
        ));
    }

    let mut fitted_groups = Vec::new();
    let mut residual_groups = Vec::new();
    let mut control_points = Vec::new();
    let mut residual_samples = Vec::new();
    let mut total_points = 0usize;

    for group in &groups {
        if group.len() < 2 {
            continue;
        }
        total_points += group.len();
        control_points.extend(group.iter().copied());
        let fit = fit_curve_group(source, group, &options)?;
        if fit.fitted_group.len() >= 2 {
            fitted_groups.push(fit.fitted_group);
        }
        if fit.residual_values.len() >= 2 {
            let residual_group = match &source.definition {
                crate::PlotDefinition::ExprCartesianLine {
                    dep_var, ind_var, ..
                } => scalar_plot_cartesian_line_group(
                    group,
                    dep_var.as_str(),
                    ind_var.as_str(),
                    &fit.residual_values,
                ),
                _ => scalar_curve_group(group, &fit.residual_values),
            };
            if residual_group.len() >= 2 {
                residual_groups.push(residual_group);
            }
        }
        residual_samples.extend(fit.residual_values.iter().map(|value| value.abs()));
    }

    if fitted_groups.is_empty() {
        return Err(AnalysisError::invalid(
            "Curve fitting could not produce a fitted curve from the selected source.",
        ));
    }

    let mut plots = vec![PlotSpec {
        name: options.output_name.clone().unwrap_or_else(|| {
            format!("{} {}", curve_fit_method_label(options.method), source.name)
        }),
        visible: true,
        domain: source.domain.clone(),
        resolution: source.resolution,
        style: PlotStyle {
            colour_mode: ColourMode::Solid([1.0, 0.72, 0.25, 1.0]),
            line_width: 2.5,
            ..PlotStyle::default()
        },
        definition: crate::PlotDefinition::DerivedPolylineGroups {
            groups: fitted_groups,
        },
    }];

    if options.show_control_points && !control_points.is_empty() {
        plots.push(PlotSpec {
            name: format!("Control Points {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.35, 0.85, 1.0, 1.0]),
                point_size: 7.0,
                ..PlotStyle::default()
            },
            definition: crate::PlotDefinition::PointAnnotations {
                points: control_points
                    .iter()
                    .enumerate()
                    .map(|(index, position)| PointAnnotation {
                        position: *position,
                        label: format!("Control {}", index + 1),
                    })
                    .collect(),
                show_labels: false,
            },
        });
    }

    if options.show_residual_plot && !residual_groups.is_empty() {
        plots.push(PlotSpec {
            name: format!("Residuals {}", source.name),
            visible: true,
            domain: source.domain.clone(),
            resolution: source.resolution,
            style: PlotStyle {
                colour_mode: ColourMode::Solid([1.0, 0.35, 0.35, 1.0]),
                line_width: 2.0,
                ..PlotStyle::default()
            },
            definition: crate::PlotDefinition::DerivedPolylineGroups {
                groups: residual_groups,
            },
        });
    }

    let rms = if residual_samples.is_empty() {
        0.0
    } else {
        (residual_samples
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            / residual_samples.len() as f32)
            .sqrt()
    };
    let max_residual = residual_samples.iter().copied().fold(0.0_f32, f32::max);

    let mut report_values = vec![
        (
            "Method".to_string(),
            curve_fit_method_label(options.method).to_string(),
        ),
        ("Samples".to_string(), total_points.to_string()),
        ("RMS Residual".to_string(), format!("{rms:.6}")),
        ("Max Residual".to_string(), format!("{max_residual:.6}")),
    ];
    match options.method {
        CurveFitMethod::Polynomial | CurveFitMethod::RobustPolynomial => {
            report_values.push(("Degree".to_string(), options.degree.to_string()));
        }
        CurveFitMethod::Fourier => {
            report_values.push(("Harmonics".to_string(), options.harmonics.to_string()));
        }
        CurveFitMethod::Spline => {
            report_values.push((
                "Smoothing Window".to_string(),
                options.smoothing_window.to_string(),
            ));
        }
    }

    Ok(AnalysisOutput::Composite {
        plots,
        reports: vec![AnalysisReport {
            title: format!("Curve Fit {}", source.name),
            values: report_values,
        }],
        tables: Vec::new(),
        diagnostics: Vec::new(),
        provenance: AnalysisProvenance {
            kind: AnalysisKind::FitCurve,
            source_plots: vec![source.name.clone()],
            parameters: vec![
                ("fit_method".to_string(), curve_fit_method_key(options.method).to_string()),
                ("degree".to_string(), options.degree.to_string()),
                ("harmonics".to_string(), options.harmonics.to_string()),
                ("smoothing_window".to_string(), options.smoothing_window.to_string()),
                (
                    "samples_per_segment".to_string(),
                    options.samples_per_segment.to_string(),
                ),
            ],
            notes: vec![
                "Derived output includes a fitted curve plus optional control-point preview and residual plot.".to_string(),
            ],
        },
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurveFitMethod {
    Polynomial,
    RobustPolynomial,
    Spline,
    Fourier,
}

#[derive(Clone, Debug)]
struct CurveFitOptions {
    method: CurveFitMethod,
    output_name: Option<String>,
    degree: usize,
    harmonics: usize,
    smoothing_window: u32,
    samples_per_segment: u32,
    show_control_points: bool,
    show_residual_plot: bool,
}

#[derive(Clone, Debug)]
struct CurveFitResult {
    fitted_group: Vec<[f32; 3]>,
    residual_values: Vec<f32>,
}

fn parameter_map(parameters: &[(String, String)]) -> HashMap<String, String> {
    parameters.iter().cloned().collect()
}

fn build_curve_fit_options(params: &HashMap<String, String>) -> CurveFitOptions {
    CurveFitOptions {
        method: parse_curve_fit_method(params.get("fit_method").map(String::as_str))
            .unwrap_or(CurveFitMethod::Polynomial),
        output_name: params.get("output_name").cloned(),
        degree: params
            .get("degree")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(5)
            .clamp(1, 12),
        harmonics: params
            .get("harmonics")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(3)
            .clamp(1, 16),
        smoothing_window: normalized_window_value(
            params
                .get("smoothing_window")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(7),
        ),
        samples_per_segment: params
            .get("samples_per_segment")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(8)
            .clamp(1, 64),
        show_control_points: params
            .get("show_control_points")
            .is_none_or(|value| matches!(value.as_str(), "1" | "true" | "yes")),
        show_residual_plot: params
            .get("show_residual_plot")
            .is_none_or(|value| matches!(value.as_str(), "1" | "true" | "yes")),
    }
}

fn build_interpolation(params: &HashMap<String, String>) -> CurveInterpolation {
    CurveInterpolation {
        kind: parse_interpolation_kind(params.get("interpolation_kind").map(String::as_str))
            .unwrap_or(CurveInterpolationKind::Linear),
        samples_per_segment: params
            .get("samples_per_segment")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1),
        closed: params
            .get("closed")
            .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes")),
        smoothing_window: normalized_window_value(
            params
                .get("smoothing_window")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(5),
        ),
    }
}

fn parse_curve_fit_method(value: Option<&str>) -> Option<CurveFitMethod> {
    match value? {
        "polynomial" => Some(CurveFitMethod::Polynomial),
        "robust_polynomial" => Some(CurveFitMethod::RobustPolynomial),
        "spline" => Some(CurveFitMethod::Spline),
        "fourier" => Some(CurveFitMethod::Fourier),
        _ => None,
    }
}

fn curve_fit_method_label(method: CurveFitMethod) -> &'static str {
    match method {
        CurveFitMethod::Polynomial => "Fit (Polynomial)",
        CurveFitMethod::RobustPolynomial => "Fit (Robust Polynomial)",
        CurveFitMethod::Spline => "Fit (Spline / Smoothed Catmull-Rom)",
        CurveFitMethod::Fourier => "Fit (Fourier Series)",
    }
}

fn curve_fit_method_key(method: CurveFitMethod) -> &'static str {
    match method {
        CurveFitMethod::Polynomial => "polynomial",
        CurveFitMethod::RobustPolynomial => "robust_polynomial",
        CurveFitMethod::Spline => "spline",
        CurveFitMethod::Fourier => "fourier",
    }
}

#[derive(Clone, Debug)]
struct PolynomialFit {
    coeffs: Vec<f64>,
    x_min: f64,
    x_max: f64,
}

impl PolynomialFit {
    fn evaluate(&self, x: f64) -> f64 {
        evaluate_polynomial(
            &self.coeffs,
            normalize_domain_value(x, self.x_min, self.x_max),
        )
    }
}

#[derive(Clone, Debug)]
struct PolynomialVectorFit {
    x: PolynomialFit,
    y: PolynomialFit,
    z: PolynomialFit,
}

impl PolynomialVectorFit {
    fn evaluate(&self, t: f64) -> Vec3 {
        Vec3::new(
            self.x.evaluate(t) as f32,
            self.y.evaluate(t) as f32,
            self.z.evaluate(t) as f32,
        )
    }
}

#[derive(Clone, Debug)]
struct FourierFit {
    constant: f64,
    cos_coeffs: Vec<f64>,
    sin_coeffs: Vec<f64>,
    x_min: f64,
    x_max: f64,
}

impl FourierFit {
    fn evaluate(&self, x: f64) -> f64 {
        let theta = normalized_angle(x, self.x_min, self.x_max);
        let mut value = self.constant;
        for (index, (cos_coeff, sin_coeff)) in self
            .cos_coeffs
            .iter()
            .zip(self.sin_coeffs.iter())
            .enumerate()
        {
            let k = index as f64 + 1.0;
            value += *cos_coeff * (k * theta).cos() + *sin_coeff * (k * theta).sin();
        }
        value
    }
}

#[derive(Clone, Debug)]
struct FourierVectorFit {
    x: FourierFit,
    y: FourierFit,
    z: FourierFit,
}

impl FourierVectorFit {
    fn evaluate(&self, t: f64) -> Vec3 {
        Vec3::new(
            self.x.evaluate(t) as f32,
            self.y.evaluate(t) as f32,
            self.z.evaluate(t) as f32,
        )
    }
}

fn polynomial_fit(
    xs: &[f64],
    ys: &[f64],
    degree: usize,
    weights: Option<&[f64]>,
) -> Result<PolynomialFit, AnalysisError> {
    let x_min = *xs
        .first()
        .ok_or_else(|| AnalysisError::invalid("No samples for fit."))?;
    let x_max = *xs
        .last()
        .ok_or_else(|| AnalysisError::invalid("No samples for fit."))?;
    let normalized: Vec<f64> = xs
        .iter()
        .map(|x| normalize_domain_value(*x, x_min, x_max))
        .collect();
    let coeffs = weighted_polynomial_coefficients(&normalized, ys, degree, weights)?;
    Ok(PolynomialFit {
        coeffs,
        x_min,
        x_max,
    })
}

fn robust_polynomial_fit_scalar(
    xs: &[f64],
    ys: &[f64],
    degree: usize,
) -> Result<PolynomialFit, AnalysisError> {
    let mut weights = vec![1.0_f64; xs.len()];
    let mut fit = polynomial_fit(xs, ys, degree, Some(&weights))?;
    for _ in 0..5 {
        let residuals: Vec<f64> = xs
            .iter()
            .zip(ys.iter())
            .map(|(x, y)| (fit.evaluate(*x) - *y).abs())
            .collect();
        weights = huber_weights(&residuals);
        fit = polynomial_fit(xs, ys, degree, Some(&weights))?;
    }
    Ok(fit)
}

fn polynomial_fit_vector(
    ts: &[f64],
    positions: &[Vec3],
    degree: usize,
    weights: Option<&[f64]>,
) -> Result<PolynomialVectorFit, AnalysisError> {
    let xs: Vec<f64> = positions.iter().map(|position| position.x as f64).collect();
    let ys: Vec<f64> = positions.iter().map(|position| position.y as f64).collect();
    let zs: Vec<f64> = positions.iter().map(|position| position.z as f64).collect();
    Ok(PolynomialVectorFit {
        x: polynomial_fit(ts, &xs, degree, weights)?,
        y: polynomial_fit(ts, &ys, degree, weights)?,
        z: polynomial_fit(ts, &zs, degree, weights)?,
    })
}

fn robust_polynomial_fit_vector(
    ts: &[f64],
    positions: &[Vec3],
    degree: usize,
) -> Result<PolynomialVectorFit, AnalysisError> {
    let mut weights = vec![1.0_f64; ts.len()];
    let mut fit = polynomial_fit_vector(ts, positions, degree, Some(&weights))?;
    for _ in 0..5 {
        let residuals: Vec<f64> = ts
            .iter()
            .zip(positions.iter())
            .map(|(t, position)| fit.evaluate(*t).distance(*position) as f64)
            .collect();
        weights = huber_weights(&residuals);
        fit = polynomial_fit_vector(ts, positions, degree, Some(&weights))?;
    }
    Ok(fit)
}

fn fourier_fit(xs: &[f64], ys: &[f64], harmonics: usize) -> Result<FourierFit, AnalysisError> {
    let x_min = *xs
        .first()
        .ok_or_else(|| AnalysisError::invalid("No samples for fit."))?;
    let x_max = *xs
        .last()
        .ok_or_else(|| AnalysisError::invalid("No samples for fit."))?;
    let design = xs
        .iter()
        .map(|x| fourier_basis(normalized_angle(*x, x_min, x_max), harmonics))
        .collect::<Vec<_>>();
    let coeffs = linear_least_squares(&design, ys, None)?;
    Ok(FourierFit {
        constant: coeffs[0],
        cos_coeffs: (0..harmonics).map(|index| coeffs[1 + index * 2]).collect(),
        sin_coeffs: (0..harmonics).map(|index| coeffs[2 + index * 2]).collect(),
        x_min,
        x_max,
    })
}

fn fourier_fit_vector(
    ts: &[f64],
    positions: &[Vec3],
    harmonics: usize,
) -> Result<FourierVectorFit, AnalysisError> {
    let xs: Vec<f64> = positions.iter().map(|position| position.x as f64).collect();
    let ys: Vec<f64> = positions.iter().map(|position| position.y as f64).collect();
    let zs: Vec<f64> = positions.iter().map(|position| position.z as f64).collect();
    Ok(FourierVectorFit {
        x: fourier_fit(ts, &xs, harmonics)?,
        y: fourier_fit(ts, &ys, harmonics)?,
        z: fourier_fit(ts, &zs, harmonics)?,
    })
}

fn weighted_polynomial_coefficients(
    xs: &[f64],
    ys: &[f64],
    degree: usize,
    weights: Option<&[f64]>,
) -> Result<Vec<f64>, AnalysisError> {
    let design = xs
        .iter()
        .map(|x| polynomial_basis(*x, degree))
        .collect::<Vec<_>>();
    linear_least_squares(&design, ys, weights)
}

fn linear_least_squares(
    design: &[Vec<f64>],
    ys: &[f64],
    weights: Option<&[f64]>,
) -> Result<Vec<f64>, AnalysisError> {
    if design.is_empty() || ys.is_empty() || design.len() != ys.len() {
        return Err(AnalysisError::invalid("Invalid least-squares system."));
    }
    let cols = design[0].len();
    let mut ata = vec![vec![0.0_f64; cols]; cols];
    let mut atb = vec![0.0_f64; cols];
    for (row_index, row) in design.iter().enumerate() {
        let weight = weights
            .and_then(|values| values.get(row_index).copied())
            .unwrap_or(1.0)
            .max(1.0e-8);
        for i in 0..cols {
            atb[i] += weight * row[i] * ys[row_index];
            for j in 0..cols {
                ata[i][j] += weight * row[i] * row[j];
            }
        }
    }
    solve_linear_system(ata, atb)
}

fn solve_linear_system(
    mut matrix: Vec<Vec<f64>>,
    mut rhs: Vec<f64>,
) -> Result<Vec<f64>, AnalysisError> {
    let size = rhs.len();
    for pivot in 0..size {
        let mut best_row = pivot;
        let mut best_value = matrix[pivot][pivot].abs();
        for row in (pivot + 1)..size {
            let value = matrix[row][pivot].abs();
            if value > best_value {
                best_value = value;
                best_row = row;
            }
        }
        if best_value <= 1.0e-10 {
            return Err(AnalysisError::invalid(
                "Curve fit is ill-conditioned for the selected method and parameters.",
            ));
        }
        if best_row != pivot {
            matrix.swap(pivot, best_row);
            rhs.swap(pivot, best_row);
        }
        let scale = matrix[pivot][pivot];
        for col in pivot..size {
            matrix[pivot][col] /= scale;
        }
        rhs[pivot] /= scale;
        for row in 0..size {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            if factor.abs() <= 1.0e-12 {
                continue;
            }
            for col in pivot..size {
                matrix[row][col] -= factor * matrix[pivot][col];
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    Ok(rhs)
}

fn polynomial_basis(x: f64, degree: usize) -> Vec<f64> {
    let mut basis = vec![1.0_f64; degree + 1];
    for index in 1..=degree {
        basis[index] = basis[index - 1] * x;
    }
    basis
}

fn fourier_basis(theta: f64, harmonics: usize) -> Vec<f64> {
    let mut basis = Vec::with_capacity(1 + harmonics * 2);
    basis.push(1.0);
    for harmonic in 1..=harmonics {
        let angle = theta * harmonic as f64;
        basis.push(angle.cos());
        basis.push(angle.sin());
    }
    basis
}

fn normalize_domain_value(x: f64, x_min: f64, x_max: f64) -> f64 {
    let range = (x_max - x_min).abs();
    if range <= f64::EPSILON {
        0.0
    } else {
        ((x - x_min) / range) * 2.0 - 1.0
    }
}

fn normalized_angle(x: f64, x_min: f64, x_max: f64) -> f64 {
    let range = (x_max - x_min).abs();
    if range <= f64::EPSILON {
        0.0
    } else {
        ((x - x_min) / range) * 2.0 * PI
    }
}

fn evaluate_polynomial(coeffs: &[f64], x: f64) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, coeff| acc * x + coeff)
}

fn huber_weights(residuals: &[f64]) -> Vec<f64> {
    let scale = residual_scale(residuals).max(1.0e-6);
    residuals
        .iter()
        .map(|residual| {
            let normalized = residual.abs() / (1.5 * scale);
            if normalized <= 1.0 {
                1.0
            } else {
                1.0 / normalized
            }
        })
        .collect()
}

fn residual_scale(residuals: &[f64]) -> f64 {
    if residuals.is_empty() {
        return 1.0;
    }
    let mut sorted = residuals
        .iter()
        .map(|value| value.abs())
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

fn smooth_scalar_values(values: &[f64], smoothing_window: u32) -> Vec<f64> {
    let radius = normalized_window_value(smoothing_window) as isize / 2;
    (0..values.len())
        .map(|index| {
            let mut total = 0.0_f64;
            let mut count = 0.0_f64;
            for offset in -radius..=radius {
                let sample_index =
                    (index as isize + offset).clamp(0, values.len() as isize - 1) as usize;
                total += values[sample_index];
                count += 1.0;
            }
            total / count
        })
        .collect()
}

fn smooth_vec3_values(values: &[Vec3], smoothing_window: u32) -> Vec<Vec3> {
    let radius = normalized_window_value(smoothing_window) as isize / 2;
    (0..values.len())
        .map(|index| {
            let mut total = Vec3::ZERO;
            let mut count = 0.0_f32;
            for offset in -radius..=radius {
                let sample_index =
                    (index as isize + offset).clamp(0, values.len() as isize - 1) as usize;
                total += values[sample_index];
                count += 1.0;
            }
            total / count
        })
        .collect()
}

fn resampled_point_count(control_point_count: usize, samples_per_segment: u32) -> usize {
    let segments = control_point_count.saturating_sub(1);
    (segments * samples_per_segment.max(1) as usize + 1).max(control_point_count)
}

fn evenly_spaced_range(start: f64, end: f64, count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![start];
    }
    (0..count)
        .map(|index| start + (end - start) * index as f64 / (count - 1) as f64)
        .collect()
}

fn evenly_spaced_parameter_values(count: usize) -> Vec<f64> {
    evenly_spaced_range(0.0, 1.0, count)
}

fn parse_interpolation_kind(value: Option<&str>) -> Option<CurveInterpolationKind> {
    match value? {
        "linear" => Some(CurveInterpolationKind::Linear),
        "catmull_rom" => Some(CurveInterpolationKind::CatmullRom),
        "centripetal_catmull_rom" => Some(CurveInterpolationKind::CentripetalCatmullRom),
        "moving_average" => Some(CurveInterpolationKind::MovingAverage),
        "savitzky_golay" => Some(CurveInterpolationKind::SavitzkyGolay),
        _ => None,
    }
}

fn parse_axis(value: Option<&str>) -> Option<SliceAxis> {
    match value?.to_ascii_lowercase().as_str() {
        "x" => Some(SliceAxis::X),
        "y" => Some(SliceAxis::Y),
        "z" => Some(SliceAxis::Z),
        _ => None,
    }
}

fn parse_axis_index(value: Option<&str>) -> Option<usize> {
    match value?.to_ascii_lowercase().as_str() {
        "x" | "0" => Some(0),
        "y" | "1" => Some(1),
        "z" | "2" => Some(2),
        _ => None,
    }
}

fn evenly_spaced_isovalues(count: usize) -> Vec<f32> {
    let count = count.max(1);
    if count == 1 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| -0.9 + 1.8 * i as f32 / (count - 1) as f32)
        .collect()
}

fn parse_error(error: impl ToString) -> AnalysisError {
    AnalysisError {
        diagnostic: Diagnostic::error(DiagnosticKind::Parse, error.to_string()),
    }
}

fn table_errors(errors: Vec<crate::TableValidationError>) -> AnalysisError {
    let summary = errors
        .into_iter()
        .map(|error| error.display())
        .collect::<Vec<_>>()
        .join("; ");
    AnalysisError::invalid(summary)
}

fn make_point_annotations(points: &[[f32; 3]], prefix: &str) -> Vec<PointAnnotation> {
    points
        .iter()
        .enumerate()
        .map(|(index, position)| PointAnnotation {
            position: *position,
            label: format!("{prefix} {}", index + 1),
        })
        .collect()
}

fn axis_name(axis: usize) -> &'static str {
    match axis {
        0 => "x",
        1 => "y",
        _ => "z",
    }
}

fn normalized_window_value(window: u32) -> u32 {
    let mut normalized = window.max(3);
    if normalized % 2 == 0 {
        normalized += 1;
    }
    normalized
}

fn derivative_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    if let Some(layout) = detect_planar_graph_layout(group) {
        return derivative_planar_graph_group(group, layout);
    }
    (0..group.len())
        .map(|index| finite_difference(group, index).to_array())
        .collect()
}

fn tangent_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    (0..group.len())
        .map(|index| {
            finite_difference(group, index)
                .normalize_or_zero()
                .to_array()
        })
        .collect()
}

fn finite_difference(group: &[[f32; 3]], index: usize) -> Vec3 {
    let current = Vec3::from_array(group[index]);
    if index == 0 {
        let next = Vec3::from_array(group[1]);
        return next - current;
    }
    if index + 1 == group.len() {
        let prev = Vec3::from_array(group[index - 1]);
        return current - prev;
    }
    let prev = Vec3::from_array(group[index - 1]);
    let next = Vec3::from_array(group[index + 1]);
    (next - prev) * 0.5
}

fn integral_curve_group(group: &[[f32; 3]], normalize_integral: bool) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    if let Some(layout) = detect_planar_graph_layout(group) {
        return integral_planar_graph_group(group, layout, normalize_integral);
    }
    let mut out = Vec::with_capacity(group.len());
    let mut accum = Vec3::ZERO;
    out.push(accum.to_array());
    let dt = 1.0_f32 / (group.len() - 1) as f32;
    for pair in group.windows(2) {
        let a = Vec3::from_array(pair[0]);
        let b = Vec3::from_array(pair[1]);
        accum += (a + b) * 0.5 * dt;
        out.push(accum.to_array());
    }
    out
}

fn cumulative_arc_lengths(group: &[[f32; 3]]) -> Vec<f32> {
    if group.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(group.len());
    let mut total = 0.0_f32;
    out.push(total);
    for pair in group.windows(2) {
        let a = Vec3::from_array(pair[0]);
        let b = Vec3::from_array(pair[1]);
        total += b.distance(a);
        out.push(total);
    }
    out
}

fn curvature_values(group: &[[f32; 3]]) -> Vec<f32> {
    if group.len() < 3 {
        return vec![0.0; group.len()];
    }
    let mut values = vec![0.0_f32; group.len()];
    for index in 1..(group.len() - 1) {
        let a = Vec3::from_array(group[index - 1]);
        let b = Vec3::from_array(group[index]);
        let c = Vec3::from_array(group[index + 1]);
        let ab = b - a;
        let bc = c - b;
        let ac = c - a;
        let denom = ab.length() * bc.length() * ac.length();
        if denom > 1.0e-6 {
            values[index] = 2.0 * ab.cross(ac).length() / denom;
        }
    }
    values[0] = values[1];
    values[group.len() - 1] = values[group.len() - 2];
    values
}

fn scalar_curve_group(group: &[[f32; 3]], values: &[f32]) -> Vec<[f32; 3]> {
    if group.len() != values.len() || group.len() < 2 {
        return Vec::new();
    }
    let denom = (group.len() - 1) as f32;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| Vec3::new(index as f32 / denom, *value, 0.0).to_array())
        .collect()
}

fn scalar_plot_cartesian_line_group(
    group: &[[f32; 3]],
    dep_var: &str,
    ind_var: &str,
    values: &[f32],
) -> Vec<[f32; 3]> {
    if group.len() != values.len() || group.len() < 2 {
        return Vec::new();
    }
    group
        .iter()
        .zip(values.iter())
        .filter_map(|(point, value)| {
            let independent = cartesian_axis_value(Vec3::from_array(*point), ind_var)?;
            Some(cartesian_line_point(dep_var, ind_var, independent, *value).to_array())
        })
        .collect()
}

fn axis_derivative_group(
    group: &[[f32; 3]],
    numerator_axis: usize,
    denominator_axis: usize,
) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    (0..group.len())
        .filter_map(|index| {
            let point = Vec3::from_array(group[index]);
            let denominator = axis_value(point, denominator_axis);
            let derivative = axis_derivative_value(group, index, numerator_axis, denominator_axis)?;
            let constant_axis =
                (0..3).find(|axis| *axis != numerator_axis && *axis != denominator_axis);
            let constant_value = constant_axis.map(|axis| average_axis_value(group, axis));
            Some(
                point_on_axes(
                    denominator_axis,
                    numerator_axis,
                    denominator,
                    derivative,
                    constant_axis.zip(constant_value),
                )
                .to_array(),
            )
        })
        .collect()
}

fn derivative_cartesian_line_group(
    group: &[[f32; 3]],
    dep_var: &str,
    ind_var: &str,
) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    (0..group.len())
        .filter_map(|index| {
            let current = Vec3::from_array(group[index]);
            let current_ind = cartesian_axis_value(current, ind_var)?;
            let derivative = scalar_derivative(group, index, dep_var, ind_var)?;
            Some(cartesian_line_point(dep_var, ind_var, current_ind, derivative).to_array())
        })
        .collect()
}

fn scalar_derivative(
    group: &[[f32; 3]],
    index: usize,
    dep_var: &str,
    ind_var: &str,
) -> Option<f32> {
    if group.len() < 2 {
        return None;
    }
    let (a, b) = if index == 0 {
        (0, 1)
    } else if index + 1 == group.len() {
        (group.len() - 2, group.len() - 1)
    } else {
        (index - 1, index + 1)
    };
    let pa = Vec3::from_array(group[a]);
    let pb = Vec3::from_array(group[b]);
    let da = cartesian_axis_value(pa, dep_var)?;
    let db = cartesian_axis_value(pb, dep_var)?;
    let ia = cartesian_axis_value(pa, ind_var)?;
    let ib = cartesian_axis_value(pb, ind_var)?;
    let denom = ib - ia;
    if denom.abs() <= 1.0e-6 {
        return Some(0.0);
    }
    Some((db - da) / denom)
}

fn axis_derivative_value(
    group: &[[f32; 3]],
    index: usize,
    numerator_axis: usize,
    denominator_axis: usize,
) -> Option<f32> {
    if group.len() < 2 {
        return None;
    }
    let (a, b) = if index == 0 {
        (0, 1)
    } else if index + 1 == group.len() {
        (group.len() - 2, group.len() - 1)
    } else {
        (index - 1, index + 1)
    };
    let pa = Vec3::from_array(group[a]);
    let pb = Vec3::from_array(group[b]);
    let na = axis_value(pa, numerator_axis);
    let nb = axis_value(pb, numerator_axis);
    let da = axis_value(pa, denominator_axis);
    let db = axis_value(pb, denominator_axis);
    let denom = db - da;
    if denom.abs() <= 1.0e-6 {
        return Some(0.0);
    }
    Some((nb - na) / denom)
}

fn cartesian_axis_value(point: Vec3, axis: &str) -> Option<f32> {
    match axis {
        "x" => Some(point.x),
        "y" => Some(point.y),
        "z" => Some(point.z),
        _ => None,
    }
}

fn axis_value(point: Vec3, axis: usize) -> f32 {
    match axis {
        0 => point.x,
        1 => point.y,
        _ => point.z,
    }
}

fn set_axis_value(point: &mut Vec3, axis: usize, value: f32) {
    match axis {
        0 => point.x = value,
        1 => point.y = value,
        _ => point.z = value,
    }
}

fn point_on_axes(
    independent_axis: usize,
    dependent_axis: usize,
    independent: f32,
    dependent: f32,
    constant_axis: Option<(usize, f32)>,
) -> Vec3 {
    let mut point = Vec3::ZERO;
    set_axis_value(&mut point, independent_axis, independent);
    set_axis_value(&mut point, dependent_axis, dependent);
    if let Some((axis, value)) = constant_axis {
        set_axis_value(&mut point, axis, value);
    }
    point
}

#[derive(Clone, Copy, Debug)]
struct PlanarGraphLayout {
    independent_axis: usize,
    dependent_axis: usize,
    constant_axis: usize,
    constant_value: f32,
}

fn detect_planar_graph_layout(group: &[[f32; 3]]) -> Option<PlanarGraphLayout> {
    if group.len() < 2 {
        return None;
    }
    let ranges = (0..3)
        .map(|axis| axis_range(group, axis))
        .collect::<Vec<_>>();
    let constant_axis = ranges
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(axis, _)| axis)?;
    if ranges[constant_axis] > 1.0e-3 {
        return None;
    }
    let varying_axes = (0..3)
        .filter(|axis| *axis != constant_axis)
        .collect::<Vec<_>>();
    let monotonic_axes = varying_axes
        .iter()
        .copied()
        .filter(|axis| axis_is_monotonic(group, *axis))
        .collect::<Vec<_>>();
    let independent_axis = if monotonic_axes.len() == 1 {
        monotonic_axes[0]
    } else {
        *varying_axes.iter().max_by(|a, b| {
            ranges[**a]
                .partial_cmp(&ranges[**b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })?
    };
    let dependent_axis = varying_axes
        .into_iter()
        .find(|axis| *axis != independent_axis)?;
    Some(PlanarGraphLayout {
        independent_axis,
        dependent_axis,
        constant_axis,
        constant_value: average_axis_value(group, constant_axis),
    })
}

fn derivative_planar_graph_group(group: &[[f32; 3]], layout: PlanarGraphLayout) -> Vec<[f32; 3]> {
    (0..group.len())
        .filter_map(|index| {
            let point = Vec3::from_array(group[index]);
            let independent = axis_value(point, layout.independent_axis);
            let derivative = axis_derivative_value(
                group,
                index,
                layout.dependent_axis,
                layout.independent_axis,
            )?;
            Some(
                point_on_axes(
                    layout.independent_axis,
                    layout.dependent_axis,
                    independent,
                    derivative,
                    Some((layout.constant_axis, layout.constant_value)),
                )
                .to_array(),
            )
        })
        .collect()
}

fn integral_planar_graph_group(
    group: &[[f32; 3]],
    layout: PlanarGraphLayout,
    normalize_integral: bool,
) -> Vec<[f32; 3]> {
    let mut out = Vec::with_capacity(group.len());
    let start_independent = axis_value(Vec3::from_array(group[0]), layout.independent_axis);
    let mut accum = 0.0_f32;
    out.push(
        point_on_axes(
            layout.independent_axis,
            layout.dependent_axis,
            start_independent,
            accum,
            Some((layout.constant_axis, layout.constant_value)),
        )
        .to_array(),
    );
    for pair in group.windows(2) {
        let a = Vec3::from_array(pair[0]);
        let b = Vec3::from_array(pair[1]);
        let ia = axis_value(a, layout.independent_axis);
        let ib = axis_value(b, layout.independent_axis);
        let da = axis_value(a, layout.dependent_axis);
        let db = axis_value(b, layout.dependent_axis);
        accum += (da + db) * 0.5 * (ib - ia);
        out.push(
            point_on_axes(
                layout.independent_axis,
                layout.dependent_axis,
                ib,
                accum,
                Some((layout.constant_axis, layout.constant_value)),
            )
            .to_array(),
        );
    }
    if normalize_integral {
        normalize_planar_graph_group(&mut out, layout.dependent_axis);
    }
    out
}

fn normalize_planar_graph_group(group: &mut [[f32; 3]], dependent_axis: usize) {
    if group.is_empty() {
        return;
    }
    let mean = group
        .iter()
        .map(|point| axis_value(Vec3::from_array(*point), dependent_axis))
        .sum::<f32>()
        / group.len() as f32;
    for point in group {
        let mut vec = Vec3::from_array(*point);
        let value = axis_value(vec, dependent_axis);
        set_axis_value(&mut vec, dependent_axis, value - mean);
        *point = vec.to_array();
    }
}

fn normalize_cartesian_line_group(group: &mut [[f32; 3]], dep_var: &str) {
    let Some(dep_axis) = parse_axis_name(dep_var) else {
        return;
    };
    normalize_planar_graph_group(group, dep_axis);
}

fn parse_axis_name(axis: &str) -> Option<usize> {
    match axis {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    }
}

fn axis_range(group: &[[f32; 3]], axis: usize) -> f32 {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for point in group {
        let value = axis_value(Vec3::from_array(*point), axis);
        min = min.min(value);
        max = max.max(value);
    }
    max - min
}

fn average_axis_value(group: &[[f32; 3]], axis: usize) -> f32 {
    group
        .iter()
        .map(|point| axis_value(Vec3::from_array(*point), axis))
        .sum::<f32>()
        / group.len() as f32
}

fn axis_is_monotonic(group: &[[f32; 3]], axis: usize) -> bool {
    let epsilon = 1.0e-6_f32;
    let mut nondecreasing = true;
    let mut nonincreasing = true;
    for pair in group.windows(2) {
        let a = axis_value(Vec3::from_array(pair[0]), axis);
        let b = axis_value(Vec3::from_array(pair[1]), axis);
        if b + epsilon < a {
            nondecreasing = false;
        }
        if b > a + epsilon {
            nonincreasing = false;
        }
    }
    nondecreasing || nonincreasing
}

fn cartesian_line_point(dep_var: &str, ind_var: &str, independent: f32, dependent: f32) -> Vec3 {
    match (dep_var, ind_var) {
        ("y", "x") => Vec3::new(independent, dependent, 0.0),
        ("z", "x") => Vec3::new(independent, 0.0, dependent),
        ("z", "y") => Vec3::new(0.0, independent, dependent),
        ("x", "y") => Vec3::new(dependent, independent, 0.0),
        ("x", "z") => Vec3::new(dependent, 0.0, independent),
        ("y", "z") => Vec3::new(0.0, dependent, independent),
        _ => Vec3::new(independent, dependent, 0.0),
    }
}

fn integral_cartesian_line_group(
    group: &[[f32; 3]],
    dep_var: &str,
    ind_var: &str,
    normalize_integral: bool,
) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(group.len());
    let start_independent =
        cartesian_axis_value(Vec3::from_array(group[0]), ind_var).unwrap_or(0.0);
    let mut accum = 0.0_f32;
    out.push(cartesian_line_point(dep_var, ind_var, start_independent, accum).to_array());
    for pair in group.windows(2) {
        let a = Vec3::from_array(pair[0]);
        let b = Vec3::from_array(pair[1]);
        let ia = match cartesian_axis_value(a, ind_var) {
            Some(value) => value,
            None => continue,
        };
        let ib = match cartesian_axis_value(b, ind_var) {
            Some(value) => value,
            None => continue,
        };
        let da = match cartesian_axis_value(a, dep_var) {
            Some(value) => value,
            None => continue,
        };
        let db = match cartesian_axis_value(b, dep_var) {
            Some(value) => value,
            None => continue,
        };
        accum += (da + db) * 0.5 * (ib - ia);
        out.push(cartesian_line_point(dep_var, ind_var, ib, accum).to_array());
    }
    if normalize_integral {
        normalize_cartesian_line_group(&mut out, dep_var);
    }
    out
}

fn normal_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 3 {
        return Vec::new();
    }
    let tangents = tangent_vectors(group);
    tangents
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let dt = if index == 0 {
                tangents[1] - tangents[0]
            } else if index + 1 == tangents.len() {
                tangents[index] - tangents[index - 1]
            } else {
                (tangents[index + 1] - tangents[index - 1]) * 0.5
            };
            dt.normalize_or_zero().to_array()
        })
        .collect()
}

fn binormal_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 3 {
        return Vec::new();
    }
    let tangents = tangent_vectors(group);
    let normals = normal_curve_group(group);
    tangents
        .iter()
        .zip(normals.iter())
        .map(|(tangent, normal)| {
            tangent
                .cross(Vec3::from_array(*normal))
                .normalize_or_zero()
                .to_array()
        })
        .collect()
}

fn tangent_vectors(group: &[[f32; 3]]) -> Vec<Vec3> {
    if group.len() < 2 {
        return Vec::new();
    }
    (0..group.len())
        .map(|index| finite_difference(group, index).normalize_or_zero())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1.0e-3
    }

    #[test]
    fn derivative_of_planar_xz_curve_stays_in_xz_plane() {
        let group = vec![[0.0, 4.0, 0.0], [1.0, 4.0, 1.0], [2.0, 4.0, 0.0]];

        let derived = derivative_curve_group(&group);

        assert_eq!(derived.len(), 3);
        assert!(approx_eq(derived[1][0], 1.0));
        assert!(approx_eq(derived[1][1], 4.0));
        assert!(approx_eq(derived[1][2], 0.0));
    }

    #[test]
    fn axis_derivative_preserves_constant_plane_axis() {
        let group = vec![[0.0, 4.0, 0.0], [1.0, 4.0, 1.0], [2.0, 4.0, 0.0]];

        let derived = axis_derivative_group(&group, 2, 0);

        assert_eq!(derived.len(), 3);
        assert!(approx_eq(derived[1][0], 1.0));
        assert!(approx_eq(derived[1][1], 4.0));
        assert!(approx_eq(derived[1][2], 0.0));
    }

    #[test]
    fn integral_of_planar_xz_curve_stays_in_xz_plane() {
        let group = vec![[0.0, 4.0, 0.0], [1.0, 4.0, 1.0], [2.0, 4.0, 0.0]];

        let derived = integral_curve_group(&group, false);

        assert_eq!(derived.len(), 3);
        assert!(approx_eq(derived[0][1], 4.0));
        assert!(approx_eq(derived[1][1], 4.0));
        assert!(approx_eq(derived[2][1], 4.0));
        assert!(approx_eq(derived[2][2], 1.0));
    }

    #[test]
    fn repeated_integral_of_sine_stays_centered_when_normalized() {
        let group = (-80..=80)
            .map(|i| {
                let x = i as f32 * 0.1;
                [x, 4.0, x.sin()]
            })
            .collect::<Vec<_>>();

        let first = integral_curve_group(&group, true);
        let second = integral_curve_group(&first, true);

        assert_eq!(second.len(), group.len());
        let mean = second.iter().map(|point| point[2]).sum::<f32>() / second.len() as f32;
        let left = second.first().map(|point| point[2]).unwrap_or_default();
        let right = second.last().map(|point| point[2]).unwrap_or_default();

        assert!(mean.abs() < 1.0e-3);
        assert!((left - right).abs() < 0.2);
    }
}
