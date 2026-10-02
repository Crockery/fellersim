use std::{collections::BTreeMap, sync::Arc};

use crate::*;

#[derive(Debug, Clone, Default)]
pub(super) struct CompiledParameters {
    pub(super) values: BTreeMap<ParameterKey, f64>,
}

impl CompiledParameters {
    fn compile(values: &BTreeMap<String, f64>) -> Result<Self, SimulationError> {
        let mut compiled = BTreeMap::new();
        let mut names_by_key = BTreeMap::new();
        for (name, value) in values {
            let key = ParameterKey::new(name);
            if let Some(previous) = names_by_key.insert(key, name.as_str())
                && previous != name
            {
                return Err(SimulationError::new(
                    SimulationErrorCode::InvalidBuild,
                    "extracted mechanic parameter identifiers must be collision-free",
                ));
            }
            compiled.insert(key, *value);
        }
        Ok(Self { values: compiled })
    }

    pub(super) fn get(&self, key: ParameterKey) -> Option<f64> {
        self.values.get(&key).copied()
    }

    pub(super) fn value(&self, key: ParameterKey) -> f64 {
        self.get(key).unwrap_or(0.0)
    }

    pub(super) fn contains(&self, key: ParameterKey) -> bool {
        self.values.contains_key(&key)
    }

    fn require(
        &self,
        source_id: &str,
        source_name: &str,
        names: &[&str],
    ) -> Result<(), SimulationError> {
        let missing = names
            .iter()
            .filter(|name| !self.contains(ParameterKey::new(name)))
            .copied()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!(
                    "{source_name} is missing extracted parameters: {}",
                    missing.join(", ")
                ),
                sources: vec![source_id.to_owned()],
            })
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CompiledAbility {
    pub(super) source: DpsAbilityModel,
    pub(super) parameters: CompiledParameters,
    pub(super) damage_source: DamageSourceKey,
}

impl CompiledAbility {
    pub(super) fn compile(source: &DpsAbilityModel) -> Result<Self, SimulationError> {
        let parameters = CompiledParameters::compile(&source.mechanic_parameters)?;
        let mut source = source.clone();
        source.mechanic_parameters.clear();
        Ok(Self {
            source,
            parameters,
            damage_source: DamageSourceKey::INVALID,
        })
    }
}

impl std::ops::Deref for CompiledAbility {
    type Target = DpsAbilityModel;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl std::ops::DerefMut for CompiledAbility {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.source
    }
}

#[derive(Debug, Clone)]
pub(super) struct CompiledTalent {
    pub(super) source: DpsTalentModel,
    pub(super) parameters: CompiledParameters,
    pub(super) kind: CompiledTalentKind,
}

impl CompiledTalent {
    fn compile(source: &DpsTalentModel) -> Result<Self, SimulationError> {
        let parameters = CompiledParameters::compile(&source.parameters)?;
        let kind = CompiledTalentKind::from_id(&source.id).ok_or_else(|| SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!("{} has no typed runtime talent configuration", source.id),
            sources: vec![source.id.clone()],
        })?;
        let mut source = source.clone();
        source.parameters.clear();
        Ok(Self {
            source,
            parameters,
            kind,
        })
    }
}

impl std::ops::Deref for CompiledTalent {
    type Target = DpsTalentModel;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CompiledTalentKind {
    Ardeos(u8),
    Rime(u8),
    Tariq(u8),
    Elarion(u8),
    Mara(u8),
    Gunde(u8),
}

impl CompiledTalentKind {
    fn from_id(id: &str) -> Option<Self> {
        let (hero, number) = if let Some(number) = id.strip_prefix("firemage-talent-id-talent") {
            (HeroIdentity::Ardeos, number)
        } else if let Some(number) = id.strip_prefix("rime-talent-id-talent") {
            (HeroIdentity::Rime, number)
        } else if let Some(number) = id.strip_prefix("ink-talent-id-talent") {
            (HeroIdentity::Tariq, number)
        } else if let Some(number) = id.strip_prefix("bowguy-talent-id-talent") {
            (HeroIdentity::Elarion, number)
        } else if let Some(number) = id.strip_prefix("mara-talent-id-talent") {
            (HeroIdentity::Mara, number)
        } else {
            let number = id.strip_prefix("gunde-talent-id-talent")?;
            (HeroIdentity::Gunde, number)
        };
        let number = number.parse::<u8>().ok()?;
        let current = match hero {
            HeroIdentity::Mara => (1..=19).contains(&number) && number != 7,
            _ => (1..=18).contains(&number),
        };
        if !current {
            return None;
        }
        Some(match hero {
            HeroIdentity::Ardeos => Self::Ardeos(number),
            HeroIdentity::Rime => Self::Rime(number),
            HeroIdentity::Tariq => Self::Tariq(number),
            HeroIdentity::Elarion => Self::Elarion(number),
            HeroIdentity::Mara => Self::Mara(number),
            HeroIdentity::Gunde => Self::Gunde(number),
        })
    }

