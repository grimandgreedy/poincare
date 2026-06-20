mod mobile_ui;

use std::collections::HashMap;
use std::sync::Arc;

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use poincare_lib::{AxisConfig, GraphScene, GraphSpec};
use poincare_mobile_core::{MobileModel, UiCommand};
use viewport_lib::{
    ButtonState, Camera, CameraFrame, GroundPlane, GroundPlaneMode, LightingSettings, MouseButton,
    OrbitCameraController, PostProcessSettings, ScrollUnits, ViewportContext, ViewportEvent,
    ViewportRenderer,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, Touch, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
#[cfg(any(
    target_os = "android",
    not(any(target_os = "ios", target_os = "android"))
))]
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Default, PartialEq)]
enum TouchMode {
    #[default]
    None,
    OneFingerOrbit,
    TwoFingerPan,
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
    startup_redraws_remaining: u8,
}

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: ViewportRenderer,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: EguiRenderer,
    camera: Camera,
    controller: OrbitCameraController,
    model: MobileModel,
    scene: GraphScene,
    touches: HashMap<u64, glam::Vec2>,
    direct_ui_touches: HashMap<u64, Vec<UiCommand>>,
    ui_hit_regions: Vec<mobile_ui::HitRegion>,
    touch_mode: TouchMode,
    prev_pinch_dist: Option<f32>,
}

#[derive(Default)]
struct DirectUiTouch {
    skip_egui: bool,
    commands: Option<Vec<UiCommand>>,
}

impl AppState {
    fn rebuild_scene(&mut self) {
        let spec = GraphSpec {
            axis_config: AxisConfig::default(),
            plots: self.model.plots(),
        };

        match spec.build_scene() {
            Ok(mut scene) => {
                match scene.upload_meshes(&self.device, &self.queue, self.renderer.resources_mut())
                {
                    Ok(()) => {
                        self.scene = scene;
                        self.model.clear_scene_error();
                    }
                    Err(err) => {
                        self.model
                            .set_scene_error(format!("mesh upload failed: {err}"));
                    }
                }
            }
            Err(err) => {
                self.model.set_scene_error(err.to_string());
            }
        }
    }

    fn apply_ui_commands(&mut self, commands: impl IntoIterator<Item = UiCommand>) -> bool {
        let effects = self.model.apply_commands(commands);
        if effects.plot_changed {
            self.rebuild_scene();
        }
        if effects.redraw_requested {
            self.window.request_redraw();
        }
        effects.redraw_requested
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default().with_title("Poincare Mobile"))
                .expect("window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: mobile_backends(),
            #[cfg(target_os = "android")]
            flags: wgpu::InstanceFlags::empty(),
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");

        let required_features = if adapter
            .features()
            .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE)
        {
            wgpu::Features::INDIRECT_FIRST_INSTANCE
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let renderer = ViewportRenderer::new(&device, format);
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        let egui_renderer =
            EguiRenderer::new(&device, format, egui_wgpu::RendererOptions::default());
        let camera = Camera {
            distance: 8.0,
            ..Camera::default()
        };
        let mut controller = OrbitCameraController::viewport_primitives();
        controller.begin_frame(ViewportContext {
            hovered: true,
            focused: true,
            viewport_size: [surface_config.width as f32, surface_config.height as f32],
        });

        let mut state = AppState {
            window,
            surface,
            device,
            queue,
            surface_config,
            renderer,
            egui_ctx,
            egui_state,
            egui_renderer,
            camera,
            controller,
            model: MobileModel::new(),
            scene: GraphScene::new(),
            touches: HashMap::new(),
            direct_ui_touches: HashMap::new(),
            ui_hit_regions: Vec::new(),
            touch_mode: TouchMode::None,
            prev_pinch_dist: None,
        };
        state.rebuild_scene();
        self.state = Some(state);
        self.startup_redraws_remaining = 2;
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
        self.startup_redraws_remaining = 0;
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.startup_redraws_remaining == 0 {
            return;
        }

        self.startup_redraws_remaining -= 1;
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        let direct_ui_touch = direct_ui_touch(state, &event);
        let is_touch_event = matches!(event, WindowEvent::Touch { .. });
        let egui_response = if direct_ui_touch.skip_egui {
            egui_winit::EventResponse::default()
        } else {
            state.egui_state.on_window_event(&state.window, &event)
        };
        if egui_response.repaint {
            state.window.request_redraw();
        }
        if direct_ui_touch.skip_egui {
            if let Some(commands) = direct_ui_touch.commands {
                state.apply_ui_commands(commands);
            }
            state.window.request_redraw();
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    state.surface_config.width = size.width;
                    state.surface_config.height = size.height;
                    state
                        .surface
                        .configure(&state.device, &state.surface_config);
                    state.window.request_redraw();
                }
            }

