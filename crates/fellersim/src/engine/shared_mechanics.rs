use crate::*;

impl Iteration<'_> {
    pub(crate) fn wayfarer_mechanic(&self) -> Option<Arc<CompiledMechanic>> {
        self.profile
            .mechanic_indexes
            .wayfarer
            .map(|index| Arc::clone(&self.profile.mechanics[index]))
    }

    pub(crate) fn start_wayfarer_cycle(&mut self) {
        let Some(mechanic) = self.wayfarer_mechanic() else {
            return;
        };
        let interval = seconds_parameter(&mechanic, parameter_key!("intervalSeconds"));
        if interval == 0 {
            return;
        }
        self.shared.wayfarer_generation = self.shared.wayfarer_generation.wrapping_add(1);
        self.shared.wayfarer_next_ms = self.common.now_ms.saturating_add(interval);
        self.push_event(
            self.shared.wayfarer_next_ms,
            SharedEvent::WayfarerProc {
                generation: self.shared.wayfarer_generation,
            },
        );
    }

    pub(crate) fn proc_wayfarer(&mut self) {
        let Some(mechanic) = self.wayfarer_mechanic() else {
            return;
        };
        self.activate_dynamic_buff(
            &mechanic,
            seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
            1,
            0.0,
        );
        self.record_dynamic_proc(&mechanic);
        self.start_wayfarer_cycle();
    }

    pub(crate) fn reduce_wayfarer_cycle(&mut self, mechanic: &CompiledMechanic, cooldown_ms: u64) {
        if self.shared.wayfarer_next_ms <= self.common.now_ms {
            return;
        }
        let denominator =
            mechanic_param(mechanic, parameter_key!("coreCooldownDenominatorSeconds"))
                .max(f64::EPSILON);
        let multiplier = mechanic_param(mechanic, parameter_key!("coreCooldownFractionMultiplier"));
        let minimum = mechanic_param(mechanic, parameter_key!("minimumReductionSeconds")).max(0.0);
        let reduction_ms = ((cooldown_ms as f64 / 1_000.0 / denominator * multiplier).max(minimum)
            * 1_000.0)
            .round()
            .max(0.0) as u64;
        let remaining = self
            .shared
            .wayfarer_next_ms
            .saturating_sub(self.common.now_ms);
        self.shared.wayfarer_next_ms = self
            .common
            .now_ms
            .saturating_add(remaining.saturating_sub(reduction_ms));
        self.shared.wayfarer_generation = self.shared.wayfarer_generation.wrapping_add(1);
        self.push_event(
            self.shared.wayfarer_next_ms,
            SharedEvent::WayfarerProc {
                generation: self.shared.wayfarer_generation,
            },
        );
    }

    pub(crate) fn trigger_dynamic_on_cast(
        &mut self,
        ability: &CompiledAbility,
        mut context: DamageContext,
    ) -> DamageContext {
        let category = context.category(Some(ability.kind)).unwrap();
        let mechanic_count = self.profile.mechanic_indexes.on_cast.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return context;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.on_cast[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            let source = mechanic.source_id.as_str();
            match source {
                "ItemTrait.ID.Wolf" => {
                    let (trigger, fraction) = match self.profile.contract.hero {
                        HeroIdentity::Ardeos => (
                            DpsAbilityKind::Wildfire,
                            parameter_key!("mediumHealingHealthFraction"),
                        ),
                        HeroIdentity::Rime => (
                            DpsAbilityKind::FlightOfTheNavir,
                            parameter_key!("largeHealingHealthFraction"),
                        ),
                        HeroIdentity::Tariq => (
                            DpsAbilityKind::ThunderCall,
                            parameter_key!("mediumHealingHealthFraction"),
                        ),
                        HeroIdentity::Elarion => (
                            DpsAbilityKind::LunarlightMark,
                            parameter_key!("smallHealingHealthFraction"),
                        ),
                        HeroIdentity::Gunde => (
                            DpsAbilityKind::Rupture,
                            parameter_key!("mediumHealingHealthFraction"),
                        ),
                        HeroIdentity::Mara => (
                            DpsAbilityKind::MaidenOfDeath,
                            parameter_key!("mediumHealingHealthFraction"),
                        ),
                    };
                    if ability.kind == trigger && mechanic_param(&mechanic, fraction) > 0.0 {
                        self.record_dynamic_proc(&mechanic);
                        // The conditional Empty-preset heal retains the untagged
                        // Perk context. Full-health overhealing still emits events.
                        self.apply_positive_healing(mechanic.damage_source, false);
                    }
                }
                "DynamicItemAbilityRank.01" if category == AbilityCategory::Basic => {
                    let maximum = mechanic_u32(&mechanic, parameter_key!("maximumStacks")).max(1);
                    let counter = &mut self.shared.dynamic_counters[mechanic.index];
                    *counter = counter.saturating_add(1).min(maximum);
                }
                "DynamicItemAbilityRank.01" if category == AbilityCategory::Power => {
                    let stacks = std::mem::take(&mut self.shared.dynamic_counters[mechanic.index]);
                    if stacks > 0 {
                        context.multiply_power_damage(
                            1.0 + stacks as f64
                                * mechanic_param(
                                    &mechanic,
                                    parameter_key!("damageIncreasePerStack"),
                                ),
                        );
                    }
                }
                "DynamicItemAbilityRank.02" if category == AbilityCategory::Power => {
                    self.activate_dynamic_buff(
                        &mechanic,
                        seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                        1,
                        0.0,
                    );
                }
                "DynamicItemAbilityRank.04" if category == AbilityCategory::Power => {
                    if self.roll_controlled_random_bool(
                        POWER_CHANCE_SPIRIT_RANDOM_STREAM_TAG,
                        mechanic_param(&mechanic, parameter_key!("procChance")),
                    ) {
                        self.shared.spirit = (self.shared.spirit
                            + mechanic_param(&mechanic, parameter_key!("spiritGain")))
                        .min(self.profile.max_spirit);
                        self.record_dynamic_random_proc(&mechanic);
                    }
                }
                "DynamicItemAbilityRank.07" if category == AbilityCategory::Core => {
                    let proc_chance = self
                        .effective_critical_strike_for_category(Some(ability.kind), Some(category))
                        .min(1.0);
                    if self.roll_controlled_random_bool(
                        CRIT_CHANCE_VERDICT_RANDOM_STREAM_TAG,
                        proc_chance,
                    ) {
                        // All six maintained heroes use the DPS role. The role-selection roll
                        // determines whether the same proc also heals, but all
                        // successful controlled-random rolls deal damage.
                        let role_selection_roll = self.common.rng.native_uniform_f32();
                        let snapshot = self.capture_damage_source_snapshot(ability.kind);
                        let maximum_targets =
                            mechanic_u32(&mechanic, parameter_key!("maximumTargets")).max(1);
                        for target_index in self.random_target_set(maximum_targets) {
                            self.damage_raw_with_spread_key(
                                Some(ability.kind),
                                mechanic.damage_source,
                                self.profile.power
                                    * mechanic_param(&mechanic, parameter_key!("powerCoefficient")),
                                Some(snapshot.expertise),
                                Some(snapshot.primary_stat_multiplier),
                                Some(snapshot.critical_chance),
                                HERO_DAMAGE_SPREAD_WIDTH,
                                0.0,
                                true,
                                false,
                                target_index,
                                context.as_proc().with_snapshot(snapshot),
                            );
                        }
                        if role_selection_roll > 0.8 {
                            let critical = self.resolve_healing_critical(Some(ability.kind));
                            self.apply_positive_healing(mechanic.damage_source, critical);
                        }
                        self.record_dynamic_random_proc(&mechanic);
                    }
                }
                "DynamicItemAbilityRank.08" if category == AbilityCategory::Major => {
                    self.activate_dynamic_buff(
                        &mechanic,
                        seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                        1,
                        0.0,
                    );
                }
                "DynamicItemAbilityRank.12" if category == AbilityCategory::Core => {
                    self.reduce_wayfarer_cycle(&mechanic, ability.cooldown_ms);
                }
                "DynamicItemAbilityRank.14" if category == AbilityCategory::Core => {
                    if self.roll_controlled_random_bool(
                        DRAIN_HEALTH_RANDOM_STREAM_TAG,
                        mechanic_param(&mechanic, parameter_key!("procChance")),
                    ) {
                        // The stationary group is admitted by the range
                        // invariant. Its co-located closest-target tie is
                        // total-DPS-equivalent for this resolved Empty-preset
                        // hit, so index zero is the deterministic tie break.
                        let outcome = self.damage_unscaled_key(
                            Some(ability.kind),
                            mechanic.damage_source,
                            self.current_primary_attribute()
                                * mechanic_param(&mechanic, parameter_key!("powerCoefficient")),
                            0,
                            DamageProvenance::Proc,
                            context.as_proc(),
                        );
                        #[cfg(test)]
                        let heal_after_generic = !self.test_heretic_before_generic;
                        #[cfg(not(test))]
                        let heal_after_generic = true;
                        if outcome.damage > 0.0 && heal_after_generic {
                            self.apply_positive_healing(mechanic.damage_source, false);
                        }
                        self.record_dynamic_random_proc(&mechanic);
                    }
                }
                "ItemTrait.ID.AbilityToIncreasedMainStat"
                    if is_primary_skill_commit(ability.kind) =>
                {
                    // The passive does not even attempt its shared real-PPM
                    // roll while the Main Stat buff is active.
                    if self
                        .shared
                        .dynamic_buffs
                        .get(mechanic.index)
                        .and_then(Option::as_ref)
                        .is_some_and(|buff| buff.until_ms > self.common.now_ms)
                    {
                        continue;
                    }
                    if self.roll_dynamic_proc(&mechanic) {
                        let expires_ms = self.shared.dynamic_counter_expirations_ms[mechanic.index];
                        if expires_ms <= self.common.now_ms {
                            self.shared.dynamic_counters[mechanic.index] = 0;
                        }
                        let required =
                            mechanic_u32(&mechanic, parameter_key!("requiredStacks")).max(1);
                        let counter = &mut self.shared.dynamic_counters[mechanic.index];
                        *counter = counter.saturating_add(1);
                        self.shared.dynamic_counter_expirations_ms[mechanic.index] =
                            self.common.now_ms.saturating_add(seconds_parameter(
                                &mechanic,
                                parameter_key!("stackDurationSeconds"),
                            ));
                        if *counter >= required {
                            *counter = 0;
                            self.shared.dynamic_counter_expirations_ms[mechanic.index] = 0;
                            self.activate_dynamic_buff(
                                &mechanic,
                                seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                                1,
                                0.0,
                            );
                        }
                    }
                }
                "ItemTrait.ID.CommitToHasteRatingAndWeaponCooldown"
                    if is_offensive_skill_commit(ability.kind) =>
                {
                    if self.roll_dynamic_proc(&mechanic) {
                        self.reduce_weapon_cooldowns(seconds_parameter(
                            &mechanic,
                            parameter_key!("weaponCooldownReductionSeconds"),
                        ));
                        self.activate_dynamic_buff(
                            &mechanic,
                            seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                            1,
                            0.0,
                        );
                    }
                }
                "ItemTrait.ID.GemCooldownRecoveryOnAbilityProc" | "setb-proc-hdt"
                    if source != "setb-proc-hdt" || is_dark_prophecy_commit(ability.kind) =>
                {
                    if self.roll_dynamic_proc(&mechanic) {
                        self.activate_dynamic_buff(
                            &mechanic,
                            seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                            1,
                            0.0,
                        );
                    }
                }
                "ItemTrait.ID.GemPulsatingOnAbilityTotemProc"
                    if category == AbilityCategory::Spirit =>
                {
                    self.spawn_aurastone(&mechanic);
                }
                "ItemTrait.ID.OffensiveAbilityHasteRatingStacking"
                    if is_hunters_focus_commit(ability.kind) =>
                {
                    let current = self
                        .shared
                        .dynamic_buffs
                        .get(mechanic.index)
                        .and_then(Option::as_ref)
                        .filter(|buff| buff.until_ms > self.common.now_ms)
                        .map(|buff| buff.stacks)
                        .unwrap_or(0);
                    self.activate_dynamic_buff(
                        &mechanic,
                        seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                        current
                            .saturating_add(1)
                            .min(mechanic_u32(&mechanic, parameter_key!("maximumStacks")).max(1)),
                        0.0,
                    );
                }
                "ItemTrait.ID.GemWhirlwindProc" if is_hunters_focus_commit(ability.kind) => {
                    if self.roll_dynamic_proc(&mechanic) {
                        self.spawn_ruby_storm(&mechanic);
                    }
                }
                "ItemTrait.ID.OffensiveAbilityToHighestStatBuff"
                    if is_offensive_skill_commit(ability.kind) =>
                {
                    if self.roll_controlled_dynamic_proc(
                        &mechanic,
                        NAVIGATORS_INTUITION_RANDOM_STREAM_TAG,
                    ) {
                        let selected_secondary = self.highest_secondary_rating_code();
                        self.activate_dynamic_buff(
                            &mechanic,
                            seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                            1,
                            selected_secondary as f64,
                        );
                    }
                }
                "ItemTrait.ID.CooldownRecoveryOnWeaponAbility"
                    if category == AbilityCategory::Weapon =>
                {
                    self.activate_dynamic_buff(
                        &mechanic,
                        ((ability_default_cooldown_ms(ability) as f64)
                            * mechanic_param(
                                &mechanic,
                                parameter_key!("durationWeaponCooldownFraction"),
                            ))
                        .round() as u64,
                        1,
                        0.0,
                    );
                }
                "ItemTrait.ID.WeaponAndSpiritPoints" if category == AbilityCategory::Weapon => {
                    let gain =
                        mechanic_param(&mechanic, parameter_key!("spiritCooldownMultiplier"))
                            * (ability_default_cooldown_ms(ability) as f64 / 1_000.0)
                            / mechanic_param(&mechanic, parameter_key!("spiritCooldownDivider"))
                                .max(f64::EPSILON);
                    self.shared.spirit = (self.shared.spirit + gain).min(self.profile.max_spirit);
                }
                "ItemTrait.ID.WeaponAndSpiritPoints" if category == AbilityCategory::Spirit => {
                    self.reduce_weapon_cooldown_fraction(mechanic_param(
                        &mechanic,
                        parameter_key!("weaponCooldownReductionFraction"),
                    ));
                }
                "ItemTrait.ID.WeaponCritChanceCooldownReduction"
                    if category == AbilityCategory::Weapon =>
                {
                    self.shared.dynamic_counters[mechanic.index] = 0;
                    self.shared.dynamic_pending_cooldown_reductions_ms[mechanic.index] = 0;
                    let generation = &mut self.shared.dynamic_event_generations[mechanic.index];
                    *generation = generation.wrapping_add(1);
                }
                "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease"
                    if category == AbilityCategory::Weapon =>
                {
                    self.activate_dynamic_buff(
                        &mechanic,
                        ((ability_default_cooldown_ms(ability) as f64)
                            * mechanic_param(
                                &mechanic,
                                parameter_key!("durationWeaponCooldownFraction"),
                            ))
                        .round() as u64,
                        1,
                        0.0,
                    );
                }
                _ => {}
            }
        }
        context
    }

    pub(crate) fn refresh_first_strike_expertise(&mut self) {
        let count = self.profile.mechanic_indexes.first_damage_expertise.len();
        if !self.common.execution.charge_usize(count) {
            return;
        }
        for position in 0..count {
            let index = self.profile.mechanic_indexes.first_damage_expertise[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            self.activate_dynamic_buff(
                &mechanic,
                seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                1,
                0.0,
            );
        }
    }

    pub(crate) fn trigger_first_damage_expertise(&mut self, target_index: u32) {
        let mechanic_count = self.profile.mechanic_indexes.first_damage_expertise.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.first_damage_expertise[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            let slot = self.mechanic_target_slot(mechanic.index, target_index);
            if !std::mem::replace(&mut self.shared.dynamic_touched_targets[slot], true) {
                self.activate_dynamic_buff(
                    &mechanic,
                    seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                    1,
                    0.0,
                );
            }
        }
    }

    pub(crate) fn trigger_weapon_critical_cooldown_reduction(
        &mut self,
        ability_kind: DpsAbilityKind,
    ) {
        let cooldown_ms = self
            .ability(ability_kind)
            .map(|ability| ability.cooldown_ms)
            .unwrap_or(0);
        let mechanic_count = self.profile.mechanic_indexes.weapon_critical_cooldown.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.weapon_critical_cooldown[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            let maximum = mechanic_u32(
                &mechanic,
                parameter_key!("maximumCriticalReductionsPerCommit"),
            )
            .max(1);
            let counted = &mut self.shared.dynamic_counters[mechanic.index];
            if *counted >= maximum {
                continue;
            }
            *counted = counted.saturating_add(1);

            let reduction_ms = (cooldown_ms as f64
                * mechanic_param(&mechanic, parameter_key!("weaponCooldownReductionPerCrit"))
                    .clamp(0.0, 1.0))
            .round()
            .max(0.0) as u64;
            let pending = &mut self.shared.dynamic_pending_cooldown_reductions_ms[mechanic.index];
            *pending = pending.saturating_add(reduction_ms);
            let generation = &mut self.shared.dynamic_event_generations[mechanic.index];
            *generation = generation.wrapping_add(1);
            let generation = *generation;
            self.push_event(
                self.common.now_ms.saturating_add(seconds_parameter(
                    &mechanic,
                    parameter_key!("cooldownReductionDelaySeconds"),
                )),
                SharedEvent::WeaponCooldownReduction {
                    mechanic_index: mechanic.index,
                    generation,
                    ability_kind,
                },
            );
            self.record_dynamic_proc(&mechanic);
        }
    }

    // OnlyMatchExact callbacks run before the native tag-container delegates.
    // Diamond is the maintained shared gear listener on that exact path.
    pub(crate) fn trigger_exact_damage_listeners(
        &mut self,
        source: DamageSourceKey,
        triggering_damage: f64,
        target_index: u32,
        context: DamageContext,
    ) {
        if triggering_damage <= 0.0 || self.profile.damage_sources[source.0].item_trait {
            return;
        }
        let count = self.profile.mechanic_indexes.on_exact_damage.len();
        if !self.common.execution.charge_usize(count) {
            return;
        }
        for position in (0..count).rev() {
            let index = self.profile.mechanic_indexes.on_exact_damage[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            // Both cooked health-event listeners reject an instigating
            // ability carrying AbilityType.Trait. Simulator item-trait
            // payloads share this source prefix; Finesse and set-bonus
            // payloads deliberately do not, because their authored
            // ability tags are AbilityType.Core/SetBonus instead.
            if self.roll_dynamic_proc(&mechanic) {
                let slot = self.mechanic_target_slot(mechanic.index, target_index);
                let current_stacks = self
                    .shared
                    .dynamic_target_buffs
                    .get(slot)
                    .and_then(Option::as_ref)
                    .filter(|buff| buff.until_ms > self.common.now_ms)
                    .map(|buff| buff.stacks)
                    .unwrap_or(0);
                let base = self.profile.power
                    * mechanic_param(&mechanic, parameter_key!("powerCoefficient"))
                    * (1.0
                        + mechanic_param(&mechanic, parameter_key!("damageIncreasePerStack"))
                            * current_stacks as f64)
                    * (1.0
                        + mechanic_param(
                            &mechanic,
                            parameter_key!("harmoniousSoulDamageIncreasePerStack"),
                        ) * mechanic_u32(&mechanic, parameter_key!("harmoniousSoulStacks"))
                            as f64);
                self.damage_raw_key(
                    None,
                    mechanic.damage_source,
                    base,
                    0.0,
                    true,
                    false,
                    target_index,
                    context.as_proc(),
                );
                self.activate_dynamic_target_buff(
                    &mechanic,
                    seconds_parameter(&mechanic, parameter_key!("debuffDurationSeconds")),
                    current_stacks
                        .saturating_add(1)
                        .min(mechanic_u32(&mechanic, parameter_key!("maximumStacks")).max(1)),
                    target_index,
                );
                // Diamond Strike applies a distinct harmful amplifier
                // debuff after its damage Gameplay Effect.
                self.notify_kindling_of_harmful_effect(
                    None,
                    mechanic.damage_source,
                    target_index,
                    current_stacks > 0,
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn trigger_dynamic_on_damage(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        damage: f64,
        critical: bool,
        target_index: u32,
        context: DamageContext,
    ) {
        self.trigger_exact_damage_listeners(source, damage, target_index, context);
        self.trigger_generic_damage_listeners(
            ability_kind,
            source,
            damage,
            critical,
            target_index,
            context,
        );
    }

    pub(crate) fn trigger_generic_damage_listeners(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        triggering_damage: f64,
        critical: bool,
        target_index: u32,
        context: DamageContext,
    ) {
        let on_damage = &self.profile.mechanic_indexes.on_damage;
        let on_critical_damage = if critical {
            self.profile.mechanic_indexes.on_critical_damage.as_slice()
        } else {
            &[]
        };
        let mechanic_count = on_damage.len().saturating_add(on_critical_damage.len());
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        let mut critical_dispatch = self.critical_listener_dispatch(critical);
        // Preserve native tag-group contiguity. The accepted approximation
        // orders groups by their first profile entry and initial peers by
        // profile order; reactivated Seized still precedes older peers.
        let critical_first = on_critical_damage
            .first()
            .is_some_and(|critical| on_damage.first().is_none_or(|damage| critical < damage));
        let (first, second) = if critical_first {
            (on_critical_damage, on_damage.as_slice())
        } else {
            (on_damage.as_slice(), on_critical_damage)
        };
        for position in 0..mechanic_count {
            let index = if position < first.len() {
                first[position]
            } else {
                second[position - first.len()]
            };
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if critical && self.dispatch_critical_stat_mechanic(&mechanic, &mut critical_dispatch) {
                continue;
            }
            match mechanic.source_id.as_str() {
                "ItemTrait.ID.GemDotHotOnCrit" if critical => {
                    self.apply_amethyst_splinters(&mechanic, triggering_damage, target_index);
                }
                "ItemTrait.ID.GemPulsatingOnAbilityTotemProc"
                    if source != mechanic.damage_source =>
                {
                    if let Some(state) = self
                        .shared
                        .aurastones
                        .get_mut(mechanic.index)
                        .and_then(Option::as_mut)
                        .filter(|state| self.common.now_ms < state.until_ms)
                    {
                        state.accumulated_damage += triggering_damage.max(0.0);
                    }
                }
                "ItemTrait.ID.GemTargetedSpikeProc" if source != mechanic.damage_source => {
                    if self.roll_dynamic_proc(&mechanic) {
                        self.damage_raw_key(
                            None,
                            mechanic.damage_source,
                            self.profile.power
                                * mechanic_param(&mechanic, parameter_key!("powerCoefficient")),
                            0.0,
                            true,
                            false,
                            target_index,
                            context.as_proc(),
                        );
                        // The Blueprint calls First Strike's Custom Trigger
                        // after damage, bypassing its touched-target guard.
                        self.refresh_first_strike_expertise();
                    }
                }
                _ => {
                    let _ = ability_kind;
                }
            }
        }
    }

    pub(crate) fn roll_dynamic_proc(&mut self, mechanic: &CompiledMechanic) -> bool {
        if self.shared.dynamic_ready_ms[mechanic.index] > self.common.now_ms {
            return false;
        }
        let did_proc =
            if let Some(base_ppm) = mechanic.parameters.get(parameter_key!("procsPerMinute")) {
                let crit_scaler = mechanic_param(mechanic, parameter_key!("ppmCriticalScaling"));
                let ppm =
                    base_ppm * (1.0 + self.effective_critical_strike(None) * crit_scaler.max(0.0));
                self.roll_dynamic_proc_per_minute(
                    mechanic.index,
                    ppm,
                    mechanic_param(mechanic, parameter_key!("ppmHasteScaling")) > 0.0,
                )
            } else {
                self.common
                    .rng
                    .chance(mechanic_param(mechanic, parameter_key!("procChance")).clamp(0.0, 1.0))
            };
        if !did_proc {
            return false;
        }
        let cooldown = seconds_parameter(mechanic, parameter_key!("cooldownSeconds"));
        if cooldown > 0 {
            self.shared.dynamic_ready_ms[mechanic.index] =
                self.common.now_ms.saturating_add(cooldown);
        }
        self.record_dynamic_random_proc(mechanic);
        true
    }

    pub(crate) fn roll_controlled_dynamic_proc(
        &mut self,
        mechanic: &CompiledMechanic,
        stream_tag: &str,
    ) -> bool {
        if self.shared.dynamic_ready_ms[mechanic.index] > self.common.now_ms {
            return false;
        }
        if !self.roll_controlled_random_bool(
            stream_tag,
            mechanic_param(mechanic, parameter_key!("procChance")),
        ) {
            return false;
        }
        let cooldown = seconds_parameter(mechanic, parameter_key!("cooldownSeconds"));
        if cooldown > 0 {
            self.shared.dynamic_ready_ms[mechanic.index] =
                self.common.now_ms.saturating_add(cooldown);
        }
        self.record_dynamic_random_proc(mechanic);
        true
    }

    pub(crate) fn record_dynamic_proc(&mut self, mechanic: &CompiledMechanic) {
        self.common.result.mechanic_proc_counts[mechanic.index] += 1.0;
    }

    pub(crate) fn record_dynamic_random_proc(&mut self, mechanic: &CompiledMechanic) {
        self.record_dynamic_proc(mechanic);
        if let Some(index) = self
            .profile
            .mechanic_proc_sources
            .get(mechanic.source_id.as_str())
        {
            self.common.result.proc_counts[*index] += 1;
        }
    }

    pub(crate) fn record_talent_proc(&mut self, talent_id: &str) {
        if let Some(index) = self.profile.talent_proc_sources.get(talent_id) {
            self.common.result.proc_counts[*index] += 1;
        }
    }

    pub(crate) fn record_ability_proc(&mut self, ability: DpsAbilityKind) {
        if let Some(index) = self.profile.ability_proc_sources.get(&ability) {
            self.common.result.proc_counts[*index] += 1;
        }
    }

    pub(crate) fn record_spirit_refund_proc(&mut self) {
        self.common.result.proc_counts[self.profile.spirit_refund_proc_source] += 1;
        self.trigger_spirit_refund_effects();
    }

    pub(crate) fn trigger_spirit_refund_effects(&mut self) {
        let mechanic_count = self
            .profile
            .mechanic_indexes
            .increased_main_stat_and_spirit
            .len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.increased_main_stat_and_spirit[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            self.activate_dynamic_buff(
                &mechanic,
                seconds_parameter(&mechanic, parameter_key!("durationSeconds")),
                1,
                0.0,
            );
            self.record_dynamic_proc(&mechanic);
        }
    }

    pub(crate) fn activate_dynamic_buff(
        &mut self,
        mechanic: &CompiledMechanic,
        duration_ms: u64,
        stacks: u32,
        value: f64,
    ) {
        let previous = self
            .shared
            .dynamic_buffs
            .get(mechanic.index)
            .and_then(Option::as_ref)
            .copied();
        if let Some(expired) = previous.filter(|buff| buff.until_ms <= self.common.now_ms) {
            let active = expired
                .until_ms
                .min(ENCOUNTER_DURATION_MS)
                .saturating_sub(expired.started_ms.min(ENCOUNTER_DURATION_MS));
            self.common.result.mechanic_buff_uptimes[mechanic.index] +=
                active as f64 / ENCOUNTER_DURATION_MS as f64;
        }
        let started_ms = previous
            .filter(|buff| buff.until_ms > self.common.now_ms)
            .map(|buff| buff.started_ms)
            .unwrap_or(self.common.now_ms);
        self.shared.dynamic_buffs[mechanic.index] = Some(DynamicBuffState {
            started_ms,
            until_ms: self.common.now_ms.saturating_add(duration_ms),
            stacks,
            value,
        });
    }

    pub(crate) fn activate_dynamic_target_buff(
        &mut self,
        mechanic: &CompiledMechanic,
        duration_ms: u64,
        stacks: u32,
        target_index: u32,
    ) {
        let slot = self.mechanic_target_slot(mechanic.index, target_index);
        let started_ms = self
            .shared
            .dynamic_target_buffs
            .get(slot)
            .and_then(Option::as_ref)
            .filter(|buff| buff.until_ms > self.common.now_ms)
            .map(|buff| buff.started_ms)
            .unwrap_or(self.common.now_ms);
        self.shared.dynamic_target_buffs[slot] = Some(DynamicBuffState {
            started_ms,
            until_ms: self.common.now_ms.saturating_add(duration_ms),
            stacks,
            value: 0.0,
        });
    }

    pub(crate) fn spawn_aurastone(&mut self, mechanic: &CompiledMechanic) {
        let generation = self
            .shared
            .aurastones
            .get(mechanic.index)
            .and_then(Option::as_ref)
            .map(|state| state.generation.wrapping_add(1))
            .unwrap_or(1);
        self.shared.aurastones[mechanic.index] = Some(AurastoneState {
            generation,
            until_ms: self.shared.heroism_until,
            accumulated_damage: 0.0,
        });
        self.push_event(
            self.common.now_ms.saturating_add(seconds_parameter(
                mechanic,
                parameter_key!("initialDamagePulseDelaySeconds"),
            )),
            SharedEvent::AurastoneDamagePulse {
                mechanic_index: mechanic.index,
                generation,
            },
        );
        self.record_dynamic_proc(mechanic);
    }

    pub(crate) fn trigger_kindling_on_harmful_effect_application(
        &mut self,
        source_ability_kind: Option<DpsAbilityKind>,
        source: DamageSourceKey,
        target_index: u32,
    ) {
        let mechanic_count = self.profile.mechanic_indexes.extra_dot_on_application.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.extra_dot_on_application[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if source == mechanic.damage_source {
                continue;
            }
            // The cooked listener admits harmful Gameplay Effect application
            // and duration-refresh events, rejects only secondary-skill source
            // abilities, environmental effects, and its own custom asset tag,
            // then performs a dedicated Haste-scaled 2.1-RPPM roll.
            // None of the maintained DPS source abilities carries the
            // ignored AbilityType.Skill.Secondary tag. Keep the source kind in
            // the contract so a future hero can make that filter explicit.
            let _ = source_ability_kind;
            if self.roll_dynamic_proc(&mechanic) {
                self.apply_kindling(&mechanic, target_index);
            }
        }
    }

    pub(crate) fn trigger_devouring_flame_kindling(
        &mut self,
        source_ability_kind: DpsAbilityKind,
        source: DamageSourceKey,
        target_index: u32,
    ) {
        if source_ability_kind != DpsAbilityKind::EngulfingFlames
            || self
                .profile
                .mechanic_indexes
                .target_damage_multiplier
                .is_empty()
        {
            return;
        }

        // Each independent Engulfing Flames adds one linked Harmful stack.
        // Extending its duration does not apply the linked effect again.
        self.trigger_kindling_on_harmful_effect_application(
            Some(source_ability_kind),
            source,
            target_index,
        );
    }

    pub(crate) fn apply_kindling(&mut self, mechanic: &CompiledMechanic, target_index: u32) {
        let slot = self.mechanic_target_slot(mechanic.index, target_index);
        let previous = self.shared.kindling[slot];
        if let Some(state) = self.kindling_state(mechanic, previous, false) {
            self.shared.kindling[slot] = Some(state);
            self.refresh_kindling_clocks();
        }
    }

    pub(crate) fn apply_amethyst_splinters(
        &mut self,
        mechanic: &CompiledMechanic,
        triggering_damage: f64,
        target_index: u32,
    ) {
        let duration_ms = seconds_parameter(mechanic, parameter_key!("durationSeconds"));
        let period_ms = seconds_parameter(mechanic, parameter_key!("periodSeconds"));
        if triggering_damage <= 0.0 || duration_ms == 0 || period_ms == 0 {
            return;
        }

        let slot = self.mechanic_target_slot(mechanic.index, target_index);
        // Accepted timing policy: a refresh at expiry starts a new phase and
        // replaces any pending old-generation tick. An already-dispatched tick
        // keeps its damage; existing event order resolves same-time callbacks.
        let previous = self
            .shared
            .amethyst_splinters
            .get(slot)
            .and_then(Option::as_ref)
            .copied()
            .filter(|state| state.until_ms > self.common.now_ms);
        let contribution =
            triggering_damage * mechanic_param(mechanic, parameter_key!("triggerDamageFraction"));
        let remaining_damage = previous
            .map(|state| {
                state.damage_per_tick * state.until_ms.saturating_sub(self.common.now_ms) as f64
                    / period_ms as f64
            })
            .unwrap_or(0.0);
        let tick_count = duration_ms as f64 / period_ms as f64;
        // Both Blueprint creation and refresh narrow the calculated double to
        // a float before storing SetByCaller.Damage (build 25485623, offsets
        // 2080 and 1303). Preserve that boundary before native damage rounding.
        let damage_per_tick = ((contribution + remaining_damage) / tick_count) as f32 as f64;
        let generation = self
            .shared
            .amethyst_splinters
            .get(slot)
            .and_then(Option::as_ref)
            .map(|state| state.generation.wrapping_add(1))
            .unwrap_or(1);
        let next_tick_ms = previous
            .map(|state| state.next_tick_ms)
            .unwrap_or_else(|| self.common.now_ms.saturating_add(period_ms));
        self.shared.amethyst_splinters[slot] = Some(AmethystSplintersState {
            generation,
            until_ms: self.common.now_ms.saturating_add(duration_ms),
            next_tick_ms,
            damage_per_tick,
        });
        self.push_event(
            next_tick_ms,
            SharedEvent::AmethystSplintersTick {
                mechanic_index: mechanic.index,
                generation,
                target_index,
            },
        );
        self.record_dynamic_proc(mechanic);
        self.trigger_kindling_on_harmful_effect_application(
            None,
            mechanic.damage_source,
            target_index,
        );
    }

    pub(crate) fn tick_amethyst_splinters(
        &mut self,
        mechanic_index: usize,
        generation: u64,
        target_index: u32,
    ) {
        let slot = self.mechanic_target_slot(mechanic_index, target_index);
        let Some(mut state) = self.shared.amethyst_splinters[slot].filter(|state| {
            state.generation == generation
                && state.next_tick_ms == self.common.now_ms
                && self.common.now_ms <= state.until_ms
        }) else {
            return;
        };
        let mechanic = Arc::clone(&self.profile.mechanics[mechanic_index]);
        // A nested critical proc can refresh this effect synchronously. Consume
        // this tick before dispatch so that refresh preserves the next period,
        // and never write the old magnitude/duration over the refreshed state.
        state.next_tick_ms = state.next_tick_ms.saturating_add(seconds_parameter(
            &mechanic,
            parameter_key!("periodSeconds"),
        ));
        self.shared.amethyst_splinters[slot] = Some(state);
        if state.next_tick_ms <= state.until_ms {
            self.push_event(
                state.next_tick_ms,
                SharedEvent::AmethystSplintersTick {
                    mechanic_index,
                    generation,
                    target_index,
                },
            );
        }

        self.damage_unscaled_key(
            None,
            mechanic.damage_source,
            state.damage_per_tick,
            target_index,
            DamageProvenance::Periodic,
            DamageContext::NONE,
        );
    }

    pub(crate) fn pulse_aurastone_damage(&mut self, mechanic_index: usize, generation: u64) {
        let mechanic = Arc::clone(&self.profile.mechanics[mechanic_index]);
        let Some(state) = self.shared.aurastones[mechanic_index]
            .as_ref()
            .filter(|state| state.generation == generation && self.common.now_ms < state.until_ms)
        else {
            return;
        };
        let accumulated_damage = state.accumulated_damage;
        let until_ms = state.until_ms;

        // The cooked pulse graph calculates the shared multi-target falloff
        // but never consumes it. It divides the stored damage evenly across
        // every actor returned by the 1,000-unit sphere, preserving total DPS
        // for the maintained stacked-target scenario.
        if accumulated_damage > 0.0 && self.common.target_count > 0 {
            let damage_per_target = accumulated_damage
                * mechanic_param(&mechanic, parameter_key!("damageAccumulationFraction"))
                / f64::from(self.common.target_count);
            if !self.charge_work_units(u64::from(self.common.target_count)) {
                return;
            }
            for target_index in 0..self.common.target_count {
                self.damage_unscaled_key(
                    None,
                    mechanic.damage_source,
                    damage_per_target,
                    target_index,
                    DamageProvenance::Proc,
                    DamageContext::NONE,
                );
            }
        }

        // The Blueprint clears the store after ApplyExternalEffectSpec returns.
        // Damage from synchronous nested procs is therefore cleared with the
        // pulse's original store, rather than carried into the next pulse.
        if let Some(state) = self.shared.aurastones[mechanic_index]
            .as_mut()
            .filter(|state| state.generation == generation)
        {
            state.accumulated_damage = 0.0;
        }

        let next_ms = self.common.now_ms.saturating_add(seconds_parameter(
            &mechanic,
            parameter_key!("pulseIntervalSeconds"),
        ));
        if next_ms < until_ms {
            self.push_event(
                next_ms,
                SharedEvent::AurastoneDamagePulse {
                    mechanic_index,
                    generation,
                },
            );
        }
    }

    pub(crate) fn dynamic_stat_bonus(&self, parameter: ParameterKey) -> f64 {
        let indexes = self.profile.mechanic_indexes.dynamic_stat(parameter);
        if !self.common.execution.charge_lightweight_loop(indexes.len()) {
            return 0.0;
        }
        indexes
            .iter()
            .map(|index| &self.profile.mechanics[*index])
            .map(|mechanic| {
                if mechanic.source_id == "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease"
                    && self.common.now_ms
                        >= seconds_parameter(mechanic, parameter_key!("applicationDelaySeconds"))
                {
                    return mechanic_param(mechanic, parameter);
                }
                let Some(buff) = self
                    .shared
                    .dynamic_buffs
                    .get(mechanic.index)
                    .and_then(Option::as_ref)
                    .filter(|buff| buff.until_ms > self.common.now_ms)
                else {
                    return 0.0;
                };
                if is_finesse_source(&mechanic.source_id, "08") {
                    return if matches!(
                        parameter,
                        key if key == parameter_key!("criticalStrikeBonus")
                            || key == parameter_key!("hasteBonus")
                            || key == parameter_key!("expertiseBonus")
                    ) {
                        // The cooked magnitude captures live Spirit, not the
                        // value at the Major commit that starts this buff.
                        self.effective_spirit()
                            .min(mechanic_param(mechanic, parameter_key!("spiritCap")))
                            * 100.0
                            * mechanic_param(mechanic, parameter_key!("statIncreasePerSpirit"))
                            / mechanic_param(mechanic, parameter_key!("spiritPerIncrease"))
                                .max(f64::EPSILON)
                    } else {
                        0.0
                    };
                }
                let direct = mechanic_param(mechanic, parameter);
                let fallback = if parameter == parameter_key!("hasteBonus") {
                    mechanic_param(mechanic, parameter_key!("haste"))
                } else if parameter == parameter_key!("expertiseBonus") {
                    mechanic_param(mechanic, parameter_key!("expertise"))
                } else {
                    0.0
                };
                (direct + fallback) * buff.stacks.max(1) as f64
            })
            .sum()
    }

    pub(crate) fn dynamic_rating_bonus(&self, parameter: ParameterKey) -> f64 {
        let indexes = self.profile.mechanic_indexes.dynamic_rating(parameter);
        if !self.common.execution.charge_lightweight_loop(indexes.len()) {
            return 0.0;
        }
        indexes
            .iter()
            .map(|index| &self.profile.mechanics[*index])
            .map(|mechanic| {
                if mechanic.source_id == "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease"
                    && self.common.now_ms
                        >= seconds_parameter(mechanic, parameter_key!("applicationDelaySeconds"))
                {
                    return mechanic_param(mechanic, parameter);
                }
                let Some(buff) = self
                    .shared
                    .dynamic_buffs
                    .get(mechanic.index)
                    .and_then(Option::as_ref)
                    .filter(|buff| buff.until_ms > self.common.now_ms)
                else {
                    return 0.0;
                };
                if mechanic.source_id == "ItemTrait.ID.OffensiveAbilityToHighestStatBuff" {
                    let selected_parameter = match buff.value.round() as u8 {
                        0 => parameter_key!("criticalRating"),
                        1 => parameter_key!("expertiseRating"),
                        2 => parameter_key!("hasteRating"),
                        3 => parameter_key!("spiritRating"),
                        _ => return 0.0,
                    };
                    return if parameter == selected_parameter {
                        mechanic_param(mechanic, parameter_key!("secondaryRating"))
                    } else {
                        0.0
                    };
                }
                mechanic_param(mechanic, parameter) * buff.stacks.max(1) as f64
            })
            .sum()
    }

    pub(crate) fn highest_secondary_rating_code(&mut self) -> u8 {
        // The cooked CDO inserts these entries in this order. Native Map_Keys
        // scans the unchanged FScriptMap's allocated sparse-array slots in
        // ascending order, and the Blueprint replaces the current winner when
        // RandomBool returns true for an exact tie.
        let candidates = [
            (
                0,
                self.profile.critical_rating
                    + self.dynamic_rating_bonus(parameter_key!("criticalRating")),
            ),
            (
                1,
                self.profile.expertise_rating
                    + self.dynamic_rating_bonus(parameter_key!("expertiseRating")),
            ),
            (
                2,
                self.profile.haste_rating
                    + self.dynamic_rating_bonus(parameter_key!("hasteRating")),
            ),
            (
                3,
                self.profile.spirit_rating
                    + self.dynamic_rating_bonus(parameter_key!("spiritRating")),
            ),
        ];
        let mut selected = candidates[0];
        for candidate in candidates.into_iter().skip(1) {
            if candidate.1 > selected.1
                || (candidate.1 == selected.1 && self.common.rng.native_uniform_f32() >= 0.5)
            {
                selected = candidate;
            }
        }
        selected.0
    }

    pub(crate) fn effective_power_multiplier(&self) -> f64 {
        let mut multiplier = 1.0;
        let mut primary_attribute_channel = 1.0;
        if !self
            .common
            .execution
            .charge_lightweight_loop(self.profile.mechanic_indexes.power_multiplier.len())
        {
            return multiplier;
        }
        for index in &self.profile.mechanic_indexes.power_multiplier {
            let mechanic = &self.profile.mechanics[*index];
            let heroism_multiplier =
                mechanic_param(mechanic, parameter_key!("heroismPowerMultiplier"));
            if matches!(
                mechanic.kind,
                CompiledMechanicKind::HeroSource(HeroSourceKind::Set(
                    SetHeroSourceKind::HeroismPower
                ))
            ) {
                if !self
                    .common
                    .execution
                    .charge_lightweight_loop(self.shared.heroism_power_set_expirations.len())
                {
                    return multiplier;
                }
                for until in &self.shared.heroism_power_set_expirations {
                    if *until > self.common.now_ms {
                        multiplier *= heroism_multiplier.max(1.0);
                    }
                }
            } else if self.common.now_ms < self.shared.heroism_until {
                multiplier *= heroism_multiplier.max(1.0);
            }
            if mechanic.source_id.starts_with("legendary-") {
                continue;
            }
            if self
                .shared
                .dynamic_buffs
                .get(mechanic.index)
                .and_then(Option::as_ref)
                .is_some_and(|buff| buff.until_ms > self.common.now_ms)
            {
                let magnitude =
                    mechanic_param(mechanic, parameter_key!("powerMultiplier")).max(1.0);
                if matches!(
                    mechanic.source_id.as_str(),
                    "ItemTrait.ID.AbilityToIncreasedMainStat"
                        | "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff"
                        | "ItemTrait.ID.IncreasedMainStatAndSpiritRating"
                        | "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease"
                ) {
                    // These four modifiers share Channel0's Multiplicitive
                    // bucket. Compound set/gem and other attribute effects
                    // remain separate products.
                    primary_attribute_channel += magnitude - 1.0;
                } else {
                    multiplier *= magnitude;
                }
            }
        }
        if let HeroState::Mara(state) = &self.hero {
            if self.common.now_ms < state.maiden_of_death_until {
                multiplier *= self
                    .ability(DpsAbilityKind::MaidenOfDeath)
                    .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                    .unwrap_or(1.0);
            }
            if self.common.now_ms < state.matriarch_macabre_until {
                multiplier *= self
                    .ability(DpsAbilityKind::MatriarchMacabre)
                    .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                    .unwrap_or(1.0);
            }
        }
        if let HeroState::Gunde(state) = &self.hero
            && self.common.now_ms < state.ancestral_instinct_until
        {
            multiplier *= self
                .common
                .selected_talents
                .get("gunde-talent-id-talent17")
                .map(|talent| param(talent, parameter_key!("powerMultiplier")))
                .unwrap_or(1.0);
        }
        multiplier * primary_attribute_channel
    }

    pub(crate) fn current_primary_attribute(&self) -> f64 {
        self.profile.power * self.effective_power_multiplier()
    }

    pub(crate) fn resolved_power_blessing_multiplier(&self) -> f64 {
        let mut multiplier = 1.0;
        if !self
            .common
            .execution
            .charge_lightweight_loop(self.profile.mechanic_indexes.source_damage_multiplier.len())
        {
            return multiplier;
        }
        for index in &self.profile.mechanic_indexes.source_damage_multiplier {
            let mechanic = &self.profile.mechanics[*index];
            if is_finesse_source(mechanic.source_id.as_str(), "10") {
                let spirit = self
                    .effective_spirit()
                    .min(mechanic_param(mechanic, parameter_key!("spiritCap")));
                multiplier *= 1.0
                    + spirit
                        * mechanic_param(mechanic, parameter_key!("damageIncreasePerSpiritAmount"))
                        / mechanic_param(mechanic, parameter_key!("spiritAmount"))
                            .max(f64::EPSILON);
            }
        }
        multiplier
    }

    pub(crate) fn source_damage_multiplier(&self, ability_kind: Option<DpsAbilityKind>) -> f64 {
        let category = ability_kind.map(ability_category);
        let mut multiplier = 1.0;
        if !self
            .common
            .execution
            .charge_lightweight_loop(self.profile.mechanic_indexes.source_damage_multiplier.len())
        {
            return multiplier;
        }
        for index in &self.profile.mechanic_indexes.source_damage_multiplier {
            let mechanic = &self.profile.mechanics[*index];
            match mechanic.source_id.as_str() {
                "ItemTrait.ID.WeaponHealDamageIncrease"
                    if category == Some(AbilityCategory::Weapon) =>
                {
                    multiplier *=
                        mechanic_param(mechanic, parameter_key!("weaponDamageMultiplier")).max(1.0);
                }
                _ => {}
            }
        }
        if let HeroState::Rime(state) = &self.hero {
            if self.common.now_ms < state.ice_blitz_until {
                multiplier *= self
                    .ability(DpsAbilityKind::IceBlitz)
                    .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                    .unwrap_or(1.2);
            }
            if self.common.now_ms < state.wrath_of_winter_until {
                multiplier *= self
                    .ability(DpsAbilityKind::WrathOfWinter)
                    .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                    .unwrap_or(1.2);
            }
        }
        multiplier
    }

    pub(crate) fn effective_critical_strike(&self, ability_kind: Option<DpsAbilityKind>) -> f64 {
        self.effective_critical_strike_for_category(
            ability_kind,
            ability_kind.map(ability_category),
        )
    }

    pub(crate) fn effective_critical_strike_for_category(
        &self,
        ability_kind: Option<DpsAbilityKind>,
        category: Option<AbilityCategory>,
    ) -> f64 {
        let mut value = self.profile.critical_strike
            + secondary_rating_delta(
                self.profile.critical_rating,
                self.dynamic_rating_bonus(parameter_key!("criticalRating")),
            )
            + self.dynamic_stat_bonus(parameter_key!("criticalStrikeBonus"));
        if category == Some(AbilityCategory::Core) {
            if !self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.critical_strike.len())
            {
                return value;
            }
            for index in &self.profile.mechanic_indexes.critical_strike {
                let mechanic = &self.profile.mechanics[*index];
                if is_finesse_source(&mechanic.source_id, "05") {
                    value += mechanic_param(mechanic, parameter_key!("criticalStrikeBonus"));
                }
            }
        }
        if category == Some(AbilityCategory::Weapon) {
            if let Some(kind) = ability_kind {
                value += self
                    .ability(kind)
                    .and_then(|ability| {
                        ability
                            .parameters
                            .get(parameter_key!("criticalStrikeBonus"))
                    })
                    .unwrap_or(0.0);
            }
            if !self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.critical_strike.len())
            {
                return value;
            }
            for index in &self.profile.mechanic_indexes.critical_strike {
                let mechanic = &self.profile.mechanics[*index];
                if mechanic.source_id == "ItemTrait.ID.WeaponCritChanceCooldownReduction" {
                    value += mechanic_param(mechanic, parameter_key!("weaponCriticalStrikeBonus"));
                }
            }
        }
        if self.common.now_ms < self.shared.weapon_critical_buff_until {
            value += f64::from(self.shared.weapon_critical_buff_stacks)
                * self
                    .ability(DpsAbilityKind::WeaponInstantAoe)
                    .and_then(|ability| {
                        ability
                            .parameters
                            .get(parameter_key!("criticalStrikeBonusPerStack"))
                    })
                    .unwrap_or(0.0);
        }
        if let HeroState::Mara(_) = &self.hero
            && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent1")
        {
            let bleeding_targets = (0..self.common.target_count)
                .filter(|target_index| {
                    self.dot_remaining_on_target(*target_index, DpsAbilityKind::HemorrhagingStrike)
                        > 0
                })
                .count() as u32;
            if bleeding_targets > 0 {
                value += (param(talent, parameter_key!("initialCriticalStrikeBonus"))
                    + f64::from(bleeding_targets.saturating_sub(1))
                        * param(
                            talent,
                            parameter_key!("additionalTargetCriticalStrikeBonus"),
                        ))
                .min(param(talent, parameter_key!("maximumCriticalStrikeBonus")));
            }
        }
        value
    }

    #[cfg(test)]
    pub(crate) fn effective_critical_multiplier(
        &self,
        ability_kind: Option<DpsAbilityKind>,
    ) -> f64 {
        self.effective_critical_multiplier_for_category(ability_kind.map(ability_category))
    }

    pub(crate) fn effective_critical_multiplier_for_category(
        &self,
        category: Option<AbilityCategory>,
    ) -> f64 {
        let mut value = self.profile.critical_multiplier;
        if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent16") {
            value *= param(talent, parameter_key!("criticalPowerMultiplier")).max(1.0);
        }
        if category == Some(AbilityCategory::Core) {
            if !self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.critical_multiplier.len())
            {
                return value;
            }
            for index in &self.profile.mechanic_indexes.critical_multiplier {
                let mechanic = &self.profile.mechanics[*index];
                value *=
                    mechanic_param(mechanic, parameter_key!("criticalPowerMultiplier")).max(1.0);
            }
        }
        value
    }

    pub(crate) fn effective_expertise(&self) -> f64 {
        let value = self.profile.expertise
            + secondary_rating_delta(
                self.profile.expertise_rating,
                self.dynamic_rating_bonus(parameter_key!("expertiseRating")),
            )
            + self.dynamic_stat_bonus(parameter_key!("expertiseBonus"))
            + if self.common.now_ms < self.shared.weapon_charge_buff_until {
                self.ability(DpsAbilityKind::WeaponCleaveCharge)
                    .and_then(|ability| ability.parameters.get(parameter_key!("buffExpertise")))
                    .unwrap_or(0.0)
            } else {
                0.0
            };
        if let HeroState::Tariq(state) = &self.hero {
            value
                + if self.common.now_ms < state.raging_tempest_until {
                    f64::from(state.raging_current_stacks)
                        * self
                            .ability(DpsAbilityKind::RagingTempest)
                            .and_then(|ability| {
                                ability.parameters.get(parameter_key!("expertisePerStack"))
                            })
                            .unwrap_or(0.0)
                } else {
                    0.0
                }
                + if self.common.now_ms < state.square_hammer_expertise_until {
                    self.common
                        .selected_talents
                        .get("ink-talent-id-talent18")
                        .map(|talent| param(talent, parameter_key!("expertiseBonus")))
                        .unwrap_or(0.0)
                } else {
                    0.0
                }
        } else if let HeroState::Elarion(state) = &self.hero {
            value
                + f64::from(state.strikers_aim_stacks)
                    * self
                        .common
                        .selected_talents
                        .get("bowguy-talent-id-talent11")
                        .map(|talent| param(talent, parameter_key!("expertisePerStack")))
                        .unwrap_or(0.0)
        } else if let HeroState::Mara(state) = &self.hero {
            value
                + if self.common.now_ms < state.drenched_in_blood_until {
                    self.profile
                        .mechanic_indexes
                        .mara_drenched_in_blood
                        .map(|index| {
                            mechanic_param(
                                &self.profile.mechanics[index],
                                parameter_key!("spiritRefundExpertise"),
                            )
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                }
        } else {
            value
        }
    }

    pub(crate) fn effective_spirit(&self) -> f64 {
        let spirit = self.profile.spirit
            + secondary_rating_delta(
                self.profile.spirit_rating,
                self.dynamic_rating_bonus(parameter_key!("spiritRating")),
            )
            + self.dynamic_stat_bonus(parameter_key!("spiritBonus"));
        if matches!(
            &self.hero,
            HeroState::Rime(state) if self.common.now_ms < state.winters_blessing_until
        ) {
            spirit
                * (1.0
                    + self
                        .ability(DpsAbilityKind::WintersBlessing)
                        .map(|ability| ability_param(ability, parameter_key!("spiritMultiplier")))
                        .unwrap_or(0.2))
        } else if let HeroState::Tariq(state) = &self.hero {
            spirit
                + if self.common.now_ms < state.far_beyond_driven_until {
                    f64::from(state.far_beyond_driven_stacks)
                        * self
                            .common
                            .selected_talents
                            .get("ink-talent-id-talent8")
                            .map(|talent| param(talent, parameter_key!("spiritPerStack")))
                            .unwrap_or(0.0)
                } else {
                    0.0
                }
        } else if let HeroState::Gunde(state) = &self.hero {
            spirit
                + if self.common.now_ms < state.massacre_until {
                    f64::from(state.massacre_stacks)
                        * self
                            .common
                            .selected_talents
                            .get("gunde-talent-id-talent9")
                            .map(|talent| param(talent, parameter_key!("spiritPerStack")))
                            .unwrap_or(0.0)
                } else {
                    0.0
                }
        } else {
            spirit
        }
    }

    pub(crate) fn effective_cooldown_recovery(&self) -> f64 {
        let mut acceleration = self.profile.cooldown_recovery;
        if self.common.now_ms < self.shared.weapon_channel_cooldown_recovery_until {
            acceleration += self
                .ability(DpsAbilityKind::WeaponArcaneChannel)
                .and_then(|ability| {
                    ability
                        .parameters
                        .get(parameter_key!("channelCooldownRecoveryMultiplier"))
                })
                .unwrap_or(1.0)
                .max(1.0)
                - 1.0;
        }
        if self.common.now_ms < self.shared.weapon_charge_buff_until {
            acceleration += self
                .ability(DpsAbilityKind::WeaponCleaveCharge)
                .and_then(|ability| {
                    ability
                        .parameters
                        .get(parameter_key!("buffCooldownRecoveryMultiplier"))
                })
                .unwrap_or(1.0)
                .max(1.0)
                - 1.0;
        }
        if !self
            .common
            .execution
            .charge_lightweight_loop(self.profile.mechanic_indexes.cooldown_recovery.len())
        {
            return acceleration.max(0.05);
        }
        for index in &self.profile.mechanic_indexes.cooldown_recovery {
            let mechanic = &self.profile.mechanics[*index];
            if self
                .shared
                .dynamic_buffs
                .get(mechanic.index)
                .and_then(Option::as_ref)
                .is_some_and(|buff| buff.until_ms > self.common.now_ms)
            {
                acceleration +=
                    mechanic_param(mechanic, parameter_key!("cooldownAccelerationMultiplier"))
                        .max(1.0)
                        - 1.0;
            }
        }
        if let HeroState::Elarion(state) = &self.hero
            && self.common.now_ms < state.event_horizon_until
        {
            // The manager updates a MultiplyCompound modifier whenever Haste changes.
            acceleration *= (1.0 + self.effective_haste()).max(0.05);
        }
        acceleration.max(0.05)
    }

    pub(crate) fn cooldown_recovery_rate(&self, ability_kind: DpsAbilityKind) -> f64 {
        let Some(ability) = self.ability(ability_kind) else {
            return 1.0;
        };
        let mut rate = 1.0;
        if ability.cooldown_scales_with_haste {
            rate += self.effective_haste();
        }
        if ability.cooldown_scales_with_cooldown_recovery {
            rate += self.effective_cooldown_recovery() - 1.0;
        }
        // The Monarch modifies tagged Major effect time rates directly; it
        // is not part of the CooldownRecovery attribute multiplied by buffs.
        if ability_category(ability_kind) == AbilityCategory::Major
            && self
                .common
                .execution
                .charge_lightweight_loop(self.profile.mechanic_indexes.cooldown_recovery.len())
        {
            for index in &self.profile.mechanic_indexes.cooldown_recovery {
                let mechanic = &self.profile.mechanics[*index];
                if is_finesse_source(&mechanic.source_id, "11") {
                    rate += mechanic_param(mechanic, parameter_key!("cooldownAcceleration"))
                        + self
                            .effective_haste()
                            .min(mechanic_param(mechanic, parameter_key!("hasteThreshold")))
                            * mechanic_param(mechanic, parameter_key!("accelerationPerHaste"));
                }
            }
        }
        if ability_kind == DpsAbilityKind::ColdSnap {
            rate += self.effective_haste();
        }
        if let HeroState::Elarion(state) = &self.hero
            && state.skylit_grace_active
            && ability_kind == DpsAbilityKind::SkystridersGrace
        {
            rate += self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent4")
                .map(|talent| param(talent, parameter_key!("cooldownAcceleration")))
                .unwrap_or(0.0);
        }
        if let HeroState::Gunde(state) = &self.hero
            && ability_kind == DpsAbilityKind::ReignInBlood
            && self.common.now_ms < state.bloodbath_until
        {
            rate += self
                .common
                .selected_talents
                .get("gunde-talent-id-talent18")
                .map(|talent| param(talent, parameter_key!("cooldownAccelerationMultiplier")))
                .unwrap_or(0.0);
        }
        rate.max(0.05)
    }

    pub(crate) fn cooldown_remaining_ms(&self, ability_kind: DpsAbilityKind) -> u64 {
        let Some(ability) = self.ability(ability_kind) else {
            return 0;
        };
        self.common
            .cooldowns
            .get(&ability_kind)
            .filter(|cooldown| cooldown.used_charges >= ability.maximum_charges.max(1))
            .map(|cooldown| {
                (cooldown.remaining_ms / self.cooldown_recovery_rate(ability_kind))
                    .ceil()
                    .max(0.0) as u64
            })
            .unwrap_or(0)
    }

    pub(crate) fn try_dynamic_on_hit(
        &mut self,
        ability_kind: Option<DpsAbilityKind>,
        triggering_damage: f64,
        target_index: u32,
        context: DamageContext,
    ) {
        let mechanic_count = self.profile.mechanic_indexes.on_hit.len();
        if !self.common.execution.charge_usize(mechanic_count) {
            return;
        }
        for position in 0..mechanic_count {
            let index = self.profile.mechanic_indexes.on_hit[position];
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if mechanic.ability_kind.is_some() && mechanic.ability_kind != ability_kind {
                continue;
            }
            if self.shared.dynamic_ready_ms[mechanic.index] > self.common.now_ms {
                continue;
            }
            let elapsed = self.shared.dynamic_last_roll_ms[mechanic.index]
                .replace(self.common.now_ms)
                .map(|last| self.common.now_ms.saturating_sub(last))
                .unwrap_or(self.common.now_ms)
                .max(1);
            let probability = mechanic
                .parameters
                .get(parameter_key!("procsPerMinute"))
                .map(|ppm| (ppm * elapsed as f64 / 60_000.0).clamp(0.0, 1.0))
                .unwrap_or_else(|| {
                    mechanic
                        .parameters
                        .get(parameter_key!("procChance"))
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0)
                });
            if !self.common.rng.chance(probability) {
                continue;
            }
            let base = self.profile.power
                * mechanic
                    .parameters
                    .get(parameter_key!("powerCoefficient"))
                    .unwrap_or(0.0)
                + triggering_damage
                    * mechanic
                        .parameters
                        .get(parameter_key!("triggerDamageFraction"))
                        .unwrap_or(0.0);
            let cooldown_ms = mechanic
                .parameters
                .get(parameter_key!("cooldownMs"))
                .unwrap_or(0.0)
                .max(0.0) as u64;
            self.shared.dynamic_ready_ms[mechanic.index] =
                self.common.now_ms.saturating_add(cooldown_ms);
            if base > 0.0 {
                self.damage_raw_key(
                    None,
                    mechanic.damage_source,
                    base,
                    0.0,
                    mechanic
                        .parameters
                        .get(parameter_key!("canCrit"))
                        .unwrap_or(0.0)
                        > 0.0,
                    false,
                    target_index,
                    context.as_proc(),
                );
            }
            self.record_dynamic_random_proc(&mechanic);
        }
    }

    pub(crate) fn damage_source_key(&self, id: &str) -> DamageSourceKey {
        self.profile
            .damage_source_key(id)
            .unwrap_or_else(|| panic!("compiled damage source registry is missing {id}"))
    }

    pub(crate) fn ability_damage_source(&self, ability: &CompiledAbility) -> DamageSourceKey {
        if ability.damage_source == DamageSourceKey::INVALID {
            self.damage_source_key(&ability.id)
        } else {
            ability.damage_source
        }
    }

    pub(crate) fn record_damage(
        &mut self,
        source: DamageSourceKey,
        damage: f64,
        crit_chance: f64,
        critical: bool,
        target_index: u32,
    ) {
        self.common.result.damage += damage;
        if let Some(target) = self.common.result.targets.get_mut(target_index as usize) {
            *target += damage;
        }
        let totals = &mut self.common.result.abilities[source.0];
        totals.damage += damage;
        totals.hits += 1;
        totals.targets_hit_mask |= 1_u32 << target_index;
        if critical {
            totals.crits += 1;
            if crit_chance > 1.0 {
                totals.grievous += 1;
            }
        }
    }
}
