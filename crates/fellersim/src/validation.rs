use std::collections::{BTreeMap, BTreeSet};

use crate::*;

const MAX_TEXT_FIELD_BYTES: usize = 4 * 1024;
const MAX_PROFILE_ABILITIES: usize = 64;
const MAX_PROFILE_TALENTS: usize = 32;
const MAX_PROFILE_MECHANICS: usize = 128;
const MAX_PROFILE_TARGET_EFFECTS: usize = 64;
const MAX_PROFILE_SCENARIO_NO_OPS: usize = 64;
const MAX_PROFILE_UPTIME_NAMES: usize = 256;
const MAX_EVIDENCE_CLAIMS: usize = 256;
const MAX_APL_RULES: usize = 128;
const MAX_APL_CONDITION_NODES: usize = 1_024;
const MAX_APL_DEPTH: usize = 64;
const MAX_APL_COMMENTS: usize = 1_024;
const MAX_PARAMETERS_PER_MODEL: usize = 64;
const MAX_DYNAMIC_PARAMETER_ABS: f64 = 1_000_000.0;
const MAX_MODEL_TIME_MS: u64 = 20 * 60 * 1_000;
const MAX_DIRECT_HITS: u32 = 256;
const MAX_SCHEDULED_TICKS: u64 = 256;
const MAX_ABILITY_CHARGES: u32 = 32;
const MAX_DOT_STACKS: u32 = 1_024;

fn invalid_structure(message: impl Into<String>) -> SimulationError {
    SimulationError::new(SimulationErrorCode::InvalidBuild, message)
}