    fn hero(self) -> HeroIdentity {
        match self {
            Self::Ardeos(_) => HeroIdentity::Ardeos,
            Self::Rime(_) => HeroIdentity::Rime,
            Self::Tariq(_) => HeroIdentity::Tariq,
            Self::Elarion(_) => HeroIdentity::Elarion,
            Self::Mara(_) => HeroIdentity::Mara,
            Self::Gunde(_) => HeroIdentity::Gunde,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TraitHeroSourceKind {
    AbilityToIncreasedMainStat,
    CommitToHasteRatingAndWeaponCooldown,
    CooldownRecoveryOnWeaponAbility,
    CritsToIncreasedCritRating,
    CritsToIncreasedPrimaryStatBuff,
    ExtraDotHotOnEffectApplicationProc,
    GemCooldownRecoveryOnAbilityProc,
    GemDotHotOnCrit,
    GemPulsatingOnAbilityTotemProc,
    GemSingleTargetProcOnDamageHeal,
    GemTargetedSpikeProc,
    GemWhirlwindProc,
    IncreasedMainStatAndSpiritRating,
    OffensiveAbilityHasteRatingStacking,
    OffensiveAbilityToHighestStatBuff,
    StandingStillStaminaExpertiseRatingIncrease,
    WeaponAndSpiritPoints,
    WeaponCritChanceCooldownReduction,
    WeaponDamageReductionPrimaryStatIncrease,
    WeaponHealDamageIncrease,
    Wolf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GemHeroSourceKind {
    EmeraldExpertise,
    RubyPeriodicHealing,
    SapphireHeroismPower,
    SapphireSpiritEfficiency,
    TopazIdleHeroismHaste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LegendaryHeroSourceKind {
    Base,
    FireToad,
    EngulfingFlames,
    ApocalypseDot,
    RimeStartingResources,
    Frostwyrm,
    BurstingIce,
    TariqThunderingVortex,
    TariqSlayersMosh,
    TariqExecutionersGrin,
    ElarionStarstrikersAscent,
    ElarionShimmer,
    ElarionAstronomersHail,
    MaraDrenchedInBlood,
    MaraArachnidPoison,
    MaraArachnidClone,
    GundeBleedingHeartsHeart,
    GundeBloodsoakedCleaver,
    GundeCarrionOnslaught,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetHeroSourceKind {
    PowerOnCrit,
    HasteOnAbility,
    HeroismPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeroSourceKind {
    Finesse(u8),
    Weapon,
    Gem(GemHeroSourceKind),
    Legendary(LegendaryHeroSourceKind),
    Set(SetHeroSourceKind),
    Trait(TraitHeroSourceKind),
    #[cfg(test)]
    TestOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompiledMechanicKind {
    StaticStats,
    DamageMultiplier,
    AbilityDamageMultiplier,
    OnHitProc,
    HeroSource(HeroSourceKind),
    ScenarioNoOp,
}

#[derive(Debug, Clone)]
pub(super) struct CompiledMechanic {
    pub(super) source: DynamicMechanicInstance,
    pub(super) parameters: CompiledParameters,
    pub(super) kind: CompiledMechanicKind,
    pub(super) damage_source: DamageSourceKey,
    pub(super) index: usize,
}

impl CompiledMechanic {
    fn compile(source: &DynamicMechanicInstance) -> Result<Self, SimulationError> {
        let parameters = CompiledParameters::compile(&source.parameters)?;
        let kind = compiled_mechanic_kind(source)?;
        validate_compiled_mechanic_parameters(source, kind, &parameters)?;
        let mut source = source.clone();
        source.parameters.clear();
        Ok(Self {
            source,
            parameters,
            kind,
            damage_source: DamageSourceKey::INVALID,
            index: usize::MAX,
        })
    }

    #[cfg(test)]
    pub(super) fn compile_test(source: &DynamicMechanicInstance) -> Result<Self, SimulationError> {
        let parameters = CompiledParameters::compile(&source.parameters)?;
        let kind = compiled_mechanic_kind(source).unwrap_or(match source.handler {
            DynamicMechanicHandler::StaticStats => CompiledMechanicKind::StaticStats,
            DynamicMechanicHandler::DamageMultiplier => CompiledMechanicKind::DamageMultiplier,
            DynamicMechanicHandler::AbilityDamageMultiplier => {
                CompiledMechanicKind::AbilityDamageMultiplier
            }
            DynamicMechanicHandler::OnHitProc => CompiledMechanicKind::OnHitProc,
            DynamicMechanicHandler::HeroSource => {
                CompiledMechanicKind::HeroSource(HeroSourceKind::TestOnly)
            }
            DynamicMechanicHandler::ScenarioNoOp => CompiledMechanicKind::ScenarioNoOp,
        });
        let mut source = source.clone();
        source.parameters.clear();
        Ok(Self {
            source,
            parameters,
            kind,
            damage_source: DamageSourceKey::INVALID,
            index: usize::MAX,
        })
    }
}

impl std::ops::Deref for CompiledMechanic {
    type Target = DynamicMechanicInstance;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

fn compiled_mechanic_kind(
    source: &DynamicMechanicInstance,
) -> Result<CompiledMechanicKind, SimulationError> {
    let kind = match source.classification {
        MechanicClassification::ScenarioNoOp => CompiledMechanicKind::ScenarioNoOp,
        MechanicClassification::Uncovered => {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!("{} has no executable runtime variant", source.source_name),
                sources: vec![source.source_id.clone()],
            });
        }
        MechanicClassification::Modeled => match source.handler {
            DynamicMechanicHandler::StaticStats => CompiledMechanicKind::StaticStats,
            DynamicMechanicHandler::DamageMultiplier => CompiledMechanicKind::DamageMultiplier,
            DynamicMechanicHandler::AbilityDamageMultiplier => {
                CompiledMechanicKind::AbilityDamageMultiplier
            }
            DynamicMechanicHandler::OnHitProc => CompiledMechanicKind::OnHitProc,
            DynamicMechanicHandler::HeroSource => {
                CompiledMechanicKind::HeroSource(hero_source_kind(source).ok_or_else(|| {
                    SimulationError {
                        diagnostics: vec![],
                        diagnostics_truncated: false,
                        code: SimulationErrorCode::UncoveredMechanics,
                        message: format!(
                            "{} has no typed runtime hero-source variant",
                            source.source_name
                        ),
                        sources: vec![source.source_id.clone()],
                    }
                })?)
            }
            DynamicMechanicHandler::ScenarioNoOp => {
                return Err(SimulationError {
                    diagnostics: vec![],
                    diagnostics_truncated: false,
                    code: SimulationErrorCode::InvalidBuild,
                    message: format!(
                        "{} is modeled but has no executable handler",
                        source.source_name
                    ),
                    sources: vec![source.source_id.clone()],
                });
            }
        },
    };
    Ok(kind)
}

fn hero_source_kind(source: &DynamicMechanicInstance) -> Option<HeroSourceKind> {
    if let Some(number) = source
        .source_id
        .strip_prefix("DynamicItemAbilityRank.")
        .and_then(|number| number.parse::<u8>().ok())
        .filter(|number| (1..=14).contains(number) && *number != 3)
    {
        return Some(HeroSourceKind::Finesse(number));
    }
    let trait_kind = match source.source_id.as_str() {
        "ItemTrait.ID.AbilityToIncreasedMainStat" => {
            TraitHeroSourceKind::AbilityToIncreasedMainStat
        }
        "ItemTrait.ID.CommitToHasteRatingAndWeaponCooldown" => {
            TraitHeroSourceKind::CommitToHasteRatingAndWeaponCooldown
        }
        "ItemTrait.ID.CooldownRecoveryOnWeaponAbility" => {
            TraitHeroSourceKind::CooldownRecoveryOnWeaponAbility
        }
        "ItemTrait.ID.CritsToIncreasedCritRating" => {
            TraitHeroSourceKind::CritsToIncreasedCritRating
        }
        "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff" => {
            TraitHeroSourceKind::CritsToIncreasedPrimaryStatBuff
        }
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc" => {
            TraitHeroSourceKind::ExtraDotHotOnEffectApplicationProc
        }
        "ItemTrait.ID.GemCooldownRecoveryOnAbilityProc" => {
            TraitHeroSourceKind::GemCooldownRecoveryOnAbilityProc
        }
        "ItemTrait.ID.GemDotHotOnCrit" => TraitHeroSourceKind::GemDotHotOnCrit,
        "ItemTrait.ID.GemPulsatingOnAbilityTotemProc" => {
            TraitHeroSourceKind::GemPulsatingOnAbilityTotemProc
        }
        "ItemTrait.ID.GemSingleTargetProcOnDamageHeal" => {
            TraitHeroSourceKind::GemSingleTargetProcOnDamageHeal
        }
        "ItemTrait.ID.GemTargetedSpikeProc" => TraitHeroSourceKind::GemTargetedSpikeProc,
        "ItemTrait.ID.GemWhirlwindProc" => TraitHeroSourceKind::GemWhirlwindProc,
        "ItemTrait.ID.IncreasedMainStatAndSpiritRating" => {
            TraitHeroSourceKind::IncreasedMainStatAndSpiritRating
        }
        "ItemTrait.ID.OffensiveAbilityHasteRatingStacking" => {
            TraitHeroSourceKind::OffensiveAbilityHasteRatingStacking
        }
        "ItemTrait.ID.OffensiveAbilityToHighestStatBuff" => {
            TraitHeroSourceKind::OffensiveAbilityToHighestStatBuff
        }
        "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease" => {
            TraitHeroSourceKind::StandingStillStaminaExpertiseRatingIncrease
        }
        "ItemTrait.ID.WeaponAndSpiritPoints" => TraitHeroSourceKind::WeaponAndSpiritPoints,
        "ItemTrait.ID.WeaponCritChanceCooldownReduction" => {
            TraitHeroSourceKind::WeaponCritChanceCooldownReduction
        }
        "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease" => {
            TraitHeroSourceKind::WeaponDamageReductionPrimaryStatIncrease
        }
        "ItemTrait.ID.WeaponHealDamageIncrease" => TraitHeroSourceKind::WeaponHealDamageIncrease,
        "ItemTrait.ID.Wolf" => TraitHeroSourceKind::Wolf,
        _ => {
            if source.source_id.starts_with("weapon-") {
                return Some(HeroSourceKind::Weapon);
            }
            if source.source_id.starts_with("legendary-") {
                let kind = if source.parameters.contains_key("startingSpirit")
                    || source.source_id.contains("rime-trait1")
                {
                    LegendaryHeroSourceKind::RimeStartingResources
                } else if source.parameters.contains_key("frostwyrmMaximumStacks")
                    || source.source_id.contains("rime-trait2")
                {
                    LegendaryHeroSourceKind::Frostwyrm
                } else if source
                    .parameters
                    .contains_key("burstingDurationIncreaseSeconds")
                    || source.source_id.contains("rime-trait3")
                {
                    LegendaryHeroSourceKind::BurstingIce
                } else if source.parameters.contains_key("fireToadSpawnChance") {
                    LegendaryHeroSourceKind::FireToad
                } else if source
                    .parameters
                    .contains_key("engulfingTargetIncomingMultiplier")
                {
                    LegendaryHeroSourceKind::EngulfingFlames
                } else if source.parameters.contains_key("apocalypseDotFraction") {
                    LegendaryHeroSourceKind::ApocalypseDot
                } else if source.parameters.contains_key("vortexStacksRequired") {
                    LegendaryHeroSourceKind::TariqThunderingVortex
                } else if source.parameters.contains_key("leapTargetDamageMultiplier") {
                    LegendaryHeroSourceKind::TariqSlayersMosh
                } else if source.parameters.contains_key("executionersGrinProcChance") {
                    LegendaryHeroSourceKind::TariqExecutionersGrin
                } else if source.parameters.contains_key("starstrikerProcChance") {
                    LegendaryHeroSourceKind::ElarionStarstrikersAscent
                } else if source.parameters.contains_key("shimmerDamageMultiplier") {
                    LegendaryHeroSourceKind::ElarionShimmer
                } else if source
                    .parameters
                    .contains_key("starfallMainTargetDamageMultiplier")
                {
                    LegendaryHeroSourceKind::ElarionAstronomersHail
                } else if source.parameters.contains_key("spiritRefundExpertise") {
                    LegendaryHeroSourceKind::MaraDrenchedInBlood
                } else if source
                    .parameters
                    .contains_key("arachnidPoisonDamageFraction")
                {
                    LegendaryHeroSourceKind::MaraArachnidPoison
                } else if source.parameters.contains_key("fromShadowsProcChance") {
                    LegendaryHeroSourceKind::MaraArachnidClone
                } else if source.parameters.contains_key("heartSplitterCharges") {
                    LegendaryHeroSourceKind::GundeBleedingHeartsHeart
                } else if source.parameters.contains_key("grimCarveAdditionalSpins") {
                    LegendaryHeroSourceKind::GundeBloodsoakedCleaver
                } else if source.parameters.contains_key("carrionDamageMultiplier") {
                    LegendaryHeroSourceKind::GundeCarrionOnslaught
                } else {
                    LegendaryHeroSourceKind::Base
                };
                return Some(HeroSourceKind::Legendary(kind));
            }
            if source.source_id.starts_with("gem-") {
                let kind = if matches!(source.source_id.as_str(), "gem-ruby-220" | "gem-ruby-1000")
                {
                    GemHeroSourceKind::RubyPeriodicHealing
                } else if source.parameters.contains_key("expertise") {
                    GemHeroSourceKind::EmeraldExpertise
                } else if source.parameters.contains_key("maxSpiritAdd") {
                    GemHeroSourceKind::SapphireHeroismPower
                } else if source.parameters.contains_key("spiritCostMultiplier") {
                    GemHeroSourceKind::SapphireSpiritEfficiency
                } else if source.parameters.contains_key("idleHeroismHaste") {
                    GemHeroSourceKind::TopazIdleHeroismHaste
                } else {
                    return None;
                };
                return Some(HeroSourceKind::Gem(kind));
            }
            return Some(HeroSourceKind::Set(match source.source_id.as_str() {
                "seta-proc-intellect" | "seta-proc-strength" | "seta-proc-agility" => {
                    SetHeroSourceKind::PowerOnCrit
                }
                "setb-proc-hdt" => SetHeroSourceKind::HasteOnAbility,
                "setd-proc-intellect" | "setd-proc-strength" | "setd-proc-agility" => {
                    SetHeroSourceKind::HeroismPower
                }
                _ => return None,
            }));
        }
    };
    Some(HeroSourceKind::Trait(trait_kind))
}

fn validate_compiled_mechanic_parameters(
    source: &DynamicMechanicInstance,
    kind: CompiledMechanicKind,
    parameters: &CompiledParameters,
) -> Result<(), SimulationError> {
    let required: &[&str] = match kind {
        CompiledMechanicKind::StaticStats | CompiledMechanicKind::ScenarioNoOp => &[],
        CompiledMechanicKind::DamageMultiplier | CompiledMechanicKind::AbilityDamageMultiplier => {
            &["damageIncrease"]
        }
        CompiledMechanicKind::OnHitProc => &[],
        CompiledMechanicKind::HeroSource(HeroSourceKind::Weapon) => &[],
        CompiledMechanicKind::HeroSource(HeroSourceKind::Finesse(number)) => match number {
            1 => &["maximumStacks", "damageIncreasePerStack"],
            2 => &["criticalStrikeBonus", "durationSeconds"],
            4 => &["procChance", "spiritGain"],
            5 => &["criticalStrikeBonus", "criticalPowerMultiplier"],
            6 => &[
                "hitThreshold",
                "criticalStrikeCap",
                "radius",
                "targetCountDamageScalingThreshold",
            ],
            7 => &["powerCoefficient", "maximumTargets", "radius"],
            8 => &[
                "durationSeconds",
                "statIncreasePerSpirit",
                "spiritPerIncrease",
                "spiritCap",
            ],
            9 => &["powerCoefficient", "periodicPowerCoefficient"],
            10 => &["damageIncreasePerSpiritAmount", "spiritAmount", "spiritCap"],
            11 => &[
                "cooldownAcceleration",
                "accelerationPerHaste",
                "hasteThreshold",
            ],
            12 => &[
                "hasteBonus",
                "durationSeconds",
                "intervalSeconds",
                "coreCooldownDenominatorSeconds",
                "coreCooldownFractionMultiplier",
                "minimumReductionSeconds",
            ],
            13 => &["startingSpirit"],
            14 => &["procChance", "powerCoefficient", "radius"],
            _ => unreachable!("hero source classifier admits current Finesse ranks"),
        },
        CompiledMechanicKind::HeroSource(HeroSourceKind::Gem(kind)) => match kind {
            GemHeroSourceKind::RubyPeriodicHealing => &["healingHealthFraction", "periodSeconds"],
            GemHeroSourceKind::EmeraldExpertise => &["expertise", "durationSeconds"],
            GemHeroSourceKind::SapphireHeroismPower => &["maxSpiritAdd", "heroismPowerMultiplier"],
            GemHeroSourceKind::SapphireSpiritEfficiency => {
                &["spiritCostMultiplier", "heroismDurationSeconds"]
            }
            GemHeroSourceKind::TopazIdleHeroismHaste => &["idleHeroismHaste"],
        },
        CompiledMechanicKind::HeroSource(HeroSourceKind::Legendary(kind)) => match kind {
            LegendaryHeroSourceKind::Base => &["powerMultiplier", "cooldownAccelerationMultiplier"],
            LegendaryHeroSourceKind::FireToad => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "fireToadSpawnChance",
                "fireToadDamageMultiplier",
                "fireToadAoeDamageMultiplier",
                "fireToadTargetCountThreshold",
                "fireToadBonusCount",
            ],
            LegendaryHeroSourceKind::EngulfingFlames => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "engulfingTargetIncomingMultiplier",
            ],
            LegendaryHeroSourceKind::ApocalypseDot => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "apocalypseDotFraction",
                "apocalypseDotDurationSeconds",
                "apocalypseDotPeriodSeconds",
            ],
            LegendaryHeroSourceKind::RimeStartingResources => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "startingSpirit",
                "wintersBlessingCharges",
                "undulatingSpiritDurationSeconds",
            ],
            LegendaryHeroSourceKind::Frostwyrm => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "frostwyrmDurationSeconds",
                "frostwyrmMaximumStacks",
                "frostwyrmDamageIncreasePerStack",
                "frostwyrmTargetsPerStack",
                "frostwyrmTargetCountThreshold",
            ],
            LegendaryHeroSourceKind::BurstingIce => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "burstingDurationIncreaseSeconds",
            ],
            LegendaryHeroSourceKind::TariqThunderingVortex => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "chainFuryMultiplier",
                "vortexStacksRequired",
                "vortexMaximumStacks",
                "vortexDamageMultiplier",
            ],
            LegendaryHeroSourceKind::TariqSlayersMosh => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "leapTargetDamageMultiplier",
                "leapTargetDamageDurationSeconds",
            ],
            LegendaryHeroSourceKind::TariqExecutionersGrin => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "executionersGrinProcChance",
                "executionersGrinMaximumStacks",
                "executionersGrinDurationSeconds",
                "executionersGrinFurySpent",
            ],
            LegendaryHeroSourceKind::ElarionStarstrikersAscent => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "starstrikerProcChance",
                "impendingHeartseekerDurationSeconds",
            ],
            LegendaryHeroSourceKind::ElarionShimmer => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "shimmerDamageMultiplier",
                "shimmerDurationSeconds",
                "shimmerMaximumStacks",
            ],
            LegendaryHeroSourceKind::ElarionAstronomersHail => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "starfallMainTargetDamageMultiplier",
                "starfallDurationIncreaseSeconds",
                "starfallExtensionPerMultishotSeconds",
            ],
            LegendaryHeroSourceKind::MaraDrenchedInBlood => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "spiritRefundExpertise",
                "spiritRefundExpertiseDurationSeconds",
            ],
            LegendaryHeroSourceKind::MaraArachnidPoison => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "arachnidPoisonDamageFraction",
                "arachnidPoisonDurationSeconds",
                "arachnidPoisonPeriodSeconds",
            ],
            LegendaryHeroSourceKind::MaraArachnidClone => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "fromShadowsProcChance",
                "fromShadowsBleedPeriodMultiplier",
                "fromShadowsDelaySeconds",
            ],
            LegendaryHeroSourceKind::GundeBleedingHeartsHeart => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "heartSplitterCharges",
                "heartSplitterAdditionalStrikeChance",
                "heartSplitterAdditionalStrikeDelaySeconds",
                "heartSplitterHealthyCriticalStrikeBonus",
            ],
            LegendaryHeroSourceKind::GundeBloodsoakedCleaver => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "grimCarveAdditionalSpins",
            ],
            LegendaryHeroSourceKind::GundeCarrionOnslaught => &[
                "powerMultiplier",
                "cooldownAccelerationMultiplier",
                "carrionDamageMultiplier",
                "carrionAdditionalDamagePerFeather",
                "carrionDurationSeconds",
                "carrionFeathersPerPulse",
                "carrionPulsePeriodSeconds",
                "carrionStacksPerFeather",
            ],
        },
        CompiledMechanicKind::HeroSource(HeroSourceKind::Set(kind)) => match kind {
            SetHeroSourceKind::PowerOnCrit => &[
                "procsPerMinute",
                "durationSeconds",
                "powerMultiplier",
                "cooldownSeconds",
                "ppmCriticalScaling",
            ],
            SetHeroSourceKind::HasteOnAbility => &[
                "procsPerMinute",
                "durationSeconds",
                "haste",
                "cooldownSeconds",
            ],
            SetHeroSourceKind::HeroismPower => &["heroismPowerMultiplier", "durationSeconds"],
        },
        CompiledMechanicKind::HeroSource(HeroSourceKind::Trait(kind)) => match kind {
            TraitHeroSourceKind::AbilityToIncreasedMainStat => &[
                "powerMultiplier",
                "durationSeconds",
                "procsPerMinute",
                "requiredStacks",
                "stackDurationSeconds",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::CommitToHasteRatingAndWeaponCooldown => &[
                "hasteRating",
                "durationSeconds",
                "procsPerMinute",
                "weaponCooldownReductionSeconds",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::CooldownRecoveryOnWeaponAbility => &[
                "cooldownAccelerationMultiplier",
                "durationWeaponCooldownFraction",
            ],
            TraitHeroSourceKind::CritsToIncreasedCritRating => &[
                "criticalRating",
                "durationSeconds",
                "requiredCriticalStrikes",
            ],
            TraitHeroSourceKind::CritsToIncreasedPrimaryStatBuff => &[
                "powerMultiplier",
                "durationSeconds",
                "procsPerMinute",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::ExtraDotHotOnEffectApplicationProc => &[
                "powerCoefficientPerTick",
                "healingPowerCoefficientPerTick",
                "healingDurationSeconds",
                "healingPeriodSeconds",
                "procsPerMinute",
                "durationSeconds",
                "periodSeconds",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::GemCooldownRecoveryOnAbilityProc => &[
                "cooldownAccelerationMultiplier",
                "durationSeconds",
                "cooldownSeconds",
                "procsPerMinute",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::GemDotHotOnCrit => {
                &["triggerDamageFraction", "durationSeconds", "periodSeconds"]
            }
            TraitHeroSourceKind::GemPulsatingOnAbilityTotemProc => &[
                "damageAccumulationFraction",
                "initialDamagePulseDelaySeconds",
                "pulseIntervalSeconds",
            ],
            TraitHeroSourceKind::GemSingleTargetProcOnDamageHeal => &[
                "powerCoefficient",
                "procsPerMinute",
                "debuffDurationSeconds",
                "damageIncreasePerStack",
                "harmoniousSoulDamageIncreasePerStack",
                "ppmHasteScaling",
                "harmoniousSoulStacks",
                "maximumStacks",
            ],
            TraitHeroSourceKind::GemTargetedSpikeProc => {
                &["powerCoefficient", "procsPerMinute", "ppmHasteScaling"]
            }
            TraitHeroSourceKind::GemWhirlwindProc => &[
                "flatDamage",
                "gemPowerDamageIncreasePerPoint",
                "procsPerMinute",
                "lifetimeSeconds",
                "movementSpeed",
                "collisionRadius",
                "oscillationAmplitude",
                "oscillationFrequencyDegreesPerSecond",
                "ppmHasteScaling",
            ],
            TraitHeroSourceKind::IncreasedMainStatAndSpiritRating => {
                &["spiritRating", "powerMultiplier", "durationSeconds"]
            }
            TraitHeroSourceKind::OffensiveAbilityHasteRatingStacking => {
                &["hasteRating", "durationSeconds", "maximumStacks"]
            }
            TraitHeroSourceKind::OffensiveAbilityToHighestStatBuff => &[
                "secondaryRating",
                "durationSeconds",
                "procChance",
                "cooldownSeconds",
            ],
            TraitHeroSourceKind::StandingStillStaminaExpertiseRatingIncrease => &[
                "expertiseRating",
                "applicationDelaySeconds",
                "maxHealthMultiplier",
            ],
            TraitHeroSourceKind::WeaponAndSpiritPoints => &[
                "spiritCooldownMultiplier",
                "spiritCooldownDivider",
                "weaponCooldownReductionFraction",
            ],
            TraitHeroSourceKind::WeaponCritChanceCooldownReduction => &[
                "weaponCriticalStrikeBonus",
                "weaponCooldownReductionPerCrit",
                "maximumCriticalReductionsPerCommit",
                "cooldownReductionDelaySeconds",
            ],
            TraitHeroSourceKind::WeaponDamageReductionPrimaryStatIncrease => {
                &["powerMultiplier", "durationWeaponCooldownFraction"]
            }
            TraitHeroSourceKind::WeaponHealDamageIncrease => &["weaponDamageMultiplier"],
            TraitHeroSourceKind::Wolf => &[
                "smallHealingHealthFraction",
                "mediumHealingHealthFraction",
                "largeHealingHealthFraction",
            ],
        },
        #[cfg(test)]
        CompiledMechanicKind::HeroSource(HeroSourceKind::TestOnly) => &[],
    };
    parameters.require(&source.source_id, &source.source_name, required)?;
    if kind == CompiledMechanicKind::OnHitProc {
        let has_probability = parameters.contains(parameter_key!("procsPerMinute"))
            || parameters.contains(parameter_key!("procChance"));
        let has_damage = parameters.contains(parameter_key!("powerCoefficient"))
            || parameters.contains(parameter_key!("triggerDamageFraction"));
        if !has_probability || !has_damage {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!(
                    "{} is missing a proc probability or damage parameter",
                    source.source_name
                ),
                sources: vec![source.source_id.clone()],
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct CompiledProfile {
    pub(super) source: NormalizedDpsProfile,
    pub(super) abilities: Vec<Arc<CompiledAbility>>,
    pub(super) talents: Vec<CompiledTalent>,
    pub(super) mechanics: Vec<Arc<CompiledMechanic>>,
    pub(super) contract: HeroContract,
    pub(super) validated_seed: u64,
    pub(super) abilities_by_kind: BTreeMap<DpsAbilityKind, usize>,
    pub(super) talents_by_id: BTreeMap<String, usize>,
    pub(super) mechanic_indexes: CompiledMechanicIndexes,
    pub(super) damage_sources: Vec<CompiledDamageSource>,
    pub(super) damage_sources_by_id: BTreeMap<String, DamageSourceKey>,
    pub(super) proc_sources: Vec<CompiledProcSource>,
    pub(super) ability_proc_sources: BTreeMap<DpsAbilityKind, usize>,
    pub(super) talent_proc_sources: BTreeMap<String, usize>,
    pub(super) mechanic_proc_sources: BTreeMap<String, usize>,
    pub(super) spirit_refund_proc_source: usize,
    pub(super) mechanic_starting_spirit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct DamageSourceKey(pub(super) usize);

impl DamageSourceKey {
    pub(super) const INVALID: Self = Self(usize::MAX);
}

#[derive(Debug, Clone)]
pub(super) struct CompiledDamageSource {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) item_trait: bool,
}

#[derive(Debug, Clone)]
pub(super) struct CompiledProcSource {
    pub(super) id: String,
    pub(super) source_id: String,
    pub(super) name: String,
}

struct ProcSourceRegistry {
    sources: Vec<CompiledProcSource>,
    abilities: BTreeMap<DpsAbilityKind, usize>,
    talents: BTreeMap<String, usize>,
    mechanics: BTreeMap<String, usize>,
    spirit_refund: usize,
}

impl ProcSourceRegistry {
    fn compile(profile: &NormalizedDpsProfile) -> Self {
        let mut sources = Vec::new();
        let mut abilities = BTreeMap::new();
        let mut talents = BTreeMap::new();
        let mut mechanics = BTreeMap::new();
        for ability in &profile.abilities {
            let index = sources.len();
            sources.push(CompiledProcSource {
                id: format!("proc:ability:{}", ability.id),
                source_id: ability.id.clone(),
                name: ability.name.clone(),
            });
            abilities.insert(ability.kind, index);
        }
        for talent in &profile.talents {
            let index = sources.len();
            sources.push(CompiledProcSource {
                id: format!("proc:talent:{}", talent.id),
                source_id: talent.id.clone(),
                name: talent.name.clone(),
            });
            talents.insert(talent.id.clone(), index);
        }
        for mechanic in &profile.mechanics {
            if mechanics.contains_key(&mechanic.source_id) {
                continue;
            }
            let index = sources.len();
            sources.push(CompiledProcSource {
                id: format!("proc:mechanic:{}", mechanic.source_id),
                source_id: mechanic.source_id.clone(),
                name: mechanic.source_name.clone(),
            });
            mechanics.insert(mechanic.source_id.clone(), index);
        }
        let spirit_refund = sources.len();
        sources.push(CompiledProcSource {
            id: "proc:system:spirit-refund".into(),
            source_id: "spirit-refund".into(),
            name: "Spirit refund".into(),
        });
        Self {
            sources,
            abilities,
            talents,
            mechanics,
            spirit_refund,
        }
    }
}

#[derive(Default)]
struct DamageSourceRegistry {
    sources: Vec<CompiledDamageSource>,
    by_id: BTreeMap<String, DamageSourceKey>,
}

impl DamageSourceRegistry {
    fn insert(&mut self, id: impl Into<String>, name: impl Into<String>) -> DamageSourceKey {
        let id = id.into();
        if let Some(key) = self.by_id.get(&id) {
            return *key;
        }
        let key = DamageSourceKey(self.sources.len());
        self.sources.push(CompiledDamageSource {
            // The Wolf is stored among item traits, but its cooked Perk passive
            // has no AbilityType.Trait tag. Diamond's heal filter admits it.
            item_trait: id.starts_with("gear:ItemTrait.ID.") && id != "gear:ItemTrait.ID.Wolf",
            id: id.clone(),
            name: name.into(),
        });
        self.by_id.insert(id, key);
        key
    }
}

const SYNTHETIC_DAMAGE_SOURCES: [(&str, &str); 8] = [
    ("talent:crackling-inferno", "Crackling Inferno"),
    ("GA_Rime_Helper_AutoDamageProjectile", "Anima Spikes"),
    ("GA_Rime_TargetedPeriodicProjectileAoe", "Frost Swallows"),
    ("GA_Rime_OnTargetPulsatingAoe", "Ice Comet"),
    ("talent:talon-strike", "Talon Strike"),
    ("talent:rising-talons", "Rising Talons"),
    ("talent:coalescing-frost", "Coalescing Frost"),
    ("talent:glacial-assault", "Glacial Assault"),
];

#[derive(Debug, Clone, Default)]
pub(super) struct CompiledMechanicIndexes {
    pub(super) on_cast: Vec<usize>,
    pub(super) on_damage: Vec<usize>,
    pub(super) on_exact_damage: Vec<usize>,
    pub(super) on_healing: Vec<usize>,
    pub(super) periodic_healing: Option<usize>,
    pub(super) on_critical_damage: Vec<usize>,
    pub(super) on_hit: Vec<usize>,
    pub(super) outgoing_damage_multiplier: Vec<usize>,
    pub(super) basic_damage_bonus: Vec<usize>,
    pub(super) basic_to_aoe: Vec<usize>,
    pub(super) first_damage_expertise: Vec<usize>,
    pub(super) weapon_critical_cooldown: Vec<usize>,
    pub(super) increased_main_stat_and_spirit: Vec<usize>,
    pub(super) extra_dot_on_application: Vec<usize>,
    pub(super) apocalypse_dot: Vec<usize>,
    pub(super) target_damage_multiplier: Vec<usize>,
    pub(super) dynamic_stat: [Vec<usize>; 4],
    pub(super) dynamic_rating: [Vec<usize>; 4],
    pub(super) power_multiplier: Vec<usize>,
    pub(super) source_damage_multiplier: Vec<usize>,
    pub(super) critical_strike: Vec<usize>,
    pub(super) critical_multiplier: Vec<usize>,
    pub(super) cooldown_recovery: Vec<usize>,
    pub(super) cooldown_rate_boundaries: Vec<usize>,
    pub(super) standing_still: Vec<usize>,
    pub(super) rime_starting_resources: Option<usize>,
    pub(super) frostwyrm: Option<usize>,
    pub(super) bursting_ice: Option<usize>,
    pub(super) tariq_thundering_vortex: Option<usize>,
    pub(super) tariq_slayers_mosh: Option<usize>,
    pub(super) tariq_executioners_grin: Option<usize>,
    pub(super) elarion_starstrikers_ascent: Option<usize>,
    pub(super) elarion_shimmer: Option<usize>,
    pub(super) elarion_astronomers_hail: Option<usize>,
    pub(super) mara_drenched_in_blood: Option<usize>,
    pub(super) mara_arachnid_poison: Option<usize>,
    pub(super) mara_arachnid_clone: Option<usize>,
    pub(super) gunde_bleeding_hearts: Option<usize>,
    pub(super) gunde_bloodsoaked_cleaver: Option<usize>,
    pub(super) gunde_carrion_onslaught: Option<usize>,
    pub(super) intrepid: Vec<usize>,
    pub(super) fire_toad: Option<usize>,
    pub(super) wayfarer: Option<usize>,
    pub(super) heroism_power_set: Option<usize>,
}

impl CompiledMechanicIndexes {
    fn compile(mechanics: &[Arc<CompiledMechanic>]) -> Self {
        let mut indexes = Self::default();
        for (index, mechanic) in mechanics.iter().enumerate() {
            let hero_kind = match mechanic.kind {
                CompiledMechanicKind::HeroSource(kind) => Some(kind),
                _ => None,
            };
            if hero_kind == Some(HeroSourceKind::Gem(GemHeroSourceKind::RubyPeriodicHealing)) {
                indexes.periodic_healing = Some(index);
            }
            if mechanic.kind == CompiledMechanicKind::OnHitProc {
                indexes.on_hit.push(index);
            }
            if matches!(
                hero_kind,
                Some(HeroSourceKind::Finesse(1 | 2 | 4 | 7 | 8 | 12 | 14))
                    | Some(HeroSourceKind::Set(SetHeroSourceKind::HasteOnAbility))
                    | Some(HeroSourceKind::Trait(
                        TraitHeroSourceKind::AbilityToIncreasedMainStat
                            | TraitHeroSourceKind::CommitToHasteRatingAndWeaponCooldown
                            | TraitHeroSourceKind::CooldownRecoveryOnWeaponAbility
                            | TraitHeroSourceKind::GemCooldownRecoveryOnAbilityProc
                            | TraitHeroSourceKind::GemPulsatingOnAbilityTotemProc
                            | TraitHeroSourceKind::GemWhirlwindProc
                            | TraitHeroSourceKind::OffensiveAbilityHasteRatingStacking
                            | TraitHeroSourceKind::OffensiveAbilityToHighestStatBuff
                            | TraitHeroSourceKind::WeaponAndSpiritPoints
                            | TraitHeroSourceKind::WeaponCritChanceCooldownReduction
                            | TraitHeroSourceKind::WeaponDamageReductionPrimaryStatIncrease
                            | TraitHeroSourceKind::Wolf
                    ))
            ) {
                indexes.on_cast.push(index);
            }
            if matches!(
                mechanic.kind,
                CompiledMechanicKind::DamageMultiplier
                    | CompiledMechanicKind::AbilityDamageMultiplier
            ) {
                indexes.outgoing_damage_multiplier.push(index);
            }
            if matches!(
                hero_kind,
                Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::CritsToIncreasedCritRating
                        | TraitHeroSourceKind::CritsToIncreasedPrimaryStatBuff
                        | TraitHeroSourceKind::GemDotHotOnCrit
                )) | Some(HeroSourceKind::Set(SetHeroSourceKind::PowerOnCrit))
            ) {
                indexes.on_critical_damage.push(index);
                if hero_kind != Some(HeroSourceKind::Trait(TraitHeroSourceKind::GemDotHotOnCrit)) {
                    indexes.on_healing.push(index);
                }
            }
            if matches!(
                hero_kind,
                Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::GemPulsatingOnAbilityTotemProc
                        | TraitHeroSourceKind::GemTargetedSpikeProc
                ))
            ) {
                indexes.on_damage.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::GemSingleTargetProcOnDamageHeal,
                ))
            {
                indexes.on_healing.push(index);
                indexes.on_exact_damage.push(index);
            }
            if hero_kind == Some(HeroSourceKind::Finesse(9)) {
                indexes.basic_damage_bonus.push(index);
            }
            if hero_kind == Some(HeroSourceKind::Finesse(1)) {
                indexes.intrepid.push(index);
            }
            if hero_kind == Some(HeroSourceKind::Finesse(12)) {
                indexes.wayfarer = Some(index);
            }
            if hero_kind == Some(HeroSourceKind::Finesse(6)) {
                indexes.basic_to_aoe.push(index);
            }
            if hero_kind == Some(HeroSourceKind::Gem(GemHeroSourceKind::EmeraldExpertise)) {
                indexes.first_damage_expertise.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::WeaponCritChanceCooldownReduction,
                ))
            {
                indexes.weapon_critical_cooldown.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::IncreasedMainStatAndSpiritRating,
                ))
            {
                indexes.increased_main_stat_and_spirit.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::StandingStillStaminaExpertiseRatingIncrease,
                ))
            {
                indexes.standing_still.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Trait(
                    TraitHeroSourceKind::ExtraDotHotOnEffectApplicationProc,
                ))
            {
                indexes.extra_dot_on_application.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::EngulfingFlames,
                ))
            {
                indexes.target_damage_multiplier.push(index);
            }
            if hero_kind
                == Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::ApocalypseDot,
                ))
            {
                indexes.apocalypse_dot.push(index);
            }
            match hero_kind {
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::FireToad)) => {
                    indexes.fire_toad = Some(index);
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::RimeStartingResources)) => {
                    indexes.rime_starting_resources = Some(index)
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::Frostwyrm)) => {
                    indexes.frostwyrm = Some(index);
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::BurstingIce)) => {
                    indexes.bursting_ice = Some(index);
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::TariqThunderingVortex)) => {
                    indexes.tariq_thundering_vortex = Some(index)
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::TariqSlayersMosh)) => {
                    indexes.tariq_slayers_mosh = Some(index)
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::TariqExecutionersGrin)) => {
                    indexes.tariq_executioners_grin = Some(index)
                }
                Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::ElarionStarstrikersAscent,
                )) => indexes.elarion_starstrikers_ascent = Some(index),
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::ElarionShimmer)) => {
                    indexes.elarion_shimmer = Some(index)
                }
                Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::ElarionAstronomersHail,
                )) => indexes.elarion_astronomers_hail = Some(index),
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::MaraDrenchedInBlood)) => {
                    indexes.mara_drenched_in_blood = Some(index)
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::MaraArachnidPoison)) => {
                    indexes.mara_arachnid_poison = Some(index)
                }
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::MaraArachnidClone)) => {
                    indexes.mara_arachnid_clone = Some(index)
                }
                Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::GundeBleedingHeartsHeart,
                )) => indexes.gunde_bleeding_hearts = Some(index),
                Some(HeroSourceKind::Legendary(
                    LegendaryHeroSourceKind::GundeBloodsoakedCleaver,
                )) => indexes.gunde_bloodsoaked_cleaver = Some(index),
                Some(HeroSourceKind::Legendary(LegendaryHeroSourceKind::GundeCarrionOnslaught)) => {
                    indexes.gunde_carrion_onslaught = Some(index)
                }
                Some(HeroSourceKind::Set(SetHeroSourceKind::HeroismPower)) => {
                    indexes.heroism_power_set = Some(index);
                }
                _ => {}
            }
            if hero_kind.is_some() {
                let stat_parameters = [
                    parameter_key!("criticalStrikeBonus"),
                    parameter_key!("hasteBonus"),
                    parameter_key!("expertiseBonus"),
                    parameter_key!("spiritBonus"),
                ];
                let rating_parameters = [
                    parameter_key!("criticalRating"),
                    parameter_key!("expertiseRating"),
                    parameter_key!("hasteRating"),
                    parameter_key!("spiritRating"),
                ];
                for (slot, parameter) in stat_parameters.into_iter().enumerate() {
                    let fallback = match slot {
                        1 => parameter_key!("haste"),
                        2 => parameter_key!("expertise"),
                        _ => parameter,
                    };
                    if mechanic.parameters.contains(parameter)
                        || mechanic.parameters.contains(fallback)
                        || hero_kind == Some(HeroSourceKind::Finesse(8))
                    {
                        indexes.dynamic_stat[slot].push(index);
                    }
                }
                for (slot, parameter) in rating_parameters.into_iter().enumerate() {
                    // Willful Momentum's Spirit rating is an infinite effect
                    // already included by profile normalization. Its proc only
                    // activates the separate primary-attribute buff.
                    if slot == 3
                        && hero_kind
                            == Some(HeroSourceKind::Trait(
                                TraitHeroSourceKind::IncreasedMainStatAndSpiritRating,
                            ))
                    {
                        continue;
                    }
                    if mechanic.parameters.contains(parameter)
                        || hero_kind
                            == Some(HeroSourceKind::Trait(
                                TraitHeroSourceKind::OffensiveAbilityToHighestStatBuff,
                            ))
                        || (slot == 1
                            && hero_kind
                                == Some(HeroSourceKind::Trait(
                                    TraitHeroSourceKind::StandingStillStaminaExpertiseRatingIncrease,
                                )))
                    {
                        indexes.dynamic_rating[slot].push(index);
                    }
                }
                if mechanic
                    .parameters
                    .contains(parameter_key!("heroismPowerMultiplier"))
                    || mechanic
                        .parameters
                        .contains(parameter_key!("powerMultiplier"))
                {
                    indexes.power_multiplier.push(index);
                }
                if hero_kind == Some(HeroSourceKind::Finesse(10))
                    || hero_kind
                        == Some(HeroSourceKind::Trait(
                            TraitHeroSourceKind::WeaponHealDamageIncrease,
                        ))
                {
                    indexes.source_damage_multiplier.push(index);
                }
                if hero_kind == Some(HeroSourceKind::Finesse(5))
                    || hero_kind
                        == Some(HeroSourceKind::Trait(
                            TraitHeroSourceKind::WeaponCritChanceCooldownReduction,
                        ))
                {
                    indexes.critical_strike.push(index);
                }
                if hero_kind == Some(HeroSourceKind::Finesse(5)) {
                    indexes.critical_multiplier.push(index);
                }
                if hero_kind == Some(HeroSourceKind::Finesse(11))
                    || mechanic
                        .parameters
                        .contains(parameter_key!("cooldownAccelerationMultiplier"))
                {
                    indexes.cooldown_recovery.push(index);
                }
            }
        }
        // Native generic delegates finish every listener in a tag-container
        // group before entering another group. Diamond's ordinary-heal group
        // cannot split the shared critical damage/healing group. Keep the
        // accepted profile priority for groups and initial peers. Actual
        // registration history is not claimed to be reconstructed.
        // Amethyst's friendly output has no DPS effect, but its subscription
        // still establishes this group's assumed position before later peers.
        if let Some(first_critical) = indexes.on_critical_damage.first().copied() {
            indexes.on_healing.sort_by_key(|index| {
                if indexes.on_critical_damage.binary_search(index).is_ok() {
                    first_critical
                } else {
                    *index
                }
            });
        }
        indexes
            .cooldown_rate_boundaries
            .extend(indexes.dynamic_stat[1].iter().copied());
        indexes
            .cooldown_rate_boundaries
            .extend(indexes.dynamic_rating[2].iter().copied());
        // Live Spirit changes The Philosopher's Haste while its buff is active.
        indexes
            .cooldown_rate_boundaries
            .extend(indexes.dynamic_stat[3].iter().copied());
        indexes
            .cooldown_rate_boundaries
            .extend(indexes.dynamic_rating[3].iter().copied());
        indexes
            .cooldown_rate_boundaries
            .extend(indexes.cooldown_recovery.iter().copied());
        indexes.cooldown_rate_boundaries.sort_unstable();
        indexes.cooldown_rate_boundaries.dedup();
        indexes
    }

    pub(super) fn dynamic_stat(&self, parameter: ParameterKey) -> &[usize] {
        if parameter == parameter_key!("criticalStrikeBonus") {
            &self.dynamic_stat[0]
        } else if parameter == parameter_key!("hasteBonus") {
            &self.dynamic_stat[1]
        } else if parameter == parameter_key!("expertiseBonus") {
            &self.dynamic_stat[2]
        } else if parameter == parameter_key!("spiritBonus") {
            &self.dynamic_stat[3]
        } else {
            &[]
        }
    }

    pub(super) fn dynamic_rating(&self, parameter: ParameterKey) -> &[usize] {
        if parameter == parameter_key!("criticalRating") {
            &self.dynamic_rating[0]
        } else if parameter == parameter_key!("expertiseRating") {
            &self.dynamic_rating[1]
        } else if parameter == parameter_key!("hasteRating") {
            &self.dynamic_rating[2]
        } else if parameter == parameter_key!("spiritRating") {
            &self.dynamic_rating[3]
        } else {
            &[]
        }
    }
}

