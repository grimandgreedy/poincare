use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Parse,
    Validation,
    Build,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub component: Option<String>,
    pub row: Option<usize>,
    pub column: Option<usize>,
}

impl DiagnosticLocation {
    pub fn new() -> Self {
        Self {
            component: None,
            row: None,
            column: None,
        }
    }

    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }

    pub fn with_row(mut self, row: usize) -> Self {
        self.row = Some(row);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }
}

impl Default for DiagnosticLocation {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticKind,
    pub message: String,
    pub location: Option<DiagnosticLocation>,
}

impl Diagnostic {
    pub fn error(kind: DiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            kind,
            message: message.into(),
            location: None,
        }
    }

    pub fn warning(kind: DiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            kind,
            message: message.into(),
            location: None,
        }
    }

    pub fn with_location(mut self, location: DiagnosticLocation) -> Self {
        self.location = Some(location);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(location) = &self.location {
            if let Some(component) = &location.component {
                write!(f, "{component}: ")?;
            }
            match (location.row, location.column) {
                (Some(row), Some(column)) => write!(f, "row {row}, col {column}: ")?,
                (Some(row), None) => write!(f, "row {row}: ")?,
                _ => {}
            }
        }
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseDiagnostic(pub Diagnostic);

impl ParseDiagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self(Diagnostic::error(DiagnosticKind::Parse, message))
    }

    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        let location = self
            .0
            .location
            .take()
            .unwrap_or_default()
            .with_component(component);
        self.0.location = Some(location);
        self
    }
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ParseDiagnostic {}

impl From<ParseDiagnostic> for Diagnostic {
    fn from(value: ParseDiagnostic) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationDiagnostic(pub Diagnostic);

impl ValidationDiagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self(Diagnostic::error(DiagnosticKind::Validation, message))
    }

    pub fn with_row(mut self, row: usize) -> Self {
        let location = self.0.location.take().unwrap_or_default().with_row(row);
        self.0.location = Some(location);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        let location = self
            .0
            .location
            .take()
            .unwrap_or_default()
            .with_column(column);
        self.0.location = Some(location);
        self
    }
}

impl fmt::Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ValidationDiagnostic {}

impl From<ValidationDiagnostic> for Diagnostic {
    fn from(value: ValidationDiagnostic) -> Self {
        value.0
    }
}