fn validate_text(value: &str) -> Result<(), SimulationError> {
    if value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(invalid_structure(format!(
            "simulation text fields must not exceed {MAX_TEXT_FIELD_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_parameters(parameters: &BTreeMap<String, f64>) -> Result<(), SimulationError> {
    if parameters.len() > MAX_PARAMETERS_PER_MODEL {
        return Err(invalid_structure(format!(
            "simulation models must not contain more than {MAX_PARAMETERS_PER_MODEL} parameters"
        )));
    }
    for (name, value) in parameters {
        validate_text(name)?;
        if !value.is_finite() || value.abs() > MAX_DYNAMIC_PARAMETER_ABS {
            return Err(invalid_structure(
                "simulation model parameters must be finite and within the executable range",
            ));
        }
        let lowercase_name = name.to_ascii_lowercase();
        let exceeds_time_limit = if lowercase_name.ends_with("seconds") {
            value.abs() > MAX_MODEL_TIME_MS as f64 / 1_000.0
        } else if lowercase_name.ends_with("ms") || lowercase_name.ends_with("milliseconds") {
            value.abs() > MAX_MODEL_TIME_MS as f64
        } else {
            false
        };
        if exceeds_time_limit {
            return Err(invalid_structure(
                "simulation timing parameters must not exceed 20 minutes",
            ));
        }
    }
    Ok(())
}

fn validate_apl_reference_text(reference: &AplNumericReference) -> Result<(), SimulationError> {
    match reference {
        AplNumericReference::CooldownRemaining { ability_id }
        | AplNumericReference::CooldownCharges { ability_id }
        | AplNumericReference::DotRemaining { ability_id } => validate_text(ability_id),
        AplNumericReference::TargetEffectRemaining { effect_id }
        | AplNumericReference::TargetEffectStacks { effect_id } => validate_text(effect_id),
        _ => Ok(()),
    }
}

fn validate_apl_structure(apl: &ActionPriorityListV2) -> Result<(), SimulationError> {
    if apl.rules.len() > MAX_APL_RULES {
        return Err(invalid_apl(
            format!("An action priority list supports at most {MAX_APL_RULES} rules."),
            Vec::new(),
        ));
    }
    let mut comment_count = apl.trailing_comments.len();
    for comment in &apl.trailing_comments {
        validate_text(comment)?;
    }

    let mut node_count = 0_usize;
    let mut stack = Vec::new();
    for rule in &apl.rules {
        validate_text(&rule.id)?;
        validate_text(&rule.ability_id)?;
        comment_count = comment_count
            .checked_add(rule.leading_comments.len())
            .and_then(|count| count.checked_add(usize::from(rule.inline_comment.is_some())))
            .ok_or_else(|| invalid_apl("The action priority list is too large.", Vec::new()))?;
        for comment in &rule.leading_comments {
            validate_text(comment)?;
        }
        if let Some(comment) = &rule.inline_comment {
            validate_text(comment)?;
        }
        if let Some(condition) = &rule.condition {
            stack.push((condition, 0_usize));
        }
    }
    if comment_count > MAX_APL_COMMENTS {
        return Err(invalid_apl(
            format!("An action priority list supports at most {MAX_APL_COMMENTS} comments."),
            Vec::new(),
        ));
    }

    while let Some((node, depth)) = stack.pop() {
        node_count += 1;
        if node_count > MAX_APL_CONDITION_NODES {
            return Err(invalid_apl(
                format!(
                    "An action priority list supports at most {MAX_APL_CONDITION_NODES} condition nodes."
                ),
                Vec::new(),
            ));
        }
        if depth > MAX_APL_DEPTH {
            return Err(invalid_apl(
                "The APL condition is nested too deeply.",
                Vec::new(),
            ));
        }
        validate_text(&node.id)?;
        match &node.expression {
            AplExpression::All { children } | AplExpression::Any { children } => {
                if children.len() > MAX_APL_CONDITION_NODES.saturating_sub(node_count) {
                    return Err(invalid_apl(
                        format!(
                            "An action priority list supports at most {MAX_APL_CONDITION_NODES} condition nodes."
                        ),
                        Vec::new(),
                    ));
                }
                stack.extend(children.iter().map(|child| (child, depth + 1)));
            }
            AplExpression::Not { child } => stack.push((child, depth + 1)),
            AplExpression::BooleanReference { reference } => match reference {
                AplBooleanReference::CooldownReady { ability_id }
                | AplBooleanReference::DotActive { ability_id } => validate_text(ability_id)?,
                AplBooleanReference::TargetEffectActive { effect_id } => validate_text(effect_id)?,
                AplBooleanReference::LegendaryEquipped { item_id } => validate_text(item_id)?,
                AplBooleanReference::TalentSelected { talent_id } => validate_text(talent_id)?,
                AplBooleanReference::BuffActive { .. } => {}
            },
            AplExpression::Comparison { left, right, .. } => {
                for operand in [left, right] {
                    match operand {
                        AplNumericOperand::Number { value }
                            if !value.is_finite() || value.abs() > MAX_DYNAMIC_PARAMETER_ABS =>
                        {
                            return Err(invalid_apl(
                                "APL numbers must be finite and within the executable range.",
                                vec![node.id.clone()],
                            ));
                        }
                        AplNumericOperand::Reference { reference } => {
                            validate_apl_reference_text(reference)?;
                        }
                        AplNumericOperand::Number { .. } => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_request_structure(request: &SimulationRequest) -> Result<(), SimulationError> {
    for value in [
        request.run_id.as_str(),
        request.data_build_id.as_str(),
        request.model_version.as_str(),
        request.profile_fingerprint.as_str(),
        request.hero_id.as_str(),
        request.scenario_id.as_str(),
        request.seed.as_str(),
        request.evidence.model_revision.as_str(),
        request.evidence.evidence_fingerprint.as_str(),
        request.profile.hero_id.as_str(),
    ] {
        validate_text(value)?;
    }
    if request.evidence.claims.len() > MAX_EVIDENCE_CLAIMS
        || request.profile.evidence_claim_ids.len() > MAX_EVIDENCE_CLAIMS
    {
        return Err(invalid_structure(format!(
            "simulation evidence supports at most {MAX_EVIDENCE_CLAIMS} claims"
        )));
    }
    if request.profile.abilities.len() > MAX_PROFILE_ABILITIES
        || request.profile.talents.len() > MAX_PROFILE_TALENTS
        || request.profile.mechanics.len() > MAX_PROFILE_MECHANICS
        || request.profile.apl_target_effects.len() > MAX_PROFILE_TARGET_EFFECTS
        || request.profile.scenario_no_op_abilities.len() > MAX_PROFILE_SCENARIO_NO_OPS
        || request.profile.uptime_names.len() > MAX_PROFILE_UPTIME_NAMES
    {
        return Err(invalid_structure(
            "the normalized profile exceeds the supported structural limits",
        ));
    }

    for claim in &request.evidence.claims {
        validate_text(&claim.id)?;
        validate_text(&claim.source_id)?;
        validate_text(&claim.source_name)?;
    }
    for claim_id in &request.profile.evidence_claim_ids {
        validate_text(claim_id)?;
    }
    for ability in &request.profile.abilities {
        validate_text(&ability.id)?;
        validate_text(&ability.name)?;
        validate_parameters(&ability.mechanic_parameters)?;
    }
    for effect in &request.profile.apl_target_effects {
        validate_text(&effect.id)?;
        validate_text(&effect.name)?;
        match &effect.source {
            AplTargetEffectSource::AbilityDot { ability_id } => validate_text(ability_id)?,
            AplTargetEffectSource::MechanicTargetBuff {
                mechanic_instance_id,
            } => validate_text(mechanic_instance_id)?,
        }
    }
    for ability in &request.profile.scenario_no_op_abilities {
        validate_text(&ability.id)?;
        validate_text(&ability.name)?;
        validate_text(&ability.reason)?;
    }
    for talent in &request.profile.talents {
        validate_text(&talent.id)?;
        validate_text(&talent.name)?;
        validate_text(&talent.mechanic_id)?;
        if let Some(reason) = &talent.reason {
            validate_text(reason)?;
        }
        validate_parameters(&talent.parameters)?;
    }
    for mechanic in &request.profile.mechanics {
        validate_text(&mechanic.instance_id)?;
        validate_text(&mechanic.source_id)?;
        validate_text(&mechanic.source_name)?;
        validate_text(&mechanic.mechanic_id)?;
        if let Some(reason) = &mechanic.reason {
            validate_text(reason)?;
        }
        validate_parameters(&mechanic.parameters)?;
    }
    for (id, name) in &request.profile.uptime_names {
        validate_text(id)?;
        validate_text(name)?;
        if id.trim().is_empty()
            || name.trim().is_empty()
            || name.starts_with("Unsupported:")
            || name == id
        {
            return Err(invalid_structure(
                "uptime entries require datamined display names",
            ));
        }
    }
    validate_apl_structure(&request.action_priority_list)
}

fn scheduled_tick_count(duration_ms: u64, interval_ms: u64) -> Option<u64> {
    if interval_ms == 0 {
        return None;
    }
    duration_ms
        .checked_add(interval_ms - 1)
        .map(|value| value / interval_ms)
}

pub(crate) fn validate(request: &SimulationRequest) -> Result<u64, SimulationError> {
    validate_request_structure(request)?;
    let hero_contract = HeroIdentity::from_id(&request.hero_id)
        .ok_or_else(|| {
            SimulationError::new(
                SimulationErrorCode::UnsupportedHero,
                "Fellersim supports Ardeos, Rime, Tariq, Elarion, Mara, and Gunde",
            )
        })?
        .contract();
    if request.schema_version != SIMULATOR_SCHEMA_VERSION
        || request.hero_id != hero_contract.hero_id
        || request.model_version != hero_contract.model_version
        || request.scenario_id != hero_contract.scenario_id
        || request.profile.hero_id != request.hero_id
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "request does not match the current Fellersim contract",
        ));
    }
    if request.scenario.schema_version != STATIONARY_DUMMY_SCENARIO_SCHEMA_VERSION
        || !(MIN_STATIONARY_DUMMY_TARGETS..=MAX_STATIONARY_DUMMY_TARGETS)
            .contains(&request.scenario.target_count)
        || request.scenario.target_distance_units != hero_contract.maximum_combat_range_units
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            format!(
                "stationary dummy targets must number between {MIN_STATIONARY_DUMMY_TARGETS} and {MAX_STATIONARY_DUMMY_TARGETS} and stand exactly {} Unreal units away",
                hero_contract.maximum_combat_range_units
            ),
        ));
    }
    let expected_scope = hero_contract.evidence_scope(request.scenario.target_count == 1);
    if request.evidence.schema_version != DPS_EVIDENCE_SNAPSHOT_SCHEMA_VERSION
        || request.evidence.build_id != request.data_build_id
        || request.evidence.scope != expected_scope
        || request.evidence.model_revision.trim().is_empty()
        || request.evidence.evidence_fingerprint.trim().is_empty()
        || request.evidence.claims.is_empty()
    {
        return Err(SimulationError::new(
            SimulationErrorCode::DataVersionMismatch,
            "request does not include current DPS evidence for this dummy scenario",
        ));
    }
    let mut evidence_claim_ids = BTreeSet::new();
    let partial_claims = request
        .evidence
        .claims
        .iter()
        .filter_map(|claim| {
            if claim.id.trim().is_empty()
                || claim.source_id.trim().is_empty()
                || claim.source_name.trim().is_empty()
                || !evidence_claim_ids.insert(claim.id.as_str())
            {
                return Some(claim.source_name.clone());
            }
            (claim.status == DpsEvidenceClaimStatus::Partial).then(|| claim.source_name.clone())
        })
        .collect::<Vec<_>>();
    if !partial_claims.is_empty() {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: "The loadout contains outgoing-DPS mechanics without current evidence.".into(),
            sources: partial_claims,
        });
    }
    let expected_claim_ids = request
        .profile
        .evidence_claim_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected_claim_ids.is_empty() || expected_claim_ids != evidence_claim_ids {
        return Err(SimulationError::new(
            SimulationErrorCode::DataVersionMismatch,
            "the normalized profile and DPS evidence snapshot do not cover the same claims",
        ));
    }
    if !(MIN_SIMULATION_ITERATIONS..=MAX_SIMULATION_ITERATIONS).contains(&request.iterations) {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            format!(
                "iterations must be between {MIN_SIMULATION_ITERATIONS} and {MAX_SIMULATION_ITERATIONS}"
            ),
        ));
    }
    let profile_numbers = [
        request.profile.power,
        request.profile.critical_strike,
        request.profile.critical_rating,
        request.profile.critical_multiplier,
        request.profile.expertise,
        request.profile.expertise_rating,
        request.profile.haste,
        request.profile.haste_rating,
        request.profile.cooldown_recovery,
        request.profile.spirit,
        request.profile.spirit_rating,
        request.profile.max_primary_resource,
        request.profile.max_spirit,
        request.profile.heroism_haste,
    ];
    if profile_numbers
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_DYNAMIC_PARAMETER_ABS)
        || request.profile.power <= 0.0
        || request.profile.critical_multiplier <= 0.0
        || request.profile.cooldown_recovery <= 0.0
        || request.profile.max_primary_resource <= 0.0
        || request.profile.max_secondary_resource == 0
        || request.profile.max_spirit <= 0.0
        || request.profile.heroism_duration_ms == 0
        || request.profile.heroism_duration_ms > MAX_MODEL_TIME_MS
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "normalized hero stats must be finite and positive",
        ));
    }
    let uncovered = request
        .profile
        .mechanics
        .iter()
        .filter(|mechanic| mechanic.classification == MechanicClassification::Uncovered)
        .map(|mechanic| mechanic.source_name.clone())
        .collect::<Vec<_>>();
    if !uncovered.is_empty() {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: "The loadout contains combat mechanics that are not covered by this model."
                .into(),
            sources: uncovered,
        });
    }
    let required: &[DpsAbilityKind] = if request.hero_id == RIME_HERO_ID {
        &[
            DpsAbilityKind::IceBlitz,
            DpsAbilityKind::BurstingIce,
            DpsAbilityKind::FrostBolt,
            DpsAbilityKind::GlacialBlast,
            DpsAbilityKind::FreezingTorrent,
            DpsAbilityKind::WintersBlessing,
            DpsAbilityKind::WrathOfWinter,
            DpsAbilityKind::ColdSnap,
            DpsAbilityKind::IceComet,
            DpsAbilityKind::FlightOfTheNavir,
            DpsAbilityKind::AnimaSpike,
        ]
    } else if request.hero_id == TARIQ_HERO_ID {
        &[
            DpsAbilityKind::HammerStorm,
            DpsAbilityKind::HeavyStrike,
            DpsAbilityKind::TariqChainLightning,
            DpsAbilityKind::ThunderCall,
            DpsAbilityKind::WildSwing,
            DpsAbilityKind::FocusedWrath,
            DpsAbilityKind::RagingTempest,
            DpsAbilityKind::SkullCrusher,
            DpsAbilityKind::CullingStrike,
            DpsAbilityKind::LeapSmash,
            DpsAbilityKind::FaceBreaker,
            DpsAbilityKind::TariqAttack,
        ]
    } else if request.hero_id == ELARION_HERO_ID {
        &[
            DpsAbilityKind::Multishot,
            DpsAbilityKind::FocusedShot,
            DpsAbilityKind::HighwindArrow,
            DpsAbilityKind::HeartseekerBarrage,
            DpsAbilityKind::LunarlightMark,
            DpsAbilityKind::SkystridersGrace,
            DpsAbilityKind::ElarionShoot,
            DpsAbilityKind::EventHorizon,
            DpsAbilityKind::SkystridersSupremacy,
            DpsAbilityKind::CelestialShot,
            DpsAbilityKind::StarfallVolley,
            DpsAbilityKind::LunarlightSalvo,
            DpsAbilityKind::LunarlightEruption,
        ]
    } else if request.hero_id == MARA_HERO_ID {
        &[
            DpsAbilityKind::SkitteringBlades,
            DpsAbilityKind::ArachnidAssault,
            DpsAbilityKind::MaraAttack,
            DpsAbilityKind::HemorrhagingStrike,
            DpsAbilityKind::WidowsBite,
            DpsAbilityKind::MaidenOfDeath,
            DpsAbilityKind::MatriarchMacabre,
            DpsAbilityKind::QueensFang,
            DpsAbilityKind::BroodingShadows,
            DpsAbilityKind::Backstab,
            DpsAbilityKind::FinalStratagem,
            DpsAbilityKind::CausticPoison,
            DpsAbilityKind::SeethingPoison,
            DpsAbilityKind::VolatilePoison,
            DpsAbilityKind::VolatilePoisonEruption,
            DpsAbilityKind::SeethingBurst,
            DpsAbilityKind::CorrosiveSpill,
            DpsAbilityKind::Hemotoxin,
            DpsAbilityKind::HemotoxinEruption,
        ]
    } else if request.hero_id == GUNDE_HERO_ID {
        &[
            DpsAbilityKind::DoubleStrike,
            DpsAbilityKind::Warbound,
            DpsAbilityKind::OwedInBlood,
            DpsAbilityKind::BloodboundSpirit,
            DpsAbilityKind::ReignInBlood,
            DpsAbilityKind::HeartSplitter,
            DpsAbilityKind::Rupture,
            DpsAbilityKind::Slaughter,
            DpsAbilityKind::BloodArc,
            DpsAbilityKind::ReaversEdge,
            DpsAbilityKind::GundeAttack,
            DpsAbilityKind::ButchersHook,
            DpsAbilityKind::GrimCarve,
            DpsAbilityKind::Rend,
            DpsAbilityKind::Exsanguinate,
            DpsAbilityKind::Bloodcraze,
            DpsAbilityKind::RavensPrecision,
            DpsAbilityKind::Oathshatter,
        ]
    } else {
        &[
            DpsAbilityKind::InfernalWave,
            DpsAbilityKind::Detonate,
            DpsAbilityKind::SearingBlaze,
            DpsAbilityKind::EngulfingFlames,
            DpsAbilityKind::FireBall,
        ]
    };
    if required.iter().any(|kind| {
        !request
            .profile
            .abilities
            .iter()
            .any(|ability| ability.kind == *kind)
    }) {
        return Err(SimulationError::new(
            SimulationErrorCode::UncoveredMechanics,
            "The current combat catalog is missing a required hero ability model",
        ));
    }
    let mut ability_kinds = BTreeMap::new();
    let mut modeled_ability_ids = BTreeSet::new();
    for ability in &request.profile.abilities {
        if !hero_allows_ability(&request.hero_id, ability.kind) {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!(
                    "{} does not belong to the current {} ability contract",
                    ability.name, request.hero_id
                ),
                sources: vec![ability.id.clone()],
            });
        }
        let channel_ticks = ability.channel.as_ref().and_then(|channel| {
            scheduled_tick_count(channel.duration_ms, channel.tick_interval_ms)
        });
        let dot_ticks = ability
            .dot
            .as_ref()
            .and_then(|dot| scheduled_tick_count(dot.duration_ms, dot.period_ms));
        if ability.id.is_empty()
            || ability.name.is_empty()
            || !ability.power_coefficient.is_finite()
            || !ability.damage_spread.is_finite()
            || ability.damage_spread < 0.0
            || !ability.primary_resource_generated.is_finite()
            || !ability.spirit_cost.is_finite()
            || ability.power_coefficient.abs() > MAX_DYNAMIC_PARAMETER_ABS
            || ability.damage_spread.abs() > MAX_DYNAMIC_PARAMETER_ABS
            || ability.primary_resource_generated.abs() > MAX_DYNAMIC_PARAMETER_ABS
            || ability.spirit_cost.abs() > MAX_DYNAMIC_PARAMETER_ABS
            || ability
                .mechanic_parameters
                .values()
                .any(|value| !value.is_finite())
            || (ability.power_coefficient > 0.0 && ability.direct_hits == 0)
            || ability.max_targets == 0
            || ability.maximum_charges == 0
            || ability.direct_hits > MAX_DIRECT_HITS
            || ability.max_targets > MAX_STATIONARY_DUMMY_TARGETS
            || ability.maximum_charges > MAX_ABILITY_CHARGES
            || [
                ability.cast_time_ms,
                ability.gcd_ms,
                ability.cooldown_ms,
                ability.effect_duration_ms,
                ability.first_hit_delay_ms,
                ability.hit_interval_ms,
                ability.dot_application_delay_ms,
                ability.dot_extension_ms,
            ]
            .into_iter()
            .any(|value| value > MAX_MODEL_TIME_MS)
            || (ability.dot_application_delay_ms > 0 && ability.applies_dot_kind.is_none())
            || ability_kinds.insert(ability.kind, ()).is_some()
            || !modeled_ability_ids.insert(ability.id.as_str())
            || ability.channel.as_ref().is_some_and(|channel| {
                channel.duration_ms == 0
                    || channel.tick_interval_ms == 0
                    || channel.duration_ms > MAX_MODEL_TIME_MS
                    || channel.tick_interval_ms > MAX_MODEL_TIME_MS
                    || channel_ticks.is_none_or(|ticks| ticks > MAX_SCHEDULED_TICKS)
            })
            || ability.dot.as_ref().is_some_and(|dot| {
                !dot.power_coefficient.is_finite()
                    || !dot.damage_spread.is_finite()
                    || dot.power_coefficient.abs() > MAX_DYNAMIC_PARAMETER_ABS
                    || dot.damage_spread.abs() > MAX_DYNAMIC_PARAMETER_ABS
                    || dot.damage_spread < 0.0
                    || !dot.cinder_proc_chance.is_finite()
                    || !(0.0..=1.0).contains(&dot.cinder_proc_chance)
                    || !dot.cinders_on_proc.is_finite()
                    || dot.cinders_on_proc.abs() > MAX_DYNAMIC_PARAMETER_ABS
                    || dot.cinders_on_proc < 0.0
                    || !dot.stack_damage_increase.is_finite()
                    || dot.stack_damage_increase.abs() > MAX_DYNAMIC_PARAMETER_ABS
                    || dot.maximum_stacks == 0
                    || dot.maximum_stacks > MAX_DOT_STACKS
                    || dot.duration_ms == 0
                    || dot.period_ms == 0
                    || dot.duration_ms > MAX_MODEL_TIME_MS
                    || dot.period_ms > MAX_MODEL_TIME_MS
                    || dot_ticks.is_none_or(|ticks| ticks > MAX_SCHEDULED_TICKS)
            })
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "normalized hero abilities must be unique, finite, and complete",
            ));
        }
        if ability.manually_castable
            && ability.cast_time_ms == 0
            && ability.gcd_ms == 0
            && ability.cooldown_ms == 0
            && ability.channel.is_none()
            && ability.secondary_resource_cost == 0
            && ability.spirit_cost <= 0.0
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: "an instantaneous action requires a cooldown, cast, global cooldown, channel, or resource lock"
                    .into(),
                sources: vec![ability.id.clone()],
            });
        }
        let required_mechanic_parameters = ability_metadata(ability.kind).required_parameters;
        if required_mechanic_parameters
            .iter()
            .any(|key| !ability.mechanic_parameters.contains_key(*key))
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!(
                    "{} is missing an extracted mechanic parameter",
                    ability.name
                ),
                sources: vec![ability.id.clone()],
            });
        }
        if ability.kind == DpsAbilityKind::Detonate
            && ability_param(ability, parameter_key!("hitsPerTarget")).round() < 1.0
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: "Detonate must have at least one extracted hit per target".into(),
                sources: vec![ability.id.clone()],
            });
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::ElarionShoot | DpsAbilityKind::GundeAttack
        ) && (ability.manually_castable
            || ability_seconds_parameter(ability, parameter_key!("swingDurationSeconds")) == 0)
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "Auto attacks require automatic scheduling and a positive swing duration",
            ));
        }
        if ability.kind == DpsAbilityKind::TariqAttack
            && (ability_seconds_parameter(ability, parameter_key!("swingDurationSeconds")) == 0
                || ability_param(ability, parameter_key!("hitWindowSeconds"))
                    > ability_param(ability, parameter_key!("swingDurationSeconds")))
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "Attack requires a positive swing duration containing its hit window",
            ));
        }
        if ability.kind == DpsAbilityKind::FireFrogs
            && (ability_param(ability, parameter_key!("assumedServerTickRateHz")) <= 0.0
                || ability_param(ability, parameter_key!("initialPathDistanceUnits")) <= 0.0
                || ability_param(ability, parameter_key!("spawnRadiusUnits")) < 0.0
                || ability_param(ability, parameter_key!("attackRangeUnits")) <= 0.0
                || ability_param(ability, parameter_key!("jumpDurationSeconds")) <= 0.0
                || ability_param(ability, parameter_key!("minimumBatchSpawnDelaySeconds")) < 0.0
                || ability_param(ability, parameter_key!("minimumBatchSpawnDelaySeconds"))
                    > ability_param(ability, parameter_key!("maximumBatchSpawnDelaySeconds"))
                || ability_param(ability, parameter_key!("minimumJumpLengthUnits")) <= 0.0
                || ability_param(ability, parameter_key!("minimumJumpLengthUnits"))
                    > ability_param(ability, parameter_key!("maximumJumpLengthUnits"))
                || ability_param(ability, parameter_key!("minimumJumpPeriodSeconds")) <= 0.0
                || ability_param(ability, parameter_key!("minimumJumpPeriodSeconds"))
                    > ability_param(ability, parameter_key!("maximumJumpPeriodSeconds")))
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: "Fire Frogs movement parameters must form positive extracted ranges"
                    .into(),
                sources: vec![ability.id.clone()],
            });
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::BurstingIce | DpsAbilityKind::IceComet | DpsAbilityKind::RagingTempest
        ) && ability_seconds_parameter(ability, parameter_key!("pulsePeriodSeconds")) == 0
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: format!(
                    "{} must have a positive extracted pulse period",
                    ability.name
                ),
                sources: vec![ability.id.clone()],
            });
        }
        if ability.kind == DpsAbilityKind::WrathOfWinter
            && ability_seconds_parameter(ability, parameter_key!("volleyPeriodSeconds")) == 0
        {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: "Wrath of Winter must have a positive extracted volley period".into(),
                sources: vec![ability.id.clone()],
            });
        }
        validate_rime_ability_values(ability)?;
    }
    let expected_no_ops = REQUIRED_SCENARIO_NO_OP_ABILITIES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut scenario_no_ops = BTreeSet::new();
    for ability in &request.profile.scenario_no_op_abilities {
        if ability.id.is_empty()
            || ability.name.is_empty()
            || ability.reason.is_empty()
            || modeled_ability_ids.contains(ability.id.as_str())
            || !scenario_no_ops.insert(ability.id.as_str())
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "scenario no-op abilities must be unique, named, explained, and separate from modeled abilities",
            ));
        }
    }
    if request.hero_id == ARDEOS_HERO_ID && scenario_no_ops != expected_no_ops {
        let missing = expected_no_ops
            .difference(&scenario_no_ops)
            .copied()
            .collect::<Vec<_>>();
        let unexpected = scenario_no_ops
            .difference(&expected_no_ops)
            .copied()
            .collect::<Vec<_>>();
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: "The current Ardeos scenario no-op ability coverage is incomplete.".into(),
            sources: missing
                .into_iter()
                .chain(unexpected)
                .map(str::to_string)
                .collect(),
        });
    }
    let mut mechanic_instances = BTreeMap::new();
    for mechanic in &request.profile.mechanics {
        if mechanic.instance_id.is_empty()
            || mechanic.source_id.is_empty()
            || mechanic.source_name.is_empty()
            || mechanic.mechanic_id.is_empty()
            || mechanic.parameters.values().any(|value| !value.is_finite())
            || mechanic_instances
                .insert(mechanic.instance_id.as_str(), ())
                .is_some()
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "dynamic combat mechanics must have unique instances and finite extracted parameters",
            ));
        }
        if mechanic.classification == MechanicClassification::ScenarioNoOp
            && (mechanic.handler != DynamicMechanicHandler::ScenarioNoOp
                || mechanic.reason.as_deref().unwrap_or_default().is_empty())
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "scenario no-op mechanics require an explicit reason",
            ));
        }
        if mechanic.classification == MechanicClassification::Modeled
            && mechanic.handler == DynamicMechanicHandler::ScenarioNoOp
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "modeled mechanics require an executable handler",
            ));
        }
        if mechanic.classification == MechanicClassification::Modeled {
            validate_modeled_mechanic(mechanic)?;
        }
        if mechanic.classification == MechanicClassification::Modeled
            && mechanic.source_id == RUBY_STORM_SOURCE_ID
        {
            let Some(maximum_reach) = ruby_storm_maximum_forward_overlap_distance(mechanic) else {
                return Err(SimulationError {
                    diagnostics: vec![],
                    diagnostics_truncated: false,
                    code: SimulationErrorCode::UncoveredMechanics,
                    message: "Ruby Storm is missing movement parameters required by the current max-range scenario".into(),
                    sources: vec![mechanic.source_id.clone()],
                });
            };
            let supported_melee = matches!(
                hero_contract.hero,
                HeroIdentity::Tariq | HeroIdentity::Mara | HeroIdentity::Gunde
            );
            if maximum_reach >= request.scenario.target_distance_units && !supported_melee {
                return Err(SimulationError {
                    diagnostics: vec![],
                    diagnostics_truncated: false,
                    code: SimulationErrorCode::UncoveredMechanics,
                    message: "Ruby Storm's accepted overlap approximation covers only the maintained melee scenarios".into(),
                    sources: vec![mechanic.source_id.clone()],
                });
            }
            if mechanic.parameters.get("gemPowerDamageIncreasePerPoint") != Some(&0.0)
                || mechanic
                    .parameters
                    .get("movementSpeed")
                    .copied()
                    .unwrap_or(0.0)
                    <= 0.0
            {
                return Err(SimulationError {
                    diagnostics: vec![],
                    diagnostics_truncated: false,
                    code: SimulationErrorCode::UncoveredMechanics,
                    message: "Ruby Storm requires positive movement speed and the reviewed zero gem-power coefficient".into(),
                    sources: vec![mechanic.source_id.clone()],
                });
            }
        }
    }
    validate_apl_target_effect_catalog(&request.profile)?;
    validate_action_priority_list(&request.action_priority_list, &request.profile)?;
    for talent in &request.profile.talents {
        validate_talent(talent, &request.hero_id, request.scenario.target_count)?;
        if talent.id == "ink-talent-id-talent7"
            && ["furyPeriodSeconds", "furyDurationSeconds"]
                .iter()
                .any(|key| {
                    talent
                        .parameters
                        .get(*key)
                        .is_none_or(|value| *value < 0.001)
                })
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "Ride the Lightning requires positive Fury timing parameters",
            ));
        }
    }
    parse_seed(&request.seed)
}

