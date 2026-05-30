# poincare-lib

A GPU-accelerated 3D graphing library for embedding interactive mathematical plots in any [`wgpu`](https://github.com/gfx-rs/wgpu) / [`egui`](https://github.com/emilk/egui) application. Built on [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib).

## Plot types

- **Surfaces**: Cartesian `z = f(x, y)`, parametric, spherical, cylindrical, implicit
- **Curves**: 3D parametric curves and piecewise polylines
- **Scatter / point clouds**: per-point colour and size
- **Vector fields**: arrow glyphs and LIC (line integral convolution)
- **Streamlines**: seeded integration through a vector field
- **Volumes and isosurfaces**: scalar field rendering
- **Contours and density plots**

Styles include solid colour, colourmap, matcap shading, opacity, two-sided lighting, and parameter-space visualisation. Expressions are parsed at runtime with no recompilation needed.

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
poincare-lib = "0.5"
viewport-lib = "0.15"   # for ViewportRenderer and Camera
```

### Minimal example

```rust
use poincare_lib::{
    AxisConfig, ColourMode, Domain, GraphSpec, PlotDefinition, PlotSpec,
    PlotStyle, Resolution,
};
use viewport_lib::{Camera, ViewportRenderer};

// 1. Declare the scene. No GPU work happens here.
let spec = GraphSpec {
    axis_config: AxisConfig { show_box: true, show_labels: true, ..Default::default() },
    plots: vec![PlotSpec {
        name: "z = sin(x*y)".into(),
        visible: true,
        domain: Domain { x: -4.0..=4.0, y: -4.0..=4.0, z: -4.0..=4.0 },
        resolution: Resolution { u: 80, v: 80 },
        style: PlotStyle {
            colour_mode: ColourMode::Solid([0.2, 0.65, 1.0, 1.0]),
            two_sided: true,
            ..Default::default()
        },
        definition: PlotDefinition::ExprCartesian {
            expression: "sin(x * y)".into(),
            parameters: vec![],
        },
    }],
};

// 2. Tessellate expressions into CPU-side triangle meshes.
let mut scene = spec.build_scene().expect("build failed");

// 3. Upload meshes to the GPU once at startup (needs wgpu device + ViewportRenderer).
//    GraphScene caches the returned MeshIds for use in build_frame().
scene.upload_meshes(&device, &queue, renderer.resources_mut()).unwrap();

// 4. Each frame, build FrameData and hand it to the renderer.
let mut frame = scene.build_frame(&camera);
frame.camera.viewport_size = [width as f32, height as f32];
frame.camera.pixels_per_point = pixels_per_point;
frame.viewport.background_colour = Some([0.05, 0.05, 0.07, 1.0]);
// ... submit frame to ViewportRenderer via your egui_wgpu callback
```

See [`poincare-dvd`](https://github.com/grimandgreedy/poincare/tree/master/crates/poincare-dvd) for a minimal self-contained application that uses this API to render a bouncing 3D graph window, showing that poincare-lib works independently of the main Poincaré application.

## Relationship to other crates

| Crate | Role |
|---|---|
| `poincare-lib` | Math, tessellation, scene construction (this crate) |
| [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) | GPU renderer, wgpu pipeline, Camera |
| [`poincare-app`](https://github.com/grimandgreedy/poincare) | Full desktop application built on top of this crate |
