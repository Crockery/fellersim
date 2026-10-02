use std::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::*;

pub(crate) const MAX_ITERATION_WORK_UNITS: u32 = 100_000;
const MAX_ACTIONS_AT_ONE_TIMESTAMP: u32 = 64;
const MAX_PENDING_EVENTS: usize = 16_384;
const CANCELLATION_CHECK_WORK_UNITS: u32 = 256;

#[derive(Debug)]
pub(crate) struct ExecutionControl<'a> {
    cancelled: &'a AtomicBool,
    work_units: Cell<u32>,
    work_since_cancel_check: Cell<u32>,
    lightweight_loop_remainder: Cell<usize>,
    action_timestamp_ms: Cell<u64>,
    actions_at_timestamp: Cell<u32>,
    failure: RefCell<Option<SimulationError>>,
    #[cfg(feature = "strict-work-budget")]
    work_by_location: RefCell<BTreeMap<(&'static str, u32), u32>>,
}

impl<'a> ExecutionControl<'a> {
    fn new(cancelled: &'a AtomicBool) -> Self {
        Self {
            cancelled,
            work_units: Cell::new(0),
            work_since_cancel_check: Cell::new(0),
            lightweight_loop_remainder: Cell::new(0),
            action_timestamp_ms: Cell::new(u64::MAX),
            actions_at_timestamp: Cell::new(0),
            failure: RefCell::new(None),
            #[cfg(feature = "strict-work-budget")]
            work_by_location: RefCell::new(BTreeMap::new()),
        }
    }

    #[track_caller]
    pub(crate) fn charge(&self, units: u32) -> bool {
        if self.failure.borrow().is_some() {
            return false;
        }
        let Some(next) = self.work_units.get().checked_add(units) else {
            self.fail_limit();
            return false;
        };
        if next > MAX_ITERATION_WORK_UNITS {
            self.fail_limit();
            return false;
        }
        #[cfg(feature = "strict-work-budget")]
        {
            let caller = std::panic::Location::caller();
            let mut work_by_location = self.work_by_location.borrow_mut();
            *work_by_location
                .entry((caller.file(), caller.line()))
                .or_default() += units;
        }
        self.work_units.set(next);
        let since_check = self.work_since_cancel_check.get().saturating_add(units);
        if since_check >= CANCELLATION_CHECK_WORK_UNITS {
            self.work_since_cancel_check.set(0);
            if self.cancelled.load(Ordering::Relaxed) {
                self.fail(SimulationError::new(
                    SimulationErrorCode::Cancelled,
                    "Simulation cancelled",
                ));
                return false;
            }
        } else {
            self.work_since_cancel_check.set(since_check);
        }
        true
    }

    #[track_caller]
    pub(crate) fn charge_usize(&self, units: usize) -> bool {
        match u32::try_from(units) {
            Ok(units) => self.charge(units),
            Err(_) => {
                self.fail_limit();
                false
            }
        }
    }

    /// Charges low-cost collection maintenance at eight entries per work unit.
    /// Partial units carry across calls so frequent small indexed scans do not
    /// each pay a full work unit.
    #[track_caller]
    pub(crate) fn charge_lightweight_loop(&self, iterations: usize) -> bool {
        let Some(total) = self
            .lightweight_loop_remainder
            .get()
            .checked_add(iterations)
        else {
            self.fail_limit();
            return false;
        };
        self.lightweight_loop_remainder.set(total % 8);
        self.charge_usize(total / 8)
    }

