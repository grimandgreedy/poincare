mod mobile_ui;
mod model;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use model::{MobileModel, UiCommand};
use poincare_lib::{AxisConfig, GraphScene, GraphSpec};
use viewport_lib::{
    ButtonState, Camera, CameraFrame, GroundPlane, GroundPlaneMode, LightingSettings, MouseButton,
    OrbitCameraController, PostProcessSettings, ScrollUnits, ViewportContext, ViewportEvent,
    ViewportRenderer, picking,
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

const DOUBLE_TAP_MAX_INTERVAL: Duration = Duration::from_millis(360);
const DOUBLE_TAP_MAX_DISTANCE: f32 = 96.0;
const TAP_MAX_MOVEMENT: f32 = 24.0;

#[derive(Default, PartialEq)]
enum TouchMode {
    #[default]
    None,
    OneFingerOrbit,
    TwoFingerZoom,
}

struct TapRecord {
    time: Instant,
    pos: glam::Vec2,
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
    startup_redraws_remaining: u8,
    pipeline_cache_path: Option<PathBuf>,
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
    touch_starts: HashMap<u64, glam::Vec2>,
    direct_ui_touches: HashMap<u64, Vec<UiCommand>>,
    ui_hit_regions: Vec<mobile_ui::HitRegion>,
    touch_mode: TouchMode,
    prev_pinch_dist: Option<f32>,
    last_tap: Option<TapRecord>,
    frame_count: u64,
    rebuild_count: u64,
    pipeline_cache_path: Option<PathBuf>,
    pipeline_cache_saved: bool,
}

impl App {
    #[cfg(target_os = "android")]
    fn with_pipeline_cache_path(pipeline_cache_path: Option<PathBuf>) -> Self {
        Self {
            pipeline_cache_path,
            ..Default::default()
        }
    }
}

#[derive(Default)]
struct DirectUiTouch {
    skip_egui: bool,
    commands: Option<Vec<UiCommand>>,
}

impl AppState {
    fn rebuild_scene(&mut self) {
        self.rebuild_count += 1;
        let rebuild_index = self.rebuild_count;
        let total_start = Instant::now();
        let spec = GraphSpec {
            axis_config: mobile_axis_config(self.window.scale_factor() as f32),
            plots: self.model.plots(),
        };
        let plot_count = spec.plots.len();
        mobile_log(format_args!(
            "rebuild_scene #{rebuild_index} start plots={plot_count}"
        ));

        let build_start = Instant::now();
        match spec.build_scene() {
            Ok(mut scene) => {
                let build_elapsed = build_start.elapsed();
                let upload_start = Instant::now();
                match scene.upload_meshes(&self.device, &self.queue, self.renderer.resources_mut())
                {
                    Ok(()) => {
                        let upload_elapsed = upload_start.elapsed();
                        let total_elapsed = total_start.elapsed();
                        self.scene = scene;
                        self.model.clear_scene_error();
                        mobile_log(format_args!(
                            "rebuild_scene #{rebuild_index} ok plots={plot_count} build_scene={} upload_meshes={} total={}",
                            fmt_duration(build_elapsed),
                            fmt_duration(upload_elapsed),
                            fmt_duration(total_elapsed),
                        ));
                    }
                    Err(err) => {
                        mobile_log(format_args!(
                            "rebuild_scene #{rebuild_index} upload failed after {}: {err}",
                            fmt_duration(upload_start.elapsed()),
                        ));
                        self.model
                            .set_scene_error(format!("mesh upload failed: {err}"));
                    }
                }
            }
            Err(err) => {
                mobile_log(format_args!(
                    "rebuild_scene #{rebuild_index} build failed after {}: {err}",
                    fmt_duration(build_start.elapsed()),
                ));
                self.model.set_scene_error(err.to_string());
            }
        }
    }

    fn apply_ui_commands(&mut self, commands: impl IntoIterator<Item = UiCommand>) -> bool {
        let commands = commands.into_iter().collect::<Vec<_>>();
        if !commands.is_empty() {
            mobile_log(format_args!("ui commands: {commands:?}"));
        }
        let effects = self.model.apply_commands(commands);
        if effects.plot_changed {
            mobile_log(format_args!("ui effects: plot_changed=true"));
        }
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

        let init_start = Instant::now();
        mobile_log(format_args!("init start"));

        let step_start = Instant::now();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default().with_title("Poincare"))
                .expect("window"),
        );
        mobile_log(format_args!(
            "init create_window={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: mobile_backends(),
            #[cfg(target_os = "android")]
            flags: wgpu::InstanceFlags::empty(),
            ..Default::default()
        });
        mobile_log(format_args!(
            "init instance={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        let surface = instance.create_surface(window.clone()).expect("surface");
        mobile_log(format_args!(
            "init create_surface={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");
        mobile_log(format_args!(
            "init request_adapter={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            required_features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }
        if adapter_features.contains(wgpu::Features::PIPELINE_CACHE) {
            required_features |= wgpu::Features::PIPELINE_CACHE;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("poincare-mobile"),
            required_features,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("device");
        mobile_log(format_args!(
            "init request_device={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
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
        mobile_log(format_args!(
            "init configure_surface={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        let egui_ctx = egui::Context::default();
        let mut egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        let mut egui_renderer =
            EguiRenderer::new(&device, format, egui_wgpu::RendererOptions::default());
        mobile_log(format_args!(
            "init egui={}",
            fmt_duration(step_start.elapsed()),
        ));

        render_startup_splash_frame(
            &window,
            &surface,
            &device,
            &queue,
            &surface_config,
            &egui_ctx,
            &mut egui_state,
            &mut egui_renderer,
        );

        let step_start = Instant::now();
        let pipeline_cache_path = self.pipeline_cache_path.clone();
        let saved_pipeline_cache = load_pipeline_cache(pipeline_cache_path.as_ref());
        let renderer = ViewportRenderer::new_with_pipeline_cache(
            &device,
            format,
            saved_pipeline_cache.as_deref(),
        );
        mobile_log(format_args!(
            "init viewport_renderer={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
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
        mobile_log(format_args!(
            "init camera_controller={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
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
            touch_starts: HashMap::new(),
            direct_ui_touches: HashMap::new(),
            ui_hit_regions: Vec::new(),
            touch_mode: TouchMode::None,
            prev_pinch_dist: None,
            last_tap: None,
            frame_count: 0,
            rebuild_count: 0,
            pipeline_cache_path,
            pipeline_cache_saved: false,
        };
        mobile_log(format_args!(
            "init app_state={}",
            fmt_duration(step_start.elapsed()),
        ));

        let step_start = Instant::now();
        state.rebuild_scene();
        mobile_log(format_args!(
            "init rebuild_scene_total={}",
            fmt_duration(step_start.elapsed()),
        ));
        self.state = Some(state);
        self.startup_redraws_remaining = 2;
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
        mobile_log(format_args!(
            "init total={}",
            fmt_duration(init_start.elapsed())
        ));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_mut() {
            save_pipeline_cache(state, "suspend");
        }
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

        if matches!(event, WindowEvent::RedrawRequested) {
            render(state);
            return;
        }

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

            _ => {}
        }

        if is_touch_event {
            state.window.request_redraw();
        }
    }
}

fn render(state: &mut AppState) {
    state.frame_count += 1;
    let frame_index = state.frame_count;
    let total_start = Instant::now();
    let acquire_start = Instant::now();
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
    let acquire_elapsed = acquire_start.elapsed();

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let w = state.surface_config.width as f32;
    let h = state.surface_config.height as f32;

    state.controller.apply_to_camera(&mut state.camera);
    state.camera.set_aspect_ratio(w, h);

    let build_frame_start = Instant::now();
    let mut frame_data = state
        .scene
        .build_frame_with_selection(&state.camera, Some(1), None);
    frame_data.camera = CameraFrame::from_camera(&state.camera, [w, h]);
    frame_data.viewport.show_grid = state.model.show_grid();
    frame_data.viewport.show_axes_indicator = !cfg!(target_os = "android");
    frame_data.viewport.background_colour = Some([0.06, 0.06, 0.07, 1.0]);
    frame_data.effects.lighting = mobile_lighting_settings();
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
        #[cfg(not(target_os = "android"))]
        {
            pp.enabled = true;
            pp.bloom = true;
        }
        pp.bloom_threshold = 1.0;
        pp.bloom_intensity = 0.1;
        pp
    };
    let build_frame_elapsed = build_frame_start.elapsed();

    let viewport_start = Instant::now();
    let viewport_cmd =
        state
            .renderer
            .owned()
            .render(&state.device, &state.queue, &view, &frame_data);
    let viewport_elapsed = viewport_start.elapsed();

    let egui_start = Instant::now();
    let egui_input = state.egui_state.take_egui_input(&state.window);
    let egui_ctx = state.egui_ctx.clone();
    let mut ui_requested_redraw = false;
    let egui_output = egui_ctx.run(egui_input, |ctx| {
        ui_requested_redraw = render_ui(state, ctx);
    });
    state
        .egui_state
        .handle_platform_output(&state.window, egui_output.platform_output);
    let egui_elapsed = egui_start.elapsed();

    let tessellate_start = Instant::now();
    let paint_jobs = state
        .egui_ctx
        .tessellate(egui_output.shapes, egui_output.pixels_per_point);
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [state.surface_config.width, state.surface_config.height],
        pixels_per_point: egui_output.pixels_per_point,
    };
    let tessellate_elapsed = tessellate_start.elapsed();

    let egui_upload_start = Instant::now();
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
    let egui_upload_elapsed = egui_upload_start.elapsed();

    let egui_render_start = Instant::now();
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
    let egui_render_elapsed = egui_render_start.elapsed();

    for id in &egui_output.textures_delta.free {
        state.egui_renderer.free_texture(id);
    }

    let submit_start = Instant::now();
    state
        .queue
        .submit([viewport_cmd, encoder.finish()].into_iter());
    let submit_elapsed = submit_start.elapsed();

    let present_start = Instant::now();
    frame.present();
    let present_elapsed = present_start.elapsed();
    save_pipeline_cache_once(state);
    let total_elapsed = total_start.elapsed();
    if total_elapsed >= Duration::from_millis(40) || frame_index <= 5 || frame_index % 60 == 0 {
        mobile_log(format_args!(
            "frame #{frame_index} total={} acquire={} build_frame={} viewport={} egui={} tessellate={} egui_upload={} egui_render={} submit={} present={} ui_redraw={}",
            fmt_duration(total_elapsed),
            fmt_duration(acquire_elapsed),
            fmt_duration(build_frame_elapsed),
            fmt_duration(viewport_elapsed),
            fmt_duration(egui_elapsed),
            fmt_duration(tessellate_elapsed),
            fmt_duration(egui_upload_elapsed),
            fmt_duration(egui_render_elapsed),
            fmt_duration(submit_elapsed),
            fmt_duration(present_elapsed),
            ui_requested_redraw,
        ));
    }
    if ui_requested_redraw {
        state.window.request_redraw();
    }

    state.controller.begin_frame(ViewportContext {
        hovered: true,
        focused: true,
        viewport_size: [w, h],
    });
}

fn mobile_axis_config(scale_factor: f32) -> AxisConfig {
    let visual_scale = (scale_factor / 2.0).clamp(1.0, 1.8);
    AxisConfig {
        axis_line_width: 2.4 * visual_scale,
        tick_line_width: 1.8 * visual_scale,
        tick_label_size: 14.0 * visual_scale,
        axis_label_size: 17.0 * visual_scale,
        ..AxisConfig::default()
    }
}

fn render_startup_splash_frame(
    window: &Window,
    surface: &wgpu::Surface<'static>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
    egui_ctx: &egui::Context,
    egui_state: &mut egui_winit::State,
    egui_renderer: &mut EguiRenderer,
) {
    let splash_start = Instant::now();
    let frame = match surface.get_current_texture() {
        Ok(frame) => frame,
        Err(err) => {
            mobile_log(format_args!("startup_splash skipped acquire_error={err:?}"));
            return;
        }
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let egui_input = egui_state.take_egui_input(window);
    let egui_output = egui_ctx.run(egui_input, |ctx| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(15, 17, 23)))
            .show(ctx, |ui| {
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Poincare")
                                .size(42.0)
                                .color(egui::Color32::from_rgb(232, 238, 244)),
                        );
                    },
                );
            });
    });
    egui_state.handle_platform_output(window, egui_output.platform_output);

    let paint_jobs = egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [surface_config.width, surface_config.height],
        pixels_per_point: egui_output.pixels_per_point,
    };
    for (id, image_delta) in &egui_output.textures_delta.set {
        egui_renderer.update_texture(device, queue, *id, image_delta);
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("startup_splash_encoder"),
    });
    egui_renderer.update_buffers(device, queue, &mut encoder, &paint_jobs, &screen_descriptor);
    {
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("startup_splash_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 15.0 / 255.0,
                            g: 17.0 / 255.0,
                            b: 23.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
    }
    for id in &egui_output.textures_delta.free {
        egui_renderer.free_texture(id);
    }
    queue.submit([encoder.finish()]);
    frame.present();
    mobile_log(format_args!(
        "startup_splash frame={}",
        fmt_duration(splash_start.elapsed())
    ));
}

fn load_pipeline_cache(path: Option<&PathBuf>) -> Option<Vec<u8>> {
    let path = path?;
    match std::fs::read(path) {
        Ok(bytes) => {
            mobile_log(format_args!(
                "pipeline_cache load ok path={} bytes={}",
                path.display(),
                bytes.len()
            ));
            Some(bytes)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            mobile_log(format_args!(
                "pipeline_cache load miss path={}",
                path.display()
            ));
            None
        }
        Err(err) => {
            mobile_log(format_args!(
                "pipeline_cache load failed path={} error={err}",
                path.display()
            ));
            None
        }
    }
}

fn save_pipeline_cache_once(state: &mut AppState) {
    if !state.pipeline_cache_saved {
        save_pipeline_cache(state, "first_frame");
    }
}

fn save_pipeline_cache(state: &mut AppState, reason: &str) {
    let Some(path) = state.pipeline_cache_path.as_ref() else {
        return;
    };
    let Some(bytes) = state.renderer.pipeline_cache_data() else {
        mobile_log(format_args!(
            "pipeline_cache save skipped reason={reason} unsupported"
        ));
        state.pipeline_cache_saved = true;
        return;
    };

    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        mobile_log(format_args!(
            "pipeline_cache save failed reason={reason} path={} error={err}",
            path.display()
        ));
        return;
    }

    let tmp = path.with_extension("tmp");
    match std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, path)) {
        Ok(()) => {
            state.pipeline_cache_saved = true;
            mobile_log(format_args!(
                "pipeline_cache save ok reason={reason} path={} bytes={}",
                path.display(),
                bytes.len()
            ));
        }
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            mobile_log(format_args!(
                "pipeline_cache save failed reason={reason} path={} error={err}",
                path.display()
            ));
        }
    }
}

