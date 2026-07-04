//! Backend-neutral evaluator API for Poincare notebooks.
//!
//! This crate defines the contract between notebook cells and evaluator
//! backends. It intentionally does not depend on egui, eframe, wgpu, or any
//! concrete language runtime.

use poincare_lib::{GraphSpec, PlotSpec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalDocumentId(pub String);

impl EvalDocumentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalCellId(pub String);

impl EvalCellId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalOutputId(pub String);

impl EvalOutputId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalAttachmentId(pub String);

impl EvalAttachmentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub trait Evaluator {
    fn metadata(&self) -> EvaluatorMetadata;

    fn evaluate_cell(&mut self, request: EvalRequest, host: &dyn RuntimeHost) -> EvalResponse;

    fn session_snapshot(&self) -> SessionSnapshot;

    fn restart(&mut self) -> EvalResponse {
        EvalResponse {
            status: EvalStatus::Complete,
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            context_delta: Some(EvalContextDelta::Restarted),
            session: Some(self.session_snapshot()),
        }
    }

    fn delete_variable(&mut self, name: &str) -> EvalResponse {
        EvalResponse::failed(vec![EvalDiagnostic {
            severity: EvalDiagnosticSeverity::Error,
            message: format!("deleting variable `{name}` is not supported by this evaluator"),
            span: None,
            code: Some("UNSUPPORTED_DELETE_VARIABLE".to_string()),
        }])
    }
}

pub trait EvaluatorFactory {
    fn metadata(&self) -> EvaluatorMetadata;
    fn create(&self, config: EvaluatorConfig) -> Box<dyn Evaluator>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorConfig {
    pub options: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorMetadata {
    pub language_id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub features: EvaluatorFeatures,
    pub safety: EvaluatorSafety,
}

impl EvaluatorMetadata {
    pub fn new(language_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            language_id: language_id.into(),
            display_name: display_name.into(),
            version: None,
            features: EvaluatorFeatures::default(),
            safety: EvaluatorSafety::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorFeatures {
    pub supports_shared_state: bool,
    pub supports_interrupt: bool,
    pub supports_attachments: bool,
    pub supports_graph_outputs: bool,
    pub supports_table_outputs: bool,
    pub supports_symbolic_expr: bool,
    pub supports_variable_delete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorSafety {
    pub isolation: IsolationLevel,
    pub capabilities: Vec<EvaluatorCapability>,
}

impl Default for EvaluatorSafety {
    fn default() -> Self {
        Self {
            isolation: IsolationLevel::InProcess,
            capabilities: vec![EvaluatorCapability::PureComputation],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    InProcess,
    Process,
    Sandbox,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluatorCapability {
    PureComputation,
    AttachmentRead,
    FilesystemRead,
    FilesystemWrite,
    Network,
    ProcessExecution,
    NativeCode,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalRequest {
    pub document_id: EvalDocumentId,
    pub cell_id: EvalCellId,
    pub source: String,
    pub context: EvalContext,
}

impl EvalRequest {
    pub fn new(
        document_id: EvalDocumentId,
        cell_id: EvalCellId,
        source: impl Into<String>,
    ) -> Self {
        Self {
            document_id,
            cell_id,
            source: source.into(),
            context: EvalContext::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EvalContext {
    pub run_count: u64,
    pub variables: Vec<VariableSummary>,
    pub stale_reasons: Vec<EvalStaleReason>,
    pub options: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalResponse {
    pub status: EvalStatus,
    pub outputs: Vec<EvalOutput>,
    pub diagnostics: Vec<EvalDiagnostic>,
    pub context_delta: Option<EvalContextDelta>,
    pub session: Option<SessionSnapshot>,
}

impl EvalResponse {
    pub fn complete(outputs: Vec<EvalOutput>) -> Self {
        Self {
            status: EvalStatus::Complete,
            outputs,
            diagnostics: Vec::new(),
            context_delta: None,
            session: None,
        }
    }

    pub fn failed(diagnostics: Vec<EvalDiagnostic>) -> Self {
        Self {
            status: EvalStatus::Failed,
            outputs: Vec::new(),
            diagnostics,
            context_delta: None,
            session: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalStatus {
    Complete,
    Failed,
    Cancelled,
    ResourceLimitExceeded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvalContextDelta {
    VariablesUpdated(Vec<VariableSummary>),
    VariablesRemoved(Vec<String>),
    Restarted,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalOutput {
    pub id: Option<EvalOutputId>,
    pub value: EvalValue,
    pub display: EvalDisplayHint,
    pub provenance: EvalOutputProvenance,
}

impl EvalOutput {
    pub fn display(value: EvalValue, source_cell: EvalCellId) -> Self {
        Self {
            id: None,
            value,
            display: EvalDisplayHint::Auto,
            provenance: EvalOutputProvenance::source_cell(source_cell),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EvalValue {
    Unit,
    Bool(bool),
    Number(NumberValue),
    String(String),
    Text(TextValue),
    List(Vec<EvalValue>),
    Function(FunctionValue),
    Expr(MathExpr),
    Attachment(AttachmentValue),
    Bytes(BytesValue),
    Table(TableValue),
    Array(ArrayValue),
    Plot(PlotSpec),
    Graph(GraphSpec),
    Analysis(AnalysisValue),
    Image(ImageRef),
    Diagnostic(EvalDiagnostic),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextValue {
    pub stream: EvalTextStream,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalTextStream {
    Stdout,
    Stderr,
    Display,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NumberValue {
    Int(i64),
    Float(f64),
    Rational { numer: i64, denom: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionValue {
    pub name: Option<String>,
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MathExpr {
    Number(NumberValue),
    Symbol(String),
    String(String),
    Bool(bool),
    Call {
        head: Box<MathExpr>,
        args: Vec<MathExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<MathExpr>,
        rhs: Box<MathExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<MathExpr>,
    },
    List(Vec<MathExpr>),
    Matrix(Vec<Vec<MathExpr>>),
    Relation {
        op: RelationOp,
        lhs: Box<MathExpr>,
        rhs: Box<MathExpr>,
    },
    Function {
        params: Vec<String>,
        body: Box<MathExpr>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentValue {
    pub id: EvalAttachmentId,
    pub display_name: String,
    pub media_type: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BytesValue {
    pub attachment: Option<EvalAttachmentId>,
    pub len: usize,
    pub preview_hex: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableValue {
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrayValue {
    pub shape: Vec<usize>,
    pub values: Vec<NumberValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisValue {
    pub title: String,
    pub reports: Vec<AnalysisReportValue>,
    pub tables: Vec<TableValue>,
    pub plots: Vec<PlotSpec>,
    pub diagnostics: Vec<EvalDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReportValue {
    pub title: String,
    pub values: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    pub path: String,
    pub mime_type: String,
    pub alt_text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalDisplayHint {
    Auto,
    Hidden,
    Text,
    Table,
    Graph,
    Image,
    Diagnostic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalOutputProvenance {
    pub source_cell: Option<EvalCellId>,
    pub produced_at: Option<String>,
    pub run_count: Option<u64>,
    pub input_hash: Option<String>,
    pub attachment_dependencies: Vec<EvalAttachmentId>,
    pub notes: Vec<String>,
}

impl EvalOutputProvenance {
    pub fn source_cell(source_cell: EvalCellId) -> Self {
        Self {
            source_cell: Some(source_cell),
            produced_at: None,
            run_count: None,
            input_hash: None,
            attachment_dependencies: Vec::new(),
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalDiagnostic {
    pub severity: EvalDiagnosticSeverity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub code: Option<String>,
}

impl EvalDiagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: EvalDiagnosticSeverity::Error,
            message: message.into(),
            span: None,
            code: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalDiagnosticSeverity {
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub run_count: u64,
    pub status: SessionStatus,
    pub variables: Vec<VariableSummary>,
}

impl Default for SessionSnapshot {
    fn default() -> Self {
        Self {
            run_count: 0,
            status: SessionStatus::Idle,
            variables: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Idle,
    Running,
    Restarted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableSummary {
    pub name: String,
    pub kind: ValueKind,
    pub preview: String,
    pub source_cell: Option<EvalCellId>,
    pub updated_at_run: Option<u64>,
    pub stale: bool,
    pub size_hint: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalStaleReason {
    SourceEdited,
    EarlierCellEdited { cell_id: EvalCellId },
    EvaluatorRestarted,
    AttachmentChanged { attachment_id: EvalAttachmentId },
    Unknown(String),
}

pub trait RuntimeHost {
    fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentValue, HostError>;

    fn attachment_bytes(&self, attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError>;

    fn attachment_text(&self, attachment: &EvalAttachmentId) -> Result<String, HostError> {
        let bytes = self.attachment_bytes(attachment)?;
        String::from_utf8(bytes).map_err(|err| HostError {
            kind: HostErrorKind::InvalidUtf8,
            message: err.to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostError {
    pub kind: HostErrorKind,
    pub message: String,
}

impl HostError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: HostErrorKind::NotFound,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostErrorKind {
    NotFound,
    PermissionDenied,
    InvalidUtf8,
    UnsupportedMediaType,
    Io,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeHost;

    impl RuntimeHost for FakeHost {
        fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentValue, HostError> {
            if name_or_id == "samples.csv" {
                Ok(AttachmentValue {
                    id: EvalAttachmentId::new("attachment-1"),
                    display_name: "samples.csv".to_string(),
                    media_type: Some("text/csv".to_string()),
                    size_bytes: Some(11),
                })
            } else {
                Err(HostError::not_found("attachment not found"))
            }
        }

        fn attachment_bytes(&self, attachment: &EvalAttachmentId) -> Result<Vec<u8>, HostError> {
            if attachment.0 == "attachment-1" {
                Ok(b"x,y\n1,2\n3,4".to_vec())
            } else {
                Err(HostError::not_found("attachment bytes not found"))
            }
        }
    }

    struct FakeEvaluator {
        snapshot: SessionSnapshot,
    }

    impl FakeEvaluator {
        fn new() -> Self {
            Self {
                snapshot: SessionSnapshot::default(),
            }
        }
    }

    impl Evaluator for FakeEvaluator {
        fn metadata(&self) -> EvaluatorMetadata {
            let mut metadata = EvaluatorMetadata::new("fake", "Fake Evaluator");
            metadata.features.supports_shared_state = true;
            metadata.features.supports_attachments = true;
            metadata
        }

        fn evaluate_cell(&mut self, request: EvalRequest, host: &dyn RuntimeHost) -> EvalResponse {
            self.snapshot.run_count += 1;
            let source_cell = request.cell_id.clone();
            if request.source.contains("attachment") {
                let attachment = host
                    .resolve_attachment("samples.csv")
                    .expect("fake attachment");
                let text = host
                    .attachment_text(&attachment.id)
                    .expect("fake attachment text");
                let table = TableValue {
                    title: Some(attachment.display_name.clone()),
                    columns: vec!["x".to_string(), "y".to_string()],
                    rows: text
                        .lines()
                        .skip(1)
                        .map(|line| line.split(',').map(str::to_string).collect())
                        .collect(),
                    truncated: false,
                };
                self.snapshot.variables.push(VariableSummary {
                    name: "data".to_string(),
                    kind: ValueKind::Table,
                    preview: "2 rows x 2 columns".to_string(),
                    source_cell: Some(source_cell.clone()),
                    updated_at_run: Some(self.snapshot.run_count),
                    stale: false,
                    size_hint: Some("2x2".to_string()),
                });
                return EvalResponse {
                    status: EvalStatus::Complete,
                    outputs: vec![EvalOutput::display(EvalValue::Table(table), source_cell)],
                    diagnostics: Vec::new(),
                    context_delta: Some(EvalContextDelta::VariablesUpdated(
                        self.snapshot.variables.clone(),
                    )),
                    session: Some(self.snapshot.clone()),
                };
            }

            EvalResponse {
                status: EvalStatus::Complete,
                outputs: vec![EvalOutput::display(
                    EvalValue::String("hello".to_string()),
                    source_cell,
                )],
                diagnostics: Vec::new(),
                context_delta: None,
                session: Some(self.snapshot.clone()),
            }
        }

        fn session_snapshot(&self) -> SessionSnapshot {
            self.snapshot.clone()
        }
    }

    #[test]
    fn fake_evaluator_returns_typed_output() {
        let mut evaluator = FakeEvaluator::new();
        let host = FakeHost;
        let response = evaluator.evaluate_cell(
            EvalRequest::new(
                EvalDocumentId::new("notebook-1"),
                EvalCellId::new("cell-1"),
                "print(\"hello\")",
            ),
            &host,
        );

        assert_eq!(response.status, EvalStatus::Complete);
        assert_eq!(response.outputs.len(), 1);
        assert!(matches!(
            response.outputs[0].value,
            EvalValue::String(ref value) if value == "hello"
        ));
    }

    #[test]
    fn fake_evaluator_uses_runtime_host_for_attachments() {
        let mut evaluator = FakeEvaluator::new();
        let host = FakeHost;
        let response = evaluator.evaluate_cell(
            EvalRequest::new(
                EvalDocumentId::new("notebook-1"),
                EvalCellId::new("cell-1"),
                "data = csv(attachment(\"samples.csv\"))",
            ),
            &host,
        );

        assert_eq!(response.status, EvalStatus::Complete);
        assert_eq!(response.outputs.len(), 1);
        assert!(matches!(response.outputs[0].value, EvalValue::Table(_)));
        let session = response.session.expect("session snapshot");
        assert_eq!(session.variables.len(), 1);
        assert_eq!(session.variables[0].name, "data");
        assert_eq!(session.variables[0].kind, ValueKind::Table);
    }

    #[test]
    fn values_and_diagnostics_round_trip_through_json() {
        let output = EvalOutput::display(
            EvalValue::Diagnostic(EvalDiagnostic {
                severity: EvalDiagnosticSeverity::Warning,
                message: "careful".to_string(),
                span: Some(SourceSpan {
                    start: SourcePosition {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                    end: SourcePosition {
                        line: 1,
                        column: 8,
                        offset: 7,
                    },
                }),
                code: Some("W0001".to_string()),
            }),
            EvalCellId::new("cell-1"),
        );

        let json = serde_json::to_string_pretty(&output).expect("serialize output");
        let restored: EvalOutput = serde_json::from_str(&json).expect("deserialize output");
        assert_eq!(
            serde_json::to_value(restored).expect("restored output json"),
            serde_json::to_value(output).expect("original output json")
        );
    }
}
