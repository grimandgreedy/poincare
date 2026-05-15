use crate::coordinate::CoordinateSystem;
use crate::domain::{DataBounds, Domain};
use crate::plot_object::{PlotComponent, PlotGeometry, PlotObject};
use crate::resolution::Resolution;
use crate::style::PlotStyle;

struct Piece {
    domain: Domain,
    plot: Box<dyn PlotObject>,
}

/// A piecewise composition of independently-defined plot objects.
pub struct PiecewisePlot {
    pieces: Vec<Piece>,
    style: PlotStyle,
}

impl PiecewisePlot {
    pub fn new() -> Self {
        Self {
            pieces: Vec::new(),
            style: PlotStyle::default(),
        }
    }

    pub fn add_piece(&mut self, domain: Domain, plot: impl PlotObject + 'static) {
        self.pieces.push(Piece {
            domain,
            plot: Box::new(plot),
        });
    }

    pub fn with_piece(mut self, domain: Domain, plot: impl PlotObject + 'static) -> Self {
        self.add_piece(domain, plot);
        self
    }
}

impl Default for PiecewisePlot {
    fn default() -> Self {
        Self::new()
    }
}

impl PlotObject for PiecewisePlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        let mut iter = self.pieces.iter();
        let first = iter.next()?;
        let mut x_min = *first.domain.x.start();
        let mut x_max = *first.domain.x.end();
        let mut y_min = *first.domain.y.start();
        let mut y_max = *first.domain.y.end();
        let mut z_min = *first.domain.z.start();
        let mut z_max = *first.domain.z.end();

        for piece in iter {
            x_min = x_min.min(*piece.domain.x.start());
            x_max = x_max.max(*piece.domain.x.end());
            y_min = y_min.min(*piece.domain.y.start());
            y_max = y_max.max(*piece.domain.y.end());
            z_min = z_min.min(*piece.domain.z.start());
            z_max = z_max.max(*piece.domain.z.end());
        }

        Some(DataBounds {
            x: x_min..=x_max,
            y: y_min..=y_max,
            z: z_min..=z_max,
        })
    }

    fn generate(&self, _domain: &Domain, resolution: Resolution) -> PlotGeometry {
        let components = self
            .pieces
            .iter()
            .map(|piece| PlotComponent {
                geometry: piece.plot.generate(&piece.domain, resolution),
                style: piece.plot.style().clone(),
            })
            .collect();
        PlotGeometry::Composite(components)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }

    fn resolution(&self) -> Resolution {
        Resolution::default()
    }
}
