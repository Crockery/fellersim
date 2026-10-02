//! Character and APL preparation shared by the offline CLI and hosted workers.
use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;
mod character;
mod stats;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBuild {
    pub schema_version: u32,
    pub hero_id: String,
    pub talent_points: u32,
    pub selected_talent_ids: Vec<String>,
    pub positions: Vec<CharacterPosition>,
    pub disabled_conditional_contribution_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterPosition {
    pub position_id: String,
    pub item: Option<CharacterItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterItem {
    pub item_id: String,
    pub item_level: u32,
    pub rarity: String,
    pub applied_tempers: u32,
    pub rolled_modifiers: Vec<CharacterModifier>,
    pub gems: Vec<CharacterGem>,
    pub trait_tree: Option<CharacterTraitTree>,
    pub blessings: Vec<CharacterBlessing>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterModifier {
    pub slot_id: String,
    pub kind: String,
    pub choice_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterGem {
    pub socket_id: String,
    pub gem_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTraitRoll {
    pub node_id: String,
    pub trait_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTraitTree {
    pub rolls: Vec<CharacterTraitRoll>,
    pub selected_node_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBlessing {
    pub slot_id: String,
    pub blessing_id: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationOptions {
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    #[serde(default = "default_targets")]
    pub targets: u32,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
            code: SimulationErrorCode::DataVersionMismatch,
            message: "Simulation input does not match the bundled game build or input schema."
                .into(),
            sources: vec![],
        });
    }
    if input.run_id.is_empty() || input.run_id.len() > 4096 {
        return Err(invalid("Invalid run ID"));
    }
    if !(MIN_SIMULATION_ITERATIONS..=MAX_SIMULATION_ITERATIONS).contains(&input.options.iterations)
        || !(1..=MAX_STATIONARY_DUMMY_TARGETS).contains(&input.options.targets)
    {
        return Err(invalid(
            "Iterations must be 100–100000 and targets must be 1–20.",
        ));
    }
    if let Some(seed) = &input.options.seed
        && (seed.len() != 16
            || !seed
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
    {
        return Err(invalid(
            "Seed must be sixteen lowercase hexadecimal characters.",
        ));
    }
    character::validate(&input.character)?;
    parse_apl(&input.apl_source)?;
    Ok(())
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
    validate_request(&request)?;
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
