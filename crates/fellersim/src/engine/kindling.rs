use crate::*;

impl Iteration<'_> {
    pub(crate) fn notify_kindling_of_harmful_effect(
        &mut self,
        source_ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        target_index: u32,
        duration_refreshed: bool,
    ) {
        // Successful timed stacking resets duration before broadcasting the
        // application. Kindling subscribes to both native delegates. Manual
        // refreshes and NeverRefresh effects instead send only one notification.
        if duration_refreshed {
            self.trigger_kindling_on_harmful_effect_application(
                source_ability_kind,
                source,
                target_index,
            );
        }
        self.trigger_kindling_on_harmful_effect_application(
            source_ability_kind,
            source,
            target_index,
        );
    }

    pub(crate) fn trigger_kindling_on_hero_debuffs(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        duration_refreshed: bool,
    ) {
        // These are distinct Harmful applications, not extra damage events.
        let applies = match ability.kind {
            DpsAbilityKind::HighwindArrow => {
                self.profile.mechanic_indexes.elarion_shimmer.is_some()
            }
            DpsAbilityKind::LeapSmash => self.profile.mechanic_indexes.tariq_slayers_mosh.is_some(),
            DpsAbilityKind::Rupture | DpsAbilityKind::ButchersHook => true,
            _ => false,
        };
        if applies {
            self.notify_kindling_of_harmful_effect(
                Some(ability.kind),
                self.ability_damage_source(ability),
                target_index,
                duration_refreshed,
            );
        }
    }

    pub(crate) fn apply_positive_healing(&mut self, source: DamageSourceKey, critical: bool) {
        self.emit_positive_healing(source, critical);
        // These call sites apply instant, non-secondary, Healing-tagged effects.
        // Periodic executions use emit_positive_healing directly instead.
        let count = self.profile.mechanic_indexes.extra_dot_on_application.len();
        if !self.common.execution.charge_usize(count) {
            return;
        }
        for position in 0..count {
            let index = self.profile.mechanic_indexes.extra_dot_on_application[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if source == mechanic.damage_source
                || mechanic_param(&mechanic, parameter_key!("healingPowerCoefficientPerTick"))
                    <= 0.0
            {
                continue;
            }
            if self.roll_proc_per_minute(
                "RandomStream.Traits.ExtraDotHotOnEffectApplicationProc.Hot",
                mechanic_param(&mechanic, parameter_key!("procsPerMinute")),
                true,
            ) {
                self.record_dynamic_random_proc(&mechanic);
                if let Some(state) =
                    self.kindling_state(&mechanic, self.shared.kindling_healing[index], true)
                {
                    self.shared.kindling_healing[index] = Some(state);
                    self.refresh_kindling_clocks();
                }
            }
        }
    }

    pub(crate) fn kindling_state(
        &self,
        mechanic: &CompiledMechanic,
        previous: Option<KindlingState>,
        healing: bool,
    ) -> Option<KindlingState> {
        let (duration, period, coefficient) = if healing {
            (
                parameter_key!("healingDurationSeconds"),
                parameter_key!("healingPeriodSeconds"),
                parameter_key!("healingPowerCoefficientPerTick"),
            )
        } else {
            (
                parameter_key!("durationSeconds"),
                parameter_key!("periodSeconds"),
                parameter_key!("powerCoefficientPerTick"),
            )
        };
        let duration = seconds_parameter(mechanic, duration);
        let period = seconds_parameter(mechanic, period);
        if duration == 0 || period == 0 {
            return None;
        }
        let now = self.common.now_ms;
        let active = previous.filter(|state| state.until_ms > now);
        Some(KindlingState {
            generation: previous.map_or(1, |state| state.generation.wrapping_add(1)),
            until_ms: now.saturating_add(duration),
            next_tick_ms: 0,
            remaining_base_ms: active.map_or(period as f64, |state| {
                state.remaining_base_ms
                    - now.saturating_sub(state.clock_updated_ms) as f64 * state.clock_rate
            }),
            clock_updated_ms: now,
            clock_rate: (1.0 + self.effective_haste()).max(0.05),
            damage_per_tick: self.profile.power * mechanic_param(mechanic, coefficient),
            expertise_snapshot: self.effective_expertise(),
            primary_stat_multiplier_snapshot: self.effective_power_multiplier(),
            critical_chance_snapshot: self.effective_critical_strike(None),
        })
    }

    pub(crate) fn refresh_kindling_clocks(&mut self) {
        let count = self.profile.mechanic_indexes.extra_dot_on_application.len();
        if !self.common.execution.charge_usize(count) {
            return;
        }
        for position in 0..count {
            let index = self.profile.mechanic_indexes.extra_dot_on_application[position];
            // One friendly owner plus the maintained hostile targets.
            if !self
                .common
                .execution
                .charge_usize(self.common.result.targets.len() + 1)
            {
                return;
            }
            for target in 0..=self.common.result.targets.len() {
                let healing = target == self.common.result.targets.len();
                let slot = if healing {
                    index
                } else {
                    self.mechanic_target_slot(index, target as u32)
                };
                let state = if healing {
                    self.shared.kindling_healing[slot]
                } else {
                    self.shared.kindling[slot]
                };
                let Some(mut state) = state.filter(|state| self.common.now_ms <= state.until_ms)
                else {
                    continue;
                };
                let now = self.common.now_ms;
                state.remaining_base_ms -=
                    now.saturating_sub(state.clock_updated_ms) as f64 * state.clock_rate;
                state.clock_updated_ms = now;
                state.clock_rate = (1.0 + self.effective_haste()).max(0.05);
                let due = now.saturating_add(
                    (state.remaining_base_ms.max(0.0) / state.clock_rate).ceil() as u64,
                );
                let wake = self.next_cooldown_rate_boundary(due);
                if wake != state.next_tick_ms || state.next_tick_ms == 0 {
                    state.next_tick_ms = wake;
                    state.generation = state.generation.wrapping_add(1);
                    if wake <= state.until_ms {
                        if healing {
                            self.push_event(
                                wake,
                                SharedEvent::KindlingHealingTick {
                                    mechanic_index: index,
                                    generation: state.generation,
                                },
                            );
                        } else {
                            self.push_event(
                                wake,
                                SharedEvent::KindlingTick {
                                    mechanic_index: index,
                                    generation: state.generation,
                                    target_index: target as u32,
                                },
                            );
                        }
                    }
                }
                if healing {
                    self.shared.kindling_healing[slot] = Some(state);
                } else {
                    self.shared.kindling[slot] = Some(state);
                }
            }
        }
    }

    pub(crate) fn tick_kindling(&mut self, index: usize, generation: u64, target_index: u32) {
        let slot = self.mechanic_target_slot(index, target_index);
        let Some(state) = self.advance_kindling_tick(index, generation, slot, false) else {
            return;
        };
        let mechanic = Arc::clone(&self.profile.mechanics[index]);
        self.damage_periodic(
            None,
            mechanic.damage_source,
            state.damage_per_tick,
            state.expertise_snapshot,
            state.primary_stat_multiplier_snapshot,
            HERO_DAMAGE_SPREAD_WIDTH,
            state.critical_chance_snapshot,
            true,
            target_index,
            DamageContext::NONE,
        );
        // Advance before synchronous damage callbacks: a nested proc can
        // refresh the spec and must keep both its snapshot and the next period.
        self.refresh_kindling_clocks();
    }

    pub(crate) fn tick_kindling_healing(&mut self, index: usize, generation: u64) {
        let Some(state) = self.advance_kindling_tick(index, generation, index, true) else {
            return;
        };
        if state.damage_per_tick > 0.0 {
            let critical = self
                .common
                .rng
                .damage_outcome_is_critical(state.critical_chance_snapshot.max(0.0));
            let _ = self.common.rng.damage_spread_roll();
            self.emit_positive_healing(self.profile.mechanics[index].damage_source, critical);
        }
        self.refresh_kindling_clocks();
    }

    fn advance_kindling_tick(
        &mut self,
        index: usize,
        generation: u64,
        slot: usize,
        healing: bool,
    ) -> Option<KindlingState> {
        let state = if healing {
            self.shared.kindling_healing[slot]
        } else {
            self.shared.kindling[slot]
        };
        let mut state = state.filter(|state| {
            state.generation == generation
                && state.next_tick_ms == self.common.now_ms
                && self.common.now_ms <= state.until_ms
        })?;
        state.remaining_base_ms -=
            self.common.now_ms.saturating_sub(state.clock_updated_ms) as f64 * state.clock_rate;
        state.clock_updated_ms = self.common.now_ms;
        if state.remaining_base_ms > 1e-6 {
            // This wake belongs to a Haste boundary, not a periodic execution.
            self.refresh_kindling_clocks();
            return None;
        }
        let period = if healing {
            parameter_key!("healingPeriodSeconds")
        } else {
            parameter_key!("periodSeconds")
        };
        state.remaining_base_ms = seconds_parameter(&*self.profile.mechanics[index], period) as f64;
        state.next_tick_ms = 0;
        if healing {
            self.shared.kindling_healing[slot] = Some(state);
        } else {
            self.shared.kindling[slot] = Some(state);
        }
        Some(state)
    }
}
