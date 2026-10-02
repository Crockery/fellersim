use crate::*;
use crate::{DamageContext, DpsAbilityKind, Iteration};

#[derive(Debug, Clone)]
pub(crate) enum RimeEvent {
    BlessingHeal {
        generation: u64,
    },
    BurstingPulse {
        generation: u64,
        context: DamageContext,
    },
    WrathVolley {
        generation: u64,
        context: DamageContext,
    },
    BirdImpact {
        generation: u64,
        bird: usize,
        context: DamageContext,
    },
    TriggeredDamage {
        source: RimeTriggeredDamageSource,
        coefficient_bits: u64,
        bonus_crit_bits: u64,
        target_index: u32,
        context: DamageContext,
    },
    SpenderPulse {
        source: RimeTriggeredDamageSource,
        coefficient_bits: u64,
        bonus_crit_bits: u64,
        targets: u32,
        context: DamageContext,
    },
    GlacialAssaultExplosion {
        damage_bits: u64,
        context: DamageContext,
    },
    OrbRefund {
        orbs: u32,
    },
    BurstingTriggered {
        context: DamageContext,
    },
    CoalescingSecondary {
        coefficient_bits: u64,
        primary: u32,
        context: DamageContext,
    },
    CoalescingExpire {
        generation: u64,
        target_index: u32,
        context: DamageContext,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RimeTriggeredDamageSource {
    AnimaSpike,
    FrostSwallow,
    IceComet,
    TalonStrike,
    GlacialAssaultTalonStrike,
    RisingTalons,
}

impl RimeTriggeredDamageSource {
    pub(crate) fn ability_kind(self) -> DpsAbilityKind {
        match self {
            Self::AnimaSpike => DpsAbilityKind::AnimaSpike,
            Self::FrostSwallow => DpsAbilityKind::FrostSwallow,
            Self::IceComet | Self::RisingTalons => DpsAbilityKind::IceComet,
            Self::TalonStrike | Self::GlacialAssaultTalonStrike => DpsAbilityKind::GlacialBlast,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RimeState {
    pub(crate) anima: f64,
    pub(crate) winter_orbs: u32,
    pub(crate) ice_blitz_until: u64,
    pub(crate) winters_blessing_until: u64,
    pub(crate) blessing_heal_generation: u64,
    pub(crate) blessing_heal_pending: f64,
    pub(crate) blessing_heal_scheduled: bool,
    pub(crate) wrath_of_winter_until: u64,
    pub(crate) flight_of_the_navir_until: u64,
    pub(crate) glacial_assault_stacks: u32,
    pub(crate) icy_flow_casting_haste: f64,
    pub(crate) icy_flow_stacks: u32,
    pub(crate) icy_flow_until: u64,
    pub(crate) soulfrost_torrent_until: u64,
    pub(crate) frostweavers_wrath_until: u64,
    pub(crate) frostweavers_wrath_stacks: u32,
    pub(crate) harrowing_ice_stacks: u32,
    pub(crate) harrowing_ice_until: u64,
    pub(crate) bursting_generation: u64,
    pub(crate) bursting_until: u64,
    pub(crate) bursting_instances: std::collections::BTreeMap<u64, (u64, u64)>,
    pub(crate) wrath_generation: u64,
    pub(crate) wrath_period_ms: u64,
    pub(crate) flight_generation: u64,
    pub(crate) flight_birds: Vec<Option<u32>>,
    pub(crate) navir_free_until: u64,
    pub(crate) navir_free_cold_snaps: u32,
    pub(crate) undulating_spirit_stacks: u32,
    pub(crate) undulating_spirit_until: u64,
    pub(crate) coalescing_stacks: Vec<u32>,
    pub(crate) coalescing_generations: Vec<u64>,
    pub(crate) frostwyrm_stacks: u32,
    pub(crate) frostwyrm_until: u64,
}

impl Iteration<'_> {
    pub(crate) fn handle_rime_event(&mut self, event: RimeEvent) {
        match event {
            RimeEvent::BlessingHeal { generation } => self.flush_rime_blessing_heal(generation),
            RimeEvent::BurstingPulse {
                generation,
                context,
            } => self.rime_bursting_pulse(generation, context),
            RimeEvent::WrathVolley {
                generation,
                context,
            } => self.rime_wrath_volley(generation, context),
            RimeEvent::BirdImpact {
                generation,
                bird,
                context,
            } => {
                self.rime_bird_impact(generation, bird, context);
            }
            RimeEvent::TriggeredDamage {
                source,
                coefficient_bits,
                bonus_crit_bits,
                target_index,
                context,
            } => {
                self.rime_triggered_damage(
                    source,
                    f64::from_bits(coefficient_bits),
                    f64::from_bits(bonus_crit_bits),
                    target_index,
                    context,
                );
            }
            RimeEvent::GlacialAssaultExplosion {
                damage_bits,
                context,
            } => {
                self.apply_rime_glacial_assault_explosion(f64::from_bits(damage_bits), context);
            }
            RimeEvent::OrbRefund { orbs } => {
                // The delayed shared modifier restores the aggregate delta in
                // one attribute change, unlike individual helper orb grants.
                let previous = self.hero.rime().winter_orbs;
                self.hero.rime_mut().winter_orbs = previous
                    .saturating_add(orbs)
                    .min(self.profile.max_secondary_resource);
                if self.hero.rime().winter_orbs > previous {
                    self.roll_rime_frostweaver();
                } else if orbs > 0 && previous == self.profile.max_secondary_resource {
                    self.reduce_rime_major_cooldowns(1);
                }
            }
            RimeEvent::SpenderPulse {
                source,
                coefficient_bits,
                bonus_crit_bits,
                targets,
                context,
            } => {
                // The actor writes its set-by-caller critical bonus once before
                // applying the same spec to the whole pulse's target list.
                let bonus = f64::from_bits(bonus_crit_bits) + self.rime_frostweaver_bonus();
                for target in 0..targets {
                    self.rime_triggered_damage(
                        source,
                        f64::from_bits(coefficient_bits),
                        bonus,
                        target,
                        context,
                    );
                    if target == 0 {
                        self.consume_rime_frostweaver();
                    }
                }
            }
            RimeEvent::BurstingTriggered { context } => self.rime_bursting_damage_pulse(context),
            RimeEvent::CoalescingSecondary {
                coefficient_bits,
                primary,
                context,
            } => {
                for target in 0..self.common.target_count {
                    if target != primary {
                        self.damage_coalescing_frost(
                            f64::from_bits(coefficient_bits),
                            target,
                            context,
                        );
                    }
                }
            }
            RimeEvent::CoalescingExpire {
                generation,
                target_index,
                context,
            } => self.expire_coalescing_frost(generation, target_index, context),
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn rime_legendary_mechanic(
        &self,
        kind: LegendaryHeroSourceKind,
    ) -> Option<Arc<CompiledMechanic>> {
        let index = match kind {
            LegendaryHeroSourceKind::RimeStartingResources => {
                self.profile.mechanic_indexes.rime_starting_resources
            }
            LegendaryHeroSourceKind::Frostwyrm => self.profile.mechanic_indexes.frostwyrm,
            LegendaryHeroSourceKind::BurstingIce => self.profile.mechanic_indexes.bursting_ice,
            _ => None,
        }?;
        Some(Arc::clone(&self.profile.mechanics[index]))
    }

    pub(crate) fn activate_fixed_buff(&mut self, buff: AplBuff, until_ms: u64) {
        let Some(id) = fixed_buff_uptime_id(buff) else {
            return;
        };
        self.common
            .fixed_buff_uptimes
            .entry(id)
            .or_default()
            .activate(self.common.now_ms, until_ms);
    }

    pub(crate) fn deactivate_fixed_buff(&mut self, buff: AplBuff) {
        let Some(id) = fixed_buff_uptime_id(buff) else {
            return;
        };
        if let Some(state) = self.common.fixed_buff_uptimes.get_mut(id) {
            state.deactivate(self.common.now_ms);
        }
    }

    pub(crate) fn apply_rime_ability_buff(&mut self, kind: DpsAbilityKind) {
        let Some(ability) = self.ability(kind).cloned() else {
            return;
        };
        let until = self
            .common
            .now_ms
            .saturating_add(ability.effect_duration_ms);
        match kind {
            DpsAbilityKind::IceBlitz => {
                self.hero.rime_mut().ice_blitz_until = until;
                self.activate_fixed_buff(AplBuff::IceBlitz, until);
            }
            DpsAbilityKind::WintersBlessing => {
                self.hero.rime_mut().blessing_heal_generation =
                    self.hero.rime().blessing_heal_generation.wrapping_add(1);
                self.hero.rime_mut().blessing_heal_pending = 0.0;
                self.hero.rime_mut().blessing_heal_scheduled = false;
                self.hero.rime_mut().winters_blessing_until = until;
                self.activate_fixed_buff(AplBuff::WintersBlessing, until);
                if let Some(mechanic) =
                    self.rime_legendary_mechanic(LegendaryHeroSourceKind::RimeStartingResources)
                {
                    self.hero.rime_mut().undulating_spirit_stacks =
                        mechanic_param(&mechanic, parameter_key!("wintersBlessingCharges"))
                            .round()
                            .max(0.0) as u32;
                    self.hero.rime_mut().undulating_spirit_until =
                        self.common.now_ms.saturating_add(seconds_parameter(
                            &mechanic,
                            parameter_key!("undulatingSpiritDurationSeconds"),
                        ));
                }
            }
            DpsAbilityKind::FlightOfTheNavir => {
                let bird_count = ability_u32_rounded(&ability, parameter_key!("projectileCount"));
                if !self.charge_work_units(u64::from(bird_count)) {
                    return;
                }
                self.hero.rime_mut().flight_of_the_navir_until = until;
                self.hero.rime_mut().flight_generation =
                    self.hero.rime().flight_generation.wrapping_add(1);
                self.hero.rime_mut().flight_birds = vec![
                    None;
                    ability_u32_rounded(&ability, parameter_key!("projectileCount"))
                        as usize
                ];
                self.activate_fixed_buff(AplBuff::FlightOfTheNavir, until);
                if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent10") {
                    let charges = param_u32(talent, parameter_key!("freeColdSnapCharges"));
                    let until = self
                        .common
                        .now_ms
                        .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                    // The source-aggregated proc caps at two and refreshes its duration.
                    self.hero.rime_mut().navir_free_cold_snaps = charges;
                    self.hero.rime_mut().navir_free_until = until;
                }
            }
            _ => {}
        }
    }

    pub(crate) fn add_rime_anima(&mut self, amount: f64, context: DamageContext) {
        self.add_rime_anima_at(amount, 0, context);
    }

    pub(crate) fn add_rime_anima_at(&mut self, amount: f64, target: u32, context: DamageContext) {
        let points = amount.round().max(0.0) as u32;
        if !self.charge_work_units(u64::from(points)) {
            return;
        }
        for _ in 0..points {
            self.hero.rime_mut().anima += 1.0;
            if self.common.now_ms < self.hero.rime().ice_blitz_until
                && let Some(spikes_per_anima) = self
                    .common
                    .selected_talents
                    .get("rime-talent-id-talent13")
                    .map(|talent| param_u32(talent, parameter_key!("spikesPerAnima")))
            {
                if !self.charge_work_units(u64::from(spikes_per_anima)) {
                    return;
                }
                for _ in 0..spikes_per_anima {
                    self.schedule_rime_projectile(
                        RimeTriggeredDamageSource::AnimaSpike,
                        self.rime_anima_spike_coefficient(),
                        0.0,
                        target,
                        context,
                        500,
                    );
                }
            }
            if self.hero.rime().anima + f64::EPSILON >= self.profile.max_primary_resource {
                self.hero.rime_mut().anima = 0.0;
                let projectile_count = self
                    .ability(DpsAbilityKind::AnimaSpike)
                    .map(|ability| ability_u32_rounded(ability, parameter_key!("projectileCount")))
                    .unwrap_or(3)
                    .max(1);
                self.fire_rime_anima_volley(projectile_count, target, context);
                self.add_rime_orbs(1);
            }
        }
    }

    pub(crate) fn fire_rime_anima_volley(
        &mut self,
        count: u32,
        primary: u32,
        context: DamageContext,
    ) {
        if !self.charge_work_units(u64::from(self.common.target_count) + u64::from(count)) {
            return;
        }
        let mut targets: Vec<u32> = (0..self.common.target_count).collect();
        // The Blueprint shuffles the whole array, including MainTarget, before
        // truncating. Padding afterwards repeats only the selected actor.
        self.common.rng.shuffle(&mut targets);
        targets.resize(count as usize, primary);
        for (projectile, target) in targets.into_iter().enumerate() {
            self.schedule_rime_projectile(
                RimeTriggeredDamageSource::AnimaSpike,
                self.rime_anima_spike_coefficient(),
                0.0,
                target,
                context,
                500 + projectile as u64 * 150,
            );
        }
    }

    pub(crate) fn rime_anima_spike_coefficient(&self) -> f64 {
        self.ability(DpsAbilityKind::AnimaSpike)
            .map(|ability| ability.power_coefficient)
            .unwrap_or(0.469)
    }

    pub(crate) fn add_rime_orbs(&mut self, amount: u32) {
        if !self.charge_work_units(u64::from(amount)) {
            return;
        }
        for _ in 0..amount {
            let overflowed = self.hero.rime().winter_orbs >= self.profile.max_secondary_resource;
            if self.hero.rime().winter_orbs < self.profile.max_secondary_resource {
                self.hero.rime_mut().winter_orbs += 1;
            }
            if overflowed {
                self.reduce_rime_major_cooldowns(1);
                continue;
            }
            self.roll_rime_frostweaver();
        }
    }

    fn roll_rime_frostweaver(&mut self) {
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent18") {
            let chance = param(talent, parameter_key!("procChance"));
            let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
            let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
            if self.roll_controlled_random_bool(
                "RandomStream.Rime.Talent.ChanceOnOrbGainNextSpenderCritIncrease",
                chance,
            ) {
                self.record_talent_proc("rime-talent-id-talent18");
                let previous = self.buff_stacks(AplBuff::FrostweaversWrath);
                self.hero.rime_mut().frostweavers_wrath_stacks =
                    previous.saturating_add(1).min(maximum_stacks);
                self.hero.rime_mut().frostweavers_wrath_until =
                    self.common.now_ms.saturating_add(duration_ms);
                self.activate_fixed_buff(
                    AplBuff::FrostweaversWrath,
                    self.hero.rime().frostweavers_wrath_until,
                );
            }
        }
    }

    pub(crate) fn rime_frostweaver_bonus(&self) -> f64 {
        if self.buff_stacks(AplBuff::FrostweaversWrath) == 0 {
            return 0.0;
        }
        self.common
            .selected_talents
            .get("rime-talent-id-talent18")
            .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
            .unwrap_or(0.0)
    }

    pub(crate) fn consume_rime_frostweaver(&mut self) {
        if self.buff_stacks(AplBuff::FrostweaversWrath) == 0 {
            return;
        }
        self.hero.rime_mut().frostweavers_wrath_stacks -= 1;
        if self.hero.rime().frostweavers_wrath_stacks == 0 {
            self.hero.rime_mut().frostweavers_wrath_until = 0;
            self.deactivate_fixed_buff(AplBuff::FrostweaversWrath);
        }
    }

    pub(crate) fn reduce_rime_major_cooldowns(&mut self, orbs: u32) {
        let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent8") else {
            return;
        };
        let reduction = ms_param(talent, parameter_key!("cooldownReductionPerOrbSeconds"))
            .saturating_mul(u64::from(orbs));
        for kind in [
            DpsAbilityKind::IceBlitz,
            DpsAbilityKind::FlightOfTheNavir,
            DpsAbilityKind::WintersBlessing,
        ] {
            self.reduce_cooldown(kind, reduction);
        }
    }

    pub(crate) fn try_rime_spirit_refund(&mut self, spent_orbs: u32) {
        let guaranteed = if self.common.now_ms < self.hero.rime().undulating_spirit_until
            && self.hero.rime().undulating_spirit_stacks > 0
        {
            self.hero.rime_mut().undulating_spirit_stacks -= 1;
            if self.hero.rime().undulating_spirit_stacks == 0 {
                self.hero.rime_mut().undulating_spirit_until = 0;
            }
            true
        } else {
            self.hero.rime_mut().undulating_spirit_stacks = 0;
            self.hero.rime_mut().undulating_spirit_until = 0;
            false
        };
        let chance = spirit_refund_chance(self.effective_spirit(), 1.0, 0.0);
        if !guaranteed {
            if !self.roll_controlled_random_bool(SPIRIT_PROC_RANDOM_STREAM_TAG, chance) {
                return;
            }
            self.record_spirit_refund_proc();
        } else {
            self.trigger_spirit_refund_effects();
        }
        self.shared.spirit = (self.shared.spirit + 2.0).min(self.profile.max_spirit);
        self.push_event(
            self.common.now_ms.saturating_add(200),
            RimeEvent::OrbRefund { orbs: spent_orbs },
        );
    }

    pub(crate) fn rime_bursting_pulse(&mut self, generation: u64, context: DamageContext) {
        let Some((until, previous)) = self
            .hero
            .rime()
            .bursting_instances
            .get(&generation)
            .copied()
        else {
            return;
        };
        if self.common.now_ms > until {
            self.hero.rime_mut().bursting_instances.remove(&generation);
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::BurstingIce).cloned() else {
            return;
        };
        let period =
            ability_seconds_parameter(&ability, parameter_key!("pulsePeriodSeconds")).max(1);
        let elapsed = self.common.now_ms.saturating_sub(previous);
        if elapsed < period && elapsed < 100 {
            self.hero.rime_mut().bursting_instances.remove(&generation);
            return;
        }
        self.rime_bursting_damage_pulse_at(0, context, (elapsed as f64 / period as f64).min(1.0));
        let remaining = until.saturating_sub(self.common.now_ms);
        // Native timed-effect removal executes a terminal proportional tick
        // only when at least 0.1 seconds have accrued since the last execution.
        if remaining >= period || remaining >= 100 {
            let now = self.common.now_ms;
            self.hero
                .rime_mut()
                .bursting_instances
                .insert(generation, (until, now));
            self.push_event(
                now.saturating_add(period).min(until),
                RimeEvent::BurstingPulse {
                    generation,
                    context,
                },
            );
        } else {
            self.hero.rime_mut().bursting_instances.remove(&generation);
        }
    }

    pub(crate) fn rime_bursting_damage_pulse(&mut self, context: DamageContext) {
        self.rime_bursting_damage_pulse_at(0, context, 1.0);
    }

    fn rime_bursting_damage_pulse_at(
        &mut self,
        primary: u32,
        context: DamageContext,
        partial_factor: f64,
    ) {
        let Some(ability) = self.ability(DpsAbilityKind::BurstingIce).cloned() else {
            return;
        };
        let mut coefficient = ability_param(&ability, parameter_key!("pulsePowerCoefficient"));
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent2") {
            coefficient *= param(talent, parameter_key!("burstingDamageMultiplier"));
        }
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent11") {
            let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
            let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
            let flight_cooldown_reduction_ms =
                ms_param(talent, parameter_key!("flightCooldownReductionSeconds"));
            let damage_increase_per_stack = param(talent, parameter_key!("damageIncreasePerStack"));
            let previous = if self.hero.rime().harrowing_ice_until > self.common.now_ms {
                self.hero.rime().harrowing_ice_stacks
            } else {
                0
            };
            let next = previous.saturating_add(1);
            if next >= maximum_stacks {
                self.hero.rime_mut().harrowing_ice_stacks = 0;
                self.hero.rime_mut().harrowing_ice_until = 0;
                self.deactivate_fixed_buff(AplBuff::HarrowingIce);
                self.reduce_cooldown(
                    DpsAbilityKind::FlightOfTheNavir,
                    flight_cooldown_reduction_ms,
                );
            } else {
                let harrowing_until = self.common.now_ms.saturating_add(duration_ms);
                self.hero.rime_mut().harrowing_ice_stacks = next;
                self.hero.rime_mut().harrowing_ice_until = harrowing_until;
                self.activate_fixed_buff(AplBuff::HarrowingIce, harrowing_until);
            }
            coefficient *= 1.0 + f64::from(previous) * damage_increase_per_stack;
        }
        let mut targets = (0..self.common.target_count).collect::<Vec<_>>();
        self.common.rng.shuffle(&mut targets);
        targets.truncate(ability.max_targets as usize);
        coefficient *= multi_target_damage_falloff(
            targets.len() as u32,
            ability_param(
                &ability,
                parameter_key!("targetCountDamageScalingThreshold"),
            ),
        );
        if !self.charge_work_units(targets.len() as u64) {
            return;
        }
        for target_index in targets {
            // The instant GE recaptures on every application, including proc pulses.
            let context = context
                .as_proc()
                .with_snapshot(self.capture_damage_source_snapshot(DpsAbilityKind::BurstingIce));
            self.damage_raw_with_spread_key(
                Some(DpsAbilityKind::BurstingIce),
                self.ability_damage_source(&ability),
                coefficient * partial_factor * self.profile.power,
                context.source_snapshot.map(|s| s.expertise),
                context.source_snapshot.map(|s| s.primary_stat_multiplier),
                context.source_snapshot.map(|s| s.critical_chance),
                ability.damage_spread,
                0.0,
                true,
                true,
                target_index,
                context,
            );
        }
        self.add_rime_anima_at(
            ability_param(&ability, parameter_key!("animaPerPulse"))
                .min(ability_param(&ability, parameter_key!("animaPerPulseCap"))),
            primary,
            context,
        );
    }

    pub(crate) fn rime_wrath_volley(&mut self, generation: u64, context: DamageContext) {
        if generation != self.hero.rime().wrath_generation
            || self.common.now_ms > self.hero.rime().wrath_of_winter_until
        {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::WrathOfWinter).cloned() else {
            return;
        };
        let projectile_count =
            ability_u32_rounded(&ability, parameter_key!("volleyProjectiles")).max(1);
        self.fire_rime_anima_volley(projectile_count, 0, context);
        // PushExecutionFlow(649) resumes here even after a successful volley.
        self.add_rime_orbs(1);
        let period = self.hero.rime().wrath_period_ms;
        let next = self.common.now_ms.saturating_add(period);
        if next <= self.hero.rime().wrath_of_winter_until {
            self.push_event(
                next,
                RimeEvent::WrathVolley {
                    generation,
                    context,
                },
            );
        }
    }

    pub(crate) fn schedule_rime_projectile(
        &mut self,
        source: RimeTriggeredDamageSource,
        coefficient: f64,
        bonus_crit: f64,
        target_index: u32,
        context: DamageContext,
        delay_ms: u64,
    ) {
        let snapshot = self.capture_damage_source_snapshot(source.ability_kind());
        self.push_event(
            self.common.now_ms.saturating_add(delay_ms),
            RimeEvent::TriggeredDamage {
                source,
                coefficient_bits: coefficient.to_bits(),
                bonus_crit_bits: bonus_crit.to_bits(),
                target_index,
                context: context.as_proc().with_snapshot(snapshot),
            },
        );
    }

    pub(crate) fn rime_triggered_damage(
        &mut self,
        source: RimeTriggeredDamageSource,
        coefficient: f64,
        bonus_crit: f64,
        target_index: u32,
        mut context: DamageContext,
    ) {
        if matches!(
            source,
            RimeTriggeredDamageSource::AnimaSpike | RimeTriggeredDamageSource::FrostSwallow
        ) {
            // These instant effects opt into native source recapture before
            // every execution, including the first application after flight.
            context =
                context.with_snapshot(self.capture_damage_source_snapshot(source.ability_kind()));
        }
        let (kind, id, mut coefficient) = match source {
            RimeTriggeredDamageSource::AnimaSpike => (
                DpsAbilityKind::AnimaSpike,
                "GA_Rime_Helper_AutoDamageProjectile",
                coefficient,
            ),
            RimeTriggeredDamageSource::FrostSwallow => {
                let spirit_scaler = self
                    .ability(DpsAbilityKind::FlightOfTheNavir)
                    .map(|ability| {
                        ability_param(ability, parameter_key!("damageIncreasePerSpirit"))
                    })
                    .unwrap_or(1.5);
                (
                    DpsAbilityKind::FrostSwallow,
                    "GA_Rime_TargetedPeriodicProjectileAoe",
                    coefficient * (1.0 + self.effective_spirit() * spirit_scaler),
                )
            }
            RimeTriggeredDamageSource::IceComet => (
                DpsAbilityKind::IceComet,
                self.ability(DpsAbilityKind::IceComet)
                    .expect("Comet ability")
                    .id
                    .as_str(),
                coefficient,
            ),
            RimeTriggeredDamageSource::TalonStrike
            | RimeTriggeredDamageSource::GlacialAssaultTalonStrike => (
                DpsAbilityKind::GlacialBlast,
                "talent:talon-strike",
                coefficient,
            ),
            RimeTriggeredDamageSource::RisingTalons => (
                DpsAbilityKind::IceComet,
                "talent:rising-talons",
                coefficient,
            ),
        };
        if matches!(
            source,
            RimeTriggeredDamageSource::IceComet | RimeTriggeredDamageSource::RisingTalons
        ) {
            coefficient *= multi_target_damage_falloff(
                self.common.target_count,
                ability_param(
                    self.ability(DpsAbilityKind::IceComet)
                        .expect("Comet ability"),
                    parameter_key!("targetCountDamageScalingThreshold"),
                ),
            );
        }
        let snapshot = context.source_snapshot;
        let damage_spread = self
            .ability(kind)
            .map(|ability| ability.damage_spread)
            .unwrap_or(HERO_DAMAGE_SPREAD_WIDTH);
        let outcome = self.damage_raw_with_spread_key(
            Some(kind),
            self.damage_source_key(id),
            coefficient * self.profile.power,
            snapshot.map(|value| value.expertise),
            snapshot.map(|value| value.primary_stat_multiplier),
            snapshot.map(|value| value.critical_chance),
            damage_spread,
            bonus_crit,
            true,
            true,
            target_index,
            context,
        );
        if source == RimeTriggeredDamageSource::GlacialAssaultTalonStrike {
            self.rime_glacial_assault_explosion(outcome.damage, context);
        }
        if matches!(
            source,
            RimeTriggeredDamageSource::AnimaSpike | RimeTriggeredDamageSource::FrostSwallow
        ) {
            self.extend_rime_ice_blitz();
            if self
                .common
                .selected_talents
                .contains_key("rime-talent-id-talent15")
            {
                let chance = param(
                    self.common
                        .selected_talents
                        .get("rime-talent-id-talent15")
                        .expect("selected Bursting Swallows talent"),
                    parameter_key!("procChance"),
                );
                if self.roll_controlled_random_bool(
                    "RandomStream.Rime.Talent.AutoDamageProjectile.TriggerCastedDebuffAoeDamage",
                    chance,
                ) {
                    self.record_talent_proc("rime-talent-id-talent15");
                    self.rime_bursting_damage_pulse_at(target_index, context.as_proc(), 1.0);
                }
            }
        }
    }

    pub(crate) fn extend_rime_ice_blitz(&mut self) {
        if self.common.now_ms >= self.hero.rime().ice_blitz_until {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::IceBlitz) else {
            return;
        };
        let maximum_until = self
            .common
            .now_ms
            .saturating_add(ability.effect_duration_ms);
        let extended = self
            .hero
            .rime()
            .ice_blitz_until
            .saturating_add(ability_seconds_parameter(
                ability,
                parameter_key!("extensionSeconds"),
            ));
        if extended > maximum_until {
            return;
        }
        self.hero.rime_mut().ice_blitz_until = extended;
        self.activate_fixed_buff(AplBuff::IceBlitz, self.hero.rime().ice_blitz_until);
    }

    pub(crate) fn command_rime_frost_swallows(
        &mut self,
        count: u32,
        target_index: u32,
        context: DamageContext,
    ) {
        if self.common.now_ms >= self.hero.rime().flight_of_the_navir_until || count == 0 {
            return;
        }
        if !self.charge_work_units(self.hero.rime().flight_birds.len() as u64) {
            return;
        }
        // Specific-target commands retarget every actor, including busy ones.
        // Co-located dummies retain the existing approximated arrival time.
        for bird in 0..self.hero.rime().flight_birds.len() {
            self.launch_rime_bird(bird, target_index, context);
        }
    }

    fn launch_rime_bird(&mut self, bird: usize, target: u32, context: DamageContext) {
        let busy = self.hero.rime().flight_birds[bird].is_some();
        self.hero.rime_mut().flight_birds[bird] = Some(target);
        if !busy {
            self.push_event(
                self.common.now_ms.saturating_add(500),
                RimeEvent::BirdImpact {
                    generation: self.hero.rime().flight_generation,
                    bird,
                    context,
                },
            );
        }
    }

    fn command_rime_random_bird(&mut self, context: DamageContext) {
        if self.common.now_ms >= self.hero.rime().flight_of_the_navir_until {
            return;
        }
        let Some(bird) = self
            .hero
            .rime()
            .flight_birds
            .iter()
            .position(Option::is_none)
        else {
            return;
        };
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return;
        }
        let mut targets: Vec<u32> = (0..self.common.target_count).collect();
        self.common.rng.shuffle(&mut targets);
        self.launch_rime_bird(bird, targets[0], context);
    }

