use crate::*;

pub(crate) fn gunde_applies_rend(kind: DpsAbilityKind) -> bool {
    matches!(
        kind,
        DpsAbilityKind::DoubleStrike
            | DpsAbilityKind::BloodboundSpirit
            | DpsAbilityKind::HeartSplitter
            | DpsAbilityKind::Rupture
            | DpsAbilityKind::BloodArc
            | DpsAbilityKind::ReaversEdge
            | DpsAbilityKind::GrimCarve
    )
}

pub(crate) fn fixed_buff_uptime_id(buff: AplBuff) -> Option<&'static str> {
    Some(match buff {
        AplBuff::Wildfire => "wildfire",
        AplBuff::IceBlitz => "ice-blitz",
        AplBuff::WintersBlessing => "winters-blessing",
        AplBuff::WrathOfWinter => "wrath-of-winter",
        AplBuff::FlightOfTheNavir => "flight-of-the-navir",
        AplBuff::GlacialAssault => "glacial-assault",
        AplBuff::IcyFlow => "icy-flow",
        AplBuff::SoulfrostTorrent => "soulfrost-torrent",
        AplBuff::HarrowingIce => "harrowing-ice",
        AplBuff::FrostweaversWrath => "frostweavers-wrath",
        AplBuff::ThunderCall => "thunder-call",
        AplBuff::FocusedWrath => "focused-wrath",
        AplBuff::RagingTempest => "raging-tempest",
        AplBuff::FarBeyondDriven => "far-beyond-driven",
        AplBuff::KillEmAll => "kill-em-all",
        AplBuff::SquareHammer => "square-hammer",
        AplBuff::SquareHammerExpertise => "square-hammer-expertise",
        AplBuff::CelestialImpetus => "celestial-impetus",
        AplBuff::EmpoweredMultishot => "empowered-multishot",
        AplBuff::SkystridersGrace => "skystriders-grace",
        AplBuff::EventHorizon => "event-horizon",
        AplBuff::SkystridersSupremacy => "skystriders-supremacy",
        AplBuff::ImpendingHeartseeker => "impending-heartseeker",
        AplBuff::ResurgentWinds => "resurgent-winds",
        AplBuff::BroodingShadows => "brooding-shadows",
        AplBuff::MaidenOfDeath => "maiden-of-death",
        AplBuff::MatriarchMacabre => "matriarch-macabre",
        AplBuff::AssassinsGuile => "assassins-guile",
        AplBuff::DeadlyScheme => "deadly-scheme",
        AplBuff::FeedTheQueen => "feed-the-queen",
        AplBuff::MalevolenceArachnid => "malevolence-arachnid",
        AplBuff::MalevolenceQueen => "malevolence-queen",
        AplBuff::DrenchedInBlood => "drenched-in-blood",
        AplBuff::SerratedEdge => "serrated-edge",
        AplBuff::ReignInBlood => "reign-in-blood",
        AplBuff::BloodboundSpirit => "bloodbound-spirit",
        AplBuff::DeathsArc => "deaths-arc",
        AplBuff::GrimHarvest => "grim-harvest",
        AplBuff::HarvestersToll => "harvesters-toll",
        AplBuff::CrimsonStrikes => "crimson-strikes",
        AplBuff::MurderOfCrows => "murder-of-crows",
        AplBuff::Massacre => "massacre",
        AplBuff::AncestralInstinct => "ancestral-instinct",
        AplBuff::Bloodbath => "bloodbath",
        AplBuff::CarrionOnslaught => "carrion-onslaught",
        AplBuff::OpenWounds => "open-wounds",
        AplBuff::SpiritOfHeroism
        | AplBuff::ApocalypticSurge
        | AplBuff::CascadingInferno
        | AplBuff::SchismHammerStorm
        | AplBuff::SchismSkullCrusher
        | AplBuff::ExecutionersGrin => {
            return None;
        }
    })
}
#[cfg(test)]
pub(crate) fn uptime_name(id: &str) -> String {
    id.trim_start_matches("dot:")
        .trim_start_matches("proc:")
        .trim_start_matches("buff:")
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub(crate) trait TalentParameterSource {
    fn parameter_value(&self, key: ParameterKey) -> f64;
}

impl TalentParameterSource for CompiledTalent {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl TalentParameterSource for DpsTalentModel {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters
            .iter()
            .find_map(|(name, value)| (ParameterKey::new(name) == key).then_some(*value))
            .unwrap_or(0.0)
    }
}

pub(crate) trait AbilityParameterSource {
    fn parameter_value(&self, key: ParameterKey) -> f64;
}

impl AbilityParameterSource for CompiledAbility {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl AbilityParameterSource for Arc<CompiledAbility> {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl AbilityParameterSource for PreparedAbility {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl AbilityParameterSource for DpsAbilityModel {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.mechanic_parameters
            .iter()
            .find_map(|(name, value)| (ParameterKey::new(name) == key).then_some(*value))
            .unwrap_or(0.0)
    }
}

pub(crate) trait MechanicParameterSource {
    fn parameter_value(&self, key: ParameterKey) -> f64;
}

impl MechanicParameterSource for CompiledMechanic {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl MechanicParameterSource for Arc<CompiledMechanic> {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters.value(key)
    }
}

impl MechanicParameterSource for DynamicMechanicInstance {
    fn parameter_value(&self, key: ParameterKey) -> f64 {
        self.parameters
            .iter()
            .find_map(|(name, value)| (ParameterKey::new(name) == key).then_some(*value))
            .unwrap_or(0.0)
    }
}

pub(crate) fn param(talent: &impl TalentParameterSource, key: ParameterKey) -> f64 {
    talent.parameter_value(key)
}
pub(crate) fn param_u32(talent: &impl TalentParameterSource, key: ParameterKey) -> u32 {
    param(talent, key).max(0.0) as u32
}
pub(crate) fn ms_param(talent: &impl TalentParameterSource, key: ParameterKey) -> u64 {
    (param(talent, key).max(0.0) * 1_000.0).round() as u64
}
pub(crate) fn ability_param(ability: &impl AbilityParameterSource, key: ParameterKey) -> f64 {
    ability.parameter_value(key)
}
pub(crate) fn ability_u32_rounded(ability: &impl AbilityParameterSource, key: ParameterKey) -> u32 {
    ability_param(ability, key)
        .round()
        .clamp(0.0, u32::MAX as f64) as u32
}
pub(crate) fn ability_seconds_parameter(
    ability: &impl AbilityParameterSource,
    key: ParameterKey,
) -> u64 {
    (ability_param(ability, key).max(0.0) * 1_000.0).round() as u64
}
pub(crate) fn ability_default_cooldown_ms(ability: &CompiledAbility) -> u64 {
    ability
        .parameters
        .get(parameter_key!("defaultCooldownMs"))
        .unwrap_or(ability.cooldown_ms as f64)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
}
pub(crate) fn advance_cooldown_state(
    cooldown: &mut CooldownState,
    elapsed_work_ms: f64,
    base_ms: f64,
    execution: &ExecutionControl<'_>,
) {
    if cooldown.used_charges == 0 {
        return;
    }
    cooldown.remaining_ms -= elapsed_work_ms.max(0.0);
    while cooldown.used_charges > 0 && cooldown.remaining_ms <= 0.0 {
        if !execution.charge(1) {
            return;
        }
        cooldown.used_charges -= 1;
        if cooldown.used_charges > 0 && base_ms > 0.0 {
            cooldown.remaining_ms += base_ms;
        } else {
            cooldown.remaining_ms = 0.0;
        }
    }
}
pub(crate) fn mechanic_param(mechanic: &impl MechanicParameterSource, key: ParameterKey) -> f64 {
    mechanic.parameter_value(key)
}
pub(crate) fn ruby_storm_maximum_forward_overlap_distance(
    mechanic: &DynamicMechanicInstance,
) -> Option<f64> {
    let lifetime_seconds = *mechanic.parameters.get("lifetimeSeconds")?;
    let movement_speed = *mechanic.parameters.get("movementSpeed")?;
    let collision_radius = *mechanic.parameters.get("collisionRadius")?;
    (lifetime_seconds >= 0.0 && movement_speed >= 0.0 && collision_radius >= 0.0)
        .then_some(lifetime_seconds * movement_speed + collision_radius)
}
pub(crate) fn mechanic_u32(mechanic: &impl MechanicParameterSource, key: ParameterKey) -> u32 {
    mechanic_param(mechanic, key).max(0.0) as u32
}
pub(crate) fn seconds_parameter(mechanic: &impl MechanicParameterSource, key: ParameterKey) -> u64 {
    (mechanic_param(mechanic, key).max(0.0) * 1_000.0).round() as u64
}
pub(crate) fn remaining_periodic_tick_equivalents(
    now_ms: u64,
    expires_ms: u64,
    last_tick_ms: u64,
    next_tick_ms: u64,
    period_ms: u64,
) -> f64 {
    let period_ms = period_ms.max(1) as f64;
    let remaining_ms = expires_ms.saturating_sub(now_ms) as f64;
    let time_to_next_ms = next_tick_ms.saturating_sub(now_ms) as f64;
    if remaining_ms >= time_to_next_ms {
        1.0 + (remaining_ms - time_to_next_ms) / period_ms
    } else {
        (now_ms.saturating_sub(last_tick_ms) as f64 + remaining_ms) / period_ms
    }
}

pub(crate) fn refreshed_periodic_tick_equivalents(
    duration_ms: u64,
    time_to_next_ms: u64,
    base_period_ms: u64,
) -> f64 {
    let base_period_ms = base_period_ms.max(1);
    debug_assert!(duration_ms >= time_to_next_ms);
    let complete_periods = duration_ms / base_period_ms;
    let phase_remainder = duration_ms.saturating_sub(time_to_next_ms) % base_period_ms;
    complete_periods as f64 + phase_remainder as f64 / base_period_ms as f64
}

pub(crate) fn multi_target_damage_falloff(target_count: u32, threshold: f64) -> f64 {
    if target_count == 0 {
        return 1.0;
    }
    let threshold = threshold.trunc().clamp(0.0, u32::MAX as f64) as u32;
    if target_count <= threshold {
        1.0
    } else {
        (threshold as f64 / target_count as f64).sqrt()
    }
}

pub(crate) fn spirit_refund_chance(spirit: f64, scale: f64, flat_increase: f64) -> f64 {
    let scaled_spirit = spirit * scale;
    flat_increase + scaled_spirit / (scaled_spirit + 1.0)
}
