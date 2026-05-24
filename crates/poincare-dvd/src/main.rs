use eframe::egui;
use poincare_lib::{
    AxisConfig, ColourMode, Domain, GraphScene, GraphSpec, PlotDefinition, PlotSpec, PlotStyle,
    Resolution,
};
use viewport_lib::{Camera, ViewportRenderer};

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Poincare DVD",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
            depth_buffer: 24,
            stencil_buffer: 8,
            ..Default::default()
        },
        Box::new(|cc| {
            let wgpu_state = cc
                .wgpu_render_state
                .as_ref()
                .expect("eframe wgpu backend required");

            let renderer = ViewportRenderer::new(&wgpu_state.device, wgpu_state.target_format);
            {
                let mut guard = wgpu_state.renderer.write();
                guard.callback_resources.insert(renderer);
            }

            Ok(Box::new(DvdApp::new(cc)?))
        }),
    )
}

struct DvdApp {
    scene: GraphScene,
    camera: Camera,
    viewport_pos: egui::Vec2,
    viewport_velocity: egui::Vec2,
    initialized_viewport: bool,
    orbit_phase: f32,
}

impl DvdApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let mut scene = build_demo_spec()
            .build_scene()
            .map_err(|err| format!("Failed to build demo scene: {err}"))?;

        let wgpu_state = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| "wgpu render state missing".to_string())?;

        {
            let mut renderer_guard = wgpu_state.renderer.write();
            let renderer = renderer_guard
                .callback_resources
                .get_mut::<ViewportRenderer>()
                .ok_or_else(|| "viewport renderer missing".to_string())?;
            scene.upload_meshes(
                &wgpu_state.device,
                &wgpu_state.queue,
                renderer.resources_mut(),
            )
            .map_err(|err| format!("Failed to upload meshes: {err}"))?;
        }

        Ok(Self {
            scene,
            camera: default_camera(),
            viewport_pos: egui::vec2(32.0, 32.0),
            viewport_velocity: egui::vec2(180.0, 140.0),
            initialized_viewport: false,
            orbit_phase: 0.0,
        })
    }

    fn tick_viewport(&mut self, bounds: egui::Rect, dt: f32, viewport_size: egui::Vec2) -> egui::Rect {
        let max_x = (bounds.width() - viewport_size.x).max(0.0);
        let max_y = (bounds.height() - viewport_size.y).max(0.0);

        if !self.initialized_viewport {
            self.viewport_pos = egui::vec2(max_x * 0.5, max_y * 0.4);
            self.initialized_viewport = true;
        }

        self.viewport_pos += self.viewport_velocity * dt;

        if self.viewport_pos.x <= 0.0 {
            self.viewport_pos.x = 0.0;
            self.viewport_velocity.x = self.viewport_velocity.x.abs();
        } else if self.viewport_pos.x >= max_x {
            self.viewport_pos.x = max_x;
            self.viewport_velocity.x = -self.viewport_velocity.x.abs();
        }

        if self.viewport_pos.y <= 0.0 {
            self.viewport_pos.y = 0.0;
            self.viewport_velocity.y = self.viewport_velocity.y.abs();
        } else if self.viewport_pos.y >= max_y {
            self.viewport_pos.y = max_y;
            self.viewport_velocity.y = -self.viewport_velocity.y.abs();
        }

        egui::Rect::from_min_size(bounds.min + self.viewport_pos, viewport_size)
    }

    fn tick_camera(&mut self, dt: f32) {
        self.orbit_phase += dt * 0.35;
        self.camera.orientation = glam::Quat::from_rotation_z(self.orbit_phase * 0.65)
            * glam::Quat::from_rotation_x(1.05 + 0.12 * self.orbit_phase.sin());
    }
}

impl eframe::App for DvdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|input| input.stable_dt).max(1.0 / 240.0);
        self.tick_camera(dt);
        ctx.request_repaint();

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(10, 10, 14)))
            .show(ctx, |ui| {
                let bounds = ui.max_rect();
                let viewport_size = egui::vec2(bounds.width() / 4.0, bounds.height() / 4.0);
                let viewport_rect = self.tick_viewport(bounds, dt, viewport_size);

                ui.painter().rect_filled(
                    viewport_rect.expand(6.0),
                    10.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                );
                ui.painter().rect_stroke(
                    viewport_rect.expand(6.0),
                    10.0,
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(245, 245, 245)),
                    egui::StrokeKind::Outside,
                );

                let mut frame_data = self.scene.build_frame(&self.camera);
                frame_data.camera.viewport_size = [viewport_rect.width(), viewport_rect.height()];
                frame_data.camera.pixels_per_point = ctx.pixels_per_point();
                frame_data.viewport.background_colour = Some([0.03, 0.03, 0.05, 1.0]);
                frame_data.viewport.show_grid = true;

                ui.painter().add(eframe::egui_wgpu::Callback::new_paint_callback(
                    viewport_rect,
                    ViewportCallback { frame: frame_data },
                ));

                let label_rect = viewport_rect.expand(6.0);
                ui.painter().text(
                    label_rect.left_top() + egui::vec2(10.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    "POINCARE DVD",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgb(240, 240, 240),
                );
            });
    }
}

struct ViewportCallback {
    frame: viewport_lib::FrameData,
}

impl eframe::egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &eframe::wgpu::Device,
        queue: &eframe::wgpu::Queue,
        _screen_descriptor: &eframe::egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut eframe::wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<eframe::wgpu::CommandBuffer> {
        if let Some(renderer) = callback_resources.get_mut::<ViewportRenderer>() {
            renderer.prepare(device, queue, &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut eframe::wgpu::RenderPass<'static>,
        callback_resources: &eframe::egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<ViewportRenderer>() {
            renderer.paint(render_pass, &self.frame);
        }
    }
}

fn build_demo_spec() -> GraphSpec {
    GraphSpec {
        axis_config: AxisConfig {
            show_box: true,
            show_labels: true,
            show_ticks: true,
            show_grid: true,
            ..AxisConfig::default()
        },
        plots: vec![
            PlotSpec {
                name: "z = sin(x*y)".to_string(),
                visible: true,
                domain: Domain {
                    x: -4.0..=4.0,
                    y: -4.0..=4.0,
                    z: -4.0..=4.0,
                },
                resolution: Resolution { u: 120, v: 120 },
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([0.20, 0.65, 1.0, 0.95]),
                    two_sided: true,
                    ..PlotStyle::default()
                },
                definition: PlotDefinition::ExprCartesian {
                    expression: "sin(y*x)".to_string(),
                    parameters: Vec::new(),
                },
            },
            PlotSpec {
                name: "r = theta*phi".to_string(),
                visible: true,
                domain: Domain {
                    x: 0.0..=std::f64::consts::PI,
                    y: 0.0..=std::f64::consts::TAU,
                    z: -20.0..=20.0,
                },
                resolution: Resolution { u: 80, v: 80 },
                style: PlotStyle {
                    colour_mode: ColourMode::Solid([1.0, 0.52, 0.22, 0.42]),
                    opacity: 0.42,
                    two_sided: true,
                    ..PlotStyle::default()
                },
                definition: PlotDefinition::ExprSpherical {
                    expression: "theta * phi".to_string(),
                    parameters: Vec::new(),
                },
            },
        ],
    }
}

fn default_camera() -> Camera {
    Camera {
        center: glam::Vec3::ZERO,
        distance: 42.0,
        orientation: glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)
            * glam::Quat::from_rotation_x(1.05),
        ..Camera::default()
    }
}
