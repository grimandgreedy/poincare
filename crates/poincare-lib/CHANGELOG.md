# Changelog

All notable changes to this crate will be documented in this file.

## [Unreleased]
- Added Phase 3 analysis support with `PointCloudStatistics` and `DataQualityChecks`.
- Added sample-data analysis for point-backed and sampled plots, including imported scatter data, vector sample positions, surface-grid samples, interpolated curves, derived polylines, and built-in scatter clouds.
- Added point-cloud statistics outputs including centroid, bounds, variance/covariance, PCA directions, and optional derived centroid/PCA geometry.
- Added data-quality outputs including exact duplicate detection, near-duplicate detection, nearest-neighbour spacing diagnostics, sparse-sample warnings, monotonicity summaries, and positional outlier detection with optional derived outlier geometry.
- Added `AnalysisOutput::Composite` report/table/diagnostic flows used by richer analysis results.
- Added `SampleGroupsKind::SampleData` so applications can query general sample-backed plot data consistently.
- Improved curve integral behaviour for planar graph-style curves by normalizing antiderivatives to avoid repeated-integration drift from carried offsets.
- Fixed curve derivative and integral outputs for planar sampled curves so graph-style plots preserve their original plotting plane.

## [0.5.0]
- Redefine the intended app/lib boundary around equations/plot structs.


## [0.4.0]

- Added `PlotStyle::glyph_type` so vector-field plots can choose between supported glyph meshes.
- Added selected-item-aware frame building so applications can mark picked plots for viewport highlighting.
- Added scene support for plot-owned world-space labels via `PlotGeometry::Labels`, allowing applications to persist annotations as normal plots instead of special overlay state.
- Exposed owning plot pick identifiers through probe surface data so applications can map cached CPU surface meshes back to plot entries for derived analysis tools.
- Fixed glyph appearance opacity to use `PlotStyle::opacity`.
- Fixed vector-field scalar colouring to use raw field vectors instead of display-scaled glyph vectors.
- Made vector-field glyph rendering unlit for consistent readability.
- Added `GraphScene::release_gpu_resources` and fixed repeated scene rebuilds to release old uploaded surface meshes before replacing them.
- Fixed cached volume submissions so they retain CPU `VolumeData` for unified `viewport-lib` picking.

## [0.2.0]

- No public API changes. The new camera controls and keyboard-shortcuts help UI were added in `poincare-app`.

## [0.1.0] - Initial pre-release

- Initial crate release.