fn validate_apl_target_effect_catalog(
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    let mut effect_ids = BTreeSet::new();
    for effect in &profile.apl_target_effects {
        let mut properties = BTreeSet::new();
        if effect.id.trim().is_empty()
            || effect.name.trim().is_empty()
            || effect.supported_properties.is_empty()
            || !effect_ids.insert(effect.id.as_str())
            || effect
                .supported_properties
                .iter()
                .any(|property| !properties.insert(*property))
        {
            return Err(SimulationError::new(
                SimulationErrorCode::InvalidBuild,
                "APL target effects must be named, unique, and expose unique properties",
            ));
        }
        let source_is_valid = match &effect.source {
            AplTargetEffectSource::AbilityDot { ability_id } => {
                effect.effect_kind == AplTargetEffectKind::DamageOverTime
                    && ability_kind_from_id(ability_id).is_some_and(|kind| {
                        profile
                            .abilities
                            .iter()
                            .any(|ability| ability.kind == kind && ability.dot.is_some())
                    })
            }
            AplTargetEffectSource::MechanicTargetBuff {
                mechanic_instance_id,
            } => {
                effect.effect_kind == AplTargetEffectKind::Debuff
                    && profile.mechanics.iter().any(|mechanic| {
                        mechanic.instance_id == *mechanic_instance_id
                            && mechanic.classification == MechanicClassification::Modeled
                            && mechanic.source_id == "ItemTrait.ID.GemSingleTargetProcOnDamageHeal"
                    })
            }
        };
        if !source_is_valid {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::InvalidBuild,
                message: format!(
                    "{} does not have an evaluable target-effect source",
                    effect.name
                ),
                sources: vec![effect.id.clone()],
            });
        }
    }
    Ok(())
}

