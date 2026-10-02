use crate::command::Detail;
use fellersim_core::*;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub const PROTOCOL_VERSION: u32 = 1;
#[derive(Debug)]
pub struct Failure {
    pub exit: u8,
    pub diagnostics: Vec<Diagnostic>,
    pub truncated: bool,
}
impl Failure {
    pub fn input(code: &str, message: impl Into<String>, path: &str) -> Self {
        Self {
            exit: 2,
            diagnostics: vec![Diagnostic::error(code, message).at(path).help(
                "Run fellersim describe for accepted commands, flags, limits, and examples.",
            )],
            truncated: false,
        }
    }
    pub fn source(mut self, path: &Path) -> Self {
        for d in &mut self.diagnostics {
            d.source = Some(path.display().to_string());
        }
        self
    }
    pub fn execution(code: &str, message: impl Into<String>, exit: u8) -> Self {
        Self {
            exit,
            diagnostics: vec![Diagnostic::error(code, message)],
            truncated: false,
        }
    }
}
impl From<SimulationError> for Failure {
    fn from(error: SimulationError) -> Self {
        let exit = match error.code {
            SimulationErrorCode::Cancelled => 130,
            SimulationErrorCode::SimulationFailed => 1,
            _ => 2,
        };
        let diagnostics = if error.diagnostics.is_empty() {
            let mut d = Diagnostic::error(
                serde_json::to_value(error.code).unwrap().as_str().unwrap(),
                error.message,
            );
            d.engine_code = Some(error.code);
            d.identifiers = error.sources;
            d.help = Some(
                "Inspect this build with prepare, or inspect valid selections with catalog.".into(),
            );
            vec![d]
        } else {
            error.diagnostics
        };
        Self {
            exit,
            diagnostics,
            truncated: error.diagnostics_truncated,
        }
    }
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    pub simulator: String,
    pub data_build_id: String,
    pub model_versions: BTreeMap<String, String>,
    pub cli_protocol: u32,
    pub simulation_schema: u32,
}
pub fn versions() -> Versions {
    Versions {
        simulator: env!("CARGO_PKG_VERSION").into(),
        data_build_id: fellersim_data::build_id().into(),
        model_versions: fellersim_data::catalog()["heroes"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, h)| {
                (
                    id.clone(),
                    h["evidence"]["modelRevision"].as_str().unwrap().into(),
                )
            })
            .collect(),
        cli_protocol: PROTOCOL_VERSION,
        simulation_schema: SIMULATOR_SCHEMA_VERSION,
    }
}
/// One response on stdout. All commands, including errors, help, and cancellation use this envelope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response<T: Serialize> {
    #[schemars(range(min = 1, max = 1))]
    pub protocol_version: u32,
    pub ok: bool,
    pub command: String,
    pub data: Option<T>,
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostics_truncated: bool,
    pub versions: Versions,
}
pub struct Outcome {
    pub data: Option<Value>,
    pub failure: Option<Failure>,
    pub artifact: Option<String>,
}
impl Outcome {
    pub fn success(data: impl Serialize) -> Self {
        Self {
            data: Some(serde_json::to_value(data).unwrap()),
            failure: None,
            artifact: None,
        }
    }
    pub fn artifact(document: impl Serialize, raw: Option<String>) -> Self {
        let value = serde_json::to_value(document).unwrap();
        let artifact = raw.unwrap_or_else(|| serde_json::to_string_pretty(&value).unwrap());
        Self {
            data: Some(value),
            failure: None,
            artifact: Some(artifact),
        }
    }
    pub fn failed(failure: Failure) -> Self {
        Self {
            data: None,
            failure: Some(failure),
            artifact: None,
        }
    }
}
pub fn emit(command: &str, json_mode: bool, outcome: Outcome) -> u8 {
    let exit = outcome.failure.as_ref().map_or(0, |f| f.exit);
    if json_mode {
        let failure = outcome.failure.unwrap_or(Failure {
            exit: 0,
            diagnostics: vec![],
            truncated: false,
        });
        let response = Response {
            protocol_version: PROTOCOL_VERSION,
            ok: exit == 0,
            command: command.into(),
            data: outcome.data,
            diagnostics: failure.diagnostics,
            diagnostics_truncated: failure.truncated,
            versions: versions(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&response).expect("response serialization")
        );
    } else {
        if let Some(failure) = outcome.failure {
            for d in failure.diagnostics {
                eprintln!(
                    "{}: {}{}{}",
                    d.code,
                    d.source.map(|s| format!("{s}: ")).unwrap_or_default(),
                    d.message,
                    d.path
                        .filter(|p| !p.is_empty())
                        .map(|p| format!(" ({p})"))
                        .unwrap_or_default()
                );
                if let Some(help) = d.help {
                    eprintln!("  {help}");
                }
            }
            if failure.truncated {
                eprintln!("Further diagnostics omitted after 100 issues.");
            }
        }
        if let Some(raw) = outcome.artifact {
            println!("{raw}");
        } else if let Some(data) = outcome.data {
            human(command, &data);
        }
    }
    exit
}
fn human(command: &str, data: &Value) {
    if command == "run" {
        println!(
            "Mean DPS: {:.2}\n95% confidence interval: {:.2}–{:.2}\nPrimary target DPS: {:.2}\nIterations: {}\nSeed: {}\nModel: {}",
            data["meanDps"].as_f64().unwrap_or(0.),
            data["confidenceInterval95"]["low"].as_f64().unwrap_or(0.),
            data["confidenceInterval95"]["high"].as_f64().unwrap_or(0.),
            data["primaryTargetDps"].as_f64().unwrap_or(0.),
            data["iterations"],
            data["seed"].as_str().unwrap_or(""),
            data["modelVersion"].as_str().unwrap_or("")
        );
        if let Some(limits) = data["modelingLimitations"].as_array()
            && !limits.is_empty()
        {
            println!(
                "Modeling limitations: {} applicable claims; inspect prepare --detail full.",
                limits.len()
            );
        }
        if let Some(abilities) = data["abilities"].as_array() {
            for ability in abilities {
                println!(
                    "  {}: {:.2} DPS",
                    ability["abilityName"].as_str().unwrap_or(""),
                    ability["meanDps"].as_f64().unwrap_or(0.)
                );
            }
        }
    } else if command == "version" {
        println!(
            "fellersim {}\nGame build: {}\nCLI protocol: {}",
            data["simulator"].as_str().unwrap(),
            data["dataBuildId"].as_str().unwrap(),
            PROTOCOL_VERSION
        );
        for (hero, model) in data["modelVersions"].as_object().unwrap() {
            println!("{hero}: {}", model.as_str().unwrap());
        }
    } else if command == "help" {
        println!("{}", data["text"].as_str().unwrap_or(""));
    } else {
        human_value(data, 0);
    }
}
fn human_value(value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if value.is_object() || value.is_array() {
                    println!("{indent}{key}:");
                    human_value(value, depth + 1);
                } else {
                    println!(
                        "{indent}{key}: {}",
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string())
                    );
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                human_value(value, depth);
            }
        }
        Value::String(text) => println!("{indent}{text}"),
        _ => println!("{indent}{value}"),
    }
}
pub fn limitations(evidence: &DpsEvidenceSnapshotV1) -> Vec<DpsEvidenceClaimSnapshot> {
    evidence
        .claims
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                DpsEvidenceClaimStatus::Approximate | DpsEvidenceClaimStatus::Partial
            )
        })
        .cloned()
        .collect()
}

