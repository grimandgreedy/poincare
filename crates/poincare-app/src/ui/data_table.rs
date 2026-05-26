use eframe::egui;

pub(crate) const DATA_TABLE_PAGE_SIZE: usize = 100;

#[derive(Clone, Default)]
pub(crate) struct DataTableState {
    pub page: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct DataTableOptions<'a> {
    pub id_source: &'a str,
    pub editable: bool,
    pub row_number_offset: usize,
    pub cell_width: f32,
    pub max_height: f32,
}

impl<'a> DataTableOptions<'a> {
    pub(crate) fn editable(id_source: &'a str) -> Self {
        Self {
            id_source,
            editable: true,
            row_number_offset: 1,
            cell_width: 110.0,
            max_height: 360.0,
        }
    }

    pub(crate) fn readonly(id_source: &'a str) -> Self {
        Self {
            id_source,
            editable: false,
            row_number_offset: 1,
            cell_width: 120.0,
            max_height: 360.0,
        }
    }
}

#[derive(Default)]
pub(crate) struct DataTableResponse {
    pub changed: bool,
    pub rows_changed: bool,
}

pub(crate) fn parse_rows(raw_text: &str, delimiter: char) -> Vec<Vec<String>> {
    raw_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            if delimiter == ' ' {
                line.split_whitespace()
                    .map(|cell| cell.trim().to_string())
                    .collect()
            } else {
                line.split(delimiter)
                    .map(|cell| cell.trim().to_string())
                    .collect()
            }
        })
        .collect()
}

pub(crate) fn serialize_rows(rows: &[Vec<String>], delimiter: char) -> String {
    let sep = delimiter.to_string();
    rows.iter()
        .map(|row| row.join(&sep))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn show_data_table(
    ui: &mut egui::Ui,
    state: &mut DataTableState,
    headers: &[String],
    rows: &mut Vec<Vec<String>>,
    options: DataTableOptions<'_>,
) -> DataTableResponse {
    let mut response = DataTableResponse::default();
    let column_count = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(headers.len())
        .max(1);
    for row in rows.iter_mut() {
        row.resize(column_count, String::new());
    }

    let total_rows = rows.len();
    let page_count = total_rows.max(1).div_ceil(DATA_TABLE_PAGE_SIZE);
    state.page = state.page.min(page_count.saturating_sub(1));
    let start_row = state.page * DATA_TABLE_PAGE_SIZE;
    let end_row = (start_row + DATA_TABLE_PAGE_SIZE).min(total_rows);

    ui.horizontal(|ui| {
        ui.label(format!(
            "Rows {}-{} of {}",
            if total_rows == 0 {
                0
            } else {
                start_row + options.row_number_offset
            },
            if total_rows == 0 {
                0
            } else {
                end_row - 1 + options.row_number_offset
            },
            total_rows
        ));
        ui.add_space(10.0);
        if ui
            .add_enabled(state.page > 0, egui::Button::new("Previous 100"))
            .clicked()
        {
            state.page -= 1;
        }
        if ui
            .add_enabled(end_row < total_rows, egui::Button::new("Next 100"))
            .clicked()
        {
            state.page += 1;
        }
        if options.editable {
            ui.add_space(10.0);
            if ui.button("Add Row").clicked() {
                rows.push(vec![String::new(); column_count]);
                let new_total_rows = rows.len();
                state.page = new_total_rows.saturating_sub(1) / DATA_TABLE_PAGE_SIZE;
                response.changed = true;
                response.rows_changed = true;
            }
        }
    });
    ui.add_space(4.0);

    let mut remove_row = None;
    let base_id = ui.id().with(options.id_source);
    egui::ScrollArea::both()
        .id_salt(base_id.with("scroll"))
        .max_height(options.max_height)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            egui::Grid::new(base_id.with("grid"))
                .spacing(egui::vec2(0.0, 0.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("#").strong());
                    for column in 0..column_count {
                        let label = headers
                            .get(column)
                            .cloned()
                            .unwrap_or_else(|| format!("col {}", column + 1));
                        ui.label(egui::RichText::new(label).strong());
                    }
                    if options.editable {
                        ui.label(egui::RichText::new("").strong());
                    }
                    ui.end_row();

                    for row_idx in start_row..end_row {
                        let row = &mut rows[row_idx];
                        ui.label((row_idx + options.row_number_offset).to_string());
                        for cell in row.iter_mut().take(column_count) {
                            if options.editable {
                                let cell_response = ui.add_sized(
                                    [options.cell_width, 24.0],
                                    egui::TextEdit::singleline(cell)
                                        .font(egui::TextStyle::Monospace)
                                        .margin(egui::vec2(4.0, 4.0)),
                                );
                                response.changed |= cell_response.changed();
                            } else {
                                ui.add_sized(
                                    [options.cell_width, 24.0],
                                    egui::Label::new(cell.as_str()).selectable(false),
                                );
                            }
                        }
                        if options.editable {
                            if ui.small_button("X").clicked() {
                                remove_row = Some(row_idx);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    if let Some(row_idx) = remove_row {
        rows.remove(row_idx);
        let new_total_rows = rows.len();
        let new_page_count = new_total_rows.max(1).div_ceil(DATA_TABLE_PAGE_SIZE);
        state.page = state.page.min(new_page_count.saturating_sub(1));
        response.changed = true;
        response.rows_changed = true;
    }

    response
}
