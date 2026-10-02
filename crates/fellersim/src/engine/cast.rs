use crate::*;

impl Iteration<'_> {
    #[cfg(test)]
    pub(crate) fn cast(&mut self, index: usize) {
        self.prepare_cast(index);
    }

    pub(crate) fn commit_prepared_cast(&mut self, mut cast: PreparedCast) {
        if matches!(
            cast.ability.kind,
            DpsAbilityKind::GlacialBlast | DpsAbilityKind::IceComet
        ) && !cast.glacial_assault
        {
            self.spend_rime_orbs(cast.ability.secondary_resource_cost);
        }
        self.commit_ability(
            cast.index,
            &mut cast.context,
            cast.free_charge,
            cast.elarion_focus_cost,
        );
        if cast.ability.kind == DpsAbilityKind::Multishot && cast.elarion_multishot_arrows > 0 {
            // Commit listeners observe the empowerment tags before OnEnd consumes charges.
            let state = self.hero.elarion_mut();
            if self.common.now_ms < state.empowered_multishot_until {
                state.empowered_multishot_stacks =
                    state.empowered_multishot_stacks.saturating_sub(1);
                if state.empowered_multishot_stacks == 0 {
                    state.empowered_multishot_until = 0;
                }
            }
            if self.common.now_ms < state.skystriders_supremacy_until
                && self
                    .common
                    .selected_talents
                    .contains_key("bowguy-talent-id-talent13")
            {
                state.skystriders_supremacy_stacks =
                    state.skystriders_supremacy_stacks.saturating_sub(1);
                if state.skystriders_supremacy_stacks == 0 {
                    state.skystriders_supremacy_until = 0;
                }
            }
            if self.hero.elarion().empowered_multishot_until == 0 {
                self.deactivate_fixed_buff(AplBuff::EmpoweredMultishot);
            }
            if self.hero.elarion().skystriders_supremacy_until == 0 {
                self.deactivate_fixed_buff(AplBuff::SkystridersSupremacy);
            }
        }
        if cast.ability.kind == DpsAbilityKind::IceComet {
            let delay = ability_seconds_parameter(
                &cast.ability,
                parameter_key!("initialSpawnDelaySeconds"),
            );
            self.schedule_rime_comet(cast.context, delay);
            return;
        }
        if cast.ability.kind == DpsAbilityKind::GlacialBlast {
            cast.bonus_crit =
                self.capture_rime_spender_bonuses(cast.glacial_assault, &mut cast.context);
            self.hero.rime_mut().icy_flow_casting_haste = 0.0;
        }
        if cast.ability.kind == DpsAbilityKind::HighwindArrow
                && let Some(talent) = self.common.selected_talents.get("bowguy-talent-id-talent6")
                && self.roll_controlled_random_bool(
                    "RandomStream.Bowguy.Talent.CastedProjectileHeavyDamage.ChanceToHaveIncreasedCritChance",
                    param(talent, parameter_key!("procChance")),
                )
            {
                self.record_talent_proc("bowguy-talent-id-talent6");
                cast.bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
            }
        self.capture_ardeos_wave(&mut cast);
        let mut consume_executioners_grin = false;
        if cast.ability.kind == DpsAbilityKind::CullingStrike {
            if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent13") {
                self.shared.spirit = (self.shared.spirit
                    + param(talent, parameter_key!("spiritPerCast")))
                .min(self.profile.max_spirit);
            }
            let executioners_grin = self.common.now_ms < self.hero.tariq().executioners_grin_until
                && self.hero.tariq().executioners_grin_stacks > 0;
            let spent = if executioners_grin {
                let mechanic = self
                    .profile
                    .mechanic_indexes
                    .tariq_executioners_grin
                    .map(|index| &self.profile.mechanics[index])
                    .expect("active Executioner's Grin has a compiled mechanic");
                mechanic_param(mechanic, parameter_key!("executionersGrinFurySpent"))
            } else {
                let spent = self.hero.tariq().fury.min(ability_param(
                    &cast.ability,
                    parameter_key!("maximumFuryCost"),
                ));
                self.hero.tariq_mut().fury -= spent;
                self.try_tariq_spirit_refund(&cast.ability, spent);
                spent
            };
            cast.context.multiply_damage(
                1.0 + spent
                    * ability_param(
                        &cast.ability,
                        parameter_key!("damageIncreasePerFuryFraction"),
                    ),
            );
            consume_executioners_grin = executioners_grin;
            cast.context.source_snapshot =
                Some(self.capture_damage_source_snapshot(cast.ability.kind));
        }
        if matches!(
            cast.ability.kind,
            DpsAbilityKind::QueensFang | DpsAbilityKind::ArachnidAssault
        ) && self.common.now_ms < self.hero.mara().matriarch_macabre_until
        {
            self.schedule_mara_matriarch_copies(&cast);
        }
        if cast.ability.kind == DpsAbilityKind::SkullCrusher
            && self.common.now_ms < self.hero.tariq().thunder_call_until
        {
            let delay = ((cast.ability.first_hit_delay_ms as f64
                + ability_seconds_parameter(&cast.ability, parameter_key!("lightningDelaySeconds"))
                    as f64)
                / cast.ability_time_rate)
                .round() as u64;
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                TariqEvent::SpenderLightning {
                    kind: cast.ability.kind,
                    scale_bits: 1.0_f64.to_bits(),
                    bonus_crit_bits: cast.bonus_crit.to_bits(),
                    context: cast.context,
                },
            );
        }
        let kind = cast.ability.kind;
        let rime_anima = cast.ability.primary_resource_generated;
        let rime_context = cast.context;
        self.schedule_prepared_cast_impacts(cast);
        if kind == DpsAbilityKind::HeavyStrike {
            self.schedule_tariq_heavy_strike_secondaries(rime_context);
        }
        if matches!(
            kind,
            DpsAbilityKind::HeavyStrike
                | DpsAbilityKind::WildSwing
                | DpsAbilityKind::FaceBreaker
                | DpsAbilityKind::SkullCrusher
        ) {
            self.reset_tariq_swing(false);
        }
        if kind == DpsAbilityKind::LeapSmash {
            let delay = self.ability(kind).unwrap().first_hit_delay_ms;
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                TariqEvent::LeapEnded,
            );
        }
        if kind == DpsAbilityKind::FocusedShot {
            // Both spawn callbacks grant Focus before the later projectile hit.
            let multiplier = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent17")
                .map(|talent| param(talent, parameter_key!("resourceMultiplier")))
                .unwrap_or(1.0);
            self.hero.elarion_mut().focus = (self.hero.elarion().focus + rime_anima * multiplier)
                .min(self.profile.max_primary_resource);
        }
        if kind == DpsAbilityKind::FrostBolt {
            // Both projectile-spawn callbacks enter the one-point resource loop.
            self.add_rime_anima(rime_anima, rime_context);
        }
        if self.profile.contract.hero == HeroIdentity::Mara {
            self.push_event(self.common.now_ms, MaraEvent::AbilityEnded { kind });
        }
        // K2_EndAbility follows ApplyGameplayEffectSpecToTarget. Visual delay
        // belongs to the spec; it does not postpone OnEnd proc consumption.
        if consume_executioners_grin {
            self.hero.tariq_mut().executioners_grin_stacks -= 1;
            if self.hero.tariq().executioners_grin_stacks == 0 {
                self.hero.tariq_mut().executioners_grin_until = 0;
            }
        }
    }

    pub(crate) fn capture_damage_source_snapshot(
        &self,
        ability_kind: DpsAbilityKind,
    ) -> DamageSourceSnapshot {
        DamageSourceSnapshot {
            hero_damage_scale: self.hero_damage_scale(Some(ability_kind)),
            expertise: self.effective_expertise(),
            primary_stat_multiplier: self.effective_power_multiplier(),
            critical_chance: self.effective_critical_strike(Some(ability_kind)),
        }
    }

    pub(crate) fn schedule_prepared_cast_impacts(&mut self, cast: PreparedCast) {
        let PreparedCast {
            index,
            ability,
            context,
            ability_time_rate,
            bonus_crit,
            ardeos_wave_empowered,
            glacial_assault,
            elarion_multishot_arrows,
            elarion_celestial_impetus,
            impending_heartseeker,
            mara_combo_points_spent,
            mara_from_stealth,
            gunde_rend_transfer_bonus,
            ..
        } = cast;
        if elarion_celestial_impetus
            && let Some(talent) = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent14")
        {
            self.common
                .cooldowns
                .remove(&DpsAbilityKind::HeartseekerBarrage);
            self.hero.elarion_mut().impending_heartseeker_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            self.activate_fixed_buff(
                AplBuff::ImpendingHeartseeker,
                self.hero.elarion().impending_heartseeker_until,
            );
        }
        let automatic_targets = self.automatic_target_count_for(ability.kind, ability.max_targets);
        let mut batches = BTreeMap::<u64, Vec<ImpactSpec>>::new();
        if ability.kind == DpsAbilityKind::Multishot && elarion_multishot_arrows > 0 {
            if !self.charge_work_units(u64::from(elarion_multishot_arrows)) {
                return;
            }
            for arrow in 0..elarion_multishot_arrows {
                batches
                    .entry(
                        self.common
                            .now_ms
                            .saturating_add(ability.first_hit_delay_ms),
                    )
                    .or_default()
                    .push(ImpactSpec {
                        single_hit: true,
                        damage_scale_bits: 1.0_f64.to_bits(),
                        target_index: if arrow < automatic_targets { arrow } else { 0 },
                    });
            }
        } else if ability.kind == DpsAbilityKind::TariqChainLightning {
            self.schedule_tariq_chain(context, 0);
            return;
        } else if ability.kind == DpsAbilityKind::WeaponChainLightning {
            let jump_delay_ms =
                ability_seconds_parameter(&ability, parameter_key!("jumpDelaySeconds"));
            let first_target_multiplier = ability
                .parameters
                .get(parameter_key!("firstTargetDamageMultiplier"))
                .unwrap_or(1.0);
            // Stable target order is the maintained encounter approximation.
            // Different target debuffs can make native random selection matter.
            // Each link constructs its own spec.
            if !self.charge_work_units(u64::from(automatic_targets)) {
                return;
            }
            for target_index in 0..automatic_targets {
                batches
                    .entry(
                        self.common.now_ms.saturating_add(
                            ability.first_hit_delay_ms.saturating_add(
                                jump_delay_ms.saturating_mul(u64::from(target_index)),
                            ),
                        ),
                    )
                    .or_default()
                    .push(ImpactSpec {
                        single_hit: true,
                        damage_scale_bits: if target_index == 0 {
                            first_target_multiplier.to_bits()
                        } else {
                            1.0_f64.to_bits()
                        },
                        target_index,
                    });
            }
        } else if ability.kind == DpsAbilityKind::WeaponFrontalCone {
            let initial = ability_param(&ability, parameter_key!("initialPowerCoefficient"));
            let repeating = ability_param(&ability, parameter_key!("repeatingPowerCoefficient"));
            let final_coefficient =
                ability_param(&ability, parameter_key!("finalPowerCoefficient"));
            let duration_ms =
                ability_seconds_parameter(&ability, parameter_key!("coneDurationSeconds"));
            // BeginPlay starts all three timers together. Only the repeating
            // interval scales with AbilityTimeRate; activation and life do not.
            let interval_ms =
                (ability_seconds_parameter(&ability, parameter_key!("coneTickIntervalSeconds"))
                    as f64
                    / ability_time_rate)
                    .round()
                    .max(1.0) as u64;
            let repeating_count = if duration_ms > interval_ms {
                (duration_ms - 1) / interval_ms
            } else {
                0
            };
            let Some(offset_count) = repeating_count.checked_add(2) else {
                self.common.execution.fail_limit();
                return;
            };
            if !self.charge_work_units(offset_count)
                || !self.charge_work_product(offset_count, u64::from(automatic_targets))
            {
                return;
            }
            let mut offsets = Vec::with_capacity(offset_count as usize);
            offsets.push((ability.first_hit_delay_ms, initial));
            let mut elapsed = interval_ms;
            while elapsed < duration_ms {
                offsets.push((elapsed, repeating));
                elapsed = elapsed.saturating_add(interval_ms);
            }
            offsets.push((duration_ms, final_coefficient));
            for target_index in 0..automatic_targets {
                for (offset, coefficient) in &offsets {
                    batches
                        .entry(self.common.now_ms.saturating_add(*offset))
                        .or_default()
                        .push(ImpactSpec {
                            single_hit: true,
                            damage_scale_bits: (*coefficient / ability.power_coefficient).to_bits(),
                            target_index,
                        });
                }
            }
        } else {
            let impacts_per_target = if ability.hit_interval_ms > 0 && ability.direct_hits > 1 {
                ability.direct_hits
            } else {
                1
            };
            if !self
                .charge_work_product(u64::from(automatic_targets), u64::from(impacts_per_target))
            {
                return;
            }
            for target_index in 0..automatic_targets {
                if ability.hit_interval_ms > 0 && ability.direct_hits > 1 {
                    for hit in 0..ability.direct_hits {
                        let offset_ms = if ability.kind == DpsAbilityKind::GrimCarve {
                            ability.first_hit_delay_ms
                                + ability.hit_interval_ms.saturating_mul(hit as u64)
                        } else {
                            (((ability.first_hit_delay_ms
                                + ability.hit_interval_ms.saturating_mul(hit as u64))
                                as f64)
                                / ability_time_rate)
                                .round()
                                .max(1.0) as u64
                        };
                        batches
                            .entry(self.common.now_ms.saturating_add(offset_ms))
                            .or_default()
                            .push(ImpactSpec {
                                single_hit: true,
                                damage_scale_bits: if ability.kind == DpsAbilityKind::WidowsBite
                                    && hit == 1
                                {
                                    ability_param(
                                        &ability,
                                        parameter_key!("secondaryHitMultiplier"),
                                    )
                                    .to_bits()
                                } else {
                                    1.0_f64.to_bits()
                                },
                                target_index,
                            });
                    }
                } else {
                    batches
                        .entry(self.common.now_ms.saturating_add(
                            if self.profile.contract.hero == HeroIdentity::Mara
                                || (self.profile.contract.hero == HeroIdentity::Tariq
                                    && ability.kind != DpsAbilityKind::LeapSmash)
                            {
                                (ability.first_hit_delay_ms as f64 / ability_time_rate).round()
                                    as u64
                            } else {
                                ability.first_hit_delay_ms
                            },
                        ))
                        .or_default()
                        .push(ImpactSpec {
                            single_hit: false,
                            // Frostwyrm builds a separate secondary-target spec;
                            // its falloff excludes the primary Cold Snap hit.
                            damage_scale_bits: if ability.kind == DpsAbilityKind::ColdSnap
                                && target_index > 0
                                && let Some(index) = self.profile.mechanic_indexes.frostwyrm
                            {
                                let mechanic = &self.profile.mechanics[index];
                                multi_target_damage_falloff(
                                    automatic_targets - 1,
                                    mechanic_param(
                                        mechanic,
                                        parameter_key!("frostwyrmTargetCountThreshold"),
                                    ),
                                )
                                .to_bits()
                            } else {
                                1.0_f64.to_bits()
                            },
                            target_index,
                        });
                }
            }
        }
        let mut first_batch = true;
        for (at_ms, impacts) in batches {
            self.push_event(
                at_ms,
                CoreEvent::ImpactBatch {
                    index,
                    impacts,
                    context: CastImpactContext {
                        damage: if first_batch {
                            context
                        } else {
                            context.without_cast_proc()
                        },
                        bonus_crit,
                        ardeos_wave_empowered,
                        glacial_assault: glacial_assault && first_batch,
                        frostweaver: false,
                        impending_heartseeker,
                        elarion_celestial_impetus,
                        mara_combo_points_spent,
                        mara_from_stealth,
                        gunde_rend_transfer_bonus,
                        gunde_legendary_strike: false,
                    },
                },
            );
            first_batch = false;
        }
        if ability.kind == DpsAbilityKind::HighwindArrow {
            self.push_event(
                self.common
                    .now_ms
                    .saturating_add(ability.first_hit_delay_ms),
                ElarionEvent::HighwindFinished {
                    ability: ability.clone(),
                    targets_hit: automatic_targets,
                },
            );
        }
        if mara_from_stealth
            && matches!(
                ability.kind,
                DpsAbilityKind::SkitteringBlades
                    | DpsAbilityKind::WidowsBite
                    | DpsAbilityKind::Backstab
            )
        {
            let delay = (ability.first_hit_delay_ms as f64
                + ability_param(&ability, parameter_key!("poisonAdditionalDelaySeconds"))
                    * 1_000.0)
                / ability_time_rate;
            self.push_event(
                self.common.now_ms.saturating_add(delay.round() as u64),
                MaraEvent::StealthPoison {
                    ability_kind: ability.kind,
                    context,
                },
            );
        }
        if ability.kind == DpsAbilityKind::WeaponFrontalCone {
            let duration_ms =
                ability_seconds_parameter(&ability, parameter_key!("coneDurationSeconds"));
            // The actor applies StunSpec after the initial and final damage
            // batches. Repeating pulses apply damage alone.
            for delay in [ability.first_hit_delay_ms, duration_ms] {
                self.push_event(
                    self.common.now_ms.saturating_add(delay),
                    SharedEvent::WeaponConeStun {
                        index,
                        targets: automatic_targets,
                    },
                );
            }
        }
        if ability.kind == DpsAbilityKind::WeaponFrostVolley
            && ability.dot.is_some()
            && ability.direct_hits > 0
        {
            let final_offset_ms = (((ability.first_hit_delay_ms
                + ability
                    .hit_interval_ms
                    .saturating_mul(u64::from(ability.direct_hits - 1)))
                as f64)
                / ability_time_rate)
                .round()
                .max(1.0) as u64;
            self.push_event(
                self.common.now_ms.saturating_add(final_offset_ms),
                CoreEvent::ApplyAbilityDot {
                    index,
                    targets: (0..automatic_targets).collect(),
                    context: context.without_cast_proc(),
                },
            );
        }
    }

    pub(crate) fn schedule_channel_impacts(
        &mut self,
        index: usize,
        ability: &PreparedAbility,
        context: CastImpactContext,
        ability_time_rate: f64,
    ) -> u64 {
        let channel = ability
            .channel
            .as_ref()
            .expect("channel scheduling requires channel data");
        let duration_rate = if channel.scale_duration_with_ability_time_rate {
            ability_time_rate
        } else {
            1.0
        };
        let duration_ms = channel.duration_ms as f64 / duration_rate;
        let interval_ms = channel.tick_interval_ms as f64 / ability_time_rate;
        let ratio = duration_ms / interval_ms;
        let full_ticks = ratio.floor().max(0.0) as u64;
        let partial_factor = ratio - full_ticks as f64;
        let immediate_ticks = u64::from(channel.tick_immediately);
        let partial_ticks = u64::from(channel.enable_partial_ticks && partial_factor > 1.0e-6);
        let Some(tick_count) = full_ticks
            .checked_add(immediate_ticks)
            .and_then(|count| count.checked_add(partial_ticks))
        else {
            self.common.execution.fail_limit();
            return 0;
        };
        if !self.charge_work_units(tick_count) {
            return 0;
        }
        let automatic_targets = self.automatic_target_count_for(ability.kind, ability.max_targets);
        if !self.charge_work_product(tick_count, u64::from(automatic_targets)) {
            return 0;
        }
        let mut ticks = Vec::with_capacity(tick_count as usize);
        if channel.tick_immediately {
            ticks.push((0.0, 1.0));
        }
        for tick in 1..=full_ticks {
            ticks.push((tick as f64 * interval_ms, 1.0));
        }
        if channel.enable_partial_ticks && partial_factor > 1.0e-6 {
            ticks.push((
                duration_ms,
                if channel.scale_partial_tick_damage {
                    partial_factor
                } else {
                    1.0
                },
            ));
        }
        let mut batches = BTreeMap::<u64, Vec<ImpactSpec>>::new();
        if ability.kind == DpsAbilityKind::HammerStorm
            && self.common.now_ms < self.hero.tariq().thunder_call_until
        {
            let visual_delay =
                (ability_seconds_parameter(ability, parameter_key!("lightningDelaySeconds")) as f64
                    / ability_time_rate)
                    .round() as u64;
            for (tick, (offset, scale)) in ticks.iter().enumerate() {
                self.push_event(
                    self.common
                        .now_ms
                        .saturating_add(offset.round() as u64)
                        .saturating_add(visual_delay),
                    TariqEvent::SpenderLightning {
                        kind: ability.kind,
                        scale_bits: (scale
                            * ability_param(ability, parameter_key!("tickDamageMultiplier"))
                                .powi(tick as i32))
                        .to_bits(),
                        bonus_crit_bits: 0.0_f64.to_bits(),
                        context: context.damage.without_cast_proc(),
                    },
                );
            }
        }
        for target_index in 0..automatic_targets {
            for (tick_index, (offset_ms, damage_scale)) in ticks.iter().enumerate() {
                let damage_scale = if ability.kind == DpsAbilityKind::HammerStorm {
                    damage_scale
                        * ability_param(ability, parameter_key!("tickDamageMultiplier"))
                            .powi(tick_index as i32)
                } else if ability.kind == DpsAbilityKind::HeartseekerBarrage
                    && context.impending_heartseeker
                {
                    damage_scale
                        * (1.0
                            + ability_param(
                                ability,
                                parameter_key!("impendingDamageIncreasePerProjectile"),
                            ) * tick_index as f64)
                } else {
                    *damage_scale
                };
                batches
                    .entry(
                        self.common
                            .now_ms
                            .saturating_add(offset_ms.round().max(0.0) as u64),
                    )
                    .or_default()
                    .push(ImpactSpec {
                        single_hit: true,
                        damage_scale_bits: damage_scale.to_bits(),
                        target_index,
                    });
            }
        }
        let mut first_batch = true;
        for (at_ms, impacts) in batches {
            let context = CastImpactContext {
                damage: if first_batch {
                    context.damage
                } else {
                    context.damage.without_cast_proc()
                },
                glacial_assault: context.glacial_assault && first_batch,
                ..context
            };
            if ability.kind == DpsAbilityKind::HeartseekerBarrage {
                self.push_event(
                    at_ms,
                    ElarionEvent::ProjectileBatch {
                        index,
                        impacts,
                        context,
                    },
                );
            } else {
                self.push_event(
                    at_ms,
                    CoreEvent::ImpactBatch {
                        index,
                        impacts,
                        context,
                    },
                );
            }
            first_batch = false;
        }
        duration_ms.round().max(0.0) as u64
    }

    pub(crate) fn spend_ability_charge(&mut self, ability: &CompiledAbility, cooldown_ms: u64) {
        if cooldown_ms == 0 {
            return;
        }
        let cooldown = self.common.cooldowns.entry(ability.kind).or_default();
        if cooldown.used_charges == 0 {
            cooldown.remaining_ms = cooldown_ms as f64;
        }
        cooldown.used_charges = cooldown
            .used_charges
            .saturating_add(1)
            .min(ability.maximum_charges.max(1));
    }

    pub(crate) fn wait_for_ability_lock(
        &mut self,
        cast_ms: u64,
        mut gcd_remaining_ms: f64,
        gcd_scales_with_cooldown_recovery: bool,
    ) {
        let cast_until = self.common.now_ms.saturating_add(cast_ms);
        self.process_events_through(self.common.now_ms);
        while self.common.now_ms < ENCOUNTER_DURATION_MS
            && (self.common.now_ms < cast_until
                || self.common.now_ms < self.shared.weapon_charge_lock_until
                || gcd_remaining_ms > 0.0)
        {
            if self.common.execution.failed() {
                return;
            }
            let gcd_rate = if gcd_scales_with_cooldown_recovery {
                self.effective_cooldown_recovery()
            } else {
                1.0
            };
            let mut next = ENCOUNTER_DURATION_MS;
            if cast_until > self.common.now_ms {
                next = next.min(cast_until);
            }
            if self.shared.weapon_charge_lock_until > self.common.now_ms {
                next = next.min(self.shared.weapon_charge_lock_until);
            }
            if gcd_remaining_ms > 0.0 {
                next = next.min(
                    self.common
                        .now_ms
                        .saturating_add((gcd_remaining_ms / gcd_rate).ceil().max(1.0) as u64),
                );
                next = next.min(self.next_cooldown_rate_boundary(next));
            }
            if let Some(event) = self
                .common
                .queue
                .peek()
                .filter(|event| event.0.at_ms > self.common.now_ms)
            {
                next = next.min(event.0.at_ms);
            }
            if next <= self.common.now_ms {
                next = self.common.now_ms.saturating_add(1);
            }
            let previous = self.common.now_ms;
            self.process_events_through(next);
            if self.common.execution.failed() {
                return;
            }
            gcd_remaining_ms =
                (gcd_remaining_ms - (self.common.now_ms - previous) as f64 * gcd_rate).max(0.0);
        }
    }

    pub(crate) fn automatic_target_count(&self, ability: &CompiledAbility) -> u32 {
        self.automatic_target_count_for(ability.kind, ability.max_targets)
    }

    pub(crate) fn automatic_target_count_for(&self, kind: DpsAbilityKind, max_targets: u32) -> u32 {
        if matches!(
            kind,
            DpsAbilityKind::Detonate
                | DpsAbilityKind::BurstingIce
                | DpsAbilityKind::FireFrogs
                | DpsAbilityKind::Wildfire
                | DpsAbilityKind::WeaponShadowMark
                | DpsAbilityKind::Pyromania
        ) {
            1
        } else {
            max_targets.min(self.common.target_count).max(1)
        }
    }

    pub(crate) fn random_target_set(&mut self, maximum_targets: u32) -> Vec<u32> {
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return Vec::new();
        }
        let mut targets = (0..self.common.target_count).collect::<Vec<_>>();
        self.common.rng.shuffle(&mut targets);
        targets.truncate(maximum_targets.min(self.common.target_count) as usize);
        targets
    }

    pub(crate) fn target_count_damage_scaler(&self, ability: &CompiledAbility) -> f64 {
        ability
            .parameters
            .get(parameter_key!("targetCountDamageScalingThreshold"))
            .map(|threshold| {
                multi_target_damage_falloff(
                    ability.max_targets.min(self.common.target_count).max(1),
                    threshold,
                )
            })
            .unwrap_or(1.0)
    }

    pub(crate) fn pyromania_targets(&self, selected_target: u32, maximum_targets: u32) -> Vec<u32> {
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return Vec::new();
        }
        let mut missing = Vec::new();
        let mut active = Vec::new();
        for target in 0..self.common.target_count {
            if target == selected_target {
                continue;
            }
            let has_engulfing = self
                .dot_instances(target, DpsAbilityKind::EngulfingFlames)
                .any(|(_, dot)| dot.expires_ms >= self.common.now_ms);
            if has_engulfing {
                active.push(target);
            } else {
                missing.push(target);
            }
        }

        // The cooked policy sorts candidates by current Health before these
        // two passes. Stationary dummies have equal, immutable Health, so
        // target index is a deterministic DPS-equivalent tie break.
        let additional_targets = maximum_targets.saturating_sub(1) as usize;
        let mut targets = missing
            .into_iter()
            .chain(active)
            .take(additional_targets)
            .collect::<Vec<_>>();
        // AppendTargetDataHandle appends the explicitly selected actor after
        // the nearby spread targets.
        targets.push(selected_target);
        targets
    }

    pub(crate) fn process_events_through(&mut self, through_ms: u64) {
        self.refresh_periodic_healing_clock();
        self.refresh_kindling_clocks();
        self.refresh_tariq_swing_clock();
        self.refresh_elarion_swing_clock();
        self.refresh_gunde_swing_clock();
        while let Some(Reverse(event)) = self.common.queue.peek().cloned() {
            if event.at_ms > through_ms || event.at_ms > ENCOUNTER_DURATION_MS {
                break;
            }
            if !self.common.execution.charge(1) {
                break;
            }
            self.common.queue.pop();
            self.advance_to(event.at_ms);
            match event.kind {
                EventKind::Core(event) => match event {
                    CoreEvent::PreparedCastCommit { cast } => {
                        self.commit_prepared_cast(*cast);
                    }
                    CoreEvent::ImpactBatch {
                        index,
                        impacts,
                        mut context,
                    } => {
                        let snapshot = context.damage.source_snapshot.unwrap_or_else(|| {
                            self.capture_damage_source_snapshot(self.profile.abilities[index].kind)
                        });
                        context.damage = context.damage.with_snapshot(snapshot);
                        if matches!(
                            self.profile.abilities[index].kind,
                            DpsAbilityKind::GlacialBlast | DpsAbilityKind::IceComet
                        ) {
                            context.frostweaver = self.buff_stacks(AplBuff::FrostweaversWrath) > 0;
                        }
                        let targets = impacts.len() as u32;
                        for impact in impacts {
                            if !self.common.execution.charge(1) {
                                break;
                            }
                            if self.profile.abilities[index].kind == DpsAbilityKind::ColdSnap
                                && impact.target_index == 1
                            {
                                // The Frostwyrm cleave spec is constructed after
                                // the primary hit has dispatched its callbacks.
                                context.damage.source_snapshot = Some(
                                    self.capture_damage_source_snapshot(DpsAbilityKind::ColdSnap),
                                );
                            }
                            self.resolve_impact(
                                index,
                                impact.single_hit,
                                f64::from_bits(impact.damage_scale_bits),
                                impact.target_index,
                                context,
                            );
                            context.glacial_assault = false;
                            context.damage = context.damage.without_cast_proc();
                        }
                        let ability = &self.profile.abilities[index];
                        if ability.kind == DpsAbilityKind::WeaponInstantAoe
                            && !self.common.execution.failed()
                        {
                            // One buff application follows the entire damage
                            // batch, with stack count equal to acquired targets.
                            if self.common.now_ms >= self.shared.weapon_critical_buff_until {
                                self.shared.weapon_critical_buff_stacks = 0;
                            }
                            self.shared.weapon_critical_buff_stacks = self
                                .shared
                                .weapon_critical_buff_stacks
                                .saturating_add(targets)
                                .min(ability_u32_rounded(
                                    ability,
                                    parameter_key!("maximumCriticalStrikeStacks"),
                                ));
                            self.shared.weapon_critical_buff_until =
                                self.common.now_ms.saturating_add(ability_seconds_parameter(
                                    ability,
                                    parameter_key!("criticalStrikeBuffDurationSeconds"),
                                ));
                        }
                    }
                    CoreEvent::DotTick {
                        kind,
                        generation,
                        target_index,
                    } => {
                        self.dot_tick(kind, generation, target_index);
                    }
                    CoreEvent::StarfallHit {
                        generation,
                        target_index,
                    } => {
                        self.starfall_hit(generation, target_index);
                    }
                    CoreEvent::DotExpire {
                        kind,
                        generation,
                        target_index,
                    } => {
                        self.dot_expire(kind, generation, target_index);
                    }
                    CoreEvent::ApplyLinkedDot {
                        index,
                        targets,
                        context,
                    } => {
                        let ability = self.profile.abilities[index].clone();
                        let snapshot = context
                            .source_snapshot
                            .unwrap_or_else(|| self.capture_damage_source_snapshot(ability.kind));
                        for target_index in targets {
                            if !self.common.execution.charge(1) {
                                break;
                            }
                            self.apply_linked_dot(
                                &ability,
                                target_index,
                                context.with_snapshot(snapshot),
                            );
                        }
                    }
                    CoreEvent::ApplyAbilityDot {
                        index,
                        targets,
                        context,
                    } => {
                        let ability = self.profile.abilities[index].clone();
                        if let Some(dot) = ability.dot {
                            let snapshot = context.source_snapshot.unwrap_or_else(|| {
                                self.capture_damage_source_snapshot(ability.kind)
                            });
                            for target_index in targets {
                                if !self.common.execution.charge(1) {
                                    break;
                                }
                                self.apply_dot(
                                    target_index,
                                    ability.kind,
                                    &ability,
                                    dot,
                                    context.with_snapshot(snapshot),
                                );
                            }
                        }
                    }
                },
                EventKind::Shared(event) => match event {
                    SharedEvent::PeriodicHealingWake { generation } => {
                        if self
                            .shared
                            .periodic_healing
                            .is_some_and(|clock| clock.generation == generation)
                        {
                            self.refresh_periodic_healing_clock();
                            self.refresh_kindling_clocks();
                            self.refresh_tariq_swing_clock();
                            self.refresh_elarion_swing_clock();
                            self.refresh_gunde_swing_clock();
                        }
                    }
                    SharedEvent::WeaponCooldownReduction {
                        mechanic_index,
                        generation,
                        ability_kind,
                    } => {
                        if self.shared.dynamic_event_generations[mechanic_index] == generation {
                            let reduction_ms = std::mem::take(
                                &mut self.shared.dynamic_pending_cooldown_reductions_ms
                                    [mechanic_index],
                            );
                            self.reduce_cooldown(ability_kind, reduction_ms);
                        }
                    }
                    SharedEvent::RubyStormHit {
                        mechanic_index,
                        damage_bits,
                        snapshot,
                    } => self.hit_ruby_storm(mechanic_index, f64::from_bits(damage_bits), snapshot),
                    SharedEvent::WeaponConeStun { index, targets } => {
                        if !self.charge_work_units(u64::from(targets)) {
                            return;
                        }
                        let ability = &self.profile.abilities[index];
                        let kind = ability.kind;
                        let source = self.ability_damage_source(ability);
                        let duration = ability_seconds_parameter(
                            ability,
                            parameter_key!("coneStunDurationSeconds"),
                        );
                        for target in 0..targets {
                            let refreshed =
                                self.shared.cone_stun_until[target as usize] > self.common.now_ms;
                            self.shared.cone_stun_until[target as usize] =
                                self.common.now_ms.saturating_add(duration);
                            self.notify_kindling_of_harmful_effect(
                                Some(kind),
                                source,
                                target,
                                refreshed,
                            );
                        }
                    }
                    SharedEvent::WeaponShadowExplode {
                        generation,
                        target_index,
                    } => {
                        if self
                            .shared
                            .shadow_marks
                            .get(&target_index)
                            .copied()
                            .filter(|mark| mark.generation == generation)
                            .is_some()
                        {
                            self.explode_shadow_mark(target_index);
                        }
                    }
                    SharedEvent::AurastoneDamagePulse {
                        mechanic_index,
                        generation,
                    } => self.pulse_aurastone_damage(mechanic_index, generation),
                    SharedEvent::AmethystSplintersTick {
                        mechanic_index,
                        generation,
                        target_index,
                    } => self.tick_amethyst_splinters(mechanic_index, generation, target_index),
                    SharedEvent::KindlingHealingTick {
                        mechanic_index,
                        generation,
                    } => {
                        self.tick_kindling_healing(mechanic_index, generation);
                    }
                    SharedEvent::KindlingTick {
                        mechanic_index,
                        generation,
                        target_index,
                    } => self.tick_kindling(mechanic_index, generation, target_index),
                    SharedEvent::WayfarerProc { generation } => {
                        if self.shared.wayfarer_generation == generation
                            && self.shared.wayfarer_next_ms == self.common.now_ms
                        {
                            self.proc_wayfarer();
                        }
                    }
                },
                EventKind::Ardeos(event) => self.handle_ardeos_event(event),
                EventKind::Rime(event) => self.handle_rime_event(event),
                EventKind::Tariq(event) => self.handle_tariq_event(event),
                EventKind::Elarion(event) => self.handle_elarion_event(event),
                EventKind::Mara(event) => self.handle_mara_event(event),
                EventKind::Gunde(event) => self.handle_gunde_event(event),
            }
            if self.common.execution.failed() {
                return;
            }
            self.refresh_periodic_healing_clock();
            self.refresh_kindling_clocks();
            self.refresh_tariq_swing_clock();
            self.refresh_elarion_swing_clock();
            self.refresh_gunde_swing_clock();
        }
        if self.common.execution.failed() {
            return;
        }
        self.advance_to(through_ms.min(ENCOUNTER_DURATION_MS));
    }

    pub(crate) fn advance_to(&mut self, time_ms: u64) {
        if time_ms <= self.common.now_ms {
            return;
        }
        while self.common.now_ms < time_ms {
            if !self.common.execution.charge(1) {
                return;
            }
            let boundary = self.next_cooldown_rate_boundary(time_ms);
            let elapsed_ms = boundary - self.common.now_ms;
            // Batch only whole passive pulses between events/rate boundaries. A
            // pulse exactly at a boundary reads the attributes at that time.
            let mut elarion_boundary_pulse = false;
            if self.profile.contract.hero == HeroIdentity::Elarion {
                let (period, gain) = self.elarion_focus_pulse();
                if period > 0 && gain > 0.0 {
                    let pulses = (boundary - 1) / period - self.common.now_ms / period;
                    self.gain_elarion_passive_focus(pulses as f64 * gain);
                    elarion_boundary_pulse = boundary.is_multiple_of(period);
                }
            }
            if self.profile.contract.hero == HeroIdentity::Mara {
                let base_regeneration = self
                    .ability(DpsAbilityKind::MaraAttack)
                    .map(|ability| {
                        ability_param(ability, parameter_key!("energyRegenerationPerSecond"))
                    })
                    .unwrap_or(0.0);
                let haste_scaler = self
                    .ability(DpsAbilityKind::MaraAttack)
                    .map(|ability| {
                        ability_param(ability, parameter_key!("energyRegenerationHasteScaler"))
                    })
                    .unwrap_or(0.0);
                let maiden_multiplier =
                    if self.common.now_ms < self.hero.mara().maiden_of_death_until {
                        self.ability(DpsAbilityKind::MaidenOfDeath)
                            .map(|ability| {
                                ability_param(ability, parameter_key!("energyGenerationMultiplier"))
                            })
                            .unwrap_or(1.0)
                    } else {
                        1.0
                    };
                let seething_multiplier = if self.common.now_ms
                    < self.hero.mara().seething_poison_max_until
                {
                    self.ability(DpsAbilityKind::SeethingPoison)
                        .map(|ability| {
                            ability_param(ability, parameter_key!("energyRegenerationMultiplier"))
                        })
                        .unwrap_or(1.0)
                } else {
                    1.0
                };
                let regeneration = base_regeneration
                    * (1.0 + self.effective_haste() * haste_scaler).max(0.05)
                    * maiden_multiplier
                    * seething_multiplier
                    * elapsed_ms as f64
                    / 1_000.0;
                self.hero.mara_mut().energy =
                    (self.hero.mara().energy + regeneration).min(self.profile.max_primary_resource);
            }
            let cooldown_count = self.common.cooldowns.len();
            if !self
                .common
                .execution
                .charge_lightweight_loop(cooldown_count)
            {
                return;
            }
            let execution = &self.common.execution;
            let mut cooldowns = std::mem::take(&mut self.common.cooldowns);
            let mut ready = Vec::new();
            for (kind, cooldown) in &mut cooldowns {
                let rate = self.cooldown_recovery_rate(*kind);
                let base_ms = self
                    .ability(*kind)
                    .map(|ability| ability.cooldown_ms as f64)
                    .unwrap_or(0.0);
                advance_cooldown_state(cooldown, elapsed_ms as f64 * rate, base_ms, execution);
                if cooldown.used_charges == 0 {
                    ready.push(*kind);
                }
            }
            if self.common.execution.failed() || !self.common.execution.charge_usize(ready.len()) {
                self.common.cooldowns = cooldowns;
                return;
            }
            for kind in ready {
                cooldowns.remove(&kind);
            }
            self.common.cooldowns = cooldowns;
            self.common.now_ms = boundary;
            if elarion_boundary_pulse {
                let (_, gain) = self.elarion_focus_pulse();
                self.gain_elarion_passive_focus(gain);
            }
        }
    }

    pub(crate) fn next_cooldown_rate_boundary(&self, limit: u64) -> u64 {
        let mut boundary = limit;
        let mut consider = |candidate: u64| {
            if candidate > self.common.now_ms {
                boundary = boundary.min(candidate);
            }
        };
        consider(self.shared.heroism_until);
        consider(self.shared.weapon_channel_cooldown_recovery_until);
        consider(self.shared.weapon_charge_buff_until);
        if let HeroState::Rime(state) = &self.hero {
            consider(state.winters_blessing_until);
        }
        if let HeroState::Tariq(state) = &self.hero {
            consider(state.thunder_call_until);
            consider(state.far_beyond_driven_until);
        }
        if let HeroState::Gunde(state) = &self.hero {
            consider(state.massacre_until);
            consider(state.bloodbath_until);
        }
        if let HeroState::Elarion(state) = &self.hero {
            consider(state.event_horizon_until);
            consider(state.skystriders_grace_until);
        }
        if let HeroState::Mara(state) = &self.hero {
            consider(state.maiden_of_death_until);
            consider(state.seething_poison_max_until);
        }
        if !self
            .common
            .execution
            .charge_lightweight_loop(self.profile.mechanic_indexes.cooldown_rate_boundaries.len())
        {
            return boundary;
        }
        for index in &self.profile.mechanic_indexes.cooldown_rate_boundaries {
            if let Some(buff) = &self.shared.dynamic_buffs[*index] {
                consider(buff.until_ms);
            }
        }
        boundary
    }
}
