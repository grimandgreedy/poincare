mod app;
mod cells;
mod dock;
mod evaluator;
mod graph_preview;
mod persistence;

use app::NotebookApp;

fn main() -> eframe::Result {
    eframe::run_native(
        "Poincare Notebook",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(NotebookApp::new(cc)))),
    )
}
