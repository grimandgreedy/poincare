# Changelog

All notable changes to this crate will be documented in this file.

This project uses internal pre-release versioning. Until `1.0.0`, breaking changes may be released in minor versions.

## [Unreleased]

- Added `PlotStyle::glyph_type` so vector-field plots can choose between supported glyph meshes.
- Fixed glyph appearance opacity to use `PlotStyle::opacity`.
- Fixed vector-field scalar colouring to use raw field vectors instead of display-scaled glyph vectors.
- Made vector-field glyph rendering unlit for consistent readability.
- Added `GraphScene::release_gpu_resources` and fixed repeated scene rebuilds to release old uploaded surface meshes before replacing them.

## [0.2.0]

- No public API changes. The new camera controls and keyboard-shortcuts help UI were added in `poincare-app`.

## [0.1.0] - Initial pre-release

- Initial crate release.
