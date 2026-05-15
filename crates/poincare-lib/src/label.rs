//! World-space label type alias.
//!
//! Re-exports [`viewport_lib::LabelItem`] as [`WorldLabel`] so callers can use
//! the Poincaré naming convention without importing the viewport crate directly.

/// A text label pinned to a 3D world-space position.
///
/// This is a type alias for [`viewport_lib::LabelItem`].
/// Use it to create tick labels, axis name labels, or any other annotation.
///
/// # Example
///
/// ```rust,ignore
/// use poincare_lib::WorldLabel;
///
/// let mut label = WorldLabel::default();
/// label.world_anchor = Some([5.0, 0.0, 0.0]);
/// label.text = "x = 5".to_string();
/// ```
pub type WorldLabel = viewport_lib::LabelItem;