/// Compact metrics and reproducibility metadata; detailed breakdowns are optional.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulationReport {
    pub schema_version: u32,
    pub run_id: String,
    pub data_build_id: String,
    pub model_version: String,
    pub profile_fingerprint: String,
    pub scenario_id: String,
    pub scenario: StationaryDummyScenarioV3,
    pub iterations: u32,
    pub seed: String,
    pub mean_dps: f64,
    pub primary_target_dps: f64,
    pub standard_deviation: f64,
    pub confidence_interval_95: ConfidenceInterval,
    pub evidence_fingerprint: String,
    pub modeling_limitations: Vec<DpsEvidenceClaimSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<DpsEvidenceSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abilities: Option<Vec<AbilityDamageResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procs: Option<Vec<ProcResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<TargetDamageResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptimes: Option<Vec<UptimeResult>>,
}
pub fn result(r: &SimulationResult, detail: Detail) -> Value {
    let full = detail == Detail::Full;
    serde_json::to_value(SimulationReport {
        schema_version: r.schema_version,
        run_id: r.run_id.clone(),
        data_build_id: r.data_build_id.clone(),
        model_version: r.model_version.clone(),
        profile_fingerprint: r.profile_fingerprint.clone(),
        scenario_id: r.scenario_id.clone(),
        scenario: r.scenario.clone(),
        iterations: r.iterations,
        seed: r.seed.clone(),
        mean_dps: r.mean_dps,
        primary_target_dps: r.primary_target_dps,
        standard_deviation: r.standard_deviation,
        confidence_interval_95: r.confidence_interval_95.clone(),
        evidence_fingerprint: r.evidence.evidence_fingerprint.clone(),
        modeling_limitations: limitations(&r.evidence),
        evidence: full.then(|| r.evidence.clone()),
        abilities: full.then(|| r.abilities.clone()),
        procs: full.then(|| r.procs.clone()),
        targets: full.then(|| r.targets.clone()),
        uptimes: full.then(|| r.uptimes.clone()),
    })
    .unwrap()
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    pub data_build_id: String,
    pub model_version: String,
    pub seed: String,
    pub profile_fingerprint: String,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAplReport {
    pub hero_id: String,
    pub source: String,
    pub fingerprint: String,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogReport {
    pub kind: String,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
    pub next_offset: Option<usize>,
    pub entries: Vec<Value>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreparationReport {
    pub hero_id: String,
    pub stats: Value,
    pub talents: Vec<DpsTalentModel>,
    pub effects: Vec<DynamicMechanicInstance>,
    pub available_abilities: Vec<Value>,
    pub target_effects: Vec<AplTargetEffectModel>,
    pub modeling_limitations: Vec<DpsEvidenceClaimSnapshot>,
    pub evidence_fingerprint: String,
    pub rules: Option<Vec<AplRuleExplanation>>,
    pub source_map: Option<Vec<AplSourceLocation>>,
    pub seed: Option<String>,
    pub profile_fingerprint: Option<String>,
    pub scenario: Option<StationaryDummyScenarioV3>,
    pub profile: Option<NormalizedDpsProfile>,
    pub evidence: Option<DpsEvidenceSnapshotV1>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceReport {
    pub iteration_index: u32,
    pub source_map: Vec<AplSourceLocation>,
    pub rules: Vec<AplRuleExplanation>,
    pub decisions: Vec<TraceDecision>,
    pub truncated: bool,
    pub capture: CaptureLimits,
    pub result: SimulationReport,
    pub work_units: u32,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLimits {
    pub seconds: u32,
    pub max_decisions: u32,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CaseStatus {
    Invalid,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    NotStarted,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaseReport {
    pub id: String,
    pub status: CaseStatus,
    pub result: Option<SimulationReport>,
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostics_truncated: bool,
    pub comparison: Option<PairedComparison>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport {
    pub cases: Vec<CaseReport>,
    pub baseline: Option<String>,
    pub metric: Option<ComparisonMetric>,
    pub family_confidence: Option<f64>,
    pub requested_comparisons: Option<usize>,
    pub uncertainty: Option<String>,
}

/// Progress is advisory: a closed stderr pipe must not abort completed work.
pub fn progress(message: std::fmt::Arguments<'_>) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{message}");
}