    fn rime_bird_impact(&mut self, generation: u64, bird: usize, context: DamageContext) {
        // The buff grants the passive with CancelAbilityImmediately removal;
        // cancellation invokes ForceDespawn and invalidates all actors.
        if generation != self.hero.rime().flight_generation
            || self.common.now_ms >= self.hero.rime().flight_of_the_navir_until
        {
            return;
        }
        let Some(target) = self.hero.rime_mut().flight_birds[bird].take() else {
            return;
        };
        let coefficient = self
            .ability(DpsAbilityKind::FlightOfTheNavir)
            .map(|ability| ability_param(ability, parameter_key!("projectilePowerCoefficient")))
            .unwrap_or(0.469);
        self.rime_triggered_damage(
            RimeTriggeredDamageSource::FrostSwallow,
            coefficient,
            0.0,
            target,
            context.as_proc(),
        );
    }

    pub(crate) fn try_rime_cold_shower(&mut self, context: DamageContext) {
        if let Some(chance) = self
            .common
            .selected_talents
            .get("rime-talent-id-talent7")
            .map(|talent| param(talent, parameter_key!("procChance")))
            && self.roll_controlled_random_bool(
                "RandomStream.Rime.Talent.ChanneledBeamSingleDamage.OnTargetPulsatingAoe.FreeCast",
                chance,
            )
        {
            self.record_talent_proc("rime-talent-id-talent7");
            self.schedule_rime_comet(context.as_proc(), 0);
        }
    }

