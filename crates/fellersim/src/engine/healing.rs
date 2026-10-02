use crate::*;

impl Iteration<'_> {
    pub(crate) fn refresh_periodic_healing_clock(&mut self) {
        let Some(index) = self.profile.mechanic_indexes.periodic_healing else {
            return;
        };
        let mechanic = Arc::clone(&self.profile.mechanics[index]);
        let base_ms = seconds_parameter(&mechanic, parameter_key!("periodSeconds"));
        if base_ms == 0 || mechanic_param(&mechanic, parameter_key!("healingHealthFraction")) <= 0.0
        {
            return;
        }
        let now = self.common.now_ms;
        // The pre-combat phase is unknown. Our explicit scenario approximation
        // starts a full period at pull, with no immediate healing tick.
        let mut clock = self
            .shared
            .periodic_healing
            .unwrap_or_else(|| PeriodicHealingClock {
                remaining_base_ms: base_ms as f64,
                updated_ms: now,
                rate: (1.0 + self.effective_haste()).max(0.05),
                wake_ms: 0,
                generation: 0,
            });
        clock.remaining_base_ms -= now.saturating_sub(clock.updated_ms) as f64 * clock.rate;
        clock.updated_ms = now;
        if clock.remaining_base_ms <= 1e-6 {
            clock.remaining_base_ms = base_ms as f64;
            // Empty-preset self-overhealing is noncritical but still reaches
            // Diamond Strike. This directly granted effect has no Trait ability.
            self.record_dynamic_proc(&mechanic);
            self.emit_positive_healing(mechanic.damage_source, false);
        }
        clock.rate = (1.0 + self.effective_haste()).max(0.05);
        let due = now.saturating_add((clock.remaining_base_ms / clock.rate).ceil().max(1.0) as u64);
        // Wake at Haste expirations as well as ticks so a rate change preserves
        // elapsed phase. Proc/cast changes refresh the clock at dispatch boundaries.
        let wake = self.next_cooldown_rate_boundary(due);
        if wake != clock.wake_ms {
            clock.wake_ms = wake;
            clock.generation = clock.generation.wrapping_add(1);
            self.push_event(
                wake,
                SharedEvent::PeriodicHealingWake {
                    generation: clock.generation,
                },
            );
        }
        self.shared.periodic_healing = Some(clock);
    }

    // Only the positive-heal event and critical outcome can affect DPS in the
    // maintained full-health solo encounter. Friendly healing amplifiers alter
    // neither, and healing never enters damage totals or damage-derived Spirit.
    pub(crate) fn emit_positive_healing(&mut self, source: DamageSourceKey, critical: bool) {
        if !self.common.execution.charge(1) {
            return;
        }
        let source_is_trait = self.profile.damage_sources[source.0].item_trait;
        let count = self.profile.mechanic_indexes.on_healing.len();
        if !self.common.execution.charge_usize(count) {
            return;
        }
        let mut critical_dispatch = self.critical_listener_dispatch(critical);
        for position in 0..count {
            let index = self.profile.mechanic_indexes.on_healing[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if critical && self.dispatch_critical_stat_mechanic(&mechanic, &mut critical_dispatch) {
                continue;
            }
            if mechanic.source_id == "ItemTrait.ID.GemSingleTargetProcOnDamageHeal"
                && !source_is_trait
            {
                // Healing and damage have independent authored RPPM streams.
                if self.roll_proc_per_minute(
                    "RandomStream.Traits.GemSingleTargetProcOnDamageHeal_Proc.Heal",
                    mechanic_param(&mechanic, parameter_key!("procsPerMinute")),
                    true,
                ) {
                    self.record_dynamic_random_proc(&mechanic);
                    let critical = self.resolve_healing_critical(None);
                    self.apply_positive_healing(mechanic.damage_source, critical);
                }
            }
        }
    }

    pub(crate) fn resolve_healing_critical(&mut self, kind: Option<DpsAbilityKind>) -> bool {
        let chance = self.effective_critical_strike(kind).max(0.0);
        let critical = self.common.rng.damage_outcome_is_critical(chance);
        // HeroMagicalDirect has an independent spread draw after hit selection.
        // Its positive magnitude is not an outgoing damage contribution.
        let _ = self.common.rng.damage_spread_roll();
        critical
    }

    pub(crate) fn critical_listener_dispatch(&self, critical: bool) -> CriticalListenerDispatch {
        if !critical
            || !self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.on_critical_damage.len())
        {
            return CriticalListenerDispatch::default();
        }
        let reactivated_counter = {
            self.profile
                .mechanic_indexes
                .on_critical_damage
                .iter()
                .copied()
                .find(|index| {
                    self.profile.mechanics[*index].source_id
                        == "ItemTrait.ID.CritsToIncreasedCritRating"
                        && self.shared.dynamic_buffs[*index]
                            .as_ref()
                            .is_some_and(|buff| buff.until_ms <= self.common.now_ms)
                })
        };
        CriticalListenerDispatch {
            reactivated_counter,
            counter_dispatched: false,
        }
    }

    pub(crate) fn dispatch_critical_stat_mechanic(
        &mut self,
        mechanic: &CompiledMechanic,
        dispatch: &mut CriticalListenerDispatch,
    ) -> bool {
        if let Some(index) = dispatch.reactivated_counter {
            if dispatch.counter_dispatched && index == mechanic.index {
                return true;
            }
            let same_critical_group = matches!(
                mechanic.kind,
                CompiledMechanicKind::HeroSource(HeroSourceKind::Trait(
                    TraitHeroSourceKind::CritsToIncreasedCritRating
                        | TraitHeroSourceKind::CritsToIncreasedPrimaryStatBuff
                        | TraitHeroSourceKind::GemDotHotOnCrit
                )) | CompiledMechanicKind::HeroSource(HeroSourceKind::Set(
                    SetHeroSourceKind::PowerOnCrit
                ))
            );
            if !dispatch.counter_dispatched && same_critical_group {
                // Seized Opportunity ends its listener for the buff interval,
                // then appends a new listener to this same native tag group.
                // Reverse multicast traversal visits it before older peers.
                // Initial peer/group order remains a separate evidence boundary.
                dispatch.counter_dispatched = true;
                let counter = Arc::clone(&self.profile.mechanics[index]);
                self.trigger_critical_stat_mechanic(&counter);
                if index == mechanic.index {
                    return true;
                }
            }
        }
        self.trigger_critical_stat_mechanic(mechanic)
    }

    pub(crate) fn trigger_critical_stat_mechanic(&mut self, mechanic: &CompiledMechanic) -> bool {
        match mechanic.source_id.as_str() {
            "ItemTrait.ID.CritsToIncreasedCritRating" => {
                if self
                    .shared
                    .dynamic_buffs
                    .get(mechanic.index)
                    .and_then(Option::as_ref)
                    .is_some_and(|buff| buff.until_ms > self.common.now_ms)
                {
                    return true;
                }
                let required =
                    mechanic_u32(mechanic, parameter_key!("requiredCriticalStrikes")).max(1);
                let counter = &mut self.shared.dynamic_counters[mechanic.index];
                *counter = counter.saturating_add(1);
                if *counter >= required {
                    *counter = 0;
                    self.activate_dynamic_buff(
                        mechanic,
                        seconds_parameter(mechanic, parameter_key!("durationSeconds")),
                        1,
                        0.0,
                    );
                    self.record_dynamic_proc(mechanic);
                }
            }
            "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff" => {
                if self.roll_dynamic_proc(mechanic) {
                    self.activate_dynamic_buff(
                        mechanic,
                        seconds_parameter(mechanic, parameter_key!("durationSeconds")),
                        1,
                        0.0,
                    );
                }
            }
            _ if matches!(
                mechanic.kind,
                CompiledMechanicKind::HeroSource(HeroSourceKind::Set(
                    SetHeroSourceKind::PowerOnCrit
                ))
            ) =>
            {
                if self.roll_dynamic_proc(mechanic) {
                    self.activate_dynamic_buff(
                        mechanic,
                        seconds_parameter(mechanic, parameter_key!("durationSeconds")),
                        1,
                        0.0,
                    );
                }
            }
            _ => return false,
        }
        true
    }
}
