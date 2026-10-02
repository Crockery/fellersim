use crate::*;

impl Iteration<'_> {
    fn preserve_overlapping_dot(&mut self, target: u32, kind: DotKind) {
        if !kind.has_independent_instances() {
            return;
        }
        let Some(dot) = self.common.dots.remove(&(target, kind)) else {
            return;
        };
        if dot.expires_ms < self.common.now_ms {
            self.record_dot_uptime(target, kind, &dot);
            return;
        }
        // Keep the previous instance and its captured spec, phase and expiry.
        // Its old events use the primary key and become stale on replacement.
        let archived =
            DotKind::OverlappingAbility(kind.ability_kind().unwrap(), self.common.sequence);
        self.push_event(
            dot.next_tick_ms.max(self.common.now_ms),
            CoreEvent::DotTick {
                kind: archived,
                generation: dot.generation,
                target_index: target,
            },
        );
        self.push_event(
            dot.expires_ms,
            CoreEvent::DotExpire {
                kind: archived,
                generation: dot.generation,
                target_index: target,
            },
        );
        self.common.dots.insert((target, archived), dot);
    }

    #[cfg(test)]
    pub(crate) fn impact(
        &mut self,
        index: usize,
        single_hit: bool,
        damage_scale: f64,
        target_index: u32,
        context: DamageContext,
    ) {
        self.resolve_impact(
            index,
            single_hit,
            damage_scale,
            target_index,
            CastImpactContext::new(context),
        );
    }

    pub(crate) fn apply_linked_dot(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        context: DamageContext,
    ) {
        let Some(dot_kind) = ability.applies_dot_kind else {
            return;
        };
        let Some(dot_ability) = self.ability(dot_kind).cloned() else {
            return;
        };
        let Some(dot) = dot_ability.dot else {
            return;
        };
        let targets = if ability.kind == DpsAbilityKind::Pyromania {
            self.pyromania_targets(target_index, ability.max_targets)
        } else {
            vec![target_index]
        };
        if !self.common.execution.charge_usize(targets.len()) {
            return;
        }
        for automatic_target in targets {
            self.apply_dot(automatic_target, dot_kind, &dot_ability, dot, context);
        }
    }

    pub(crate) fn apply_dot(
        &mut self,
        target_index: u32,
        kind: DpsAbilityKind,
        ability: &CompiledAbility,
        mut model: DotModel,
        context: DamageContext,
    ) {
        if kind == DpsAbilityKind::EngulfingFlames
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent12")
        {
            model.duration_ms += ms_param(talent, parameter_key!("durationIncreaseSeconds"));
        }
        let dot_kind = DotKind::Ability(kind);
        self.preserve_overlapping_dot(target_index, dot_kind);
        if kind == DpsAbilityKind::WeaponFrostVolley
            && self
                .common
                .dots
                .get(&(target_index, dot_kind))
                .is_some_and(|dot| dot.expires_ms >= self.common.now_ms)
        {
            // Native stacking replaces the spec even with NeverRefresh and
            // NeverReset. Update captures without restarting the periodic
            // clock or recreating its linked damage-amplifier effect.
            let snapshot = context
                .source_snapshot
                .unwrap_or_else(|| self.capture_damage_source_snapshot(kind));
            let active = self
                .common
                .dots
                .get_mut(&(target_index, dot_kind))
                .expect("active Frost Volley was checked above");
            active.expertise_snapshot = snapshot.expertise;
            active.primary_stat_multiplier_snapshot = snapshot.primary_stat_multiplier;
            active.critical_chance_override = Some(snapshot.critical_chance.max(0.0));
            active.context = context.with_snapshot(snapshot);
            self.trigger_kindling_on_harmful_effect_application(
                Some(kind),
                self.ability_damage_source(ability),
                target_index,
            );
            return;
        }
        if kind == DpsAbilityKind::SearingBlaze
            && self
                .common
                .selected_talents
                .contains_key("firemage-talent-id-talent3")
            && self
                .common
                .dots
                .get(&(target_index, dot_kind))
                .is_some_and(|dot| dot.expires_ms >= self.common.now_ms)
        {
            // With Agonizing Blaze selected, the ability graph refreshes the
            // existing stacking effect instead of applying a new spec. Keep
            // its proc snapshot, stack count, source context, and tick phase.
            let expires_ms = self.common.now_ms.saturating_add(model.duration_ms);
            let generation = {
                let active = self
                    .common
                    .dots
                    .get_mut(&(target_index, dot_kind))
                    .expect("active Searing Blaze was checked above");
                active.expires_ms = expires_ms;
                active.generation
            };
            self.push_event(
                expires_ms,
                CoreEvent::DotExpire {
                    kind: dot_kind,
                    generation,
                    target_index,
                },
            );
            self.trigger_kindling_on_harmful_effect_application(
                Some(kind),
                self.ability_damage_source(ability),
                target_index,
            );
            self.trigger_devouring_flame_kindling(
                kind,
                self.ability_damage_source(ability),
                target_index,
            );
            return;
        }
        let generation = self.common.sequence.wrapping_add(1);
        let existing = self
            .common
            .dots
            .get(&(target_index, dot_kind))
            .filter(|dot| dot.expires_ms >= self.common.now_ms)
            .cloned();
        // These cooked effects explicitly use AggregateBySource and refresh
        // duration on successful application. Do not infer that policy for
        // actor-driven periodic damage or effects with unresolved defaults.
        let duration_refreshed = existing.is_some()
            && matches!(
                kind,
                DpsAbilityKind::SearingBlaze
                    | DpsAbilityKind::Incinerate
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::SeethingPoison
                    | DpsAbilityKind::VolatilePoison
                    | DpsAbilityKind::Hemotoxin
            );
        if let Some(expired) = self
            .common
            .dots
            .get(&(target_index, dot_kind))
            .filter(|dot| dot.expires_ms < self.common.now_ms)
            .cloned()
        {
            self.record_dot_uptime(target_index, dot_kind, &expired);
        }
        let stacks = existing
            .as_ref()
            .map(|dot| {
                if matches!(kind, DpsAbilityKind::Incinerate | DpsAbilityKind::Hemotoxin) {
                    dot.stacks
                        .saturating_add(1)
                        .min(model.maximum_stacks.max(1))
                } else {
                    dot.stacks
                }
            })
            .unwrap_or(1);
        let started_ms = existing
            .as_ref()
            .map(|dot| dot.started_ms)
            .unwrap_or(self.common.now_ms);
        let period = self.dot_period(dot_kind, model.period_ms);
        // Both effects opt into the reviewed native CarryOverDuration rule.
        // Native refresh caps the retained time using the NEW spec's duration.
        let carry_ms = if matches!(
            kind,
            DpsAbilityKind::HemorrhagingStrike | DpsAbilityKind::Hemotoxin
        ) {
            existing
                .as_ref()
                .map(|dot| {
                    dot.expires_ms.saturating_sub(self.common.now_ms).min(
                        (model.duration_ms as f64
                            * ability_param(
                                ability,
                                if kind == DpsAbilityKind::Hemotoxin {
                                    parameter_key!("refreshCarryOverFraction")
                                } else {
                                    parameter_key!("bleedRefreshCarryOverFraction")
                                },
                            ))
                        .round() as u64,
                    )
                })
                .unwrap_or(0)
        } else {
            0
        };
        let expires_ms = self
            .common
            .now_ms
            .saturating_add(model.duration_ms)
            .saturating_add(carry_ms);
        let preserved_phase = existing
            .as_ref()
            .filter(|dot| dot.next_tick_ms > self.common.now_ms)
            .map(|dot| (dot.last_tick_ms, dot.next_tick_ms, dot.scheduled_period_ms));
        let (last_tick_ms, next_tick_ms, scheduled_period_ms) = preserved_phase.unwrap_or((
            self.common.now_ms,
            if kind == DpsAbilityKind::StarfallVolley {
                self.common.now_ms
            } else {
                self.common.now_ms.saturating_add(period)
            },
            period,
        ));
        let bonus_crit = self.roll_spontaneous_combustion(kind);
        let source_snapshot = context
            .source_snapshot
            .unwrap_or_else(|| self.capture_damage_source_snapshot(kind));
        let context = context.with_snapshot(source_snapshot);
        let dot_critical_chance = source_snapshot.critical_chance
            + bonus_crit
            + if is_ardeos_hero_dot(kind) {
                self.common
                    .selected_talents
                    .get("firemage-talent-id-talent11")
                    .map(|talent| param(talent, parameter_key!("dotCriticalStrikeBonus")))
                    .unwrap_or(0.0)
            } else {
                0.0
            };
        self.common.dots.insert(
            (target_index, dot_kind),
            DotState {
                model,
                source: self.ability_damage_source(ability),
                expertise_snapshot: source_snapshot.expertise,
                primary_stat_multiplier_snapshot: source_snapshot.primary_stat_multiplier,
                derived_damage_per_tick: None,
                gunde_rend_buckets: None,
                critical_chance_override: Some(dot_critical_chance.max(0.0)),
                bonus_crit,
                generation,
                started_ms,
                expires_ms,
                stacks,
                context,
                last_tick_ms,
                next_tick_ms,
                scheduled_period_ms,
            },
        );
        self.push_event(
            next_tick_ms,
            CoreEvent::DotTick {
                kind: dot_kind,
                generation,
                target_index,
            },
        );
        self.push_event(
            expires_ms,
            CoreEvent::DotExpire {
                kind: dot_kind,
                generation,
                target_index,
            },
        );
        if !dot_kind.applies_effect_each_tick() {
            self.notify_kindling_of_harmful_effect(
                Some(kind),
                self.ability_damage_source(ability),
                target_index,
                duration_refreshed,
            );
        }
        if kind == DpsAbilityKind::WeaponFrostVolley {
            // CRLinkedEffectComponent creates a separate Harmful amplifier
            // when the active DoT is first added, not on reapplication/ticks.
            self.trigger_kindling_on_harmful_effect_application(
                Some(kind),
                self.ability_damage_source(ability),
                target_index,
            );
        }
        self.trigger_devouring_flame_kindling(
            kind,
            self.ability_damage_source(ability),
            target_index,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_damage_derived_dot(
        &mut self,
        target_index: u32,
        kind: DotKind,
        source: DamageSourceKey,
        model: DotModel,
        added_damage: f64,
        context: DamageContext,
    ) {
        if added_damage <= 0.0 || model.duration_ms == 0 || model.period_ms == 0 {
            return;
        }
        debug_assert!(kind.is_damage_derived());
        // The accumulator creates its own effect from resolved health loss.
        // Original cast multipliers are already included in added_damage.
        let context = context.as_proc();
        self.preserve_overlapping_dot(target_index, kind);
        let generation = self.common.sequence.wrapping_add(1);
        let existing = self
            .common
            .dots
            .get(&(target_index, kind))
            .filter(|dot| dot.expires_ms >= self.common.now_ms)
            .cloned();
        if let Some(expired) = self
            .common
            .dots
            .get(&(target_index, kind))
            .filter(|dot| dot.expires_ms < self.common.now_ms)
            .cloned()
        {
            self.record_dot_uptime(target_index, kind, &expired);
        }
        let base_period = model.period_ms.max(1);
        let period = self.dot_period(kind, base_period);
        let expires_ms = self.common.now_ms.saturating_add(model.duration_ms);
        let (started_ms, next_tick_ms, last_tick_ms, old_remaining_damage) =
            if let Some(dot) = existing.as_ref() {
                let next_tick_ms = dot.next_tick_ms.max(self.common.now_ms.saturating_add(1));
                let remaining_equivalents = remaining_periodic_tick_equivalents(
                    self.common.now_ms,
                    dot.expires_ms,
                    dot.last_tick_ms,
                    next_tick_ms,
                    base_period,
                );
                (
                    dot.started_ms,
                    next_tick_ms,
                    dot.last_tick_ms,
                    dot.derived_damage_per_tick.unwrap_or(0.0) * remaining_equivalents,
                )
            } else {
                (
                    self.common.now_ms,
                    self.common.now_ms.saturating_add(period),
                    self.common.now_ms,
                    0.0,
                )
            };
        let refreshed_equivalents = if existing.is_some() {
            refreshed_periodic_tick_equivalents(
                model.duration_ms,
                next_tick_ms.saturating_sub(self.common.now_ms),
                base_period,
            )
        } else {
            // CrAddToAndRecalculatePeriodicEffect calculates a new effect's
            // magnitude before Wildfire's custom period scaler is installed.
            // These derived effects do not execute once on application.
            model.duration_ms as f64 / base_period as f64
        };
        let damage_per_tick =
            (old_remaining_damage + added_damage) / refreshed_equivalents.max(f64::EPSILON);
        let critical_chance_override = Some(
            self.common
                .selected_talents
                .get("firemage-talent-id-talent11")
                .map(|talent| param(talent, parameter_key!("dotCriticalStrikeBonus")))
                .unwrap_or(0.0),
        );
        self.common.dots.insert(
            (target_index, kind),
            DotState {
                model,
                source,
                // Damage-derived periodic effects receive already resolved
                // source damage and therefore do not reapply source Expertise.
                expertise_snapshot: 0.0,
                primary_stat_multiplier_snapshot: 1.0,
                derived_damage_per_tick: Some(damage_per_tick),
                gunde_rend_buckets: None,
                critical_chance_override,
                bonus_crit: 0.0,
                generation,
                started_ms,
                expires_ms,
                stacks: 1,
                context,
                last_tick_ms,
                next_tick_ms,
                scheduled_period_ms: period,
            },
        );
        self.push_event(
            next_tick_ms,
            CoreEvent::DotTick {
                kind,
                generation,
                target_index,
            },
        );
        self.push_event(
            expires_ms,
            CoreEvent::DotExpire {
                kind,
                generation,
                target_index,
            },
        );
        if kind != DotKind::Ability(DpsAbilityKind::Rend) || existing.is_none() {
            self.trigger_kindling_on_harmful_effect_application(
                kind.ability_kind(),
                source,
                target_index,
            );
        }
    }

    fn roll_spontaneous_combustion(&mut self, kind: DpsAbilityKind) -> f64 {
        let spontaneous_combustion = is_spontaneous_combustion_relevant_dot(kind)
            .then(|| {
                self.common
                    .selected_talents
                    .get("firemage-talent-id-talent9")
            })
            .flatten()
            .map(|talent| {
                let step = param(talent, parameter_key!("criticalChanceStep")).max(f64::EPSILON);
                // Current cooked graph passes Floor(CritChance / step) directly;
                // it no longer grants one extra increment below the first step.
                let steps = (self.profile.critical_strike / step).floor();
                (
                    param(talent, parameter_key!("baseProcChance"))
                        + steps * param(talent, parameter_key!("procChancePerStep")),
                    param(talent, parameter_key!("criticalStrikeBonus")),
                )
            });
        let spontaneous_combustion_procced = spontaneous_combustion.filter(|(chance, _)| {
            self.roll_controlled_random_bool(SPONTANEOUS_COMBUSTION_RANDOM_STREAM_TAG, *chance)
        });
        if spontaneous_combustion_procced.is_some() {
            self.record_talent_proc("firemage-talent-id-talent9");
        }
        spontaneous_combustion_procced
            .map(|(_, critical_strike_bonus)| critical_strike_bonus)
            .unwrap_or(0.0)
    }

    pub(crate) fn dot_tick(&mut self, kind: DotKind, generation: u64, target_index: u32) {
        let Some(dot) = self.common.dots.get(&(target_index, kind)).cloned() else {
            return;
        };
        if dot.generation != generation || self.common.now_ms > dot.expires_ms {
            return;
        }
        if kind.ability_kind() == Some(DpsAbilityKind::StarfallVolley) {
            if self.common.now_ms >= dot.expires_ms {
                return;
            }
            // GA_Bowguy_TargetedAoeDamage sets the first minimum from its
            // cooked Visual Delay (0.2 s), scaled by cast Haste. The actor
            // clears the minimum after applying its first batch. Its maximum
            // and repeating period are fixed when the actor is spawned.
            let remaining = dot.expires_ms - self.common.now_ms;
            let maximum = dot.scheduled_period_ms.min(remaining);
            let visual_delay = self
                .ability(DpsAbilityKind::StarfallVolley)
                .map(|ability| {
                    ability_seconds_parameter(ability, parameter_key!("visualDelaySeconds"))
                })
                .unwrap_or(0);
            let minimum = if self.common.now_ms == dot.started_ms {
                (visual_delay as f64 * dot.scheduled_period_ms as f64
                    / dot.model.period_ms.max(1) as f64)
                    .round() as u64
            } else {
                0
            }
            .min(maximum);
            let delay = self
                .common
                .rng
                .uniform_f64(minimum as f64, maximum as f64)
                .round() as u64;
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                CoreEvent::StarfallHit {
                    generation,
                    target_index,
                },
            );
        } else {
            self.execute_dot_tick(kind, &dot, target_index, 1.0);
        }
        let period = if kind.applies_effect_each_tick() {
            dot.scheduled_period_ms
        } else {
            self.dot_period(kind, dot.model.period_ms)
        };
        let next = self.common.now_ms.saturating_add(period);
        if let Some(active) = self.common.dots.get_mut(&(target_index, kind))
            && active.generation == generation
        {
            active.last_tick_ms = self.common.now_ms;
            active.next_tick_ms = next;
            active.scheduled_period_ms = period;
        }
        // Keep one future wake even beyond the current expiry. A duration
        // extension after the last full tick must retain the periodic clock;
        // without an extension the expiry removes the instance first.
        self.push_event(
            next,
            CoreEvent::DotTick {
                kind,
                generation,
                target_index,
            },
        );
    }

    pub(crate) fn starfall_hit(&mut self, generation: u64, target_index: u32) {
        // A recast may have moved this actor under an overlapping key while
        // its per-target delayed callback was pending.
        let active = self
            .dot_instances(target_index, DpsAbilityKind::StarfallVolley)
            .find(|(_, dot)| dot.generation == generation && self.common.now_ms < dot.expires_ms)
            .map(|(kind, dot)| (kind, dot.clone()));
        let Some((kind, mut dot)) = active else {
            return;
        };
        // The damage effect explicitly recaptures source attributes on every
        // execution; the actor retains its cast context and application clock.
        let snapshot = self.capture_damage_source_snapshot(DpsAbilityKind::StarfallVolley);
        dot.expertise_snapshot = snapshot.expertise;
        dot.primary_stat_multiplier_snapshot = snapshot.primary_stat_multiplier;
        dot.critical_chance_override = Some(snapshot.critical_chance);
        dot.context = dot.context.with_snapshot(snapshot);
        self.execute_dot_tick(kind, &dot, target_index, 1.0);
    }

    pub(crate) fn dot_expire(&mut self, kind: DotKind, generation: u64, target_index: u32) {
        let Some(dot) = self.common.dots.get(&(target_index, kind)).cloned() else {
            return;
        };
        if dot.generation != generation || dot.expires_ms != self.common.now_ms {
            return;
        }
        let timed_terminal_dot = matches!(
            kind.ability_kind(),
            Some(
                DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::SeethingPoison
                    | DpsAbilityKind::VolatilePoison
                    | DpsAbilityKind::Hemotoxin
                    | DpsAbilityKind::Rend
                    | DpsAbilityKind::Slaughter
            )
        );
        // CRASC slot 0x948 (25485623 RVA 0x529e680) executes a partial
        // terminal tick for Dot/Hot unless IgnorePartialTicks is present.
        // DefaultGame.ini requires at least 0.1 seconds of partial progress.
        // Infinite ground effects removed by their actor do not take this path.
        let terminal_elapsed = if timed_terminal_dot {
            // Use the current timer's phase, including a custom scaler changed
            // since the last execution, rather than wall time since that tick.
            dot.scheduled_period_ms
                .saturating_sub(dot.next_tick_ms.saturating_sub(dot.expires_ms))
        } else {
            dot.expires_ms.saturating_sub(dot.last_tick_ms)
        };
        if (kind.is_ardeos_hero_dot()
            || (timed_terminal_dot
                && (dot.next_tick_ms <= dot.expires_ms || terminal_elapsed >= 100)))
            && dot.last_tick_ms < dot.expires_ms
        {
            let fraction = terminal_elapsed as f64 / dot.scheduled_period_ms.max(1) as f64;
            if fraction > f64::EPSILON {
                // The expiration event may have been queued before the final
                // periodic event. Execute a full final period when the two
                // share a timestamp, or the proportional remainder otherwise.
                self.execute_dot_tick(kind, &dot, target_index, fraction.min(1.0));
            }
        }
        if !self.common.dots.contains_key(&(target_index, kind)) {
            return;
        }
        if kind == DotKind::Ability(DpsAbilityKind::VolatilePoison) {
            let delay = self.common.rng.uniform_f64(0.0, 200.0).round() as u64;
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                MaraEvent::VolatileEruption {
                    target_index,
                    context: dot.context,
                },
            );
        }
        if kind.ability_kind() == Some(DpsAbilityKind::StarfallVolley) {
            // Each actor removes the shared capped effect at EndPlay, even
            // when another independent Starfall actor remains alive.
            self.hero.elarion_mut().skylit_grace_active = false;
        }
        self.record_dot_uptime(target_index, kind, &dot);
        self.common.dots.remove(&(target_index, kind));
    }

    pub(crate) fn execute_dot_tick(
        &mut self,
        kind: DotKind,
        dot: &DotState,
        target_index: u32,
        tick_fraction: f64,
    ) {
        let mut live_dot;
        let dot = if matches!(
            kind.ability_kind(),
            Some(
                DpsAbilityKind::SearingBlaze
                    | DpsAbilityKind::EngulfingFlames
                    | DpsAbilityKind::Incinerate
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::CorrosiveSpill
                    | DpsAbilityKind::SeethingPoison
                    | DpsAbilityKind::VolatilePoison
            )
        ) {
            live_dot = dot.clone();
            let snapshot = self.capture_damage_source_snapshot(kind.ability_kind().unwrap());
            live_dot.expertise_snapshot = snapshot.expertise;
            live_dot.primary_stat_multiplier_snapshot = snapshot.primary_stat_multiplier;
            let ardeos_bonus = if kind.is_ardeos_hero_dot() {
                dot.bonus_crit
                    + self
                        .common
                        .selected_talents
                        .get("firemage-talent-id-talent11")
                        .map(|talent| param(talent, parameter_key!("dotCriticalStrikeBonus")))
                        .unwrap_or(0.0)
            } else {
                0.0
            };
            live_dot.critical_chance_override = Some(snapshot.critical_chance + ardeos_bonus);
            live_dot.context = live_dot.context.with_snapshot(snapshot);
            if kind.is_ardeos_hero_dot()
                && let Some(active) = self.common.dots.get_mut(&(target_index, kind))
                && active.generation == dot.generation
            {
                // Native periodic execution recaptures into the active spec;
                // Detonate subsequently samples its most recent captures.
                active.expertise_snapshot = live_dot.expertise_snapshot;
                active.primary_stat_multiplier_snapshot = live_dot.primary_stat_multiplier_snapshot;
                active.critical_chance_override = live_dot.critical_chance_override;
                active.context = live_dot.context;
            }
            &live_dot
        } else {
            dot
        };
        let ability_kind = kind.ability_kind();
        // Rend's signal carries PartialTickFactor, but its manager deliberately
        // consumes a whole bucket without reading that magnitude.
        let fraction = if ability_kind == Some(DpsAbilityKind::Rend) {
            1.0
        } else {
            tick_fraction
        };
        let mut raw_damage = self.dot_raw_damage_per_tick(kind, dot)
            * fraction
            * self.mara_periodic_target_multiplier(kind, target_index);
        if ability_kind == Some(DpsAbilityKind::Rend) {
            raw_damage *= self.gunde_rend_frequency();
            // The target modifier also requires CharacterStatus.Brimborn,
            // which the current Gunde kit never grants. Slaughter instead
            // reads Open Wounds explicitly while converting stored damage.
        }
        if ability_kind == Some(DpsAbilityKind::StarfallVolley)
            && target_index == 0
            && let Some(index) = self.profile.mechanic_indexes.elarion_astronomers_hail
        {
            raw_damage *= mechanic_param(
                &self.profile.mechanics[index],
                parameter_key!("starfallMainTargetDamageMultiplier"),
            );
        }
        let outcome = if kind.is_damage_derived() {
            self.damage_derived_periodic(
                ability_kind,
                dot.source,
                raw_damage,
                dot.critical_chance_override.unwrap_or(0.0),
                target_index,
                dot.context,
            )
        } else {
            let critical_chance = dot.critical_chance_override.unwrap_or_else(|| {
                let talent_bonus = ability_kind
                    .filter(|kind| is_ardeos_hero_dot(*kind))
                    .and_then(|_| {
                        self.common
                            .selected_talents
                            .get("firemage-talent-id-talent11")
                    })
                    .map(|talent| param(talent, parameter_key!("dotCriticalStrikeBonus")))
                    .unwrap_or(0.0);
                self.effective_critical_strike(ability_kind) + dot.bonus_crit + talent_bonus
            });
            self.damage_periodic(
                ability_kind,
                dot.source,
                raw_damage,
                dot.expertise_snapshot,
                dot.primary_stat_multiplier_snapshot,
                dot.model.damage_spread,
                critical_chance,
                dot.model.can_crit,
                target_index,
                dot.context,
            )
        };
        if kind.applies_effect_each_tick() || ability_kind == Some(DpsAbilityKind::Rend) {
            // Keep periodic damage accounting, but notify the application
            // listener once per actor-applied spec, including zero damage.
            self.trigger_kindling_on_harmful_effect_application(
                ability_kind,
                dot.source,
                target_index,
            );
        }
        if ability_kind == Some(DpsAbilityKind::Rend) {
            self.roll_gunde_rend_feathers();
        }
        if ability_kind == Some(DpsAbilityKind::BloodboundSpirit) && outcome.damage > 0.0 {
            let mut transfer = self
                .ability(DpsAbilityKind::BloodboundSpirit)
                .map(|ability| ability_param(ability, parameter_key!("rendTransferFraction")))
                .unwrap_or(0.0);
            if self.common.now_ms < self.hero.gunde().reign_in_blood_until {
                transfer += self
                    .ability(DpsAbilityKind::ReignInBlood)
                    .map(|ability| ability_param(ability, parameter_key!("rendTransferFraction")))
                    .unwrap_or(0.0);
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent11") {
                    transfer += param(talent, parameter_key!("addedRendTransferFraction"));
                }
            }
            self.apply_gunde_rend(target_index, outcome.damage * transfer, dot.context);
        }
        if ability_kind == Some(DpsAbilityKind::HemorrhagingStrike) {
            self.try_mara_from_shadows(target_index, dot.context);
            let energy = self
                .ability(DpsAbilityKind::HemorrhagingStrike)
                .map(|ability| ability_param(ability, parameter_key!("energyPerBleedTick")))
                .unwrap_or(0.0);
            self.gain_mara_energy(energy);
        }
        if ability_kind == Some(DpsAbilityKind::SeethingPoison)
            && let Some(chance) = self
                .common
                .selected_talents
                .get("mara-talent-id-talent10")
                .map(|talent| param(talent, parameter_key!("procChance")))
            && self.roll_controlled_random_bool(
                "RandomStream.Mara.Talent.ChargedDoubleAttackBuilder.AoeDamage",
                chance,
            )
            && let Some(burst) = self.ability(DpsAbilityKind::SeethingBurst).cloned()
        {
            self.record_talent_proc("mara-talent-id-talent10");
            for current_target in 0..self.common.target_count {
                self.damage_hit(
                    &burst,
                    burst.power_coefficient
                        * self.target_count_damage_scaler(&burst)
                        * self.profile.power
                        * self.mara_creeping_death_multiplier(),
                    0.0,
                    true,
                    current_target,
                    dot.context.as_proc(),
                );
            }
        }
        if ability_kind == Some(DpsAbilityKind::SearingBlaze) && outcome.damage > 0.0 {
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent7")
            {
                // Crash and Burn's two exact-event listeners are deliberately
                // asymmetric: the ordinary Searing effect is accepted only by
                // Event.Damage.DoT, while Agonizing Blaze's replacement effect
                // is accepted only by Event.Damage.CriticalHit.DoT.
                let agonizing_blaze = self
                    .common
                    .selected_talents
                    .contains_key("firemage-talent-id-talent3");
                if agonizing_blaze == outcome.critical {
                    self.reduce_cooldown(
                        DpsAbilityKind::FireBall,
                        ms_param(talent, parameter_key!("cooldownReductionSeconds")),
                    );
                }
            }
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent8")
            {
                // Rolling Flames listens to both exact event tags and accepts
                // both Searing effect classes, so every damaging tick applies.
                self.reduce_cooldown(
                    DpsAbilityKind::EngulfingFlames,
                    ms_param(talent, parameter_key!("searingBlazeReductionSeconds")),
                );
            }
        }
        if ability_kind == Some(DpsAbilityKind::SearingBlaze)
            && let Some(active) = self.common.dots.get_mut(&(target_index, kind))
            && active.generation == dot.generation
        {
            let cap = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent3")
                .map(|talent| param_u32(talent, parameter_key!("maximumStacks")))
                .unwrap_or(u32::MAX);
            active.stacks = active.stacks.saturating_add(1).min(cap);
        }
        self.try_firemage_resource_reward(
            ability_kind,
            dot.model.cinder_proc_chance,
            dot.model.cinders_on_proc,
        );
        if kind.is_pyrophibian_relevant() {
            self.try_pyrophibian(outcome.critical, target_index, dot.context);
        }
        if ability_kind == Some(DpsAbilityKind::FireBall)
            && outcome.damage > 0.0
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent18")
        {
            let step = ms_param(talent, parameter_key!("extensionPerTickSeconds"));
            self.increase_dot_duration(
                target_index,
                DotKind::Ability(DpsAbilityKind::SearingBlaze),
                step,
            );
            self.increase_dot_duration(
                target_index,
                DotKind::Ability(DpsAbilityKind::EngulfingFlames),
                step,
            );
            // Slow Burn's damage listener performs its own resource roll once
            // after the extension loop, in addition to the source-ability signal.
            self.try_firemage_resource_reward(
                ability_kind,
                dot.model.cinder_proc_chance,
                dot.model.cinders_on_proc,
            );
        }
        // WaitGameplayEffectApplied_Target also subscribes to periodic
        // execution. Its callback updates the active spec after this tick,
        // including the terminal fractional period, for the next execution.
        if let Some(ability_kind) =
            ability_kind.filter(|kind| is_spontaneous_combustion_relevant_dot(*kind))
        {
            let bonus = self.roll_spontaneous_combustion(ability_kind);
            if let Some(active) = self.common.dots.get_mut(&(target_index, kind)) {
                if let Some(chance) = active.critical_chance_override.as_mut() {
                    *chance += bonus - active.bonus_crit;
                }
                active.bonus_crit = bonus;
            }
        }
        if ability_kind == Some(DpsAbilityKind::Rend) {
            self.advance_gunde_rend_bucket(target_index);
        }
    }

    pub(crate) fn dot_raw_damage_per_tick(&self, kind: DotKind, dot: &DotState) -> f64 {
        let mut damage = dot
            .derived_damage_per_tick
            .unwrap_or(dot.model.power_coefficient * self.profile.power);
        if kind.ability_kind() == Some(DpsAbilityKind::StarfallVolley)
            && let Some(ability) = self.ability(DpsAbilityKind::StarfallVolley)
        {
            damage *= self.target_count_damage_scaler(ability);
        }
        if matches!(
            kind.ability_kind(),
            Some(DpsAbilityKind::Incinerate | DpsAbilityKind::Hemotoxin)
        ) {
            damage *= 1.0 + dot.model.stack_damage_increase * dot.stacks.saturating_sub(1) as f64;
        }
        if kind.ability_kind() == Some(DpsAbilityKind::SearingBlaze)
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent3")
        {
            damage *= 1.0
                + param(talent, parameter_key!("damagePerStack"))
                    * (dot
                        .stacks
                        .saturating_sub(1)
                        .min(param_u32(talent, parameter_key!("maximumStacks"))))
                        as f64;
        }
        damage
    }

    fn mara_periodic_target_multiplier(&self, kind: DotKind, target_index: u32) -> f64 {
        if matches!(
            kind.ability_kind(),
            Some(
                DpsAbilityKind::CorrosiveSpill
                    | DpsAbilityKind::SeethingPoison
                    | DpsAbilityKind::VolatilePoison
            )
        ) {
            return self.mara_creeping_death_multiplier();
        }
        if kind.ability_kind() == Some(DpsAbilityKind::HemorrhagingStrike)
            && self
                .hero
                .mara()
                .seething_poison_until
                .get(target_index as usize)
                .is_some_and(|until| self.common.now_ms < *until)
            && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent16")
        {
            return self.mara_creeping_death_multiplier()
                * param(talent, parameter_key!("seethingBleedDamageMultiplier"));
        }
        if kind.ability_kind() == Some(DpsAbilityKind::HemorrhagingStrike) {
            self.mara_creeping_death_multiplier()
        } else {
            1.0
        }
    }

    pub(crate) fn approximate_dot_average_damage(
        &self,
        kind: DotKind,
        dot: &DotState,
        target_index: u32,
    ) -> f64 {
        let ability_kind = kind.ability_kind();
        // The native approximation (25485623 RVA 0x50c7400) explicitly uses
        // EHitType::Hit and zero spread. It is not a crit-weighted expectation.
        let source_multiplier = if kind.is_damage_derived() {
            1.0
        } else {
            self.outgoing_damage_multiplier_with_expertise(
                ability_kind,
                Some(dot.expertise_snapshot),
                Some(dot.primary_stat_multiplier_snapshot),
                None,
            )
        };
        let raw_damage = (self.dot_raw_damage_per_tick(kind, dot)
            * source_multiplier
            * dot.context.damage_multiplier
            * self.mara_periodic_target_multiplier(kind, target_index))
        .round();
        let damage = (raw_damage * self.target_damage_multiplier(target_index)).round();
        damage / (dot.scheduled_period_ms.max(1) as f64 / 1_000.0)
    }

    pub(crate) fn roll_proc_per_minute(
        &mut self,
        stream_tag: &str,
        real_ppm: f64,
        scale_with_haste: bool,
    ) -> bool {
        let now_seconds = self.common.now_ms as f32 / 1_000.0;
        let haste = if scale_with_haste {
            (1.0 + self.effective_haste()) as f32
        } else {
            1.0
        };
        let state = *self
            .shared
            .proc_per_minute_states
            .entry(stream_tag.to_owned())
            .or_insert(ProcPerMinuteState {
                last_roll_seconds: now_seconds,
                last_proc_seconds: now_seconds,
                has_procced: false,
            });
        let probability = real_ppm_probability(now_seconds, state, real_ppm as f32 * haste);
        let did_proc = self.common.rng.native_15_bit_chance(probability);
        let state = self
            .shared
            .proc_per_minute_states
            .get_mut(stream_tag)
            .expect("proc-per-minute state was inserted before rolling");
        state.last_roll_seconds = now_seconds;
        if did_proc {
            state.last_proc_seconds = now_seconds;
            state.has_procced = true;
        }
        did_proc
    }

    pub(crate) fn roll_dynamic_proc_per_minute(
        &mut self,
        mechanic_index: usize,
        real_ppm: f64,
        scale_with_haste: bool,
    ) -> bool {
        let now_seconds = self.common.now_ms as f32 / 1_000.0;
        let haste = if scale_with_haste {
            (1.0 + self.effective_haste()) as f32
        } else {
            1.0
        };
        let state = self.shared.dynamic_proc_per_minute_states[mechanic_index].unwrap_or(
            ProcPerMinuteState {
                last_roll_seconds: now_seconds,
                last_proc_seconds: now_seconds,
                has_procced: false,
            },
        );
        let probability = real_ppm_probability(now_seconds, state, real_ppm as f32 * haste);
        let did_proc = self.common.rng.native_15_bit_chance(probability);
        self.shared.dynamic_proc_per_minute_states[mechanic_index] = Some(ProcPerMinuteState {
            last_roll_seconds: now_seconds,
            last_proc_seconds: if did_proc {
                now_seconds
            } else {
                state.last_proc_seconds
            },
            has_procced: state.has_procced || did_proc,
        });
        did_proc
    }

    pub(crate) fn roll_controlled_random_bool(
        &mut self,
        stream_tag: &str,
        chance_factor: f64,
    ) -> bool {
        let chance_factor = chance_factor as f32;
        if chance_factor.abs() <= 1.0e-8 {
            return false;
        }
        let state = self
            .shared
            .controlled_random_states
            .entry(stream_tag.to_owned())
            .or_insert(ControlledRandomState {
                failure_threshold: 1.0,
                chance_factor,
                chance_bucket: controlled_random_bucket(chance_factor),
            });
        if state.chance_factor != chance_factor {
            state.chance_factor = chance_factor;
            // The native update path clamps to 1..99, unlike initial insertion
            // which has explicit buckets for probabilities zero and one.
            state.chance_bucket = ((chance_factor * 100.0 + 0.5) as usize).clamp(1, 99);
        }
        state.failure_threshold *= CONTROLLED_RANDOM_FACTORS[state.chance_bucket];
        let failure_threshold = state.failure_threshold;
        let did_proc = self.common.rng.native_uniform_f32() > failure_threshold;
        if did_proc {
            self.shared
                .controlled_random_states
                .get_mut(stream_tag)
                .expect("controlled-random state was inserted before rolling")
                .failure_threshold = 1.0;
        }
        did_proc
    }

    pub(crate) fn damage_hit(
        &mut self,
        ability: &CompiledAbility,
        base: f64,
        bonus_crit: f64,
        can_crit: bool,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        // The ordinary graph explicitly appends each target to its effect
        // context. CopySpawner replaces that context without actor entries;
        // native target-data application passes IncludeActorArray=false.
        let base = if ability.kind == DpsAbilityKind::ArachnidAssault {
            base * self.mara_arachnid_target_multiplier(target_index)
        } else {
            base
        };
        let source_snapshot = context.source_snapshot;
        self.damage_raw_with_spread_key(
            Some(ability.kind),
            self.ability_damage_source(ability),
            base,
            source_snapshot.map(|snapshot| snapshot.expertise),
            source_snapshot.map(|snapshot| snapshot.primary_stat_multiplier),
            source_snapshot.map(|snapshot| snapshot.critical_chance),
            ability.damage_spread,
            bonus_crit,
            can_crit,
            true,
            target_index,
            context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage_raw_key(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        base: f64,
        bonus_crit: f64,
        can_crit: bool,
        trigger_dynamic_mechanics: bool,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        // The modeled raw gear paths use ordinary hero-damage executions. Any
        // remaining preset ambiguity is mean-DPS neutral because this spread is
        // symmetric around one.
        self.damage_raw_with_spread_key(
            ability_kind,
            source,
            base,
            None,
            None,
            None,
            HERO_DAMAGE_SPREAD_WIDTH,
            bonus_crit,
            can_crit,
            trigger_dynamic_mechanics,
            target_index,
            context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage_raw_with_spread_key(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        base: f64,
        expertise_snapshot: Option<f64>,
        primary_stat_multiplier_snapshot: Option<f64>,
        critical_chance_override: Option<f64>,
        damage_spread: f64,
        bonus_crit: f64,
        can_crit: bool,
        trigger_dynamic_mechanics: bool,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source,
            ability_kind,
            context,
            target_index,
            provenance: if trigger_dynamic_mechanics {
                DamageProvenance::Direct
            } else {
                DamageProvenance::Proc
            },
            amount: DamageAmount::Base(base),
            expertise_snapshot,
            primary_stat_multiplier_snapshot,
            damage_spread,
            bonus_crit,
            critical_chance_override,
            can_crit,
            proc_eligible: trigger_dynamic_mechanics,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage_periodic(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        base: f64,
        expertise_snapshot: f64,
        primary_stat_multiplier_snapshot: f64,
        damage_spread: f64,
        critical_chance: f64,
        can_crit: bool,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source,
            ability_kind,
            context,
            target_index,
            provenance: DamageProvenance::Periodic,
            amount: DamageAmount::Base(base),
            expertise_snapshot: Some(expertise_snapshot),
            primary_stat_multiplier_snapshot: Some(primary_stat_multiplier_snapshot),
            damage_spread,
            bonus_crit: 0.0,
            critical_chance_override: Some(critical_chance.max(0.0)),
            can_crit,
            proc_eligible: true,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage_derived_periodic(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        damage: f64,
        critical_chance: f64,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source,
            ability_kind,
            context,
            target_index,
            provenance: DamageProvenance::Periodic,
            amount: DamageAmount::TargetScaled(damage),
            expertise_snapshot: None,
            primary_stat_multiplier_snapshot: None,
            damage_spread: 0.0,
            bonus_crit: 0.0,
            critical_chance_override: Some(critical_chance.max(0.0)),
            can_crit: true,
            proc_eligible: true,
        })
    }

    pub(crate) fn damage_detonate(
        &mut self,
        ability: &CompiledAbility,
        damage: f64,
        critical_chance: f64,
        target_index: u32,
        context: DamageContext,
    ) -> DamageOutcome {
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source: self.ability_damage_source(ability),
            ability_kind: Some(ability.kind),
            context,
            target_index,
            provenance: DamageProvenance::Direct,
            // The graph divides out the incoming multiplier before native raw
            // damage rounding. Target scaling follows that rounding, so these
            // two multiplications cannot be algebraically cancelled.
            amount: DamageAmount::TargetScaled(
                damage / self.target_damage_multiplier(target_index),
            ),
            expertise_snapshot: None,
            primary_stat_multiplier_snapshot: None,
            damage_spread: ability.damage_spread,
            bonus_crit: 0.0,
            critical_chance_override: Some(critical_chance.max(0.0)),
            can_crit: true,
            proc_eligible: true,
        })
    }

    pub(crate) fn damage_unscaled_key(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        damage: f64,
        target_index: u32,
        provenance: DamageProvenance,
        context: DamageContext,
    ) -> DamageOutcome {
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source,
            ability_kind,
            context,
            target_index,
            provenance,
            amount: DamageAmount::Unscaled(damage),
            expertise_snapshot: None,
            primary_stat_multiplier_snapshot: None,
            damage_spread: 0.0,
            bonus_crit: 0.0,
            critical_chance_override: None,
            can_crit: false,
            proc_eligible: false,
        })
    }

    pub(crate) fn explode_shadow_mark(&mut self, target_index: u32) {
        let Some(mark) = self.shared.shadow_marks.remove(&target_index) else {
            return;
        };
        if mark.accumulated_damage <= 0.0 {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::WeaponShadowMark).cloned() else {
            return;
        };
        let cap = self.current_primary_attribute()
            * ability
                .parameters
                .get(parameter_key!("maximumPowerCoefficient"))
                .unwrap_or(0.0);
        let damage = mark.accumulated_damage.min(cap).max(0.0);
        if damage <= 0.0 {
            return;
        }
        let snapshot = self.capture_damage_source_snapshot(ability.kind);
        self.emit_outgoing_damage(OutgoingDamageEvent {
            source: self.ability_damage_source(&ability),
            ability_kind: Some(DpsAbilityKind::WeaponShadowMark),
            context: mark.context.with_snapshot(snapshot),
            target_index,
            provenance: DamageProvenance::Explosion,
            // The monitor passes an already absolute stored-damage magnitude
            // into CRUserDefinedDamageCalculation. HeroMagicalDirect reapplies
            // Expertise and generic source/target multipliers, but it does not
            // multiply that set-by-caller value by PrimaryAttribute again.
            amount: DamageAmount::Base(damage),
            expertise_snapshot: Some(snapshot.expertise),
            primary_stat_multiplier_snapshot: Some(1.0),
            damage_spread: ability.damage_spread,
            bonus_crit: 0.0,
            critical_chance_override: Some(snapshot.critical_chance),
            can_crit: true,
            proc_eligible: true,
        });
    }

    pub(crate) fn emit_outgoing_damage(&mut self, event: OutgoingDamageEvent) -> DamageOutcome {
        if !self.common.execution.charge(1) {
            return DamageOutcome::default();
        }
        let mut multiplier = match event.amount {
            DamageAmount::Base(_) => self.outgoing_damage_multiplier_with_expertise(
                event.ability_kind,
                event.expertise_snapshot,
                event.primary_stat_multiplier_snapshot,
                event
                    .context
                    .source_snapshot
                    .map(|snapshot| snapshot.hero_damage_scale),
            ),
            DamageAmount::TargetScaled(_) | DamageAmount::Unscaled(_) => 1.0,
        };
        let chance = event
            .critical_chance_override
            .map(|chance| chance + event.bonus_crit)
            .unwrap_or_else(|| {
                self.effective_critical_strike_for_category(
                    event.ability_kind,
                    event.context.category(event.ability_kind),
                ) + event.bonus_crit
            })
            .max(0.0);
        // CRDamageCalculationBase resolves the hit outcome before drawing the
        // independent damage-spread roll. It still draws the outcome roll for
        // guaranteed grievous critical strikes.
        let miss_chance = if matches!(event.amount, DamageAmount::Base(_))
            && event.provenance == DamageProvenance::Direct
            && matches!(
                event.ability_kind,
                Some(
                    DpsAbilityKind::TariqAttack
                        | DpsAbilityKind::WildSwing
                        | DpsAbilityKind::ElarionShoot
                        | DpsAbilityKind::MaraAttack
                        | DpsAbilityKind::GundeAttack
                )
            ) {
            0.05
        } else {
            0.0
        };
        let outcome = if event.can_crit || miss_chance > 0.0 {
            self.common
                .rng
                .damage_outcome(miss_chance, if event.can_crit { chance } else { 0.0 })
        } else {
            HitOutcome::Hit
        };
        let critical = outcome == HitOutcome::Critical;
        if critical {
            multiplier *= self.effective_critical_multiplier_for_category(
                event.context.category(event.ability_kind),
            ) + (chance - 1.0).max(0.0) * GLOBAL_GRIEVOUS_CRIT_SCALAR;
        }
        let spread_width = event.damage_spread.max(0.0);
        if spread_width > 0.0 {
            multiplier *= 1.0 + self.common.rng.damage_spread_roll() * spread_width;
        }
        if outcome == HitOutcome::Miss {
            // Native 0x50c0400 skips damage/pre-health calculation; 0x52b53a0
            // emits Event.Damage.Missed. Parent Event.Damage subscriptions
            // still receive it: Elarion updates cached tags, and Emerald can
            // roll without requiring health loss. Exact hit/crit listeners do
            // not receive it.
            // The harmful effect itself was still applied (Kindling listens
            // to applications). A miss is not a landed zero-damage hit.
            self.try_elarion_mark_proc(event.source, event.target_index, false, 0.0, event.context);
            self.trigger_generic_damage_listeners(
                event.ability_kind,
                event.source,
                0.0,
                false,
                event.target_index,
                event.context,
            );
            self.trigger_kindling_on_harmful_effect_application(
                event.ability_kind,
                event.source,
                event.target_index,
            );
            return DamageOutcome::default();
        }
        let raw = match event.amount {
            DamageAmount::Base(base)
            | DamageAmount::TargetScaled(base)
            | DamageAmount::Unscaled(base) => base,
        };
        let mut damage = (raw * multiplier).max(0.0);
        if event.proc_eligible {
            damage *= event.context.damage_multiplier;
        }
        // CRDamageCalculationBase (25485623 RVA 0x50ea8b0) rounds RawDamage
        // half away from zero before 0x50ea980 applies target modifiers.
        // The Empty preset also rounds, including transferred damage magnitudes.
        damage = damage.round();
        if !matches!(event.amount, DamageAmount::Unscaled(_)) {
            damage *= self.target_damage_multiplier(event.target_index);
        }
        // The final damage helper (RVA 0x50ea7d0) rounds after target modifiers.
        damage = damage.round();
        // The native pre-health callbacks (0x50b7e30 / 0x50f6f50) run after
        // both execution rounding boundaries. Their modified values are not
        // passed through the raw/target calculation again.
        if event.proc_eligible
            && event.context.category(event.ability_kind) == Some(AbilityCategory::Power)
        {
            damage *=
                event.context.power_damage_multiplier * self.resolved_power_blessing_multiplier();
        }
        if event.proc_eligible
            && damage > 0.0
            && event.context.category(event.ability_kind) == Some(AbilityCategory::Basic)
        {
            let coefficient_parameter = if event.provenance == DamageProvenance::Periodic {
                parameter_key!("periodicPowerCoefficient")
            } else {
                parameter_key!("powerCoefficient")
            };
            if !self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.basic_damage_bonus.len())
            {
                return DamageOutcome::default();
            }
            damage += self
                .profile
                .mechanic_indexes
                .basic_damage_bonus
                .iter()
                .map(|index| &self.profile.mechanics[*index])
                .map(|mechanic| {
                    self.current_primary_attribute()
                        * mechanic_param(mechanic, coefficient_parameter)
                })
                .sum::<f64>();
        }
        self.record_damage(event.source, damage, chance, critical, event.target_index);
        self.handle_mara_damage_event(&event, damage);
        // Winter's Blessing registers its exact listener when cast, after
        // equipment passives. Native reverse multicast order visits it first.
        self.accumulate_rime_blessing_heal(&event, damage);
        self.trigger_exact_damage_listeners(
            event.source,
            damage,
            event.target_index,
            event.context,
        );
        if damage > 0.0 {
            // First Strike receives the health-change event after this hit has
            // resolved. Array_AddUnique admits each living target once, so the
            // triggering hit is unbuffed and a newly touched target refreshes
            // the self-buff for subsequent damage.
            self.trigger_first_damage_expertise(event.target_index);
        }
        self.try_elarion_mark_proc(
            event.source,
            event.target_index,
            critical,
            damage,
            event.context,
        );
        if critical
            && let Some(kind) = event
                .ability_kind
                .filter(|kind| ability_category(*kind) == AbilityCategory::Weapon)
        {
            self.trigger_weapon_critical_cooldown_reduction(kind);
        }
        // The granted monitor listens on Source Actor's ASC, not on the
        // marked target. Every outgoing health-loss event feeds each active
        // mark, including explosions on other targets. A detonating mark is
        // removed before its hit dispatches, so it cannot feed itself.
        if let Some((fraction, cap)) = self.ability(DpsAbilityKind::WeaponShadowMark).map(|mark| {
            (
                ability_param(mark, parameter_key!("accumulationFraction")),
                self.current_primary_attribute()
                    * ability_param(mark, parameter_key!("maximumPowerCoefficient")),
            )
        }) {
            if !self.charge_work_units(self.shared.shadow_marks.len() as u64) {
                return DamageOutcome { damage, critical };
            }
            let targets = self.shared.shadow_marks.keys().copied().collect::<Vec<_>>();
            for target in targets {
                let should_explode = self
                    .shared
                    .shadow_marks
                    .get_mut(&target)
                    .filter(|mark| self.common.now_ms < mark.until_ms)
                    .is_some_and(|mark| {
                        mark.accumulated_damage += damage * fraction;
                        mark.accumulated_damage >= cap
                    });
                if should_explode {
                    self.explode_shadow_mark(target);
                }
            }
        }
        self.shared.spirit = (self.shared.spirit + training_dummy_damage_spirit(damage))
            .min(self.profile.max_spirit);
        if event.proc_eligible {
            self.try_basic_to_aoe(&event, damage);
            self.try_dynamic_on_hit(
                event.ability_kind,
                damage,
                event.target_index,
                event.context,
            );
        }
        #[cfg(test)]
        if self.test_heretic_before_generic
            && damage > 0.0
            && self.profile.damage_sources[event.source.0].id == "gear:DynamicItemAbilityRank.14"
        {
            self.emit_positive_healing(event.source, false);
        }
        self.trigger_generic_damage_listeners(
            event.ability_kind,
            event.source,
            damage,
            critical,
            event.target_index,
            event.context,
        );
        if event.provenance != DamageProvenance::Periodic {
            self.trigger_kindling_on_harmful_effect_application(
                event.ability_kind,
                event.source,
                event.target_index,
            );
        }
        DamageOutcome { damage, critical }
    }

    pub(crate) fn try_basic_to_aoe(&mut self, event: &OutgoingDamageEvent, triggering_damage: f64) {
        if triggering_damage <= 0.0
            || event.context.category(event.ability_kind) != Some(AbilityCategory::Basic)
        {
            return;
        }
        if !event.context.cast_proc_eligible {
            return;
        }
        let mechanic_count = self.profile.mechanic_indexes.basic_to_aoe.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.basic_to_aoe[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            let threshold = mechanic_u32(&mechanic, parameter_key!("hitThreshold")).max(1);
            let counter = &mut self.shared.dynamic_counters[mechanic.index];
            *counter = counter.saturating_add(1);
            if *counter < threshold {
                continue;
            }
            *counter = 0;

            // The listener reads live CritChance, not the triggering spec's
            // critical chance (which can include ability-specific bonuses).
            let critical_bonus = self.effective_critical_strike(None).clamp(
                0.0,
                mechanic_param(&mechanic, parameter_key!("criticalStrikeCap")),
            );
            let damage = triggering_damage
                * (1.0 + critical_bonus)
                * multi_target_damage_falloff(
                    self.common.target_count,
                    mechanic_param(
                        &mechanic,
                        parameter_key!("targetCountDamageScalingThreshold"),
                    ),
                );
            let registered_targets = self
                .common
                .actor_registration_order
                .enemy_targets()
                .to_vec();
            if !self.common.execution.charge_usize(registered_targets.len()) {
                return;
            }
            for target_index in registered_targets {
                self.damage_unscaled_key(
                    None,
                    mechanic.damage_source,
                    damage,
                    target_index,
                    DamageProvenance::Proc,
                    event.context.as_proc(),
                );
            }
            self.record_dynamic_random_proc(&mechanic);
        }
    }

    pub(crate) fn outgoing_damage_multiplier_with_expertise(
        &self,
        ability_kind: Option<DpsAbilityKind>,
        expertise_snapshot: Option<f64>,
        primary_stat_multiplier_snapshot: Option<f64>,
        hero_damage_scale_snapshot: Option<f64>,
    ) -> f64 {
        let expertise = expertise_snapshot.unwrap_or_else(|| self.effective_expertise());
        let mut multiplier = (1.0 + expertise).max(0.0);
        multiplier *=
            primary_stat_multiplier_snapshot.unwrap_or_else(|| self.effective_power_multiplier());
        multiplier *= self.source_damage_multiplier(ability_kind);
        if !self.common.execution.charge_usize(
            self.profile
                .mechanic_indexes
                .outgoing_damage_multiplier
                .len(),
        ) {
            return multiplier;
        }
        for index in &self.profile.mechanic_indexes.outgoing_damage_multiplier {
            let mechanic = &self.profile.mechanics[*index];
            let applies = mechanic.ability_kind.is_none() || mechanic.ability_kind == ability_kind;
            match mechanic.kind {
                CompiledMechanicKind::DamageMultiplier if applies => {
                    multiplier *= 1.0 + mechanic_param(mechanic, parameter_key!("damageIncrease"));
                }
                CompiledMechanicKind::AbilityDamageMultiplier if applies => {
                    multiplier *= 1.0 + mechanic_param(mechanic, parameter_key!("damageIncrease"));
                }
                _ => {}
            }
        }
        multiplier *=
            hero_damage_scale_snapshot.unwrap_or_else(|| self.hero_damage_scale(ability_kind));
        multiplier
    }

    pub(crate) fn target_damage_multiplier(&self, target_index: u32) -> f64 {
        let engulfing_stacks = if self
            .profile
            .mechanic_indexes
            .target_damage_multiplier
            .is_empty()
        {
            0
        } else {
            self.dot_instances(target_index, DpsAbilityKind::EngulfingFlames)
                .filter(|(_, dot)| dot.expires_ms >= self.common.now_ms)
                .map(|(_, dot)| dot.stacks.max(1))
                .sum::<u32>()
        };
        let mut multiplier = if engulfing_stacks == 0 {
            1.0
        } else {
            self.profile
                .mechanic_indexes
                .target_damage_multiplier
                .iter()
                .map(|index| &self.profile.mechanics[*index])
                .fold(1.0, |multiplier, mechanic| {
                    multiplier
                        * mechanic_param(
                            mechanic,
                            parameter_key!("engulfingTargetIncomingMultiplier"),
                        )
                        .max(1.0)
                        .powi(engulfing_stacks as i32)
                })
        };
        if let HeroState::Tariq(state) = &self.hero
            && state
                .slayers_mosh_until
                .get(target_index as usize)
                .is_some_and(|until| *until > self.common.now_ms)
            && let Some(index) = self.profile.mechanic_indexes.tariq_slayers_mosh
        {
            multiplier *= mechanic_param(
                &self.profile.mechanics[index],
                parameter_key!("leapTargetDamageMultiplier"),
            )
            .max(1.0);
        }
        if let HeroState::Elarion(state) = &self.hero
            && state
                .shimmer_until
                .get(target_index as usize)
                .is_some_and(|until| *until > self.common.now_ms)
            && let Some(index) = self.profile.mechanic_indexes.elarion_shimmer
        {
            let stacks = state.shimmer_stacks[target_index as usize];
            // Native ComputeStackedModifierMagnitude uses bias +
            // (magnitude - bias) * count for Multiplicitive (bias = 1).
            let magnitude = mechanic_param(
                &self.profile.mechanics[index],
                parameter_key!("shimmerDamageMultiplier"),
            );
            multiplier *= 1.0 + (magnitude - 1.0) * f64::from(stacks);
        }
        multiplier
    }
}

// Build 25485623: TargetDummy BaseHealth=1337, Stamina=0, instance and
// difficulty health multipliers=1, SpiritPointValue=3. The passive grants
// min(abs(TotalHealthChange), MaxHealth) / MaxHealth * SpiritPointValue * 0.25.
// These are fixed encounter semantics, not a hero stat or a configurable
// damage coefficient. See docs/static-mechanics-analysis.md for native proof.
fn training_dummy_damage_spirit(damage: f64) -> f64 {
    let resolved_damage = damage as f32;
    let fraction = f64::from(resolved_damage.max(0.0)).min(1337.0) / 1337.0;
    // Kismet promotes the captured attributes to double, then narrows the
    // set-by-caller value to float before adding it to SpiritPoints.
    f64::from((fraction * 3.0 * 0.25) as f32)
}
