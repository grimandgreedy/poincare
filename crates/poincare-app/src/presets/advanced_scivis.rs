use super::example_plots::ExamplePlot;
use crate::PlotEntry;

pub fn build() -> Vec<PlotEntry> {
    vec![
        ExamplePlot::Streamlines.build(),
        ExamplePlot::VolumeRender.build(),
        ExamplePlot::Isosurface.build(),
    ]
}