            WindowEvent::Touch(Touch {
                phase,
                location,
                id,
                ..
            }) => {
                let pos = glam::Vec2::new(location.x as f32, location.y as f32);
                let egui_has_touch = egui_response.consumed
                    || state.egui_ctx.wants_pointer_input()
                    || state.egui_ctx.is_using_pointer();
                let viewport_has_touch = state.touches.contains_key(&id);
                if !egui_has_touch || viewport_has_touch {
                    handle_touch(state, phase, id, pos);
                }
                state.window.request_redraw();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if !egui_response.consumed {
                    handle_keyboard(state, event);
                }
            }

            WindowEvent::RedrawRequested => render(state),

            _ => {}
        }

        if is_touch_event {
            state.window.request_redraw();
        }
    }
}

fn render(state: &mut AppState) {
    let frame = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            state
                .surface
                .configure(&state.device, &state.surface_config);
            return;
        }
        Err(_) => return,
    };

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let w = state.surface_config.width as f32;
    let h = state.surface_config.height as f32;

    state.controller.apply_to_camera(&mut state.camera);
    state.camera.set_aspect_ratio(w, h);

    let mut frame_data = state
        .scene
        .build_frame_with_selection(&state.camera, Some(1), None);
    frame_data.camera = CameraFrame::from_camera(&state.camera, [w, h]);
    frame_data.viewport.show_grid = state.model.show_grid();
    frame_data.viewport.show_axes_indicator = true;
    frame_data.viewport.background_colour = Some([0.06, 0.06, 0.07, 1.0]);
    frame_data.effects.lighting = LightingSettings::default();
    frame_data.effects.ground_plane = GroundPlane {
        mode: if state.model.show_ground() {
            GroundPlaneMode::Tile
        } else {
            GroundPlaneMode::None
        },
        height: 0.0,
        colour: [0.18, 0.18, 0.18, 1.0],
        tile_colour2: [0.12, 0.12, 0.12, 1.0],
        tile_size: 1.0,
        shadow_colour: [0.0, 0.0, 0.0, 1.0],
        shadow_opacity: 0.25,
    };
    frame_data.effects.post_process = {
        let mut pp = PostProcessSettings::default();
        pp.enabled = true;
        pp.bloom = true;
        pp.bloom_threshold = 1.0;
        pp.bloom_intensity = 0.1;
        pp
    };

    let viewport_cmd =
        state
            .renderer
            .owned()
            .render(&state.device, &state.queue, &view, &frame_data);

    let egui_input = state.egui_state.take_egui_input(&state.window);
    let egui_ctx = state.egui_ctx.clone();
    let mut ui_requested_redraw = false;
    let egui_output = egui_ctx.run(egui_input, |ctx| {
        ui_requested_redraw = render_ui(state, ctx);
    });
    state
        .egui_state
        .handle_platform_output(&state.window, egui_output.platform_output);

    let paint_jobs = state
        .egui_ctx
        .tessellate(egui_output.shapes, egui_output.pixels_per_point);
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [state.surface_config.width, state.surface_config.height],
        pixels_per_point: egui_output.pixels_per_point,
    };

    for (id, image_delta) in &egui_output.textures_delta.set {
        state
            .egui_renderer
            .update_texture(&state.device, &state.queue, *id, image_delta);
    }

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui_overlay_encoder"),
        });
    state.egui_renderer.update_buffers(
        &state.device,
        &state.queue,
        &mut encoder,
        &paint_jobs,
        &screen_descriptor,
    );
    {
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_overlay_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        state
            .egui_renderer
            .render(&mut render_pass, &paint_jobs, &screen_descriptor);
    }

    for id in &egui_output.textures_delta.free {
        state.egui_renderer.free_texture(id);
    }

    state
        .queue
        .submit([viewport_cmd, encoder.finish()].into_iter());
    frame.present();
    if ui_requested_redraw {
        state.window.request_redraw();
    }

    state.controller.begin_frame(ViewportContext {
        hovered: true,
        focused: true,
        viewport_size: [w, h],
    });
}

