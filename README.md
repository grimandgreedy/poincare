## Poincare

An interactive 3D mathematical graphing application, and a reusable 3D plotting library.

![Poincare demo](https://raw.githubusercontent.com/grimandgreedy/poincare/refs/heads/master/assets/demo1.png)


Install and run:

```
$ cargo install poincare-app
$ poincare
```

Plot surfaces, curves, point clouds, vector fields, streamlines, volumes, and isosurfaces. Type an expression (Cartesian, parametric, spherical, cylindrical, or implicit) and the surface appears immediately. Adjust domain, resolution, style, and shading without restarting.

Analysis tools include surface normals, curvature, moving frames, arc-length, torsion, PCA, and CSV data import.

[`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) provides Poincare's interactive 3D viewport and rendering.

**Hot tip**: Poincare turns equations into sampled geometry: surfaces become triangle meshes, and curves become polylines. That means visual quality and many analysis results are tied to plot resolution.

- If a surface looks jagged, faceted, or unstable, increase its resolution in the plot properties panel.
- If points from a surface/surface intersection are too sparse, increase the resolutions of the input surfaces.
- If a curve looks segmented, increase the curve resolution.
- If analysis output looks too coarse, check the source plot resolution before treating the result as exact geometry.

---

## poincare-lib

[`poincare-lib`](https://crates.io/crates/poincare-lib) is the plotting engine extracted from the application. It is independent of the app UI and can be embedded in any wgpu project to add interactive 3D graphing with a few lines of code.

It uses [`viewport-lib`](https://github.com/grimandgreedy/viewport-lib) for the interactive viewport, camera, and GPU rendering layer. Both crates need to be in your dependency tree.

See [`poincare-dvd`](crates/poincare-dvd) for a minimal standalone demo (a bouncing viewport window rendering two live 3D plots) showing what embedding poincare-lib looks like without the full application.