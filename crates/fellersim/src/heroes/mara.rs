use crate::Iteration;
use crate::*;

#[derive(Debug, Clone)]
pub(crate) enum MaraEvent {
    StealthPoison {
        ability_kind: DpsAbilityKind,
        context: DamageContext,
    },
    VolatileEruption {
        target_index: u32,
        context: DamageContext,
    },
    AbilityEnded {
        kind: DpsAbilityKind,
    },
    ResourceRefund {
        energy_bits: u64,
        combo_points: u32,
    },
    FromShadowsStrike {
        target_index: u32,
        context: DamageContext,
    },
    MatriarchStrike {
        ability_kind: DpsAbilityKind,
        target_index: u32,
        bonus_crit: f64,
        context: DamageContext,
    },
}

#[derive(Debug)]
pub(crate) struct MaraState {
    pub(crate) energy: f64,
    pub(crate) combo_points: u32,
    pub(crate) stealth_active: bool,
    pub(crate) maiden_of_death_until: u64,
    pub(crate) matriarch_macabre_until: u64,
    pub(crate) assassins_guile_until: u64,
    pub(crate) deadly_scheme_stacks: u32,
    pub(crate) deadly_scheme_energy_remainder: f64,
    pub(crate) deadly_scheme_overflow: u32,
    pub(crate) deadly_scheme_until: u64,
    pub(crate) feed_the_queen_stacks: u32,
    pub(crate) feed_the_queen_until: u64,
    pub(crate) malevolence_arachnid_stacks: u32,
    pub(crate) malevolence_arachnid_until: u64,
    pub(crate) malevolence_queen_stacks: u32,
    pub(crate) malevolence_queen_until: u64,
    pub(crate) drenched_in_blood_until: u64,
    pub(crate) seething_poison_until: Vec<u64>,
    pub(crate) seething_poison_max_until: u64,
}