fn invalid_apl(message: impl Into<String>, sources: Vec<String>) -> SimulationError {
    SimulationError {
        diagnostics: vec![],
        diagnostics_truncated: false,
        code: SimulationErrorCode::InvalidActionPriorityList,
        message: message.into(),
        sources,
    }
}

pub(crate) fn validate_action_priority_list(
    apl: &ActionPriorityListV2,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    if apl.schema_version != ACTION_PRIORITY_LIST_SCHEMA_VERSION {
        return Err(invalid_apl(
            "The action priority list uses an unsupported schema.",
            vec![apl.schema_version.to_string()],
        ));
    }
    if apl.rules.is_empty() {
        return Err(invalid_apl(
            "Add at least one cast action before running the simulation.",
            Vec::new(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for (index, rule) in apl.rules.iter().enumerate() {
        let checked = (|| {
            validate_apl_id(&rule.id, &mut ids)?;
            if !rule.enabled {
                if let Some(condition) = &rule.condition {
                    validate_apl_node_ids(condition, &mut ids, 0)?;
                }
                return Ok(());
            }
            validate_apl_ability(profile, &rule.ability_id, false)?;
            if let Some(condition) = &rule.condition {
                validate_apl_expression(condition, profile, &mut ids, 0)?;
            }
            Ok::<(), SimulationError>(())
        })();
        if let Err(error) = checked {
            let truncated = error.diagnostics_truncated;
            let mut found = apl_diagnostics(error);
            for d in &mut found {
                d.path = Some(format!("/rules/{index}"));
                d.identifiers.push(rule.id.clone());
            }
            diagnostics.extend(found);
            if truncated && diagnostics.len() <= MAX_DIAGNOSTICS {
                diagnostics.push(Diagnostic::error(
                    "diagnostics-truncated",
                    "Further semantic issues were omitted.",
                ));
            }
            if diagnostics.len() > MAX_DIAGNOSTICS {
                break;
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(SimulationError::from_diagnostics(
            SimulationErrorCode::InvalidActionPriorityList,
            diagnostics,
        ))
    }
}

fn apl_diagnostics(error: SimulationError) -> Vec<Diagnostic> {
    if !error.diagnostics.is_empty() {
        return error.diagnostics;
    }
    let mut diagnostic = Diagnostic::error("invalid-action-priority-list", error.message)
        .help("Inspect catalog apl-references for the same hero or build.");
    diagnostic.identifiers = error.sources;
    vec![diagnostic]
}
fn independent_apl_checks(
    checks: impl Iterator<Item = Result<(), SimulationError>>,
) -> Result<(), SimulationError> {
    let mut diagnostics = Vec::new();
    for check in checks {
        if let Err(error) = check {
            let truncated = error.diagnostics_truncated;
            diagnostics.extend(apl_diagnostics(error));
            if truncated && diagnostics.len() <= MAX_DIAGNOSTICS {
                diagnostics.push(Diagnostic::error(
                    "diagnostics-truncated",
                    "Further semantic issues were omitted.",
                ));
            }
            if diagnostics.len() > MAX_DIAGNOSTICS {
                break;
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(SimulationError::from_diagnostics(
            SimulationErrorCode::InvalidActionPriorityList,
            diagnostics,
        ))
    }
}

fn validate_apl_id(id: &str, ids: &mut BTreeSet<String>) -> Result<(), SimulationError> {
    if id.trim().is_empty() || !ids.insert(id.to_string()) {
        return Err(invalid_apl(
            "Every APL rule and condition must have a stable, unique identifier.",
            vec![id.to_string()],
        ));
    }
    Ok(())
}

fn validate_apl_node_ids(
    node: &AplExpressionNode,
    ids: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(), SimulationError> {
    if depth > 64 {
        return Err(invalid_apl(
            "The APL condition is nested too deeply.",
            Vec::new(),
        ));
    }
    validate_apl_id(&node.id, ids)?;
    match &node.expression {
        AplExpression::All { children } | AplExpression::Any { children } => {
            if children.is_empty() {
                return Err(invalid_apl(
                    "ALL OF and ANY OF groups must contain at least one condition.",
                    vec![node.id.clone()],
                ));
            }
            for child in children {
                validate_apl_node_ids(child, ids, depth + 1)?;
            }
        }
        AplExpression::Not { child } => validate_apl_node_ids(child, ids, depth + 1)?,
        AplExpression::BooleanReference { .. } | AplExpression::Comparison { .. } => {}
    }
    Ok(())
}

fn validate_apl_ability(
    profile: &NormalizedDpsProfile,
    ability_id: &str,
    require_dot: bool,
) -> Result<DpsAbilityKind, SimulationError> {
    let Some(kind) = ability_kind_from_id(ability_id) else {
        return Err(invalid_apl(
            format!("The APL references an unknown ability: {ability_id}."),
            vec![ability_id.to_string()],
        ));
    };
    let Some(ability) = profile
        .abilities
        .iter()
        .find(|ability| ability.kind == kind)
    else {
        if hero_allows_loadout_dependent_ability(&profile.hero_id, kind)
            && (!require_dot || kind == DpsAbilityKind::WeaponFrostVolley)
        {
            return Ok(kind);
        }
        return Err(invalid_apl(
            format!("The current loadout does not provide {ability_id}."),
            vec![ability_id.to_string()],
        ));
    };
    if !ability.manually_castable {
        return Err(invalid_apl(
            format!("{ability_id} is a triggered effect, not a cast action."),
            vec![ability_id.to_string()],
        ));
    }
    if require_dot && ability.dot.is_none() {
        return Err(invalid_apl(
            format!("{ability_id} does not create a queryable damage-over-time effect."),
            vec![ability_id.to_string()],
        ));
    }
    Ok(kind)
}

fn hero_allows_loadout_dependent_ability(hero_id: &str, kind: DpsAbilityKind) -> bool {
    match HeroIdentity::from_id(hero_id) {
        Some(HeroIdentity::Ardeos | HeroIdentity::Rime | HeroIdentity::Elarion) => matches!(
            kind,
            DpsAbilityKind::WeaponFrostVolley
                | DpsAbilityKind::WeaponArcaneChannel
                | DpsAbilityKind::WeaponChainLightning
                | DpsAbilityKind::WeaponShadowMark
        ),
        Some(HeroIdentity::Tariq | HeroIdentity::Mara | HeroIdentity::Gunde) => matches!(
            kind,
            DpsAbilityKind::WeaponShadowMark
                | DpsAbilityKind::WeaponCleaveCharge
                | DpsAbilityKind::WeaponFrontalCone
                | DpsAbilityKind::WeaponInstantAoe
        ),
        None => false,
    }
}

pub(crate) fn validate_apl_expression(
    node: &AplExpressionNode,
    profile: &NormalizedDpsProfile,
    ids: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(), SimulationError> {
    if depth > 64 {
        return Err(invalid_apl(
            "The APL condition is nested too deeply.",
            Vec::new(),
        ));
    }
    validate_apl_id(&node.id, ids)?;
    match &node.expression {
        AplExpression::All { children } | AplExpression::Any { children } => {
            if children.is_empty() {
                return Err(invalid_apl(
                    "ALL OF and ANY OF groups must contain at least one condition.",
                    vec![node.id.clone()],
                ));
            }
            independent_apl_checks(
                children
                    .iter()
                    .map(|child| validate_apl_expression(child, profile, ids, depth + 1)),
            )
        }
        AplExpression::Not { child } => validate_apl_expression(child, profile, ids, depth + 1),
        AplExpression::BooleanReference { reference } => match reference {
            AplBooleanReference::CooldownReady { ability_id } => {
                validate_apl_ability(profile, ability_id, false).map(drop)
            }
            AplBooleanReference::DotActive { ability_id } => {
                validate_apl_ability(profile, ability_id, true).map(drop)
            }
            AplBooleanReference::BuffActive { buff } => validate_apl_buff(*buff, profile),
            AplBooleanReference::TargetEffectActive { effect_id } => {
                validate_target_effect(effect_id, AplTargetEffectProperty::Active, profile)
            }
            AplBooleanReference::LegendaryEquipped { item_id } => {
                let known = match profile.hero_id.as_str() {
                    ARDEOS_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-criticalstrike"
                            | "legendary-wrists-a-expertise"
                            | "legendary-ring-c-criticalstrike-haste"
                    ),
                    RIME_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-haste"
                            | "legendary-wrists-a-haste"
                            | "legendary-ring-c-criticakstrike-expertise"
                    ),
                    ELARION_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-haste"
                            | "legendary-wrists-a-criticalstrike"
                            | "legendary-ring-c-spirit-spirit"
                    ),
                    TARIQ_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-haste"
                            | "legendary-wrists-a-expertise"
                            | "legendary-ring-c-criticalstrike-haste"
                    ),
                    GUNDE_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-haste"
                            | "legendary-wrists-a-criticalstrike"
                            | "legendary-ring-c-criticakstrike-expertise"
                    ),
                    MARA_HERO_ID => matches!(
                        item_id.as_str(),
                        "legendary-back-a-haste"
                            | "legendary-wrists-a-expertise"
                            | "legendary-ring-c-spirit-spirit"
                    ),
                    _ => false,
                };
                if known {
                    Ok(())
                } else {
                    Err(invalid_apl(
                        format!("The APL references an unknown hero legendary: {item_id}."),
                        vec![item_id.clone()],
                    ))
                }
            }
            AplBooleanReference::TalentSelected { talent_id } => {
                let prefix = match profile.hero_id.as_str() {
                    RIME_HERO_ID => "rime-talent-id-talent",
                    TARIQ_HERO_ID => "ink-talent-id-talent",
                    ELARION_HERO_ID => "bowguy-talent-id-talent",
                    MARA_HERO_ID => "mara-talent-id-talent",
                    GUNDE_HERO_ID => "gunde-talent-id-talent",
                    _ => "firemage-talent-id-talent",
                };
                if talent_id.starts_with(prefix) && is_known_dps_talent_id(talent_id) {
                    Ok(())
                } else {
                    Err(invalid_apl(
                        format!("The APL references an unknown hero talent: {talent_id}."),
                        vec![talent_id.clone()],
                    ))
                }
            }
        },
        AplExpression::Comparison { left, right, .. } => independent_apl_checks(
            [left, right]
                .into_iter()
                .map(|operand| validate_apl_operand(operand, profile)),
        ),
    }
}

fn validate_apl_operand(
    operand: &AplNumericOperand,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    match operand {
        AplNumericOperand::Number { value } if !value.is_finite() => Err(invalid_apl(
            "APL numeric literals must be finite.",
            vec![value.to_string()],
        )),
        AplNumericOperand::Reference { reference } => validate_apl_reference(reference, profile),
        AplNumericOperand::Number { .. } => Ok(()),
    }
}

fn validate_apl_reference(
    reference: &AplNumericReference,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    match reference {
        AplNumericReference::CooldownRemaining { ability_id }
        | AplNumericReference::CooldownCharges { ability_id } => {
            validate_apl_ability(profile, ability_id, false).map(drop)
        }
        AplNumericReference::DotRemaining { ability_id } => {
            validate_apl_ability(profile, ability_id, true).map(drop)
        }
        AplNumericReference::Resource { resource, .. } => validate_apl_resource(*resource, profile),
        AplNumericReference::BuffRemaining { buff } | AplNumericReference::BuffStacks { buff } => {
            validate_apl_buff(*buff, profile)
        }
        AplNumericReference::TargetEffectRemaining { effect_id } => {
            validate_target_effect(effect_id, AplTargetEffectProperty::Remaining, profile)
        }
        AplNumericReference::TargetEffectStacks { effect_id } => {
            validate_target_effect(effect_id, AplTargetEffectProperty::Stacks, profile)
        }
        _ => Ok(()),
    }
}

fn validate_target_effect(
    effect_id: &str,
    property: AplTargetEffectProperty,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    let Some(effect) = profile
        .apl_target_effects
        .iter()
        .find(|effect| effect.id == effect_id)
    else {
        return Err(invalid_apl(
            format!("The current loadout does not provide target effect {effect_id}."),
            vec![effect_id.to_string()],
        ));
    };
    if effect.supported_properties.contains(&property) {
        Ok(())
    } else {
        Err(invalid_apl(
            format!(
                "{} does not expose the requested APL property.",
                effect.name
            ),
            vec![effect_id.to_string()],
        ))
    }
}

pub(crate) fn validate_apl_resource(
    resource: AplResource,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    let valid = resource_supported(resource, &profile.hero_id);
    if valid {
        Ok(())
    } else {
        Err(invalid_apl(
            "The APL references a resource that does not belong to this hero.",
            vec![format!("{resource:?}")],
        ))
    }
}
pub(crate) fn resource_supported(resource: AplResource, hero: &str) -> bool {
    match hero {
        "rime" => matches!(
            resource,
            AplResource::Anima | AplResource::WinterOrbs | AplResource::Spirit
        ),
        TARIQ_HERO_ID => matches!(resource, AplResource::Fury | AplResource::Spirit),
        ELARION_HERO_ID => matches!(resource, AplResource::Focus | AplResource::Spirit),
        MARA_HERO_ID => matches!(
            resource,
            AplResource::Energy | AplResource::ComboPoints | AplResource::Spirit
        ),
        GUNDE_HERO_ID => matches!(resource, AplResource::BloodFeathers | AplResource::Spirit),
        _ => matches!(
            resource,
            AplResource::Cinders | AplResource::Embers | AplResource::Spirit
        ),
    }
}

pub(crate) fn validate_apl_buff(
    buff: AplBuff,
    profile: &NormalizedDpsProfile,
) -> Result<(), SimulationError> {
    let valid = match profile.hero_id.as_str() {
        "rime" => matches!(
            buff,
            AplBuff::SpiritOfHeroism
                | AplBuff::IceBlitz
                | AplBuff::WintersBlessing
                | AplBuff::WrathOfWinter
                | AplBuff::FlightOfTheNavir
                | AplBuff::GlacialAssault
                | AplBuff::IcyFlow
                | AplBuff::SoulfrostTorrent
                | AplBuff::HarrowingIce
                | AplBuff::FrostweaversWrath
        ),
        TARIQ_HERO_ID => matches!(
            buff,
            AplBuff::SpiritOfHeroism
                | AplBuff::ThunderCall
                | AplBuff::FocusedWrath
                | AplBuff::RagingTempest
                | AplBuff::FarBeyondDriven
                | AplBuff::KillEmAll
                | AplBuff::SquareHammer
                | AplBuff::SquareHammerExpertise
                | AplBuff::SchismHammerStorm
                | AplBuff::SchismSkullCrusher
                | AplBuff::ExecutionersGrin
        ),
        ELARION_HERO_ID => matches!(
            buff,
            AplBuff::SpiritOfHeroism
                | AplBuff::CelestialImpetus
                | AplBuff::EmpoweredMultishot
                | AplBuff::SkystridersGrace
                | AplBuff::EventHorizon
                | AplBuff::SkystridersSupremacy
                | AplBuff::ImpendingHeartseeker
                | AplBuff::ResurgentWinds
        ),
        MARA_HERO_ID => matches!(
            buff,
            AplBuff::SpiritOfHeroism
                | AplBuff::BroodingShadows
                | AplBuff::MaidenOfDeath
                | AplBuff::MatriarchMacabre
                | AplBuff::AssassinsGuile
                | AplBuff::DeadlyScheme
                | AplBuff::FeedTheQueen
                | AplBuff::MalevolenceArachnid
                | AplBuff::MalevolenceQueen
                | AplBuff::DrenchedInBlood
        ),
        GUNDE_HERO_ID => matches!(
            buff,
            AplBuff::SpiritOfHeroism
                | AplBuff::SerratedEdge
                | AplBuff::ReignInBlood
                | AplBuff::BloodboundSpirit
                | AplBuff::DeathsArc
                | AplBuff::GrimHarvest
                | AplBuff::HarvestersToll
                | AplBuff::CrimsonStrikes
                | AplBuff::MurderOfCrows
                | AplBuff::Massacre
                | AplBuff::AncestralInstinct
                | AplBuff::Bloodbath
                | AplBuff::CarrionOnslaught
                | AplBuff::OpenWounds
        ),
        _ => matches!(
            buff,
            AplBuff::Wildfire
                | AplBuff::SpiritOfHeroism
                | AplBuff::ApocalypticSurge
                | AplBuff::CascadingInferno
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_apl(
            "The APL references a player effect that does not belong to this hero.",
            vec![format!("{buff:?}")],
        ))
    }
}

fn is_known_dps_talent_id(id: &str) -> bool {
    if let Some(number) = id
        .strip_prefix("mara-talent-id-talent")
        .and_then(|value| value.parse::<u8>().ok())
    {
        return (1..=19).contains(&number) && number != 7;
    }
    if let Some(number) = id
        .strip_prefix("gunde-talent-id-talent")
        .and_then(|value| value.parse::<u8>().ok())
    {
        return (1..=18).contains(&number);
    }
    id.strip_prefix("firemage-talent-id-talent")
        .or_else(|| id.strip_prefix("rime-talent-id-talent"))
        .or_else(|| id.strip_prefix("ink-talent-id-talent"))
        .or_else(|| id.strip_prefix("bowguy-talent-id-talent"))
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|number| (1..=18).contains(&number))
}

fn hero_allows_ability(hero_id: &str, kind: DpsAbilityKind) -> bool {
    let owner = ability_metadata(kind).owner;
    matches!(owner, AbilityOwner::Shared)
        || matches!(
            (HeroIdentity::from_id(hero_id), owner),
            (Some(HeroIdentity::Ardeos), AbilityOwner::Ardeos)
                | (Some(HeroIdentity::Rime), AbilityOwner::Rime)
                | (Some(HeroIdentity::Tariq), AbilityOwner::Tariq)
                | (Some(HeroIdentity::Elarion), AbilityOwner::Elarion)
                | (Some(HeroIdentity::Mara), AbilityOwner::Mara)
                | (Some(HeroIdentity::Gunde), AbilityOwner::Gunde)
        )
}

fn validate_rime_ability_values(ability: &DpsAbilityModel) -> Result<(), SimulationError> {
    let requires_duration = matches!(
        ability.kind,
        DpsAbilityKind::IceBlitz
            | DpsAbilityKind::BurstingIce
            | DpsAbilityKind::WintersBlessing
            | DpsAbilityKind::WrathOfWinter
            | DpsAbilityKind::FlightOfTheNavir
    );
    let requires_server_tick_rate = matches!(
        ability.kind,
        DpsAbilityKind::FrostBolt
            | DpsAbilityKind::GlacialBlast
            | DpsAbilityKind::FlightOfTheNavir
            | DpsAbilityKind::AnimaSpike
    );
    let requires_projectile_count = matches!(
        ability.kind,
        DpsAbilityKind::WrathOfWinter
            | DpsAbilityKind::FlightOfTheNavir
            | DpsAbilityKind::AnimaSpike
    );
    let invalid = (requires_duration && ability.effect_duration_ms == 0)
        || (requires_server_tick_rate
            && ability_param(ability, parameter_key!("serverTickRateCapHz")) <= 0.0)
        || (requires_projectile_count
            && ability_u32_rounded(
                ability,
                if ability.kind == DpsAbilityKind::WrathOfWinter {
                    parameter_key!("volleyProjectiles")
                } else {
                    parameter_key!("projectileCount")
                },
            ) == 0);
    if invalid {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::InvalidBuild,
            message: format!("{} has invalid executable parameters", ability.name),
            sources: vec![ability.id.clone()],
        });
    }
    Ok(())
}

fn validate_modeled_mechanic(mechanic: &DynamicMechanicInstance) -> Result<(), SimulationError> {
    if mechanic.handler == DynamicMechanicHandler::HeroSource
        && !is_known_hero_source_mechanic(mechanic)
    {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!(
                "{} has no current executable hero-source handler",
                mechanic.source_name
            ),
            sources: vec![mechanic.source_id.clone()],
        });
    }

    let is_undulating_spirit = mechanic.parameters.contains_key("wintersBlessingCharges");
    let required_positive_seconds: &[&str] = if is_finesse_source(&mechanic.source_id, "12") {
        &["durationSeconds", "intervalSeconds"]
    } else if is_undulating_spirit {
        &["undulatingSpiritDurationSeconds"]
    } else {
        match mechanic.source_id.as_str() {
            "gem-ruby-220" | "gem-ruby-1000" => &["periodSeconds"],
            "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc" | "ItemTrait.ID.GemDotHotOnCrit" => {
                &["durationSeconds", "periodSeconds"]
            }
            "ItemTrait.ID.GemPulsatingOnAbilityTotemProc" => {
                &["initialDamagePulseDelaySeconds", "pulseIntervalSeconds"]
            }
            _ => &[],
        }
    };
    let missing = required_positive_seconds
        .iter()
        .filter(|key| !mechanic.parameters.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!(
                "{} is missing extracted parameters: {}",
                mechanic.source_name,
                missing.join(", ")
            ),
            sources: vec![mechanic.source_id.clone()],
        });
    }
    if required_positive_seconds
        .iter()
        .any(|key| seconds_parameter(mechanic, ParameterKey::new(key)) == 0)
        || (matches!(
            mechanic.source_id.as_str(),
            "gem-ruby-220" | "gem-ruby-1000"
        ) && mechanic_param(mechanic, parameter_key!("healingHealthFraction")) <= 0.0)
        || (is_undulating_spirit
            && mechanic_param(mechanic, parameter_key!("wintersBlessingCharges"))
                .round()
                .max(0.0) as u32
                == 0)
    {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::InvalidBuild,
            message: format!(
                "{} must use positive extracted effect timings and stack counts",
                mechanic.source_name
            ),
            sources: vec![mechanic.source_id.clone()],
        });
    }
    Ok(())
}

