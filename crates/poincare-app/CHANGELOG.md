# Changelog

All notable changes to this crate will be documented in this file.

This project uses internal pre-release versioning. Until `1.0.0`, breaking changes may be released in minor versions.

## [Dev, Unreleased Changes]

### Improvements
- Replaced the left-pane colour dots with plot-type markers for points, curves, streamlines, surfaces, isosurfaces, volumes, and vector fields, and reused those markers in the plot-properties header.
- Added plot-properties summaries that show each selected plot's type plus available sample counts, including point totals for point/polyline-style plots and imported table datasets.
- Added an `Interpolation` workflow in the `Analysis` tab for point and ordered-sample plots, with derived interpolated-curve plot generation and persisted method settings.
- Added smoothing-oriented curve methods in the interpolation modal, including `Smoothing (Moving Average)` and `Smoothing (Savitzky-Golay)`, backed by `poincare-lib` curve sampling support.
- Added `Extract Points` in the `Analysis` tab for polyline-like plots, including imported curves, derived polylines, and interpolated curves.
- Expanded `Curve Analysis` in the `Analysis` tab with derivative, integral, tangent, arc-length, curvature, normal, and binormal derived-curve outputs, including graph-space handling for Cartesian line plots.
- Added a modal-driven `Differentiate by Axis...` curve-analysis workflow for sampled and expression-backed curves, with explicit axis choices for outputs such as `dy/dx` and `dz/dx`.




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

## [0.3.0]

- Added a dedicated `Camera` tab as a second bottom `grimdock` tab alongside plot properties.
- Added animated camera controls for named views, framing, projection switching, and saved camera slots.
- Routed camera actions through shared commands so the bottom camera tab, View menu, command palette, axis indicator, and shortcuts use consistent behaviour.
- Added a `Help` menu with a `Keyboard Shortcuts` modal documenting the app's current shortcuts.
- Refactored expression-parameter sweep controls into a reusable scalar control component shared with domain range editing.
- Cleaned up plot property layout for parameter/domain controls, including improved alignment, compact sizing, and better behaviour in wide inspector panes.

## [0.2.0] - Initial pre-release

- Initial crate release.