fn render_ui(state: &mut AppState, ctx: &egui::Context) -> bool {
    let snapshot = state.model.snapshot();
    let output = mobile_ui::render(ctx, &snapshot);
    state.ui_hit_regions = output.hit_regions;
    state.apply_ui_commands(output.commands)
}

fn direct_ui_touch(state: &mut AppState, event: &WindowEvent) -> DirectUiTouch {
    let WindowEvent::Touch(touch) = event else {
        return DirectUiTouch::default();
    };

    match touch.phase {
        TouchPhase::Started => {
            if let Some(commands) = direct_ui_commands_at_touch(state, touch) {
                state.direct_ui_touches.insert(touch.id, commands);
                return DirectUiTouch {
                    skip_egui: true,
                    commands: None,
                };
            }
        }
        TouchPhase::Ended => {
            if let Some(commands) = state.direct_ui_touches.remove(&touch.id) {
                let commands =
                    if direct_ui_commands_at_touch(state, touch) == Some(commands.clone()) {
                        Some(commands)
                    } else {
                        None
                    };
                return DirectUiTouch {
                    skip_egui: true,
                    commands,
                };
            }
        }
        TouchPhase::Moved => {
            if state.direct_ui_touches.contains_key(&touch.id) {
                return DirectUiTouch {
                    skip_egui: true,
                    commands: None,
                };
            }
        }
        TouchPhase::Cancelled => {
            if state.direct_ui_touches.remove(&touch.id).is_some() {
                return DirectUiTouch {
                    skip_egui: true,
                    commands: None,
                };
            }
        }
    }

    DirectUiTouch::default()
}

fn direct_ui_commands_at_touch(state: &AppState, touch: &Touch) -> Option<Vec<UiCommand>> {
    let raw_pos = egui::pos2(touch.location.x as f32, touch.location.y as f32);
    let scaled_pos = egui_touch_pos(state, touch);

    state
        .ui_hit_regions
        .iter()
        .rev()
        .find(|region| region.rect.contains(scaled_pos) || region.rect.contains(raw_pos))
        .map(|region| region.commands.clone())
        .or_else(|| {
            let screen_size = egui_screen_size(state);
            mobile_ui::hit_top_control(screen_size, scaled_pos).map(|command| vec![command])
        })
        .or_else(|| {
            let size = state.window.inner_size();
            let screen_size = egui::vec2(size.width as f32, size.height as f32);
            mobile_ui::hit_top_control(screen_size, raw_pos).map(|command| vec![command])
        })
}

fn egui_touch_pos(state: &AppState, touch: &Touch) -> egui::Pos2 {
    let scale = state.window.scale_factor() as f32;
    egui::pos2(
        touch.location.x as f32 / scale,
        touch.location.y as f32 / scale,
    )
}

fn egui_screen_size(state: &AppState) -> egui::Vec2 {
    let scale = state.window.scale_factor() as f32;
    let size = state.window.inner_size();
    egui::vec2(size.width as f32 / scale, size.height as f32 / scale)
}

fn handle_keyboard(state: &mut AppState, event: KeyEvent) {
    if event.state != ElementState::Pressed || event.repeat {
        return;
    }
    match event.logical_key {
        Key::Character(ref key) if key == "+" => {
            state.apply_ui_commands([UiCommand::OpenEditor]);
        }
        _ => {}
    }
}