fn is_known_hero_source_mechanic(mechanic: &DynamicMechanicInstance) -> bool {
    let source_id = mechanic.source_id.as_str();
    if source_id.starts_with("weapon-")
        || source_id.starts_with("gem-")
        || source_id.starts_with("legendary-")
        || matches!(
            source_id,
            "seta-proc-intellect" | "setb-proc-hdt" | "setd-proc-intellect"
        )
    {
        return true;
    }
    if let Some(suffix) = source_id.strip_prefix("DynamicItemAbilityRank.") {
        return suffix
            .parse::<u8>()
            .is_ok_and(|number| (1..=14).contains(&number) && number != 3);
    }
    matches!(
        source_id,
        "ItemTrait.ID.AbilityToIncreasedMainStat"
            | "ItemTrait.ID.CommitToHasteRatingAndWeaponCooldown"
            | "ItemTrait.ID.CooldownRecoveryOnWeaponAbility"
            | "ItemTrait.ID.CritsToIncreasedCritRating"
            | "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff"
            | "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"
            | "ItemTrait.ID.GemCooldownRecoveryOnAbilityProc"
            | "ItemTrait.ID.GemDotHotOnCrit"
            | "ItemTrait.ID.GemPulsatingOnAbilityTotemProc"
            | "ItemTrait.ID.GemSingleTargetProcOnDamageHeal"
            | "ItemTrait.ID.GemTargetedSpikeProc"
            | "ItemTrait.ID.GemWhirlwindProc"
            | "ItemTrait.ID.IncreasedMainStatAndSpiritRating"
            | "ItemTrait.ID.OffensiveAbilityHasteRatingStacking"
            | "ItemTrait.ID.OffensiveAbilityToHighestStatBuff"
            | "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease"
            | "ItemTrait.ID.WeaponAndSpiritPoints"
            | "ItemTrait.ID.WeaponCritChanceCooldownReduction"
            | "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease"
            | "ItemTrait.ID.WeaponHealDamageIncrease"
    )
}

