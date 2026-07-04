use serde::{Deserialize, Serialize};

use crate::{
    RuntimeBindingKind, RuntimeBindingSummary, RuntimeEvaluation, RuntimeSessionSnapshot,
    RuntimeSessionStatus,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInspectionSnapshot {
    pub run_count: u64,
    pub status: RuntimeSessionStatus,
    pub variables: Vec<RuntimeBindingSummary>,
    pub functions: Vec<RuntimeBindingSummary>,
}

impl RuntimeInspectionSnapshot {
    pub fn from_session(snapshot: &RuntimeSessionSnapshot) -> Self {
        let mut variables = Vec::new();
        let mut functions = Vec::new();

        for binding in &snapshot.bindings {
            match binding.binding_kind {
                RuntimeBindingKind::Variable => variables.push(binding.clone()),
                RuntimeBindingKind::Function => functions.push(binding.clone()),
            }
        }

        Self {
            run_count: snapshot.run_count,
            status: snapshot.status,
            variables,
            functions,
        }
    }

    pub fn binding(&self, name: &str) -> Option<&RuntimeBindingSummary> {
        self.variables
            .iter()
            .chain(self.functions.iter())
            .find(|binding| binding.name == name)
    }

    pub fn variable(&self, name: &str) -> Option<&RuntimeBindingSummary> {
        self.variables.iter().find(|binding| binding.name == name)
    }

    pub fn function(&self, name: &str) -> Option<&RuntimeBindingSummary> {
        self.functions.iter().find(|binding| binding.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeDeleteVariableStatus {
    Deleted,
    Unsupported,
    Failed { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeDeleteVariableResult {
    pub name: String,
    pub status: RuntimeDeleteVariableStatus,
    pub evaluation: RuntimeEvaluation,
}

pub fn delete_variable_status(evaluation: &RuntimeEvaluation) -> RuntimeDeleteVariableStatus {
    match evaluation.response.status {
        poincare_evaluator::EvalStatus::Complete => RuntimeDeleteVariableStatus::Deleted,
        _ => {
            let message = evaluation
                .response
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone())
                .unwrap_or_else(|| "delete variable failed".to_string());
            if evaluation
                .response
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("UNSUPPORTED_DELETE_VARIABLE"))
            {
                RuntimeDeleteVariableStatus::Unsupported
            } else {
                RuntimeDeleteVariableStatus::Failed { message }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NotebookCellId, NotebookId, RuntimeRevision, RuntimeSessionId};

    fn binding(name: &str, kind: RuntimeBindingKind) -> RuntimeBindingSummary {
        RuntimeBindingSummary {
            name: name.to_string(),
            binding_kind: kind,
            value_kind: crate::ValueKind::Number,
            preview: "1".to_string(),
            source_cell: Some(NotebookCellId::new("cell-1")),
            updated_at_run: Some(1),
            stale: false,
            size_hint: None,
        }
    }

    #[test]
    fn inspection_snapshot_splits_variables_and_functions() {
        let snapshot = RuntimeSessionSnapshot {
            session_id: RuntimeSessionId::new("session-1"),
            document_id: NotebookId::new("notebook-1"),
            evaluator_language_id: "fake".to_string(),
            evaluator_display_name: "Fake".to_string(),
            revision: RuntimeRevision(1),
            run_count: 2,
            status: RuntimeSessionStatus::Idle,
            bindings: vec![
                binding("a", RuntimeBindingKind::Variable),
                binding("f", RuntimeBindingKind::Function),
            ],
        };

        let inspection = RuntimeInspectionSnapshot::from_session(&snapshot);

        assert_eq!(inspection.variables.len(), 1);
        assert_eq!(inspection.variables[0].name, "a");
        assert_eq!(inspection.functions.len(), 1);
        assert_eq!(inspection.functions[0].name, "f");
        assert_eq!(inspection.binding("a").expect("binding").preview, "1");
    }
}
