use poincare_lib::{Domain, GraphSpec, PlotSpec, PlotStyle, Resolution};

use crate::plot::kind::PlotKind;

pub(crate) type PlotId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlotRelationship {
    Primary,
    DerivedAnalysis,
}

#[derive(Clone)]
pub(crate) struct PlotEntry {
    pub(crate) plot_id: PlotId,
    pub(crate) parent_plot_id: Option<PlotId>,
    pub(crate) relationship: PlotRelationship,
    pub(crate) name: String,
    pub(crate) visible: bool,
    pub(crate) domain: Domain,
    pub(crate) resolution: Resolution,
    pub(crate) style: PlotStyle,
    pub(crate) kind: PlotKind,
}

impl PlotEntry {
    pub(crate) fn from_plot_spec(spec: PlotSpec) -> Self {
        Self {
            plot_id: 0,
            parent_plot_id: None,
            relationship: PlotRelationship::Primary,
            name: spec.name,
            visible: spec.visible,
            domain: spec.domain,
            resolution: spec.resolution,
            style: spec.style,
            kind: spec.definition,
        }
    }

    pub(crate) fn as_analysis_child(mut self, parent_plot_id: PlotId) -> Self {
        self.parent_plot_id = Some(parent_plot_id);
        self.relationship = PlotRelationship::DerivedAnalysis;
        self
    }

    pub(crate) fn to_plot_spec(&self) -> PlotSpec {
        PlotSpec {
            name: self.name.clone(),
            visible: self.visible,
            domain: self.domain.clone(),
            resolution: self.resolution,
            style: self.style.clone(),
            definition: self.kind.clone(),
        }
    }
}

pub(crate) fn build_graph_spec(
    entries: &[PlotEntry],
    axis_config: poincare_lib::AxisConfig,
) -> GraphSpec {
    GraphSpec {
        axis_config,
        plots: entries.iter().map(PlotEntry::to_plot_spec).collect(),
    }
}
