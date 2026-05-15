use super::example_plots::ExamplePlot;
use crate::PlotEntry;

pub fn build() -> Vec<PlotEntry> {
    vec![
        ExamplePlot::ContouredSurface.build(),
        ExamplePlot::ScatterCloud.build(),
        ExamplePlot::VectorField.build(),
    ]
}
