use std::collections::BTreeMap;

use poincare_evaluator as evaluator;
use serde::{Deserialize, Serialize};

use crate::{AttachmentId, BundlePath, NotebookAttachment};

#[derive(Clone, Debug, Default)]
pub struct NotebookAttachmentHost {
    attachments: BTreeMap<String, NotebookAttachment>,
    names: BTreeMap<String, String>,
    bytes: BTreeMap<String, Vec<u8>>,
}

impl NotebookAttachmentHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_attachment(mut self, attachment: NotebookAttachment, bytes: Vec<u8>) -> Self {
        self.insert_attachment(attachment, bytes);
        self
    }

    pub fn insert_attachment(&mut self, attachment: NotebookAttachment, bytes: Vec<u8>) {
        self.names
            .insert(attachment.display_name.clone(), attachment.id.0.clone());
        self.bytes.insert(attachment.id.0.clone(), bytes);
        self.attachments.insert(attachment.id.0.clone(), attachment);
    }

    pub fn attachments(&self) -> impl Iterator<Item = &NotebookAttachment> {
        self.attachments.values()
    }

    fn attachment(
        &self,
        id: &evaluator::EvalAttachmentId,
    ) -> Result<&NotebookAttachment, evaluator::HostError> {
        self.attachments.get(&id.0).ok_or_else(|| {
            evaluator::HostError::not_found(format!("attachment `{}` not found", id.0))
        })
    }
}

impl evaluator::RuntimeHost for NotebookAttachmentHost {
    fn resolve_attachment(
        &self,
        name_or_id: &str,
    ) -> Result<evaluator::AttachmentValue, evaluator::HostError> {
        let id = self
            .attachments
            .contains_key(name_or_id)
            .then(|| name_or_id.to_string())
            .or_else(|| self.names.get(name_or_id).cloned())
            .ok_or_else(|| {
                evaluator::HostError::not_found(format!("attachment `{name_or_id}` not found"))
            })?;
        let attachment = self
            .attachments
            .get(&id)
            .expect("attachment id came from attachment index");

        Ok(evaluator::AttachmentValue {
            id: evaluator::EvalAttachmentId(id),
            display_name: attachment.display_name.clone(),
            media_type: attachment.media_type.clone(),
            size_bytes: attachment.size_bytes,
            hash: attachment.hash.clone(),
        })
    }

