use super::example_plots::ExamplePlot;
use crate::PlotEntry;

pub fn build() -> Vec<PlotEntry> {
    vec![
        ExamplePlot::ContouredSurface.build(),
        ExamplePlot::SphericalHarmonic.build(),
        ExamplePlot::GridSurface.build(),
    ]
}
