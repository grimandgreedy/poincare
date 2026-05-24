use std::collections::HashMap;
use std::f64::consts::PI;

use glam::Vec3;

use crate::{
    ColourMode, CurveInterpolation, CurveInterpolationKind, Diagnostic, DiagnosticKind,
    PlotMetadata, PlotSpec, PlotStyle, PointAnnotation, SliceAxis, TableDataSet,
    default_slice_position, eval_curve_point, eval_with_vars, parse_curve_expr,
    parse_expr_with_vars, sample_curve_points,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisKind {
    InterpolateCurve,
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
    capabilities_for_metadata(&metadata)
}

pub fn sample_groups(
    plot: &PlotSpec,
    kind: SampleGroupsKind,
) -> Result<Vec<Vec<[f32; 3]>>, AnalysisError> {
    match kind {
        SampleGroupsKind::Curve => curve_sample_groups(plot),
        SampleGroupsKind::Polyline => polyline_sample_groups(plot),
        SampleGroupsKind::InterpolationSource => interpolation_source_groups(plot),
    }
}

pub fn run_analysis(plot: &PlotSpec, request: &AnalysisRequest) -> Result<AnalysisOutput, AnalysisError> {
    let params = parameter_map(&request.parameters);
    let provenance = AnalysisProvenance {
        kind: request.kind,
        source_plots: vec![plot.name.clone()],
        parameters: request.parameters.clone(),
        notes: Vec::new(),
    };

    let plots = match request.kind {
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
        AnalysisKind::AxisDerivativeCurve => vec![make_axis_derivative_plot(
            plot,
            parse_axis_index(params.get("numerator_axis").map(String::as_str)).unwrap_or(1),
            parse_axis_index(params.get("denominator_axis").map(String::as_str)).unwrap_or(0),
            params.get("output_name").cloned(),
        )?],
        AnalysisKind::IntegralCurve => vec![make_curve_integral_plot(plot)?],
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
                parameters: vec![],
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

fn make_curve_integral_plot(source: &PlotSpec) -> Result<PlotSpec, AnalysisError> {
    let groups = curve_sample_groups(source)?;
    let derived_groups = match &source.definition {
        crate::PlotDefinition::ExprCartesianLine {
            dep_var, ind_var, ..
        } => groups
            .iter()
            .map(|group| integral_cartesian_line_group(group, dep_var.as_str(), ind_var.as_str()))
            .filter(|group| group.len() >= 2)
            .collect(),
        _ => groups
            .iter()
            .map(|group| integral_curve_group(group))
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
    let positions = polyline_sample_groups(source)?.into_iter().flatten().collect::<Vec<_>>();
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
        crate::PlotDefinition::PointAnnotations { points, .. } => Ok(vec![points
            .iter()
            .map(|point| point.position)
            .collect()]),
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
                &points.iter().map(|point| Vec3::from_array(*point)).collect::<Vec<_>>(),
                *interpolation,
            );
            Ok(vec![sampled.into_iter().map(|point| point.to_array()).collect()])
        }
        _ => Err(AnalysisError::unsupported(
            "Point extraction is available for polyline and interpolated curve plots.",
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
                    Vec3::new((t.cos() * 3.0) as f32, (t.sin() * 3.0) as f32, (t * 0.15) as f32)
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
            let parsed = parse_expr_with_vars(expression, &[ind_var.as_str()]).map_err(parse_error)?;
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
                    cartesian_line_point(dep.as_str(), ind.as_str(), t as f32, val as f32).to_array()
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
                &points.iter().map(|point| Vec3::from_array(*point)).collect::<Vec<_>>(),
                *interpolation,
            );
            Ok(vec![sampled.into_iter().map(|point| point.to_array()).collect()])
        }
        _ => Err(AnalysisError::unsupported(
            "Curve calculus tools are available for curve and polyline plots.",
        )),
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

fn parameter_map(parameters: &[(String, String)]) -> HashMap<String, String> {
    parameters.iter().cloned().collect()
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
    (0..group.len())
        .map(|index| finite_difference(group, index).to_array())
        .collect()
}

fn tangent_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    (0..group.len())
        .map(|index| finite_difference(group, index).normalize_or_zero().to_array())
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

fn integral_curve_group(group: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
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
            Some(Vec3::new(denominator, derivative, 0.0).to_array())
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
) -> Vec<[f32; 3]> {
    if group.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(group.len());
    let start_independent = cartesian_axis_value(Vec3::from_array(group[0]), ind_var).unwrap_or(0.0);
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
        .map(|(tangent, normal)| tangent.cross(Vec3::from_array(*normal)).normalize_or_zero().to_array())
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