impl CompiledProfile {
    fn compile(
        source: &NormalizedDpsProfile,
        validated_seed: u64,
    ) -> Result<Self, SimulationError> {
        let proc_source_registry = ProcSourceRegistry::compile(source);
        let mut damage_source_registry = DamageSourceRegistry::default();
        let abilities = source
            .abilities
            .iter()
            .map(|source| {
                let mut ability = CompiledAbility::compile(source)?;
                ability.damage_source =
                    damage_source_registry.insert(source.id.clone(), source.name.clone());
                Ok(ability)
            })
            .map(|ability| ability.map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        let talents = source
            .talents
            .iter()
            .map(CompiledTalent::compile)
            .collect::<Result<Vec<_>, _>>()?;
        let mechanics = source
            .mechanics
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let mut mechanic = CompiledMechanic::compile(source)?;
                mechanic.index = index;
                mechanic.damage_source = damage_source_registry.insert(
                    format!("gear:{}", source.source_id),
                    source.source_name.clone(),
                );
                Ok(mechanic)
            })
            .map(|mechanic| mechanic.map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        for (id, name) in SYNTHETIC_DAMAGE_SOURCES {
            damage_source_registry.insert(id, name);
        }
        if source
            .talents
            .iter()
            .any(|talent| talent.id == "firemage-talent-id-talent17")
        {
            damage_source_registry.insert("firemage-talent-id-talent17", "Flare Up");
        }
        let mut runtime_source = source.clone();
        runtime_source.abilities.clear();
        runtime_source.talents.clear();
        runtime_source.mechanics.clear();
        let contract = HeroIdentity::from_id(&source.hero_id)
            .expect("validated profiles have a supported hero")
            .contract();
        if talents
            .iter()
            .any(|talent| talent.kind.hero() != contract.hero)
        {
            return Err(SimulationError::new(
                SimulationErrorCode::UncoveredMechanics,
                "compiled talents must belong to the profile hero",
            ));
        }
        let mechanic_indexes = CompiledMechanicIndexes::compile(&mechanics);
        let mechanic_starting_spirit = mechanics
            .iter()
            .filter_map(|mechanic| mechanic.parameters.get(parameter_key!("startingSpirit")))
            .sum();
        Ok(Self {
            contract,
            validated_seed,
            abilities_by_kind: source
                .abilities
                .iter()
                .enumerate()
                .map(|(index, ability)| (ability.kind, index))
                .collect(),
            talents_by_id: source
                .talents
                .iter()
                .enumerate()
                .map(|(index, talent)| (talent.id.clone(), index))
                .collect(),
            mechanic_indexes,
            damage_sources: damage_source_registry.sources,
            damage_sources_by_id: damage_source_registry.by_id,
            proc_sources: proc_source_registry.sources,
            ability_proc_sources: proc_source_registry.abilities,
            talent_proc_sources: proc_source_registry.talents,
            mechanic_proc_sources: proc_source_registry.mechanics,
            spirit_refund_proc_source: proc_source_registry.spirit_refund,
            mechanic_starting_spirit,
            source: runtime_source,
            abilities,
            talents,
            mechanics,
        })
    }