pub(crate) fn validate_talent(
    talent: &DpsTalentModel,
    hero_id: &str,
    _target_count: u32,
) -> Result<(), SimulationError> {
    let expected_prefix = match hero_id {
        ARDEOS_HERO_ID => "firemage-talent-id-talent",
        RIME_HERO_ID => "rime-talent-id-talent",
        TARIQ_HERO_ID => "ink-talent-id-talent",
        ELARION_HERO_ID => "bowguy-talent-id-talent",
        MARA_HERO_ID => "mara-talent-id-talent",
        GUNDE_HERO_ID => "gunde-talent-id-talent",
        _ => "",
    };
    if expected_prefix.is_empty() || !talent.id.starts_with(expected_prefix) {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!(
                "Talent {} does not belong to the current {hero_id} contract",
                talent.id
            ),
            sources: vec![talent.id.clone()],
        });
    }
    let required_parameters = match (talent.id.as_str(), talent.mechanic_id.as_str()) {
        (
            "rime-talent-id-talent1",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-glacialassault",
        ) => &[
            "maximumStacks",
            "damageMultiplier",
            "explosionDamageFraction",
            "explosionRadius",
            "targetCountDamageScalingThreshold",
        ][..],
        (
            "rime-talent-id-talent2",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent17",
        ) => &["burstingDamageMultiplier", "procChance"][..],
        (
            "rime-talent-id-talent3",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-chillblain",
        ) => &[
            "durationSeconds",
            "powerCoefficientPerStack",
            "targetCountDamageScalingThreshold",
            "criticalExtraStackChance",
            "criticalExtraStacks",
            "maximumStacks",
        ][..],
        (
            "rime-talent-id-talent4",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-unrelentingice",
        ) => &[
            "burstingCooldownReductionSeconds",
            "torrentCooldownReductionSeconds",
        ][..],
        (
            "rime-talent-id-talent5",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-icyflow",
        ) => &[
            "durationSeconds",
            "maximumStacks",
            "castHaste",
            "cometInitialDelaySeconds",
            "criticalStrikeBonus",
        ][..],
        (
            "rime-talent-id-talent6",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-avalanche",
        ) => &["oneExtraChance", "twoExtraChance"][..],
        (
            "rime-talent-id-talent7",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-talent",
        ) => &["procChance"][..],
        (
            "rime-talent-id-talent8",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-wisdomofthenorth",
        ) => &["cooldownReductionPerOrbSeconds"][..],
        (
            "rime-talent-id-talent9",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent19",
        ) => &[
            "procsPerMinute",
            "durationSeconds",
            "tickRateMultiplier",
            "criticalStrikeBonus",
        ][..],
        (
            "rime-talent-id-talent10",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent12",
        ) => &["freeColdSnapCharges", "durationSeconds"][..],
        (
            "rime-talent-id-talent11",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent22",
        ) => &[
            "durationSeconds",
            "maximumStacks",
            "damageIncreasePerStack",
            "flightCooldownReductionSeconds",
        ][..],
        (
            "rime-talent-id-talent12",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent20",
        ) => &["damageMultiplier", "durationIncreaseSeconds"][..],
        (
            "rime-talent-id-talent13",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent16",
        ) => &["spikesPerAnima"][..],
        (
            "rime-talent-id-talent14",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent13",
        ) => &["damageMultiplier", "castTimeIncreaseSeconds"][..],
        (
            "rime-talent-id-talent15",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent21",
        ) => &["procChance"][..],
        (
            "rime-talent-id-talent16",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent7",
        ) => &["criticalPowerMultiplier"][..],
        (
            "rime-talent-id-talent17",
            "augmentation:fellowship-content-abilities-talents-rime-caa-rime-trait1",
        ) => &[
            "singleTargetDamageMultiplier",
            "multiTargetDamageMultiplier",
            "singleTargetPulsePeriodSeconds",
            "multiTargetPulsePeriodSeconds",
            "singleTargetInitialDelaySeconds",
            "multiTargetInitialDelaySeconds",
        ][..],
        (
            "rime-talent-id-talent18",
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-chanceonorbgain-nextspendercritincrease",
        ) => &[
            "procChance",
            "durationSeconds",
            "criticalStrikeBonus",
            "maximumStacks",
        ][..],
        (
            "firemage-talent-id-talent1",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-pyrophibianfrenzy",
        ) => &["numberOfFrogs", "procChance", "criticalProcChance"][..],
        (
            "firemage-talent-id-talent2",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-reignoffire",
        ) => &[
            "procsPerMinute",
            "criticalStrikeBonus",
            "durationSeconds",
            "maximumStacks",
        ],
        (
            "firemage-talent-id-talent3",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-agonizingblaze",
        ) => &["damagePerStack", "maximumStacks"],
        (
            "firemage-talent-id-talent4",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-intensifyinginferno",
        ) => &["damagePerUniqueDot", "maximumDamageIncrease"],
        (
            "firemage-talent-id-talent5",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait3",
        ) => &[
            "stacksThreshold",
            "criticalStrikeBonus",
            "cindersMultiplier",
            "maximumStacks",
        ],
        (
            "firemage-talent-id-talent6",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-frogsquad",
        ) => &[
            "additionalFrogs",
            "additionalLeaps",
            "baseFrogs",
            "baseLeaps",
            "damageIncrease",
        ],
        (
            "firemage-talent-id-talent7",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-ouroboros",
        ) => &["cooldownReductionSeconds"],
        (
            "firemage-talent-id-talent8",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-rollingflames-new",
        ) => &[
            "searingBlazeReductionSeconds",
            "infernalWaveReductionSeconds",
        ],
        (
            "firemage-talent-id-talent9",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-warmth",
        ) => &[
            "baseProcChance",
            "criticalStrikeBonus",
            "criticalChanceStep",
            "procChancePerStep",
        ],
        (
            "firemage-talent-id-talent10",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent10",
        ) => &["extensionSeconds"],
        (
            "firemage-talent-id-talent11",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent2",
        ) => &["dotCriticalStrikeBonus"],
        (
            "firemage-talent-id-talent12",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent3",
        ) => &["durationIncreaseSeconds"],
        (
            "firemage-talent-id-talent13",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent11",
        ) => &[
            "criticalStrikeBonus",
            "burnDamageFraction",
            "burnDurationSeconds",
            "burnTickPeriodSeconds",
        ],
        (
            "firemage-talent-id-talent14",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait1",
        ) => &[
            "castTimeReductionSeconds",
            "surgeStacks",
            "maximumSurgeStacks",
            "surgeDurationSeconds",
        ],
        (
            "firemage-talent-id-talent15",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait2",
        ) => &["startingSpirit", "startingEmbers"],
        (
            "firemage-talent-id-talent16",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent7",
        ) => &["directDamageIncrease", "additionalDotFraction"],
        (
            "firemage-talent-id-talent17",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-burstingflames",
        ) => &[
            "damageFraction",
            "maximumRadius",
            "targetCountDamageScalingThreshold",
            "visualDelaySeconds",
        ],
        (
            "firemage-talent-id-talent18",
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent9",
        ) => &["extensionPerTickSeconds"],
        (
            "ink-talent-id-talent1",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait8",
        ) => &["uniqueTargetDamageIncrease"],
        (
            "ink-talent-id-talent2",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent2",
        ) => &["criticalStrikeBonus"],
        (
            "ink-talent-id-talent3",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent3",
        ) => &["procChance", "activationDelaySeconds"],
        (
            "ink-talent-id-talent4",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent4",
        ) => &["cleaveDamageMultiplier"],
        (
            "ink-talent-id-talent5",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent5",
        ) => &["focusedWrathStacks"],
        (
            "ink-talent-id-talent6",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent6",
        ) => &["furyPerSecond"],
        (
            "ink-talent-id-talent7",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent7",
        ) => &[
            "furyPerTick",
            "furyPeriodSeconds",
            "furyDurationSeconds",
            "hasteBonus",
        ],
        (
            "ink-talent-id-talent8",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent8",
        ) => &["spiritPerStack", "maximumStacks", "durationSeconds"],
        (
            "ink-talent-id-talent9",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent9",
        ) => &[
            "damageMultiplier",
            "procChance",
            "maximumStacks",
            "durationSeconds",
        ],
        (
            "ink-talent-id-talent10",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent1",
        ) => &["procChance", "criticalStrikeBonus"],
        (
            "ink-talent-id-talent11",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait9",
        ) => &[
            "procChance",
            "charges",
            "durationSeconds",
            "damageMultiplier",
        ],
        (
            "ink-talent-id-talent12",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent6",
        ) => &["criticalStrikeBonus"],
        (
            "ink-talent-id-talent13",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent4",
        ) => &["startingSpirit", "spiritPerCast"],
        (
            "ink-talent-id-talent14",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent10",
        ) => &[],
        (
            "ink-talent-id-talent15",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait5",
        ) => &["procsPerMinute", "maximumCharges", "activationDelaySeconds"],
        (
            "ink-talent-id-talent16",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent7",
        ) => &["lightningCriticalStrikeBonus"],
        (
            "ink-talent-id-talent17",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent8",
        ) => &["additionalChainHits"],
        (
            "ink-talent-id-talent18",
            "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait7",
        ) => &[
            "maximumStacks",
            "durationSeconds",
            "cooldownReductionPerStackSeconds",
            "expertiseBonus",
            "expertiseDurationSeconds",
        ],
        (
            "bowguy-talent-id-talent1",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent1",
        ) => &[
            "cooldownReductionSeconds",
            "empoweredCooldownReductionSeconds",
        ],
        (
            "bowguy-talent-id-talent2",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-trait3",
        ) => &["additionalTargets", "damageMultiplier"],
        (
            "bowguy-talent-id-talent3",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent3",
        ) => &[
            "durationSeconds",
            "offensiveProcChance",
            "stacks",
            "maximumStacks",
        ],
        (
            "bowguy-talent-id-talent4",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent8",
        ) => &["cooldownAcceleration"],
        (
            "bowguy-talent-id-talent5",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent2",
        ) => &["durationIncreaseSeconds"],
        (
            "bowguy-talent-id-talent6",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent9",
        ) => &["procChance", "criticalStrikeBonus"],
        (
            "bowguy-talent-id-talent7",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent10",
        ) => &[
            "procChance",
            "damageMultiplier",
            "stacks",
            "maximumStacks",
            "durationSeconds",
        ],
        (
            "bowguy-talent-id-talent8",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent13",
        ) => &["cooldownReductionSeconds"],
        (
            "bowguy-talent-id-talent9",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent5",
        ) => &["cooldownReductionSeconds"],
        (
            "bowguy-talent-id-talent10",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent7",
        ) => &["criticalStrikeBonus"],
        (
            "bowguy-talent-id-talent11",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent14",
        ) => &["expertisePerStack", "maximumStacks", "decayIntervalSeconds"],
        (
            "bowguy-talent-id-talent12",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent10",
        ) => &["cooldownReductionSeconds"],
        (
            "bowguy-talent-id-talent13",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent7",
        ) => &[
            "cooldownReductionSeconds",
            "damageMultiplier",
            "durationSeconds",
            "maximumStacks",
        ],
        (
            "bowguy-talent-id-talent14",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent11",
        ) => &["damageIncreasePerProjectile", "durationSeconds"],
        (
            "bowguy-talent-id-talent15",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent12",
        ) => &["damageMultiplier", "maximumBounces"],
        (
            "bowguy-talent-id-talent16",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent8",
        ) => &[],
        (
            "bowguy-talent-id-talent17",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent15",
        ) => &["damageMultiplier", "resourceMultiplier"],
        (
            "bowguy-talent-id-talent18",
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent4",
        ) => &["procChanceMultiplier", "damageMultiplier"],
        (
            "mara-talent-id-talent1",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent1",
        ) => &[
            "initialCriticalStrikeBonus",
            "additionalTargetCriticalStrikeBonus",
            "maximumCriticalStrikeBonus",
        ],
        (
            "mara-talent-id-talent2",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent15",
        ) => &["damageMultiplier", "durationSeconds", "maximumStacks"],
        (
            "mara-talent-id-talent3",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent3",
        ) => &[
            "energyPerStack",
            "maximumStacks",
            "durationSeconds",
            "criticalStrikeBonus",
        ],
        (
            "mara-talent-id-talent4",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-trait3",
        ) => &["procChancePerComboPoint"],
        (
            "mara-talent-id-talent5",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-passivetalent2",
        ) => &["procChance", "energyGain"],
        (
            "mara-talent-id-talent6",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-passivetalent3",
        ) => &["energyPerComboPoint"],
        ("mara-talent-id-talent8", "talent:mara:macabre-stratagem") => {
            &["durationSeconds", "additionalDurationSeconds"]
        }
        (
            "mara-talent-id-talent9",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent14",
        ) => &[],
        (
            "mara-talent-id-talent10",
            "augmentation:fellowship-content-abilities-talents-mara-caa-mara-trait1",
        ) => &["procChance"],
        (
            "mara-talent-id-talent11",
            "augmentation:fellowship-content-abilities-talents-mara-caa-mara-trait2",
        ) => &["procChance"],
        (
            "mara-talent-id-talent12",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent16",
        ) => &["cooldownReductionPerComboPointSeconds"],
        (
            "mara-talent-id-talent13",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-trait1",
        ) => &["damageMultiplier", "durationSeconds"],
        (
            "mara-talent-id-talent14",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent13",
        ) => &["procChance"],
        (
            "mara-talent-id-talent15",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent8",
        ) => &["damageIncreasePerStack", "durationSeconds", "maximumStacks"],
        (
            "mara-talent-id-talent16",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent9",
        ) => &["seethingBleedDamageMultiplier", "additionalTargets"],
        (
            "mara-talent-id-talent17",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-passivetalent4",
        ) => &["criticalStrikeBonus"],
        (
            "mara-talent-id-talent18",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent11",
        ) => &["damageMultiplier"],
        (
            "mara-talent-id-talent19",
            "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent10",
        ) => &["tickRateMultiplier"],
        (
            "gunde-talent-id-talent1",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait1",
        ) => &["procChance", "criticalStrikeBonus", "durationSeconds"],
        (
            "gunde-talent-id-talent2",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait20",
        ) => &["powerCoefficient", "targetCountDamageScalingThreshold"],
        (
            "gunde-talent-id-talent3",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait3",
        ) => &["procChance", "criticalStrikeBonus", "durationSeconds"],
        (
            "gunde-talent-id-talent4",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait4",
        ) => &["damageMultiplier", "durationSeconds"],
        (
            "gunde-talent-id-talent5",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait5",
        ) => &["damageMultiplier"],
        (
            "gunde-talent-id-talent6",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait18",
        ) => &["directDamageMultiplier", "lowHealthCriticalStrikeBonus"],
        (
            "gunde-talent-id-talent7",
            "augmentation:fellowship-content-abilities-heroes-legendarytraitsforheroesinplugins-caa-gunde-trait27",
        ) => &[],
        (
            "gunde-talent-id-talent8",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait19",
        ) => &["stacks", "durationSeconds"],
        (
            "gunde-talent-id-talent9",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait9",
        ) => &["spiritPerStack", "maximumStacks", "durationSeconds"],
        (
            "gunde-talent-id-talent10",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait23",
        ) => &["cooldownReductionSeconds"],
        (
            "gunde-talent-id-talent11",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait11",
        ) => &["addedRendTransferFraction"],
        (
            "gunde-talent-id-talent12",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait12",
        ) => &[
            "tickRateMultiplier",
            "extraFeathersChance",
            "extraFeathersAmount",
        ],
        (
            "gunde-talent-id-talent13",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait21",
        ) => &[
            "powerCoefficient",
            "damageIncreasePerStack",
            "durationSeconds",
            "periodSeconds",
        ],
        (
            "gunde-talent-id-talent14",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait22",
        ) => &[
            "criticalStrikeBonus",
            "explosionDamageFraction",
            "targetCountDamageScalingThreshold",
        ],
        (
            "gunde-talent-id-talent15",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait15",
        ) => &["damageMultiplier", "cooldownReductionPerSpinSeconds"],
        (
            "gunde-talent-id-talent16",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait10",
        ) => &["addedRendTransferFraction"],
        (
            "gunde-talent-id-talent17",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait17",
        ) => &["procChance", "powerMultiplier", "durationSeconds"],
        (
            "gunde-talent-id-talent18",
            "augmentation:fellowship-plugins-gamefeatures-heroes-gunde-content-abilities-data-traits-caa-gunde-trait24",
        ) => &["cooldownAccelerationMultiplier", "durationSeconds"],
        _ => {
            return Err(SimulationError {
                diagnostics: vec![],
                diagnostics_truncated: false,
                code: SimulationErrorCode::UncoveredMechanics,
                message: format!(
                    "Talent {} has no versioned handler for mechanic {}",
                    talent.id, talent.mechanic_id
                ),
                sources: vec![talent.id.clone(), talent.mechanic_id.clone()],
            });
        }
    };
    let missing = required_parameters
        .iter()
        .filter(|key| {
            !talent
                .parameters
                .get(**key)
                .is_some_and(|value| value.is_finite())
        })
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!(
                "Talent {} is missing extracted parameters: {}",
                talent.id,
                missing.join(", ")
            ),
            sources: vec![talent.id.clone()],
        });
    }
    if talent.id == "bowguy-talent-id-talent11"
        && ms_param(talent, parameter_key!("decayIntervalSeconds")) == 0
    {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::InvalidBuild,
            message: "Striker's Aim must use a positive stack-decay interval".into(),
            sources: vec![talent.id.clone()],
        });
    }
    let valid_classification =
        if talent.id == "mara-talent-id-talent9" || talent.id == "gunde-talent-id-talent7" {
            talent.classification == MechanicClassification::ScenarioNoOp
                && talent
                    .reason
                    .as_deref()
                    .is_some_and(|reason| !reason.is_empty())
        } else {
            talent.classification == MechanicClassification::Modeled
        };
    if !valid_classification {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!("Talent {} requires a current modeled handler", talent.id),
            sources: vec![talent.id.clone()],
        });
    }
    if talent.id.starts_with("rime-talent-id-talent") {
        validate_rime_talent_values(talent)?;
    }
    Ok(())
}