fn render_ui(state: &mut AppState, ctx: &egui::Context) -> bool {
    let snapshot = state.model.snapshot();
    let output = mobile_ui::render(ctx, &snapshot);
    state.ui_hit_regions = output.hit_regions;
    let command_redraw = state.apply_ui_commands(output.commands);
    command_redraw || output.redraw_requested
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
            state.touch_starts.insert(id, pos);

            match state.touches.len() {
                1 => {
                    let orbit_pos = orbit_touch_pos(state, pos);
                    state.controller.push_event(ViewportEvent::PointerMoved {
                        position: orbit_pos,
                    });
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
                    state.prev_pinch_dist = Some(touches_distance(&state.touches));
                    state.touch_mode = TouchMode::TwoFingerZoom;
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
                    let orbit_pos = orbit_touch_pos(state, pos);
                    state.controller.push_event(ViewportEvent::PointerMoved {
                        position: orbit_pos,
                    });
                }
                TouchMode::TwoFingerZoom => {
                    let centroid = touches_centroid(&state.touches);
                    state
                        .controller
                        .push_event(ViewportEvent::PointerMoved { position: centroid });

                    let dist = touches_distance(&state.touches);
                    if let Some(prev) = state.prev_pinch_dist {
                        let delta = dist - prev;
                        if delta.abs() > 0.5 {
                            state.controller.push_event(ViewportEvent::Wheel {
                                delta: glam::Vec2::new(0.0, delta * 0.5),
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
            let start_pos = state.touch_starts.remove(&id);
            state.touches.remove(&id);

            match state.touch_mode {
                TouchMode::OneFingerOrbit => {
                    state.controller.push_event(ViewportEvent::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    });
                    state.touch_mode = TouchMode::None;
                    if phase == TouchPhase::Ended
                        && start_pos
                            .map(|start| start.distance(pos) <= TAP_MAX_MOVEMENT)
                            .unwrap_or(false)
                    {
                        handle_tap(state, pos);
                    }
                }
                TouchMode::TwoFingerZoom => {
                    state.prev_pinch_dist = None;

                    state.touch_mode = TouchMode::None;
                }
                TouchMode::None => {}
            }
        }
    }
}

fn handle_tap(state: &mut AppState, pos: glam::Vec2) {
    let now = Instant::now();
    let is_double_tap = state
        .last_tap
        .as_ref()
        .map(|tap| {
            now.duration_since(tap.time) <= DOUBLE_TAP_MAX_INTERVAL
                && tap.pos.distance(pos) <= DOUBLE_TAP_MAX_DISTANCE
        })
        .unwrap_or(false);

    if is_double_tap {
        state.last_tap = None;
        if let Some(plot_index) = pick_plot_at(state, pos) {
            state.apply_ui_commands([UiCommand::OpenPlotProperties(plot_index)]);
        }
    } else {
        state.last_tap = Some(TapRecord { time: now, pos });
    }
}

fn pick_plot_at(state: &AppState, pos: glam::Vec2) -> Option<usize> {
    let viewport_size = glam::vec2(
        state.surface_config.width as f32,
        state.surface_config.height as f32,
    );
    if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
        return None;
    }

    let (ray_origin, ray_dir) = picking::screen_to_ray(
        pos,
        viewport_size,
        state.camera.view_proj_matrix().inverse(),
    );
    let mut best: Option<(f32, usize)> = None;
    for surface in state.scene.probe_data().surfaces {
        let Some(plot_index) = surface.pick_id.checked_sub(1).map(|id| id as usize) else {
            continue;
        };
        for triangle in surface.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (
                surface.positions.get(triangle[0] as usize),
                surface.positions.get(triangle[1] as usize),
                surface.positions.get(triangle[2] as usize),
            ) else {
                continue;
            };
            let a = *a;
            let b = *b;
            let c = *c;
            let a = glam::vec3(a[0], a[1], a[2]);
            let b = glam::vec3(b[0], b[1], b[2]);
            let c = glam::vec3(c[0], c[1], c[2]);
            if let Some(t) = ray_triangle_intersection(ray_origin, ray_dir, a, b, c)
                && best.map(|(best_t, _)| t < best_t).unwrap_or(true)
            {
                best = Some((t, plot_index));
            }
        }
    }

    best.map(|(_, plot_index)| plot_index)
}

fn ray_triangle_intersection(
    origin: glam::Vec3,
    dir: glam::Vec3,
    a: glam::Vec3,
    b: glam::Vec3,
    c: glam::Vec3,
) -> Option<f32> {
    let edge1 = b - a;
    let edge2 = c - a;
    let h = dir.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < 1.0e-6 {
        return None;
    }

    let inv_det = 1.0 / det;
    let s = origin - a;
    let u = inv_det * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = inv_det * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = inv_det * edge2.dot(q);
    (t > 1.0e-4).then_some(t)
}

fn orbit_touch_pos(state: &AppState, pos: glam::Vec2) -> glam::Vec2 {
    glam::vec2(state.surface_config.width as f32 - pos.x, pos.y)
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
    state.touch_starts.clear();
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

pub(crate) fn fmt_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros >= 1_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else {
        format!("{micros}us")
    }
}

pub(crate) fn mobile_log(args: fmt::Arguments<'_>) {
    let message = args.to_string();
    #[cfg(target_os = "android")]
    android_log(&message);
    #[cfg(not(target_os = "android"))]
    eprintln!("[poincare-mobile perf] {message}");
}

fn mobile_lighting_settings() -> LightingSettings {
    #[cfg(target_os = "android")]
    {
        let mut lighting = LightingSettings::default();
        lighting.shadows_enabled = false;
        return lighting;
    }

    #[cfg(not(target_os = "android"))]
    {
        LightingSettings::default()
    }
}

#[cfg(target_os = "android")]
fn android_log(message: &str) {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    const ANDROID_LOG_INFO: c_int = 4;

    unsafe extern "C" {
        fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
    }

    let tag = CString::new("poincare-mobile").expect("static tag has no nul");
    let fmt = CString::new("%s").expect("static format has no nul");
    let message = CString::new(message.replace('\0', "\\0")).expect("nul replaced");
    unsafe {
        __android_log_print(
            ANDROID_LOG_INFO,
            tag.as_ptr(),
            fmt.as_ptr(),
            message.as_ptr(),
        );
    }
}

#[cfg(target_os = "android")]
fn init_rust_logging() {
    use std::sync::Once;

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let filter = std::env::var("RUST_LOG")
            .ok()
            .or_else(|| option_env!("RUST_LOG").map(ToOwned::to_owned))
            .unwrap_or_else(|| "warn".to_owned());
        let env_filter = tracing_subscriber::EnvFilter::try_new(&filter)
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
        let init_result = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(AndroidTraceWriter)
            .with_ansi(false)
            .compact()
            .try_init();
        if init_result.is_ok() {
            mobile_log(format_args!("RUST_LOG={filter}"));
        }
    });
}

