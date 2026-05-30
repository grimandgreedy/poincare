# Poincaré

An interactive 3D mathematical graphing application.

![Poincare demo](https://raw.githubusercontent.com/grimandgreedy/poincare/refs/heads/master/assets/demo1.png)

## Install

```
cargo install poincare-app
```

## What it does

Poincaré lets you plot and explore 3D mathematical objects interactively. Type an expression and see the surface immediately; adjust domain, resolution, and style without restarting.

**Plot types**

- Surfaces: Cartesian `z = f(x, y)`, parametric, spherical, cylindrical, implicit
- Curves: 3D parametric and piecewise
- Scatter / point clouds
- Vector fields with arrow glyphs and LIC shading
- Streamlines
- Volumes and isosurfaces
- Contour and density plots

**Analysis tools**

- Surface normals, curvature, mesh quality, surface area
- Moving frames (Frenet, Bishop, Darboux, surface-aligned)
- Curve arc-length, curvature, torsion, tangent field
- PCA, centroid, point cloud statistics
- Data import from CSV

**Rendering**

Built on [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) and wgpu. Supports matcap shading, colourmap, transparency, two-sided lighting, ground plane, shadow, and parameter-space visualisation. Exports PNG and video.

## Crates

| Crate | Purpose |
|---|---|
| `poincare-app` | The desktop application (this crate) |
| [`poincare-lib`](https://crates.io/crates/poincare-lib) | Reusable plotting library, embeddable in other apps |
| [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) | GPU renderer |