fn validate_rime_talent_values(talent: &DpsTalentModel) -> Result<(), SimulationError> {
    let positive_millisecond_parameters: &[&str] = match talent.id.as_str() {
        "rime-talent-id-talent3"
        | "rime-talent-id-talent5"
        | "rime-talent-id-talent9"
        | "rime-talent-id-talent11"
        | "rime-talent-id-talent18" => &["durationSeconds"],
        "rime-talent-id-talent17" => &[
            "singleTargetPulsePeriodSeconds",
            "multiTargetPulsePeriodSeconds",
        ],
        _ => &[],
    };
    let invalid_timing = positive_millisecond_parameters
        .iter()
        .any(|key| ms_param(talent, ParameterKey::new(key)) == 0)
        || (talent.id == "rime-talent-id-talent9"
            && (param(talent, parameter_key!("procsPerMinute")) <= 0.0
                || param(talent, parameter_key!("tickRateMultiplier")) <= 0.0));
    let probability_parameters: &[&str] = match talent.id.as_str() {
        "rime-talent-id-talent3" => &["criticalExtraStackChance"],
        "rime-talent-id-talent6" => &["oneExtraChance", "twoExtraChance"],
        "rime-talent-id-talent7" | "rime-talent-id-talent15" | "rime-talent-id-talent18" => {
            &["procChance"]
        }
        _ => &[],
    };
    let invalid_probability = probability_parameters
        .iter()
        .any(|key| !(0.0..=1.0).contains(&param(talent, ParameterKey::new(key))));
    let positive_count_parameters: &[&str] = match talent.id.as_str() {
        "rime-talent-id-talent1" | "rime-talent-id-talent5" | "rime-talent-id-talent11" => {
            &["maximumStacks"]
        }
        "rime-talent-id-talent3" => &["criticalExtraStacks", "maximumStacks"],
        "rime-talent-id-talent10" => &["freeColdSnapCharges"],
        "rime-talent-id-talent13" => &["spikesPerAnima"],
        "rime-talent-id-talent18" => &["maximumStacks"],
        _ => &[],
    };
    let invalid_count = positive_count_parameters
        .iter()
        .any(|key| param_u32(talent, ParameterKey::new(key)) == 0);
    if invalid_timing || invalid_probability || invalid_count {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::InvalidBuild,
            message: format!("Talent {} has invalid executable parameters", talent.id),
            sources: vec![talent.id.clone()],
        });
    }
    Ok(())
}