    #[cfg(test)]
    pub(super) fn compile_test(source: &NormalizedDpsProfile) -> Self {
        let mut compiled = Self::compile(source, 0);
        if compiled.is_err() {
            let mut source_without_mechanics = source.clone();
            source_without_mechanics.mechanics.clear();
            let mut profile = Self::compile(&source_without_mechanics, 0)
                .expect("test profile abilities and talents compile");
            profile.mechanics = source
                .mechanics
                .iter()
                .enumerate()
                .map(|(index, source)| {
                    let mut mechanic = CompiledMechanic::compile_test(source)?;
                    mechanic.index = index;
                    Ok::<_, SimulationError>(mechanic)
                })
                .map(|mechanic| mechanic.map(Arc::new))
                .collect::<Result<Vec<_>, _>>()
                .expect("test mechanic parameters compile");
            profile.mechanic_indexes = CompiledMechanicIndexes::compile(&profile.mechanics);
            profile.mechanic_starting_spirit = profile
                .mechanics
                .iter()
                .filter_map(|mechanic| mechanic.parameters.get(parameter_key!("startingSpirit")))
                .sum();
            let mut registry = DamageSourceRegistry::default();
            for ability in &mut profile.abilities {
                let ability = Arc::make_mut(ability);
                ability.damage_source = registry.insert(ability.id.clone(), ability.name.clone());
            }
            for mechanic in &mut profile.mechanics {
                let mechanic = Arc::make_mut(mechanic);
                mechanic.damage_source = registry.insert(
                    format!("gear:{}", mechanic.source_id),
                    mechanic.source_name.clone(),
                );
            }
            for (id, name) in SYNTHETIC_DAMAGE_SOURCES {
                registry.insert(id, name);
            }
            if profile
                .talents
                .iter()
                .any(|talent| talent.id == "firemage-talent-id-talent17")
            {
                registry.insert("firemage-talent-id-talent17", "Flare Up");
            }
            profile.damage_sources = registry.sources;
            profile.damage_sources_by_id = registry.by_id;
            let proc_registry = ProcSourceRegistry::compile(source);
            profile.proc_sources = proc_registry.sources;
            profile.ability_proc_sources = proc_registry.abilities;
            profile.talent_proc_sources = proc_registry.talents;
            profile.mechanic_proc_sources = proc_registry.mechanics;
            profile.spirit_refund_proc_source = proc_registry.spirit_refund;
            compiled = Ok(profile);
        }
        compiled.expect("test profile parameters compile")
    }

