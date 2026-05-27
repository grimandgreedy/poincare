pub use crate::table_data::TableVectorSample;
use crate::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PlotGeometry, PlotObject, PlotStyle,
    Resolution,
};

pub struct TableVectorFieldPlot {
    samples: Vec<TableVectorSample>,
    bounds: DataBounds,
    style: PlotStyle,
}

impl TableVectorFieldPlot {
    pub fn new(samples: Vec<TableVectorSample>, bounds: DataBounds, style: PlotStyle) -> Self {
        Self {
            samples,
            bounds,
            style,
        }
    }
}

impl PlotObject for TableVectorFieldPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        Some(self.bounds.clone())
    }

    fn generate(&self, _domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let glyphs = self
            .samples
            .iter()
            .map(|sample| GlyphInstance {
                position: sample.position,
                vector: sample.vector.normalize_or_zero(),
                raw_vector: sample.vector,
            })
            .collect();
        PlotGeometry::Glyphs(glyphs)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}
