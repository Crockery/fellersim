mod apl;
mod cast;
mod cooldown;
mod damage;
mod event;
mod healing;
mod kindling;
mod ruby_storm;
mod shared_mechanics;
mod state;
mod support;

pub(crate) use state::*;
pub(crate) use support::*;

pub use apl::AplNodeEvaluationTrace;
pub(crate) use apl::{
    RuntimeAplRule, compile_runtime_apl, compile_runtime_apl_report, fight_threshold_times,
};
pub(crate) use event::*;

// Immortal benchmark target: Eternal 23 Warlord Brogg. Source observations
// and sampling provenance are in tests/fixtures/dummy-target.json.
// These are fixed encounter assumptions, not character-dependent scaling.
pub(crate) const STATIONARY_DUMMY_MAX_HEALTH: f64 = 6_637_912.0;
pub(crate) const STATIONARY_DUMMY_SPIRIT_VALUE: f64 = 45.0;
