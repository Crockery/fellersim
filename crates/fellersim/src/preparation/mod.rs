//! Character and APL preparation shared by the offline CLI and hosted workers.
use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;
mod character;
mod stats;

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBuild {
    #[schemars(range(min = 6, max = 6))]
    pub schema_version: u32,
    pub hero_id: String,
    #[schemars(range(max = 14))]
    pub talent_points: u32,
    #[schemars(length(max = 32))]
    pub selected_talent_ids: Vec<String>,
    #[schemars(length(min = 14, max = 14))]
    pub positions: Vec<CharacterPosition>,
    #[schemars(length(max = 256))]
    pub disabled_conditional_contribution_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterPosition {
    pub position_id: String,
    pub item: Option<CharacterItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterItem {
    pub item_id: String,
    pub item_level: u32,
    pub rarity: String,
    pub applied_tempers: u32,
    #[schemars(length(max = 64))]
    pub rolled_modifiers: Vec<CharacterModifier>,
    #[schemars(length(max = 4))]
    pub gems: Vec<CharacterGem>,
    pub trait_tree: Option<CharacterTraitTree>,
    #[schemars(length(max = 64))]
    pub blessings: Vec<CharacterBlessing>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterModifier {
    pub slot_id: String,
    pub kind: String,
    pub choice_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterGem {
    pub socket_id: String,
    pub gem_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTraitRoll {
    pub node_id: String,
    pub trait_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTraitTree {
    #[schemars(length(max = 128))]
    pub rolls: Vec<CharacterTraitRoll>,
    #[schemars(length(max = 32))]
    pub selected_node_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBlessing {
    pub slot_id: String,
    pub blessing_id: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationOptions {
    #[serde(default = "default_iterations")]
    #[schemars(range(min = 100, max = 100000))]
    pub iterations: u32,
    #[serde(default = "default_targets")]
    #[schemars(range(min = 1, max = 20))]
    pub targets: u32,
    #[serde(default)]
    #[schemars(regex(pattern = "^[0-9a-f]{16}$"))]
    pub seed: Option<String>,
}
fn default_iterations() -> u32 {
    DEFAULT_SIMULATION_ITERATIONS
}
fn default_targets() -> u32 {
    1
}
impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            iterations: default_iterations(),
            targets: 1,
            seed: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationInput {
    pub schema_version: u32,
    pub run_id: String,
    pub data_build_id: String,
    pub character: CharacterBuild,
    pub apl_source: String,
    pub options: SimulationOptions,
}
pub const INPUT_SCHEMA_VERSION: u32 = 1;

fn invalid(message: impl Into<String>) -> SimulationError {
    SimulationError {
        diagnostics: vec![],
        diagnostics_truncated: false,
        code: SimulationErrorCode::InvalidBuild,
        message: message.into(),
        sources: vec![],
    }
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.0)
}
fn number(v: &Value, key: &str, default: f64) -> f64 {
    v[key].as_f64().unwrap_or(default)
}
fn has(values: &Value, value: &str) -> bool {
    arr(values).iter().any(|v| v.as_str() == Some(value))
}
fn digest(value: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("canonical JSON"))
    )
}

pub fn validate_input(input: &SimulationInput) -> Result<(), SimulationError> {
    if input.schema_version != INPUT_SCHEMA_VERSION
        || input.data_build_id != fellersim_data::build_id()
    {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::DataVersionMismatch,
            message: "Simulation input does not match the bundled game build or input schema."
                .into(),
            sources: vec![],
        });
    }
    let mut diagnostics = character::diagnose(&input.character);
    for d in &mut diagnostics {
        d.path = d.path.take().map(|p| format!("/character/build{p}"));
        d.engine_code = Some(SimulationErrorCode::InvalidBuild);
    }
    diagnostics.extend(diagnose_options(&input.options));
    if input.run_id.is_empty() || input.run_id.len() > 4096 {
        diagnostics.push(
            Diagnostic::error("invalid-run-id", "Run ID must contain 1–4096 bytes.").at("/runId"),
        );
    }
    if let Err(error) = parse_apl(&input.apl_source) {
        diagnostics.extend(error.diagnostics);
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        let code = if diagnostics
            .iter()
            .all(|d| d.engine_code == Some(SimulationErrorCode::InvalidActionPriorityList))
        {
            SimulationErrorCode::InvalidActionPriorityList
        } else {
            SimulationErrorCode::InvalidBuild
        };
        Err(SimulationError::from_diagnostics(code, diagnostics))
    }
}

pub fn diagnose_options(options: &SimulationOptions) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !(MIN_SIMULATION_ITERATIONS..=MAX_SIMULATION_ITERATIONS).contains(&options.iterations) {
        diagnostics.push(
            Diagnostic::error("invalid-iterations", "Iterations must be 100–100000.")
                .at("/options/iterations"),
        );
    }
    if !(1..=MAX_STATIONARY_DUMMY_TARGETS).contains(&options.targets) {
        diagnostics.push(
            Diagnostic::error("invalid-targets", "Targets must be 1–20.").at("/options/targets"),
        );
    }
    if options.seed.as_ref().is_some_and(|s| {
        s.len() != 16
            || !s
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    }) {
        diagnostics.push(
            Diagnostic::error(
                "invalid-seed",
                "Use sixteen lowercase hexadecimal characters.",
            )
            .at("/options/seed"),
        );
    }
    for d in &mut diagnostics {
        d.engine_code = Some(SimulationErrorCode::InvalidBuild);
        d.help = Some("Use describe run for accepted limits and defaults.".into());
    }
    diagnostics
}

pub fn prepare(input: &SimulationInput) -> Result<SimulationRequest, SimulationError> {
    validate_input(input)?;
    let (profile, evidence) = prepare_character(&input.character, input.options.targets)?;
    let hero = HeroIdentity::from_id(&input.character.hero_id)
        .ok_or_else(|| invalid("Unsupported hero"))?
        .contract();
    let scenario = StationaryDummyScenarioV3 {
        schema_version: STATIONARY_DUMMY_SCENARIO_SCHEMA_VERSION,
        target_count: input.options.targets,
        target_distance_units: hero.maximum_combat_range_units,
    };
    let action_priority_list = parse_apl(&input.apl_source)?;
    let mut semantic_apl = serde_json::to_value(&action_priority_list).unwrap();
    fn strip(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for key in ["id", "leadingComments", "inlineComment", "trailingComments"] {
                    fields.remove(key);
                }
                for value in fields.values_mut() {
                    strip(value);
                }
            }
            Value::Array(values) => values.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut semantic_apl);
    let profile_fingerprint = digest(
        &json!({"dataBuildId": input.data_build_id, "modelVersion":hero.model_version,"profile":profile,"evidence":evidence,"scenario":scenario,"apl":semantic_apl}),
    );
    let seed = input.options.seed.clone().unwrap_or_else(|| {
        digest(&json!({"profileFingerprint":profile_fingerprint}))[..16].to_owned()
    });
    let request = SimulationRequest {
        schema_version: SIMULATOR_SCHEMA_VERSION,
        run_id: input.run_id.clone(),
        data_build_id: input.data_build_id.clone(),
        model_version: hero.model_version.into(),
        profile_fingerprint,
        hero_id: input.character.hero_id.clone(),
        scenario_id: hero.scenario_id.into(),
        scenario,
        evidence,
        iterations: input.options.iterations,
        seed,
        profile,
        action_priority_list,
    };
    validate_request(&request).map_err(|mut error| {
        if error.code == SimulationErrorCode::InvalidActionPriorityList
            && let Ok(document) = crate::parse_apl_document(&input.apl_source)
        {
            for diagnostic in &mut error.diagnostics {
                if let Some(location) = document
                    .source_map
                    .iter()
                    .find(|location| diagnostic.identifiers.contains(&location.rule_id))
                {
                    diagnostic.line = Some(location.line);
                    diagnostic.column = Some(location.column);
                }
            }
        }
        error
    })?;
    Ok(request)
}

pub fn prepare_character(
    build: &CharacterBuild,
    targets: u32,
) -> Result<(NormalizedDpsProfile, DpsEvidenceSnapshotV1), SimulationError> {
    character::validate(build)?;
    if !(1..=MAX_STATIONARY_DUMMY_TARGETS).contains(&targets) {
        return Err(invalid("Targets must be 1–20"));
    }
    stats::prepare(build, targets)
}

/// Construct the current planner's complete, unequipped character document.
pub fn empty_character(hero_id: &str) -> Result<CharacterBuild, SimulationError> {
    if fellersim_data::catalog()["heroes"][hero_id].is_null() {
        return Err(invalid("Unsupported hero"));
    }
    Ok(CharacterBuild {
        schema_version: 6,
        hero_id: hero_id.into(),
        talent_points: 14,
        selected_talent_ids: vec![],
        disabled_conditional_contribution_ids: vec![],
        positions: arr(&fellersim_data::catalog()["positions"])
            .iter()
            .map(|p| CharacterPosition {
                position_id: string(p, "id").into(),
                item: None,
            })
            .collect(),
    })
}
pub fn diagnose_character(build: &CharacterBuild) -> Vec<Diagnostic> {
    character::diagnose(build)
}
