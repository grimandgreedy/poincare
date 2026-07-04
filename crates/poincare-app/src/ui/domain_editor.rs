use eframe::egui;
use poincare_lib::{Domain, DomainEditorMetadata, Resolution};

use crate::ui::scalar_control::{ScalarControl, edit_scalar_control};

pub(crate) fn edit_domain(
    ui: &mut egui::Ui,
    domain: &mut Domain,
    metadata: DomainEditorMetadata,
) -> bool {
    if metadata == DomainEditorMetadata::Fixed {
        ui.label(
            egui::RichText::new("Fixed construction data. No domain to edit.")
                .weak()
                .small(),
        );
        return false;
    }
    ui.label(egui::RichText::new("Domain").weak().small());
    let mut changed = false;
    match metadata {
        DomainEditorMetadata::Fixed => unreachable!(),
        DomainEditorMetadata::One { primary } => {
            changed |= edit_range(ui, &display_domain_label(&primary), &mut domain.x);
        }
        DomainEditorMetadata::Two { primary, secondary } => {
            changed |= edit_range(ui, &display_domain_label(&primary), &mut domain.x);
            changed |= edit_range(ui, &display_domain_label(&secondary), &mut domain.y);
        }
        DomainEditorMetadata::Three { x, y, z } => {
            changed |= edit_range(ui, &display_domain_label(&x), &mut domain.x);
            changed |= edit_range(ui, &display_domain_label(&y), &mut domain.y);
            changed |= edit_range(ui, &display_domain_label(&z), &mut domain.z);
        }
    }
    changed
}

fn display_domain_label(label: &str) -> String {
    match label {
        "theta" => "θ".to_string(),
        "phi" => "φ".to_string(),
        _ => label.to_uppercase(),
    }
}

pub(crate) fn edit_range(
    ui: &mut egui::Ui,
    label: &str,
    range: &mut std::ops::RangeInclusive<f64>,
) -> bool {
    let mut start = *range.start();
    let mut end = *range.end();
    let span = (end - start).abs();
    let mut step = (span / 100.0).max(0.1);
    let resp = edit_scalar_control(
        ui,
        ("domain_range", label),
        ScalarControl {
            label,
            framed: false,
            value: None,
            min: &mut start,
            max: &mut end,
            step: Some(&mut step),
            speed: None,
            playing: None,
            reset_label: None,
        },
    );
    let changed = resp.changed;
    if changed {
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        *range = start..=end;
    }
    changed
}

pub(crate) fn edit_resolution(
    ui: &mut egui::Ui,
    resolution: &mut Resolution,
    enabled: bool,
) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        let mut samples = resolution.u.max(resolution.v).clamp(2, 512);
        changed |= ui
            .add(egui::Slider::new(&mut samples, 2..=512).text("Samples"))
            .changed();
        if changed {
            resolution.u = samples;
            resolution.v = samples;
        }
    });
    if !enabled {
        ui.label("This plot ignores resolution overrides.");
    }
    changed
}

/// Truncate `s` to at most `max_chars` Unicode characters, including the "…" if cut.
pub(crate) fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let truncated: String = s.chars().take(max_chars - 1).collect();
    format!("{truncated}…")
}
