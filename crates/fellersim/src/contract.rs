use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub const SIMULATOR_SCHEMA_VERSION: u32 = 18;
pub const ACTION_PRIORITY_LIST_SCHEMA_VERSION: u32 = 2;
pub const STATIONARY_DUMMY_SCENARIO_SCHEMA_VERSION: u32 = 3;
pub const DPS_EVIDENCE_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const ARDEOS_MODEL_VERSION: &str = "ardeos-stationary-dummies-v45";
pub const ARDEOS_SCENARIO_ID: &str = "ardeos-stationary-dummies-300s-v3";
pub const ARDEOS_HERO_ID: &str = "firemage";
pub const ARDEOS_MAX_COMBAT_RANGE_UNITS: f64 = 3_000.0;
pub const RIME_MODEL_VERSION: &str = "rime-stationary-dummies-v31";
pub const RIME_SCENARIO_ID: &str = "rime-stationary-dummies-300s-v3";
pub const RIME_HERO_ID: &str = "rime";
pub const RIME_MAX_COMBAT_RANGE_UNITS: f64 = 3_000.0;
pub const TARIQ_MODEL_VERSION: &str = "tariq-stationary-dummies-v32";
pub const TARIQ_SCENARIO_ID: &str = "tariq-stationary-dummies-300s-v3";
pub const TARIQ_HERO_ID: &str = "ink";
pub const TARIQ_MAX_COMBAT_RANGE_UNITS: f64 = 550.0;
pub const ELARION_MODEL_VERSION: &str = "elarion-stationary-dummies-v36";
pub const ELARION_SCENARIO_ID: &str = "elarion-stationary-dummies-300s-v3";
pub const ELARION_HERO_ID: &str = "bowguy";
pub const ELARION_MAX_COMBAT_RANGE_UNITS: f64 = 3_000.0;
pub const MARA_MODEL_VERSION: &str = "mara-stationary-dummies-v33";
pub const MARA_SCENARIO_ID: &str = "mara-stationary-dummies-300s-v3";
pub const MARA_HERO_ID: &str = "mara";
pub const MARA_MAX_COMBAT_RANGE_UNITS: f64 = 500.0;
pub const GUNDE_MODEL_VERSION: &str = "gunde-stationary-dummies-v31";
pub const GUNDE_SCENARIO_ID: &str = "gunde-stationary-dummies-300s-v3";
pub const GUNDE_HERO_ID: &str = "gunde";
pub const GUNDE_MAX_COMBAT_RANGE_UNITS: f64 = 500.0;
pub const MIN_SIMULATION_ITERATIONS: u32 = 100;
pub const DEFAULT_SIMULATION_ITERATIONS: u32 = 10_000;
pub const MAX_SIMULATION_ITERATIONS: u32 = 100_000;
pub const MAX_SIMULATION_WORKERS: usize = 4;
pub const ENCOUNTER_DURATION_MS: u64 = 300_000;
pub const MIN_STATIONARY_DUMMY_TARGETS: u32 = 1;
pub const MAX_STATIONARY_DUMMY_TARGETS: u32 = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SimulationRequest {
    pub schema_version: u32,
    pub run_id: String,
    pub data_build_id: String,
    pub model_version: String,
    pub profile_fingerprint: String,
    pub hero_id: String,
    pub scenario_id: String,
    pub scenario: StationaryDummyScenarioV3,
    pub evidence: DpsEvidenceSnapshotV1,
    pub iterations: u32,
    /// Lowercase, sixteen-character hexadecimal u64. A string avoids JS precision loss.
    pub seed: String,
    pub profile: NormalizedDpsProfile,
    pub action_priority_list: ActionPriorityListV2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
/// Targets form one co-located stationary group directly in front of the
/// player at the hero's maximum authored combat range. Melee heroes use their
/// maximum melee reach; ranged heroes use their maximum ranged reach.
pub struct StationaryDummyScenarioV3 {
    pub schema_version: u32,
    /// Every target belongs to the same stationary group in front of the
    /// player and inside all source-centered acquisition radii.
    pub target_count: u32,
    /// Distance between the player and every target's reference point in
    /// Unreal units. Each hero contract requires its authored scenario range.
    pub target_distance_units: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum DpsEvidenceScope {
    ArdeosSingleDummy,
    ArdeosStackedDummies,
    RimeSingleDummy,
    RimeStackedDummies,
    TariqSingleDummy,
    TariqStackedDummies,
    ElarionSingleDummy,
    ElarionStackedDummies,
    MaraSingleDummy,
    MaraStackedDummies,
    GundeSingleDummy,
    GundeStackedDummies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum DpsEvidenceClaimStatus {
    Verified,
    Approximate,
    Partial,
    NoImpact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DpsEvidenceClaimSnapshot {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub status: DpsEvidenceClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DpsEvidenceSnapshotV1 {
    pub schema_version: u32,
    pub build_id: String,
    pub model_revision: String,
    pub evidence_fingerprint: String,
    pub scope: DpsEvidenceScope,
    pub claims: Vec<DpsEvidenceClaimSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActionPriorityListV2 {
    pub schema_version: u32,
    pub rules: Vec<AplRule>,
    pub trailing_comments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AplRule {
    pub id: String,
    pub enabled: bool,
    pub ability_id: String,
    pub leading_comments: Vec<String>,
    pub inline_comment: Option<String>,
    pub condition: Option<AplExpressionNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AplExpressionNode {
    pub id: String,
    #[serde(flatten)]
    pub expression: AplExpression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AplExpression {
    All {
        children: Vec<AplExpressionNode>,
    },
    Any {
        children: Vec<AplExpressionNode>,
    },
    Not {
        child: Box<AplExpressionNode>,
    },
    BooleanReference {
        reference: AplBooleanReference,
    },
    Comparison {
        left: AplNumericOperand,
        operator: AplComparisonOperator,
        right: AplNumericOperand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AplBooleanReference {
    CooldownReady { ability_id: String },
    DotActive { ability_id: String },
    BuffActive { buff: AplBuff },
    TargetEffectActive { effect_id: String },
    LegendaryEquipped { item_id: String },
    TalentSelected { talent_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AplNumericOperand {
    Number { value: f64 },
    Reference { reference: AplNumericReference },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AplNumericReference {
    CooldownRemaining {
        ability_id: String,
    },
    CooldownCharges {
        ability_id: String,
    },
    DotRemaining {
        ability_id: String,
    },
    Resource {
        resource: AplResource,
        measure: AplResourceMeasure,
    },
    BuffRemaining {
        buff: AplBuff,
    },
    BuffStacks {
        buff: AplBuff,
    },
    TargetEffectRemaining {
        effect_id: String,
    },
    TargetEffectStacks {
        effect_id: String,
    },
    TargetCount,
    TargetHealthPercent,
    TargetTimeToDie,
    FightElapsed,
    FightRemaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplComparisonOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplResource {
    Cinders,
    Embers,
    Anima,
    WinterOrbs,
    Spirit,
    Fury,
    Focus,
    Energy,
    ComboPoints,
    BloodFeathers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplResourceMeasure {
    Current,
    Max,
    Deficit,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplBuff {
    Wildfire,
    SpiritOfHeroism,
    ApocalypticSurge,
    CascadingInferno,
    IceBlitz,
    WintersBlessing,
    WrathOfWinter,
    FlightOfTheNavir,
    GlacialAssault,
    IcyFlow,
    SoulfrostTorrent,
    HarrowingIce,
    FrostweaversWrath,
    ThunderCall,
    FocusedWrath,
    RagingTempest,
    FarBeyondDriven,
    KillEmAll,
    SquareHammer,
    SquareHammerExpertise,
    SchismHammerStorm,
    SchismSkullCrusher,
    ExecutionersGrin,
    CelestialImpetus,
    EmpoweredMultishot,
    SkystridersGrace,
    EventHorizon,
    SkystridersSupremacy,
    ImpendingHeartseeker,
    ResurgentWinds,
    BroodingShadows,
    MaidenOfDeath,
    MatriarchMacabre,
    AssassinsGuile,
    DeadlyScheme,
    FeedTheQueen,
    MalevolenceArachnid,
    MalevolenceQueen,
    DrenchedInBlood,
    SerratedEdge,
    ReignInBlood,
    BloodboundSpirit,
    DeathsArc,
    GrimHarvest,
    HarvestersToll,
    CrimsonStrikes,
    MurderOfCrows,
    Massacre,
    AncestralInstinct,
    Bloodbath,
    CarrionOnslaught,
    OpenWounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedDpsProfile {
    pub hero_id: String,
    /// Applicable build-bound DPS evidence claims, sorted by claim identifier.
    pub evidence_claim_ids: Vec<String>,
    pub power: f64,
    pub critical_strike: f64,
    pub critical_rating: f64,
    pub critical_multiplier: f64,
    pub expertise: f64,
    pub expertise_rating: f64,
    pub haste: f64,
    pub haste_rating: f64,
    /// Cooldown Recovery is a time-rate attribute whose neutral value is one.
    pub cooldown_recovery: f64,
    pub spirit: f64,
    pub spirit_rating: f64,
    pub max_primary_resource: f64,
    pub max_secondary_resource: u32,
    pub max_spirit: f64,
    pub heroism_haste: f64,
    pub heroism_duration_ms: u64,
    pub abilities: Vec<DpsAbilityModel>,
    /// Target effects that are safe for APL conditions to query at runtime.
    pub apl_target_effects: Vec<AplTargetEffectModel>,
    pub scenario_no_op_abilities: Vec<ScenarioNoOpAbility>,
    pub talents: Vec<DpsTalentModel>,
    pub mechanics: Vec<DynamicMechanicInstance>,
    /// Datamined display names for every duration tracked in public results.
    pub uptime_names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AplTargetEffectModel {
    pub id: String,
    pub name: String,
    pub source: AplTargetEffectSource,
    pub effect_kind: AplTargetEffectKind,
    pub supported_properties: Vec<AplTargetEffectProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AplTargetEffectSource {
    AbilityDot { ability_id: String },
    MechanicTargetBuff { mechanic_instance_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplTargetEffectKind {
    DamageOverTime,
    Debuff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AplTargetEffectProperty {
    Active,
    Remaining,
    Stacks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioNoOpAbility {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum DpsAbilityKind {
    InfernalWave,
    Detonate,
    Apocalypse,
    SearingBlaze,
    EngulfingFlames,
    Incinerate,
    FireFrogs,
    Wildfire,
    Pyromania,
    FireBall,
    IceBlitz,
    BurstingIce,
    FrostBolt,
    GlacialBlast,
    FreezingTorrent,
    WintersBlessing,
    WrathOfWinter,
    ColdSnap,
    IceComet,
    FlightOfTheNavir,
    AnimaSpike,
    FrostSwallow,
    HammerStorm,
    HeavyStrike,
    TariqChainLightning,
    ThunderCall,
    WildSwing,
    FocusedWrath,
    RagingTempest,
    SkullCrusher,
    CullingStrike,
    LeapSmash,
    FaceBreaker,
    TariqAttack,
    Multishot,
    FocusedShot,
    HighwindArrow,
    HeartseekerBarrage,
    LunarlightMark,
    SkystridersGrace,
    ElarionShoot,
    EventHorizon,
    SkystridersSupremacy,
    CelestialShot,
    StarfallVolley,
    LunarlightSalvo,
    LunarlightEruption,
    SkitteringBlades,
    ArachnidAssault,
    MaraAttack,
    HemorrhagingStrike,
    WidowsBite,
    MaidenOfDeath,
    MatriarchMacabre,
    QueensFang,
    BroodingShadows,
    Backstab,
    FinalStratagem,
    CausticPoison,
    SeethingPoison,
    VolatilePoison,
    VolatilePoisonEruption,
    SeethingBurst,
    CorrosiveSpill,
    Hemotoxin,
    HemotoxinEruption,
    DoubleStrike,
    Warbound,
    OwedInBlood,
    BloodboundSpirit,
    ReignInBlood,
    HeartSplitter,
    Rupture,
    Slaughter,
    BloodArc,
    ReaversEdge,
    GundeAttack,
    ButchersHook,
    GrimCarve,
    Rend,
    Exsanguinate,
    Bloodcraze,
    RavensPrecision,
    Oathshatter,
    WeaponFrostVolley,
    WeaponArcaneChannel,
    WeaponChainLightning,
    WeaponShadowMark,
    WeaponCleaveCharge,
    WeaponFrontalCone,
    WeaponInstantAoe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum DpsGcdHasteMode {
    None,
    Standard,
    /// Positive Haste cannot shorten the GCD, but negative Haste can lengthen it.
    SlowOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DpsAbilityModel {
    pub id: String,
    pub name: String,
    pub kind: DpsAbilityKind,
    /// Trigger-only helpers participate in damage attribution but cannot be
    /// selected as player actions.
    pub manually_castable: bool,
    pub power_coefficient: f64,
    /// Full multiplicative spread width. A value of 0.2 rolls from 0.9 to 1.1.
    pub damage_spread: f64,
    pub cast_time_ms: u64,
    pub gcd_ms: u64,
    pub scale_time_with_haste: bool,
    pub gcd_haste_mode: DpsGcdHasteMode,
    pub gcd_scales_with_cooldown_reduction: bool,
    pub gcd_scales_with_cooldown_recovery: bool,
    pub cooldown_ms: u64,
    pub cooldown_scales_with_haste: bool,
    pub cooldown_scales_with_cooldown_recovery: bool,
    pub maximum_charges: u32,
    pub effect_duration_ms: u64,
    pub direct_hits: u32,
    /// One targets the primary dummy. Larger values cap automatic targeting.
    pub max_targets: u32,
    pub first_hit_delay_ms: u64,
    pub hit_interval_ms: u64,
    pub primary_resource_generated: f64,
    pub secondary_resource_cost: u32,
    pub spirit_cost: f64,
    pub channel: Option<ChannelModel>,
    pub dot: Option<DotModel>,
    pub applies_dot_kind: Option<DpsAbilityKind>,
    pub dot_application_delay_ms: u64,
    pub dot_extension_ms: u64,
    pub off_gcd: bool,
    pub mechanic_parameters: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChannelModel {
    pub duration_ms: u64,
    pub tick_interval_ms: u64,
    pub tick_immediately: bool,
    pub scale_duration_with_ability_time_rate: bool,
    pub enable_partial_ticks: bool,
    /// The channel task always reports the final fractional tick. Individual
    /// ability graphs decide whether that factor scales the applied damage.
    pub scale_partial_tick_damage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DotModel {
    pub power_coefficient: f64,
    /// Full multiplicative spread width. Damage-derived Firemage DoTs use zero.
    pub damage_spread: f64,
    pub duration_ms: u64,
    pub period_ms: u64,
    pub can_crit: bool,
    pub cinder_proc_chance: f64,
    pub cinders_on_proc: f64,
    pub stack_damage_increase: f64,
    pub maximum_stacks: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DpsTalentModel {
    pub id: String,
    pub name: String,
    pub mechanic_id: String,
    pub classification: MechanicClassification,
    pub parameters: BTreeMap<String, f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum MechanicClassification {
    Modeled,
    ScenarioNoOp,
    Uncovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum DynamicMechanicHandler {
    StaticStats,
    DamageMultiplier,
    AbilityDamageMultiplier,
    OnHitProc,
    HeroSource,
    ScenarioNoOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMechanicInstance {
    pub instance_id: String,
    pub source_id: String,
    pub source_name: String,
    pub mechanic_id: String,
    pub classification: MechanicClassification,
    pub handler: DynamicMechanicHandler,
    pub ability_kind: Option<DpsAbilityKind>,
    pub parameters: BTreeMap<String, f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SimulationProgress {
    pub run_id: String,
    pub completed_iterations: u32,
    pub total_iterations: u32,
    pub fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SimulationResult {
    pub schema_version: u32,
    pub run_id: String,
    pub data_build_id: String,
    pub model_version: String,
    pub profile_fingerprint: String,
    pub scenario_id: String,
    pub scenario: StationaryDummyScenarioV3,
    pub evidence: DpsEvidenceSnapshotV1,
    pub iterations: u32,
    pub seed: String,
    pub mean_dps: f64,
    pub primary_target_dps: f64,
    pub standard_deviation: f64,
    pub confidence_interval_95: ConfidenceInterval,
    pub abilities: Vec<AbilityDamageResult>,
    pub procs: Vec<ProcResult>,
    pub targets: Vec<TargetDamageResult>,
    pub uptimes: Vec<UptimeResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceInterval {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AbilityDamageResult {
    pub ability_id: String,
    pub ability_name: String,
    pub mean_damage: f64,
    pub mean_dps: f64,
    pub share: f64,
    pub mean_hits: f64,
    pub mean_crits: f64,
    pub mean_grievous_crits: f64,
    pub mean_casts: f64,
    pub mean_targets_hit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcResult {
    pub id: String,
    pub source_id: String,
    pub name: String,
    pub mean_count: f64,
    pub mean_per_minute: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TargetDamageResult {
    pub target_index: u32,
    pub primary: bool,
    pub mean_damage: f64,
    pub mean_dps: f64,
    pub share: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UptimeResult {
    pub id: String,
    pub name: String,
    pub mean_uptime: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum SimulationErrorCode {
    InvalidBuild,
    DataVersionMismatch,
    IncompleteItemConfiguration,
    UnsupportedHero,
    UncoveredMechanics,
    InvalidActionPriorityList,
    Cancelled,
    SimulationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct SimulationError {
    pub code: SimulationErrorCode,
    pub message: String,
    pub sources: Vec<String>,
}

impl SimulationError {
    pub(crate) fn new(code: SimulationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            sources: Vec::new(),
        }
    }
}
