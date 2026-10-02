use crate::*;
use crate::{DamageContext, Iteration};

#[derive(Debug, Clone)]
pub(crate) enum GundeEvent {
    DeathsArcReset,
    SwingTimer {
        generation: u64,
    },
    FeatherPickup {
        count: u32,
    },
    BloodcrazePulse {
        generation: u64,
        stacks: u32,
        context: DamageContext,
    },
    CarrionFeatherPulse {
        generation: u64,
        remaining: u32,
    },
}

#[derive(Debug)]
pub(crate) struct GundeState {
    pub(crate) swing_remaining_ms: f64,
    pub(crate) swing_updated_ms: u64,
    pub(crate) swing_rate: f64,
    pub(crate) swing_generation: u64,
    pub(crate) swing_next_ms: Option<u64>,
    pub(crate) auto_attacking: bool,
    pub(crate) auto_blocked_until: u64,

    pub(crate) spirit_proc_activations: std::collections::VecDeque<u64>,
    pub(crate) blood_feathers: u32,
    pub(crate) blood_feathers_until: u64,
    pub(crate) ground_feathers: u32,
    pub(crate) reign_in_blood_until: u64,
    pub(crate) bloodbound_spirit_until: u64,
    pub(crate) serrated_edge_until: u64,
    pub(crate) deaths_arc_until: u64,
    pub(crate) grim_harvest_until: u64,
    pub(crate) harvesters_toll_until: u64,
    pub(crate) crimson_strikes_until: u64,
    pub(crate) murder_of_crows_stacks: u32,
    pub(crate) murder_of_crows_until: u64,
    pub(crate) massacre_stacks: u32,
    pub(crate) massacre_until: u64,
    pub(crate) ancestral_instinct_until: u64,
    pub(crate) bloodbath_until: u64,
    pub(crate) carrion_onslaught_until: u64,
    pub(crate) carrion_damage_multiplier: f64,
    pub(crate) carrion_pending_feathers: u32,
    pub(crate) carrion_generation: u64,
    pub(crate) bloodcraze_generation: u64,
    pub(crate) open_wounds_until: Vec<u64>,
}