    pub(super) fn ability(&self, kind: DpsAbilityKind) -> Option<&Arc<CompiledAbility>> {
        self.abilities_by_kind
            .get(&kind)
            .map(|index| &self.abilities[*index])
    }

    pub(super) fn talent(&self, id: &str) -> Option<&CompiledTalent> {
        self.talents_by_id
            .get(id)
            .map(|index| &self.talents[*index])
    }

    pub(super) fn damage_source_key(&self, id: &str) -> Option<DamageSourceKey> {
        self.damage_sources_by_id.get(id).copied()
    }
}

impl std::ops::Deref for CompiledProfile {
    type Target = NormalizedDpsProfile;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl TryFrom<&SimulationRequest> for CompiledProfile {
    type Error = SimulationError;

    fn try_from(request: &SimulationRequest) -> Result<Self, Self::Error> {
        let validated_seed = validate(request)?;
        Self::compile(&request.profile, validated_seed)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SelectedTalents<'a> {
    pub(super) profile: &'a CompiledProfile,
}

impl<'a> SelectedTalents<'a> {
    pub(super) fn get(&self, id: &str) -> Option<&'a CompiledTalent> {
        self.profile.talent(id)
    }

    pub(super) fn contains_key(&self, id: &str) -> bool {
        self.profile.talents_by_id.contains_key(id)
    }
}

impl std::ops::Index<&str> for SelectedTalents<'_> {
    type Output = CompiledTalent;

