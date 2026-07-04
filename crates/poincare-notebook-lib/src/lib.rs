//! Shared document model for Poincare notebooks.
//!
//! This crate intentionally contains no UI, GPU, or platform file-dialog code.
//! It defines the persistent notebook schema that `poincare-notebook-app`,
//! evaluator crates, and bundle read/write code can share.

use poincare_lib::{GraphSpec, PlotSpec};
use serde::{Deserialize, Serialize};

pub mod inspection;
pub mod outputs;
pub mod runner;
pub mod runtime;
pub use inspection::*;
pub use outputs::*;
pub use runner::*;
pub use runtime::*;

pub const NOTEBOOK_SCHEMA_VERSION: u32 = 1;
pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotebookId(pub String);

impl NotebookId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotebookCellId(pub String);

impl NotebookCellId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotebookOutputId(pub String);

impl NotebookOutputId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttachmentId(pub String);

impl AttachmentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphBlockId(pub String);

impl GraphBlockId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookDocument {
    pub schema_version: u32,
    pub id: NotebookId,
    pub title: String,
    pub metadata: NotebookMetadata,
    pub blocks: Vec<NotebookBlock>,
}

impl NotebookDocument {
    pub fn new(id: NotebookId, title: impl Into<String>) -> Self {
        Self {
            schema_version: NOTEBOOK_SCHEMA_VERSION,
            id,
            title: title.into(),
            metadata: NotebookMetadata::default(),
            blocks: Vec::new(),
        }
    }

