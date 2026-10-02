//! Deterministic DPS-hero stationary-dummy simulation and its cross-language contract.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
    sync::Arc,
};

#[cfg(test)]
use std::sync::atomic::AtomicBool;

mod apl_source;
mod contract;
mod engine;
mod heroes;
pub mod preparation;
mod rng;
pub use apl_source::parse_apl;

pub use contract::*;

use engine::*;
use heroes::*;
use rng::*;

// The current HeroMagicalDirect, HeroMagicalArea, and HeroMagicalOT presets all
// use a spread width of 0.2. The native calculation multiplies damage by
// `1 + roll * width`, where roll is uniformly distributed over [-0.5, 0.5].
const HERO_DAMAGE_SPREAD_WIDTH: f64 = 0.2;
// DefaultGame.ini selects Global.GrievousCritScalar from CT_AbilityConstants;
// the current build's curve is constant at 1.0.
const GLOBAL_GRIEVOUS_CRIT_SCALAR: f64 = 1.0;
const REIGN_OF_FIRE_PPM_STREAM_TAG: &str = "RandomStream.Firemage.Talent.PlacedProjectileAoeHasCharges.CastedDotDetonateChargeRecoveryProc";
const SPONTANEOUS_COMBUSTION_RANDOM_STREAM_TAG: &str =
    "RandomStream.Firemage.Talent.DotDoubleDamageAndHeal.ProcPpm";
const PYROPHIBIAN_CRIT_RANDOM_STREAM_TAG: &str =
    "RandomStream.Firemage.Talent.FireFrogOnDotCritProc.Crit";
const PYROPHIBIAN_NON_CRIT_RANDOM_STREAM_TAG: &str =
    "RandomStream.Firemage.Talent.FireFrogOnDotCritProc.NotCrit";
const FIRE_TOAD_RANDOM_STREAM_TAG: &str = "RandomStream.Firemage.Talent.FireToad.SpawnChance";
const DOT_TICK_RESOURCE_RANDOM_STREAM_TAG: &str =
    "RandomStream.Firemage.RollForDotTickResourceReward";
const SPIRIT_PROC_RANDOM_STREAM_TAG: &str = "RandomStream.Shared.Hero.SpiritProc";
const POWER_CHANCE_SPIRIT_RANDOM_STREAM_TAG: &str = "RandomStream.Finesse.PowerChanceSpirit";
const CRIT_CHANCE_VERDICT_RANDOM_STREAM_TAG: &str = "RandomStream.Finesse.CritChanceToHealOrDamage";
const DRAIN_HEALTH_RANDOM_STREAM_TAG: &str = "RandomStream.Finesse.DrainHealthBasedOnPrimaryStat";
const NAVIGATORS_INTUITION_RANDOM_STREAM_TAG: &str =
    "RandomStream.Traits.OffensiveAbilityToHighestStatBuff";
