use glam::Vec3;

use viewport_lib::{
    AppearanceSettings, AttributeKind, AttributeRef, Camera, ColourmapId, FrameData, GlyphItem,
    LabelItem, LightKind, LightSource, LightingSettings, Material, MeshId, PointCloudItem,
    PolylineItem, RenderCamera, SceneRenderItem, StreamtubeItem, SurfaceLICConfig, SurfaceLICItem,
    SurfaceSubmission, ViewportError, ViewportGpuResources, VolumeId, VolumeItem,
};

use crate::axis::Axis3;
use crate::axis::AxisConfig;
use crate::domain::Domain;
use crate::plot_object::{GlyphInstance, PlotComponent, PlotGeometry, PlotObject};
use crate::style::{
    ColormapSource, ColourMode, MatcapSource, PlotStyle, ShadingMode, SurfaceLicSettings,
};
use crate::{axis, ticks};

struct CachedSurface {
    mesh_index: MeshId,
    style: PlotStyle,
    colourmap_id: Option<ColourmapId>,
    matcap_id: Option<viewport_lib::MatcapId>,
    lic_vector_attributes: Vec<String>,
    /// CPU-side copies kept for probe picking (ray-triangle intersection).
    cpu_positions: Vec<[f32; 3]>,
    cpu_indices: Vec<u32>,
}

struct CachedPolyline {
    positions: Vec<Vec3>,
    strip_lengths: Vec<u32>,
    scalars: Option<Vec<f32>>,
    style: PlotStyle,
    colourmap_id: Option<ColourmapId>,
}

struct CachedPointCloud {
    positions: Vec<Vec3>,
    scalars: Option<Vec<f32>>,
    style: PlotStyle,
    colourmap_id: Option<ColourmapId>,
}

struct CachedGlyphs {
    instances: Vec<GlyphInstance>,
    style: PlotStyle,
    colourmap_id: Option<ColourmapId>,
}

struct CachedStreamtube {
    positions: Vec<glam::Vec3>,
    strip_lengths: Vec<u32>,
    radius: f32,
    style: PlotStyle,
}

struct CachedVolume {
    volume_id: VolumeId,
    dims: [u32; 3],
    origin: [f32; 3],
    spacing: [f32; 3],
    style: PlotStyle,
    scalar_range: (f32, f32),
}

#[derive(Default)]
struct CachedPlot {
    surfaces: Vec<CachedSurface>,
    polylines: Vec<CachedPolyline>,
    point_clouds: Vec<CachedPointCloud>,
    glyphs: Vec<CachedGlyphs>,
    streamtubes: Vec<CachedStreamtube>,
    volumes: Vec<CachedVolume>,
}

/// A collection of plot objects rendered together in one scene.
///
/// # Usage
///
/// 1. Add plots with [`add`].
/// 2. Upload GPU meshes once at startup (or when plots change) with [`upload_meshes`].
/// 3. Each frame, call [`build_frame`] with the current camera to get `FrameData`.
/// 4. Submit `FrameData` to [`viewport_lib::ViewportRenderer::prepare`] / `paint`.
pub struct GraphScene {
    /// Plotting domain applied to all objects in the scene.
    pub domain: Domain,
    /// Configuration for the labelled axis box rendered around the scene.
    pub axis_config: AxisConfig,
    plots: Vec<Box<dyn PlotObject>>,
    /// Cached renderable geometry per root plot.
    cached_plots: Vec<CachedPlot>,
}

impl GraphScene {
    pub fn new() -> Self {
        Self {
            domain: Domain::default(),
            axis_config: AxisConfig::default(),
            plots: Vec::new(),
            cached_plots: Vec::new(),
        }
    }

    pub fn with_domain(domain: Domain) -> Self {
        Self {
            domain,
            axis_config: AxisConfig::default(),
            plots: Vec::new(),
            cached_plots: Vec::new(),
        }
    }

    /// Add a plot object to the scene. Call `upload_meshes` after adding plots.
    pub fn add(&mut self, plot: impl PlotObject + 'static) {
        self.plots.push(Box::new(plot));
        self.cached_plots.push(CachedPlot::default());
    }

