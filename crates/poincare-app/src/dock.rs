use eframe::egui;
use grimdock::{
    ChildSide, DropPolicy, HeaderVisibility, Node, PaneOptions, PanelContext, PanelTree, SplitDir,
    Tab,
};
use grimdock::TabStyleOverride;

use crate::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DockTab {
    Plots,
    Viewport,
    PlotProperties,
    Export,
}

fn tab(title: &str, id: DockTab) -> Tab<DockTab> {
    let (icon, color) = match id {
        DockTab::Plots         => ("☰", egui::Color32::from_rgb(110, 160, 220)), // soft blue
        DockTab::Viewport      => ("⬡", egui::Color32::from_rgb(100, 200, 140)), // soft green
        DockTab::PlotProperties => ("⚙", egui::Color32::from_rgb(210, 150,  80)), // amber
        DockTab::Export        => ("↑", egui::Color32::from_rgb(180, 120, 210)), // lavender
    };
    Tab::new(title, id)
        .with_leading_visual(icon)
        .with_style_override(TabStyleOverride { icon_color: Some(color), ..TabStyleOverride::none() })
}

pub(crate) fn build_panel_tree() -> PanelTree<DockTab> {
    let mut tree = PanelTree::new(vec![
        tab("Viewport", DockTab::Viewport).with_draggable(false),
    ]);
    tree.split_leaf(
        0,
        SplitDir::Horizontal,
        tab("Plots", DockTab::Plots),
        ChildSide::First,
    );
    tree.split_leaf(
        2,
        SplitDir::Horizontal,
        tab("Plot", DockTab::PlotProperties),
        ChildSide::Second,
    );
    tree.insert_tab_into_leaf(6, tab("Export", DockTab::Export));

    if let Node::Split { ratio, .. } = tree.node_mut(0) {
        *ratio = 0.24;
    }
    if let Node::Split { ratio, .. } = tree.node_mut(2) {
        *ratio = 0.68;
    }
    tree.set_pane_options(
        5,
        PaneOptions {
            header_visibility: HeaderVisibility::Hidden,
            allow_collapse: false,
            allow_tab_reorder: false,
            allow_tab_drag_out: false,
            allow_resize: true,
            drop_policy: DropPolicy::none(),
            lock_layout: false,
            paint_content_bg: false,
            ..PaneOptions::default()
        },
    );

    tree
}

impl App {
    pub(crate) fn focus_plot_tab(&mut self) {
        self.pending_focus_tab = Some(DockTab::PlotProperties);
    }

    pub(crate) fn dock_ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut panel_tree = self.panel_tree.take().unwrap_or_else(build_panel_tree);
        let panel_style = self.panel_style.clone();
        let output = PanelContext::new(ui, &mut panel_tree, &panel_style).show(|ui, tab| {
            self.render_dock_tab(ui, frame, *tab);
        });

        if let Some(tab) = self.pending_focus_tab.take() {
            panel_tree.focus_tab(&tab);
        }

        self.panel_tree = Some(panel_tree);

        if !output.closed_tabs.is_empty() {
            for tab in output.closed_tabs {
                let (title, target_leaf) = match tab {
                    DockTab::Plots => ("Plots", 1),
                    DockTab::Viewport => ("Viewport", 5),
                    DockTab::PlotProperties => ("Plot", 6),
                    DockTab::Export => ("Export", 6),
                };
                self.panel_tree
                    .as_mut()
                    .expect("panel tree restored after dock render")
                    .ensure_tab_in_leaf(target_leaf, crate::dock::tab(title, tab).with_closable(false));
            }
        }
    }

    fn render_dock_tab(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame, tab: DockTab) {
        match tab {
            DockTab::Plots => {
                self.left_panel(ui);
            }
            DockTab::Viewport => {
                let ctx = ui.ctx().clone();
                self.viewport(&ctx, ui);
            }
            DockTab::PlotProperties => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.plot_properties_panel(ui);
                });
            }
            DockTab::Export => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.export_panel(ui, frame);
                });
            }
        }
    }
}