impl Iteration<'_> {
    pub(crate) fn handle_gunde_event(&mut self, event: GundeEvent) {
        match event {
            GundeEvent::DeathsArcReset => {
                self.restore_ability_charge(DpsAbilityKind::BloodArc);
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent1") {
                    self.hero.gunde_mut().deaths_arc_until = self
                        .common
                        .now_ms
                        .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                    self.activate_fixed_buff(
                        AplBuff::DeathsArc,
                        self.hero.gunde().deaths_arc_until,
                    );
                }
            }
            GundeEvent::SwingTimer { generation } => self.gunde_swing_tick(generation),
            GundeEvent::FeatherPickup { count } => self.collect_gunde_feathers(count),
            GundeEvent::BloodcrazePulse {
                generation,
                stacks,
                context,
            } => self.pulse_gunde_bloodcraze(generation, stacks, context),
            GundeEvent::CarrionFeatherPulse {
                generation,
                remaining,
            } => self.pulse_gunde_carrion_feathers(generation, remaining),
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn collect_gunde_feathers(&mut self, count: u32) {
        if self.profile.contract.hero != HeroIdentity::Gunde || count == 0 {
            return;
        }
        let collected = count.min(self.hero.gunde().ground_feathers);
        let maximum = self
            .ability(DpsAbilityKind::OwedInBlood)
            .map(|ability| ability_u32_rounded(ability, parameter_key!("maximumBloodFeathers")))
            .unwrap_or(self.profile.max_primary_resource.round().max(0.0) as u32);
        let now_ms = self.common.now_ms;
        let duration = self
            .ability(DpsAbilityKind::OwedInBlood)
            .map(|ability| {
                ability_seconds_parameter(ability, parameter_key!("bloodFeatherDurationSeconds"))
            })
            .unwrap_or(0);
        let state = self.hero.gunde_mut();
        if now_ms >= state.blood_feathers_until {
            state.blood_feathers = 0;
        }
        state.ground_feathers -= collected;
        state.blood_feathers = state.blood_feathers.saturating_add(collected).min(maximum);
        if collected == 0 {
            return;
        }
        state.blood_feathers_until = now_ms.saturating_add(duration);
        if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent10") {
            let reduction = u64::from(collected)
                .saturating_mul(ms_param(talent, parameter_key!("cooldownReductionSeconds")));
            self.reduce_cooldown(DpsAbilityKind::Rupture, reduction);
        }
        if self
            .common
            .selected_talents
            .contains_key("gunde-talent-id-talent2")
            && let Some(ability) = self.ability(DpsAbilityKind::RavensPrecision).cloned()
        {
            let scaler = self.target_count_damage_scaler(&ability);
            if !self.charge_work_product(
                u64::from(collected),
                u64::from(self.automatic_target_count(&ability)),
            ) {
                return;
            }
            for _ in 0..collected {
                let context = DamageContext::NONE
                    .with_snapshot(self.capture_damage_source_snapshot(ability.kind));
                for target_index in 0..self.automatic_target_count(&ability) {
                    self.damage_hit(
                        &ability,
                        ability.power_coefficient * self.profile.power * scaler,
                        0.0,
                        true,
                        target_index,
                        context,
                    );
                }
            }
        }
    }

    pub(crate) fn pulse_gunde_bloodcraze(
        &mut self,
        generation: u64,
        stacks: u32,
        context: DamageContext,
    ) {
        if generation != self.hero.gunde().bloodcraze_generation {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::Bloodcraze).cloned() else {
            return;
        };
        let talent = self
            .common
            .selected_talents
            .get("gunde-talent-id-talent13")
            .expect("Bloodcraze events require the selected talent");
        let damage_multiplier =
            1.0 + f64::from(stacks) * param(talent, parameter_key!("damageIncreasePerStack"));
        let context = context
            .as_proc()
            .with_snapshot(self.capture_damage_source_snapshot(ability.kind));
        let scaler = self.target_count_damage_scaler(&ability);
        if !self.charge_work_units(u64::from(self.automatic_target_count(&ability))) {
            return;
        }
        for target_index in 0..self.automatic_target_count(&ability) {
            self.damage_hit(
                &ability,
                ability.power_coefficient * self.profile.power * damage_multiplier * scaler,
                0.0,
                true,
                target_index,
                context.as_proc(),
            );
        }
    }

    pub(crate) fn pulse_gunde_carrion_feathers(&mut self, generation: u64, remaining: u32) {
        if generation != self.hero.gunde().carrion_generation || remaining == 0 {
            return;
        }
        let per_pulse = self
            .gunde_mechanic_param(parameter_key!("carrionFeathersPerPulse"))
            .unwrap_or(0.0)
            .round()
            .max(0.0) as u32;
        let spawned = remaining.min(per_pulse);
        self.spawn_gunde_feathers(spawned);
        let remaining = remaining.saturating_sub(spawned);
        if remaining > 0 {
            let period = self
                .gunde_mechanic_param(parameter_key!("carrionPulsePeriodSeconds"))
                .map(|seconds| (seconds * 1_000.0).round().max(1.0) as u64)
                .unwrap_or(2_000);
            self.push_event(
                self.common.now_ms.saturating_add(period),
                GundeEvent::CarrionFeatherPulse {
                    generation,
                    remaining,
                },
            );
        }
    }

    pub(crate) fn gunde_mechanic_param(&self, key: ParameterKey) -> Option<f64> {
        [
            self.profile.mechanic_indexes.gunde_bleeding_hearts,
            self.profile.mechanic_indexes.gunde_bloodsoaked_cleaver,
            self.profile.mechanic_indexes.gunde_carrion_onslaught,
        ]
        .into_iter()
        .flatten()
        .find_map(|index| self.profile.mechanics[index].parameters.get(key))
    }

    pub(crate) fn gunde_blood_feathers(&self) -> u32 {
        if self.common.now_ms < self.hero.gunde().blood_feathers_until {
            self.hero.gunde().blood_feathers
        } else {
            0
        }
    }

    pub(crate) fn try_gunde_spirit_refund(&mut self, context: DamageContext) {
        let Some(activation) = context.source_cast else {
            return;
        };
        let seen = &mut self.hero.gunde_mut().spirit_proc_activations;
        if seen.contains(&activation) {
            return;
        }
        if seen.len() == 100 {
            seen.pop_front();
        }
        seen.push_back(activation);
        let stacks = if self.common.now_ms < self.hero.gunde().murder_of_crows_until {
            self.hero.gunde().murder_of_crows_stacks
        } else {
            0
        };
        // Gunde uses the shared random stream. Murder of Crows modifies the
        // native chance; unlike Rime's tag override it does not bypass the roll.
        let chance = spirit_refund_chance(self.effective_spirit(), 1.0, f64::from(stacks));
        if !self.roll_controlled_random_bool(SPIRIT_PROC_RANDOM_STREAM_TAG, chance) {
            return;
        }
        if stacks > 0 {
            self.hero.gunde_mut().murder_of_crows_stacks -= 1;
            if self.hero.gunde().murder_of_crows_stacks == 0 {
                self.hero.gunde_mut().murder_of_crows_until = 0;
                self.deactivate_fixed_buff(AplBuff::MurderOfCrows);
            }
        }
        self.record_spirit_refund_proc();
        let Some(rend) = self.ability(DpsAbilityKind::Rend).cloned() else {
            return;
        };
        self.shared.spirit = (self.shared.spirit
            + ability_param(&rend, parameter_key!("spiritRefundSpiritGain")))
        .min(self.profile.max_spirit);
        let count = ability_u32_rounded(&rend, parameter_key!("spiritRefundFeathers"));
        let delay = ability_seconds_parameter(&rend, parameter_key!("spiritRefundDelaySeconds"));
        self.push_event(
            self.common.now_ms.saturating_add(delay),
            GundeEvent::FeatherPickup { count },
        );
        self.hero.gunde_mut().ground_feathers = self
            .hero
            .gunde()
            .ground_feathers
            .saturating_add(count)
            .min(ability_u32_rounded(
                &rend,
                parameter_key!("maximumGroundFeathers"),
            ));
    }

    pub(crate) fn spawn_gunde_feathers(&mut self, count: u32) {
        if count == 0 {
            return;
        }
        let Some(rend) = self.ability(DpsAbilityKind::Rend).cloned() else {
            return;
        };
        let maximum = ability_u32_rounded(&rend, parameter_key!("maximumGroundFeathers"));
        let available = maximum.saturating_sub(self.hero.gunde().ground_feathers);
        let count = count.min(available);
        if count == 0 {
            return;
        }
        self.hero.gunde_mut().ground_feathers += count;
        self.push_event(
            self.common.now_ms.saturating_add(ability_seconds_parameter(
                &rend,
                parameter_key!("featherActivationDelaySeconds"),
            )),
            GundeEvent::FeatherPickup { count },
        );
    }
}