    /// Generate CPU-side geometry for all plots and upload surface meshes to the GPU.
    ///
    /// Call this once at startup (or whenever plots are added or changed).
    /// The returned mesh indices are cached internally and used by `build_frame`.
    pub fn upload_meshes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &mut ViewportGpuResources,
    ) -> Result<(), ViewportError> {
        resources.ensure_colourmaps_initialized(device, queue);
        resources.ensure_matcaps_initialized(device, queue);

        for (i, plot) in self.plots.iter().enumerate() {
            self.cached_plots[i] = CachedPlot::default();
            let domain = plot.domain_override().unwrap_or(&self.domain);
            let geometry = plot.generate(domain, plot.resolution());
            cache_geometry(
                &mut self.cached_plots[i],
                geometry,
                plot.style().clone(),
                device,
                queue,
                resources,
            )?;
        }
        Ok(())
    }

    /// Build the `FrameData` for rendering this scene with the given camera.
    ///
    /// Uses cached mesh indices from a previous `upload_meshes` call.
    /// The caller should set `frame.viewport_size` after this call, since the
    /// scene does not know the physical pixel dimensions.
    pub fn build_frame(&self, camera: &Camera) -> FrameData {
        let view = camera.view_matrix();
        let proj = camera.proj_matrix();

        let scene_items: Vec<SceneRenderItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.surfaces.iter().map(|surface| {
                    let mut item = SceneRenderItem::default();
                    item.mesh_id = surface.mesh_index;
                    item.material = material_from_style(&surface.style, surface.matcap_id);
                    item.appearance = appearance_from_style(&surface.style);
                    if surface.style.two_sided {
                        item.material.backface_policy = viewport_lib::BackfacePolicy::Identical;
                    }
                    apply_surface_colour_mode(&mut item, &surface.style, surface.colourmap_id);
                    item
                })
            })
            .collect();

        let lic_items: Vec<SurfaceLICItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.surfaces.iter().filter_map(|surface| {
                    let lic = surface.style.surface_lic.as_ref()?;
                    let attribute = lic.vector_field.attribute_name();
                    if !surface
                        .lic_vector_attributes
                        .iter()
                        .any(|name| name == attribute)
                    {
                        return None;
                    }
                    Some(SurfaceLICItem::new(
                        surface.mesh_index,
                        attribute,
                        glam::Mat4::IDENTITY.to_cols_array_2d(),
                        surface_lic_config(lic),
                    ))
                })
            })
            .collect();

        let mut polylines: Vec<PolylineItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.polylines.iter().filter_map(|polyline| {
                    if polyline.positions.is_empty() || polyline.strip_lengths.is_empty() {
                        return None;
                    }
                    let mut item = PolylineItem::default();
                    item.positions = polyline.positions.iter().map(|p| p.to_array()).collect();
                    item.strip_lengths = polyline.strip_lengths.clone();
                    item.default_colour = default_colour_rgba(&polyline.style);
                    item.line_width = polyline.style.line_width;
                    item.appearance = appearance_from_style(&polyline.style);
                    apply_polyline_colour_mode(
                        &mut item,
                        &polyline.style,
                        &polyline.positions,
                        polyline.scalars.as_deref(),
                        polyline.colourmap_id,
                    );
                    Some(item)
                })
            })
            .collect();

        let point_clouds: Vec<PointCloudItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.point_clouds.iter().filter_map(|points| {
                    if points.positions.is_empty() {
                        return None;
                    }
                    let mut item = PointCloudItem::default();
                    item.positions = points.positions.iter().map(|p| p.to_array()).collect();
                    item.point_size = points.style.point_size;
                    item.default_colour = default_colour_rgba(&points.style);
                    item.appearance = appearance_from_style(&points.style);
                    apply_point_colour_mode(
                        &mut item,
                        &points.style,
                        &points.positions,
                        points.scalars.as_deref(),
                        points.colourmap_id,
                    );
                    Some(item)
                })
            })
            .collect();

        let glyphs: Vec<GlyphItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.glyphs.iter().filter_map(|glyphs| {
                    if glyphs.instances.is_empty() {
                        return None;
                    }
                    let mut item = GlyphItem::default();
                    item.positions = glyphs
                        .instances
                        .iter()
                        .map(|g| g.position.to_array())
                        .collect();
                    item.vectors = glyphs
                        .instances
                        .iter()
                        .map(|g| g.vector.to_array())
                        .collect();
                    item.scale = 1.0;
                    item.scale_by_magnitude = false;
                    item.glyph_type = glyphs.style.glyph_type;
                    item.default_colour = default_colour_rgba(&glyphs.style);
                    item.use_default_colour =
                        matches!(glyphs.style.colour_mode, ColourMode::Solid(_));
                    item.appearance = appearance_from_style(&glyphs.style);
                    item.appearance.unlit = true;
                    apply_glyph_colour_mode(
                        &mut item,
                        &glyphs.style,
                        &glyphs.instances,
                        glyphs.colourmap_id,
                    );
                    Some(item)
                })
            })
            .collect();

        // Assemble streamtube items.
        let streamtube_items: Vec<StreamtubeItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.streamtubes.iter().filter_map(|st| {
                    if st.positions.is_empty() || st.strip_lengths.is_empty() {
                        return None;
                    }
                    let mut item = StreamtubeItem::default();
                    item.positions = st.positions.iter().map(|p| p.to_array()).collect();
                    item.strip_lengths = st.strip_lengths.clone();
                    item.radius = st.radius;
                    item.colour = default_colour_rgba(&st.style);
                    item.appearance = appearance_from_style(&st.style);
                    Some(item)
                })
            })
            .collect();

        // Assemble volume items.
        let volumes: Vec<VolumeItem> = self
            .cached_plots
            .iter()
            .flat_map(|plot| {
                plot.volumes.iter().map(|vol| {
                    let tf = vol.style.transfer_function.as_ref();
                    let opacity_scale = tf.map_or(0.5, |t| t.opacity_scale);
                    let (threshold_min, threshold_max) =
                        tf.and_then(|t| t.threshold).unwrap_or(vol.scalar_range);

                    let bbox_max = [
                        vol.origin[0] + vol.spacing[0] * (vol.dims[0].saturating_sub(1)) as f32,
                        vol.origin[1] + vol.spacing[1] * (vol.dims[1].saturating_sub(1)) as f32,
                        vol.origin[2] + vol.spacing[2] * (vol.dims[2].saturating_sub(1)) as f32,
                    ];

                    let mut item = VolumeItem::default();
                    item.volume_id = vol.volume_id;
                    item.bbox_min = vol.origin;
                    item.bbox_max = bbox_max;
                    item.scalar_range = vol.scalar_range;
                    item.opacity_scale = opacity_scale;
                    item.threshold_min = threshold_min;
                    item.threshold_max = threshold_max;
                    item.enable_shading = true;
                    item.appearance = appearance_from_style(&vol.style);
                    item
                })
            })
            .collect();

        // Compute per-axis display extents so each axis line spans the full viewport.
        // Uses a ray-slab intersection in clip space so the range is correct even when
        // the world origin is panned off-screen.
        let vp = proj * view;
        let fb = camera.distance;
        let (x_lo, x_hi) = vp_axis_visible_range(&vp, glam::Vec3::X).unwrap_or((-fb, fb));
        let (y_lo, y_hi) = vp_axis_visible_range(&vp, glam::Vec3::Y).unwrap_or((-fb, fb));
        let (z_lo, z_hi) = vp_axis_visible_range(&vp, glam::Vec3::Z).unwrap_or((-fb, fb));
        let axis_domain = Domain {
            x: (x_lo as f64)..=(x_hi as f64),
            y: (y_lo as f64)..=(y_hi as f64),
            z: (z_lo as f64)..=(z_hi as f64),
        };

        // Build axis box + tick polylines and annotation labels.
        let tick_x = screen_spaced_axis_ticks(
            &vp,
            Axis3::X,
            x_lo as f64,
            x_hi as f64,
            self.axis_config.tick_count[0],
        );
        let tick_y = screen_spaced_axis_ticks(
            &vp,
            Axis3::Y,
            y_lo as f64,
            y_hi as f64,
            self.axis_config.tick_count[1],
        );
        let tick_z = screen_spaced_axis_ticks(
            &vp,
            Axis3::Z,
            z_lo as f64,
            z_hi as f64,
            self.axis_config.tick_count[2],
        );
        let ticks_per_axis = [tick_x, tick_y, tick_z];

        let lic_active = !lic_items.is_empty();
        if !lic_active {
            let mut axis_polylines = axis::build_axis_polyline_projected(
                &axis_domain,
                &self.axis_config,
                &ticks_per_axis,
                Some(&vp),
            );
            polylines.append(&mut axis_polylines);
        } else {
            // viewport-lib's HDR LIC path currently does not support the polyline pipeline.
            // Drop polyline submissions while LIC is active so the surface pass can render
            // instead of crashing on an HDR/LDR pipeline-format mismatch.
            polylines.clear();
        }

        let axis_labels: Vec<LabelItem> = axis::build_axis_labels_projected(
            &axis_domain,
            &self.axis_config,
            &ticks_per_axis,
            Some(&vp),
        );

        let mut frame = FrameData::default();
        frame.camera.render_camera = RenderCamera::from_camera(camera);
        frame.effects.lighting = hemisphere_lighting();
        if !lic_items.is_empty() {
            frame.effects.post_process.enabled = true;
        }
        frame.scene.surfaces = SurfaceSubmission::Flat(scene_items.into());
        frame.scene.lic_items = lic_items;
        frame.scene.polylines = polylines;
        frame.scene.point_clouds = point_clouds;
        frame.scene.glyphs = glyphs;
        frame.scene.streamtube_items = streamtube_items;
        frame.scene.volumes = volumes;
        frame.viewport.show_axes_indicator = true;
        frame.overlays.labels = axis_labels;

        frame
    }
}

