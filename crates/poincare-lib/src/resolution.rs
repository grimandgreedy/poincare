/// Mesh sampling resolution for evaluating plot functions on a grid.
///
/// `u` samples along the first axis, `v` along the second.
/// Default 100×100 gives smooth surfaces for typical analytical functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub u: u32,
    pub v: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self { u: 100, v: 100 }
    }
}
