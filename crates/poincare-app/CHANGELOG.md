# Changelog

All notable changes to this crate will be documented in this file.

This project uses internal pre-release versioning. Until `1.0.0`, breaking changes may be released in minor versions.

## [Unreleased]

### Features
- Added document-level undo/redo with menu items, command-palette actions, and `Cmd/Ctrl+Z` / `Cmd/Ctrl+Shift+Z` shortcuts.
- Added vector-field glyph shape selection in plot style controls.
- Added a growable saved-view list in the camera tab, with track playback built from those saved views using `viewport-lib` camera interpolation.
- Added a dedicated bottom `Export` tab with PNG export plus GIF/MP4 animation export driven by the saved-view track.
- Updated the export tab to use image/video modes, home-directory defaults, and directory selection for export destinations.
- Added canvas-based plot picking using `viewport-lib`'s unified picker, including click-to-select, hover highlight, and double-click frame-to-selection.
- Added probe pinning with persistent pinned points, distance measurement between pinned points, and `Cmd+Click` pinning in the viewport.

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

## [0.3.0]

- Added a dedicated `Camera` tab as a second bottom `grimdock` tab alongside plot properties.
- Added animated camera controls for named views, framing, projection switching, and saved camera slots.
- Routed camera actions through shared commands so the bottom camera tab, View menu, command palette, axis indicator, and shortcuts use consistent behaviour.
- Added a `Help` menu with a `Keyboard Shortcuts` modal documenting the app's current shortcuts.
- Refactored expression-parameter sweep controls into a reusable scalar control component shared with domain range editing.
- Cleaned up plot property layout for parameter/domain controls, including improved alignment, compact sizing, and better behaviour in wide inspector panes.

## [0.2.0] - Initial pre-release

- Initial crate release.