impl Default for GraphScene {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphScene {
    pub fn release_gpu_resources(&self, resources: &mut ViewportGpuResources) {
        for cached_plot in &self.cached_plots {
            for surface in &cached_plot.surfaces {
                let _ = resources.remove_mesh(surface.mesh_index);
            }
        }
    }

    /// Upload a custom 256-sample RGBA colourmap through the viewport resource manager.
    pub fn upload_colourmap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &mut ViewportGpuResources,
        rgba_data: &[[u8; 4]; 256],
    ) -> ColourmapId {
        resources.upload_colourmap(device, queue, rgba_data)
    }
}

fn cache_geometry(
    cache: &mut CachedPlot,
    geometry: PlotGeometry,
    style: PlotStyle,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &mut ViewportGpuResources,
) -> Result<(), ViewportError> {
    let colourmap_id = resolve_colourmap_id(&style, resources);
    let matcap_id = resolve_matcap_id(&style, resources);
    match geometry {
        PlotGeometry::Surface(mesh_data) => {
            let cpu_positions = mesh_data.positions.clone();
            let cpu_indices = mesh_data.indices.clone();
            let lic_vector_attributes: Vec<String> = mesh_data
                .attributes
                .iter()
                .filter_map(|(name, value)| {
                    matches!(value, viewport_lib::AttributeData::VertexVector(_))
                        .then_some(name.clone())
                })
                .collect();
            let mesh_index = resources.upload_mesh_data(device, &mesh_data)?;
            cache.surfaces.push(CachedSurface {
                mesh_index,
                style,
                colourmap_id,
                matcap_id,
                lic_vector_attributes,
                cpu_positions,
                cpu_indices,
            });
        }
        PlotGeometry::Polyline {
            positions,
            strip_lengths,
            scalars,
        } => {
            cache.polylines.push(CachedPolyline {
                positions,
                strip_lengths,
                scalars,
                style,
                colourmap_id,
            });
        }
        PlotGeometry::Points { positions, scalars } => {
            cache.point_clouds.push(CachedPointCloud {
                positions,
                scalars,
                style,
                colourmap_id,
            });
        }
        PlotGeometry::Glyphs(instances) => {
            cache.glyphs.push(CachedGlyphs {
                instances,
                style,
                colourmap_id,
            });
        }
        PlotGeometry::Streamtube {
            positions,
            strip_lengths,
            radius,
        } => {
            cache.streamtubes.push(CachedStreamtube {
                positions,
                strip_lengths,
                radius,
                style,
            });
        }
        PlotGeometry::Volume {
            data,
            dims,
            origin,
            spacing,
        } => {
            // Compute scalar range from voxel data for normalization.
            let (min_val, max_val) = data
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |acc, &v| {
                    (acc.0.min(v), acc.1.max(v))
                });
            let scalar_range = (min_val, max_val);

            let volume_id = resources.upload_volume(device, queue, &data, dims);
            cache.volumes.push(CachedVolume {
                volume_id,
                dims,
                origin,
                spacing,
                style,
                scalar_range,
            });
        }
        PlotGeometry::Composite(components) => {
            for PlotComponent { geometry, style } in components {
                cache_geometry(cache, geometry, style, device, queue, resources)?;
            }
        }
    }
    Ok(())
}

