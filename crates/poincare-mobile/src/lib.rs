use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::Arc;

use poincare_lib::{
    AxisConfig, ColormapSource, ColourMode, Domain, GraphScene, GraphSpec, MatcapSource,
    ParamVisSettings, PlotDefinition, PlotSpec, PlotStyle, Resolution, ShadingMode,
};
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
use winit::keyboard::{Key, NamedKey};
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
}

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: ViewportRenderer,
    camera: Camera,
    controller: OrbitCameraController,
    presets: Vec<PlotSpec>,
    active_plot: usize,
    scene: GraphScene,
    scene_error: Option<String>,
    touches: HashMap<u64, glam::Vec2>,
    touch_mode: TouchMode,
    prev_pinch_dist: Option<f32>,
}

impl AppState {
    fn rebuild_scene(&mut self) {
        let spec = GraphSpec {
            axis_config: AxisConfig::default(),
            plots: vec![self.presets[self.active_plot].clone()],
        };

        match spec.build_scene() {
            Ok(mut scene) => {
                match scene.upload_meshes(&self.device, &self.queue, self.renderer.resources_mut())
                {
                    Ok(()) => {
                        self.scene = scene;
                        self.scene_error = None;
                    }
                    Err(err) => {
                        self.scene_error = Some(format!("mesh upload failed: {err}"));
                    }
                }
            }
            Err(err) => {
                self.scene_error = Some(err.to_string());
            }
        }
    }

    fn cycle_plot(&mut self) {
        if self.presets.is_empty() {
            return;
        }
        self.active_plot = (self.active_plot + 1) % self.presets.len();
        self.rebuild_scene();
        self.window.request_redraw();
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
            camera,
            controller,
            presets: preset_plots(),
            active_plot: 0,
            scene: GraphScene::new(),
            scene_error: None,
            touches: HashMap::new(),
            touch_mode: TouchMode::None,
            prev_pinch_dist: None,
        };
        state.rebuild_scene();
        state.window.request_redraw();
        self.state = Some(state);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
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
                handle_touch(state, phase, id, pos);
                state.window.request_redraw();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                handle_keyboard(state, event);
            }

            WindowEvent::RedrawRequested => render(state),

            _ => {}
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
    frame_data.viewport.show_grid = true;
    frame_data.viewport.show_axes_indicator = true;
    frame_data.viewport.background_colour = Some([0.06, 0.06, 0.07, 1.0]);
    frame_data.effects.lighting = LightingSettings::default();
    frame_data.effects.ground_plane = GroundPlane {
        mode: GroundPlaneMode::Tile,
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

    let cmd = state
        .renderer
        .owned()
        .render(&state.device, &state.queue, &view, &frame_data);
    state.queue.submit(std::iter::once(cmd));
    frame.present();

    state.controller.begin_frame(ViewportContext {
        hovered: true,
        focused: true,
        viewport_size: [w, h],
    });
}

fn handle_keyboard(state: &mut AppState, event: KeyEvent) {
    if event.state != ElementState::Pressed || event.repeat {
        return;
    }
    match event.logical_key {
        Key::Named(NamedKey::Space) | Key::Named(NamedKey::ArrowRight) => state.cycle_plot(),
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
                    state.cycle_plot();
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

fn preset_plots() -> Vec<PlotSpec> {
    vec![
        PlotSpec {
            name: "Torus".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: 0.0..=(2.0 * PI),
                z: -1.5..=1.5,
            },
            resolution: Resolution { u: 80, v: 40 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(viewport_lib::BuiltinColourmap::Viridis),
                    scalar_range: None,
                },
                two_sided: true,
                shading: ShadingMode::Smooth,
                matcap: Some(MatcapSource::Builtin(viewport_lib::BuiltinMatcap::Clay)),
                ..PlotStyle::default()
            },
            definition: PlotDefinition::ExprParametricSurface {
                expression: "(2+cos(v))*cos(u)|(2+cos(v))*sin(u)|sin(v)".to_string(),
                parameters: Vec::new(),
            },
        },
        PlotSpec {
            name: "Mobius Strip".to_string(),
            visible: true,
            domain: Domain {
                x: 0.0..=(2.0 * PI),
                y: -1.0..=1.0,
                z: -0.5..=0.5,
            },
            resolution: Resolution { u: 100, v: 20 },
            style: PlotStyle {
                colour_mode: ColourMode::Solid([0.8, 0.5, 1.0, 1.0]),
                two_sided: true,
                shading: ShadingMode::Smooth,
                param_vis: Some(ParamVisSettings {
                    mode: viewport_lib::ParamVisMode::Checker,
                    scale: 12.0,
                }),
                ..PlotStyle::default()
            },
            definition: PlotDefinition::ExprParametricSurface {
                expression: "(1+v/2*cos(u/2))*cos(u)|(1+v/2*cos(u/2))*sin(u)|v/2*sin(u/2)"
                    .to_string(),
                parameters: Vec::new(),
            },
        },
        PlotSpec {
            name: "Monkey Saddle".to_string(),
            visible: true,
            domain: Domain {
                x: -2.0..=2.0,
                y: -2.0..=2.0,
                z: -8.0..=8.0,
            },
            resolution: Resolution { u: 80, v: 80 },
            style: PlotStyle {
                colour_mode: ColourMode::Colormap {
                    colormap: ColormapSource::Builtin(viewport_lib::BuiltinColourmap::Plasma),
                    scalar_range: None,
                },
                two_sided: true,
                ..PlotStyle::default()
            },
            definition: PlotDefinition::ExprCartesian {
                expression: "x^3-3*x*y^2".to_string(),
                parameters: Vec::new(),
            },
        },
    ]
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