    fn attachment_bytes(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<Vec<u8>, evaluator::HostError> {
        self.attachment(attachment)?;
        self.bytes.get(&attachment.0).cloned().ok_or_else(|| {
            evaluator::HostError::not_found(format!(
                "attachment `{}` has no bundled bytes",
                attachment.0
            ))
        })
    }

    fn attachment_table(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<evaluator::TableValue, evaluator::HostError> {
        let metadata = self.attachment(attachment)?;
        let text = self.attachment_text(attachment)?;
        csv_table_value(metadata.display_name.clone(), &text)
    }

    fn attachment_array(
        &self,
        attachment: &evaluator::EvalAttachmentId,
    ) -> Result<evaluator::ArrayValue, evaluator::HostError> {
        let text = self.attachment_text(attachment)?;
        csv_array_value(&text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedCsvShape {
    pub rows: usize,
    pub columns: usize,
}

pub fn csv_table_value(
    title: impl Into<String>,
    text: &str,
) -> Result<evaluator::TableValue, evaluator::HostError> {
    let rows = parse_csv_rows(text);
    if rows.is_empty() {
        return Err(evaluator::HostError {
            kind: evaluator::HostErrorKind::UnsupportedMediaType,
            message: "CSV attachment is empty".to_string(),
        });
    }

    let header = detect_header(&rows);
    let (columns, data_rows) = if header {
        (rows[0].clone(), rows[1..].to_vec())
    } else {
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        (
            (0..width)
                .map(|index| format!("column_{}", index + 1))
                .collect(),
            rows,
        )
    };

    Ok(evaluator::TableValue {
        title: Some(title.into()),
        columns,
        rows: data_rows,
        truncated: false,
    })
}

pub fn csv_array_value(text: &str) -> Result<evaluator::ArrayValue, evaluator::HostError> {
    let rows = parse_csv_rows(text);
    if rows.is_empty() {
        return Err(evaluator::HostError {
            kind: evaluator::HostErrorKind::UnsupportedMediaType,
            message: "CSV attachment is empty".to_string(),
        });
    }

    let data_rows = if detect_header(&rows) {
        &rows[1..]
    } else {
        &rows[..]
    };
    let column_count = data_rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut values = Vec::new();
    for (row_index, row) in data_rows.iter().enumerate() {
        if row.len() != column_count {
            return Err(evaluator::HostError {
                kind: evaluator::HostErrorKind::UnsupportedMediaType,
                message: format!("CSV row {} has inconsistent column count", row_index + 1),
            });
        }
        for cell in row {
            let value = cell.parse::<f64>().map_err(|_| evaluator::HostError {
                kind: evaluator::HostErrorKind::UnsupportedMediaType,
                message: format!("CSV value `{cell}` is not numeric"),
            })?;
            values.push(evaluator::NumberValue::Float(value));
        }
    }

    Ok(evaluator::ArrayValue {
        shape: vec![data_rows.len(), column_count],
        values,
    })
}

fn parse_csv_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split(',')
                .map(|cell| trim_csv_cell(cell).to_string())
                .collect()
        })
        .collect()
}

fn trim_csv_cell(cell: &str) -> &str {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(trimmed)
}

fn detect_header(rows: &[Vec<String>]) -> bool {
    rows.first()
        .is_some_and(|row| row.iter().any(|cell| cell.parse::<f64>().is_err()))
}

pub fn attachment(
    id: impl Into<String>,
    display_name: impl Into<String>,
    path: impl Into<String>,
    media_type: Option<String>,
) -> NotebookAttachment {
    NotebookAttachment {
        id: AttachmentId::new(id),
        display_name: display_name.into(),
        path: BundlePath::new(path),
        media_type,
        original_path: None,
        size_bytes: None,
        hash: None,
        created_at: None,
        updated_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evaluator::RuntimeHost;

    #[test]
    fn resolves_attachment_by_id_or_name_and_reads_bytes() {
        let host = NotebookAttachmentHost::new().with_attachment(
            attachment(
                "attachment-1",
                "samples.csv",
                "attachments/attachment-1/samples.csv",
                Some("text/csv".to_string()),
            ),
            b"x,y\n1,2".to_vec(),
        );

        let by_name = host.resolve_attachment("samples.csv").expect("by name");
        let by_id = host.resolve_attachment("attachment-1").expect("by id");
        assert_eq!(by_name.id, by_id.id);
        assert_eq!(
            host.attachment_bytes(&by_name.id).expect("bytes"),
            b"x,y\n1,2"
        );
    }

    #[test]
    fn parses_csv_attachment_as_table() {
        let host = NotebookAttachmentHost::new().with_attachment(
            attachment(
                "attachment-1",
                "samples.csv",
                "samples.csv",
                Some("text/csv".to_string()),
            ),
            b"x,y\n1,2\n3,4".to_vec(),
        );
        let attachment = host.resolve_attachment("samples.csv").expect("attachment");

        let table = host.attachment_table(&attachment.id).expect("table");

        assert_eq!(table.columns, vec!["x", "y"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["1", "2"]);
    }

    #[test]
    fn parses_numeric_csv_attachment_as_array() {
        let host = NotebookAttachmentHost::new().with_attachment(
            attachment(
                "attachment-1",
                "grid.csv",
                "grid.csv",
                Some("text/csv".to_string()),
            ),
            b"x,y\n1,2\n3,4".to_vec(),
        );
        let attachment = host.resolve_attachment("grid.csv").expect("attachment");

        let array = host.attachment_array(&attachment.id).expect("array");

        assert_eq!(array.shape, vec![2, 2]);
        assert_eq!(array.values.len(), 4);
    }
}