fn resolve_colourmap_id(
    style: &PlotStyle,
    resources: &ViewportGpuResources,
) -> Option<ColourmapId> {
    match &style.colour_mode {
        ColourMode::Solid(_) => None,
        ColourMode::Colormap { colormap, .. } => match colormap {
            ColormapSource::Builtin(preset) => Some(resources.builtin_colourmap_id(*preset)),
            ColormapSource::Uploaded(id) => Some(*id),
        },
        ColourMode::ByAttribute { .. } => {
            Some(resources.builtin_colourmap_id(viewport_lib::BuiltinColourmap::Viridis))
        }
    }
}

fn resolve_matcap_id(
    style: &PlotStyle,
    resources: &ViewportGpuResources,
) -> Option<viewport_lib::MatcapId> {
    match style.matcap {
        Some(MatcapSource::Builtin(preset)) => Some(resources.builtin_matcap_id(preset)),
        None => None,
    }
}

fn material_from_style(style: &PlotStyle, matcap_id: Option<viewport_lib::MatcapId>) -> Material {
    let rgba = default_colour_rgba(style);
    let mut material = Material::from_colour([rgba[0], rgba[1], rgba[2]]);
    material.matcap_id = matcap_id;
    material.param_vis = style.param_vis.map(Into::into);

    match style.shading {
        ShadingMode::Flat => {
            material.specular = 0.0;
            material.shininess = 1.0;
        }
        ShadingMode::Smooth => {}
        ShadingMode::Unlit => {
            material.ambient = 1.0;
            material.diffuse = 0.0;
            material.specular = 0.0;
            material.shininess = 1.0;
        }
    }

    material
}

fn appearance_from_style(style: &PlotStyle) -> AppearanceSettings {
    let mut appearance = AppearanceSettings::default();
    appearance.opacity = style.opacity.clamp(0.0, 1.0);
    appearance.unlit = matches!(style.shading, ShadingMode::Unlit);
    appearance
}

fn surface_lic_config(settings: &SurfaceLicSettings) -> SurfaceLICConfig {
    let mut config = SurfaceLICConfig::default();
    config.steps = settings.steps;
    config.step_size = settings.step_size;
    config.strength = settings.strength;
    config
}

fn default_colour_rgba(style: &PlotStyle) -> [f32; 4] {
    match style.colour_mode {
        ColourMode::Solid(rgba) => [rgba[0], rgba[1], rgba[2], rgba[3].clamp(0.0, 1.0)],
        ColourMode::Colormap { .. } | ColourMode::ByAttribute { .. } => [1.0, 1.0, 1.0, 1.0],
    }
}

