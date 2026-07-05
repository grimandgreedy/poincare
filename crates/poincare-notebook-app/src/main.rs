mod app;
mod cells;
mod dock;
mod evaluator;
mod graph_preview;
mod graph_viewport;
mod persistence;

use app::NotebookApp;

fn main() -> eframe::Result {
    eframe::run_native(
        "Poincare Notebook",
        eframe::NativeOptions {
            // The wgpu backend is required: graphs render through viewport-lib
            // into textures that are displayed as egui images.
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(NotebookApp::new(cc)))),
    )
}
