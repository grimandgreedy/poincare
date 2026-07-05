//! Embedded 3D graph rendering for notebook cells.
//!
//! Each computed graph output is rendered by `poincare-lib`/`viewport-lib` into
//! its own GPU texture (`ViewportRenderer::render_to_texture`, no CPU readback),
//! which is then registered with egui and drawn as an ordinary image. Unlike an
//! `egui_wgpu` paint callback, a plain image renders correctly inside a
//! `ScrollArea`, so this works anywhere in the notebook.
//!
//! A graph is static by default (rendered once, or again when resized). The one
//! *active* graph additionally consumes drag/scroll input to orbit its camera
//! and re-renders whenever the view changes; when it is deactivated it simply
//! keeps its last rendered image.

use std::collections::HashMap;

use eframe::egui;
use eframe::wgpu;
use viewport_lib::{
    ButtonState, Camera, Modifiers, MouseButton, OrbitCameraController, ScrollUnits,
    ViewportContext, ViewportEvent, ViewportRenderer,
};

use poincare_lib::{GraphScene, GraphSpec};

/// Width-to-height ratio of a graph viewport in the notebook.
pub const PREVIEW_ASPECT: f32 = 720.0 / 440.0;
/// Default logical size used before a cell reports its real display rect.
const DEFAULT_SIZE: [f32; 2] = [720.0, 440.0];
/// Opaque background for rendered graphs (matches the dark panel fill).
const BACKGROUND: [f32; 4] = [0.06, 0.07, 0.09, 1.0];
/// egui native textures must be `Rgba8Unorm`, so the dedicated renderer and the
/// target textures all use this format.
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Owns a dedicated 3D renderer plus the per-graph scene and GPU texture state.
#[derive(Default)]
pub struct GraphRenderManager {
    renderer: Option<ViewportRenderer>,
    graphs: HashMap<String, GraphEntry>,
}

struct GraphEntry {
    scene: GraphScene,
    camera: Camera,
    controller: OrbitCameraController,
    /// Logical (point) size of the display rect, updated by the cell each frame.
    logical_size: [f32; 2],
    pixels_per_point: f32,
    /// The rendered image is out of date and must be redrawn.
    dirty: bool,
    gpu: Option<GpuTexture>,
}

struct GpuTexture {
    texture: wgpu::Texture,
    id: egui::TextureId,
    /// Physical pixel size of the texture.
    size: [u32; 2],
}

impl GraphRenderManager {
    /// Reconcile GPU state with the current document and render each graph into
    /// its texture. Runs where the wgpu render state is available (before cells).
    pub fn sync(
        &mut self,
        frame: &mut eframe::Frame,
        graphs: &[(String, GraphSpec)],
        _active_id: Option<&str>,
    ) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };
        let device = &render_state.device;
        let queue = &render_state.queue;
        let renderer = self
            .renderer
            .get_or_insert_with(|| ViewportRenderer::new(device, TEXTURE_FORMAT));
        let mut egui_renderer = render_state.renderer.write();

        // Drop scenes and egui textures for graphs that no longer exist.
        self.graphs.retain(|id, entry| {
            let keep = graphs.iter().any(|(gid, _)| gid == id);
            if !keep {
                if let Some(gpu) = &entry.gpu {
                    egui_renderer.free_texture(&gpu.id);
                }
                entry.scene.release_gpu_resources(renderer.resources_mut());
            }
            keep
        });

        for (id, spec) in graphs {
            let entry = match self.graphs.get_mut(id) {
                Some(entry) => entry,
                None => {
                    let Some(entry) = build_entry(spec, renderer, device, queue) else {
                        continue;
                    };
                    self.graphs.entry(id.clone()).or_insert(entry)
                }
            };

            // Ensure a texture at the current display resolution.
            let physical = [
                (entry.logical_size[0] * entry.pixels_per_point).round().max(1.0) as u32,
                (entry.logical_size[1] * entry.pixels_per_point).round().max(1.0) as u32,
            ];
            let needs_texture = entry.gpu.as_ref().map(|g| g.size) != Some(physical);
            if needs_texture {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("graph-viewport"),
                    size: wgpu::Extent3d {
                        width: physical[0],
                        height: physical[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TEXTURE_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let id = match &entry.gpu {
                    Some(gpu) => {
                        egui_renderer.update_egui_texture_from_wgpu_texture(
                            device,
                            &view,
                            wgpu::FilterMode::Linear,
                            gpu.id,
                        );
                        gpu.id
                    }
                    None => {
                        egui_renderer.register_native_texture(device, &view, wgpu::FilterMode::Linear)
                    }
                };
                entry.gpu = Some(GpuTexture {
                    texture,
                    id,
                    size: physical,
                });
                entry.dirty = true;
            }

            if entry.dirty {
                if let Some(gpu) = &entry.gpu {
                    let view = gpu.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut frame_data = entry.scene.build_frame(&entry.camera);
                    frame_data.camera.viewport_size = entry.logical_size;
                    frame_data.camera.pixels_per_point = entry.pixels_per_point;
                    frame_data.viewport.background_colour = Some(BACKGROUND);
                    renderer.render_to_texture(device, queue, &view, &frame_data);
                    entry.dirty = false;
                }
            }
        }
    }

    /// Update a graph's display size and, if it is the active graph, apply orbit
    /// input from `response`. Call once per frame from the cell.
    pub fn interact(
        &mut self,
        id: &str,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        is_active: bool,
    ) {
        let Some(entry) = self.graphs.get_mut(id) else {
            return;
        };
        let ppp = ui.ctx().pixels_per_point();
        if entry.logical_size != [rect.width(), rect.height()] || entry.pixels_per_point != ppp {
            entry.logical_size = [rect.width(), rect.height()];
            entry.pixels_per_point = ppp;
            entry.dirty = true;
        }

        if is_active {
            let moved = push_orbit_events(&mut entry.controller, ui, response, rect);
            entry.controller.apply_to_camera(&mut entry.camera);
            if rect.height() > 0.0 {
                entry.camera.set_aspect_ratio(rect.width(), rect.height());
            }
            if moved {
                entry.dirty = true;
                ui.ctx().request_repaint();
            }
        }
    }

    /// The egui texture id for a graph's rendered image, if it is ready.
    pub fn image(&self, id: &str) -> Option<egui::TextureId> {
        self.graphs.get(id).and_then(|e| e.gpu.as_ref()).map(|g| g.id)
    }
}

