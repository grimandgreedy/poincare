# poincare-mobile

Experimental mobile shell for Poincare.

M1 renders one active preset plot full-screen using `winit`, `wgpu`,
`viewport-lib`, and `poincare-lib`.

Controls:

- 1-finger drag: orbit
- 2-finger drag: pan
- 2-finger pinch: zoom
- 3-finger touch: cycle the active preset plot and rebuild/reupload the scene
- Desktop helper: Space or ArrowRight cycles the active preset plot

## UI frontend boundary

The mobile shell keeps viewport/rendering state separate from menu commands.
`crates/poincare-mobile-core` defines the small command/snapshot/model surface
that any UI layer should use:

- `UiSnapshot`: read-only state needed by menus and sheets
- `UiCommand`: actions such as opening the drawer, selecting a preset, or
  submitting the equation editor
- `MobileModel`: renderer-independent state transition layer

The current implementation uses `egui-winit`/`egui-wgpu` as an overlay in the
same `winit` window. `src/mobile_ui.rs` is the start of a small mobile component
layer for that path: touch-sized icon buttons, compact action buttons, a drawer,
bottom sheet, and shared mobile styling. It emits `UiCommand` values instead of
mutating renderer state directly.

The Tauri/WebView prototype in `crates/poincare-mobile-tauri` is retained as an
experiment and calls into the same command layer. It is not the primary UI path:
Tauri is a WebView app model, not a drop-in widget overlay for this `winit`/`wgpu`
app.

Build probes:

```sh
cargo run -p poincare-mobile --bin poincare-mobile-desktop
cargo check -p poincare-mobile --target aarch64-apple-ios-sim
cargo check -p poincare-mobile --target aarch64-linux-android
```
