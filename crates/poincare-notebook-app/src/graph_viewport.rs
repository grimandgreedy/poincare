//! Embedded 3D graph rendering for notebook cells.
//!
//! Computed graph outputs render through `poincare-lib`/`viewport-lib`. Each
//! graph is a **static headless image** by default — rendered once (at evaluate
//! time / on the first frame it appears, and again when interaction ends) and
//! shown as an egui texture. Clicking a graph makes it the single **active**
//! graph, which renders live through an `egui_wgpu` paint callback with orbit
//! controls. When it is deactivated, a fresh headless image is captured from
//! the last camera and it reverts to static.

use std::collections::HashMap;

use eframe::egui;
use viewport_lib::{
    ButtonState, Camera, FrameData, Modifiers, MouseButton, OrbitCameraController, ScrollUnits,
    ViewportContext, ViewportEvent, ViewportRenderer,
};

use poincare_lib::{GraphScene, GraphSpec};

/// Size, in physical pixels, of a headless graph preview image.
const PREVIEW_SIZE: [u32; 2] = [720, 440];
/// Opaque background for rendered graphs (matches the dark panel fill).
const BACKGROUND: [f32; 4] = [0.06, 0.07, 0.09, 1.0];

/// Owns the GPU-backed render state for all graph outputs: cached static
/// images plus the one live, interactive graph.
#[derive(Default)]
pub struct GraphRenderManager {
    statics: HashMap<String, egui::TextureHandle>,
    live: Option<LiveGraph>,
}

struct LiveGraph {
    id: String,
    scene: GraphScene,
    camera: Camera,
    controller: OrbitCameraController,
}

impl GraphRenderManager {
    /// Reconcile GPU state with the current document each frame.
    ///
    /// `graphs` lists every visible graph output (id + its spec), and
    /// `active_id` is the graph the user is currently interacting with, if any.
    pub fn sync(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        graphs: &[(String, GraphSpec)],
        active_id: Option<&str>,
    ) {
        // Drop cached images and any live scene for graphs that no longer exist.
        self.statics
            .retain(|id, _| graphs.iter().any(|(gid, _)| gid == id));

        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };
        let device = render_state.device.clone();
        let queue = render_state.queue.clone();
        let mut renderer_guard = render_state.renderer.write();
        let Some(renderer) = renderer_guard
            .callback_resources
            .get_mut::<ViewportRenderer>()
        else {
            return;
        };

        // If the active graph changed (including deactivation), retire the old
        // live scene: capture a final static image from its camera, then free
        // its GPU meshes.
        if self.live.as_ref().map(|l| l.id.as_str()) != active_id {
            if let Some(live) = self.live.take() {
                if let Some(texture) =
                    render_texture(ctx, renderer, &device, &queue, &live.scene, &live.camera)
                {
                    self.statics.insert(live.id.clone(), texture);
                }
                live.scene.release_gpu_resources(renderer.resources_mut());
            }

            // Promote the newly active graph to live: build and upload its scene.
            if let Some(id) = active_id {
                if let Some((_, spec)) = graphs.iter().find(|(gid, _)| gid == id) {
                    if let Some(live) = build_live(id, spec, renderer, &device, &queue) {
                        self.live = Some(live);
                        // The stale static image is replaced by the live view.
                        self.statics.remove(id);
                    }
                }
            }
        }

        // Ensure every non-active graph has a static image.
        for (id, spec) in graphs {
            if active_id == Some(id.as_str()) || self.statics.contains_key(id) {
                continue;
            }
            if let Some(texture) =
                render_spec_texture(ctx, renderer, &device, &queue, spec)
            {
                self.statics.insert(id.clone(), texture);
            }
        }
    }

    /// The cached static image for a graph, if one has been rendered.
    pub fn static_texture(&self, id: &str) -> Option<&egui::TextureHandle> {
        self.statics.get(id)
    }

    /// Whether `id` is the live, interactive graph.
    pub fn is_live(&self, id: &str) -> bool {
        self.live.as_ref().map(|l| l.id.as_str()) == Some(id)
    }

    /// Drive and render the live graph into `rect`, applying orbit input.
    /// Does nothing if `id` is not the live graph.
    pub fn show_live(
        &mut self,
        id: &str,
        ui: &egui::Ui,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let Some(live) = self.live.as_mut().filter(|l| l.id == id) else {
            return;
        };

        push_orbit_events(&mut live.controller, ui, response, rect);
        live.controller.apply_to_camera(&mut live.camera);
        if rect.height() > 0.0 {
            live.camera.set_aspect_ratio(rect.width(), rect.height());
        }

        let mut frame_data = live.scene.build_frame(&live.camera);
        frame_data.camera.viewport_size = [rect.width(), rect.height()];
        frame_data.camera.pixels_per_point = ui.ctx().pixels_per_point();
        frame_data.viewport.background_colour = Some(BACKGROUND);

        ui.painter().add(eframe::egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback { frame: frame_data },
        ));
    }
}