/// Build a graph's scene, upload its meshes, and frame a starting camera.
fn build_entry(
    spec: &GraphSpec,
    renderer: &mut ViewportRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<GraphEntry> {
    let mut scene = spec.build_scene().ok()?;
    scene
        .upload_meshes(device, queue, renderer.resources_mut())
        .ok()?;
    let camera = framing_camera(&scene);
    Some(GraphEntry {
        scene,
        camera,
        controller: OrbitCameraController::viewport_primitives(),
        logical_size: DEFAULT_SIZE,
        pixels_per_point: 1.0,
        dirty: true,
        gpu: None,
    })
}

/// A camera framed to a scene's bounds, using the app's isometric convention.
fn framing_camera(scene: &GraphScene) -> Camera {
    let orientation = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
        * glam::Quat::from_rotation_x(1.1);
    let (center, distance) = match scene_bounds(scene) {
        Some((min, max)) => {
            let center = (min + max) * 0.5;
            let radius = (max - min).length() * 0.5;
            (center, (radius * 2.6).max(1.0))
        }
        None => (glam::Vec3::ZERO, 35.0),
    };
    Camera {
        center,
        distance,
        orientation,
        ..Camera::default()
    }
}

/// The world-space bounding box of a scene's renderable geometry.
fn scene_bounds(scene: &GraphScene) -> Option<(glam::Vec3, glam::Vec3)> {
    let data = scene.probe_data();
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    let mut consider = |pos: glam::Vec3| {
        min = min.min(pos);
        max = max.max(pos);
        any = true;
    };
    for surface in &data.surfaces {
        for &pos in surface.positions {
            consider(pos.into());
        }
    }
    for polyline in &data.polylines {
        for &pos in polyline.positions {
            consider(pos.into());
        }
    }
    for points in &data.points {
        for &pos in points.positions {
            consider(pos.into());
        }
    }
    if !any {
        return None;
    }
    if (max - min).length_squared() < 1.0e-8 {
        let pad = glam::Vec3::splat(0.5);
        min -= pad;
        max += pad;
    }
    Some((min, max))
}

/// Translate egui pointer/scroll input into orbit-controller events. Returns
/// whether any navigation input (drag or scroll) was applied this frame.
fn push_orbit_events(
    controller: &mut OrbitCameraController,
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
) -> bool {
    let hovered = response.hovered();
    controller.begin_frame(ViewportContext {
        hovered,
        focused: hovered,
        viewport_size: [rect.width(), rect.height()],
    });

    let mut moved = false;
    ui.input(|i| {
        let owns_pointer = hovered || response.dragged() || response.is_pointer_button_down_on();

        controller.push_event(ViewportEvent::ModifiersChanged(Modifiers {
            alt: i.modifiers.alt,
            shift: i.modifiers.shift,
            ctrl: i.modifiers.command,
        }));

        if owns_pointer {
            if let Some(pos) = i.pointer.interact_pos() {
                let local = glam::Vec2::new(pos.x - rect.left(), pos.y - rect.top());
                controller.push_event(ViewportEvent::PointerMoved { position: local });
            }
        }

        for event in &i.events {
            match event {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => {
                    if !owns_pointer {
                        continue;
                    }
                    let vp_button = match button {
                        egui::PointerButton::Primary => MouseButton::Left,
                        egui::PointerButton::Secondary => MouseButton::Right,
                        egui::PointerButton::Middle => MouseButton::Middle,
                        _ => continue,
                    };
                    controller.push_event(ViewportEvent::MouseButton {
                        button: vp_button,
                        state: if *pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                    });
                }
                egui::Event::MouseWheel { unit, delta, .. } => {
                    if !hovered {
                        continue;
                    }
                    let units = match unit {
                        egui::MouseWheelUnit::Line => ScrollUnits::Lines,
                        egui::MouseWheelUnit::Point => ScrollUnits::Pixels,
                        egui::MouseWheelUnit::Page => ScrollUnits::Pages,
                    };
                    controller.push_event(ViewportEvent::Wheel {
                        delta: glam::Vec2::new(delta.x, delta.y),
                        units,
                    });
                    moved = true;
                }
                _ => {}
            }
        }
    });

    if response.dragged() {
        moved = true;
    }
    moved
}
