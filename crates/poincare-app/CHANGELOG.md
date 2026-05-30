# Changelog

All notable changes to this crate will be documented in this file.

## [Unreleased]

### Improvements
- Added selected-plot analysis actions to the command palette and sorted disabled commands to the bottom of filtered results.
- Made example plots append to the current document instead of replacing existing plots.
- Changed empty viewport clicks to keep the same sidebar selection while clearing the in-scene selection highlight, and softened selected-object highlight intensity.
- Renamed the plot-properties panel source file from `right_panel.rs` to `bottom_panel.rs`.
- Added a reusable `Data` panel for analysis-driven reports and tables, plus a reusable `data_table` UI component shared with cell-style data editing.
- Restored modal editing for plot data while keeping the `Data` panel analysis-only.
- Added structured plot-metadata clipboard export from the plot-properties header, including sample-group support and available analysis capability summaries.
- Switched the app-wide default font to `Hack-Regular.ttf` with the Nerd Font kept as a glyph fallback for icon coverage.
- Added app-level plot hierarchy metadata so derived analysis plots can attach to their source plot as child plots.
- Updated the left plot list to render simple parent/child nesting for derived plots, with child rows indented and width-reduced relative to their parent rows.

### Analysis
- Added `Point Statistics` and `Data Quality Checks` actions in the `Analysis` tab and command palette for sample-backed plots.
- Added a right-side `Data` tab that opens for Phase 3 analysis results and shows reports, diagnostics, and read-only tables.
- Added source-data tables to Phase 3 analysis results so reports include the sampled positions they were computed from.
- Added titled analysis tables in the `Data` tab so Phase 3 flagged-sample outputs are surfaced as named datasets instead of anonymous table slots.
- Made point-statistics and data-quality tools available for more sample-backed plot kinds, including imported scatter data and built-in scatter clouds.
- Allowed point-statistics analysis for single-point datasets while keeping data-quality checks gated to datasets with at least two samples.
- Added Phase 4 surface-analysis actions in the `Analysis` tab and command palette for normals, curvature, area, and mesh-quality workflows backed by cached surface meshes.
- Changed bulk analysis-generated markers to default to unlabeled points instead of auto-generated `label 1`, `label 2`-style annotation text, while keeping meaningful labels such as centroids and PCA axes.
- Switched analysis normals and glyph-oriented vector defaults to `RdBu` so direction-heavy glyph plots are easier to distinguish at a glance.

### Fixes
- Fixed analysis-result table widget id collisions in the `Data` panel.
- Fixed the `Data` tab open/close flow so analysis actions open the tab reliably and switching to unrelated plots closes it and returns focus to `Plot Properties`.
- Fixed left-panel plot row sizing regressions introduced by the font change, including over-wide rows and incorrect child-row width handling.
- Switched `grimdock` to the local path dependency and added per-tab max-width overrides so dock tab headers can stay compact by default while giving `Plot Properties` a wider cap.
- Fixed glyph-scale controls for analysis arrows, normals, and other glyph-backed plots by routing size through the renderer's glyph-scale path instead of baking it into the instance vectors.

## [0.6.0]

### Improvements
- Replaced the left-pane colour dots with plot-type markers for points, curves, streamlines, surfaces, isosurfaces, volumes, and vector fields, and reused those markers in the plot-properties header.
- Added plot-properties summaries that show each selected plot's type plus available sample counts, including point totals for point/polyline-style plots and imported table datasets.
- Added an `Interpolation` workflow in the `Analysis` tab for point and ordered-sample plots, with derived interpolated-curve plot generation and persisted method settings.
- Added smoothing-oriented curve methods in the interpolation modal, including `Smoothing (Moving Average)` and `Smoothing (Savitzky-Golay)`, backed by `poincare-lib` curve sampling support.
- Added `Extract Points` in the `Analysis` tab for polyline-like plots, including imported curves, derived polylines, and interpolated curves.
- Expanded `Curve Analysis` in the `Analysis` tab with derivative, integral, tangent, arc-length, curvature, normal, and binormal derived-curve outputs, including graph-space handling for Cartesian line plots.
- Added a modal-driven `Differentiate by Axis...` curve-analysis workflow for sampled and expression-backed curves, with explicit axis choices for outputs such as `dy/dx` and `dz/dx`.
- Added a modal-driven `Fit Curve...` analysis workflow with polynomial, robust polynomial, spline, and Fourier fitting, plus optional control-point previews, residual plots, and fit diagnostics.
- Added keyboard shortcuts for selected-plot workflows, including `V` to toggle visibility, `J`/`K` to cycle plot selection, `E` to edit the selected plot, and `Shift+A` to open the add-plot modal with input focus.
- Added selected-plot editing flows that open the equation editor for expression-backed plots and a data editor for imported tables, intersection markers, and derived intersection polylines.
- Added raw/cell editing modes to the data editor, including spreadsheet-style contiguous cell editing and 100-row paging for larger datasets.
- Made the left plot list auto-scroll to keep the newly selected plot visible.