    fn index(&self, id: &str) -> &Self::Output {
        self.get(id).expect("selected talent exists")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AbilityCategory {
    Basic,
    Power,
    Core,
    Major,
    Spirit,
    Weapon,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AbilityOwner {
    Ardeos,
    Rime,
    Tariq,
    Elarion,
    Mara,
    Gunde,
    Shared,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AbilityMetadata {
    pub(super) id: &'static str,
    pub(super) owner: AbilityOwner,
    pub(super) category: AbilityCategory,
    pub(super) source_snapshot_at_commit: bool,
    pub(super) required_parameters: &'static [&'static str],
}

pub(super) fn ability_metadata(kind: DpsAbilityKind) -> AbilityMetadata {
    use AbilityCategory::{Basic, Core, Major, Other, Power, Spirit, Weapon};
    use AbilityOwner::{Ardeos, Elarion, Gunde, Mara, Rime, Shared, Tariq};

    let (id, owner, category, source_snapshot_at_commit, required_parameters): (
        &'static str,
        AbilityOwner,
        AbilityCategory,
        bool,
        &'static [&'static str],
    ) = match kind {
        DpsAbilityKind::InfernalWave => ("infernal-wave", Ardeos, Basic, true, &[][..]),
        DpsAbilityKind::Detonate => (
            "detonate",
            Ardeos,
            Power,
            false,
            &[
                "initialDelaySeconds",
                "perTargetHitDelaySeconds",
                "hitsPerTarget",
                "betweenHitDelaySeconds",
                "targetCountDamageScalingThreshold",
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundChanceScale",
                "spiritRefundChanceFlatIncrease",
            ],
        ),
        DpsAbilityKind::Apocalypse => (
            "apocalypse",
            Ardeos,
            Major,
            true,
            &[
                "targetCountDamageScalingThreshold",
                "resourceProcChance",
                "cindersOnResourceProc",
            ],
        ),
        DpsAbilityKind::SearingBlaze => ("searing-blaze", Ardeos, Core, true, &[]),
        DpsAbilityKind::EngulfingFlames => ("engulfing-flames", Ardeos, Core, true, &[]),
        DpsAbilityKind::Incinerate => (
            "incinerate",
            Ardeos,
            Spirit,
            false,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::FireFrogs => (
            "fire-frogs",
            Ardeos,
            Core,
            true,
            &[
                "frogCount",
                "attacksPerFrog",
                "damageToDotTransferFraction",
                "assumedServerTickRateHz",
                "initialPathDistanceUnits",
                "spawnRadiusUnits",
                "minimumBatchSpawnDelaySeconds",
                "maximumBatchSpawnDelaySeconds",
                "attackRangeUnits",
                "jumpDurationSeconds",
                "minimumJumpLengthUnits",
                "maximumJumpLengthUnits",
                "minimumJumpPeriodSeconds",
                "maximumJumpPeriodSeconds",
            ],
        ),
        DpsAbilityKind::Wildfire => ("wildfire", Ardeos, Major, false, &["tickRateMultiplier"]),
        DpsAbilityKind::Pyromania => ("pyromania", Ardeos, Major, true, &[]),
        DpsAbilityKind::FireBall => (
            "fire-ball",
            Ardeos,
            Core,
            true,
            &[
                "targetCountDamageScalingThreshold",
                "damageToDotTransferFraction",
                "resourceProcChance",
                "cindersOnResourceProc",
            ],
        ),
        DpsAbilityKind::IceBlitz => (
            "ice-blitz",
            Rime,
            Major,
            false,
            &["damageMultiplier", "extensionSeconds"],
        ),
        DpsAbilityKind::BurstingIce => (
            "bursting-ice",
            Rime,
            Core,
            false,
            &[
                "pulsePowerCoefficient",
                "pulsePeriodSeconds",
                "targetCountDamageScalingThreshold",
                "animaPerPulse",
                "animaPerPulseCap",
            ],
        ),
        DpsAbilityKind::FrostBolt => (
            "frost-bolt",
            Rime,
            Basic,
            true,
            &["assumedProjectileImpactDelaySeconds", "serverTickRateCapHz"],
        ),
        DpsAbilityKind::GlacialBlast => (
            "glacial-blast",
            Rime,
            Power,
            true,
            &["assumedProjectileImpactDelaySeconds", "serverTickRateCapHz"],
        ),
        DpsAbilityKind::FreezingTorrent => ("freezing-torrent", Rime, Core, false, &[]),
        DpsAbilityKind::WintersBlessing => (
            "winters-blessing",
            Rime,
            Major,
            false,
            &[
                "spiritMultiplier",
                "healingBatchSeconds",
                "damageToHealingFactor",
            ],
        ),
        DpsAbilityKind::WrathOfWinter => (
            "wrath-of-winter",
            Rime,
            Spirit,
            false,
            &[
                "damageMultiplier",
                "volleyPeriodSeconds",
                "volleyProjectiles",
            ],
        ),
        DpsAbilityKind::ColdSnap => (
            "cold-snap",
            Rime,
            Basic,
            true,
            &["orbGain", "extraCooldownAccelerationPerHaste"],
        ),
        DpsAbilityKind::IceComet => (
            "ice-comet",
            Rime,
            Power,
            true,
            &[
                "targetCountDamageScalingThreshold",
                "initialSpawnDelaySeconds",
                "pulsePeriodSeconds",
            ],
        ),
        DpsAbilityKind::FlightOfTheNavir => (
            "flight-of-the-navir",
            Rime,
            Major,
            false,
            &[
                "projectileCount",
                "projectilePowerCoefficient",
                "damageIncreasePerSpirit",
                "assumedProjectileImpactDelaySeconds",
                "serverTickRateCapHz",
            ],
        ),
        DpsAbilityKind::AnimaSpike => (
            "anima-spike",
            Rime,
            Core,
            false,
            &[
                "projectileCount",
                "orbGain",
                "assumedProjectileImpactDelaySeconds",
                "serverTickRateCapHz",
            ],
        ),
        DpsAbilityKind::FrostSwallow => ("frost-swallow", Rime, Core, false, &[]),
        DpsAbilityKind::HammerStorm => (
            "hammer-storm",
            Tariq,
            Power,
            false,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "tickDamageMultiplier",
                "maximumFuryCost",
                "targetCountDamageScalingThreshold",
                "lightningPowerCoefficient",
                "lightningTargetCountDamageScalingThreshold",
                "lightningDelaySeconds",
            ],
        ),
        DpsAbilityKind::HeavyStrike => (
            "heavy-strike",
            Tariq,
            Core,
            false,
            &[
                "weakDamageMultiplier",
                "weakResourceMultiplier",
                "cleaveDamageMultiplier",
                "cleaveTargetCountDamageScalingThreshold",
                "lightningPowerCoefficient",
                "lightningTargetCountDamageScalingThreshold",
                "lightningDelaySeconds",
            ],
        ),
        DpsAbilityKind::TariqChainLightning => (
            "tariq-chain-lightning",
            Tariq,
            Core,
            false,
            &[
                "chainHits",
                "jumpDelaySeconds",
                "uniqueTargetDamageIncrease",
                "furyPerHit",
            ],
        ),
        DpsAbilityKind::ThunderCall => ("thunder-call", Tariq, Major, false, &[]),
        DpsAbilityKind::WildSwing => (
            "wild-swing",
            Tariq,
            Basic,
            true,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::FocusedWrath => (
            "focused-wrath",
            Tariq,
            Major,
            false,
            &[
                "costMultiplier",
                "damageMultiplier",
                "stacks",
                "maximumStacks",
            ],
        ),
        DpsAbilityKind::RagingTempest => (
            "raging-tempest",
            Tariq,
            Spirit,
            false,
            &[
                "pulsePowerCoefficient",
                "pulseVisualDelaySeconds",
                "pulseDamageSpread",
                "pulsePeriodSeconds",
                "expertisePerStack",
                "maximumStacks",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::SkullCrusher => (
            "skull-crusher",
            Tariq,
            Power,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "cleaveDelaySeconds",
                "furyCost",
                "lightningPowerCoefficient",
                "lightningDelaySeconds",
                "cleaveDamageMultiplier",
                "cleaveTargetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::CullingStrike => (
            "culling-strike",
            Tariq,
            Power,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "maximumFuryCost",
                "damageIncreasePerFuryFraction",
            ],
        ),
        DpsAbilityKind::LeapSmash => (
            "leap-smash",
            Tariq,
            Core,
            true,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::FaceBreaker => (
            "face-breaker",
            Tariq,
            Basic,
            true,
            &[
                "cleaveDamageMultiplier",
                "cleaveTargetCountDamageScalingThreshold",
                "cleaveDelaySeconds",
            ],
        ),
        DpsAbilityKind::TariqAttack => (
            "tariq-attack",
            Tariq,
            Other,
            true,
            &["swingDurationSeconds", "hitWindowSeconds"],
        ),
        DpsAbilityKind::Multishot => (
            "multishot",
            Elarion,
            Core,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundMainTargetStacks",
                "spiritRefundAdditionalTargetStacks",
                "projectileSpawnDelaySeconds",
                "focusCost",
                "targetCountDamageScalingThreshold",
                "procDamageMultiplier",
                "maximumProcStacks",
                "empoweredMinimumProjectiles",
                "empoweredCostMultiplier",
            ],
        ),
        DpsAbilityKind::FocusedShot => (
            "focused-shot",
            Elarion,
            Basic,
            true,
            &[
                "focusRegenerationPerPulse",
                "focusRegenerationIntervalSeconds",
                "celestialImpetusProcsPerMinute",
                "celestialImpetusDurationSeconds",
                "celestialImpetusMaximumStacks",
            ],
        ),
        DpsAbilityKind::HighwindArrow => (
            "highwind-arrow",
            Elarion,
            Power,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundMainTargetStacks",
                "spiritRefundAdditionalTargetStacks",
                "focusCost",
                "bounceDamageMultiplier",
                "maximumBounces",
                "minimumTargetsForMultishotProc",
                "resurgentDamageMultiplier",
            ],
        ),
        DpsAbilityKind::HeartseekerBarrage => (
            "heartseeker-barrage",
            Elarion,
            Power,
            false,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundMainTargetStacks",
                "spiritRefundAdditionalTargetStacks",
                "focusCost",
                "impendingDamageIncreasePerProjectile",
            ],
        ),
        DpsAbilityKind::LunarlightMark => (
            "lunarlight-mark",
            Elarion,
            Major,
            false,
            &[
                "stacksApplied",
                "maximumStacks",
                "salvoProcChance",
                "salvoCriticalProcChance",
                "eruptionProcChance",
            ],
        ),
        DpsAbilityKind::SkystridersGrace => {
            ("skystriders-grace", Elarion, Major, false, &["hasteBonus"])
        }
        DpsAbilityKind::ElarionShoot => (
            "elarion-shoot",
            Elarion,
            Other,
            true,
            &["swingDurationSeconds"],
        ),
        DpsAbilityKind::EventHorizon => (
            "event-horizon",
            Elarion,
            Spirit,
            false,
            &[
                "damageMultiplier",
                "focusCostMultiplier",
                "heartseekerCooldownReductionPerHighwindHitSeconds",
                "starfallCooldownReductionPerHeartseekerHitSeconds",
            ],
        ),
        DpsAbilityKind::SkystridersSupremacy => (
            "skystriders-supremacy",
            Elarion,
            Major,
            false,
            &[
                "empoweredMinimumProjectiles",
                "empoweredCostMultiplier",
                "maximumStacks",
            ],
        ),
        DpsAbilityKind::CelestialShot => (
            "celestial-shot",
            Elarion,
            Basic,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundMainTargetStacks",
                "spiritRefundAdditionalTargetStacks",
                "focusCost",
            ],
        ),
        DpsAbilityKind::StarfallVolley => (
            "starfall-volley",
            Elarion,
            Power,
            true,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundMainTargetStacks",
                "spiritRefundAdditionalTargetStacks",
                "focusCost",
                "visualDelaySeconds",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::LunarlightSalvo => ("lunarlight-salvo", Elarion, Core, false, &[]),
        DpsAbilityKind::LunarlightEruption => (
            "lunarlight-eruption",
            Elarion,
            Core,
            false,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::SkitteringBlades => (
            "skittering-blades",
            Mara,
            Basic,
            true,
            &[
                "energyCost",
                "comboPointsPerHit",
                "comboPointsPerCriticalHit",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::ArachnidAssault => (
            "arachnid-assault",
            Mara,
            Power,
            true,
            &[
                "energyCost",
                "damageMultiplierPerComboPoint",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::MaraAttack => (
            "mara-attack",
            Mara,
            Other,
            true,
            &[
                "energyRegenerationPerSecond",
                "energyRegenerationHasteScaler",
                "spiritRefundDelaySeconds",
                "creepingDeathHasteScaler",
            ],
        ),
        DpsAbilityKind::HemorrhagingStrike => (
            "hemorrhaging-strike",
            Mara,
            Power,
            true,
            &[
                "energyCost",
                "bleedDurationPerComboPointSeconds",
                "energyPerBleedTick",
                "bleedRefreshCarryOverFraction",
            ],
        ),
        DpsAbilityKind::WidowsBite => (
            "widows-bite",
            Mara,
            Core,
            true,
            &[
                "energyGain",
                "comboPointsPerHit",
                "comboPointsPerCriticalHit",
                "poisonAdditionalDelaySeconds",
            ],
        ),
        DpsAbilityKind::MaidenOfDeath => (
            "maiden-of-death",
            Mara,
            Major,
            false,
            &["energyGenerationMultiplier", "damageMultiplier"],
        ),
        DpsAbilityKind::MatriarchMacabre => (
            "matriarch-macabre",
            Mara,
            Spirit,
            false,
            &[
                "damageMultiplier",
                "cloneDamageMultiplier",
                "copyActivationDelaySeconds",
            ],
        ),
        DpsAbilityKind::QueensFang => (
            "queens-fang",
            Mara,
            Power,
            true,
            &["energyCost", "damageMultiplierPerComboPoint"],
        ),
        DpsAbilityKind::BroodingShadows => ("brooding-shadows", Mara, Major, false, &[]),
        DpsAbilityKind::Backstab => (
            "backstab",
            Mara,
            Basic,
            true,
            &[
                "energyCost",
                "comboPointsPerHit",
                "comboPointsPerCriticalHit",
                "behindDamageMultiplier",
                "causticCriticalStrikeBonus",
                "poisonComboPointMultiplier",
                "poisonAdditionalDelaySeconds",
            ],
        ),
        DpsAbilityKind::FinalStratagem => ("final-stratagem", Mara, Major, false, &[]),
        DpsAbilityKind::CausticPoison => ("caustic-poison", Mara, Core, false, &[]),
        DpsAbilityKind::SeethingPoison => (
            "seething-poison",
            Mara,
            Core,
            false,
            &["energyRegenerationMultiplier"],
        ),
        DpsAbilityKind::VolatilePoison => (
            "volatile-poison",
            Mara,
            Core,
            false,
            &[
                "minimumDurationSeconds",
                "maximumDurationSeconds",
                "maximumTicks",
            ],
        ),
        DpsAbilityKind::VolatilePoisonEruption => {
            ("volatile-poison-eruption", Mara, Core, false, &[])
        }
        DpsAbilityKind::SeethingBurst => (
            "seething-burst",
            Mara,
            Core,
            false,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::CorrosiveSpill => ("corrosive-spill", Mara, Core, false, &[]),
        DpsAbilityKind::Hemotoxin => (
            "hemotoxin",
            Mara,
            Core,
            false,
            &["refreshCarryOverFraction"],
        ),
        DpsAbilityKind::HemotoxinEruption => (
            "hemotoxin-eruption",
            Mara,
            Core,
            false,
            &[
                "primaryBleedFraction",
                "areaBleedFraction",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::DoubleStrike => (
            "double-strike",
            Gunde,
            Basic,
            true,
            &["rendTransferFraction"],
        ),
        DpsAbilityKind::Warbound => ("warbound", Gunde, Other, true, &[]),
        DpsAbilityKind::OwedInBlood => (
            "owed-in-blood",
            Gunde,
            Other,
            false,
            &[
                "bloodFeatherDurationSeconds",
                "rendDamagePerFeather",
                "bloodcrazePowerCoefficient",
                "bloodcrazePulsePeriodSeconds",
                "bloodcrazePulses",
            ],
        ),
        DpsAbilityKind::BloodboundSpirit => (
            "bloodbound-spirit",
            Gunde,
            Spirit,
            true,
            &["rendTransferFraction", "damageMultiplier"],
        ),
        DpsAbilityKind::ReignInBlood => (
            "reign-in-blood",
            Gunde,
            Major,
            false,
            &["rendTransferFraction", "talentRendTransferFraction"],
        ),
        DpsAbilityKind::HeartSplitter => (
            "heart-splitter",
            Gunde,
            Core,
            true,
            &["rendTransferFraction", "exsanguinateFraction"],
        ),
        DpsAbilityKind::Rupture => (
            "rupture",
            Gunde,
            Power,
            true,
            &[
                "rendTransferFraction",
                "openWoundsDamageMultiplier",
                "openWoundsDurationSeconds",
            ],
        ),
        DpsAbilityKind::Slaughter => (
            "slaughter",
            Gunde,
            Power,
            false,
            &["consumedRendDamageMultiplier"],
        ),
        DpsAbilityKind::BloodArc => ("blood-arc", Gunde, Core, true, &["rendTransferFraction"]),
        DpsAbilityKind::ReaversEdge => (
            "reavers-edge",
            Gunde,
            Basic,
            true,
            &["rendTransferFraction", "targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::GundeAttack => (
            "gunde-attack",
            Gunde,
            Other,
            true,
            &["swingDurationSeconds"],
        ),
        DpsAbilityKind::ButchersHook => ("butchers-hook", Gunde, Other, true, &[]),
        DpsAbilityKind::GrimCarve => (
            "grim-carve",
            Gunde,
            Power,
            true,
            &[
                "rendTransferFraction",
                "targetCountDamageScalingThreshold",
                "projectileSpawnDelaySeconds",
            ],
        ),
        DpsAbilityKind::Rend => (
            "rend",
            Gunde,
            Core,
            false,
            &[
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundFeathers",
                "featherActivationDelaySeconds",
                "maximumGroundFeathers",
            ],
        ),
        DpsAbilityKind::Exsanguinate => ("exsanguinate", Gunde, Power, false, &[]),
        DpsAbilityKind::Bloodcraze => (
            "bloodcraze",
            Gunde,
            Core,
            false,
            &["damageMultiplierPerAdditionalTarget"],
        ),
        DpsAbilityKind::RavensPrecision => (
            "ravens-precision",
            Gunde,
            Core,
            false,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::Oathshatter => (
            "oathshatter",
            Gunde,
            Power,
            false,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::WeaponFrostVolley => (
            "weapon-frost-volley",
            Shared,
            Weapon,
            true,
            &["targetCountDamageScalingThreshold"],
        ),
        DpsAbilityKind::WeaponArcaneChannel => (
            "weapon-arcane-channel",
            Shared,
            Weapon,
            false,
            &[
                "channelCooldownRecoveryMultiplier",
                "targetCountDamageScalingThreshold",
            ],
        ),
        DpsAbilityKind::WeaponChainLightning => (
            "weapon-chain-lightning",
            Shared,
            Weapon,
            false,
            &[
                "criticalStrikeBonus",
                "firstTargetDamageMultiplier",
                "jumpDelaySeconds",
            ],
        ),
        DpsAbilityKind::WeaponShadowMark => (
            "weapon-shadow-mark",
            Shared,
            Weapon,
            false,
            &["accumulationFraction", "maximumPowerCoefficient"],
        ),
        DpsAbilityKind::WeaponCleaveCharge => (
            "weapon-cleave-charge",
            Shared,
            Weapon,
            false,
            &[
                "cleaveDamageMultiplier",
                "cleaveTargetCountDamageScalingThreshold",
                "postHitDelaySeconds",
                "buffDurationSeconds",
                "buffCooldownRecoveryMultiplier",
                "buffExpertise",
            ],
        ),
        DpsAbilityKind::WeaponFrontalCone => (
            "weapon-frontal-cone",
            Shared,
            Weapon,
            true,
            &[
                "targetCountDamageScalingThreshold",
                "initialPowerCoefficient",
                "repeatingPowerCoefficient",
                "finalPowerCoefficient",
                "coneDurationSeconds",
                "coneStunDurationSeconds",
                "coneTickIntervalSeconds",
            ],
        ),
        DpsAbilityKind::WeaponInstantAoe => (
            "weapon-instant-aoe",
            Shared,
            Weapon,
            false,
            &[
                "targetCountDamageScalingThreshold",
                "criticalStrikeBonusPerStack",
                "maximumCriticalStrikeStacks",
                "criticalStrikeBuffDurationSeconds",
            ],
        ),
    };
    AbilityMetadata {
        id,
        owner,
        category,
        source_snapshot_at_commit,
        required_parameters,
    }
}

pub(super) fn is_finesse_source(source_id: &str, suffix: &str) -> bool {
    source_id.strip_prefix("DynamicItemAbilityRank.") == Some(suffix)
}
pub(super) fn ability_category(kind: DpsAbilityKind) -> AbilityCategory {
    ability_metadata(kind).category
}

pub(super) fn secondary_rating_percentage(rating: f64) -> f64 {
    // Current CRHeroAttributeSet rating conversion. Each bracket's penalty is
    // cumulative, matching item-stat-model.json and the game-side Twaxel path.
    const BASE_STAT_MULTIPLIER: f64 = 0.16;
    const BRACKETS: [(f64, f64); 4] = [(10.0, 1.0), (15.0, 0.98), (20.0, 0.96), (25.0, 0.94)];
    const FINAL_PENALTY: f64 = 0.92;

    // The native magnitude calculation rounds the aggregate float rating
    // before indexing its cached integer-rating conversion table.
    let mut remaining_rating = f64::from((rating as f32).round_ties_even().max(0.0));
    let mut previous_maximum = 0.0;
    let mut percentage_points = 0.0;
    let mut cumulative_penalty = 1.0;
    for (maximum, penalty) in BRACKETS {
        cumulative_penalty *= penalty;
        let raw_bracket_size = maximum - previous_maximum;
        let rating_for_bracket = raw_bracket_size / (BASE_STAT_MULTIPLIER * cumulative_penalty);
        let consumed = remaining_rating.min(rating_for_bracket);
        percentage_points += consumed * BASE_STAT_MULTIPLIER * cumulative_penalty;
        remaining_rating -= consumed;
        previous_maximum = maximum;
        if remaining_rating <= 0.0 {
            break;
        }
    }
    if remaining_rating > 0.0 {
        cumulative_penalty *= FINAL_PENALTY;
        percentage_points += remaining_rating * BASE_STAT_MULTIPLIER * cumulative_penalty;
    }
    percentage_points / 100.0
}

pub(super) fn secondary_rating_delta(base_rating: f64, added_rating: f64) -> f64 {
    secondary_rating_percentage(base_rating + added_rating)
        - secondary_rating_percentage(base_rating)
}

pub(super) fn source_damage_spec_created_at_commit(kind: DpsAbilityKind) -> bool {
    ability_metadata(kind).source_snapshot_at_commit
}
// Current cooked ability tags, including primary auto attacks. A damage source
// or a Core category alone does not imply a skill commit (for example, Rime's
// helper projectiles). Keep these filters separate from targeting heuristics.
pub(super) fn is_primary_skill_commit(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::InfernalWave
            | DpsAbilityKind::Detonate
            | DpsAbilityKind::Apocalypse
            | DpsAbilityKind::SearingBlaze
            | DpsAbilityKind::EngulfingFlames
            | DpsAbilityKind::Incinerate
            | DpsAbilityKind::FireFrogs
            | DpsAbilityKind::Wildfire
            | DpsAbilityKind::Pyromania
            | DpsAbilityKind::FireBall
            | DpsAbilityKind::IceBlitz
            | DpsAbilityKind::BurstingIce
            | DpsAbilityKind::FrostBolt
            | DpsAbilityKind::GlacialBlast
            | DpsAbilityKind::FreezingTorrent
            | DpsAbilityKind::WintersBlessing
            | DpsAbilityKind::WrathOfWinter
            | DpsAbilityKind::ColdSnap
            | DpsAbilityKind::IceComet
            | DpsAbilityKind::FlightOfTheNavir
            | DpsAbilityKind::HammerStorm
            | DpsAbilityKind::HeavyStrike
            | DpsAbilityKind::TariqChainLightning
            | DpsAbilityKind::ThunderCall
            | DpsAbilityKind::WildSwing
            | DpsAbilityKind::FocusedWrath
            | DpsAbilityKind::RagingTempest
            | DpsAbilityKind::SkullCrusher
            | DpsAbilityKind::CullingStrike
            | DpsAbilityKind::LeapSmash
            | DpsAbilityKind::FaceBreaker
            | DpsAbilityKind::TariqAttack
            | DpsAbilityKind::Multishot
            | DpsAbilityKind::FocusedShot
            | DpsAbilityKind::HighwindArrow
            | DpsAbilityKind::HeartseekerBarrage
            | DpsAbilityKind::LunarlightMark
            | DpsAbilityKind::SkystridersGrace
            | DpsAbilityKind::ElarionShoot
            | DpsAbilityKind::EventHorizon
            | DpsAbilityKind::SkystridersSupremacy
            | DpsAbilityKind::CelestialShot
            | DpsAbilityKind::StarfallVolley
            | DpsAbilityKind::SkitteringBlades
            | DpsAbilityKind::ArachnidAssault
            | DpsAbilityKind::MaraAttack
            | DpsAbilityKind::HemorrhagingStrike
            | DpsAbilityKind::WidowsBite
            | DpsAbilityKind::MaidenOfDeath
            | DpsAbilityKind::MatriarchMacabre
            | DpsAbilityKind::QueensFang
            | DpsAbilityKind::BroodingShadows
            | DpsAbilityKind::Backstab
            | DpsAbilityKind::FinalStratagem
            | DpsAbilityKind::DoubleStrike
            | DpsAbilityKind::Warbound
            | DpsAbilityKind::OwedInBlood
            | DpsAbilityKind::BloodboundSpirit
            | DpsAbilityKind::ReignInBlood
            | DpsAbilityKind::HeartSplitter
            | DpsAbilityKind::Rupture
            | DpsAbilityKind::Slaughter
            | DpsAbilityKind::BloodArc
            | DpsAbilityKind::ReaversEdge
            | DpsAbilityKind::GundeAttack
            | DpsAbilityKind::ButchersHook
            | DpsAbilityKind::GrimCarve
            | DpsAbilityKind::WeaponFrostVolley
            | DpsAbilityKind::WeaponArcaneChannel
            | DpsAbilityKind::WeaponChainLightning
            | DpsAbilityKind::WeaponShadowMark
            | DpsAbilityKind::WeaponCleaveCharge
            | DpsAbilityKind::WeaponFrontalCone
            | DpsAbilityKind::WeaponInstantAoe
    )
}
// Dark Prophecy requires Skill, including non-offensive buffs, but its cooked
// query excludes AutoAttack, Movement and Utility among maintained player casts.
pub(super) fn is_dark_prophecy_commit(kind: DpsAbilityKind) -> bool {
    is_primary_skill_commit(kind)
        && !matches!(
            kind,
            DpsAbilityKind::TariqAttack
                | DpsAbilityKind::ElarionShoot
                | DpsAbilityKind::MaraAttack
                | DpsAbilityKind::GundeAttack
                | DpsAbilityKind::LeapSmash
                | DpsAbilityKind::Warbound
                | DpsAbilityKind::BroodingShadows
                | DpsAbilityKind::OwedInBlood
        )
}

pub(super) fn is_offensive_skill_commit(kind: DpsAbilityKind) -> bool {
    is_primary_skill_commit(kind)
        && !matches!(
            kind,
            DpsAbilityKind::IceBlitz
                | DpsAbilityKind::WintersBlessing
                | DpsAbilityKind::WrathOfWinter
                | DpsAbilityKind::ThunderCall
                | DpsAbilityKind::FocusedWrath
                | DpsAbilityKind::LunarlightMark
                | DpsAbilityKind::SkystridersGrace
                | DpsAbilityKind::SkystridersSupremacy
                | DpsAbilityKind::MaidenOfDeath
                | DpsAbilityKind::MatriarchMacabre
                | DpsAbilityKind::BroodingShadows
                | DpsAbilityKind::FinalStratagem
                | DpsAbilityKind::BloodboundSpirit
                | DpsAbilityKind::ReignInBlood
        )
}
pub(super) fn is_hunters_focus_commit(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::InfernalWave
            | DpsAbilityKind::SearingBlaze
            | DpsAbilityKind::EngulfingFlames
            | DpsAbilityKind::FireFrogs
            | DpsAbilityKind::Pyromania
            | DpsAbilityKind::BurstingIce
            | DpsAbilityKind::FrostBolt
            | DpsAbilityKind::GlacialBlast
            | DpsAbilityKind::FreezingTorrent
            | DpsAbilityKind::ColdSnap
            | DpsAbilityKind::IceComet
            | DpsAbilityKind::FlightOfTheNavir
            | DpsAbilityKind::HeavyStrike
            | DpsAbilityKind::TariqChainLightning
            | DpsAbilityKind::WildSwing
            | DpsAbilityKind::SkullCrusher
            | DpsAbilityKind::CullingStrike
            | DpsAbilityKind::FaceBreaker
            | DpsAbilityKind::Multishot
            | DpsAbilityKind::FocusedShot
            | DpsAbilityKind::HighwindArrow
            | DpsAbilityKind::HeartseekerBarrage
            | DpsAbilityKind::EventHorizon
            | DpsAbilityKind::CelestialShot
            | DpsAbilityKind::StarfallVolley
            | DpsAbilityKind::SkitteringBlades
            | DpsAbilityKind::HemorrhagingStrike
            | DpsAbilityKind::WidowsBite
            | DpsAbilityKind::QueensFang
            | DpsAbilityKind::Backstab
            | DpsAbilityKind::DoubleStrike
            | DpsAbilityKind::HeartSplitter
            | DpsAbilityKind::Rupture
            | DpsAbilityKind::BloodArc
            | DpsAbilityKind::ReaversEdge
            | DpsAbilityKind::GrimCarve
            | DpsAbilityKind::WeaponArcaneChannel
            | DpsAbilityKind::WeaponChainLightning
            | DpsAbilityKind::WeaponShadowMark
    )
}

pub(super) fn is_ardeos_hero_dot(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::SearingBlaze
            | DpsAbilityKind::EngulfingFlames
            | DpsAbilityKind::Incinerate
            | DpsAbilityKind::FireFrogs
            | DpsAbilityKind::FireBall
    )
}
pub(super) fn is_mara_poison(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::CausticPoison
            | DpsAbilityKind::SeethingPoison
            | DpsAbilityKind::VolatilePoison
            | DpsAbilityKind::VolatilePoisonEruption
            | DpsAbilityKind::SeethingBurst
            | DpsAbilityKind::CorrosiveSpill
            | DpsAbilityKind::Hemotoxin
            | DpsAbilityKind::HemotoxinEruption
    )
}
pub(super) fn is_spontaneous_combustion_relevant_dot(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::SearingBlaze | DpsAbilityKind::EngulfingFlames
    )
}
pub(super) fn is_pyrophibian_relevant_dot(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::SearingBlaze
            | DpsAbilityKind::EngulfingFlames
            | DpsAbilityKind::Incinerate
            | DpsAbilityKind::FireFrogs
            | DpsAbilityKind::FireBall
    )
}
pub(super) fn ability_kind_id(kind: DpsAbilityKind) -> &'static str {
    ability_metadata(kind).id
}

pub(super) fn ability_kind_from_id(id: &str) -> Option<DpsAbilityKind> {
    Some(match id {
        "infernal-wave" => DpsAbilityKind::InfernalWave,
        "detonate" => DpsAbilityKind::Detonate,
        "apocalypse" => DpsAbilityKind::Apocalypse,
        "searing-blaze" => DpsAbilityKind::SearingBlaze,
        "engulfing-flames" => DpsAbilityKind::EngulfingFlames,
        "incinerate" => DpsAbilityKind::Incinerate,
        "fire-frogs" => DpsAbilityKind::FireFrogs,
        "wildfire" => DpsAbilityKind::Wildfire,
        "pyromania" => DpsAbilityKind::Pyromania,
        "fire-ball" => DpsAbilityKind::FireBall,
        "ice-blitz" => DpsAbilityKind::IceBlitz,
        "bursting-ice" => DpsAbilityKind::BurstingIce,
        "frost-bolt" => DpsAbilityKind::FrostBolt,
        "glacial-blast" => DpsAbilityKind::GlacialBlast,
        "freezing-torrent" => DpsAbilityKind::FreezingTorrent,
        "winters-blessing" => DpsAbilityKind::WintersBlessing,
        "wrath-of-winter" => DpsAbilityKind::WrathOfWinter,
        "cold-snap" => DpsAbilityKind::ColdSnap,
        "ice-comet" => DpsAbilityKind::IceComet,
        "flight-of-the-navir" => DpsAbilityKind::FlightOfTheNavir,
        "anima-spike" => DpsAbilityKind::AnimaSpike,
        "frost-swallow" => DpsAbilityKind::FrostSwallow,
        "hammer-storm" => DpsAbilityKind::HammerStorm,
        "heavy-strike" => DpsAbilityKind::HeavyStrike,
        "tariq-chain-lightning" => DpsAbilityKind::TariqChainLightning,
        "thunder-call" => DpsAbilityKind::ThunderCall,
        "wild-swing" => DpsAbilityKind::WildSwing,
        "focused-wrath" => DpsAbilityKind::FocusedWrath,
        "raging-tempest" => DpsAbilityKind::RagingTempest,
        "skull-crusher" => DpsAbilityKind::SkullCrusher,
        "culling-strike" => DpsAbilityKind::CullingStrike,
        "leap-smash" => DpsAbilityKind::LeapSmash,
        "face-breaker" => DpsAbilityKind::FaceBreaker,
        "tariq-attack" => DpsAbilityKind::TariqAttack,
        "multishot" => DpsAbilityKind::Multishot,
        "focused-shot" => DpsAbilityKind::FocusedShot,
        "highwind-arrow" => DpsAbilityKind::HighwindArrow,
        "heartseeker-barrage" => DpsAbilityKind::HeartseekerBarrage,
        "lunarlight-mark" => DpsAbilityKind::LunarlightMark,
        "skystriders-grace" => DpsAbilityKind::SkystridersGrace,
        "elarion-shoot" => DpsAbilityKind::ElarionShoot,
        "event-horizon" => DpsAbilityKind::EventHorizon,
        "skystriders-supremacy" => DpsAbilityKind::SkystridersSupremacy,
        "celestial-shot" => DpsAbilityKind::CelestialShot,
        "starfall-volley" => DpsAbilityKind::StarfallVolley,
        "lunarlight-salvo" => DpsAbilityKind::LunarlightSalvo,
        "lunarlight-eruption" => DpsAbilityKind::LunarlightEruption,
        "skittering-blades" => DpsAbilityKind::SkitteringBlades,
        "arachnid-assault" => DpsAbilityKind::ArachnidAssault,
        "mara-attack" => DpsAbilityKind::MaraAttack,
        "hemorrhaging-strike" => DpsAbilityKind::HemorrhagingStrike,
        "widows-bite" => DpsAbilityKind::WidowsBite,
        "maiden-of-death" => DpsAbilityKind::MaidenOfDeath,
        "matriarch-macabre" => DpsAbilityKind::MatriarchMacabre,
        "queens-fang" => DpsAbilityKind::QueensFang,
        "brooding-shadows" => DpsAbilityKind::BroodingShadows,
        "backstab" => DpsAbilityKind::Backstab,
        "final-stratagem" => DpsAbilityKind::FinalStratagem,
        "caustic-poison" => DpsAbilityKind::CausticPoison,
        "seething-poison" => DpsAbilityKind::SeethingPoison,
        "volatile-poison" => DpsAbilityKind::VolatilePoison,
        "volatile-poison-eruption" => DpsAbilityKind::VolatilePoisonEruption,
        "seething-burst" => DpsAbilityKind::SeethingBurst,
        "corrosive-spill" => DpsAbilityKind::CorrosiveSpill,
        "hemotoxin" => DpsAbilityKind::Hemotoxin,
        "hemotoxin-eruption" => DpsAbilityKind::HemotoxinEruption,
        "double-strike" => DpsAbilityKind::DoubleStrike,
        "warbound" => DpsAbilityKind::Warbound,
        "owed-in-blood" => DpsAbilityKind::OwedInBlood,
        "bloodbound-spirit" => DpsAbilityKind::BloodboundSpirit,
        "reign-in-blood" => DpsAbilityKind::ReignInBlood,
        "heart-splitter" => DpsAbilityKind::HeartSplitter,
        "rupture" => DpsAbilityKind::Rupture,
        "slaughter" => DpsAbilityKind::Slaughter,
        "blood-arc" => DpsAbilityKind::BloodArc,
        "reavers-edge" => DpsAbilityKind::ReaversEdge,
        "gunde-attack" => DpsAbilityKind::GundeAttack,
        "butchers-hook" => DpsAbilityKind::ButchersHook,
        "grim-carve" => DpsAbilityKind::GrimCarve,
        "rend" => DpsAbilityKind::Rend,
        "exsanguinate" => DpsAbilityKind::Exsanguinate,
        "bloodcraze" => DpsAbilityKind::Bloodcraze,
        "ravens-precision" => DpsAbilityKind::RavensPrecision,
        "oathshatter" => DpsAbilityKind::Oathshatter,
        "weapon-frost-volley" => DpsAbilityKind::WeaponFrostVolley,
        "weapon-arcane-channel" => DpsAbilityKind::WeaponArcaneChannel,
        "weapon-chain-lightning" => DpsAbilityKind::WeaponChainLightning,
        "weapon-shadow-mark" => DpsAbilityKind::WeaponShadowMark,
        "weapon-cleave-charge" => DpsAbilityKind::WeaponCleaveCharge,
        "weapon-frontal-cone" => DpsAbilityKind::WeaponFrontalCone,
        "weapon-instant-aoe" => DpsAbilityKind::WeaponInstantAoe,
        _ => return None,
    })
}
