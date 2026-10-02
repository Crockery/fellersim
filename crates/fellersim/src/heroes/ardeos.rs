use std::collections::BTreeMap;

use crate::*;
use crate::{AplBuff, DamageContext, DamageSourceKey, FireFrogState, Iteration};

#[derive(Debug, Clone)]
pub(crate) enum ArdeosEvent {
    FlareUp {
        source: DamageSourceKey,
        damage_bits: u64,
        targets: Vec<u32>,
        context: DamageContext,
    },
    DetonateHit {
        index: usize,
        damage_bits: u64,
        target_index: u32,
        context: DamageContext,
    },
    FireFrogSpawn {
        actor_id: u64,
    },
    FireFrogHit {
        actor_id: u64,
    },
    SpiritResourceRefund {
        cinders_bits: u64,
    },
    WildfireExpire {
        until_ms: u64,
    },
}

#[derive(Debug)]
pub(crate) struct ArdeosState {
    pub(crate) cinders: f64,
    pub(crate) embers: u32,
    pub(crate) wildfire_until: u64,
    pub(crate) cascading_stacks: u32,
    pub(crate) apocalyptic_surge: u32,
    pub(crate) apocalyptic_surge_until: u64,
    pub(crate) reign_fireball_stacks: u32,
    pub(crate) reign_fireball_until: u64,
    pub(crate) fire_frogs: BTreeMap<u64, FireFrogState>,
    pub(crate) fire_frog_busy_targets: Vec<u32>,
    pub(crate) fire_frog_actor_sequence: u64,
}