fn apply_surface_colour_mode(
    item: &mut SceneRenderItem,
    style: &PlotStyle,
    resolved_colourmap_id: Option<ColourmapId>,
) {
    match &style.colour_mode {
        ColourMode::Solid(_) => {}
        ColourMode::Colormap { scalar_range, .. } => {
            item.active_attribute = Some(surface_attribute_ref(style, "value".to_string()));
            item.scalar_range = *scalar_range;
            item.colourmap_id = resolved_colourmap_id;
        }
        ColourMode::ByAttribute { name, kind } => {
            item.active_attribute = Some(AttributeRef {
                name: name.clone(),
                kind: *kind,
            });
            item.colourmap_id = resolved_colourmap_id;
        }
    }
}

fn apply_polyline_colour_mode(
    item: &mut PolylineItem,
    style: &PlotStyle,
    points: &[Vec3],
    explicit_scalars: Option<&[f32]>,
    resolved_colourmap_id: Option<ColourmapId>,
) {
    match &style.colour_mode {
        ColourMode::Solid(_) => {}
        ColourMode::Colormap { scalar_range, .. } => {
            item.scalars = explicit_scalars
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| default_scalars_for_positions(points));
            item.scalar_range = *scalar_range;
            item.colourmap_id = resolved_colourmap_id;
        }
        ColourMode::ByAttribute { name, .. } => {
            if let Some(scalars) = derive_position_scalars(name, points, explicit_scalars) {
                item.scalars = scalars;
                item.colourmap_id = None;
            }
        }
    }
}

fn apply_point_colour_mode(
    item: &mut PointCloudItem,
    style: &PlotStyle,
    points: &[Vec3],
    explicit_scalars: Option<&[f32]>,
    resolved_colourmap_id: Option<ColourmapId>,
) {
    match &style.colour_mode {
        ColourMode::Solid(_) => {}
        ColourMode::Colormap { scalar_range, .. } => {
            item.scalars = explicit_scalars
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| default_scalars_for_positions(points));
            item.scalar_range = *scalar_range;
            item.colourmap_id = resolved_colourmap_id;
        }
        ColourMode::ByAttribute { name, .. } => {
            if let Some(scalars) = derive_position_scalars(name, points, explicit_scalars) {
                item.scalars = scalars;
                item.colourmap_id = None;
            }
        }
    }
}

fn apply_glyph_colour_mode(
    item: &mut GlyphItem,
    style: &PlotStyle,
    instances: &[GlyphInstance],
    resolved_colourmap_id: Option<ColourmapId>,
) {
    match &style.colour_mode {
        ColourMode::Solid(_) => {}
        ColourMode::Colormap { scalar_range, .. } => {
            item.scalars = instances.iter().map(|g| g.raw_vector.length()).collect();
            item.scalar_range = *scalar_range;
            item.colourmap_id = resolved_colourmap_id;
        }
        ColourMode::ByAttribute { name, .. } => {
            if let Some(scalars) = derive_glyph_scalars(name, instances) {
                item.scalars = scalars;
                item.colourmap_id = None;
            }
        }
    }
}

fn surface_attribute_ref(style: &PlotStyle, fallback_name: String) -> AttributeRef {
    match style.face_quantity {
        Some(quantity) => AttributeRef {
            name: quantity.attribute_name().to_string(),
            kind: AttributeKind::Face,
        },
        None => AttributeRef {
            name: fallback_name,
            kind: AttributeKind::Vertex,
        },
    }
}

fn default_scalars_for_positions(points: &[Vec3]) -> Vec<f32> {
    points.iter().map(|p| p.z).collect()
}

// ---------------------------------------------------------------------------
// Probe picking data — CPU-side geometry exposed for coordinate probing.
// ---------------------------------------------------------------------------

/// CPU-side mesh geometry for one surface, usable for ray-triangle picking.
pub struct SurfacePickData<'a> {
    pub positions: &'a [[f32; 3]],
    pub indices: &'a [u32],
}

/// CPU-side positions for one polyline (may consist of multiple strips).
pub struct PolylinePickData<'a> {
    pub positions: &'a [Vec3],
    pub strip_lengths: &'a [u32],
}

/// CPU-side positions for one point cloud.
pub struct PointPickData<'a> {
    pub positions: &'a [Vec3],
}

/// All pickable CPU-side geometry from a [`GraphScene`].
pub struct ProbePickData<'a> {
    pub surfaces: Vec<SurfacePickData<'a>>,
    pub polylines: Vec<PolylinePickData<'a>>,
    pub points: Vec<PointPickData<'a>>,
}

impl GraphScene {
    /// Returns CPU-side geometry for all cached plots, suitable for probe picking.
    pub fn probe_data(&self) -> ProbePickData<'_> {
        let mut surfaces = Vec::new();
        let mut polylines = Vec::new();
        let mut points = Vec::new();

        for plot in &self.cached_plots {
            for s in &plot.surfaces {
                if !s.cpu_positions.is_empty() && !s.cpu_indices.is_empty() {
                    surfaces.push(SurfacePickData {
                        positions: &s.cpu_positions,
                        indices: &s.cpu_indices,
                    });
                }
            }
            for p in &plot.polylines {
                if !p.positions.is_empty() {
                    polylines.push(PolylinePickData {
                        positions: &p.positions,
                        strip_lengths: &p.strip_lengths,
                    });
                }
            }
            for pc in &plot.point_clouds {
                if !pc.positions.is_empty() {
                    points.push(PointPickData {
                        positions: &pc.positions,
                    });
                }
            }
        }

