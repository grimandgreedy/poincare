use poincare_lib::{Domain, GraphSpec, PlotSpec, PlotStyle, Resolution};

use crate::plot::kind::PlotKind;

#[derive(Clone)]
pub(crate) struct PlotEntry {
    pub(crate) name: String,
    pub(crate) visible: bool,
    pub(crate) domain: Domain,
    pub(crate) resolution: Resolution,
    pub(crate) style: PlotStyle,
    pub(crate) kind: PlotKind,
}

impl PlotEntry {
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

pub(crate) fn build_graph_spec(entries: &[PlotEntry], axis_config: poincare_lib::AxisConfig) -> GraphSpec {
    GraphSpec {
        axis_config,
        plots: entries.iter().map(PlotEntry::to_plot_spec).collect(),
    }
}
