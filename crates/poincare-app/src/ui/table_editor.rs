use eframe::egui;

use crate::plot::table::{OptionalColumn, TableColumnMapping, TableImportDefinition};

pub(crate) fn edit_table_import(ui: &mut egui::Ui, definition: &mut TableImportDefinition) -> bool {
    let mut dirty = false;

    ui.horizontal(|ui| {
        if ui.button("Load CSV...").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Delimited text", &["csv", "tsv", "txt", "dat"])
                .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    definition.raw_text = contents;
                    definition.source_path = Some(path.display().to_string());
                    definition.auto_configure();
                    dirty = true;
                }
                Err(_) => {}
            }
        }
        if ui.button("Auto Detect").clicked() {
            definition.auto_configure();
            dirty = true;
        }
        if let Some(path) = &definition.source_path
            && ui.button("Refresh From File").clicked()
            && let Ok(contents) = std::fs::read_to_string(path)
        {
            definition.raw_text = contents;
            dirty = true;
        }
    });

    ui.label(
        egui::RichText::new(definition.source_summary())
            .small()
            .weak(),
    );
    ui.horizontal(|ui| {
        egui::ComboBox::from_label("Delimiter")
            .selected_text(definition.delimiter.label())
            .show_ui(ui, |ui| {
                for delimiter in crate::plot::table::TableDelimiter::ALL {
                    dirty |= ui
                        .selectable_value(&mut definition.delimiter, delimiter, delimiter.label())
                        .changed();
                }
            });
        dirty |= ui
            .checkbox(&mut definition.header_row, "Header row")
            .changed();
    });

    let preview = definition.preview();
    ui.label(format!(
        "{} columns, {} data rows",
        preview.column_count,
        preview.rows.len()
    ));

    edit_mapping(
        ui,
        definition,
        &preview.headers,
        preview.column_count,
        &mut dirty,
    );

    ui.add_space(6.0);
    ui.label("Source Data");
    dirty |= ui
        .add(
            egui::TextEdit::multiline(&mut definition.raw_text)
                .font(egui::TextStyle::Monospace)
                .desired_rows(6),
        )
        .changed();

    ui.add_space(6.0);
    ui.label("Preview");
    egui::ScrollArea::both().max_height(180.0).show(ui, |ui| {
        egui::Grid::new(ui.id().with("table_preview"))
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("#").strong());
                for header in &preview.headers {
                    ui.label(egui::RichText::new(header).strong());
                }
                ui.end_row();
                for row in preview.rows.iter().take(8) {
                    ui.label(row.source_row.to_string());
                    for column in 0..preview.column_count {
                        ui.label(row.cells.get(column).map(String::as_str).unwrap_or(""));
                    }
                    ui.end_row();
                }
            });
    });

    match definition.validate() {
        Ok(_) => {
            ui.colored_label(egui::Color32::from_rgb(120, 210, 150), "Import is valid");
        }
        Err(errors) => {
            for error in errors.iter().take(5) {
                ui.colored_label(egui::Color32::from_rgb(255, 110, 110), error.display());
            }
            if errors.len() > 5 {
                ui.label(format!("{} more validation errors", errors.len() - 5));
            }
        }
    }

    dirty
}

fn edit_mapping(
    ui: &mut egui::Ui,
    definition: &mut TableImportDefinition,
    headers: &[String],
    column_count: usize,
    dirty: &mut bool,
) {
    ui.separator();
    ui.label(format!("Column Mapping for {}", definition.target.label()));
    match &mut definition.mapping {
        TableColumnMapping::SurfaceGrid { x, y, z } => {
            required_column_combo(ui, "x", x, headers, column_count, dirty);
            required_column_combo(ui, "y", y, headers, column_count, dirty);
            required_column_combo(ui, "z", z, headers, column_count, dirty);
        }
        TableColumnMapping::Curve {
            x,
            y,
            z,
            label,
            group,
        } => {
            required_column_combo(ui, "x", x, headers, column_count, dirty);
            required_column_combo(ui, "y", y, headers, column_count, dirty);
            optional_column_combo(ui, "z", z, headers, column_count, dirty);
            optional_column_combo(ui, "label", label, headers, column_count, dirty);
            optional_column_combo(ui, "group", group, headers, column_count, dirty);
        }
        TableColumnMapping::Scatter {
            x,
            y,
            z,
            scalar,
            label,
            group,
        } => {
            required_column_combo(ui, "x", x, headers, column_count, dirty);
            required_column_combo(ui, "y", y, headers, column_count, dirty);
            optional_column_combo(ui, "z", z, headers, column_count, dirty);
            optional_column_combo(ui, "scalar", scalar, headers, column_count, dirty);
            optional_column_combo(ui, "label", label, headers, column_count, dirty);
            optional_column_combo(ui, "group", group, headers, column_count, dirty);
        }
        TableColumnMapping::VectorField {
            x,
            y,
            z,
            vx,
            vy,
            vz,
            scalar,
            label,
            group,
        } => {
            required_column_combo(ui, "x", x, headers, column_count, dirty);
            required_column_combo(ui, "y", y, headers, column_count, dirty);
            optional_column_combo(ui, "z", z, headers, column_count, dirty);
            required_column_combo(ui, "vx", vx, headers, column_count, dirty);
            required_column_combo(ui, "vy", vy, headers, column_count, dirty);
            optional_column_combo(ui, "vz", vz, headers, column_count, dirty);
            optional_column_combo(ui, "scalar", scalar, headers, column_count, dirty);
            optional_column_combo(ui, "label", label, headers, column_count, dirty);
            optional_column_combo(ui, "group", group, headers, column_count, dirty);
        }
    }
}

fn required_column_combo(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut usize,
    headers: &[String],
    column_count: usize,
    dirty: &mut bool,
) {
    egui::ComboBox::from_label(label)
        .selected_text(column_label(*value, headers))
        .show_ui(ui, |ui| {
            for index in 0..column_count.max(headers.len()) {
                *dirty |= ui
                    .selectable_value(value, index, column_label(index, headers))
                    .changed();
            }
        });
}

fn optional_column_combo(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut OptionalColumn,
    headers: &[String],
    column_count: usize,
    dirty: &mut bool,
) {
    egui::ComboBox::from_label(label)
        .selected_text(match value {
            OptionalColumn::None => "None".to_string(),
            OptionalColumn::Column(index) => column_label(*index, headers),
        })
        .show_ui(ui, |ui| {
            *dirty |= ui
                .selectable_value(value, OptionalColumn::None, "None")
                .changed();
            for index in 0..column_count.max(headers.len()) {
                *dirty |= ui
                    .selectable_value(
                        value,
                        OptionalColumn::Column(index),
                        column_label(index, headers),
                    )
                    .changed();
            }
        });
}

fn column_label(index: usize, headers: &[String]) -> String {
    headers
        .get(index)
        .cloned()
        .unwrap_or_else(|| format!("Column {}", index + 1))
}