        ProbePickData {
            surfaces,
            polylines,
            points,
        }
    }
}

fn derive_position_scalars(
    attribute: &str,
    points: &[Vec3],
    explicit_scalars: Option<&[f32]>,
) -> Option<Vec<f32>> {
    match attribute {
        "x" => Some(points.iter().map(|p| p.x).collect()),
        "y" => Some(points.iter().map(|p| p.y).collect()),
        "z" => Some(points.iter().map(|p| p.z).collect()),
        "radius" => Some(points.iter().map(|p| p.length()).collect()),
        "index" => Some((0..points.len()).map(|i| i as f32).collect()),
        "scalar" | "value" => explicit_scalars.map(ToOwned::to_owned),
        "magnitude" => explicit_scalars.map(|values| values.iter().map(|v| v.abs()).collect()),
        _ => None,
    }
}

fn derive_glyph_scalars(attribute: &str, instances: &[GlyphInstance]) -> Option<Vec<f32>> {
    match attribute {
        "x" => Some(instances.iter().map(|g| g.position.x).collect()),
        "y" => Some(instances.iter().map(|g| g.position.y).collect()),
        "z" => Some(instances.iter().map(|g| g.position.z).collect()),
        "radius" => Some(instances.iter().map(|g| g.position.length()).collect()),
        "magnitude" | "value" => Some(instances.iter().map(|g| g.raw_vector.length()).collect()),
        "vx" => Some(instances.iter().map(|g| g.raw_vector.x).collect()),
        "vy" => Some(instances.iter().map(|g| g.raw_vector.y).collect()),
        "vz" => Some(instances.iter().map(|g| g.raw_vector.z).collect()),
        "index" => Some((0..instances.len()).map(|i| i as f32).collect()),
        _ => None,
    }
}

/// Build a `LightingSettings` using hemisphere ambient lighting with a single key light.
///
/// The hemisphere provides smooth, orientation-aware ambient fill: surfaces facing up
/// receive sky color, surfaces facing down receive ground color. The key light adds
/// shape definition and specular highlights. Shadows are disabled — they add cost
/// without visual benefit for mathematical plot geometry.
fn hemisphere_lighting() -> LightingSettings {
    LightingSettings {
        lights: vec![LightSource {
            kind: LightKind::Directional {
                // ~65° elevation, slight front-right bias (Z-up convention).
                direction: [0.4, 0.3, 1.5],
            },
            colour: [1.0, 1.0, 1.0],
            intensity: 0.75,
        }],
        shadows_enabled: false,
        sky_colour: [0.8, 0.9, 1.0],
        ground_colour: [0.5, 0.55, 0.6],
        hemisphere_intensity: 0.65,
        ..LightingSettings::default()
    }
}

/// Find the visible segment `[t_lo, t_hi]` of the axis line `P(t) = t * dir` (through
/// the world origin) by intersecting it with the view frustum in clip space.
///
/// For each point `P(t)`, clip coordinates are `clip(t) = t*D + O` where
/// `D = VP * [dir; 0]` and `O = VP * [0,0,0; 1]`.  Visibility requires:
///   |clip.x| ≤ clip.w,  |clip.y| ≤ clip.w,  0 ≤ clip.z ≤ clip.w  (wgpu NDC)
/// Each of these is a linear inequality `a*t ≤ b`, solved as a standard slab test.
///
/// Returns `None` when the axis line is entirely outside the frustum.
fn vp_axis_visible_range(vp: &glam::Mat4, dir: glam::Vec3) -> Option<(f32, f32)> {
    let d = *vp * dir.extend(0.0);
    let o = *vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);

    let mut t_lo = f32::NEG_INFINITY;
    let mut t_hi = f32::INFINITY;

    // Each entry is (a, b) representing the constraint `a*t ≤ b`.
    let constraints: [(f32, f32); 7] = [
        (d.x - d.w, o.w - o.x),    // ndc_x ≤ +1
        (-(d.x + d.w), o.w + o.x), // ndc_x ≥ -1
        (d.y - d.w, o.w - o.y),    // ndc_y ≤ +1
        (-(d.y + d.w), o.w + o.y), // ndc_y ≥ -1
        (-d.w, o.w),               // clip.w ≥ 0  (in front of camera)
        (d.z - d.w, o.w - o.z),    // ndc_z ≤ +1 (far plane, wgpu [0,1] depth)
        (-d.z, o.z),               // ndc_z ≥  0 (near plane)
    ];

    for (a, b) in constraints {
        if a.abs() < 1e-8 {
            if b < -1e-6 {
                return None;
            } // constraint never satisfied
        } else if a > 0.0 {
            t_hi = t_hi.min(b / a);
        } else {
            t_lo = t_lo.max(b / a);
        }
    }

    if t_lo > t_hi + 1e-6 {
        None
    } else {
        Some((t_lo, t_hi))
    }
}

