use eframe::egui;
use poincare_lib::{Domain, Resolution};

use crate::plot::kind::DomainLabels;
use crate::ui::scalar_control::{ScalarControl, edit_scalar_control};

pub(crate) fn edit_domain(ui: &mut egui::Ui, domain: &mut Domain, labels: DomainLabels) -> bool {
    if labels == DomainLabels::None {
        ui.label(
            egui::RichText::new("Fixed construction data. No domain to edit.")
                .weak()
                .small(),
        );
        return false;
    }
    ui.label(egui::RichText::new("Domain").weak().small());
    let mut changed = false;
    match labels {
        DomainLabels::Xy => {
            changed |= edit_range(ui, "X", &mut domain.x);
            changed |= edit_range(ui, "Y", &mut domain.y);
        }
        DomainLabels::Xyz => {
            changed |= edit_range(ui, "X", &mut domain.x);
            changed |= edit_range(ui, "Y", &mut domain.y);
            changed |= edit_range(ui, "Z", &mut domain.z);
        }
        DomainLabels::Uv => {
            changed |= edit_range(ui, "U", &mut domain.x);
            changed |= edit_range(ui, "V", &mut domain.y);
        }
        DomainLabels::ThetaPhi => {
            changed |= edit_range(ui, "θ", &mut domain.x);
            changed |= edit_range(ui, "φ", &mut domain.y);
        }
        DomainLabels::ThetaZ => {
            changed |= edit_range(ui, "θ", &mut domain.x);
            changed |= edit_range(ui, "Z", &mut domain.y);
        }
        DomainLabels::Theta => {
            changed |= edit_range(ui, "θ", &mut domain.x);
        }
        DomainLabels::T => {
            changed |= edit_range(ui, "T", &mut domain.x);
        }
        DomainLabels::SingleVar(label) => {
            changed |= edit_range(ui, &label.to_uppercase(), &mut domain.x);
        }
        DomainLabels::None => unreachable!(),
    }
    changed
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
