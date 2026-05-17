use std::collections::HashMap;

use poincare_lib::{
    CoordinateSystem, DataBounds, Domain, GlyphInstance, PiecewisePlot, PlotGeometry, PlotObject,
    PlotStyle, Resolution,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TableDelimiter {
    Comma,
    Semicolon,
    Tab,
    Space,
    Pipe,
}

impl TableDelimiter {
    pub(crate) const ALL: [Self; 5] = [
        Self::Comma,
        Self::Semicolon,
        Self::Tab,
        Self::Space,
        Self::Pipe,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Comma => "Comma",
            Self::Semicolon => "Semicolon",
            Self::Tab => "Tab",
            Self::Space => "Space",
            Self::Pipe => "Pipe",
        }
    }

    fn split_line(self, line: &str) -> Vec<String> {
        match self {
            Self::Comma => line.split(',').map(|cell| cell.trim().to_string()).collect(),
            Self::Semicolon => line.split(';').map(|cell| cell.trim().to_string()).collect(),
            Self::Tab => line.split('\t').map(|cell| cell.trim().to_string()).collect(),
            Self::Space => line
                .split_whitespace()
                .map(|cell| cell.trim().to_string())
                .collect(),
            Self::Pipe => line.split('|').map(|cell| cell.trim().to_string()).collect(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TablePlotTarget {
    SurfaceGrid,
    Curve,
    Scatter,
    VectorField,
}

impl TablePlotTarget {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SurfaceGrid => "Surface Grid",
            Self::Curve => "Curve",
            Self::Scatter => "Scatter",
            Self::VectorField => "Vector Field",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TableImportDefinition {
    pub(crate) source_path: Option<String>,
    pub(crate) raw_text: String,
    pub(crate) delimiter: TableDelimiter,
    pub(crate) header_row: bool,
    pub(crate) target: TablePlotTarget,
    pub(crate) mapping: TableColumnMapping,
}

impl TableImportDefinition {
    pub(crate) fn empty(target: TablePlotTarget) -> Self {
        Self {
            source_path: None,
            raw_text: String::new(),
            delimiter: TableDelimiter::Comma,
            header_row: false,
            target,
            mapping: TableColumnMapping::default_for(target),
        }
    }

    pub(crate) fn auto_configure(&mut self) {
        self.delimiter = detect_delimiter(&self.raw_text);
        self.header_row = detect_header_row(&self.raw_text, self.delimiter);
    }

    pub(crate) fn set_target(&mut self, target: TablePlotTarget) {
        if self.target != target {
            self.target = target;
            self.mapping = TableColumnMapping::default_for(target);
        }
    }

    pub(crate) fn preview(&self) -> TablePreview {
        parse_table_preview(&self.raw_text, self.delimiter, self.header_row)
    }

    pub(crate) fn validate(&self) -> Result<TableDataSet, Vec<TableValidationError>> {
        build_dataset(self)
    }

    pub(crate) fn source_summary(&self) -> String {
        match &self.source_path {
            Some(path) => format!("Linked file: {path}"),
            None => "Embedded table".to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OptionalColumn {
    None,
    Column(usize),
}

impl OptionalColumn {
    pub(crate) fn into_option(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Column(index) => Some(index),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum TableColumnMapping {
    SurfaceGrid {
        x: usize,
        y: usize,
        z: usize,
    },
    Curve {
        x: usize,
        y: usize,
        z: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
    Scatter {
        x: usize,
        y: usize,
        z: OptionalColumn,
        scalar: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
    VectorField {
        x: usize,
        y: usize,
        z: OptionalColumn,
        vx: usize,
        vy: usize,
        vz: OptionalColumn,
        scalar: OptionalColumn,
        label: OptionalColumn,
        group: OptionalColumn,
    },
}

impl TableColumnMapping {
    pub(crate) fn default_for(target: TablePlotTarget) -> Self {
        match target {
            TablePlotTarget::SurfaceGrid => Self::SurfaceGrid { x: 0, y: 1, z: 2 },
            TablePlotTarget::Curve => Self::Curve {
                x: 0,
                y: 1,
                z: OptionalColumn::Column(2),
                label: OptionalColumn::None,
                group: OptionalColumn::None,
            },
            TablePlotTarget::Scatter => Self::Scatter {
                x: 0,
                y: 1,
                z: OptionalColumn::Column(2),
                scalar: OptionalColumn::Column(3),
                label: OptionalColumn::None,
                group: OptionalColumn::None,
            },
            TablePlotTarget::VectorField => Self::VectorField {
                x: 0,
                y: 1,
                z: OptionalColumn::Column(2),
                vx: 3,
                vy: 4,
                vz: OptionalColumn::Column(5),
                scalar: OptionalColumn::None,
                label: OptionalColumn::None,
                group: OptionalColumn::None,
            },
        }
    }
}

#[derive(Clone)]
pub(crate) struct TablePreview {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<TableRow>,
    pub(crate) column_count: usize,
}

#[derive(Clone)]
pub(crate) struct TableRow {
    pub(crate) source_row: usize,
    pub(crate) cells: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct TableValidationError {
    pub(crate) row: Option<usize>,
    pub(crate) column: Option<usize>,
    pub(crate) message: String,
}

impl TableValidationError {
    fn general(message: impl Into<String>) -> Self {
        Self {
            row: None,
            column: None,
            message: message.into(),
        }
    }

    fn at(row: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            row: Some(row),
            column: Some(column),
            message: message.into(),
        }
    }

    pub(crate) fn display(&self) -> String {
        match (self.row, self.column) {
            (Some(row), Some(column)) => format!("row {row}, col {column}: {}", self.message),
            (Some(row), None) => format!("row {row}: {}", self.message),
            _ => self.message.clone(),
        }
    }
}

pub(crate) enum TableDataSet {
    SurfaceGrid {
        xs: Vec<f64>,
        ys: Vec<f64>,
        zs: Vec<f64>,
    },
    Curve {
        groups: Vec<Vec<glam::Vec3>>,
        bounds: DataBounds,
    },
    Scatter {
        points: Vec<glam::Vec3>,
        scalars: Option<Vec<f32>>,
        bounds: DataBounds,
    },
    VectorField {
        samples: Vec<TableVectorSample>,
        bounds: DataBounds,
    },
}

#[derive(Clone)]
pub(crate) struct TableVectorSample {
    pub(crate) position: glam::Vec3,
    pub(crate) vector: glam::Vec3,
}

pub(crate) struct TableVectorFieldPlot {
    samples: Vec<TableVectorSample>,
    bounds: DataBounds,
    style: PlotStyle,
}

impl TableVectorFieldPlot {
    pub(crate) fn new(
        samples: Vec<TableVectorSample>,
        bounds: DataBounds,
        style: PlotStyle,
    ) -> Self {
        Self {
            samples,
            bounds,
            style,
        }
    }
}

impl PlotObject for TableVectorFieldPlot {
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Cartesian
    }

    fn natural_bounds(&self) -> Option<DataBounds> {
        Some(self.bounds.clone())
    }

    fn generate(&self, _domain: &Domain, _resolution: Resolution) -> PlotGeometry {
        let glyphs = self
            .samples
            .iter()
            .map(|sample| GlyphInstance {
                position: sample.position,
                vector: sample.vector.normalize_or_zero() * self.style.glyph_scale,
                raw_vector: sample.vector,
            })
            .collect();
        PlotGeometry::Glyphs(glyphs)
    }

    fn style(&self) -> &PlotStyle {
        &self.style
    }
}

fn parse_table_preview(raw_text: &str, delimiter: TableDelimiter, header_row: bool) -> TablePreview {
    let parsed_rows = raw_text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(TableRow {
                    source_row: index + 1,
                    cells: delimiter.split_line(trimmed),
                })
            }
        })
        .collect::<Vec<_>>();
    let column_count = parsed_rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
    let mut rows = parsed_rows;
    let headers = if header_row && !rows.is_empty() {
        let header = rows.remove(0);
        normalize_headers(header.cells, column_count)
    } else {
        (0..column_count).map(|index| format!("Column {}", index + 1)).collect()
    };
    TablePreview {
        headers,
        rows,
        column_count,
    }
}

fn normalize_headers(mut cells: Vec<String>, column_count: usize) -> Vec<String> {
    cells.resize_with(column_count, String::new);
    cells.into_iter()
        .enumerate()
        .map(|(index, header)| {
            let trimmed = header.trim();
            if trimmed.is_empty() {
                format!("Column {}", index + 1)
            } else {
                trimmed.to_string()
            }
        })
        .collect()
}

fn detect_delimiter(raw_text: &str) -> TableDelimiter {
    let sample_lines = raw_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
        .collect::<Vec<_>>();
    let candidates = [
        (TableDelimiter::Comma, ','),
        (TableDelimiter::Semicolon, ';'),
        (TableDelimiter::Tab, '\t'),
        (TableDelimiter::Pipe, '|'),
    ];
    let mut best = TableDelimiter::Comma;
    let mut best_score = 0usize;
    for (delimiter, marker) in candidates {
        let score = sample_lines
            .iter()
            .map(|line| line.matches(marker).count())
            .sum::<usize>();
        if score > best_score {
            best = delimiter;
            best_score = score;
        }
    }
    if best_score == 0 && sample_lines.iter().any(|line| line.split_whitespace().count() > 1) {
        TableDelimiter::Space
    } else {
        best
    }
}

fn detect_header_row(raw_text: &str, delimiter: TableDelimiter) -> bool {
    let mut rows = raw_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| delimiter.split_line(line))
        .take(2);
    let Some(first) = rows.next() else {
        return false;
    };
    let second = rows.next();
    let first_numeric = first.iter().filter(|cell| cell.parse::<f64>().is_ok()).count();
    let second_numeric = second
        .as_ref()
        .map(|row| row.iter().filter(|cell| cell.parse::<f64>().is_ok()).count())
        .unwrap_or(0);
    first_numeric < first.len().saturating_div(2) && second_numeric >= first_numeric
}

fn build_dataset(definition: &TableImportDefinition) -> Result<TableDataSet, Vec<TableValidationError>> {
    let preview = definition.preview();
    if preview.column_count == 0 {
        return Err(vec![TableValidationError::general("table is empty")]);
    }
    if preview.rows.is_empty() {
        return Err(vec![TableValidationError::general(
            "table has no data rows after applying header settings",
        )]);
    }
    match (&definition.target, &definition.mapping) {
        (TablePlotTarget::SurfaceGrid, TableColumnMapping::SurfaceGrid { x, y, z }) => {
            build_surface_grid(&preview, *x, *y, *z)
        }
        (
            TablePlotTarget::Curve,
            TableColumnMapping::Curve {
                x,
                y,
                z,
                label: _,
                group,
            },
        ) => build_curve_data(&preview, *x, *y, *z, *group),
        (
            TablePlotTarget::Scatter,
            TableColumnMapping::Scatter {
                x,
                y,
                z,
                scalar,
                label: _,
                group: _,
            },
        ) => build_scatter_data(&preview, *x, *y, *z, *scalar),
        (
            TablePlotTarget::VectorField,
            TableColumnMapping::VectorField {
                x,
                y,
                z,
                vx,
                vy,
                vz,
                scalar: _,
                label: _,
                group: _,
            },
        ) => build_vector_field_data(&preview, *x, *y, *z, *vx, *vy, *vz),
        _ => Err(vec![TableValidationError::general(
            "table mapping does not match the selected plot type",
        )]),
    }
}

fn build_surface_grid(
    preview: &TablePreview,
    x_col: usize,
    y_col: usize,
    z_col: usize,
) -> Result<TableDataSet, Vec<TableValidationError>> {
    let mut errors = Vec::new();
    let mut x_values = Vec::<(String, f64)>::new();
    let mut y_values = Vec::<(String, f64)>::new();
    let mut x_lookup = HashMap::<String, usize>::new();
    let mut y_lookup = HashMap::<String, usize>::new();
    let mut rows = Vec::new();
    for row in &preview.rows {
        let x_raw = required_cell(preview, row, x_col, "x", &mut errors);
        let y_raw = required_cell(preview, row, y_col, "y", &mut errors);
        let z_raw = required_cell(preview, row, z_col, "z", &mut errors);
        if let (Some(x_raw), Some(y_raw), Some(z_raw)) = (x_raw, y_raw, z_raw) {
            let x = parse_f64(row.source_row, x_col, "x", x_raw, &mut errors);
            let y = parse_f64(row.source_row, y_col, "y", y_raw, &mut errors);
            let z = parse_f64(row.source_row, z_col, "z", z_raw, &mut errors);
            if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                let x_index = *x_lookup.entry(x_raw.to_string()).or_insert_with(|| {
                    x_values.push((x_raw.to_string(), x));
                    x_values.len() - 1
                });
                let y_index = *y_lookup.entry(y_raw.to_string()).or_insert_with(|| {
                    y_values.push((y_raw.to_string(), y));
                    y_values.len() - 1
                });
                rows.push((row.source_row, x_index, y_index, z));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let width = x_values.len();
    let height = y_values.len();
    if width == 0 || height == 0 {
        return Err(vec![TableValidationError::general(
            "surface grid needs at least one x column and one y column",
        )]);
    }
    let mut zs = vec![None; width * height];
    for (source_row, x_index, y_index, z) in rows {
        let slot = &mut zs[y_index * width + x_index];
        if slot.is_some() {
            errors.push(TableValidationError::at(
                source_row,
                z_col + 1,
                "duplicate surface sample for the same x/y pair",
            ));
        } else {
            *slot = Some(z);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if zs.iter().any(Option::is_none) {
        return Err(vec![TableValidationError::general(
            "surface grid is incomplete; every x/y combination must be present",
        )]);
    }
    Ok(TableDataSet::SurfaceGrid {
        xs: x_values.into_iter().map(|(_, value)| value).collect(),
        ys: y_values.into_iter().map(|(_, value)| value).collect(),
        zs: zs.into_iter().flatten().collect(),
    })
}

fn build_curve_data(
    preview: &TablePreview,
    x_col: usize,
    y_col: usize,
    z_col: OptionalColumn,
    group_col: OptionalColumn,
) -> Result<TableDataSet, Vec<TableValidationError>> {
    let mut errors = Vec::new();
    let mut groups = HashMap::<String, Vec<glam::Vec3>>::new();
    let mut order = Vec::<String>::new();
    let mut bounds = BoundsAccumulator::default();
    for row in &preview.rows {
        let x = parse_required_number(preview, row, x_col, "x", &mut errors);
        let y = parse_required_number(preview, row, y_col, "y", &mut errors);
        let z = parse_optional_number(preview, row, z_col.into_option(), "z", &mut errors)
            .unwrap_or(0.0);
        if let (Some(x), Some(y)) = (x, y) {
            let point = glam::Vec3::new(x as f32, y as f32, z as f32);
            let group_key = optional_cell(preview, row, group_col.into_option())
                .unwrap_or("")
                .to_string();
            if !groups.contains_key(&group_key) {
                order.push(group_key.clone());
            }
            groups.entry(group_key).or_default().push(point);
            bounds.add(point);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let grouped_points = order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .filter(|points| !points.is_empty())
        .collect::<Vec<_>>();
    let Some(bounds) = bounds.finish() else {
        return Err(vec![TableValidationError::general("curve table has no valid points")]);
    };
    Ok(TableDataSet::Curve {
        groups: grouped_points,
        bounds,
    })
}

fn build_scatter_data(
    preview: &TablePreview,
    x_col: usize,
    y_col: usize,
    z_col: OptionalColumn,
    scalar_col: OptionalColumn,
) -> Result<TableDataSet, Vec<TableValidationError>> {
    let mut errors = Vec::new();
    let mut points = Vec::new();
    let mut scalars = Vec::new();
    let mut has_scalar = false;
    let mut bounds = BoundsAccumulator::default();
    for row in &preview.rows {
        let x = parse_required_number(preview, row, x_col, "x", &mut errors);
        let y = parse_required_number(preview, row, y_col, "y", &mut errors);
        let z = parse_optional_number(preview, row, z_col.into_option(), "z", &mut errors)
            .unwrap_or(0.0);
        let scalar =
            parse_optional_number(preview, row, scalar_col.into_option(), "scalar", &mut errors);
        if let (Some(x), Some(y)) = (x, y) {
            let point = glam::Vec3::new(x as f32, y as f32, z as f32);
            points.push(point);
            bounds.add(point);
            if let Some(value) = scalar {
                scalars.push(value as f32);
                has_scalar = true;
            } else {
                scalars.push(z as f32);
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let Some(bounds) = bounds.finish() else {
        return Err(vec![TableValidationError::general("scatter table has no valid points")]);
    };
    Ok(TableDataSet::Scatter {
        points,
        scalars: has_scalar.then_some(scalars),
        bounds,
    })
}

fn build_vector_field_data(
    preview: &TablePreview,
    x_col: usize,
    y_col: usize,
    z_col: OptionalColumn,
    vx_col: usize,
    vy_col: usize,
    vz_col: OptionalColumn,
) -> Result<TableDataSet, Vec<TableValidationError>> {
    let mut errors = Vec::new();
    let mut samples = Vec::new();
    let mut bounds = BoundsAccumulator::default();
    for row in &preview.rows {
        let x = parse_required_number(preview, row, x_col, "x", &mut errors);
        let y = parse_required_number(preview, row, y_col, "y", &mut errors);
        let z = parse_optional_number(preview, row, z_col.into_option(), "z", &mut errors)
            .unwrap_or(0.0);
        let vx = parse_required_number(preview, row, vx_col, "vx", &mut errors);
        let vy = parse_required_number(preview, row, vy_col, "vy", &mut errors);
        let vz = parse_optional_number(preview, row, vz_col.into_option(), "vz", &mut errors)
            .unwrap_or(0.0);
        if let (Some(x), Some(y), Some(vx), Some(vy)) = (x, y, vx, vy) {
            let position = glam::Vec3::new(x as f32, y as f32, z as f32);
            let vector = glam::Vec3::new(vx as f32, vy as f32, vz as f32);
            samples.push(TableVectorSample { position, vector });
            bounds.add(position);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let Some(bounds) = bounds.finish() else {
        return Err(vec![TableValidationError::general(
            "vector field table has no valid samples",
        )]);
    };
    Ok(TableDataSet::VectorField { samples, bounds })
}

fn required_cell<'a>(
    preview: &TablePreview,
    row: &'a TableRow,
    column: usize,
    label: &str,
    errors: &mut Vec<TableValidationError>,
) -> Option<&'a str> {
    let value = optional_cell(preview, row, Some(column));
    match value {
        Some(value) if !value.trim().is_empty() => Some(value),
        _ => {
            if column >= preview.column_count {
                errors.push(TableValidationError::general(format!(
                    "{label} column {} is outside the table width",
                    column + 1
                )));
            } else {
                errors.push(TableValidationError::at(
                    row.source_row,
                    column + 1,
                    format!("missing {label} value"),
                ));
            }
            None
        }
    }
}

fn optional_cell<'a>(preview: &TablePreview, row: &'a TableRow, column: Option<usize>) -> Option<&'a str> {
    let column = column?;
    if column >= preview.column_count {
        return None;
    }
    row.cells.get(column).map(String::as_str)
}

fn parse_required_number(
    preview: &TablePreview,
    row: &TableRow,
    column: usize,
    label: &str,
    errors: &mut Vec<TableValidationError>,
) -> Option<f64> {
    let value = required_cell(preview, row, column, label, errors)?;
    parse_f64(row.source_row, column, label, value, errors)
}

fn parse_optional_number(
    preview: &TablePreview,
    row: &TableRow,
    column: Option<usize>,
    label: &str,
    errors: &mut Vec<TableValidationError>,
) -> Option<f64> {
    let Some(column) = column else {
        return None;
    };
    let Some(value) = optional_cell(preview, row, Some(column)) else {
        return None;
    };
    if value.trim().is_empty() {
        return None;
    }
    parse_f64(row.source_row, column, label, value, errors)
}

fn parse_f64(
    row: usize,
    column: usize,
    label: &str,
    value: &str,
    errors: &mut Vec<TableValidationError>,
) -> Option<f64> {
    match value.parse::<f64>() {
        Ok(number) => Some(number),
        Err(_) => {
            errors.push(TableValidationError::at(
                row,
                column + 1,
                format!("{label} is not a number"),
            ));
            None
        }
    }
}

#[derive(Default)]
struct BoundsAccumulator {
    min: Option<glam::Vec3>,
    max: Option<glam::Vec3>,
}

impl BoundsAccumulator {
    fn add(&mut self, point: glam::Vec3) {
        self.min = Some(self.min.map(|value| value.min(point)).unwrap_or(point));
        self.max = Some(self.max.map(|value| value.max(point)).unwrap_or(point));
    }

    fn finish(self) -> Option<DataBounds> {
        let min = self.min?;
        let max = self.max?;
        Some(DataBounds {
            x: min.x as f64..=max.x as f64,
            y: min.y as f64..=max.y as f64,
            z: min.z as f64..=max.z as f64,
        })
    }
}

pub(crate) fn build_curve_piecewise(groups: &[Vec<glam::Vec3>], style: PlotStyle) -> PiecewisePlot {
    let mut plot = PiecewisePlot::new();
    for points in groups {
        if points.is_empty() {
            continue;
        }
        let bounds = bounds_for_points(points);
        plot.add_piece(
            bounds,
            poincare_lib::Curve3D::from_points(points).with_style(style.clone()),
        );
    }
    plot
}

fn bounds_for_points(points: &[glam::Vec3]) -> Domain {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for &point in points {
        min = min.min(point);
        max = max.max(point);
    }
    Domain {
        x: min.x as f64..=max.x as f64,
        y: min.y as f64..=max.y as f64,
        z: min.z as f64..=max.z as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_semicolon_and_header_row() {
        let raw = "x;y;z\n1;2;3\n4;5;6\n";
        assert_eq!(detect_delimiter(raw), TableDelimiter::Semicolon);
        assert!(detect_header_row(raw, TableDelimiter::Semicolon));
    }

    #[test]
    fn validates_surface_grid_completeness() {
        let table = TableImportDefinition {
            source_path: None,
            raw_text: "x,y,z\n0,0,1\n1,0,2\n".to_string(),
            delimiter: TableDelimiter::Comma,
            header_row: true,
            target: TablePlotTarget::SurfaceGrid,
            mapping: TableColumnMapping::SurfaceGrid { x: 0, y: 1, z: 2 },
        };
        let result = table.validate();
        assert!(result.is_ok());
    }
}