fn screen_spaced_axis_ticks(
    vp: &glam::Mat4,
    axis_kind: Axis3,
    lo: f64,
    hi: f64,
    target_count: u32,
) -> Vec<(f64, String)> {
    if target_count == 0 {
        return Vec::new();
    }
    let fallback = ticks::nice_ticks(lo, hi, target_count);
    let anchor_domain = axis_anchor_domain(axis_kind, lo, hi);
    let anchor_ticks = [(lo, String::new()), (hi, String::new())];
    let Some(p_lo) = project_to_ndc_xy(
        vp,
        axis::tick_label_anchor(&anchor_domain, Some(vp), axis_kind, &anchor_ticks, lo),
    ) else {
        return fallback;
    };
    let Some(p_hi) = project_to_ndc_xy(
        vp,
        axis::tick_label_anchor(&anchor_domain, Some(vp), axis_kind, &anchor_ticks, hi),
    ) else {
        return fallback;
    };
    let axis_vec = p_hi - p_lo;
    if axis_vec.length_squared() < 1e-8 {
        return fallback;
    }

    let count = target_count.max(2) as usize;
    let mut raw_values = Vec::with_capacity(count + 1);
    for i in 0..count {
        let fraction = if count <= 1 {
            0.0
        } else {
            i as f32 / (count - 1) as f32
        };
        raw_values.push(invert_projected_axis_fraction(
            vp, axis_kind, lo as f32, hi as f32, fraction,
        ) as f64);
    }
    if lo <= 0.0 && 0.0 <= hi {
        raw_values.push(0.0);
    }
    raw_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    dedup_close_values(&mut raw_values);

    let estimate = estimated_tick_step(&raw_values)
        .unwrap_or_else(|| ((hi - lo).abs() / target_count.max(1) as f64).max(1e-6));
    let steps = nice_step_candidates(estimate);

    let mut best: Option<(f32, Vec<(f64, String)>)> = None;
    for step in steps {
        let candidate_values = candidate_tick_values(&raw_values, lo, hi, step);
        if candidate_values.len() < 2 {
            continue;
        }
        let Some(selected) =
            select_screen_aligned_ticks(vp, axis_kind, lo, hi, &candidate_values, count)
        else {
            continue;
        };
        let score = tick_selection_score(vp, axis_kind, lo, hi, &selected, count);
        match &mut best {
            Some((best_score, best_ticks)) if score < *best_score => {
                *best_score = score;
                *best_ticks = format_axis_tick_values(&selected);
            }
            None => {
                best = Some((score, format_axis_tick_values(&selected)));
            }
            _ => {}
        }
    }

    best.map(|(_, ticks)| ticks)
        .unwrap_or_else(|| format_axis_tick_values(&raw_values))
}

fn invert_projected_axis_fraction(
    vp: &glam::Mat4,
    axis_kind: Axis3,
    lo: f32,
    hi: f32,
    target_fraction: f32,
) -> f32 {
    let mut a = lo;
    let mut b = hi;
    for _ in 0..32 {
        let mid = 0.5 * (a + b);
        let fraction =
            projected_axis_fraction(vp, axis_kind, lo, hi, mid).unwrap_or(target_fraction);
        if fraction < target_fraction {
            a = mid;
        } else {
            b = mid;
        }
    }
    0.5 * (a + b)
}

fn projected_axis_fraction(
    vp: &glam::Mat4,
    axis_kind: Axis3,
    lo: f32,
    hi: f32,
    t: f32,
) -> Option<f32> {
    let anchor_domain = axis_anchor_domain(axis_kind, lo as f64, hi as f64);
    let anchor_ticks = [(lo as f64, String::new()), (hi as f64, String::new())];
    let p_lo = project_to_ndc_xy(
        vp,
        axis::tick_label_anchor(
            &anchor_domain,
            Some(vp),
            axis_kind,
            &anchor_ticks,
            lo as f64,
        ),
    )?;
    let p_hi = project_to_ndc_xy(
        vp,
        axis::tick_label_anchor(
            &anchor_domain,
            Some(vp),
            axis_kind,
            &anchor_ticks,
            hi as f64,
        ),
    )?;
    let p = project_to_ndc_xy(
        vp,
        axis::tick_label_anchor(&anchor_domain, Some(vp), axis_kind, &anchor_ticks, t as f64),
    )?;
    let axis_vec = p_hi - p_lo;
    let axis_len2 = axis_vec.length_squared();
    if axis_len2 < 1e-8 {
        return None;
    }
    Some(((p - p_lo).dot(axis_vec) / axis_len2).clamp(0.0, 1.0))
}

fn dedup_close_values(values: &mut Vec<f64>) {
    values.dedup_by(|a, b| (*a - *b).abs() < 1e-3 * (1.0 + a.abs().max(b.abs())));
}

fn estimated_tick_step(values: &[f64]) -> Option<f64> {
    let mut deltas: Vec<f64> = values
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .filter(|d| *d > 1e-9)
        .collect();
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(deltas[deltas.len() / 2])
}