    pub fn add_block(&mut self, block: NotebookBlock) {
        self.blocks.push(block);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NotebookMetadata {
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookBlock {
    pub id: NotebookCellId,
    pub kind: NotebookBlockKind,
    pub outputs: Vec<NotebookOutput>,
    pub state: NotebookBlockState,
}

impl NotebookBlock {
    pub fn markdown(id: NotebookCellId, source: impl Into<String>) -> Self {
        Self {
            id,
            kind: NotebookBlockKind::Text(TextCell {
                format: TextFormat::Markdown,
                source: source.into(),
            }),
            outputs: Vec::new(),
            state: NotebookBlockState::default(),
        }
    }

    pub fn executable(
        id: NotebookCellId,
        language_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind: NotebookBlockKind::Executable(ExecutableCell {
                language_id: language_id.into(),
                source: source.into(),
                execution: ExecutionState::Idle,
            }),
            outputs: Vec::new(),
            state: NotebookBlockState::default(),
        }
    }

    pub fn graph(id: NotebookCellId, graph: GraphBlock) -> Self {
        Self {
            id,
            kind: NotebookBlockKind::Graph(graph),
            outputs: Vec::new(),
            state: NotebookBlockState::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NotebookBlockKind {
    Text(TextCell),
    Executable(ExecutableCell),
    Graph(GraphBlock),
    Table(TableBlock),
    Diagnostic(DiagnosticBlock),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFormat {
    Markdown,
    PlainText,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextCell {
    pub format: TextFormat,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutableCell {
    pub language_id: String,
    pub source: String,
    pub execution: ExecutionState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Idle,
    Queued,
    Running,
    Complete { run_count: u64 },
    Failed { run_count: u64 },
    Stale { reasons: Vec<StaleReason> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaleReason {
    SourceEdited,
    EarlierCellEdited { cell_id: NotebookCellId },
    EvaluatorRestarted,
    AttachmentChanged { attachment_id: AttachmentId },
    GraphEdited { graph_id: GraphBlockId },
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookBlockState {
    pub collapsed: bool,
    pub outputs_collapsed: bool,
}

impl Default for NotebookBlockState {
    fn default() -> Self {
        Self {
            collapsed: false,
            outputs_collapsed: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookOutput {
    pub id: NotebookOutputId,
    pub kind: NotebookOutputKind,
    pub provenance: OutputProvenance,
    pub stale: Vec<StaleReason>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NotebookOutputKind {
    Text(TextOutput),
    Value(ValueOutput),
    Table(TableOutput),
    Graph(GraphOutput),
    Image(ImageOutput),
    Diagnostic(NotebookDiagnostic),
    Analysis(AnalysisSnapshot),
    Attachment(AttachmentRef),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextOutput {
    pub stream: TextStream,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextStream {
    Stdout,
    Stderr,
    Display,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueOutput {
    pub value_kind: ValueKind,
    pub preview: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Unit,
    Bool,
    Number,
    String,
    List,
    Function,
    Attachment,
    Bytes,
    Expression,
    Table,
    Array,
    Plot,
    Graph,
    Analysis,
    Image,
    Diagnostic,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableOutput {
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphOutput {
    pub graph_id: GraphBlockId,
    pub ownership: GraphOwnership,
    pub preview: Option<BundlePath>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageOutput {
    pub path: BundlePath,
    pub mime_type: String,
    pub alt_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisSnapshot {
    pub title: String,
    pub reports: Vec<AnalysisReportSnapshot>,
    pub tables: Vec<TableOutput>,
    pub plots: Vec<PlotSpec>,
    pub diagnostics: Vec<NotebookDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReportSnapshot {
    pub title: String,
    pub values: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputProvenance {
    pub source_cell: Option<NotebookCellId>,
    pub produced_at: Option<String>,
    pub run_count: Option<u64>,
    pub input_hash: Option<String>,
    pub graph_dependencies: Vec<GraphBlockId>,
    pub attachment_dependencies: Vec<AttachmentId>,
    pub notes: Vec<String>,
}

impl OutputProvenance {
    pub fn source_cell(source_cell: NotebookCellId) -> Self {
        Self {
            source_cell: Some(source_cell),
            produced_at: None,
            run_count: None,
            input_hash: None,
            graph_dependencies: Vec::new(),
            attachment_dependencies: Vec::new(),
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub code: Option<String>,
}

impl NotebookDiagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            span: None,
            code: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphBlock {
    pub id: GraphBlockId,
    pub ownership: GraphOwnership,
    pub graph: GraphSpec,
    pub view: GraphViewState,
    pub presentation: GraphPresentation,
    pub preview: Option<BundlePath>,
    pub provenance: GraphProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphOwnership {
    Snapshot,
    Linked { source: LinkedGraphSource },
    Computed { source_cell: NotebookCellId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedGraphSource {
    pub path: String,
    pub graph_id: Option<String>,
    pub last_known_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphViewState {
    pub camera_position: [f32; 3],
    pub camera_target: [f32; 3],
    pub camera_up: [f32; 3],
    pub distance: f32,
    pub azimuth: f32,
    pub elevation: f32,
    pub roll: f32,
    pub projection: ProjectionMode,
    pub viewport_size: [u32; 2],
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self {
            camera_position: [3.0, 3.0, 3.0],
            camera_target: [0.0, 0.0, 0.0],
            camera_up: [0.0, 1.0, 0.0],
            distance: 5.0,
            azimuth: 45.0,
            elevation: 30.0,
            roll: 0.0,
            projection: ProjectionMode::Perspective,
            viewport_size: [1200, 800],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionMode {
    Perspective,
    Orthographic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphPresentation {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub background: Option<[f32; 4]>,
    pub show_scalarbars: bool,
    pub show_legend: bool,
    pub show_labels: bool,
}

impl Default for GraphPresentation {
    fn default() -> Self {
        Self {
            title: None,
            caption: None,
            background: None,
            show_scalarbars: true,
            show_legend: true,
            show_labels: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProvenance {
    pub source_cell: Option<NotebookCellId>,
    pub source_attachments: Vec<AttachmentId>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableBlock {
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticBlock {
    pub diagnostics: Vec<NotebookDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub id: AttachmentId,
    pub path: BundlePath,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookAttachment {
    pub id: AttachmentId,
    pub display_name: String,
    pub path: BundlePath,
    pub media_type: Option<String>,
    pub original_path: Option<String>,
    pub size_bytes: Option<u64>,
    pub hash: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundlePath(pub String);

impl BundlePath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotebookBundleManifest {
    pub schema_version: u32,
    pub notebook_id: NotebookId,
    pub document_path: BundlePath,
    pub attachments: Vec<NotebookAttachment>,
    pub graph_assets: Vec<GraphAsset>,
    pub output_assets: Vec<OutputAsset>,
    pub metadata: BundleMetadata,
}

impl NotebookBundleManifest {
    pub fn new(notebook_id: NotebookId) -> Self {
        Self {
            schema_version: BUNDLE_SCHEMA_VERSION,
            notebook_id,
            document_path: BundlePath::new("document.json"),
            attachments: Vec::new(),
            graph_assets: Vec::new(),
            output_assets: Vec::new(),
            metadata: BundleMetadata::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAsset {
    pub graph_id: GraphBlockId,
    pub graph_path: BundlePath,
    pub view_path: BundlePath,
    pub preview_path: Option<BundlePath>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputAsset {
    pub output_id: NotebookOutputId,
    pub path: BundlePath,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_round_trips_through_json() {
        let cell_id = NotebookCellId::new("cell-1");
        let mut document = NotebookDocument::new(NotebookId::new("notebook-1"), "Demo");
        let mut cell = NotebookBlock::executable(cell_id.clone(), "poincare", "print(\"hello\")");
        cell.outputs.push(NotebookOutput {
            id: NotebookOutputId::new("output-1"),
            kind: NotebookOutputKind::Text(TextOutput {
                stream: TextStream::Stdout,
                text: "hello\n".to_string(),
            }),
            provenance: OutputProvenance::source_cell(cell_id),
            stale: Vec::new(),
        });
        document.add_block(cell);

        let json = serde_json::to_string_pretty(&document).expect("serialize document");
        let restored: NotebookDocument = serde_json::from_str(&json).expect("deserialize document");
        assert_eq!(restored.schema_version, NOTEBOOK_SCHEMA_VERSION);
        assert_eq!(restored.id, NotebookId::new("notebook-1"));
        assert_eq!(restored.blocks.len(), 1);
        let restored_json =
            serde_json::to_value(&restored).expect("serialize restored document to value");
        let original_json =
            serde_json::to_value(&document).expect("serialize original document to value");
        assert_eq!(restored_json, original_json);
    }

    #[test]
    fn graph_block_preserves_graph_and_view_state() {
        let graph_id = GraphBlockId::new("graph-1");
        let graph = GraphBlock {
            id: graph_id.clone(),
            ownership: GraphOwnership::Snapshot,
            graph: GraphSpec::new(),
            view: GraphViewState {
                camera_position: [1.0, 2.0, 3.0],
                camera_target: [0.0, 0.5, 0.0],
                distance: 10.0,
                ..GraphViewState::default()
            },
            presentation: GraphPresentation {
                title: Some("Surface".to_string()),
                ..GraphPresentation::default()
            },
            preview: Some(BundlePath::new("graphs/graph-1/preview.png")),
            provenance: GraphProvenance::default(),
        };
        let document = NotebookDocument {
            schema_version: NOTEBOOK_SCHEMA_VERSION,
            id: NotebookId::new("notebook-1"),
            title: "Graphs".to_string(),
            metadata: NotebookMetadata::default(),
            blocks: vec![NotebookBlock::graph(NotebookCellId::new("cell-1"), graph)],
        };

        let json = serde_json::to_string(&document).expect("serialize graph document");
        let restored: NotebookDocument =
            serde_json::from_str(&json).expect("deserialize graph document");
        assert_eq!(restored.blocks.len(), 1);
        assert!(json.contains("graphs/graph-1/preview.png"));
        assert!(json.contains("Surface"));
        assert!(json.contains(&graph_id.0));
    }

    #[test]
    fn bundle_manifest_round_trips_with_attachment() {
        let mut manifest = NotebookBundleManifest::new(NotebookId::new("notebook-1"));
        manifest.attachments.push(NotebookAttachment {
            id: AttachmentId::new("attachment-1"),
            display_name: "samples.csv".to_string(),
            path: BundlePath::new("attachments/attachment-1/samples.csv"),
            media_type: Some("text/csv".to_string()),
            original_path: Some("/tmp/samples.csv".to_string()),
            size_bytes: Some(128),
            hash: Some("sha256:abc".to_string()),
            created_at: None,
            updated_at: None,
        });

        let json = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        let restored: NotebookBundleManifest =
            serde_json::from_str(&json).expect("deserialize manifest");
        assert_eq!(restored, manifest);
    }
}
