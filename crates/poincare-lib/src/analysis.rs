use crate::{Diagnostic, PlotMetadata, PlotSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisKind {
    InterpolateCurve,
    DifferentiateCurve,
    IntegralCurve,
    ArcLengthCurve,
    CurvatureCurve,
    TangentField,
    NormalField,
    BinormalField,
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

#[derive(Clone, Debug)]
pub struct AnalysisError {
    pub diagnostic: Diagnostic,
}

impl AnalysisError {
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            diagnostic: Diagnostic::error(crate::DiagnosticKind::Build, message),
        }
    }
}

pub fn available_analyses(plot: &PlotSpec) -> Vec<AnalysisCapability> {
    let metadata = plot.metadata();
    capabilities_for_metadata(&metadata)
}

fn capabilities_for_metadata(metadata: &PlotMetadata) -> Vec<AnalysisCapability> {
    let mut capabilities = Vec::new();

    if metadata.style_caps.line {
        capabilities.extend([
            AnalysisCapability {
                kind: AnalysisKind::InterpolateCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["interpolation"],
            },
            AnalysisCapability {
                kind: AnalysisKind::DifferentiateCurve,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples"],
            },
            AnalysisCapability {
                kind: AnalysisKind::IntegralCurve,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples"],
            },
            AnalysisCapability {
                kind: AnalysisKind::ArcLengthCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples"],
            },
            AnalysisCapability {
                kind: AnalysisKind::CurvatureCurve,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples"],
            },
            AnalysisCapability {
                kind: AnalysisKind::TangentField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples", "scale"],
            },
            AnalysisCapability {
                kind: AnalysisKind::NormalField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples", "scale"],
            },
            AnalysisCapability {
                kind: AnalysisKind::BinormalField,
                target_kind: AnalysisTargetKind::SampledData,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["samples", "scale"],
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
                parameters: vec!["step"],
            },
            AnalysisCapability {
                kind: AnalysisKind::DivergenceField,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["step", "resolution"],
            },
            AnalysisCapability {
                kind: AnalysisKind::CurlField,
                target_kind: AnalysisTargetKind::Definition,
                output_kind: AnalysisOutputKind::PlotSpec,
                parameters: vec!["step"],
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