fn nice_step_candidates(estimate: f64) -> Vec<f64> {
    let estimate = estimate.max(1e-9);
    let decimal_exp = estimate.log10().floor() as i32;
    let binary_exp = estimate.log2().floor() as i32;
    let mut steps = Vec::new();

    for exp in (decimal_exp - 3)..=(decimal_exp + 3) {
        let base = 10_f64.powi(exp);
        for mult in [1.0, 2.0, 5.0] {
            steps.push(mult * base);
        }
    }
    for exp in (binary_exp - 4)..=(binary_exp + 4) {
        steps.push(2_f64.powi(exp));
    }

    steps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    dedup_close_values(&mut steps);
    steps
}

fn candidate_tick_values(seed_values: &[f64], lo: f64, hi: f64, step: f64) -> Vec<f64> {
    if step <= 0.0 {
        return Vec::new();
    }
    let mut values = Vec::new();
    for &seed in seed_values {
        let snapped = (seed / step).round() * step;
        if snapped >= lo - step * 1e-6 && snapped <= hi + step * 1e-6 {
            values.push(snapped);
            values.push(snapped - step);
            values.push(snapped + step);
        }
    }
    if lo <= 0.0 && 0.0 <= hi {
        values.push(0.0);
    }
    if let Some(first) = seed_values.first() {
        values.push((first / step).floor() * step);
    }
    if let Some(last) = seed_values.last() {
        values.push((last / step).ceil() * step);
    }
    values.retain(|v| *v >= lo - step * 1e-6 && *v <= hi + step * 1e-6);
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    dedup_close_values(&mut values);
    values
}

fn select_screen_aligned_ticks(
    vp: &glam::Mat4,
    axis_kind: Axis3,
    lo: f64,
    hi: f64,
    candidates: &[f64],
    target_count: usize,
) -> Option<Vec<f64>> {
    let mut projected: Vec<(f64, f32)> = candidates
        .iter()
        .filter_map(|&value| {
            projected_axis_fraction(vp, axis_kind, lo as f32, hi as f32, value as f32)
                .map(|fraction| (value, fraction))
        })
        .collect();
    if projected.len() < 2 {
        return None;
    }
    projected.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut selected = Vec::new();
    for i in 0..target_count {
        let target_fraction = if target_count <= 1 {
            0.0
        } else {
            i as f32 / (target_count - 1) as f32
        };
        if let Some((value, _)) = projected.iter().min_by(|a, b| {
            (a.1 - target_fraction)
                .abs()
                .partial_cmp(&(b.1 - target_fraction).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            selected.push(*value);
        }
    }
    if lo <= 0.0 && 0.0 <= hi {
        selected.push(0.0);
    }
    selected.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    dedup_close_values(&mut selected);
    if selected.len() < 2 {
        None
    } else {
        Some(selected)
    }
}

fn tick_selection_score(
    vp: &glam::Mat4,
    axis_kind: Axis3,
    lo: f64,
    hi: f64,
    values: &[f64],
    target_count: usize,
) -> f32 {
    let projected: Vec<f32> = values
        .iter()
        .filter_map(|&value| {
            projected_axis_fraction(vp, axis_kind, lo as f32, hi as f32, value as f32)
        })
        .collect();
    if projected.is_empty() {
        return f32::INFINITY;
    }
    let mut error = (projected.len() as i32 - target_count as i32).unsigned_abs() as f32 * 0.4;
    for (i, fraction) in projected.iter().enumerate() {
        let target = if target_count <= 1 {
            0.0
        } else {
            i as f32 / (target_count.saturating_sub(1).max(1)) as f32
        };
        error += (*fraction - target).abs();
    }
    error
}

fn format_axis_tick_values(values: &[f64]) -> Vec<(f64, String)> {
    if values.is_empty() {
        return Vec::new();
    }
    let min_step = values
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .filter(|d| *d > 1e-9)
        .fold(f64::INFINITY, f64::min);
    let decimals = if min_step.is_finite() {
        (-min_step.log10().floor()).max(0.0) as usize
    } else {
        0
    }
    .min(6);
    values
        .iter()
        .map(|&value| {
            let snapped = if value.abs() < 1e-10 { 0.0 } else { value };
            (value, format!("{:.prec$}", snapped, prec = decimals))
        })
        .collect()
}

fn project_to_ndc_xy(vp: &glam::Mat4, world: glam::Vec3) -> Option<glam::Vec2> {
    let clip = *vp * world.extend(1.0);
    if clip.w.abs() < 1e-6 || clip.z < -clip.w || clip.z > clip.w {
        return None;
    }
    let inv_w = 1.0 / clip.w;
    Some(glam::Vec2::new(clip.x * inv_w, clip.y * inv_w))
}

fn axis_anchor_domain(axis_kind: Axis3, lo: f64, hi: f64) -> Domain {
    let range = lo..=hi;
    match axis_kind {
        Axis3::X => Domain {
            x: range.clone(),
            y: 0.0..=1.0,
            z: 0.0..=1.0,
        },
        Axis3::Y => Domain {
            x: 0.0..=1.0,
            y: range.clone(),
            z: 0.0..=1.0,
        },
        Axis3::Z => Domain {
            x: 0.0..=1.0,
            y: 0.0..=1.0,
            z: range,
        },
    }
}
