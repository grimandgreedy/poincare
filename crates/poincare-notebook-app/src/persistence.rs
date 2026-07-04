use std::fs;
use std::path::Path;

use poincare_notebook_lib::NotebookDocument;

pub fn load_document(path: &Path) -> Result<NotebookDocument, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

pub fn save_document(path: &Path, document: &NotebookDocument) -> Result<(), String> {
    let text = serde_json::to_string_pretty(document)
        .map_err(|err| format!("failed to serialize notebook: {err}"))?;
    fs::write(path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
}
