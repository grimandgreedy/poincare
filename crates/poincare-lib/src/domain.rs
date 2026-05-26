use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// The plotting domain — the range of each axis over which plots are evaluated.
///
/// Default is [-10, 10] on all axes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub x: RangeInclusive<f64>,
    pub y: RangeInclusive<f64>,
    pub z: RangeInclusive<f64>,
}

impl Default for Domain {
    fn default() -> Self {
        Self {
            x: -10.0..=10.0,
            y: -10.0..=10.0,
            z: -10.0..=10.0,
        }
    }
}

/// The natural data bounds of a plot object (for auto-fitting).
///
/// `PlotObject::natural_bounds()` returns `None` for analytical functions
/// where auto-fitting from output doesn't make semantic sense.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataBounds {
    pub x: RangeInclusive<f64>,
    pub y: RangeInclusive<f64>,
    pub z: RangeInclusive<f64>,
}