impl Iteration<'_> {
    pub(crate) fn handle_ardeos_event(&mut self, event: ArdeosEvent) {
        match event {
            ArdeosEvent::FlareUp {
                source,
                damage_bits,
                targets,
                context,
            } => {
                self.apply_flare_up(source, f64::from_bits(damage_bits), &targets, context);
            }
            ArdeosEvent::DetonateHit {
                index,
                damage_bits,
                target_index,
                context,
            } => {
                let ability = self.profile.abilities[index].clone();
                self.damage_detonate(
                    &ability,
                    f64::from_bits(damage_bits),
                    self.effective_critical_strike(Some(DpsAbilityKind::Detonate)),
                    target_index,
                    context,
                );
            }
            ArdeosEvent::FireFrogSpawn { actor_id } => self.spawn_fire_frog(actor_id),
            ArdeosEvent::FireFrogHit { actor_id } => self.fire_frog_hit(actor_id),
            ArdeosEvent::SpiritResourceRefund { cinders_bits } => {
                self.add_cinders(f64::from_bits(cinders_bits));
            }
            ArdeosEvent::WildfireExpire { until_ms } => {
                if self.hero.ardeos().wildfire_until == until_ms {
                    self.hero.ardeos_mut().wildfire_until = 0;
                    self.deactivate_fixed_buff(AplBuff::Wildfire);
                    self.reschedule_dot_ticks();
                }
            }
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn capture_ardeos_wave(&mut self, cast: &mut PreparedCast) {
        if cast.ability.kind == DpsAbilityKind::InfernalWave {
            // The GA constructs the damage scale and records the proc on the
            // projectile before spawning it. Flight cannot change either.
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent4")
            {
                let count = self
                    .common
                    .dots
                    .iter()
                    .filter(|((target, kind), dot)| {
                        *target == 0
                            && kind.is_ardeos_hero_dot()
                            && dot.expires_ms > self.common.now_ms
                    })
                    .count();
                let increase = count as f64 * param(talent, parameter_key!("damagePerUniqueDot"));
                cast.context.multiply_damage(
                    1.0 + increase
                        .clamp(0.0, param(talent, parameter_key!("maximumDamageIncrease"))),
                );
            }
            if cast.ardeos_wave_empowered {
                let talent = self
                    .common
                    .selected_talents
                    .get("firemage-talent-id-talent5")
                    .expect("selected Cascading Inferno talent");
                self.hero.ardeos_mut().cascading_stacks = self
                    .hero
                    .ardeos()
                    .cascading_stacks
                    .saturating_sub(param_u32(talent, parameter_key!("stacksThreshold")));
                cast.bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
            }
        }
    }

    pub(crate) fn add_cinders(&mut self, amount: f64) {
        let maximum =
            f64::from(self.profile.max_secondary_resource) * self.profile.max_primary_resource;
        let total = (f64::from(self.hero.ardeos().embers) * self.profile.max_primary_resource
            + self.hero.ardeos().cinders
            + amount)
            .clamp(0.0, maximum);
        self.hero.ardeos_mut().embers = (total / self.profile.max_primary_resource).floor() as u32;
        self.hero.ardeos_mut().cinders =
            if self.hero.ardeos().embers >= self.profile.max_secondary_resource {
                0.0
            } else {
                total - f64::from(self.hero.ardeos().embers) * self.profile.max_primary_resource
            };
    }
}

impl Iteration<'_> {
    pub(crate) fn execute_fire_frog_batch(
        &mut self,
        ability: &CompiledAbility,
        batch: FireFrogBatch,
    ) {
        if batch.frog_count == 0 || batch.attacks_per_frog == 0 || self.common.target_count == 0 {
            return;
        }
        if !self.charge_work_units(u64::from(self.common.target_count))
            || !self.charge_work_product(
                u64::from(batch.frog_count),
                u64::from(self.common.target_count),
            )
        {
            return;
        }

        // AcquireInitialGroupTargets inserts the selected target first, then
        // AddUnique-appends every other actor in the stacked 500-unit volume.
        let mut initial_group = Vec::with_capacity(self.common.target_count as usize);
        initial_group.push(batch.main_target.min(self.common.target_count - 1));
        initial_group
            .extend((0..self.common.target_count).filter(|target| *target != batch.main_target));
        let ability_index = self.common.abilities_by_kind[&DpsAbilityKind::FireFrogs];
        let snapshot = if batch.allow_bonus_toad {
            batch
                .context
                .source_snapshot
                .unwrap_or_else(|| self.capture_damage_source_snapshot(ability.kind))
        } else {
            // Pyrophibian creates a new Fire Frogs damage spec when its DoT
            // event procs, independently of the triggering effect's snapshot.
            self.capture_damage_source_snapshot(ability.kind)
        };
        let has_fire_toad = self.fire_toad_mechanic().is_some();
        let mut spawn_ms = self.common.now_ms;
        for frog_index in 0..batch.frog_count {
            let frog_context = if frog_index == 0 {
                batch.context
            } else {
                batch.context.without_cast_proc()
            };
            self.hero.ardeos_mut().fire_frog_actor_sequence =
                self.hero.ardeos().fire_frog_actor_sequence.wrapping_add(1);
            let actor_id = self.hero.ardeos().fire_frog_actor_sequence;
            self.hero.ardeos_mut().fire_frogs.insert(
                actor_id,
                FireFrogState {
                    ability_index,
                    target_index: initial_group[frog_index as usize % initial_group.len()],
                    target_order: initial_group.clone(),
                    attacks_remaining: batch.attacks_per_frog,
                    coefficient: batch.coefficient,
                    context: frog_context.with_snapshot(snapshot),
                    roll_for_toad: has_fire_toad,
                    is_toad: false,
                },
            );
            self.push_event(spawn_ms, ArdeosEvent::FireFrogSpawn { actor_id });
            if frog_index + 1 < batch.frog_count {
                spawn_ms = spawn_ms.saturating_add(self.sample_fire_frog_timer_ms(
                    ability,
                    parameter_key!("minimumBatchSpawnDelaySeconds"),
                    parameter_key!("maximumBatchSpawnDelaySeconds"),
                ));
            }
        }

        if batch.allow_bonus_toad
            && let Some(mechanic) = self.fire_toad_mechanic()
        {
            let bonus_count = mechanic_param(&mechanic, parameter_key!("fireToadBonusCount"))
                .round()
                .clamp(0.0, u32::MAX as f64) as u32;
            if !self
                .charge_work_product(u64::from(bonus_count), u64::from(self.common.target_count))
            {
                return;
            }
            for bonus_index in 0..bonus_count {
                if bonus_index > 0 {
                    spawn_ms = spawn_ms.saturating_add(self.sample_fire_frog_timer_ms(
                        ability,
                        parameter_key!("minimumBatchSpawnDelaySeconds"),
                        parameter_key!("maximumBatchSpawnDelaySeconds"),
                    ));
                }
                self.hero.ardeos_mut().fire_frog_actor_sequence =
                    self.hero.ardeos().fire_frog_actor_sequence.wrapping_add(1);
                let actor_id = self.hero.ardeos().fire_frog_actor_sequence;
                self.hero.ardeos_mut().fire_frogs.insert(
                    actor_id,
                    FireFrogState {
                        ability_index,
                        target_index: initial_group[bonus_index as usize % initial_group.len()],
                        target_order: initial_group.clone(),
                        attacks_remaining: 1,
                        coefficient: batch.coefficient,
                        context: batch.context.without_cast_proc().with_snapshot(snapshot),
                        roll_for_toad: false,
                        is_toad: true,
                    },
                );
                self.push_event(spawn_ms, ArdeosEvent::FireFrogSpawn { actor_id });
            }
        }
    }

    pub(crate) fn fire_toad_mechanic(&self) -> Option<Arc<CompiledMechanic>> {
        self.profile
            .mechanic_indexes
            .fire_toad
            .map(|index| Arc::clone(&self.profile.mechanics[index]))
    }

    pub(crate) fn spawn_fire_frog(&mut self, actor_id: u64) {
        let Some(mut state) = self.hero.ardeos_mut().fire_frogs.remove(&actor_id) else {
            return;
        };
        let ability = self.profile.abilities[state.ability_index].clone();
        if state.roll_for_toad
            && let Some(mechanic) = self.fire_toad_mechanic()
            && self.roll_controlled_random_bool(
                FIRE_TOAD_RANDOM_STREAM_TAG,
                mechanic_param(&mechanic, parameter_key!("fireToadSpawnChance")),
            )
        {
            state.is_toad = true;
            state.attacks_remaining = 1;
            self.record_dynamic_random_proc(&mechanic);
        }
        self.reserve_fire_frog_target(state.target_index);
        let delay_ms = self.sample_fire_frog_first_hit_delay_ms(&ability);
        self.hero.ardeos_mut().fire_frogs.insert(actor_id, state);
        self.push_event(
            self.common.now_ms.saturating_add(delay_ms),
            ArdeosEvent::FireFrogHit { actor_id },
        );
    }

    pub(crate) fn fire_frog_hit(&mut self, actor_id: u64) {
        let Some(mut state) = self.hero.ardeos_mut().fire_frogs.remove(&actor_id) else {
            return;
        };
        let ability = self.profile.abilities[state.ability_index].clone();
        if state.is_toad {
            if let Some(mechanic) = self.fire_toad_mechanic() {
                self.execute_fire_toad_hit(
                    &ability,
                    state.target_index,
                    state.coefficient,
                    state.context,
                    &mechanic,
                );
            }
            state.context = state.context.without_cast_proc();
        } else {
            let hit_context = state.context;
            let damage = self
                .damage_hit(
                    &ability,
                    state.coefficient * self.profile.power,
                    0.0,
                    true,
                    state.target_index,
                    hit_context,
                )
                .damage;
            state.context = state.context.without_cast_proc();
            self.transfer_fire_frog_damage(&ability, state.target_index, damage, state.context);
        }

        state.attacks_remaining = state.attacks_remaining.saturating_sub(1);
        if state.attacks_remaining == 0 {
            self.release_fire_frog_target(state.target_index);
            return;
        }

        let previous_target = state.target_index;
        let next_target = state
            .target_order
            .iter()
            .copied()
            .find(|target| self.hero.ardeos().fire_frog_busy_targets[*target as usize] == 0)
            .unwrap_or_else(|| {
                self.common.rng.shuffle(&mut state.target_order);
                state
                    .target_order
                    .iter()
                    .copied()
                    .find(|target| *target != previous_target)
                    .unwrap_or(previous_target)
            });
        // The tracker reserves the replacement before releasing the previous
        // target, and its BusyTargets array preserves duplicate reservations.
        self.reserve_fire_frog_target(next_target);
        self.release_fire_frog_target(previous_target);
        state.target_index = next_target;
        let next_hit_delay_ms = self
            .sample_fire_frog_timer_ms(
                &ability,
                parameter_key!("minimumJumpPeriodSeconds"),
                parameter_key!("maximumJumpPeriodSeconds"),
            )
            .saturating_add(
                self.quantized_fire_frog_duration_ms(
                    &ability,
                    parameter_key!("jumpDurationSeconds"),
                ),
            );
        self.hero.ardeos_mut().fire_frogs.insert(actor_id, state);
        self.push_event(
            self.common.now_ms.saturating_add(next_hit_delay_ms),
            ArdeosEvent::FireFrogHit { actor_id },
        );
    }

    pub(crate) fn execute_fire_toad_hit(
        &mut self,
        ability: &CompiledAbility,
        main_target: u32,
        coefficient: f64,
        mut context: DamageContext,
        mechanic: &CompiledMechanic,
    ) {
        let main_damage = self
            .damage_hit(
                ability,
                coefficient
                    * self.profile.power
                    * mechanic_param(mechanic, parameter_key!("fireToadDamageMultiplier")),
                0.0,
                true,
                main_target,
                context,
            )
            .damage;
        context = context.without_cast_proc();
        self.transfer_fire_frog_damage(ability, main_target, main_damage, context);

        let nearby_targets = self.common.target_count.saturating_sub(1);
        if nearby_targets == 0 {
            return;
        }
        let threshold = mechanic_param(mechanic, parameter_key!("fireToadTargetCountThreshold"));
        let falloff = multi_target_damage_falloff(nearby_targets, threshold);
        let base = coefficient
            * self.profile.power
            * mechanic_param(mechanic, parameter_key!("fireToadAoeDamageMultiplier"))
            * falloff;
        if !self.charge_work_units(u64::from(nearby_targets)) {
            return;
        }
        for target_index in (0..self.common.target_count).filter(|target| *target != main_target) {
            let damage = self
                .damage_hit(ability, base, 0.0, true, target_index, context)
                .damage;
            self.transfer_fire_frog_damage(ability, target_index, damage, context);
        }
    }

    pub(crate) fn transfer_fire_frog_damage(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        damage: f64,
        context: DamageContext,
    ) {
        let Some(dot) = ability.dot else {
            return;
        };
        self.apply_damage_derived_dot(
            target_index,
            DotKind::Ability(DpsAbilityKind::FireFrogs),
            self.ability_damage_source(ability),
            dot,
            damage * ability_param(ability, parameter_key!("damageToDotTransferFraction")),
            context,
        );
    }

    pub(crate) fn reserve_fire_frog_target(&mut self, target_index: u32) {
        let busy = &mut self.hero.ardeos_mut().fire_frog_busy_targets[target_index as usize];
        *busy = busy.saturating_add(1);
    }

    pub(crate) fn release_fire_frog_target(&mut self, target_index: u32) {
        let busy = &mut self.hero.ardeos_mut().fire_frog_busy_targets[target_index as usize];
        *busy = busy.saturating_sub(1);
    }

    pub(crate) fn sample_fire_frog_first_hit_delay_ms(&mut self, ability: &CompiledAbility) -> u64 {
        let distance = ability_param(ability, parameter_key!("initialPathDistanceUnits"));
        let spawn_radius = ability_param(ability, parameter_key!("spawnRadiusUnits"));
        // GetNextTurretSpawnPoint places each actor on the extracted 30-unit
        // ring around the hero using random axes. In the straight, flat
        // stationary scenario, a uniform angle is the closest map-independent
        // representation of that native placement helper.
        let spawn_angle = self.common.rng.uniform_f64(0.0, std::f64::consts::TAU);
        let mut remaining_distance = (distance * distance + spawn_radius * spawn_radius
            - 2.0 * distance * spawn_radius * spawn_angle.cos())
        .sqrt();
        let attack_range = ability_param(ability, parameter_key!("attackRangeUnits"));
        let mut delay_ms =
            self.quantized_fire_frog_duration_ms(ability, parameter_key!("jumpDurationSeconds"));
        while remaining_distance > attack_range {
            if !self.charge_work_units(1) {
                return 0;
            }
            delay_ms = delay_ms.saturating_add(self.sample_fire_frog_timer_ms(
                ability,
                parameter_key!("minimumJumpPeriodSeconds"),
                parameter_key!("maximumJumpPeriodSeconds"),
            ));
            remaining_distance -= self.common.rng.uniform_f64(
                ability_param(ability, parameter_key!("minimumJumpLengthUnits")),
                ability_param(ability, parameter_key!("maximumJumpLengthUnits")),
            );
        }
        delay_ms = delay_ms.saturating_add(self.sample_fire_frog_timer_ms(
            ability,
            parameter_key!("minimumJumpPeriodSeconds"),
            parameter_key!("maximumJumpPeriodSeconds"),
        ));
        delay_ms.saturating_add(
            self.quantized_fire_frog_duration_ms(ability, parameter_key!("jumpDurationSeconds")),
        )
    }

    pub(crate) fn sample_fire_frog_timer_ms(
        &mut self,
        ability: &CompiledAbility,
        minimum_parameter: ParameterKey,
        maximum_parameter: ParameterKey,
    ) -> u64 {
        let seconds = self.common.rng.uniform_f64(
            ability_param(ability, minimum_parameter),
            ability_param(ability, maximum_parameter),
        );
        self.quantize_fire_frog_seconds(ability, seconds)
    }

    pub(crate) fn quantized_fire_frog_duration_ms(
        &self,
        ability: &CompiledAbility,
        parameter: ParameterKey,
    ) -> u64 {
        self.quantize_fire_frog_seconds(ability, ability_param(ability, parameter))
    }

    pub(crate) fn quantize_fire_frog_seconds(
        &self,
        ability: &CompiledAbility,
        seconds: f64,
    ) -> u64 {
        let tick_rate = ability_param(ability, parameter_key!("assumedServerTickRateHz")).max(1.0);
        let ticks = (seconds.max(0.0) * tick_rate).ceil();
        (ticks * 1_000.0 / tick_rate).round().max(1.0) as u64
    }

    pub(crate) fn try_pyrophibian(
        &mut self,
        critical: bool,
        target_index: u32,
        context: DamageContext,
    ) {
        let Some((chance, number_of_frogs)) = self
            .common
            .selected_talents
            .get("firemage-talent-id-talent1")
            .map(|talent| {
                (
                    if critical {
                        param(talent, parameter_key!("criticalProcChance"))
                    } else {
                        param(talent, parameter_key!("procChance"))
                    },
                    param(talent, parameter_key!("numberOfFrogs"))
                        .round()
                        .clamp(0.0, u32::MAX as f64) as u32,
                )
            })
        else {
            return;
        };
        let stream_tag = if critical {
            PYROPHIBIAN_CRIT_RANDOM_STREAM_TAG
        } else {
            PYROPHIBIAN_NON_CRIT_RANDOM_STREAM_TAG
        };
        if !self.roll_controlled_random_bool(stream_tag, chance) {
            return;
        }
        self.record_talent_proc("firemage-talent-id-talent1");
        if let Some(frogs) = self.ability(DpsAbilityKind::FireFrogs).cloned() {
            let extra_leaps = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent6")
                .map(|talent| param_u32(talent, parameter_key!("additionalLeaps")))
                .unwrap_or(0);
            let attacks_per_frog = ability_u32_rounded(&frogs, parameter_key!("attacksPerFrog"))
                .saturating_add(extra_leaps)
                .max(1);
            let damage_scale = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent6")
                .map(|talent| 1.0 + param(talent, parameter_key!("damageIncrease")))
                .unwrap_or(1.0);
            self.execute_fire_frog_batch(
                &frogs,
                FireFrogBatch {
                    main_target: target_index,
                    frog_count: number_of_frogs,
                    attacks_per_frog,
                    coefficient: frogs.power_coefficient * damage_scale,
                    context: context.as_proc(),
                    allow_bonus_toad: false,
                },
            );
        }
    }

    pub(crate) fn try_flare_up(
        &mut self,
        triggering_damage: f64,
        _primary_target: u32,
        context: DamageContext,
    ) {
        let Some((damage_fraction, maximum_radius, threshold, visual_delay_ms)) = self
            .common
            .selected_talents
            .get("firemage-talent-id-talent17")
            .filter(|talent| talent.classification == MechanicClassification::Modeled)
            .map(|talent| {
                (
                    param(talent, parameter_key!("damageFraction")),
                    param(talent, parameter_key!("maximumRadius")),
                    param(talent, parameter_key!("targetCountDamageScalingThreshold")),
                    ms_param(talent, parameter_key!("visualDelaySeconds")),
                )
            })
        else {
            return;
        };
        if triggering_damage <= 0.0 || damage_fraction <= 0.0 || maximum_radius <= 0.0 {
            return;
        }
        let targets = (0..self.common.target_count)
            .filter(|target| {
                self.common
                    .dots
                    .get(&(*target, DotKind::Ability(DpsAbilityKind::SearingBlaze)))
                    .is_some_and(|dot| dot.expires_ms > self.common.now_ms)
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return;
        }
        let damage = triggering_damage
            * damage_fraction
            * multi_target_damage_falloff(targets.len() as u32, threshold);
        let source = self.damage_source_key("firemage-talent-id-talent17");
        if visual_delay_ms > 0 {
            self.push_event(
                self.common.now_ms.saturating_add(visual_delay_ms),
                ArdeosEvent::FlareUp {
                    source,
                    damage_bits: damage.to_bits(),
                    targets,
                    context: context.as_proc(),
                },
            );
            return;
        }
        self.apply_flare_up(source, damage, &targets, context.as_proc());
    }

    pub(crate) fn apply_flare_up(
        &mut self,
        source: DamageSourceKey,
        damage: f64,
        targets: &[u32],
        context: DamageContext,
    ) {
        if !self.common.execution.charge_usize(targets.len()) {
            return;
        }
        for target in targets {
            self.emit_outgoing_damage(OutgoingDamageEvent {
                source,
                ability_kind: None,
                context,
                target_index: *target,
                provenance: DamageProvenance::Proc,
                amount: DamageAmount::Unscaled(damage),
                expertise_snapshot: None,
                primary_stat_multiplier_snapshot: None,
                damage_spread: 0.0,
                bonus_crit: 0.0,
                critical_chance_override: None,
                can_crit: false,
                proc_eligible: false,
            });
        }
    }

    pub(crate) fn try_reign_of_fire(&mut self) {
        let Some(talent) = self
            .common
            .selected_talents
            .get("firemage-talent-id-talent2")
        else {
            return;
        };
        let ppm = param(talent, parameter_key!("procsPerMinute"));
        let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
        let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
        if self.roll_proc_per_minute(REIGN_OF_FIRE_PPM_STREAM_TAG, ppm, true) {
            // The current graph applies the buff after IncreaseNumOfCharges
            // even when Fire Ball already has every charge available.
            self.restore_ability_charge(DpsAbilityKind::FireBall);
            let now = self.common.now_ms;
            let state = self.hero.ardeos_mut();
            if state.reign_fireball_until <= now {
                state.reign_fireball_stacks = 0;
            }
            state.reign_fireball_stacks = state
                .reign_fireball_stacks
                .saturating_add(1)
                .min(maximum_stacks);
            state.reign_fireball_until = now.saturating_add(duration_ms);
            self.record_talent_proc("firemage-talent-id-talent2");
        }
    }

    pub(crate) fn consume_reign_of_fire(&mut self) -> f64 {
        if self.hero.ardeos().reign_fireball_until <= self.common.now_ms {
            self.hero.ardeos_mut().reign_fireball_stacks = 0;
        }
        if self.hero.ardeos().reign_fireball_stacks == 0 {
            return 0.0;
        }
        self.hero.ardeos_mut().reign_fireball_stacks -= 1;
        self.common
            .selected_talents
            .get("firemage-talent-id-talent2")
            .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
            .unwrap_or(0.0)
    }

    pub(crate) fn try_firemage_resource_reward(
        &mut self,
        source: Option<DpsAbilityKind>,
        chance: f64,
        cinders: f64,
    ) {
        if cinders <= 0.0
            || !self.roll_controlled_random_bool(DOT_TICK_RESOURCE_RANDOM_STREAM_TAG, chance)
        {
            return;
        }
        if let Some(source) = source {
            self.record_ability_proc(source);
        }
        let haste = (1.0 + self.effective_haste()).max(0.05);
        self.add_cinders(cinders / haste);
    }

    pub(crate) fn try_firemage_spirit_refund(&mut self, ability: &CompiledAbility) {
        let chance = spirit_refund_chance(
            self.effective_spirit(),
            ability_param(ability, parameter_key!("spiritRefundChanceScale")),
            ability_param(ability, parameter_key!("spiritRefundChanceFlatIncrease")),
        );
        if !self.roll_controlled_random_bool(SPIRIT_PROC_RANDOM_STREAM_TAG, chance) {
            return;
        }
        self.record_spirit_refund_proc();

        let cinders =
            f64::from(ability.secondary_resource_cost) * self.profile.max_primary_resource;
        self.push_event(
            self.common.now_ms.saturating_add(ability_seconds_parameter(
                ability,
                parameter_key!("spiritRefundDelaySeconds"),
            )),
            ArdeosEvent::SpiritResourceRefund {
                cinders_bits: cinders.to_bits(),
            },
        );
        self.shared.spirit = (self.shared.spirit
            + ability
                .parameters
                .get(parameter_key!("spiritRefundSpiritGain"))
                .unwrap_or(0.0))
        .min(self.profile.max_spirit);
    }
}