fn handle_touch(state: &mut AppState, phase: TouchPhase, id: u64, pos: glam::Vec2) {
    match phase {
        TouchPhase::Started => {
            state.touches.insert(id, pos);

            match state.touches.len() {
                1 => {
                    state
                        .controller
                        .push_event(ViewportEvent::PointerMoved { position: pos });
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                    });
                    state.touch_mode = TouchMode::OneFingerOrbit;
                }
                2 => {
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    });
                    let centroid = touches_centroid(&state.touches);
                    state
                        .controller
                        .push_event(ViewportEvent::PointerMoved { position: centroid });
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Middle,
                        state: ButtonState::Pressed,
                    });
                    state.prev_pinch_dist = Some(touches_distance(&state.touches));
                    state.touch_mode = TouchMode::TwoFingerPan;
                }
                3 => {
                    release_touch_buttons(state);
                    state.apply_ui_commands([UiCommand::OpenEditor]);
                }
                _ => {}
            }
        }

        TouchPhase::Moved => {
            state.touches.insert(id, pos);

            match state.touch_mode {
                TouchMode::OneFingerOrbit => {
                    state
                        .controller
                        .push_event(ViewportEvent::PointerMoved { position: pos });
                }
                TouchMode::TwoFingerPan => {
                    let centroid = touches_centroid(&state.touches);
                    state
                        .controller
                        .push_event(ViewportEvent::PointerMoved { position: centroid });

                    let dist = touches_distance(&state.touches);
                    if let Some(prev) = state.prev_pinch_dist {
                        let delta = dist - prev;
                        if delta.abs() > 0.5 {
                            state.controller.push_event(ViewportEvent::Wheel {
                                delta: glam::Vec2::new(0.0, delta * 0.05),
                                units: ScrollUnits::Pixels,
                            });
                        }
                    }
                    state.prev_pinch_dist = Some(dist);
                }
                TouchMode::None => {}
            }
        }

        TouchPhase::Ended | TouchPhase::Cancelled => {
            state.touches.remove(&id);

            match state.touch_mode {
                TouchMode::OneFingerOrbit => {
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    });
                    state.touch_mode = TouchMode::None;
                }
                TouchMode::TwoFingerPan => {
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Middle,
                        state: ButtonState::Released,
                    });
                    state.prev_pinch_dist = None;

                    if state.touches.len() == 1 {
                        let remaining = *state.touches.values().next().unwrap();
                        state.controller.push_event(ViewportEvent::PointerMoved {
                            position: remaining,
                        });
                        state.controller.push_event(ViewportEvent::MouseButton {
                            button: MouseButton::Left,
                            state: ButtonState::Pressed,
                        });
                        state.touch_mode = TouchMode::OneFingerOrbit;
                    } else {
                        state.touch_mode = TouchMode::None;
                    }
                }
                TouchMode::None => {}
            }
        }
    }
}

fn release_touch_buttons(state: &mut AppState) {
    state.controller.push_event(ViewportEvent::MouseButton {
        button: MouseButton::Left,
        state: ButtonState::Released,
    });
    state.controller.push_event(ViewportEvent::MouseButton {
        button: MouseButton::Middle,
        state: ButtonState::Released,
    });
    state.touch_mode = TouchMode::None;
    state.prev_pinch_dist = None;
}

fn touches_centroid(touches: &HashMap<u64, glam::Vec2>) -> glam::Vec2 {
    touches.values().copied().sum::<glam::Vec2>() / touches.len() as f32
}

fn touches_distance(touches: &HashMap<u64, glam::Vec2>) -> f32 {
    let mut it = touches.values().copied();
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => (a - b).length(),
        _ => 0.0,
    }
}

fn mobile_backends() -> wgpu::Backends {
    #[cfg(target_os = "ios")]
    {
        wgpu::Backends::METAL
    }
    #[cfg(target_os = "android")]
    {
        wgpu::Backends::VULKAN
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        wgpu::Backends::PRIMARY
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn run() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.run_app(&mut App::default()).expect("run");
}

#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn run() {
    panic!("use the platform entrypoint on mobile targets");
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn start_app() {
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    event_loop.run_app(&mut App::default()).expect("run");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    use winit::error::EventLoopError;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    let event_loop = match EventLoop::builder().with_android_app(app).build() {
        Ok(event_loop) => event_loop,
        Err(EventLoopError::RecreationAttempt) => return,
        Err(err) => panic!("event loop: {err}"),
    };
    match event_loop.run_app(&mut App::default()) {
        Ok(()) | Err(EventLoopError::RecreationAttempt) => {}
        Err(err) => panic!("run: {err}"),
    }
}
