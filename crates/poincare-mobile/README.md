# poincare-mobile

Experimental mobile shell for Poincare.

M1 renders a mobile document full-screen using `winit`, `wgpu`,
`viewport-lib`, and `poincare-lib`. M2 starts with equation entry: the Add Plot
sheet creates `poincare-lib::PlotSpec` values and rebuilds/uploads the scene.

Controls:

- 1-finger drag: orbit
- 2-finger drag: pan
- 2-finger pinch: zoom
- 3-finger touch: open the Add Plot sheet
- Desktop helper: `+` opens the Add Plot sheet

## UI structure

The mobile shell keeps viewport/rendering state separate from menu commands.
`src/model.rs` defines the small command/snapshot/model surface used by the UI:

- `UiSnapshot`: read-only state needed by menus and sheets
- `UiCommand`: actions such as opening the drawer, editing equation text, or
  submitting the equation editor
- `MobileModel`: renderer-independent state transition layer

The current implementation uses `egui-winit`/`egui-wgpu` as an overlay in the
same `winit` window. `src/mobile_ui.rs` is the start of a small mobile component
layer for that path: touch-sized icon buttons, compact action buttons, a drawer,
bottom sheet, and shared mobile styling. It emits `UiCommand` values instead of
mutating renderer state directly.

Build probes:

```sh
cargo run -p poincare-mobile --bin poincare-mobile-desktop
cargo check -p poincare-mobile --target aarch64-apple-ios-sim
cargo check -p poincare-mobile --target aarch64-linux-android
```