impl Iteration<'_> {
    pub(crate) fn commit_gunde_ability(&mut self, ability: &CompiledAbility) {
        if self.profile.contract.hero != HeroIdentity::Gunde {
            return;
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::DoubleStrike | DpsAbilityKind::HeartSplitter | DpsAbilityKind::Rupture
        ) {
            self.hero.gunde_mut().auto_attacking = true;
        }
        match ability.kind {
            DpsAbilityKind::ReignInBlood => {
                self.hero.gunde_mut().reign_in_blood_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::ReignInBlood,
                    self.hero.gunde().reign_in_blood_until,
                );
            }
            DpsAbilityKind::BloodboundSpirit => {
                self.hero.gunde_mut().bloodbound_spirit_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::BloodboundSpirit,
                    self.hero.gunde().bloodbound_spirit_until,
                );
            }
            DpsAbilityKind::Rupture => {
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent4") {
                    self.hero.gunde_mut().harvesters_toll_until = self
                        .common
                        .now_ms
                        .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                    self.activate_fixed_buff(
                        AplBuff::HarvestersToll,
                        self.hero.gunde().harvesters_toll_until,
                    );
                }
            }
            DpsAbilityKind::BloodArc => {
                let pending_feathers =
                    std::mem::take(&mut self.hero.gunde_mut().carrion_pending_feathers);
                if pending_feathers > 0 {
                    // The empowered Blood Arc removes the previous OrbDropper
                    // before applying its replacement. Granting the proc alone
                    // leaves the old emitter running.
                    self.hero.gunde_mut().carrion_generation =
                        self.hero.gunde().carrion_generation.wrapping_add(1);
                    self.push_event(
                        self.common.now_ms,
                        GundeEvent::CarrionFeatherPulse {
                            generation: self.hero.gunde().carrion_generation,
                            remaining: pending_feathers,
                        },
                    );
                }
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent1")
                    && self.roll_controlled_random_bool(
                        "RandomStream.Gunde.Talent.InstantAoeWithBuff.ChanceCooldownReset",
                        param(talent, parameter_key!("procChance")),
                    )
                {
                    self.record_talent_proc("gunde-talent-id-talent1");
                    // DelayUntilNextTick: use the next millisecond boundary
                    // under the maintained scheduling approximation.
                    self.push_event(
                        self.common.now_ms.saturating_add(1),
                        GundeEvent::DeathsArcReset,
                    );
                }
            }
            DpsAbilityKind::ReaversEdge => {
                if self
                    .common
                    .selected_talents
                    .contains_key("gunde-talent-id-talent2")
                {
                    self.collect_gunde_feathers(self.hero.gunde().ground_feathers);
                }
            }
            DpsAbilityKind::GrimCarve => {
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent18") {
                    self.hero.gunde_mut().bloodbath_until = self
                        .common
                        .now_ms
                        .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                    self.activate_fixed_buff(AplBuff::Bloodbath, self.hero.gunde().bloodbath_until);
                }
            }
            _ => {}
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::ReaversEdge | DpsAbilityKind::GrimCarve
        ) && let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent17")
            && self.roll_controlled_random_bool(
                "RandomStream.Gunde.Talent.AbilityToBuffChance",
                param(talent, parameter_key!("procChance")),
            )
        {
            self.record_talent_proc("gunde-talent-id-talent17");
            self.hero.gunde_mut().ancestral_instinct_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            self.activate_fixed_buff(
                AplBuff::AncestralInstinct,
                self.hero.gunde().ancestral_instinct_until,
            );
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::ReaversEdge
                | DpsAbilityKind::BloodArc
                | DpsAbilityKind::HeartSplitter
                | DpsAbilityKind::Rupture
        ) && let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent3")
            && self.roll_controlled_random_bool(
                "RandomStream.Gunde.Talent.TargetedAoeProjectile.Empowered",
                param(talent, parameter_key!("procChance")),
            )
        {
            self.record_talent_proc("gunde-talent-id-talent3");
            self.restore_ability_charge(DpsAbilityKind::GrimCarve);
            self.hero.gunde_mut().grim_harvest_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            self.activate_fixed_buff(AplBuff::GrimHarvest, self.hero.gunde().grim_harvest_until);
        }
    }

    pub(crate) fn finish_gunde_blood_arc(&mut self, ability: &CompiledAbility) {
        self.hero.gunde_mut().serrated_edge_until = self.common.now_ms.saturating_add(
            ability_seconds_parameter(ability, parameter_key!("serratedEdgeDurationSeconds")),
        );
        self.activate_fixed_buff(AplBuff::SerratedEdge, self.hero.gunde().serrated_edge_until);
        if self
            .common
            .selected_talents
            .contains_key("gunde-talent-id-talent5")
        {
            self.hero.gunde_mut().crimson_strikes_until = ENCOUNTER_DURATION_MS;
            self.activate_fixed_buff(
                AplBuff::CrimsonStrikes,
                self.hero.gunde().crimson_strikes_until,
            );
        }
    }

    pub(crate) fn finish_gunde_rupture(&mut self) {
        if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent8") {
            self.hero.gunde_mut().murder_of_crows_stacks =
                param_u32(talent, parameter_key!("stacks"));
            self.hero.gunde_mut().murder_of_crows_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            self.activate_fixed_buff(
                AplBuff::MurderOfCrows,
                self.hero.gunde().murder_of_crows_until,
            );
        }
    }

    pub(crate) fn resolve_gunde_impact(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        outcome: DamageOutcome,
        total_direct_damage: f64,
        impact_context: CastImpactContext,
        context: DamageContext,
    ) {
        if self.profile.contract.hero != HeroIdentity::Gunde {
            return;
        }
        if ability.kind == DpsAbilityKind::OwedInBlood && target_index == 0 {
            let stacks = self.gunde_blood_feathers();
            self.hero.gunde_mut().blood_feathers = 0;
            self.hero.gunde_mut().blood_feathers_until = 0;
            if stacks == 0 {
                return;
            }
            self.apply_gunde_rend(
                target_index,
                f64::from(stacks) * ability_param(ability, parameter_key!("rendDamagePerFeather")),
                context,
            );
            if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent13") {
                self.hero.gunde_mut().bloodcraze_generation =
                    self.hero.gunde().bloodcraze_generation.wrapping_add(1);
                let generation = self.hero.gunde().bloodcraze_generation;
                let period = ms_param(talent, parameter_key!("periodSeconds"));
                let pulses = (param(talent, parameter_key!("durationSeconds"))
                    / param(talent, parameter_key!("periodSeconds")).max(f64::EPSILON))
                .round()
                .max(0.0) as u32;
                if !self.charge_work_units(u64::from(pulses)) {
                    return;
                }
                for pulse in 1..=pulses {
                    self.push_event(
                        self.common
                            .now_ms
                            .saturating_add(period.saturating_mul(u64::from(pulse))),
                        GundeEvent::BloodcrazePulse {
                            generation,
                            stacks,
                            context,
                        },
                    );
                }
            }
            if let Some(base_multiplier) =
                self.gunde_mechanic_param(parameter_key!("carrionDamageMultiplier"))
            {
                let per_stack = self
                    .gunde_mechanic_param(parameter_key!("carrionAdditionalDamagePerFeather"))
                    .unwrap_or(0.0);
                let duration = self
                    .gunde_mechanic_param(parameter_key!("carrionDurationSeconds"))
                    .map(|seconds| (seconds * 1_000.0).round().max(0.0) as u64)
                    .unwrap_or(0);
                self.hero.gunde_mut().carrion_damage_multiplier =
                    base_multiplier + per_stack * f64::from(stacks);
                self.hero.gunde_mut().carrion_onslaught_until =
                    self.common.now_ms.saturating_add(duration);
                self.activate_fixed_buff(
                    AplBuff::CarrionOnslaught,
                    self.hero.gunde().carrion_onslaught_until,
                );
                let per_feather = self
                    .gunde_mechanic_param(parameter_key!("carrionStacksPerFeather"))
                    .unwrap_or(1.0)
                    .max(1.0);
                let feather_count = (f64::from(stacks) / per_feather).round().max(1.0) as u32;
                self.hero.gunde_mut().carrion_pending_feathers = feather_count;
            }
            return;
        }
        if ability.kind == DpsAbilityKind::Slaughter {
            let Some(rend) = self
                .common
                .dots
                .remove(&(target_index, DotKind::Ability(DpsAbilityKind::Rend)))
            else {
                return;
            };
            let remaining = self.remaining_derived_dot_damage(&rend);
            self.record_dot_uptime(target_index, DotKind::Ability(DpsAbilityKind::Rend), &rend);
            let open_wounds_active =
                self.common.now_ms < self.hero.gunde().open_wounds_until[target_index as usize];
            let open_wounds = if open_wounds_active {
                self.ability(DpsAbilityKind::Rupture)
                    .map(|rupture| {
                        ability_param(rupture, parameter_key!("openWoundsDamageMultiplier"))
                    })
                    .unwrap_or(1.0)
            } else {
                1.0
            };
            if open_wounds_active {
                self.hero.gunde_mut().open_wounds_until[target_index as usize] = 0;
                if target_index == 0 {
                    self.deactivate_fixed_buff(AplBuff::OpenWounds);
                }
            }
            if let Some(dot) = ability.dot {
                self.apply_damage_derived_dot(
                    target_index,
                    DotKind::Ability(DpsAbilityKind::Slaughter),
                    self.ability_damage_source(ability),
                    dot,
                    remaining
                        * ability_param(ability, parameter_key!("consumedRendDamageMultiplier"))
                        * open_wounds
                        * self.gunde_rend_frequency(),
                    context,
                );
            }
            if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent9") {
                if self.common.now_ms >= self.hero.gunde().massacre_until {
                    self.hero.gunde_mut().massacre_stacks = 0;
                }
                self.hero.gunde_mut().massacre_stacks = self
                    .hero
                    .gunde()
                    .massacre_stacks
                    .saturating_add(
                        (remaining
                            / (self.profile.power * self.effective_power_multiplier())
                                .max(f64::EPSILON))
                        .round()
                        .max(1.0) as u32,
                    )
                    .min(param_u32(talent, parameter_key!("maximumStacks")));
                self.hero.gunde_mut().massacre_until = self
                    .common
                    .now_ms
                    .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                self.activate_fixed_buff(AplBuff::Massacre, self.hero.gunde().massacre_until);
            }
            return;
        }
        if ability.kind == DpsAbilityKind::Rupture {
            self.hero.gunde_mut().open_wounds_until[target_index as usize] =
                self.common.now_ms.saturating_add(ability_seconds_parameter(
                    ability,
                    parameter_key!("openWoundsDurationSeconds"),
                ));
            if target_index == 0 {
                self.activate_fixed_buff(
                    AplBuff::OpenWounds,
                    self.hero.gunde().open_wounds_until[0],
                );
            }
        }
        if ability.kind == DpsAbilityKind::GrimCarve
            && target_index == 0
            && let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent15")
        {
            let reduction = ms_param(talent, parameter_key!("cooldownReductionPerSpinSeconds"))
                .saturating_mul(u64::from(if ability.hit_interval_ms > 0 {
                    1
                } else {
                    ability.direct_hits.max(1)
                }));
            for kind in [
                DpsAbilityKind::ReaversEdge,
                DpsAbilityKind::BloodArc,
                DpsAbilityKind::HeartSplitter,
                DpsAbilityKind::Rupture,
            ] {
                self.reduce_cooldown(kind, reduction);
            }
        }
        if outcome.damage <= 0.0 {
            return;
        }

        if gunde_applies_rend(ability.kind) {
            let mut transfer = ability_param(ability, parameter_key!("rendTransferFraction"))
                + impact_context.gunde_rend_transfer_bonus;
            if self.common.now_ms < self.hero.gunde().reign_in_blood_until {
                transfer += self
                    .ability(DpsAbilityKind::ReignInBlood)
                    .map(|reign| ability_param(reign, parameter_key!("rendTransferFraction")))
                    .unwrap_or(0.0);
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent11") {
                    transfer += param(talent, parameter_key!("addedRendTransferFraction"));
                }
            }
            self.apply_gunde_rend(target_index, total_direct_damage * transfer, context);
        }

        if ability.kind == DpsAbilityKind::HeartSplitter {
            let exsanguinate_damage = self.gunde_rend_remaining(target_index)
                * self.gunde_rend_frequency()
                * ability_param(ability, parameter_key!("exsanguinateFraction"));
            if exsanguinate_damage > 0.0
                && let Some(exsanguinate) = self.ability(DpsAbilityKind::Exsanguinate).cloned()
            {
                self.damage_unscaled_key(
                    Some(DpsAbilityKind::Exsanguinate),
                    self.ability_damage_source(&exsanguinate),
                    exsanguinate_damage,
                    target_index,
                    DamageProvenance::Proc,
                    context.as_proc(),
                );
                if outcome.critical
                    && self.common.target_count > 1
                    && let Some(talent) =
                        self.common.selected_talents.get("gunde-talent-id-talent14")
                    && let Some(explosion) = self.ability(DpsAbilityKind::Oathshatter).cloned()
                {
                    let scaler = multi_target_damage_falloff(
                        self.common.target_count - 1,
                        param(talent, parameter_key!("targetCountDamageScalingThreshold")),
                    );
                    let damage = exsanguinate_damage
                        * param(talent, parameter_key!("explosionDamageFraction"))
                        * scaler;
                    if !self.charge_work_units(u64::from(self.common.target_count - 1)) {
                        return;
                    }
                    for current_target in 0..self.common.target_count {
                        if current_target == target_index {
                            continue;
                        }
                        self.damage_unscaled_key(
                            Some(DpsAbilityKind::Oathshatter),
                            self.ability_damage_source(&explosion),
                            damage,
                            current_target,
                            DamageProvenance::Explosion,
                            context.as_proc(),
                        );
                    }
                }
            }
            if let Some(index) = self.profile.mechanic_indexes.gunde_bleeding_hearts
                && let mechanic = Arc::clone(&self.profile.mechanics[index])
                && let chance = mechanic_param(
                    &mechanic,
                    parameter_key!("heartSplitterAdditionalStrikeChance"),
                )
                && self.roll_controlled_random_bool(
                    "RandomStream.Gunde.Talent.HeavyMeleeDotBased.DoubleStrike",
                    chance,
                )
                && !impact_context.gunde_legendary_strike
                && let Some(index) = self
                    .common
                    .abilities_by_kind
                    .get(&DpsAbilityKind::HeartSplitter)
                    .copied()
            {
                self.record_dynamic_random_proc(&mechanic);
                let delay = self
                    .gunde_mechanic_param(parameter_key!(
                        "heartSplitterAdditionalStrikeDelaySeconds"
                    ))
                    .map(|seconds| {
                        (seconds * 1_000.0 / (1.0 + self.effective_haste()).max(0.05))
                            .round()
                            .max(0.0) as u64
                    })
                    .unwrap_or(0);
                self.hero.gunde_mut().auto_blocked_until = self.common.now_ms.saturating_add(delay);
                self.push_event(
                    self.common.now_ms.saturating_add(delay),
                    CoreEvent::ImpactBatch {
                        index,
                        impacts: vec![ImpactSpec {
                            single_hit: true,
                            damage_scale_bits: 1.0_f64.to_bits(),
                            target_index,
                        }],
                        context: CastImpactContext {
                            gunde_legendary_strike: true,
                            damage: DamageContext {
                                source_snapshot: None,
                                ..impact_context.damage
                            },
                            ..impact_context
                        },
                    },
                );
            }
        }
    }

    pub(crate) fn apply_gunde_rend(
        &mut self,
        target_index: u32,
        damage: f64,
        context: DamageContext,
    ) {
        if damage <= 0.0 {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::Rend).cloned() else {
            return;
        };
        let Some(mut dot) = ability.dot else {
            return;
        };
        let frequency = self.gunde_rend_frequency();
        // The manager divides each new contribution across its circular buckets.
        // Existing buckets and the current tick phase survive duration refresh.
        let count = (dot.duration_ms as f64 / (dot.period_ms as f64 / frequency))
            .round()
            .max(1.0) as usize;
        dot.period_ms = (dot.period_ms as f64 / frequency).round().max(1.0) as u64;
        let existing = self
            .common
            .dots
            .get_mut(&(target_index, DotKind::Ability(DpsAbilityKind::Rend)))
            .filter(|active| active.expires_ms >= self.common.now_ms)
            .and_then(|active| active.gunde_rend_buckets.take());
        let mut buckets = if let Some(buckets) = existing {
            // Reuse the ring. Updating numeric entries is collection maintenance,
            // charged at the engine's existing lightweight-loop rate.
            if !self.common.execution.charge_lightweight_loop(buckets.len()) {
                return;
            }
            buckets
        } else {
            // Guard allocation before constructing a user-sized ring.
            if !self.common.execution.charge_usize(count) {
                return;
            }
            std::collections::VecDeque::from(vec![0.0; count])
        };
        let share = damage / buckets.len() as f64;
        for bucket in &mut buckets {
            *bucket += share;
        }
        self.apply_damage_derived_dot(
            target_index,
            DotKind::Ability(DpsAbilityKind::Rend),
            self.ability_damage_source(&ability),
            dot,
            damage,
            context,
        );
        if let Some(active) = self
            .common
            .dots
            .get_mut(&(target_index, DotKind::Ability(DpsAbilityKind::Rend)))
        {
            active.derived_damage_per_tick = buckets.front().copied();
            active.gunde_rend_buckets = Some(buckets);
        }
    }

    pub(crate) fn gunde_rend_frequency(&self) -> f64 {
        self.common
            .selected_talents
            .get("gunde-talent-id-talent12")
            .map(|talent| param(talent, parameter_key!("tickRateMultiplier")))
            .unwrap_or(1.0)
            .max(0.05)
    }

    pub(crate) fn advance_gunde_rend_bucket(&mut self, target_index: u32) {
        let key = (target_index, DotKind::Ability(DpsAbilityKind::Rend));
        let Some(active) = self.common.dots.get_mut(&key) else {
            return;
        };
        let Some(buckets) = active.gunde_rend_buckets.as_mut() else {
            return;
        };
        buckets.pop_front();
        buckets.push_back(0.0);
        active.derived_damage_per_tick = buckets.front().copied();
        if buckets.front().is_none_or(|damage| *damage <= 0.0) {
            let removed = self.common.dots.remove(&key).unwrap();
            self.record_dot_uptime(target_index, key.1, &removed);
        }
    }

    pub(crate) fn remaining_derived_dot_damage(&self, dot: &DotState) -> f64 {
        if let Some(buckets) = &dot.gunde_rend_buckets {
            return buckets.iter().sum();
        }
        dot.derived_damage_per_tick.unwrap_or(0.0)
            * remaining_periodic_tick_equivalents(
                self.common.now_ms,
                dot.expires_ms,
                dot.last_tick_ms,
                dot.next_tick_ms,
                dot.model.period_ms,
            )
    }

    pub(crate) fn gunde_rend_remaining(&self, target_index: u32) -> f64 {
        self.common
            .dots
            .get(&(target_index, DotKind::Ability(DpsAbilityKind::Rend)))
            .filter(|dot| dot.expires_ms >= self.common.now_ms)
            .map(|dot| self.remaining_derived_dot_damage(dot))
            .unwrap_or(0.0)
    }
}

