//! Startup launcher screen.
//!
//! When the app opens it shows a welcome screen (Mathematica-style) offering to
//! create a new notebook or reopen a recent one, instead of dropping straight
//! into an empty document. Recent notebooks are tracked here and persisted
//! through eframe storage.

use std::path::PathBuf;

use eframe::egui;
use serde::{Deserialize, Serialize};

/// Storage key for the persisted recent-notebook list.
pub(crate) const RECENTS_STORAGE_KEY: &str = "poincare_recent_notebooks";

/// Maximum number of recent notebooks to remember.
const MAX_RECENTS: usize = 8;

/// A previously opened or saved notebook, shown on the launcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RecentNotebook {
    pub path: PathBuf,
    pub title: String,
}

/// Record a notebook as most-recently-used: move it to the front, de-duplicate
/// by path, and cap the list length.
pub(crate) fn push_recent(recents: &mut Vec<RecentNotebook>, path: PathBuf, title: String) {
    recents.retain(|entry| entry.path != path);
    recents.insert(0, RecentNotebook { path, title });
    recents.truncate(MAX_RECENTS);
}

/// What the user chose on the launcher screen.
pub(crate) enum LauncherAction {
    NewNotebook,
    OpenDialog,
    OpenRecent(PathBuf),
    RemoveRecent(PathBuf),
}

/// Draw the launcher and return an action if the user made a choice this frame.
pub(crate) fn show(
    ctx: &egui::Context,
    recents: &[RecentNotebook],
    error: Option<&str>,
) -> Option<LauncherAction> {
    let mut action = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(56.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(560.0);

            ui.label(
                egui::RichText::new("Poincaré Notebook")
                    .size(34.0)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Create a new notebook or reopen a recent one.")
                    .size(15.0)
                    .color(ui.visuals().weak_text_color()),
            );

            if let Some(error) = error {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(error)
                        .color(ui.visuals().error_fg_color)
                        .small(),
                );
            }

            ui.add_space(28.0);

            let accent = egui::Color32::from_rgb(100, 200, 140);
            ui.horizontal(|ui| {
                // Center the two primary actions within the fixed-width column.
                let button_w = 190.0;
                let spacing = 16.0;
                let pad = ((ui.available_width() - button_w * 2.0 - spacing) / 2.0).max(0.0);
                ui.add_space(pad);
                if ui
                    .add_sized(
                        [button_w, 44.0],
                        egui::Button::new(
                            egui::RichText::new("＋  New Notebook")
                                .size(16.0)
                                .color(egui::Color32::BLACK),
                        )
                        .fill(accent),
                    )
                    .clicked()
                {
                    action = Some(LauncherAction::NewNotebook);
                }
                ui.add_space(spacing);
                if ui
                    .add_sized(
                        [button_w, 44.0],
                        egui::Button::new(egui::RichText::new("Open Notebook…").size(16.0)),
                    )
                    .clicked()
                {
                    action = Some(LauncherAction::OpenDialog);
                }
            });

            ui.add_space(30.0);
            ui.separator();
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Recent").size(15.0).strong());
            });
            ui.add_space(8.0);

            if recents.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("No recent notebooks yet.")
                        .color(ui.visuals().weak_text_color()),
                );
            } else {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for entry in recents {
                            if let Some(card_action) = recent_card(ui, entry) {
                                action = Some(card_action);
                            }
                            ui.add_space(6.0);
                        }
                    });
            }
        });
    });

    action
}

/// One recent-notebook card: a clickable panel showing the title and path, with
/// a remove (✕) button. Returns an action if either was clicked.
fn recent_card(ui: &mut egui::Ui, entry: &RecentNotebook) -> Option<LauncherAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        let remove_w = 28.0;
        let card_h = 46.0;
        let card_w = (ui.available_width() - remove_w - 6.0).max(0.0);

        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
        let hovered = response.hovered();

        let fill = if hovered {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            ui.visuals().faint_bg_color
        };
        let strong_color = ui.visuals().strong_text_color();
        let weak_color = ui.visuals().weak_text_color();
        // Clone the painter so it does not hold an immutable borrow of `ui`
        // across the mutable `ui.add_sized` call for the remove button below.
        let painter = ui.painter().clone();
        painter.rect_filled(rect, egui::CornerRadius::same(8), fill);

        let title = if entry.title.trim().is_empty() {
            file_stem(&entry.path)
        } else {
            entry.title.clone()
        };
        painter.text(
            rect.left_top() + egui::vec2(14.0, 8.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(16.0),
            strong_color,
        );
        painter.text(
            rect.left_top() + egui::vec2(14.0, 27.0),
            egui::Align2::LEFT_TOP,
            elide_path(&entry.path, 68),
            egui::FontId::proportional(11.5),
            weak_color,
        );

        if response.clicked() {
            action = Some(LauncherAction::OpenRecent(entry.path.clone()));
        }
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        response.on_hover_text(entry.path.display().to_string());

        if ui
            .add_sized(
                [remove_w, card_h],
                egui::Button::new("✕").fill(egui::Color32::TRANSPARENT),
            )
            .on_hover_text("Remove from recent")
            .clicked()
        {
            action = Some(LauncherAction::RemoveRecent(entry.path.clone()));
        }
    });

    action
}

fn file_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// Shorten a path from the left with an ellipsis so long paths stay readable.
fn elide_path(path: &std::path::Path, max_chars: usize) -> String {
    let text = path.display().to_string();
    let count = text.chars().count();
    if count <= max_chars {
        return text;
    }
    let tail: String = text
        .chars()
        .skip(count.saturating_sub(max_chars.saturating_sub(1)))
        .collect();
    format!("…{tail}")
}
