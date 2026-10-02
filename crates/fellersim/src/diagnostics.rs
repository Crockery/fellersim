use crate::{SimulationError, SimulationErrorCode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_DIAGNOSTICS: usize = 100;

/// A machine-readable explanation of an input or execution problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub engine_code: Option<SimulationErrorCode>,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub identifiers: Vec<String>,
    pub help: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            engine_code: None,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            source: None,
            path: None,
            line: None,
            column: None,
            identifiers: vec![],
            help: None,
        }
    }
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl SimulationError {
    pub fn from_diagnostics(code: SimulationErrorCode, mut diagnostics: Vec<Diagnostic>) -> Self {
        let diagnostics_truncated = diagnostics.len() > MAX_DIAGNOSTICS;
        diagnostics.truncate(MAX_DIAGNOSTICS);
        for diagnostic in &mut diagnostics {
            if diagnostic.engine_code.is_none() {
                diagnostic.engine_code = Some(code);
            }
        }
        Self {
            code,
            message: diagnostics
                .first()
                .map(|d| d.message.clone())
                .unwrap_or_default(),
            sources: diagnostics
                .iter()
                .flat_map(|d| d.identifiers.clone())
                .collect(),
            diagnostics,
            diagnostics_truncated,
        }
    }
}