impl Iteration<'_> {
    pub(crate) fn hero_damage_scale(&self, ability_kind: Option<DpsAbilityKind>) -> f64 {
        let gunde = if let HeroState::Gunde(state) = &self.hero
            && self.common.now_ms < state.harvesters_toll_until
        {
            self.common
                .selected_talents
                .get("gunde-talent-id-talent4")
                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let crimson = if let HeroState::Gunde(state) = &self.hero
            && self.common.now_ms < state.crimson_strikes_until
            && ability_kind.is_some_and(|kind| {
                gunde_applies_rend(kind) && kind != DpsAbilityKind::BloodboundSpirit
            }) {
            self.common
                .selected_talents
                .get("gunde-talent-id-talent5")
                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0)
        } else {
            1.0
        };
        // Toll and Carnage share the native Multiplicitive bucket.
        let carnage = if self.profile.contract.hero == HeroIdentity::Gunde
            && ability_kind == Some(DpsAbilityKind::GrimCarve)
        {
            self.common
                .selected_talents
                .get("gunde-talent-id-talent15")
                .map(|talent| param(talent, parameter_key!("damageMultiplier")) - 1.0)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let bloodbound = if let HeroState::Gunde(state) = &self.hero
            && self.common.now_ms < state.bloodbound_spirit_until
            && matches!(
                ability_kind,
                Some(DpsAbilityKind::HeartSplitter | DpsAbilityKind::GrimCarve)
            ) {
            self.ability(DpsAbilityKind::BloodboundSpirit)
                .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0)
        } else {
            1.0
        };
        self.elarion_horizon_damage_multiplier() * (gunde + carnage) * crimson * bloodbound
    }
}

