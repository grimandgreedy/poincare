use eframe::egui;
use grimdock::TabStyleOverride;
use grimdock::{
    ChildSide, DropPolicy, Node, PaneOptions, PanelContext, PanelStyle, PanelTree, SplitDir, Tab,
};

use crate::app::NotebookApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DockTab {
    Notebook,
    Session,
}

pub(crate) fn default_panel_style() -> PanelStyle {
    PanelStyle {
        content_inset: 10.0,
        ..PanelStyle::default()
    }
}

pub(crate) fn tab(title: &str, id: DockTab) -> Tab<DockTab> {
    let (icon, color, max_width) = match id {
        DockTab::Notebook => ("N", egui::Color32::from_rgb(100, 200, 140), Some(140.0)),
        DockTab::Session => ("S", egui::Color32::from_rgb(110, 160, 220), Some(140.0)),
    };
    Tab::new(title, id)
        .with_leading_visual(icon)
        .with_style_override(TabStyleOverride {
            icon_color: Some(color),
            max_width,
            ..TabStyleOverride::none()
        })
}

pub(crate) fn build_panel_tree(show_session: bool) -> PanelTree<DockTab> {
    let mut tree = PanelTree::new(vec![
        tab("Notebook", DockTab::Notebook)
            .with_closable(false)
            .with_draggable(false),
    ]);

    if show_session {
        tree.split_leaf(
            0,
            SplitDir::Horizontal,
            tab("Session", DockTab::Session).with_closable(false),
            ChildSide::Second,
        );
        if let Node::Split { ratio, .. } = tree.node_mut(0) {
            *ratio = 0.76;
        }
    }

    configure_tree(&mut tree);
    tree.focus_tab(&DockTab::Notebook);
    tree
}

pub(crate) fn sync_session_tab(tree: &mut PanelTree<DockTab>, show_session: bool) {
    if show_session {
        tree.ensure_tab_in_leaf(2, tab("Session", DockTab::Session).with_closable(false));
    } else {
        tree.remove_tab(&DockTab::Session);
    }
}

fn configure_tree(tree: &mut PanelTree<DockTab>) {
    tree.set_pane_options(
        1,
        PaneOptions {
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
    tree.set_pane_options(
        2,
        PaneOptions {
            allow_collapse: true,
            allow_tab_reorder: false,
            allow_tab_drag_out: false,
            allow_resize: true,
            drop_policy: DropPolicy::none(),
            lock_layout: false,
            paint_content_bg: true,
            ..PaneOptions::default()
        },
    );
}

impl NotebookApp {
    pub(crate) fn dock_ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut panel_tree = self
            .panel_tree
            .take()
            .unwrap_or_else(|| build_panel_tree(self.show_side_panel));
        sync_session_tab(&mut panel_tree, self.show_side_panel);

        let panel_style = self.panel_style.clone();
        let output = PanelContext::new(ui, &mut panel_tree, &panel_style).show(|ui, tab| {
            self.render_dock_tab(ui, frame, *tab);
        });

        if let Some(tab) = self.pending_focus_tab.take() {
            panel_tree.focus_tab(&tab);
        }

        if output.closed_tabs.contains(&DockTab::Session) {
            self.show_side_panel = false;
            panel_tree.remove_tab(&DockTab::Session);
        }

        self.panel_tree = Some(panel_tree);
    }

    fn render_dock_tab(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame, tab: DockTab) {
        match tab {
            DockTab::Notebook => self.show_document_ui(ui),
            DockTab::Session => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_session_panel_ui(ui);
                });
            }
        }
    }
}