/// Build and upload the scene for a graph so it can render live.
fn build_live(
    id: &str,
    spec: &GraphSpec,
    renderer: &mut ViewportRenderer,
    device: &eframe::wgpu::Device,
    queue: &eframe::wgpu::Queue,
) -> Option<LiveGraph> {
    let mut scene = spec.build_scene().ok()?;
    scene
        .upload_meshes(device, queue, renderer.resources_mut())
        .ok()?;
    let camera = framing_camera(&scene);
    Some(LiveGraph {
        id: id.to_string(),
        scene,
        camera,
        controller: OrbitCameraController::viewport_primitives(),
    })
}

/// Build a scene from a spec, upload it, headless-render it, then release it.
fn render_spec_texture(
    ctx: &egui::Context,
    renderer: &mut ViewportRenderer,
    device: &eframe::wgpu::Device,
    queue: &eframe::wgpu::Queue,
    spec: &GraphSpec,
) -> Option<egui::TextureHandle> {
    let mut scene = spec.build_scene().ok()?;
    scene
        .upload_meshes(device, queue, renderer.resources_mut())
        .ok()?;
    let camera = framing_camera(&scene);
    let texture = render_texture(ctx, renderer, device, queue, &scene, &camera);
    scene.release_gpu_resources(renderer.resources_mut());
    texture
}

/// Headless-render an already-uploaded scene into an egui texture.
fn render_texture(
    ctx: &egui::Context,
    renderer: &mut ViewportRenderer,
    device: &eframe::wgpu::Device,
    queue: &eframe::wgpu::Queue,
    scene: &GraphScene,
    camera: &Camera,
) -> Option<egui::TextureHandle> {
    let [width, height] = PREVIEW_SIZE;
    let mut camera = camera.clone();
    camera.set_aspect_ratio(width as f32, height as f32);

    let mut frame_data = scene.build_frame(&camera);
    frame_data.camera.viewport_size = [width as f32, height as f32];
    frame_data.viewport.background_colour = Some(BACKGROUND);

    let pixels = renderer.render_offscreen(device, queue, &frame_data, width, height);
    if pixels.len() != (width * height * 4) as usize {
        return None;
    }
    let image =
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
    Some(ctx.load_texture("graph-preview", image, egui::TextureOptions::LINEAR))
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

/// Translate egui pointer/scroll input into orbit-controller events.
fn push_orbit_events(
    controller: &mut OrbitCameraController,
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
) {
    let hovered = response.hovered();
    controller.begin_frame(ViewportContext {
        hovered,
        focused: hovered,
        viewport_size: [rect.width(), rect.height()],
    });

    ui.input(|i| {
        let owns_pointer =
            hovered || response.dragged() || response.is_pointer_button_down_on();

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
                }
                _ => {}
            }
        }
    });
}

/// egui_wgpu callback that draws one graph frame through the shared renderer.
struct ViewportCallback {
    frame: FrameData,
}

impl eframe::egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &eframe::wgpu::Device,
        queue: &eframe::wgpu::Queue,
        _screen: &eframe::egui_wgpu::ScreenDescriptor,
        _encoder: &mut eframe::wgpu::CommandEncoder,
        resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<eframe::wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<ViewportRenderer>() {
            return renderer.pass().prepare(device, queue, &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut eframe::wgpu::RenderPass<'static>,
        resources: &eframe::egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<ViewportRenderer>() {
            renderer.pass_view().paint(render_pass, &self.frame);
        }
    }
}