## [0.5.0]

### Features
- Added document-level undo/redo with menu items, command-palette actions, and `Cmd/Ctrl+Z` / `Cmd/Ctrl+Shift+Z` shortcuts.
- Added vector-field glyph shape selection in plot style controls.
- Added a growable saved-view list in the camera tab, with track playback built from those saved views using `viewport-lib` camera interpolation.
- Added a dedicated bottom `Export` tab with PNG export plus GIF/MP4 animation export driven by the saved-view track.
- Updated the export tab to use image/video modes, home-directory defaults, and directory selection for export destinations.
- Added canvas-based plot picking using `viewport-lib`'s unified picker, including click-to-select, hover highlight, and double-click frame-to-selection.
- Added probe pinning with persistent pinned points, distance measurement between pinned points, and `Cmd+Click` pinning in the viewport.
- Added table-backed import editing for surface grids, curves, scatter plots, and sampled vector fields, including preview, delimiter/header detection, explicit column mapping, validation, and refresh-from-file support.
- Added an `Analysis` inspector tab for generating slice plots, contour cross-sections, gradient/divergence/curl derived plots, probe annotations, pinned-probe sample plots, and cached curve-intersection marker plots as normal persisted plot entries.
- Added surface-surface intersection extraction in the `Analysis` tab, including target-surface selection, tolerance/stitch controls, derived intersection curve plots, and optional isolated-contact point markers.

### Fixes
- Focus on plot properties tab by default.
- Fixed the viewport going black after the latest `viewport-lib` HDR callback-path changes by returning the renderer's `prepare()` command buffers from the egui viewport callback.
- Fixed vector-field glyph colouring so `Glyph Scale` no longer changes magnitude-based colour mapping.
- Fixed vector-field opacity controls so the style opacity slider affects rendered glyphs.
- Made vector fields render unlit by default so glyphs stay clearly visible regardless of scene lighting.
- Fixed animated export output naming so GIF/MP4 exports use the correct file extension automatically.
- Added export progress reporting: determinate progress while rendering animation frames, and pending status while `ffmpeg` encodes the final file.
- Fixed a crash during parameter sweeps and camera playback by releasing old GPU scene meshes before rebuilt meshes are uploaded.
- Fixed volume plots so they participate in viewport selection.
- Fixed probe pinning so the pin action still works after moving from the plotted point to the button.
- Fixed quit confirmation so `Quit Without Saving` actually closes the app after confirmation.
- Removed the temporary `Solo` viewport control.
- Fixed generated annotation plots so label visibility can be toggled per plot and large annotation sets no longer truncate label editing after the first four items.
- Added modal close handling so the equation editor and data editor discard immediately when unchanged, and otherwise prompt with `discard` / `save` / `cancel` when closed via `Escape`.
- Made the add-plot modal close on `Escape` when its inputs are still empty.

## [0.3.0]

- Added a dedicated `Camera` tab as a second bottom `grimdock` tab alongside plot properties.
- Added animated camera controls for named views, framing, projection switching, and saved camera slots.
- Routed camera actions through shared commands so the bottom camera tab, View menu, command palette, axis indicator, and shortcuts use consistent behaviour.
- Added a `Help` menu with a `Keyboard Shortcuts` modal documenting the app's current shortcuts.
- Refactored expression-parameter sweep controls into a reusable scalar control component shared with domain range editing.
- Cleaned up plot property layout for parameter/domain controls, including improved alignment, compact sizing, and better behaviour in wide inspector panes.

## [0.2.0] - Initial pre-release

- Initial crate release.
