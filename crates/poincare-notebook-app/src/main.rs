mod app;
mod cells;
mod dock;
mod evaluator;
mod graph_preview;
mod graph_viewport;
mod persistence;

use app::NotebookApp;
use viewport_lib::ViewportRenderer;

fn main() -> eframe::Result {
    eframe::run_native(
        "Poincare Notebook",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
            // A depth/stencil buffer is required for the embedded 3D graph
            // viewports rendered through viewport-lib.
            depth_buffer: 24,
            stencil_buffer: 8,
            ..Default::default()
        },
        Box::new(|cc| {
            // Create the shared 3D renderer once and hand it to egui's wgpu
            // backend so graph paint callbacks and headless renders can use it.
            let wgpu_state = cc
                .wgpu_render_state
                .as_ref()
                .expect("eframe wgpu backend required for graph rendering");
            let renderer =
                ViewportRenderer::new(&wgpu_state.device, wgpu_state.target_format);
            wgpu_state
                .renderer
                .write()
                .callback_resources
                .insert(renderer);
            Ok(Box::new(NotebookApp::new(cc)))
        }),
    )
}