impl Iteration<'_> {
    pub(crate) fn handle_mara_event(&mut self, event: MaraEvent) {
        match event {
            MaraEvent::StealthPoison {
                ability_kind,
                context,
            } => {
                self.apply_mara_stealth_poison(ability_kind, context);
            }
            MaraEvent::VolatileEruption {
                target_index,
                context,
            } => {
                if let Some(eruption) = self
                    .ability(DpsAbilityKind::VolatilePoisonEruption)
                    .cloned()
                {
                    // The graph checks a cylinder but applies its spec to the
                    // granting effect's owner only. Its falloff row is unused.
                    self.damage_hit(
                        &eruption,
                        eruption.power_coefficient
                            * self.profile.power
                            * self.mara_creeping_death_multiplier(),
                        0.0,
                        true,
                        target_index,
                        context.as_proc(),
                    );
                }
            }
            MaraEvent::AbilityEnded { kind } => self.finish_mara_ability(kind),
            MaraEvent::MatriarchStrike {
                ability_kind,
                target_index,
                bonus_crit,
                context,
            } => {
                let Some(ability) = self.ability(ability_kind).cloned() else {
                    return;
                };
                let Some(matriarch) = self.ability(DpsAbilityKind::MatriarchMacabre).cloned()
                else {
                    return;
                };
                // Clone tags retain ordinary damage listeners, but bypass
                // resource/commit processing and Vexira's explicit non-copy filter.
                self.damage_raw_with_spread_key(
                    Some(ability_kind),
                    self.ability_damage_source(&matriarch),
                    ability.power_coefficient
                        * self.profile.power
                        * self.target_count_damage_scaler(&ability),
                    context.source_snapshot.map(|s| s.expertise),
                    context.source_snapshot.map(|s| s.primary_stat_multiplier),
                    context.source_snapshot.map(|s| s.critical_chance),
                    ability.damage_spread,
                    bonus_crit,
                    true,
                    true,
                    target_index,
                    context,
                );
            }
            MaraEvent::FromShadowsStrike {
                target_index,
                context,
            } => {
                let Some(index) = self.profile.mechanic_indexes.mara_arachnid_clone else {
                    return;
                };
                let Some(queen) = self.ability(DpsAbilityKind::QueensFang).cloned() else {
                    return;
                };
                // OtherSource copy damage bypasses the spender commit and its
                // resource/refund/poison-transfer listeners.
                self.damage_raw_with_spread_key(
                    Some(queen.kind),
                    self.profile.mechanics[index].damage_source,
                    queen.power_coefficient * self.profile.power,
                    context.source_snapshot.map(|snapshot| snapshot.expertise),
                    context
                        .source_snapshot
                        .map(|snapshot| snapshot.primary_stat_multiplier),
                    context
                        .source_snapshot
                        .map(|snapshot| snapshot.critical_chance),
                    queen.damage_spread,
                    0.0,
                    true,
                    true,
                    target_index,
                    context,
                );
            }
            MaraEvent::ResourceRefund {
                energy_bits,
                combo_points,
            } => {
                let energy = f64::from_bits(energy_bits);
                let maximum_energy = self.profile.max_primary_resource;
                let maximum_combo_points = self.profile.max_secondary_resource;
                let state = self.hero.mara_mut();
                state.energy = (state.energy + energy).min(maximum_energy);
                state.combo_points = state
                    .combo_points
                    .saturating_add(combo_points)
                    .min(maximum_combo_points);
            }
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn commit_mara_ability(&mut self, ability: &CompiledAbility) {
        if self.profile.contract.hero != HeroIdentity::Mara {
            return;
        }
        // The passive listens to ability commitment, not damage. One Widow
        // cast rolls once; an auto-attack miss still keeps its poison application.
        if matches!(
            ability.kind,
            DpsAbilityKind::MaraAttack | DpsAbilityKind::WidowsBite | DpsAbilityKind::Backstab
        ) && let Some(chance) = self
            .common
            .selected_talents
            .get("mara-talent-id-talent14")
            .map(|talent| param(talent, parameter_key!("procChance")))
            && self.roll_controlled_random_bool(
                "RandomStream.Mara.Talent.BleedAttackSpender.ExplosivePoison",
                chance,
            )
        {
            self.record_talent_proc("mara-talent-id-talent14");
            self.apply_mara_dot(0, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
        }
        match ability.kind {
            DpsAbilityKind::BroodingShadows => {
                // Casting again removes the effect directly: no damage-cancel
                // event, Guile proc, or manual cooldown commitment.
                if self.hero.mara().stealth_active {
                    self.hero.mara_mut().stealth_active = false;
                    self.deactivate_fixed_buff(AplBuff::BroodingShadows);
                } else {
                    self.hero.mara_mut().stealth_active = true;
                    self.activate_fixed_buff(AplBuff::BroodingShadows, ENCOUNTER_DURATION_MS);
                }
            }
            DpsAbilityKind::MaidenOfDeath => {
                self.hero.mara_mut().maiden_of_death_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::MaidenOfDeath,
                    self.hero.mara().maiden_of_death_until,
                );
            }
            DpsAbilityKind::MatriarchMacabre => {
                self.hero.mara_mut().matriarch_macabre_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::MatriarchMacabre,
                    self.hero.mara().matriarch_macabre_until,
                );
            }
            DpsAbilityKind::FinalStratagem => {
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent8") {
                    let active = self.common.now_ms < self.hero.mara().matriarch_macabre_until;
                    let duration = ms_param(
                        talent,
                        if active {
                            parameter_key!("additionalDurationSeconds")
                        } else {
                            parameter_key!("durationSeconds")
                        },
                    );
                    let base = self
                        .hero
                        .mara()
                        .matriarch_macabre_until
                        .max(self.common.now_ms);
                    self.hero.mara_mut().matriarch_macabre_until = base.saturating_add(duration);
                    self.activate_fixed_buff(
                        AplBuff::MatriarchMacabre,
                        self.hero.mara().matriarch_macabre_until,
                    );
                } else {
                    self.hero.mara_mut().energy = self.profile.max_primary_resource;
                    self.hero.mara_mut().combo_points = self.profile.max_secondary_resource;
                    for kind in [
                        DpsAbilityKind::HemorrhagingStrike,
                        DpsAbilityKind::WidowsBite,
                        DpsAbilityKind::MaidenOfDeath,
                        DpsAbilityKind::BroodingShadows,
                    ] {
                        self.reset_cooldown(kind);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn gain_mara_energy(&mut self, amount: f64) {
        let multiplier = if self.common.now_ms < self.hero.mara().maiden_of_death_until {
            self.ability(DpsAbilityKind::MaidenOfDeath)
                .map(|ability| ability_param(ability, parameter_key!("energyGenerationMultiplier")))
                .unwrap_or(1.0)
        } else {
            1.0
        };
        self.hero.mara_mut().energy =
            (self.hero.mara().energy + amount * multiplier).min(self.profile.max_primary_resource);
    }

    pub(crate) fn accumulate_mara_deadly_scheme(&mut self, energy: f64) {
        let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent3") else {
            return;
        };
        let per_stack = param(talent, parameter_key!("energyPerStack")).max(1.0);
        let maximum = param_u32(talent, parameter_key!("maximumStacks"));
        // Kismet rounds NewValue - OldValue before taking its absolute value.
        let rounded_spend = (-energy + 0.5).floor().abs();
        let state = self.hero.mara_mut();
        state.deadly_scheme_energy_remainder += rounded_spend;
        if state.deadly_scheme_energy_remainder < per_stack {
            return;
        }
        let added = (state.deadly_scheme_energy_remainder / per_stack).floor() as u32;
        state.deadly_scheme_stacks = state
            .deadly_scheme_stacks
            .saturating_add(added)
            .saturating_add(state.deadly_scheme_overflow);
        state.deadly_scheme_overflow = if state.deadly_scheme_stacks > maximum {
            state
                .deadly_scheme_overflow
                .saturating_add(state.deadly_scheme_stacks - maximum)
        } else {
            0
        };
        state.deadly_scheme_energy_remainder -= f64::from(added) * per_stack;
    }

    pub(crate) fn finish_mara_ability(&mut self, kind: DpsAbilityKind) {
        let is_damage_spender = matches!(
            kind,
            DpsAbilityKind::QueensFang | DpsAbilityKind::ArachnidAssault
        );
        if is_damage_spender {
            self.hero.mara_mut().deadly_scheme_until = 0;
            self.deactivate_fixed_buff(AplBuff::DeadlyScheme);
        }
        // The passive ignores auto-attack endings. Other offensive ability
        // endings promote accumulated stacks only AFTER their specs were made.
        if kind != DpsAbilityKind::MaraAttack
            && is_offensive_skill_commit(kind)
            && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent3")
            && self.hero.mara().deadly_scheme_stacks
                >= param_u32(talent, parameter_key!("maximumStacks"))
        {
            let until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            // The native setter reapplies the stacking GE, whose cap is MaxStack.
            // Its visible count contributes no further overflow at promotion.
            self.hero.mara_mut().deadly_scheme_stacks = 0;
            self.hero.mara_mut().deadly_scheme_until = until;
            self.activate_fixed_buff(AplBuff::DeadlyScheme, until);
            self.record_talent_proc("mara-talent-id-talent3");
        }
        if is_damage_spender {
            let state = self.hero.mara_mut();
            if kind == DpsAbilityKind::QueensFang {
                state.malevolence_queen_stacks = state.malevolence_queen_stacks.saturating_sub(1);
                if state.malevolence_queen_stacks == 0 {
                    state.malevolence_queen_until = 0;
                    self.deactivate_fixed_buff(AplBuff::MalevolenceQueen);
                }
            } else {
                state.malevolence_arachnid_stacks =
                    state.malevolence_arachnid_stacks.saturating_sub(1);
                if state.malevolence_arachnid_stacks == 0 {
                    state.malevolence_arachnid_until = 0;
                    self.deactivate_fixed_buff(AplBuff::MalevolenceArachnid);
                }
            }
        }
        match kind {
            DpsAbilityKind::QueensFang => {
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent2") {
                    let maximum = param_u32(talent, parameter_key!("maximumStacks"));
                    let duration = ms_param(talent, parameter_key!("durationSeconds"));
                    if self.common.now_ms >= self.hero.mara().malevolence_arachnid_until {
                        self.hero.mara_mut().malevolence_arachnid_stacks = 0;
                    }
                    self.hero.mara_mut().malevolence_arachnid_stacks = self
                        .hero
                        .mara()
                        .malevolence_arachnid_stacks
                        .saturating_add(1)
                        .min(maximum);
                    self.hero.mara_mut().malevolence_arachnid_until =
                        self.common.now_ms.saturating_add(duration);
                    self.activate_fixed_buff(
                        AplBuff::MalevolenceArachnid,
                        self.hero.mara().malevolence_arachnid_until,
                    );
                }
            }
            DpsAbilityKind::ArachnidAssault => {
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent2") {
                    let maximum = param_u32(talent, parameter_key!("maximumStacks"));
                    let duration = ms_param(talent, parameter_key!("durationSeconds"));
                    if self.common.now_ms >= self.hero.mara().malevolence_queen_until {
                        self.hero.mara_mut().malevolence_queen_stacks = 0;
                    }
                    self.hero.mara_mut().malevolence_queen_stacks = self
                        .hero
                        .mara()
                        .malevolence_queen_stacks
                        .saturating_add(1)
                        .min(maximum);
                    self.hero.mara_mut().malevolence_queen_until =
                        self.common.now_ms.saturating_add(duration);
                    self.activate_fixed_buff(
                        AplBuff::MalevolenceQueen,
                        self.hero.mara().malevolence_queen_until,
                    );
                }
            }
            _ => {}
        }
    }

    pub(crate) fn mara_arachnid_target_multiplier(&self, target_index: u32) -> f64 {
        if self.dot_remaining_on_target(target_index, DpsAbilityKind::HemorrhagingStrike) > 0 {
            self.common
                .selected_talents
                .get("mara-talent-id-talent18")
                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0)
        } else {
            1.0
        }
    }

    pub(crate) fn try_mara_spirit_refund(&mut self, energy_spent: f64, combo_points_spent: u32) {
        let chance = spirit_refund_chance(self.effective_spirit(), 1.0, 0.0);
        if !self.roll_controlled_random_bool(SPIRIT_PROC_RANDOM_STREAM_TAG, chance) {
            return;
        }
        self.record_spirit_refund_proc();
        self.shared.spirit = (self.shared.spirit + 1.0).min(self.profile.max_spirit);
        if let Some(index) = self.profile.mechanic_indexes.mara_drenched_in_blood {
            let duration = seconds_parameter(
                &self.profile.mechanics[index],
                parameter_key!("spiritRefundExpertiseDurationSeconds"),
            );
            self.hero.mara_mut().drenched_in_blood_until =
                self.common.now_ms.saturating_add(duration);
            self.activate_fixed_buff(
                AplBuff::DrenchedInBlood,
                self.hero.mara().drenched_in_blood_until,
            );
        }
        let delay = self
            .ability(DpsAbilityKind::MaraAttack)
            .map(|ability| {
                ability_seconds_parameter(ability, parameter_key!("spiritRefundDelaySeconds"))
            })
            .unwrap_or(200);
        self.push_event(
            self.common.now_ms.saturating_add(delay),
            MaraEvent::ResourceRefund {
                energy_bits: energy_spent.to_bits(),
                combo_points: combo_points_spent,
            },
        );
    }

    pub(crate) fn try_mara_corrosive_spill(
        &mut self,
        combo_points_spent: u32,
        context: DamageContext,
    ) {
        let Some(chance) = self
            .common
            .selected_talents
            .get("mara-talent-id-talent4")
            .map(|talent| param(talent, parameter_key!("procChancePerComboPoint")))
        else {
            return;
        };
        if !self.charge_work_units(u64::from(combo_points_spent)) {
            return;
        }
        let triggered = (0..combo_points_spent).any(|_| {
            self.roll_controlled_random_bool(
                "RandomStream.Mosse.Talents.ComboPointsChanceAoeDamage",
                chance,
            )
        });
        if !triggered {
            return;
        }
        self.record_talent_proc("mara-talent-id-talent4");
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::CorrosiveSpill).cloned() else {
            return;
        };
        let Some(model) = ability.dot else {
            return;
        };
        for target_index in 0..self.common.target_count {
            let kind = DotKind::Ability(ability.kind);
            let expires_ms = self.common.now_ms.saturating_add(model.duration_ms);
            if let Some(active) = self
                .common
                .dots
                .get_mut(&(target_index, kind))
                .filter(|dot| dot.expires_ms > self.common.now_ms)
            {
                // CRAoeActorBase's LimitToSingleApplication reference-counts
                // one effect. Equal-duration stationary areas form one interval;
                // a new overlap neither reapplies the effect nor resets its clock.
                active.expires_ms = active.expires_ms.max(expires_ms);
                let generation = active.generation;
                self.push_event(
                    expires_ms,
                    CoreEvent::DotExpire {
                        kind,
                        generation,
                        target_index,
                    },
                );
            } else {
                self.apply_dot(
                    target_index,
                    ability.kind,
                    &ability,
                    model,
                    context.as_proc(),
                );
                // This GE inherits execution-on-application=true. The Infinite
                // effect's unused duration row does not govern the actor lifetime.
                if let Some(dot) = self.common.dots.get(&(target_index, kind)).cloned() {
                    self.execute_dot_tick(kind, &dot, target_index, 1.0);
                }
            }
        }
    }

    pub(crate) fn apply_mara_stealth_poison(
        &mut self,
        kind: DpsAbilityKind,
        context: DamageContext,
    ) {
        match kind {
            DpsAbilityKind::Backstab => {
                let Some(poison) = self.ability(DpsAbilityKind::CausticPoison).cloned() else {
                    return;
                };
                let bonus = self
                    .ability(kind)
                    .map(|a| ability_param(a, parameter_key!("causticCriticalStrikeBonus")))
                    .unwrap_or(0.0);
                self.damage_hit(
                    &poison,
                    poison.power_coefficient
                        * self.profile.power
                        * self.mara_creeping_death_multiplier(),
                    bonus,
                    true,
                    0,
                    context.as_proc(),
                );
            }
            DpsAbilityKind::WidowsBite => {
                // CrRemoveAllActiveEffectsOfTypeWithSource runs before apply,
                // including when reapplying to the same target.
                for target in 0..self.common.target_count {
                    let key = (target, DotKind::Ability(DpsAbilityKind::SeethingPoison));
                    if let Some(old) = self.common.dots.remove(&key) {
                        self.record_dot_uptime(target, key.1, &old);
                    }
                }
                self.hero.mara_mut().seething_poison_until.fill(0);
                self.hero.mara_mut().seething_poison_max_until = 0;
                self.apply_mara_dot(0, DpsAbilityKind::SeethingPoison, context, 1.0);
                if let Some(dot) = self
                    .common
                    .dots
                    .get(&(0, DotKind::Ability(DpsAbilityKind::SeethingPoison)))
                {
                    let expiry = dot.expires_ms;
                    self.hero.mara_mut().seething_poison_until[0] = expiry;
                    self.hero.mara_mut().seething_poison_max_until = expiry;
                }
            }
            DpsAbilityKind::SkitteringBlades => {
                let Some(poison) = self.ability(DpsAbilityKind::VolatilePoison).cloned() else {
                    return;
                };
                let Some(mut model) = poison.dot else {
                    return;
                };
                // One outgoing spec is shared by the whole target batch.
                let minimum = ability_param(&poison, parameter_key!("minimumDurationSeconds"));
                let maximum = ability_param(&poison, parameter_key!("maximumDurationSeconds"));
                model.duration_ms = (self.common.rng.uniform_f64(minimum, maximum) * 1_000.0)
                    .round()
                    .max(1.0) as u64;
                model.period_ms = (model.duration_ms as f64
                    / ability_param(&poison, parameter_key!("maximumTicks")).max(1.0))
                .round()
                .max(1.0) as u64;
                for target in 0..self
                    .common
                    .target_count
                    .min(self.ability(kind).map(|a| a.max_targets).unwrap_or(1))
                {
                    self.apply_dot(target, poison.kind, &poison, model, context.as_proc());
                    self.update_mara_dot_period(
                        target,
                        DotKind::Ability(poison.kind),
                        model.period_ms,
                    );
                }
            }
            _ => {}
        }
    }

    fn update_mara_dot_period(&mut self, target: u32, kind: DotKind, base_period: u64) {
        let period = self.dot_period(kind, base_period);
        if let Some(active) = self.common.dots.get_mut(&(target, kind)) {
            let remaining = active.next_tick_ms.saturating_sub(self.common.now_ms) as f64
                / active.scheduled_period_ms.max(1) as f64;
            active.model.period_ms = base_period;
            active.scheduled_period_ms = period;
            active.next_tick_ms = self
                .common
                .now_ms
                .saturating_add((remaining * period as f64).round().max(1.0) as u64);
            active.generation = active.generation.wrapping_add(1);
            let generation = active.generation;
            let next_tick_ms = active.next_tick_ms;
            let expires_ms = active.expires_ms;
            self.push_event(
                next_tick_ms,
                CoreEvent::DotTick {
                    kind,
                    generation,
                    target_index: target,
                },
            );
            self.push_event(
                expires_ms,
                CoreEvent::DotExpire {
                    kind,
                    generation,
                    target_index: target,
                },
            );
        }
    }

    pub(crate) fn resolve_mara_impact(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        outcome: DamageOutcome,
        total_direct_damage: f64,
        impact_context: CastImpactContext,
        context: DamageContext,
    ) {
        if self.profile.contract.hero != HeroIdentity::Mara {
            return;
        }
        if ability.kind == DpsAbilityKind::Backstab {
            let target_is_bleeding =
                self.dot_remaining_on_target(target_index, DpsAbilityKind::HemorrhagingStrike) > 0;
            let caustic_random = target_is_bleeding
                && self
                    .common
                    .selected_talents
                    .get("mara-talent-id-talent11")
                    .map(|talent| param(talent, parameter_key!("procChance")))
                    .is_some_and(|chance| {
                        self.roll_controlled_random_bool(
                            "RandomStream.Mara.Talent.StrongerBehindAttackBuilder.PoisonChance",
                            chance,
                        )
                    });
            if caustic_random {
                self.record_talent_proc("mara-talent-id-talent11");
            }
            // Caustic Wounds creates a separate spec from the stealth manager.
            let poison_hits = u32::from(caustic_random);
            for _ in 0..poison_hits {
                let Some(poison) = self.ability(DpsAbilityKind::CausticPoison).cloned() else {
                    break;
                };
                let mut poison_context = context.as_proc();
                poison_context.multiply_damage(self.mara_creeping_death_multiplier());
                self.damage_hit(
                    &poison,
                    poison.power_coefficient * self.profile.power,
                    ability_param(ability, parameter_key!("causticCriticalStrikeBonus")),
                    true,
                    target_index,
                    poison_context,
                );
            }
        }

        let maiden_active = self.common.now_ms < self.hero.mara().maiden_of_death_until;
        if ability.kind == DpsAbilityKind::HemorrhagingStrike {
            let spent = impact_context.mara_combo_points_spent.max(1);
            let mut targets = Vec::new();
            let mut replacing = false;
            if maiden_active
                && self
                    .common
                    .selected_talents
                    .contains_key("mara-talent-id-talent16")
            {
                replacing = true;
                targets.extend(
                    (0..self.common.target_count)
                        .filter(|candidate| *candidate != target_index)
                        .take(
                            self.common
                                .selected_talents
                                .get("mara-talent-id-talent16")
                                .map(|talent| {
                                    param_u32(talent, parameter_key!("additionalTargets"))
                                })
                                .unwrap_or(0) as usize,
                        ),
                );
            }
            targets.push(target_index);
            for (target_position, bleed_target) in targets.into_iter().enumerate() {
                let Some(bleed_ability) = self.ability(DpsAbilityKind::HemorrhagingStrike).cloned()
                else {
                    continue;
                };
                let Some(mut dot) = bleed_ability.dot else {
                    continue;
                };
                dot.duration_ms = dot
                    .duration_ms
                    .saturating_add(u64::from(spent).saturating_mul(ability_seconds_parameter(
                        ability,
                        parameter_key!("bleedDurationPerComboPointSeconds"),
                    )));
                if let Some(index) = self.profile.mechanic_indexes.mara_arachnid_clone {
                    dot.period_ms = (dot.period_ms as f64
                        * mechanic_param(
                            &self.profile.mechanics[index],
                            parameter_key!("fromShadowsBleedPeriodMultiplier"),
                        ))
                    .round()
                    .max(1.0) as u64;
                }
                // The original application reaches duration/application listeners
                // before Gushing removes it and applies the tagged copies.
                if replacing && target_position == 0 {
                    self.apply_dot(target_index, ability.kind, &bleed_ability, dot, context);
                    let key = (target_index, DotKind::Ability(ability.kind));
                    if let Some(original) = self.common.dots.remove(&key) {
                        self.record_dot_uptime(key.0, key.1, &original);
                    }
                }
                let bleed_context = context;
                if replacing {
                    let key = (
                        bleed_target,
                        DotKind::Ability(DpsAbilityKind::HemorrhagingStrike),
                    );
                    if let Some(previous) = self.common.dots.remove(&key) {
                        self.record_dot_uptime(key.0, key.1, &previous);
                    }
                }
                self.apply_dot(
                    bleed_target,
                    DpsAbilityKind::HemorrhagingStrike,
                    &bleed_ability,
                    dot,
                    bleed_context,
                );
                self.trigger_mara_hemotoxin(bleed_target, context);
                // Both the base GA and Gushing set Bloodrush's custom period
                // after ApplyGameplayEffectSpecToTarget returns. Hemotoxin's
                // synchronous application listener sees the pre-adjustment timer.
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent19") {
                    let tick_rate = param(talent, parameter_key!("tickRateMultiplier")).max(0.05);
                    let kind = DotKind::Ability(ability.kind);
                    let base_period = (dot.period_ms as f64 / tick_rate).round().max(1.0) as u64;
                    self.update_mara_dot_period(bleed_target, kind, base_period);
                }
            }
        }

        if outcome.damage <= 0.0 {
            return;
        }
        if ability.kind == DpsAbilityKind::QueensFang {
            self.hero.mara_mut().feed_the_queen_stacks = 0;
            self.hero.mara_mut().feed_the_queen_until = 0;
            self.deactivate_fixed_buff(AplBuff::FeedTheQueen);
        }
        let combo_gain = |normal_key: ParameterKey, crit_key: ParameterKey| {
            if outcome.critical {
                ability_param(ability, crit_key)
            } else {
                ability_param(ability, normal_key)
            }
            .round()
            .max(0.0) as u32
        };
        let gained_combo_points = match ability.kind {
            DpsAbilityKind::SkitteringBlades => combo_gain(
                parameter_key!("comboPointsPerHit"),
                parameter_key!("comboPointsPerCriticalHit"),
            ),
            DpsAbilityKind::WidowsBite | DpsAbilityKind::Backstab => combo_gain(
                parameter_key!("comboPointsPerHit"),
                parameter_key!("comboPointsPerCriticalHit"),
            ),
            _ => 0,
        };
        if gained_combo_points > 0 {
            let gained_combo_points = if maiden_active {
                self.profile.max_secondary_resource
            } else if ability.kind == DpsAbilityKind::Backstab && impact_context.mara_from_stealth {
                (gained_combo_points as f64
                    * ability_param(ability, parameter_key!("poisonComboPointMultiplier")))
                .round() as u32
            } else {
                gained_combo_points
            };
            self.hero.mara_mut().combo_points = self
                .hero
                .mara()
                .combo_points
                .saturating_add(gained_combo_points)
                .min(self.profile.max_secondary_resource);
        }

        if ability.kind == DpsAbilityKind::SkitteringBlades
            && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent15")
        {
            let maximum = param_u32(talent, parameter_key!("maximumStacks"));
            if self.common.now_ms >= self.hero.mara().feed_the_queen_until {
                self.hero.mara_mut().feed_the_queen_stacks = 0;
            }
            self.hero.mara_mut().feed_the_queen_stacks = self
                .hero
                .mara()
                .feed_the_queen_stacks
                .saturating_add(1)
                .min(maximum);
            self.hero.mara_mut().feed_the_queen_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
            self.activate_fixed_buff(AplBuff::FeedTheQueen, self.hero.mara().feed_the_queen_until);
        }

        if matches!(
            ability.kind,
            DpsAbilityKind::ArachnidAssault | DpsAbilityKind::QueensFang
        ) && outcome.critical
            && let Some(index) = self.profile.mechanic_indexes.mara_arachnid_poison
        {
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            let dot = DotModel {
                power_coefficient: 0.0,
                damage_spread: 0.0,
                duration_ms: seconds_parameter(
                    &mechanic,
                    parameter_key!("arachnidPoisonDurationSeconds"),
                ),
                period_ms: seconds_parameter(
                    &mechanic,
                    parameter_key!("arachnidPoisonPeriodSeconds"),
                ),
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            };
            self.apply_damage_derived_dot(
                target_index,
                DotKind::MaraArachnidPoison,
                mechanic.damage_source,
                dot,
                total_direct_damage
                    * mechanic_param(&mechanic, parameter_key!("arachnidPoisonDamageFraction")),
                context,
            );
        }
    }

    pub(crate) fn schedule_mara_matriarch_copies(&mut self, cast: &PreparedCast) {
        let Some(matriarch) = self.ability(DpsAbilityKind::MatriarchMacabre).cloned() else {
            return;
        };
        let delay =
            ability_seconds_parameter(&matriarch, parameter_key!("copyActivationDelaySeconds"));
        let count = self.automatic_target_count_for(cast.ability.kind, cast.ability.max_targets);
        if !self.charge_work_product(2, u64::from(count)) {
            return;
        }
        let mut context = cast.context.without_cast_proc();
        context.multiply_damage(
            cast.mara_copy_damage_multiplier
                * ability_param(&matriarch, parameter_key!("cloneDamageMultiplier")),
        );
        let mut at = self.common.now_ms;
        for _ in 0..2 {
            at = at.saturating_add(delay);
            let mut targets = (0..count).collect::<Vec<_>>();
            self.common.rng.shuffle(&mut targets);
            for target_index in targets {
                // CopySpawner waits an unscaled 10 ms between target dispatches,
                // then another activation delay before starting the second copy.
                at = at.saturating_add(10);
                self.push_event(
                    at,
                    MaraEvent::MatriarchStrike {
                        ability_kind: cast.ability.kind,
                        target_index,
                        bonus_crit: cast.bonus_crit,
                        context,
                    },
                );
            }
        }
    }

    pub(crate) fn try_mara_from_shadows(&mut self, target_index: u32, context: DamageContext) {
        let Some(index) = self.profile.mechanic_indexes.mara_arachnid_clone else {
            return;
        };
        let mechanic = Arc::clone(&self.profile.mechanics[index]);
        if !self.roll_controlled_random_bool(
            "RandomStream.Mara.Talent.AoeAttackSpender.CloneAttack",
            mechanic_param(&mechanic, parameter_key!("fromShadowsProcChance")),
        ) {
            return;
        }
        let Some(queen) = self.ability(DpsAbilityKind::QueensFang) else {
            return;
        };
        // The graph explicitly overwrites the CDO's one point with six. A
        // fresh spec captures current source stats; no bleed multipliers carry.
        let mut copy_context = context
            .as_proc()
            .with_snapshot(self.capture_damage_source_snapshot(queen.kind));
        copy_context.multiply_damage(
            1.0 + 6.0 * ability_param(queen, parameter_key!("damageMultiplierPerComboPoint")),
        );
        // OtherSource excludes both Feed the Queen and Malevolence. It can
        // still capture an active Assassin's Guile modifier on the new spec.
        if self.common.now_ms < self.hero.mara().assassins_guile_until
            && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent13")
        {
            copy_context.multiply_damage(param(talent, parameter_key!("damageMultiplier")));
        }
        self.record_dynamic_random_proc(&mechanic);
        self.push_event(
            self.common.now_ms.saturating_add(seconds_parameter(
                &mechanic,
                parameter_key!("fromShadowsDelaySeconds"),
            )),
            MaraEvent::FromShadowsStrike {
                target_index,
                context: copy_context,
            },
        );
    }

    pub(crate) fn apply_mara_dot(
        &mut self,
        target_index: u32,
        kind: DpsAbilityKind,
        mut context: DamageContext,
        damage_multiplier: f64,
    ) {
        let Some(ability) = self.ability(kind).cloned() else {
            return;
        };
        let Some(dot) = ability.dot else {
            return;
        };
        context = context.as_proc();
        context.multiply_damage(damage_multiplier);
        self.apply_dot(target_index, kind, &ability, dot, context);
    }

    pub(crate) fn mara_creeping_death_multiplier(&self) -> f64 {
        let scaler = self
            .ability(DpsAbilityKind::MaraAttack)
            .map(|ability| ability_param(ability, parameter_key!("creepingDeathHasteScaler")))
            .unwrap_or(0.0);
        1.0 + self.effective_haste().max(0.0) * scaler
    }

    pub(crate) fn handle_mara_damage_event(&mut self, event: &OutgoingDamageEvent, damage: f64) {
        if self.profile.contract.hero != HeroIdentity::Mara || damage <= 0.0 {
            return;
        }
        // These are the current maintained effects' cooked tags. The two copy
        // sources keep their original ability kind, so kind alone is insufficient.
        let copy = self
            .ability(DpsAbilityKind::MatriarchMacabre)
            .is_some_and(|ability| ability.damage_source == event.source)
            || self
                .profile
                .mechanic_indexes
                .mara_arachnid_clone
                .is_some_and(|index| self.profile.mechanics[index].damage_source == event.source);
        let magic_poison = event.ability_kind.is_some_and(is_mara_poison)
            || (event.provenance == DamageProvenance::Periodic
                && self
                    .profile
                    .mechanic_indexes
                    .mara_arachnid_poison
                    .is_some_and(|index| {
                        self.profile.mechanics[index].damage_source == event.source
                    }));
        let finesse_poison = self
            .profile
            .mechanic_indexes
            .basic_to_aoe
            .iter()
            .any(|index| self.profile.mechanics[*index].damage_source == event.source);
        let excluded_gem = matches!(
            self.profile.damage_sources[event.source.0].id.as_str(),
            "gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal"
                | "gear:ItemTrait.ID.GemTargetedSpikeProc"
        );
        // The monitor subscribes to exact Ability/AutoAttack hit and crit events,
        // not DoT or Miss. Finesse has EffectType.Poison, which does not match
        // its narrower EffectType.Magic.Poison exclusion.
        if self.hero.mara().stealth_active
            && event.provenance != DamageProvenance::Periodic
            && !copy
            && !magic_poison
            && !excluded_gem
        {
            self.hero.mara_mut().stealth_active = false;
            self.deactivate_fixed_buff(AplBuff::BroodingShadows);
            if let Some(stealth) = self.ability(DpsAbilityKind::BroodingShadows).cloned() {
                self.spend_ability_charge(&stealth, stealth.cooldown_ms);
            }
            if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent13") {
                let until = self
                    .common
                    .now_ms
                    .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                self.hero.mara_mut().assassins_guile_until = until;
                self.activate_fixed_buff(AplBuff::AssassinsGuile, until);
            }
        }
        // Venomous Delight listens to exact Ability/DoT damage and accepts both
        // poison tag spellings. Central dispatch covers pools and equipment and
        // prevents the former manual callbacks from refunding twice.
        if !copy && (magic_poison || finesse_poison) {
            self.trigger_mara_poison_damage();
        }
    }

    fn trigger_mara_poison_damage(&mut self) {
        let Some(chance) = self
            .common
            .selected_talents
            .get("mara-talent-id-talent5")
            .map(|talent| param(talent, parameter_key!("procChance")))
        else {
            return;
        };
        if self.roll_controlled_random_bool(
            "RandomStream.Mara.Energy.PoisonChanceToRefundEnergy",
            chance,
        ) {
            self.record_talent_proc("mara-talent-id-talent5");
            let energy = self
                .common
                .selected_talents
                .get("mara-talent-id-talent5")
                .map(|talent| param(talent, parameter_key!("energyGain")))
                .unwrap_or(0.0);
            self.hero.mara_mut().energy =
                (self.hero.mara().energy + energy).min(self.profile.max_primary_resource);
        }
    }

    pub(crate) fn dot_remaining_on_target(&self, target_index: u32, kind: DpsAbilityKind) -> u64 {
        self.common
            .dots
            .get(&(target_index, DotKind::Ability(kind)))
            .map(|dot| dot.expires_ms.saturating_sub(self.common.now_ms))
            .unwrap_or(0)
    }

    pub(crate) fn trigger_mara_hemotoxin(&mut self, target_index: u32, context: DamageContext) {
        if !self
            .common
            .selected_talents
            .contains_key("mara-talent-id-talent14")
        {
            return;
        }
        let key = (target_index, DotKind::Ability(DpsAbilityKind::Hemotoxin));
        if !self
            .common
            .dots
            .get(&key)
            .is_some_and(|dot| dot.stacks > 0 && dot.expires_ms > self.common.now_ms)
        {
            return;
        }
        let bleed_kind = DotKind::Ability(DpsAbilityKind::HemorrhagingStrike);
        let Some(bleed) = self.common.dots.get(&(target_index, bleed_kind)).cloned() else {
            return;
        };
        let Some(explosion) = self.ability(DpsAbilityKind::HemotoxinEruption).cloned() else {
            return;
        };
        // The listener samples an active effect after application, including
        // carried duration and its actual timer period. This is an ordinary-hit
        // estimate, not expected critical damage or a count of future ticks.
        let damage = self.approximate_dot_average_damage(bleed_kind, &bleed, target_index)
            * bleed.expires_ms.saturating_sub(self.common.now_ms) as f64
            / 1_000.0
            * ability_param(&explosion, parameter_key!("primaryBleedFraction"));
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return;
        }
        let secondary_count = self.common.target_count.saturating_sub(1);
        let area_scaler = ability_param(&explosion, parameter_key!("areaBleedFraction"))
            * multi_target_damage_falloff(
                secondary_count.max(1),
                ability_param(
                    &explosion,
                    parameter_key!("targetCountDamageScalingThreshold"),
                ),
            );
        let snapshot = self.capture_damage_source_snapshot(explosion.kind);
        for current_target in std::iter::once(target_index)
            .chain((0..self.common.target_count).filter(|target| *target != target_index))
        {
            self.emit_outgoing_damage(OutgoingDamageEvent {
                ability_kind: Some(explosion.kind),
                source: explosion.damage_source,
                context: context.as_proc(),
                target_index: current_target,
                provenance: DamageProvenance::Direct,
                // The graph cancels the new spec's DamageScale and Expertise;
                // the sampled damage already contains the bleed's source stats.
                amount: DamageAmount::TargetScaled(
                    damage
                        * if current_target == target_index {
                            1.0
                        } else {
                            area_scaler
                        },
                ),
                expertise_snapshot: None,
                primary_stat_multiplier_snapshot: None,
                critical_chance_override: Some(snapshot.critical_chance),
                damage_spread: explosion.damage_spread,
                bonus_crit: 0.0,
                can_crit: true,
                proc_eligible: true,
            });
        }
        // The Blueprint removes a stack after both primary and area damage.
        if let Some(active) = self.common.dots.get_mut(&key) {
            active.stacks = active.stacks.saturating_sub(1);
            if active.stacks == 0 {
                let removed = self.common.dots.remove(&key).expect("Hemotoxin exists");
                self.record_dot_uptime(key.0, key.1, &removed);
            }
        }
    }
}