    pub(crate) fn record_action(&self, now_ms: u64) -> bool {
        if !self.charge(1) {
            return false;
        }
        if self.action_timestamp_ms.get() != now_ms {
            self.action_timestamp_ms.set(now_ms);
            self.actions_at_timestamp.set(1);
            return true;
        }
        let count = self.actions_at_timestamp.get().saturating_add(1);
        self.actions_at_timestamp.set(count);
        if count > MAX_ACTIONS_AT_ONE_TIMESTAMP {
            self.fail(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation performed too many actions without advancing time",
            ));
            return false;
        }
        true
    }

    pub(crate) fn check_cancelled(&self) -> bool {
        if self.failure.borrow().is_some() {
            return false;
        }
        if self.cancelled.load(Ordering::Relaxed) {
            self.fail(SimulationError::new(
                SimulationErrorCode::Cancelled,
                "Simulation cancelled",
            ));
            return false;
        }
        true
    }

    pub(crate) fn fail_limit(&self) {
        #[cfg(feature = "strict-work-budget")]
        let message = format!(
            "simulation exceeded the safe execution budget; largest charges: {:?}",
            self.work_breakdown()
                .into_iter()
                .take(12)
                .collect::<Vec<_>>()
        );
        #[cfg(not(feature = "strict-work-budget"))]
        let message = "simulation exceeded the safe execution budget";
        self.fail(SimulationError::new(
            SimulationErrorCode::SimulationFailed,
            message,
        ));
    }

    pub(crate) fn fail(&self, error: SimulationError) {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(error);
        }
    }

    pub(crate) fn failed(&self) -> bool {
        self.failure.borrow().is_some()
    }

    fn error(&self) -> Option<SimulationError> {
        self.failure.borrow().clone()
    }

    fn work_units(&self) -> u32 {
        self.work_units.get()
    }

    #[cfg(feature = "strict-work-budget")]
    fn work_breakdown(&self) -> Vec<(&'static str, u32, u32)> {
        let mut work = self
            .work_by_location
            .borrow()
            .iter()
            .map(|((file, line), units)| (*file, *line, *units))
            .collect::<Vec<_>>();
        work.sort_by_key(|(_, _, units)| std::cmp::Reverse(*units));
        work
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AbilityTotals {
    pub(crate) damage: f64,
    pub(crate) hits: u64,
    pub(crate) crits: u64,
    pub(crate) grievous: u64,
    pub(crate) casts: u64,
    pub(crate) targets_hit_mask: u32,
    pub(crate) targets_hit_total: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct IterationResult {
    pub(crate) trace: Vec<crate::TraceDecision>,
    pub(crate) trace_truncated: bool,
    pub(crate) damage: f64,
    pub(crate) targets: Vec<f64>,
    pub(crate) abilities: Vec<AbilityTotals>,
    pub(crate) mechanic_proc_counts: Vec<f64>,
    pub(crate) proc_counts: Vec<u64>,
    pub(crate) mechanic_buff_uptimes: Vec<f64>,
    pub(crate) uptimes: BTreeMap<String, f64>,
    pub(crate) work_units: u32,
    #[cfg(feature = "strict-work-budget")]
    pub(crate) work_breakdown: Vec<(&'static str, u32, u32)>,
}

#[derive(Debug, Clone)]
pub(crate) struct DotState {
    pub(crate) model: DotModel,
    pub(crate) source: DamageSourceKey,
    pub(crate) expertise_snapshot: f64,
    pub(crate) primary_stat_multiplier_snapshot: f64,
    pub(crate) derived_damage_per_tick: Option<f64>,
    pub(crate) gunde_rend_buckets: Option<std::collections::VecDeque<f64>>,
    pub(crate) critical_chance_override: Option<f64>,
    pub(crate) bonus_crit: f64,
    pub(crate) generation: u64,
    pub(crate) started_ms: u64,
    pub(crate) expires_ms: u64,
    pub(crate) stacks: u32,
    pub(crate) context: DamageContext,
    pub(crate) last_tick_ms: u64,
    pub(crate) next_tick_ms: u64,
    pub(crate) scheduled_period_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PeriodicHealingClock {
    pub(crate) remaining_base_ms: f64,
    pub(crate) updated_ms: u64,
    pub(crate) rate: f64,
    pub(crate) wake_ms: u64,
    pub(crate) generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DotKind {
    Ability(DpsAbilityKind),
    OverlappingAbility(DpsAbilityKind, u64),
    CracklingInferno,
    ApocalypseBurn,
    MaraArachnidPoison,
}

impl DotKind {
    pub(crate) fn has_independent_instances(self) -> bool {
        matches!(
            self.ability_kind(),
            Some(
                DpsAbilityKind::EngulfingFlames
                    | DpsAbilityKind::Slaughter
                    | DpsAbilityKind::StarfallVolley
                    | DpsAbilityKind::BloodboundSpirit
            )
        )
    }

    pub(crate) fn applies_effect_each_tick(self) -> bool {
        // These area actors repeatedly apply a damage spec. Their DotState
        // represents the actor's lifetime, not an active effect on the target.
        matches!(
            self.ability_kind(),
            Some(DpsAbilityKind::StarfallVolley | DpsAbilityKind::BloodboundSpirit)
        )
    }

    pub(crate) fn ability_kind(self) -> Option<DpsAbilityKind> {
        match self {
            Self::Ability(kind) | Self::OverlappingAbility(kind, _) => Some(kind),
            Self::CracklingInferno | Self::ApocalypseBurn | Self::MaraArachnidPoison => None,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Ability(kind) | Self::OverlappingAbility(kind, _) => ability_kind_id(kind),
            Self::CracklingInferno => "crackling-inferno",
            Self::ApocalypseBurn => "apocalypse-burn",
            Self::MaraArachnidPoison => "mara-arachnid-poison",
        }
    }

    pub(crate) fn is_ardeos_hero_dot(self) -> bool {
        match self {
            Self::Ability(kind) | Self::OverlappingAbility(kind, _) => is_ardeos_hero_dot(kind),
            Self::CracklingInferno | Self::ApocalypseBurn => true,
            Self::MaraArachnidPoison => false,
        }
    }

    pub(crate) fn is_damage_derived(self) -> bool {
        matches!(self.ability_kind(), Some(DpsAbilityKind::Slaughter))
            || matches!(
                self,
                Self::Ability(DpsAbilityKind::FireBall | DpsAbilityKind::FireFrogs)
                    | Self::Ability(DpsAbilityKind::Rend)
                    | Self::CracklingInferno
                    | Self::ApocalypseBurn
                    | Self::MaraArachnidPoison
            )
    }

    pub(crate) fn is_pyrophibian_relevant(self) -> bool {
        match self {
            Self::Ability(kind) | Self::OverlappingAbility(kind, _) => {
                is_pyrophibian_relevant_dot(kind)
            }
            Self::CracklingInferno | Self::ApocalypseBurn => true,
            Self::MaraArachnidPoison => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DynamicBuffState {
    pub(crate) started_ms: u64,
    pub(crate) until_ms: u64,
    pub(crate) stacks: u32,
    pub(crate) value: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FixedBuffUptimeState {
    pub(crate) started_ms: Option<u64>,
    pub(crate) until_ms: u64,
    pub(crate) accumulated_ms: u64,
}

impl FixedBuffUptimeState {
    pub(crate) fn activate(&mut self, now_ms: u64, until_ms: u64) {
        self.settle_expired(now_ms);
        if self.started_ms.is_none() && until_ms > now_ms {
            self.started_ms = Some(now_ms);
        }
        self.until_ms = until_ms;
    }

    pub(crate) fn deactivate(&mut self, now_ms: u64) {
        if let Some(started_ms) = self.started_ms.take() {
            self.accumulated_ms = self
                .accumulated_ms
                .saturating_add(now_ms.min(self.until_ms).saturating_sub(started_ms));
        }
        self.until_ms = 0;
    }

    pub(crate) fn total_active_ms(&self, through_ms: u64) -> u64 {
        self.accumulated_ms.saturating_add(
            self.started_ms
                .map(|started_ms| through_ms.min(self.until_ms).saturating_sub(started_ms))
                .unwrap_or(0),
        )
    }

    pub(crate) fn settle_expired(&mut self, now_ms: u64) {
        if self.started_ms.is_some() && self.until_ms <= now_ms {
            self.deactivate(self.until_ms);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DamageSourceSnapshot {
    pub(crate) hero_damage_scale: f64,
    pub(crate) expertise: f64,
    pub(crate) primary_stat_multiplier: f64,
    pub(crate) critical_chance: f64,
}

impl PartialEq for DamageSourceSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.hero_damage_scale.to_bits() == other.hero_damage_scale.to_bits()
            && self.expertise.to_bits() == other.expertise.to_bits()
            && self.primary_stat_multiplier.to_bits() == other.primary_stat_multiplier.to_bits()
            && self.critical_chance.to_bits() == other.critical_chance.to_bits()
    }
}

impl Eq for DamageSourceSnapshot {}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DamageContext {
    pub(crate) category_override: Option<AbilityCategory>,
    pub(crate) source_cast: Option<u64>,
    pub(crate) source_snapshot: Option<DamageSourceSnapshot>,
    pub(crate) damage_multiplier: f64,
    pub(crate) power_damage_multiplier: f64,
    pub(crate) cast_proc_eligible: bool,
}

impl PartialEq for DamageContext {
    fn eq(&self, other: &Self) -> bool {
        self.category_override == other.category_override
            && self.source_cast == other.source_cast
            && self.source_snapshot == other.source_snapshot
            && self.damage_multiplier.to_bits() == other.damage_multiplier.to_bits()
            && self.power_damage_multiplier.to_bits() == other.power_damage_multiplier.to_bits()
            && self.cast_proc_eligible == other.cast_proc_eligible
    }
}

impl Eq for DamageContext {}

impl Default for DamageContext {
    fn default() -> Self {
        Self::NONE
    }
}

impl DamageContext {
    pub(crate) fn category(self, kind: Option<DpsAbilityKind>) -> Option<AbilityCategory> {
        self.category_override
            .or_else(|| kind.map(ability_category))
    }

    pub(crate) const NONE: Self = Self {
        category_override: None,
        source_cast: None,
        source_snapshot: None,
        damage_multiplier: 1.0,
        power_damage_multiplier: 1.0,
        cast_proc_eligible: false,
    };

    pub(crate) const fn for_cast(cast_id: u64) -> Self {
        Self {
            category_override: None,
            source_cast: Some(cast_id),
            source_snapshot: None,
            damage_multiplier: 1.0,
            power_damage_multiplier: 1.0,
            cast_proc_eligible: true,
        }
    }

    pub(crate) const fn with_snapshot(self, source_snapshot: DamageSourceSnapshot) -> Self {
        Self {
            source_snapshot: Some(source_snapshot),
            ..self
        }
    }

    pub(crate) const fn as_proc(self) -> Self {
        Self {
            category_override: None,
            source_snapshot: None,
            damage_multiplier: 1.0,
            power_damage_multiplier: 1.0,
            cast_proc_eligible: false,
            ..self
        }
    }

    pub(crate) const fn without_cast_proc(self) -> Self {
        Self {
            cast_proc_eligible: false,
            ..self
        }
    }

    pub(crate) fn multiply_damage(&mut self, multiplier: f64) {
        self.damage_multiplier *= multiplier.max(0.0);
    }

    pub(crate) fn multiply_power_damage(&mut self, multiplier: f64) {
        self.power_damage_multiplier *= multiplier.max(0.0);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CastImpactContext {
    pub(crate) damage: DamageContext,
    pub(crate) bonus_crit: f64,
    pub(crate) ardeos_wave_empowered: bool,
    pub(crate) glacial_assault: bool,
    pub(crate) frostweaver: bool,
    pub(crate) impending_heartseeker: bool,
    pub(crate) elarion_celestial_impetus: bool,
    pub(crate) mara_combo_points_spent: u32,
    pub(crate) mara_from_stealth: bool,
    pub(crate) gunde_rend_transfer_bonus: f64,
    pub(crate) gunde_legendary_strike: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ImpactSpec {
    pub(crate) single_hit: bool,
    pub(crate) damage_scale_bits: u64,
    pub(crate) target_index: u32,
}

impl CastImpactContext {
    pub(crate) const fn new(damage: DamageContext) -> Self {
        Self {
            damage,
            bonus_crit: 0.0,
            ardeos_wave_empowered: false,
            glacial_assault: false,
            frostweaver: false,
            impending_heartseeker: false,
            elarion_celestial_impetus: false,
            mara_combo_points_spent: 0,
            mara_from_stealth: false,
            gunde_rend_transfer_bonus: 0.0,
            gunde_legendary_strike: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedCast {
    pub(crate) index: usize,
    pub(crate) ability: PreparedAbility,
    pub(crate) context: DamageContext,
    pub(crate) ability_time_rate: f64,
    pub(crate) free_charge: bool,
    pub(crate) bonus_crit: f64,
    pub(crate) ardeos_wave_empowered: bool,
    pub(crate) glacial_assault: bool,
    pub(crate) elarion_multishot_arrows: u32,
    pub(crate) elarion_focus_cost: f64,
    pub(crate) mara_copy_damage_multiplier: f64,
    pub(crate) elarion_celestial_impetus: bool,
    pub(crate) impending_heartseeker: bool,
    pub(crate) mara_combo_points_spent: u32,
    pub(crate) mara_from_stealth: bool,
    pub(crate) gunde_rend_transfer_bonus: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedAbility {
    pub(crate) model: Arc<CompiledAbility>,
    pub(crate) cast_time_ms: u64,
    pub(crate) first_hit_delay_ms: u64,
    pub(crate) max_targets: u32,
    pub(crate) channel: Option<ChannelModel>,
}

impl PreparedAbility {
    pub(crate) fn new(model: Arc<CompiledAbility>) -> Self {
        Self {
            cast_time_ms: model.cast_time_ms,
            first_hit_delay_ms: model.first_hit_delay_ms,
            max_targets: model.max_targets,
            channel: model.channel,
            model,
        }
    }
}

impl std::ops::Deref for PreparedAbility {
    type Target = CompiledAbility;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct CooldownState {
    pub(crate) remaining_ms: f64,
    pub(crate) used_charges: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShadowMarkState {
    pub(crate) generation: u64,
    pub(crate) until_ms: u64,
    pub(crate) accumulated_damage: f64,
    pub(crate) context: DamageContext,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AurastoneState {
    pub(crate) generation: u64,
    pub(crate) until_ms: u64,
    pub(crate) accumulated_damage: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AmethystSplintersState {
    pub(crate) generation: u64,
    pub(crate) until_ms: u64,
    pub(crate) next_tick_ms: u64,
    pub(crate) damage_per_tick: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct KindlingState {
    pub(crate) remaining_base_ms: f64,
    pub(crate) clock_updated_ms: u64,
    pub(crate) clock_rate: f64,
    pub(crate) generation: u64,
    pub(crate) until_ms: u64,
    pub(crate) next_tick_ms: u64,
    pub(crate) damage_per_tick: f64,
    pub(crate) expertise_snapshot: f64,
    pub(crate) primary_stat_multiplier_snapshot: f64,
    pub(crate) critical_chance_snapshot: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct FireFrogState {
    pub(crate) ability_index: usize,
    pub(crate) target_index: u32,
    pub(crate) target_order: Vec<u32>,
    pub(crate) attacks_remaining: u32,
    pub(crate) coefficient: f64,
    pub(crate) context: DamageContext,
    pub(crate) roll_for_toad: bool,
    pub(crate) is_toad: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FireFrogBatch {
    pub(crate) main_target: u32,
    pub(crate) frog_count: u32,
    pub(crate) attacks_per_frog: u32,
    pub(crate) coefficient: f64,
    pub(crate) context: DamageContext,
    pub(crate) allow_bonus_toad: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamageProvenance {
    Direct,
    Periodic,
    Proc,
    Explosion,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DamageAmount {
    Base(f64),
    TargetScaled(f64),
    Unscaled(f64),
}

#[derive(Debug, Clone)]
pub(crate) struct OutgoingDamageEvent {
    pub(crate) source: DamageSourceKey,
    pub(crate) ability_kind: Option<DpsAbilityKind>,
    pub(crate) context: DamageContext,
    pub(crate) target_index: u32,
    pub(crate) provenance: DamageProvenance,
    pub(crate) amount: DamageAmount,
    pub(crate) expertise_snapshot: Option<f64>,
    pub(crate) primary_stat_multiplier_snapshot: Option<f64>,
    pub(crate) damage_spread: f64,
    pub(crate) bonus_crit: f64,
    pub(crate) critical_chance_override: Option<f64>,
    pub(crate) can_crit: bool,
    pub(crate) proc_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorRegistrationOrder {
    pub(crate) enemy_targets: Vec<u32>,
}

impl ActorRegistrationOrder {
    pub(crate) fn primary_then_synthetic(target_count: u32) -> Self {
        Self {
            // The stationary scenario creates its selected primary target
            // first, followed by synthetic targets in index order. Native
            // actor-database queries preserve this registration sequence.
            enemy_targets: (0..target_count).collect(),
        }
    }

    pub(crate) fn enemy_targets(&self) -> &[u32] {
        &self.enemy_targets
    }
}

#[derive(Debug)]
pub(crate) struct CommonState<'a> {
    pub(crate) action_priority_list: &'a ActionPriorityListV2,
    pub(crate) runtime_action_priority_list: Vec<RuntimeAplRule>,
    pub(crate) apl_fight_threshold_times: Vec<u64>,
    pub(crate) apl_fight_threshold_cursor: Cell<usize>,
    pub(crate) abilities_by_kind: &'a BTreeMap<DpsAbilityKind, usize>,
    pub(crate) selected_talents: SelectedTalents<'a>,
    pub(crate) now_ms: u64,
    pub(crate) sequence: u64,
    pub(crate) cast_sequence: u64,
    pub(crate) queue: BinaryHeap<Reverse<Event>>,
    pub(crate) cooldowns: BTreeMap<DpsAbilityKind, CooldownState>,
    pub(crate) dots: BTreeMap<(u32, DotKind), DotState>,
    pub(crate) dot_uptime_intervals: BTreeMap<(u32, DpsAbilityKind), Vec<(u64, u64)>>,
    pub(crate) target_count: u32,
    pub(crate) actor_registration_order: ActorRegistrationOrder,
    pub(crate) fixed_buff_uptimes: BTreeMap<&'static str, FixedBuffUptimeState>,
    pub(crate) rng: SplitMix64,
    pub(crate) result: IterationResult,
    pub(crate) execution: ExecutionControl<'a>,
}

#[derive(Debug, Default)]
pub(crate) struct CriticalListenerDispatch {
    pub(crate) reactivated_counter: Option<usize>,
    pub(crate) counter_dispatched: bool,
}

#[derive(Debug)]
pub(crate) struct SharedMechanicsState {
    pub(crate) periodic_healing: Option<PeriodicHealingClock>,
    pub(crate) spirit: f64,
    pub(crate) heroism_started_ms: Option<u64>,
    pub(crate) heroism_until: u64,
    pub(crate) heroism_power_set_expirations: Vec<u64>,
    pub(crate) proc_per_minute_states: BTreeMap<String, ProcPerMinuteState>,
    pub(crate) dynamic_proc_per_minute_states: Vec<Option<ProcPerMinuteState>>,
    pub(crate) controlled_random_states: BTreeMap<String, ControlledRandomState>,
    pub(crate) dynamic_last_roll_ms: Vec<Option<u64>>,
    pub(crate) dynamic_ready_ms: Vec<u64>,
    pub(crate) dynamic_buffs: Vec<Option<DynamicBuffState>>,
    pub(crate) dynamic_target_buffs: Vec<Option<DynamicBuffState>>,
    pub(crate) dynamic_counters: Vec<u32>,
    pub(crate) dynamic_counter_expirations_ms: Vec<u64>,
    pub(crate) dynamic_event_generations: Vec<u64>,
    pub(crate) dynamic_pending_cooldown_reductions_ms: Vec<u64>,
    pub(crate) dynamic_touched_targets: Vec<bool>,
    pub(crate) shadow_marks: BTreeMap<u32, ShadowMarkState>,
    pub(crate) cone_stun_until: Vec<u64>,
    pub(crate) aurastones: Vec<Option<AurastoneState>>,
    pub(crate) amethyst_splinters: Vec<Option<AmethystSplintersState>>,
    pub(crate) kindling: Vec<Option<KindlingState>>,
    pub(crate) kindling_healing: Vec<Option<KindlingState>>,
    pub(crate) weapon_channel_cooldown_recovery_until: u64,
    pub(crate) weapon_charge_buff_until: u64,
    pub(crate) weapon_charge_lock_until: u64,
    pub(crate) weapon_critical_buff_until: u64,
    pub(crate) weapon_critical_buff_stacks: u32,
    pub(crate) wayfarer_next_ms: u64,
    pub(crate) wayfarer_generation: u64,
}

#[derive(Debug)]
pub(crate) struct Iteration<'a> {
    // Counterfactual placement for the offline sensitivity sweep only.
    #[cfg(test)]
    pub(crate) test_heretic_before_generic: bool,
    pub(crate) profile: &'a CompiledProfile,
    pub(crate) common: CommonState<'a>,
    pub(crate) shared: SharedMechanicsState,
    pub(crate) hero: HeroState,
}

impl<'a> Iteration<'a> {
    pub(crate) fn from_compiled(
        profile: &'a CompiledProfile,
        action_priority_list: &'a ActionPriorityListV2,
        target_count: u32,
        seed: u64,
        cancelled: &'a AtomicBool,
    ) -> Self {
        let selected_talents = SelectedTalents { profile };
        let (runtime_action_priority_list, apl_compile_work) =
            compile_runtime_apl(action_priority_list, profile, target_count);
        let mut apl_fight_threshold_times = Vec::new();
        let mut apl_threshold_collection_work = runtime_action_priority_list.len();
        for rule in &runtime_action_priority_list {
            if rule.active_from_ms > 0 {
                apl_fight_threshold_times.push(rule.active_from_ms);
            }
            if rule.active_until_ms < u64::MAX {
                apl_fight_threshold_times.push(rule.active_until_ms);
            }
            if let Some(condition) = &rule.condition {
                let (times, visited) = fight_threshold_times(condition);
                apl_threshold_collection_work =
                    apl_threshold_collection_work.saturating_add(visited);
                apl_fight_threshold_times.extend(times);
            }
        }
        apl_fight_threshold_times.sort_unstable();
        apl_fight_threshold_times.dedup();
        let execution = ExecutionControl::new(cancelled);
        execution.charge_usize(apl_compile_work.saturating_add(apl_threshold_collection_work));
        let result = IterationResult {
            targets: vec![0.0; target_count as usize],
            abilities: vec![AbilityTotals::default(); profile.damage_sources.len()],
            mechanic_proc_counts: vec![0.0; profile.mechanics.len()],
            proc_counts: vec![0; profile.proc_sources.len()],
            mechanic_buff_uptimes: vec![0.0; profile.mechanics.len()],
            ..IterationResult::default()
        };
        let ardeos_starting = selected_talents.contains_key("firemage-talent-id-talent15");
        let talent_starting_spirit = selected_talents
            .get("firemage-talent-id-talent15")
            .and_then(|talent| talent.parameters.get(parameter_key!("startingSpirit")))
            .unwrap_or(0.0)
            + selected_talents
                .get("ink-talent-id-talent13")
                .and_then(|talent| talent.parameters.get(parameter_key!("startingSpirit")))
                .unwrap_or(0.0);
        let mechanic_starting_spirit = profile.mechanic_starting_spirit;
        let starting_embers = selected_talents
            .get("firemage-talent-id-talent15")
            .and_then(|talent| talent.parameters.get(parameter_key!("startingEmbers")))
            .unwrap_or(0.0) as u32;
        let hero = match profile.contract.hero {
            HeroIdentity::Ardeos => HeroState::Ardeos(ArdeosState {
                cinders: 0.0,
                embers: if ardeos_starting {
                    starting_embers.min(profile.max_secondary_resource)
                } else {
                    0
                },
                wildfire_until: 0,
                cascading_stacks: 0,
                apocalyptic_surge: 0,
                apocalyptic_surge_until: 0,
                reign_fireball_stacks: 0,
                reign_fireball_until: 0,
                fire_frogs: BTreeMap::new(),
                fire_frog_busy_targets: vec![0; target_count as usize],
                fire_frog_actor_sequence: 0,
            }),
            HeroIdentity::Rime => HeroState::Rime(Box::new(RimeState {
                anima: 0.0,
                winter_orbs: 0,
                ice_blitz_until: 0,
                winters_blessing_until: 0,
                blessing_heal_generation: 0,
                blessing_heal_pending: 0.0,
                blessing_heal_scheduled: false,
                wrath_of_winter_until: 0,
                flight_of_the_navir_until: 0,
                glacial_assault_stacks: 0,
                icy_flow_casting_haste: 0.0,
                icy_flow_stacks: 0,
                icy_flow_until: 0,
                soulfrost_torrent_until: 0,
                frostweavers_wrath_until: 0,
                frostweavers_wrath_stacks: 0,
                harrowing_ice_stacks: 0,
                harrowing_ice_until: 0,
                bursting_generation: 0,
                bursting_until: 0,
                bursting_instances: BTreeMap::new(),
                wrath_generation: 0,
                wrath_period_ms: 0,
                flight_generation: 0,
                flight_birds: Vec::new(),
                navir_free_until: 0,
                navir_free_cold_snaps: 0,
                undulating_spirit_stacks: 0,
                undulating_spirit_until: 0,
                coalescing_stacks: vec![0; target_count as usize],
                coalescing_generations: vec![0; target_count as usize],
                frostwyrm_stacks: 0,
                frostwyrm_until: 0,
            })),
            HeroIdentity::Tariq => HeroState::Tariq(TariqState {
                swing_remaining_ms: profile
                    .ability(DpsAbilityKind::TariqAttack)
                    .map(|attack| {
                        ability_param(attack, parameter_key!("hitWindowSeconds")) * 1_000.0
                    })
                    .unwrap_or(0.0),
                swing_updated_ms: 0,
                swing_rate: 0.0,
                swing_next_ms: None,
                swing_generation: 0,
                auto_attacking: false,
                auto_blocked_until: 0,
                fury: 0.0,
                thunder_fury_until: 0,
                thunder_fury_generation: 0,
                raging_period_ms: 0,
                thunder_call_until: 0,
                focused_wrath_until: 0,
                focused_wrath_stacks: 0,
                raging_tempest_until: 0,
                raging_tempest_generation: 0,
                raging_current_stacks: 0,
                far_beyond_driven_until: 0,
                far_beyond_driven_stacks: 0,
                kill_em_all_until: 0,
                kill_em_all_stacks: 0,
                square_hammer_until: 0,
                square_hammer_stacks: 0,
                square_hammer_expertise_until: 0,
                schism_hammer_stacks: 0,
                schism_hammer_until: 0,
                schism_skull_stacks: 0,
                schism_skull_until: 0,
                passive_fury_generation: 0,
                thundering_vortex_stacks: 0,
                slayers_mosh_until: vec![0; target_count as usize],
                executioners_grin_until: 0,
                executioners_grin_stacks: 0,
            }),
            HeroIdentity::Elarion => HeroState::Elarion(ElarionState {
                focus: profile.max_primary_resource,
                swing_remaining_ms: 0.0,
                swing_updated_ms: 0,
                swing_rate: 0.0,
                swing_generation: 0,
                swing_next_ms: None,
                auto_attacking: false,
                auto_blocked_until: 0,
                skylit_grace_active: false,
                celestial_impetus_stacks: 0,
                celestial_impetus_until: 0,
                multishot_proc_stacks: 0,
                empowered_multishot_stacks: 0,
                empowered_multishot_until: 0,
                skystriders_grace_until: 0,
                event_horizon_until: 0,
                skystriders_supremacy_until: 0,
                skystriders_supremacy_stacks: 0,
                impending_heartseeker_until: 0,
                resurgent_winds_stacks: 0,
                resurgent_winds_until: 0,
                highwind_casts: 0,
                strikers_aim_stacks: 0,
                strikers_aim_generation: 0,
                strikers_aim_next_decay_ms: 0,
                mark_stacks: vec![0; target_count as usize],
                mark_until: vec![0; target_count as usize],
                previous_mark_event_was_starfall: false,
                preserve_mark_stack: false,
                cached_mark_target_index: 0,
                shimmer_stacks: vec![0; target_count as usize],
                shimmer_until: vec![0; target_count as usize],
            }),
            HeroIdentity::Mara => HeroState::Mara(MaraState {
                energy: profile.max_primary_resource,
                combo_points: 0,
                stealth_active: false,
                maiden_of_death_until: 0,
                matriarch_macabre_until: 0,
                assassins_guile_until: 0,
                deadly_scheme_stacks: 0,
                deadly_scheme_energy_remainder: 0.0,
                deadly_scheme_overflow: 0,
                deadly_scheme_until: 0,
                feed_the_queen_stacks: 0,
                feed_the_queen_until: 0,
                malevolence_arachnid_stacks: 0,
                malevolence_arachnid_until: 0,
                malevolence_queen_stacks: 0,
                malevolence_queen_until: 0,
                drenched_in_blood_until: 0,
                seething_poison_until: vec![0; target_count as usize],
                seething_poison_max_until: 0,
            }),
            HeroIdentity::Gunde => HeroState::Gunde(GundeState {
                swing_remaining_ms: 0.0,
                swing_updated_ms: 0,
                swing_rate: 0.0,
                swing_generation: 0,
                swing_next_ms: None,
                auto_attacking: false,
                auto_blocked_until: 0,
                spirit_proc_activations: std::collections::VecDeque::new(),
                blood_feathers: 0,
                blood_feathers_until: 0,
                ground_feathers: 0,
                reign_in_blood_until: 0,
                bloodbound_spirit_until: 0,
                serrated_edge_until: 0,
                deaths_arc_until: 0,
                grim_harvest_until: 0,
                harvesters_toll_until: 0,
                crimson_strikes_until: 0,
                murder_of_crows_stacks: 0,
                murder_of_crows_until: 0,
                massacre_stacks: 0,
                massacre_until: 0,
                ancestral_instinct_until: 0,
                bloodbath_until: 0,
                carrion_onslaught_until: 0,
                carrion_damage_multiplier: 1.0,
                carrion_pending_feathers: 0,
                carrion_generation: 0,
                bloodcraze_generation: 0,
                open_wounds_until: vec![0; target_count as usize],
            }),
        };
        let mut iteration = Self {
            #[cfg(test)]
            test_heretic_before_generic: false,
            profile,
            common: CommonState {
                action_priority_list,
                runtime_action_priority_list,
                apl_fight_threshold_times,
                apl_fight_threshold_cursor: Cell::new(0),
                abilities_by_kind: &profile.abilities_by_kind,
                selected_talents,
                now_ms: 0,
                sequence: 0,
                cast_sequence: 0,
                queue: BinaryHeap::new(),
                cooldowns: BTreeMap::new(),
                dots: BTreeMap::new(),
                dot_uptime_intervals: BTreeMap::new(),
                target_count,
                actor_registration_order: ActorRegistrationOrder::primary_then_synthetic(
                    target_count,
                ),
                fixed_buff_uptimes: BTreeMap::new(),
                rng: SplitMix64::new(seed),
                result,
                execution,
            },
            shared: SharedMechanicsState {
                periodic_healing: None,
                spirit: (talent_starting_spirit + mechanic_starting_spirit).min(profile.max_spirit),
                heroism_started_ms: None,
                heroism_until: 0,
                heroism_power_set_expirations: Vec::new(),
                proc_per_minute_states: BTreeMap::new(),
                dynamic_proc_per_minute_states: vec![None; profile.mechanics.len()],
                controlled_random_states: BTreeMap::new(),
                dynamic_last_roll_ms: vec![None; profile.mechanics.len()],
                dynamic_ready_ms: vec![0; profile.mechanics.len()],
                dynamic_buffs: vec![None; profile.mechanics.len()],
                dynamic_target_buffs: vec![None; profile.mechanics.len() * target_count as usize],
                dynamic_counters: vec![0; profile.mechanics.len()],
                dynamic_counter_expirations_ms: vec![0; profile.mechanics.len()],
                dynamic_event_generations: vec![0; profile.mechanics.len()],
                dynamic_pending_cooldown_reductions_ms: vec![0; profile.mechanics.len()],
                dynamic_touched_targets: vec![
                    false;
                    profile.mechanics.len() * target_count as usize
                ],
                shadow_marks: BTreeMap::new(),
                cone_stun_until: vec![0; target_count as usize],
                aurastones: vec![None; profile.mechanics.len()],
                amethyst_splinters: vec![None; profile.mechanics.len() * target_count as usize],
                kindling: vec![None; profile.mechanics.len() * target_count as usize],
                kindling_healing: vec![None; profile.mechanics.len()],
                weapon_channel_cooldown_recovery_until: 0,
                weapon_charge_buff_until: 0,
                weapon_charge_lock_until: 0,
                weapon_critical_buff_until: 0,
                weapon_critical_buff_stacks: 0,
                wayfarer_next_ms: 0,
                wayfarer_generation: 0,
            },
            hero,
        };
        iteration.start_wayfarer_cycle();
        iteration.refresh_periodic_healing_clock();
        iteration.start_tariq_passive_fury();
        iteration
    }

    #[cfg(test)]
    pub(crate) fn new(
        profile: &NormalizedDpsProfile,
        action_priority_list: &'a ActionPriorityListV2,
        target_count: u32,
        seed: u64,
    ) -> Self {
        static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
        let profile = Box::leak(Box::new(CompiledProfile::compile_test(profile)));
        Self::from_compiled(
            profile,
            action_priority_list,
            target_count,
            seed,
            &NEVER_CANCELLED,
        )
    }

    #[cfg(test)]
    pub(crate) fn ability_totals(&self, id: &str) -> &AbilityTotals {
        let source = self
            .profile
            .damage_source_key(id)
            .unwrap_or_else(|| panic!("test profile has no damage source {id}"));
        &self.common.result.abilities[source.0]
    }

    #[cfg(test)]
    pub(crate) fn has_ability_totals(&self, id: &str) -> bool {
        let totals = self.ability_totals(id);
        totals.damage != 0.0
            || totals.hits != 0
            || totals.crits != 0
            || totals.grievous != 0
            || totals.casts != 0
            || totals.targets_hit_mask != 0
    }

    #[cfg(test)]
    pub(crate) fn test_damage_source(&self, id: &str) -> DamageSourceKey {
        self.profile
            .damage_source_key(id)
            .unwrap_or_else(|| self.profile.abilities[0].damage_source)
    }

    #[cfg(test)]
    pub(crate) fn test_mechanic_index(&self, instance_id: &str) -> usize {
        self.profile
            .mechanics
            .iter()
            .position(|mechanic| mechanic.instance_id == instance_id)
            .unwrap_or_else(|| panic!("test profile has no mechanic {instance_id}"))
    }

    #[cfg(test)]
    pub(crate) fn test_dynamic_buff(&self, instance_id: &str) -> DynamicBuffState {
        self.shared.dynamic_buffs[self.test_mechanic_index(instance_id)]
            .unwrap_or_else(|| panic!("test mechanic {instance_id} has no active buff"))
    }

    #[cfg(test)]
    pub(crate) fn test_dynamic_counter(&self, instance_id: &str) -> u32 {
        self.shared.dynamic_counters[self.test_mechanic_index(instance_id)]
    }

    #[cfg(test)]
    pub(crate) fn test_dynamic_ready_ms(&self, instance_id: &str) -> u64 {
        self.shared.dynamic_ready_ms[self.test_mechanic_index(instance_id)]
    }

    #[cfg(test)]
    pub(crate) fn test_dynamic_target_buff(
        &self,
        instance_id: &str,
        target_index: u32,
    ) -> DynamicBuffState {
        let slot = self.mechanic_target_slot(self.test_mechanic_index(instance_id), target_index);
        self.shared.dynamic_target_buffs[slot]
            .unwrap_or_else(|| panic!("test mechanic {instance_id} has no target buff"))
    }

    #[cfg(test)]
    pub(crate) fn test_amethyst_splinters(
        &self,
        instance_id: &str,
        target_index: u32,
    ) -> AmethystSplintersState {
        let slot = self.mechanic_target_slot(self.test_mechanic_index(instance_id), target_index);
        self.shared.amethyst_splinters[slot]
            .unwrap_or_else(|| panic!("test mechanic {instance_id} has no Amethyst state"))
    }

    #[cfg(test)]
    pub(crate) fn test_kindling(&self, instance_id: &str, target_index: u32) -> KindlingState {
        let slot = self.mechanic_target_slot(self.test_mechanic_index(instance_id), target_index);
        self.shared.kindling[slot]
            .unwrap_or_else(|| panic!("test mechanic {instance_id} has no Kindling state"))
    }

    #[cfg(test)]
    pub(crate) fn test_aurastone(&self, instance_id: &str) -> AurastoneState {
        self.shared.aurastones[self.test_mechanic_index(instance_id)]
            .unwrap_or_else(|| panic!("test mechanic {instance_id} has no Aurastone state"))
    }

    #[cfg(test)]
    pub(crate) fn test_uptime(&self, id: &str) -> f64 {
        if let Some(instance_id) = id.strip_prefix("proc:")
            && let Some(index) = self
                .profile
                .mechanics
                .iter()
                .position(|mechanic| mechanic.instance_id == instance_id)
        {
            return self.common.result.mechanic_proc_counts[index];
        }
        self.common.result.uptimes.get(id).copied().unwrap_or(0.0)
    }

    #[cfg(test)]
    pub(crate) fn test_proc_count(&self, source_id: &str) -> u64 {
        let index = self
            .profile
            .talent_proc_sources
            .get(source_id)
            .or_else(|| self.profile.mechanic_proc_sources.get(source_id))
            .copied()
            .or_else(|| {
                self.profile
                    .proc_sources
                    .iter()
                    .position(|source| source.source_id == source_id)
            });
        index
            .and_then(|index| self.common.result.proc_counts.get(index))
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn mechanic_target_slot(&self, mechanic_index: usize, target_index: u32) -> usize {
        mechanic_index * self.common.target_count as usize + target_index as usize
    }

    pub(crate) fn run(self) -> Result<IterationResult, SimulationError> {
        self.run_observed(None)
    }
    pub(crate) fn run_observed(
        mut self,
        capture: Option<crate::TraceOptions>,
    ) -> Result<IterationResult, SimulationError> {
        while self.common.now_ms < ENCOUNTER_DURATION_MS {
            if !self.common.execution.check_cancelled() {
                break;
            }
            self.process_events_through(self.common.now_ms);
            if self.common.execution.failed() {
                break;
            }
            let action = if let Some(capture) = &capture {
                if self.common.now_ms <= capture.until_ms
                    && self.common.result.trace.len() < capture.max_decisions
                {
                    let state = self.trace_state();
                    let mut rules = Vec::new();
                    let action = self.choose_action_observed(Some(&mut rules));
                    self.common.result.trace.push(crate::TraceDecision {
                        time_ms: self.common.now_ms,
                        rules,
                        state,
                        selected_ability: action.map(|i| self.profile.abilities[i].id.clone()),
                    });
                    action
                } else {
                    self.common.result.trace_truncated = true;
                    self.choose_action()
                }
            } else {
                self.choose_action()
            };
            if self.common.execution.failed() {
                break;
            }
            let Some(index) = action else {
                let next = self.next_interesting_time();
                self.process_events_through(next);
                self.common.now_ms = next;
                continue;
            };
            if !self.common.execution.record_action(self.common.now_ms) {
                break;
            }
            self.prepare_cast(index);
        }
        if let Some(error) = self.common.execution.error() {
            return Err(error);
        }
        self.process_events_through(ENCOUNTER_DURATION_MS);
        if let Some(error) = self.common.execution.error() {
            return Err(error);
        }
        if !self.common.execution.charge_usize(self.common.dots.len())
            || !self
                .common
                .execution
                .charge_usize(self.shared.dynamic_buffs.len())
            || !self
                .common
                .execution
                .charge_usize(self.common.fixed_buff_uptimes.len())
        {
            return Err(self
                .common
                .execution
                .error()
                .expect("failed work charge records an error"));
        }
        for ((target, kind), dot) in self.common.dots.clone() {
            self.record_dot_uptime(target, kind, &dot);
        }
        if let Some(started_ms) = self.shared.heroism_started_ms {
            let active = self
                .shared
                .heroism_until
                .min(ENCOUNTER_DURATION_MS)
                .saturating_sub(started_ms.min(ENCOUNTER_DURATION_MS));
            *self
                .common
                .result
                .uptimes
                .entry("buff:spirit-of-heroism".into())
                .or_default() += active as f64 / ENCOUNTER_DURATION_MS as f64;
        }
        for (index, buff) in self.shared.dynamic_buffs.iter().enumerate() {
            let Some(buff) = buff else {
                continue;
            };
            let active = buff
                .until_ms
                .min(ENCOUNTER_DURATION_MS)
                .saturating_sub(buff.started_ms.min(ENCOUNTER_DURATION_MS));
            self.common.result.mechanic_buff_uptimes[index] +=
                active as f64 / ENCOUNTER_DURATION_MS as f64;
        }
        for (id, buff) in &self.common.fixed_buff_uptimes {
            *self
                .common
                .result
                .uptimes
                .entry(format!("buff:{id}"))
                .or_default() +=
                buff.total_active_ms(ENCOUNTER_DURATION_MS) as f64 / ENCOUNTER_DURATION_MS as f64;
        }
        self.common.result.work_units = self.common.execution.work_units();
        #[cfg(feature = "strict-work-budget")]
        {
            self.common.result.work_breakdown = self.common.execution.work_breakdown();
        }
        Ok(self.common.result)
    }

    pub(crate) fn next_interesting_time(&self) -> u64 {
        let mut next = ENCOUNTER_DURATION_MS;
        let mut consider = |time: u64| {
            if time > self.common.now_ms {
                next = next.min(time);
            }
        };
        if !self
            .common
            .execution
            .charge_usize(self.common.cooldowns.len())
        {
            return next;
        }
        for (kind, cooldown) in &self.common.cooldowns {
            if cooldown.used_charges > 0 && cooldown.remaining_ms > 0.0 {
                let rate = self.cooldown_recovery_rate(*kind);
                consider(
                    self.common
                        .now_ms
                        .saturating_add((cooldown.remaining_ms / rate).ceil().max(1.0) as u64),
                );
            }
        }
        if let Some(event) = self.common.queue.peek() {
            consider(event.0.at_ms);
        }
        if !self.common.execution.charge_usize(self.common.dots.len()) {
            return next;
        }
        for dot in self.common.dots.values() {
            consider(dot.expires_ms);
        }
        for time in [
            self.shared.heroism_until,
            self.shared.weapon_channel_cooldown_recovery_until,
            self.shared.weapon_charge_buff_until,
            self.shared.weapon_critical_buff_until,
        ] {
            consider(time);
        }
        match &self.hero {
            HeroState::Ardeos(state) => {
                consider(state.wildfire_until);
                consider(state.apocalyptic_surge_until);
            }
            HeroState::Rime(state) => {
                for time in [
                    state.ice_blitz_until,
                    state.winters_blessing_until,
                    state.flight_of_the_navir_until,
                    state.wrath_of_winter_until,
                    state.bursting_until,
                    state.icy_flow_until,
                    state.soulfrost_torrent_until,
                    state.frostweavers_wrath_until,
                ] {
                    consider(time);
                }
            }
            HeroState::Tariq(state) => {
                for time in [
                    state.thunder_call_until,
                    state.focused_wrath_until,
                    state.raging_tempest_until,
                    state.far_beyond_driven_until,
                    state.kill_em_all_until,
                    state.square_hammer_until,
                    state.square_hammer_expertise_until,
                ] {
                    consider(time);
                }
            }
            HeroState::Elarion(state) => {
                for time in [
                    state.celestial_impetus_until,
                    state.empowered_multishot_until,
                    state.skystriders_grace_until,
                    state.event_horizon_until,
                    state.skystriders_supremacy_until,
                    state.impending_heartseeker_until,
                    state.resurgent_winds_until,
                ] {
                    consider(time);
                }
                if !self.common.execution.charge_usize(
                    state
                        .mark_until
                        .len()
                        .saturating_add(state.shimmer_until.len()),
                ) {
                    return next;
                }
                for time in state.mark_until.iter().chain(&state.shimmer_until) {
                    consider(*time);
                }
                if state.focus < self.profile.max_primary_resource {
                    let (period, gain) = self.elarion_focus_pulse();
                    if period > 0 && gain > 0.0 {
                        if !self
                            .common
                            .execution
                            .charge_usize(self.profile.abilities.len())
                        {
                            return next;
                        }
                        let mut smallest_deficit = self.profile.max_primary_resource - state.focus;
                        for ability in &self.profile.abilities {
                            let cost = ability
                                .parameters
                                .get(parameter_key!("focusCost"))
                                .unwrap_or(0.0);
                            if cost > state.focus {
                                smallest_deficit = smallest_deficit.min(cost - state.focus);
                            }
                        }
                        let pulses = (smallest_deficit / gain).ceil().max(1.0) as u64;
                        consider(
                            (self.common.now_ms / period)
                                .saturating_add(pulses)
                                .saturating_mul(period),
                        );
                    }
                }
            }
            HeroState::Mara(state) => {
                for time in [
                    state.maiden_of_death_until,
                    state.matriarch_macabre_until,
                    state.assassins_guile_until,
                    state.deadly_scheme_until,
                    state.feed_the_queen_until,
                    state.malevolence_arachnid_until,
                    state.malevolence_queen_until,
                    state.drenched_in_blood_until,
                ] {
                    consider(time);
                }
                consider(state.seething_poison_max_until);
            }
            HeroState::Gunde(state) => {
                for time in [
                    state.blood_feathers_until,
                    state.reign_in_blood_until,
                    state.bloodbound_spirit_until,
                    state.serrated_edge_until,
                    state.deaths_arc_until,
                    state.grim_harvest_until,
                    state.harvesters_toll_until,
                    state.crimson_strikes_until,
                    state.murder_of_crows_until,
                    state.massacre_until,
                    state.ancestral_instinct_until,
                    state.bloodbath_until,
                    state.carrion_onslaught_until,
                ] {
                    consider(time);
                }
                if !self
                    .common
                    .execution
                    .charge_usize(state.open_wounds_until.len())
                {
                    return next;
                }
                for time in &state.open_wounds_until {
                    consider(*time);
                }
            }
        }
        if !self
            .common
            .execution
            .charge_usize(self.profile.mechanic_indexes.cooldown_rate_boundaries.len())
        {
            return next;
        }
        for index in &self.profile.mechanic_indexes.cooldown_rate_boundaries {
            if let Some(buff) = &self.shared.dynamic_buffs[*index] {
                consider(buff.until_ms);
            }
        }
        let mut threshold_cursor = self.common.apl_fight_threshold_cursor.get();
        while self
            .common
            .apl_fight_threshold_times
            .get(threshold_cursor)
            .is_some_and(|time| *time <= self.common.now_ms)
        {
            if !self.common.execution.charge(1) {
                return next;
            }
            threshold_cursor += 1;
        }
        self.common.apl_fight_threshold_cursor.set(threshold_cursor);
        if let Some(time) = self.common.apl_fight_threshold_times.get(threshold_cursor) {
            consider(*time);
        }
        next.min(ENCOUNTER_DURATION_MS)
    }

    pub(crate) fn push_event(&mut self, at_ms: u64, kind: impl Into<EventKind>) {
        if self.common.queue.len() >= MAX_PENDING_EVENTS {
            self.common.execution.fail(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation queued too many simultaneous events",
            ));
            return;
        }
        if !self.common.execution.charge(1) {
            return;
        }
        self.common.sequence = self.common.sequence.wrapping_add(1);
        self.common.queue.push(Reverse(Event {
            at_ms,
            sequence: self.common.sequence,
            kind: kind.into(),
        }));
    }

    #[track_caller]
    pub(crate) fn charge_work_units(&self, units: u64) -> bool {
        match usize::try_from(units) {
            Ok(units) => self.common.execution.charge_usize(units),
            Err(_) => {
                self.common.execution.fail_limit();
                false
            }
        }
    }

    #[track_caller]
    pub(crate) fn charge_work_product(&self, left: u64, right: u64) -> bool {
        match left.checked_mul(right) {
            Some(units) => self.charge_work_units(units),
            None => {
                self.common.execution.fail_limit();
                false
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DamageOutcome {
    pub(crate) damage: f64,
    pub(crate) critical: bool,
}