const RUBY_STORM_SOURCE_ID: &str = "ItemTrait.ID.GemWhirlwindProc";
const REQUIRED_SCENARIO_NO_OP_ABILITIES: [&str; 6] = [
    "GA_Firemage_DashForward",
    "GA_Firemage_InstantSingleDisorient",
    "GA_Firemage_InstantSingleInterrupt",
    "GA_Firemage_Passive_FrogDamageMonitor",
    "GA_Firemage_Passive_PlacedProjectileAoeDamageMonitor",
    "GA_Firemage_SelfDefenceBuff",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeroIdentity {
    Ardeos,
    Rime,
    Tariq,
    Elarion,
    Mara,
    Gunde,
}

impl HeroIdentity {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            ARDEOS_HERO_ID => Some(Self::Ardeos),
            RIME_HERO_ID => Some(Self::Rime),
            TARIQ_HERO_ID => Some(Self::Tariq),
            ELARION_HERO_ID => Some(Self::Elarion),
            MARA_HERO_ID => Some(Self::Mara),
            GUNDE_HERO_ID => Some(Self::Gunde),
            _ => None,
        }
    }

    const fn contract(self) -> HeroContract {
        match self {
            Self::Ardeos => HeroContract {
                hero: self,
                hero_id: ARDEOS_HERO_ID,
                model_version: ARDEOS_MODEL_VERSION,
                scenario_id: ARDEOS_SCENARIO_ID,
                maximum_combat_range_units: ARDEOS_MAX_COMBAT_RANGE_UNITS,
            },
            Self::Rime => HeroContract {
                hero: self,
                hero_id: RIME_HERO_ID,
                model_version: RIME_MODEL_VERSION,
                scenario_id: RIME_SCENARIO_ID,
                maximum_combat_range_units: RIME_MAX_COMBAT_RANGE_UNITS,
            },
            Self::Tariq => HeroContract {
                hero: self,
                hero_id: TARIQ_HERO_ID,
                model_version: TARIQ_MODEL_VERSION,
                scenario_id: TARIQ_SCENARIO_ID,
                maximum_combat_range_units: TARIQ_MAX_COMBAT_RANGE_UNITS,
            },
            Self::Elarion => HeroContract {
                hero: self,
                hero_id: ELARION_HERO_ID,
                model_version: ELARION_MODEL_VERSION,
                scenario_id: ELARION_SCENARIO_ID,
                maximum_combat_range_units: ELARION_MAX_COMBAT_RANGE_UNITS,
            },
            Self::Mara => HeroContract {
                hero: self,
                hero_id: MARA_HERO_ID,
                model_version: MARA_MODEL_VERSION,
                scenario_id: MARA_SCENARIO_ID,
                maximum_combat_range_units: MARA_MAX_COMBAT_RANGE_UNITS,
            },
            Self::Gunde => HeroContract {
                hero: self,
                hero_id: GUNDE_HERO_ID,
                model_version: GUNDE_MODEL_VERSION,
                scenario_id: GUNDE_SCENARIO_ID,
                maximum_combat_range_units: GUNDE_MAX_COMBAT_RANGE_UNITS,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HeroContract {
    hero: HeroIdentity,
    hero_id: &'static str,
    model_version: &'static str,
    scenario_id: &'static str,
    maximum_combat_range_units: f64,
}

impl HeroContract {
    const fn evidence_scope(self, single_target: bool) -> DpsEvidenceScope {
        match (self.hero, single_target) {
            (HeroIdentity::Ardeos, true) => DpsEvidenceScope::ArdeosSingleDummy,
            (HeroIdentity::Ardeos, false) => DpsEvidenceScope::ArdeosStackedDummies,
            (HeroIdentity::Rime, true) => DpsEvidenceScope::RimeSingleDummy,
            (HeroIdentity::Rime, false) => DpsEvidenceScope::RimeStackedDummies,
            (HeroIdentity::Tariq, true) => DpsEvidenceScope::TariqSingleDummy,
            (HeroIdentity::Tariq, false) => DpsEvidenceScope::TariqStackedDummies,
            (HeroIdentity::Elarion, true) => DpsEvidenceScope::ElarionSingleDummy,
            (HeroIdentity::Elarion, false) => DpsEvidenceScope::ElarionStackedDummies,
            (HeroIdentity::Mara, true) => DpsEvidenceScope::MaraSingleDummy,
            (HeroIdentity::Mara, false) => DpsEvidenceScope::MaraStackedDummies,
            (HeroIdentity::Gunde, true) => DpsEvidenceScope::GundeSingleDummy,
            (HeroIdentity::Gunde, false) => DpsEvidenceScope::GundeStackedDummies,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ParameterKey(u64);

impl ParameterKey {
    const fn new(name: &str) -> Self {
        let bytes = name.as_bytes();
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut index = 0;
        while index < bytes.len() {
            hash ^= bytes[index] as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            index += 1;
        }
        Self(hash)
    }
}

macro_rules! parameter_key {
    ($name:literal) => {
        ParameterKey::new($name)
    };
}

pub(crate) use parameter_key;

mod aggregate;
mod compiled_profile;
mod validation;

pub use aggregate::{simulate, simulate_owned, simulate_profile_sweep};

/// Validates an untrusted simulation request without executing it.
pub fn validate_request(request: &SimulationRequest) -> Result<(), SimulationError> {
    validation::validate(request).map(|_| ())
}

#[cfg(test)]
use aggregate::{aggregate, aggregate_profile_sweep};
use compiled_profile::*;
use validation::validate;
#[cfg(test)]
use validation::{validate_action_priority_list, validate_talent};

#[cfg(test)]
mod tests;