impl Iteration<'_> {
    pub(crate) fn roll_gunde_rend_feathers(&mut self) {
        let active_rends = self
            .common
            .dots
            .iter()
            .filter(|((_, kind), dot)| {
                *kind == DotKind::Ability(DpsAbilityKind::Rend)
                    && dot.expires_ms >= self.common.now_ms
            })
            .count();
        if active_rends == 0 {
            return;
        }
        if active_rends == 1
            && let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent12")
            && self.roll_controlled_random_bool(
                "RandomStream.Gunde.Talent.OrbDropper.IncreasedDropAmount.Proc",
                param(talent, parameter_key!("extraFeathersChance")),
            )
        {
            let count = param_u32(talent, parameter_key!("extraFeathersAmount"));
            self.record_talent_proc("gunde-talent-id-talent12");
            self.spawn_gunde_feathers(count);
            return;
        }
        let Some(rend) = self.ability(DpsAbilityKind::Rend) else {
            return;
        };
        let key = match active_rends {
            1 => parameter_key!("featherChance1"),
            2 => parameter_key!("featherChance2"),
            3 => parameter_key!("featherChance3"),
            4 => parameter_key!("featherChance4"),
            5 => parameter_key!("featherChance5"),
            6 => parameter_key!("featherChance6"),
            _ => parameter_key!("featherChance7"),
        };
        if self.roll_controlled_random_bool(
            "RandomStream.Gunde.DotTransfer.OrbDropper.PeriodSpawnChance",
            ability_param(rend, key),
        ) {
            self.record_ability_proc(DpsAbilityKind::Rend);
            self.spawn_gunde_feathers(1);
        }
    }
}