#[cfg(target_os = "android")]
#[derive(Clone, Copy)]
struct AndroidTraceWriter;

#[cfg(target_os = "android")]
impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for AndroidTraceWriter {
    type Writer = AndroidTraceLine;

    fn make_writer(&'writer self) -> Self::Writer {
        AndroidTraceLine::default()
    }
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct AndroidTraceLine {
    buffer: Vec<u8>,
}

#[cfg(target_os = "android")]
impl AndroidTraceLine {
    fn emit(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let message = String::from_utf8_lossy(&self.buffer);
        let message = message.trim_end();
        if !message.is_empty() {
            android_log(message);
        }
        self.buffer.clear();
    }
}

#[cfg(target_os = "android")]
impl std::io::Write for AndroidTraceLine {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.emit();
        Ok(())
    }
}

#[cfg(target_os = "android")]
impl Drop for AndroidTraceLine {
    fn drop(&mut self) {
        self.emit();
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

    init_rust_logging();
    let pipeline_cache_path = app
        .internal_data_path()
        .map(|path| path.join("viewport_pipeline_cache.bin"));

    let event_loop = match EventLoop::builder().with_android_app(app).build() {
        Ok(event_loop) => event_loop,
        Err(EventLoopError::RecreationAttempt) => return,
        Err(err) => panic!("event loop: {err}"),
    };
    match event_loop.run_app(&mut App::with_pipeline_cache_path(pipeline_cache_path)) {
        Ok(()) | Err(EventLoopError::RecreationAttempt) => {}
        Err(err) => panic!("run: {err}"),
    }
}