    pub(crate) fn on_rime_torrent_tick(
        &mut self,
        critical: bool,
        target_index: u32,
        context: DamageContext,
    ) {
        self.command_rime_random_bird(context);
        let coalescing_was_active = self.hero.rime().coalescing_stacks[target_index as usize] > 0;
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent4") {
            self.reduce_cooldown(
                DpsAbilityKind::BurstingIce,
                ms_param(talent, parameter_key!("burstingCooldownReductionSeconds")),
            );
        }
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent3") {
            let critical_extra_stack_chance =
                param(talent, parameter_key!("criticalExtraStackChance"));
            let critical_extra_stacks = param_u32(talent, parameter_key!("criticalExtraStacks"));
            let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
            let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
            let mut stacks = 1;
            // The Blueprint rolls before AND-ing with the critical flag, so
            // noncritical ticks also advance this controlled-random stream.
            let extra_stack = self.roll_controlled_random_bool(
                "RandomStream.Rime.Talent.ChanneledBeamSingleDamage.ApplyDoubleStacks",
                critical_extra_stack_chance,
            );
            if critical && extra_stack {
                self.record_talent_proc("rime-talent-id-talent3");
                stacks += critical_extra_stacks;
            }
            let target = target_index as usize;
            self.hero.rime_mut().coalescing_stacks[target] = self.hero.rime().coalescing_stacks
                [target]
                .saturating_add(stacks)
                .min(maximum_stacks);
            self.hero.rime_mut().coalescing_generations[target] =
                self.hero.rime().coalescing_generations[target].wrapping_add(1);
            self.push_event(
                self.common.now_ms.saturating_add(duration_ms),
                RimeEvent::CoalescingExpire {
                    generation: self.hero.rime().coalescing_generations[target],
                    target_index,
                    context: context.as_proc(),
                },
            );
        }
        if self
            .common
            .selected_talents
            .contains_key("rime-talent-id-talent3")
            && let Some(ability) = self.ability(DpsAbilityKind::FreezingTorrent).cloned()
        {
            // Stack quantity does not change the number of applications,
            // but an existing timed effect also emits a duration refresh.
            self.notify_kindling_of_harmful_effect(
                Some(ability.kind),
                self.ability_damage_source(&ability),
                target_index,
                coalescing_was_active,
            );
        }
        if let Some(mechanic) = self.rime_legendary_mechanic(LegendaryHeroSourceKind::Frostwyrm) {
            if self.common.now_ms >= self.hero.rime().frostwyrm_until {
                self.hero.rime_mut().frostwyrm_stacks = 0;
            }
            self.hero.rime_mut().frostwyrm_stacks = self
                .hero
                .rime()
                .frostwyrm_stacks
                .saturating_add(1)
                .min(mechanic_u32(
                    &mechanic,
                    parameter_key!("frostwyrmMaximumStacks"),
                ));
            self.hero.rime_mut().frostwyrm_until = self.common.now_ms.saturating_add(
                seconds_parameter(&mechanic, parameter_key!("frostwyrmDurationSeconds")),
            );
        }
    }

    pub(crate) fn expire_coalescing_frost(
        &mut self,
        generation: u64,
        target_index: u32,
        context: DamageContext,
    ) {
        let target = target_index as usize;
        if self.hero.rime().coalescing_generations.get(target).copied() != Some(generation) {
            return;
        }
        let stacks = std::mem::take(&mut self.hero.rime_mut().coalescing_stacks[target]);
        if stacks == 0 {
            return;
        }
        let talent = self
            .common
            .selected_talents
            .get("rime-talent-id-talent3")
            .expect("selected Coalescing Frost talent");
        let coefficient =
            param(talent, parameter_key!("powerCoefficientPerStack")) * f64::from(stacks);
        let secondary_coefficient = coefficient
            * multi_target_damage_falloff(
                self.common.target_count.saturating_sub(1).max(1),
                param(talent, parameter_key!("targetCountDamageScalingThreshold")),
            );
        // A fresh spec is created at removal, applied to the owner immediately,
        // and reused for the delayed secondary list (which excludes the owner).
        let context = context
            .as_proc()
            .with_snapshot(self.capture_damage_source_snapshot(DpsAbilityKind::FreezingTorrent));
        self.damage_coalescing_frost(coefficient, target_index, context);
        if self.common.target_count > 1 {
            self.push_event(
                self.common.now_ms.saturating_add(200),
                RimeEvent::CoalescingSecondary {
                    coefficient_bits: secondary_coefficient.to_bits(),
                    primary: target_index,
                    context,
                },
            );
        }
    }

    fn damage_coalescing_frost(&mut self, coefficient: f64, target: u32, context: DamageContext) {
        let snapshot = context
            .source_snapshot
            .expect("Coalescing captures on removal");
        self.damage_raw_with_spread_key(
            Some(DpsAbilityKind::FreezingTorrent),
            self.damage_source_key("talent:coalescing-frost"),
            coefficient * self.profile.power,
            Some(snapshot.expertise),
            Some(snapshot.primary_stat_multiplier),
            Some(snapshot.critical_chance),
            HERO_DAMAGE_SPREAD_WIDTH,
            0.0,
            true,
            false,
            target,
            context,
        );
    }

    pub(crate) fn rime_glacial_assault_explosion(
        &mut self,
        resolved_damage: f64,
        context: DamageContext,
    ) {
        self.push_event(
            self.common.now_ms.saturating_add(150),
            RimeEvent::GlacialAssaultExplosion {
                damage_bits: resolved_damage.to_bits(),
                context,
            },
        );
    }

    fn apply_rime_glacial_assault_explosion(
        &mut self,
        resolved_damage: f64,
        context: DamageContext,
    ) {
        let talent = self
            .common
            .selected_talents
            .get("rime-talent-id-talent1")
            .expect("selected Glacial Assault talent");
        let affected = self.common.target_count.saturating_sub(1);
        let falloff = multi_target_damage_falloff(
            affected.max(1),
            param(talent, parameter_key!("targetCountDamageScalingThreshold")),
        );
        if !self.charge_work_units(u64::from(affected)) {
            return;
        }
        for target_index in 1..self.common.target_count {
            self.damage_unscaled_key(
                Some(DpsAbilityKind::GlacialBlast),
                self.damage_source_key("talent:glacial-assault"),
                resolved_damage
                    * param(talent, parameter_key!("explosionDamageFraction"))
                    * falloff,
                target_index,
                DamageProvenance::Explosion,
                context.as_proc(),
            );
        }
    }

    pub(crate) fn spend_rime_orbs(&mut self, requested: u32) {
        let spent = requested.min(self.hero.rime().winter_orbs);
        self.hero.rime_mut().winter_orbs -= spent;
        if spent > 0 {
            self.try_rime_spirit_refund(spent);
            self.reduce_rime_major_cooldowns(spent);
        }
    }

    fn consume_rime_icy_flow(&mut self) {
        if self.buff_stacks(AplBuff::IcyFlow) > 0 {
            self.hero.rime_mut().icy_flow_stacks -= 1;
            if self.hero.rime().icy_flow_stacks == 0 {
                self.hero.rime_mut().icy_flow_until = 0;
                self.deactivate_fixed_buff(AplBuff::IcyFlow);
            }
        }
    }

    pub(crate) fn capture_rime_spender_bonuses(
        &mut self,
        assault: bool,
        context: &mut DamageContext,
    ) -> f64 {
        if assault {
            context.multiply_damage(param(
                self.common
                    .selected_talents
                    .get("rime-talent-id-talent1")
                    .unwrap(),
                parameter_key!("damageMultiplier"),
            ));
            self.hero.rime_mut().glacial_assault_stacks = 0;
            self.deactivate_fixed_buff(AplBuff::GlacialAssault);
        }
        let crit = if self.buff_stacks(AplBuff::IcyFlow) > 0 {
            param(
                self.common
                    .selected_talents
                    .get("rime-talent-id-talent5")
                    .unwrap(),
                parameter_key!("criticalStrikeBonus"),
            )
        } else {
            0.0
        };
        self.consume_rime_icy_flow();
        crit
    }

    pub(crate) fn roll_rime_avalanche(&mut self) -> u32 {
        let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent6") else {
            return 0;
        };
        let total = param(talent, parameter_key!("oneExtraChance"))
            + param(talent, parameter_key!("twoExtraChance"));
        let triple = param(talent, parameter_key!("twoExtraChance"));
        if total <= 0.0
            || !self.roll_controlled_random_bool(
                "RandomStream.Rime.Talent.OnTargetPulsatingAoe.Multistrike",
                total,
            )
        {
            return 0;
        }
        self.record_talent_proc("rime-talent-id-talent6");
        1 + u32::from(self.roll_controlled_random_bool(
            "RandomStream.Rime.Talent.OnTargetPulsatingAoe.Multistrike.TrippleCheck",
            triple / total,
        ))
    }

    pub(crate) fn schedule_rime_comet(&mut self, context: DamageContext, initial_delay_ms: u64) {
        let Some(ability) = self.ability(DpsAbilityKind::IceComet).cloned() else {
            return;
        };
        let swapped = self
            .common
            .selected_talents
            .contains_key("rime-talent-id-talent17");
        let avalanche = self
            .common
            .selected_talents
            .contains_key("rime-talent-id-talent6");
        let extras = self.roll_rime_avalanche();
        if swapped && avalanche && extras == 0 {
            return;
        }
        // Without Avalanche the graph still spawns a zero-count swapped
        // actor. Its stop check runs after its first periodic application.
        let pulses = (u32::from(!swapped) + extras).max(1);
        let active_flow = self.buff_stacks(AplBuff::IcyFlow) > 0;
        let (initial_delay_ms, bonus_crit) = if active_flow {
            let talent = self
                .common
                .selected_talents
                .get("rime-talent-id-talent5")
                .unwrap();
            (
                ms_param(talent, parameter_key!("cometInitialDelaySeconds")),
                param(talent, parameter_key!("criticalStrikeBonus")),
            )
        } else {
            (initial_delay_ms, 0.0)
        };
        self.consume_rime_icy_flow();
        let delay = (initial_delay_ms as f64 / (1.0 + self.effective_haste()).max(0.05)).round()
            as u64
            + ability.first_hit_delay_ms;
        let period = ability_seconds_parameter(&ability, parameter_key!("pulsePeriodSeconds"));
        for pulse in 0..pulses {
            self.schedule_rime_spender_pulse(
                RimeTriggeredDamageSource::IceComet,
                ability.power_coefficient,
                bonus_crit,
                self.common.target_count,
                context,
                delay.saturating_add(period.saturating_mul(u64::from(pulse))),
            );
        }
    }

    pub(crate) fn schedule_rime_spender_pulse(
        &mut self,
        source: RimeTriggeredDamageSource,
        coefficient: f64,
        bonus_crit: f64,
        targets: u32,
        context: DamageContext,
        delay_ms: u64,
    ) {
        if !self.charge_work_units(u64::from(targets)) {
            return;
        }
        let snapshot = self.capture_damage_source_snapshot(source.ability_kind());
        self.push_event(
            self.common.now_ms.saturating_add(delay_ms),
            RimeEvent::SpenderPulse {
                source,
                coefficient_bits: coefficient.to_bits(),
                bonus_crit_bits: bonus_crit.to_bits(),
                targets,
                context: context.without_cast_proc().with_snapshot(snapshot),
            },
        );
    }

    pub(crate) fn try_soulfrost_torrent_proc(&mut self, ability: &CompiledAbility) {
        let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent9") else {
            return;
        };
        if !is_offensive_skill_commit(ability.kind)
            || ability.kind == DpsAbilityKind::FreezingTorrent
            || self.common.now_ms < self.hero.rime().soulfrost_torrent_until
        {
            return;
        }
        let procs_per_minute = param(talent, parameter_key!("procsPerMinute"));
        let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
        if self.roll_proc_per_minute(
            "RandomStream.Rime.Talent.ChanneledBeamSingleDamage.BoostByCritProc",
            procs_per_minute,
            true,
        ) {
            self.record_talent_proc("rime-talent-id-talent9");
            self.hero.rime_mut().soulfrost_torrent_until =
                self.common.now_ms.saturating_add(duration_ms);
            self.activate_fixed_buff(
                AplBuff::SoulfrostTorrent,
                self.hero.rime().soulfrost_torrent_until,
            );
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn accumulate_rime_blessing_heal(
        &mut self,
        event: &OutgoingDamageEvent,
        damage: f64,
    ) {
        if self.profile.contract.hero != HeroIdentity::Rime
            || damage <= 0.0
            || event.provenance == DamageProvenance::Periodic
            || self.common.now_ms >= self.hero.rime().winters_blessing_until
        {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::WintersBlessing).cloned() else {
            return;
        };
        self.hero.rime_mut().blessing_heal_pending +=
            damage * ability_param(&ability, parameter_key!("damageToHealingFactor"));
        if self.hero.rime().blessing_heal_scheduled {
            return;
        }
        self.hero.rime_mut().blessing_heal_scheduled = true;
        // Ending the buff flushes its pending amount before cancelling the timer.
        let at = self
            .common
            .now_ms
            .saturating_add(ability_seconds_parameter(
                &ability,
                parameter_key!("healingBatchSeconds"),
            ))
            .min(self.hero.rime().winters_blessing_until);
        self.push_event(
            at,
            RimeEvent::BlessingHeal {
                generation: self.hero.rime().blessing_heal_generation,
            },
        );
    }

    fn flush_rime_blessing_heal(&mut self, generation: u64) {
        if generation != self.hero.rime().blessing_heal_generation {
            return;
        }
        let amount = std::mem::take(&mut self.hero.rime_mut().blessing_heal_pending);
        self.hero.rime_mut().blessing_heal_scheduled = false;
        if amount <= 0.0 {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::WintersBlessing) else {
            return;
        };
        self.apply_positive_healing(self.ability_damage_source(ability), false);
    }
}
