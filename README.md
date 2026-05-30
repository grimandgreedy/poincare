# Poincaré

An interactive 3D mathematical graphing application, and a reusable 3D plotting library.

![Poincare demo](https://raw.githubusercontent.com/grimandgreedy/poincare/refs/heads/master/assets/demo1.png)

## Poincare

Install and run:

```
cargo install poincare-app
poincare
```

Plot surfaces, curves, point clouds, vector fields, streamlines, volumes, and isosurfaces. Type an expression (Cartesian, parametric, spherical, cylindrical, or implicit) and the surface appears immediately. Adjust domain, resolution, style, and shading without restarting.

Analysis tools include surface normals, curvature, mesh quality, moving frames (Frenet / Bishop / Darboux), arc-length, torsion, PCA, and CSV data import.

Rendering is GPU-accelerated via [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) and wgpu, with matcap shading, colourmap, transparency, ground plane, shadow, and PNG / video export.

---

## poincare-lib

[`poincare-lib`](https://crates.io/crates/poincare-lib) is the plotting engine extracted from the application. It is independent of the app UI and can be embedded in any wgpu project to add interactive 3D graphing with a few lines of code.

It has a meaningful dependency on [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib), which provides the GPU renderer and `Camera` type. Both crates need to be in your dependency tree.

```toml
[dependencies]
poincare-lib = "0.5"
viewport-lib = "0.15"
```

```rust
let spec = GraphSpec {
    plots: vec![PlotSpec {
        definition: PlotDefinition::ExprCartesian {
            expression: "sin(x * y)".into(),
            ..
        },
        ..
    }],
    ..
};

let mut scene = spec.build_scene()?;
scene.upload_meshes(&device, &queue, renderer.resources_mut())?;

// each frame:
let frame = scene.build_frame(&camera);
```

See [`poincare-dvd`](crates/poincare-dvd) for a minimal standalone demo (a bouncing viewport window rendering two live 3D plots) showing what embedding poincare-lib looks like without the full application.

---

## Crates in this workspace

| Crate | Description |
|---|---|
| [`poincare-app`](crates/poincare-app) | Desktop application |
| [`poincare-lib`](crates/poincare-lib) | Embeddable 3D plotting library |
| [`poincare-dvd`](crates/poincare-dvd) | Minimal embedding demo |
