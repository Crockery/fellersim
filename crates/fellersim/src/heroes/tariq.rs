use crate::*;
use crate::{DamageContext, Iteration};

#[derive(Debug, Clone)]
pub(crate) enum TariqEvent {
    SwingTimer {
        generation: u64,
    },
    LeapEnded,
    FreeChain {
        target: u32,
        context: DamageContext,
    },
    ThunderFury {
        generation: u64,
    },
    RagingDamage {
        target: u32,
        context: DamageContext,
    },
    SpenderLightning {
        kind: DpsAbilityKind,
        scale_bits: u64,
        bonus_crit_bits: u64,
        context: DamageContext,
    },
    SkullCleave {
        damage_bits: u64,
        context: DamageContext,
    },
    ResourceRefund {
        amount_bits: u64,
    },
    RagingCurrent {
        generation: u64,
        context: DamageContext,
    },
    FaceBreakerCleave {
        damage_bits: u64,
        context: DamageContext,
    },
    HeavyStrikeSecondary {
        lightning: bool,
        context: DamageContext,
    },
    PassiveFury {
        generation: u64,
    },
}

#[derive(Debug)]
pub(crate) struct TariqState {
    pub(crate) swing_remaining_ms: f64,
    pub(crate) swing_updated_ms: u64,
    pub(crate) swing_rate: f64,
    pub(crate) swing_next_ms: Option<u64>,
    pub(crate) swing_generation: u64,
    pub(crate) auto_attacking: bool,
    pub(crate) auto_blocked_until: u64,
    pub(crate) fury: f64,
    pub(crate) thunder_fury_until: u64,
    pub(crate) thunder_fury_generation: u64,
    pub(crate) raging_period_ms: u64,
    pub(crate) thunder_call_until: u64,
    pub(crate) focused_wrath_until: u64,
    pub(crate) focused_wrath_stacks: u32,
    pub(crate) raging_tempest_until: u64,
    pub(crate) raging_tempest_generation: u64,
    pub(crate) raging_current_stacks: u32,
    pub(crate) far_beyond_driven_until: u64,
    pub(crate) far_beyond_driven_stacks: u32,
    pub(crate) kill_em_all_until: u64,
    pub(crate) kill_em_all_stacks: u32,
    pub(crate) square_hammer_until: u64,
    pub(crate) square_hammer_stacks: u32,
    pub(crate) square_hammer_expertise_until: u64,
    pub(crate) schism_hammer_stacks: u32,
    pub(crate) schism_hammer_until: u64,
    pub(crate) schism_skull_stacks: u32,
    pub(crate) schism_skull_until: u64,
    pub(crate) passive_fury_generation: u64,
    pub(crate) thundering_vortex_stacks: u32,
    pub(crate) slayers_mosh_until: Vec<u64>,
    pub(crate) executioners_grin_until: u64,
    pub(crate) executioners_grin_stacks: u32,
}

impl Iteration<'_> {
    pub(crate) fn handle_tariq_event(&mut self, event: TariqEvent) {
        match event {
            TariqEvent::FreeChain { target, context } => self.schedule_tariq_chain(context, target),
            TariqEvent::ThunderFury { generation } => self.tariq_thunder_fury(generation),
            TariqEvent::RagingDamage { target, context } => {
                self.tariq_raging_damage(target, context)
            }
            TariqEvent::SpenderLightning {
                kind,
                scale_bits,
                bonus_crit_bits,
                context,
            } => {
                self.tariq_spender_lightning(
                    kind,
                    f64::from_bits(scale_bits),
                    f64::from_bits(bonus_crit_bits),
                    context,
                );
            }
            TariqEvent::SkullCleave {
                damage_bits,
                context,
            } => {
                let ability = self.ability(DpsAbilityKind::SkullCrusher).unwrap().clone();
                for target in 1..self.common.target_count {
                    self.damage_unscaled_key(
                        Some(ability.kind),
                        self.ability_damage_source(&ability),
                        f64::from_bits(damage_bits),
                        target,
                        DamageProvenance::Explosion,
                        context.as_proc(),
                    );
                    self.try_tariq_motherload(
                        &ability,
                        target,
                        f64::from_bits(damage_bits),
                        context.as_proc(),
                    );
                }
            }
            TariqEvent::LeapEnded => self.tariq_leap_ended(),
            TariqEvent::SwingTimer { generation } => self.tariq_swing_tick(generation),
            TariqEvent::ResourceRefund { amount_bits } => {
                self.hero.tariq_mut().fury = (self.hero.tariq().fury + f64::from_bits(amount_bits))
                    .min(self.profile.max_primary_resource);
            }
            TariqEvent::RagingCurrent {
                generation,
                context,
            } => self.tariq_raging_current(generation, context),
            TariqEvent::FaceBreakerCleave {
                damage_bits,
                context,
            } => {
                let Some(ability) = self.ability(DpsAbilityKind::FaceBreaker).cloned() else {
                    return;
                };
                if !self.charge_work_units(u64::from(self.common.target_count.saturating_sub(1))) {
                    return;
                }
                for target in 1..self.common.target_count {
                    self.damage_unscaled_key(
                        Some(ability.kind),
                        self.ability_damage_source(&ability),
                        f64::from_bits(damage_bits),
                        target,
                        DamageProvenance::Explosion,
                        context.as_proc(),
                    );
                }
            }
            TariqEvent::HeavyStrikeSecondary { lightning, context } => {
                self.tariq_heavy_strike_secondary(lightning, context);
            }
            TariqEvent::PassiveFury { generation } => {
                self.tariq_passive_fury(generation);
            }
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn try_tariq_spirit_refund(&mut self, ability: &CompiledAbility, spent: f64) {
        if spent <= 0.0
            || !self.roll_controlled_random_bool(
                SPIRIT_PROC_RANDOM_STREAM_TAG,
                spirit_refund_chance(self.effective_spirit(), 1.0, 0.0),
            )
        {
            return;
        }
        self.record_spirit_refund_proc();
        self.push_event(
            self.common.now_ms.saturating_add(ability_seconds_parameter(
                ability,
                parameter_key!("spiritRefundDelaySeconds"),
            )),
            TariqEvent::ResourceRefund {
                amount_bits: spent.to_bits(),
            },
        );
        self.shared.spirit = (self.shared.spirit
            + ability_param(ability, parameter_key!("spiritRefundSpiritGain")))
        .min(self.profile.max_spirit);
    }

    pub(crate) fn commit_tariq_ability(
        &mut self,
        ability: &CompiledAbility,
        context: DamageContext,
    ) {
        if self.profile.contract.hero != HeroIdentity::Tariq {
            return;
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::WildSwing
                | DpsAbilityKind::FaceBreaker
                | DpsAbilityKind::HeavyStrike
                | DpsAbilityKind::TariqAttack
        ) {
            let scale =
                if ability.kind == DpsAbilityKind::HeavyStrike && !self.tariq_in_hit_window() {
                    ability_param(ability, parameter_key!("weakResourceMultiplier"))
                } else {
                    1.0
                };
            self.add_tariq_fury(ability.primary_resource_generated * scale);
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::HeavyStrike
                | DpsAbilityKind::WildSwing
                | DpsAbilityKind::FaceBreaker
                | DpsAbilityKind::SkullCrusher
                | DpsAbilityKind::TariqChainLightning
                | DpsAbilityKind::CullingStrike
        ) {
            self.hero.tariq_mut().auto_attacking = true;
            self.refresh_tariq_swing_clock();
        }
        if ability.kind == DpsAbilityKind::HammerStorm {
            self.reset_tariq_swing(false);
            let channel = ability.channel.as_ref().unwrap();
            let rate =
                if channel.scale_duration_with_ability_time_rate && ability.scale_time_with_haste {
                    (1.0 + self.effective_haste()).max(0.05)
                } else {
                    1.0
                };
            self.hero.tariq_mut().auto_blocked_until = self
                .common
                .now_ms
                .saturating_add((channel.duration_ms as f64 / rate).round() as u64);
        }
        if ability.kind == DpsAbilityKind::LeapSmash {
            self.hero.tariq_mut().auto_blocked_until = self
                .common
                .now_ms
                .saturating_add(ability.first_hit_delay_ms);
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::ThunderCall | DpsAbilityKind::RagingTempest
        ) {
            self.start_tariq_thunder_fury();
        }
        if ability.kind == DpsAbilityKind::RagingTempest {
            self.hero.tariq_mut().raging_tempest_until = self
                .common
                .now_ms
                .saturating_add(ability.effect_duration_ms);
            self.hero.tariq_mut().thunder_call_until = self.hero.tariq().raging_tempest_until;
            self.hero.tariq_mut().raging_tempest_generation =
                self.hero.tariq().raging_tempest_generation.wrapping_add(1);
            self.hero.tariq_mut().raging_current_stacks = 1;
            self.hero.tariq_mut().auto_blocked_until = self
                .common
                .now_ms
                .saturating_add(ability.first_hit_delay_ms);
            self.hero.tariq_mut().raging_period_ms =
                (ability_seconds_parameter(ability, parameter_key!("pulsePeriodSeconds")) as f64
                    / (1.0 + self.effective_haste()).max(0.05))
                .round()
                .max(1.0) as u64;
            let until = self.hero.tariq().raging_tempest_until;
            self.activate_fixed_buff(AplBuff::RagingTempest, until);
            self.activate_fixed_buff(AplBuff::ThunderCall, until);
            self.push_event(
                self.common
                    .now_ms
                    .saturating_add(self.hero.tariq().raging_period_ms),
                TariqEvent::RagingCurrent {
                    generation: self.hero.tariq().raging_tempest_generation,
                    context,
                },
            );
        }
        if ability.kind == DpsAbilityKind::HeavyStrike {
            if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent8") {
                let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
                let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
                let active_stacks =
                    if self.common.now_ms < self.hero.tariq().far_beyond_driven_until {
                        self.hero.tariq().far_beyond_driven_stacks
                    } else {
                        0
                    };
                self.hero.tariq_mut().far_beyond_driven_stacks =
                    active_stacks.saturating_add(1).min(maximum_stacks);
                self.hero.tariq_mut().far_beyond_driven_until =
                    self.common.now_ms.saturating_add(duration_ms);
                let until = self.hero.tariq().far_beyond_driven_until;
                self.activate_fixed_buff(AplBuff::FarBeyondDriven, until);
            }
            if self.tariq_in_hit_window()
                && let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent18")
            {
                let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
                let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
                let active_stacks = if self.common.now_ms < self.hero.tariq().square_hammer_until {
                    self.hero.tariq().square_hammer_stacks
                } else {
                    0
                };
                self.hero.tariq_mut().square_hammer_stacks =
                    active_stacks.saturating_add(1).min(maximum_stacks);
                self.hero.tariq_mut().square_hammer_until =
                    self.common.now_ms.saturating_add(duration_ms);
                let until = self.hero.tariq().square_hammer_until;
                self.activate_fixed_buff(AplBuff::SquareHammer, until);
            }
        }
        if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent9") {
            let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
            let duration_ms = ms_param(talent, parameter_key!("durationSeconds"));
            let proc_chance = param(talent, parameter_key!("procChance"));
            if ability.kind == DpsAbilityKind::HammerStorm
                && self.roll_controlled_random_bool(
                    "RandomStream.Ink.Talents.ChanceIncreasedSpenderDamage.AoeAttack",
                    proc_chance,
                )
            {
                self.record_talent_proc("ink-talent-id-talent9");
                let previous = if self.common.now_ms < self.hero.tariq().schism_hammer_until {
                    self.hero.tariq().schism_hammer_stacks
                } else {
                    0
                };
                self.hero.tariq_mut().schism_hammer_stacks =
                    previous.saturating_add(1).min(maximum_stacks);
                self.hero.tariq_mut().schism_hammer_until =
                    self.common.now_ms.saturating_add(duration_ms);
            } else if ability.kind == DpsAbilityKind::SkullCrusher
                && self.roll_controlled_random_bool(
                    "RandomStream.Ink.Talents.ChanceIncreasedSpenderDamage.HeavySingleTargetAttack",
                    proc_chance,
                )
            {
                self.record_talent_proc("ink-talent-id-talent9");
                let previous = if self.common.now_ms < self.hero.tariq().schism_skull_until {
                    self.hero.tariq().schism_skull_stacks
                } else {
                    0
                };
                self.hero.tariq_mut().schism_skull_stacks =
                    previous.saturating_add(1).min(maximum_stacks);
                self.hero.tariq_mut().schism_skull_until =
                    self.common.now_ms.saturating_add(duration_ms);
            }
        }
        let kill_em_all = self
            .common
            .selected_talents
            .get("ink-talent-id-talent11")
            .map(|talent| {
                (
                    param(talent, parameter_key!("procChance")),
                    param_u32(talent, parameter_key!("charges")),
                    ms_param(talent, parameter_key!("durationSeconds")),
                )
            });
        if let Some((proc_chance, charges, duration_ms)) = kill_em_all
            && matches!(
                ability.kind,
                DpsAbilityKind::RagingTempest
                    | DpsAbilityKind::WildSwing
                    | DpsAbilityKind::FaceBreaker
                    | DpsAbilityKind::TariqChainLightning
                    | DpsAbilityKind::HammerStorm
                    | DpsAbilityKind::SkullCrusher
                    | DpsAbilityKind::CullingStrike
            )
            && !(self.hero.tariq().kill_em_all_stacks > 0
                && self.common.now_ms < self.hero.tariq().kill_em_all_until)
            && self.roll_controlled_random_bool(
                "RandomStream.Ink.Talents.AutoAttackBuff.AlwaysHitWindowBuff",
                proc_chance,
            )
        {
            self.record_talent_proc("ink-talent-id-talent11");
            self.reset_cooldown(DpsAbilityKind::HeavyStrike);
            self.hero.tariq_mut().kill_em_all_stacks = charges;
            self.hero.tariq_mut().kill_em_all_until =
                self.common.now_ms.saturating_add(duration_ms);
            let until = self.hero.tariq().kill_em_all_until;
            self.activate_fixed_buff(AplBuff::KillEmAll, until);
        }
        if is_primary_skill_commit(ability.kind)
            && let Some(index) = self.profile.mechanic_indexes.tariq_executioners_grin
        {
            let mechanic = &self.profile.mechanics[index];
            let proc_chance =
                mechanic_param(mechanic, parameter_key!("executionersGrinProcChance"));
            let duration_ms =
                (mechanic_param(mechanic, parameter_key!("executionersGrinDurationSeconds"))
                    * 1_000.0)
                    .round()
                    .max(0.0) as u64;
            if self.roll_controlled_random_bool(
                "RandomStream.Ink.Talents.LowHealthSingleTargetResourceDamage.ChanceActiveOnCommit",
                proc_chance,
            ) {
                self.record_dynamic_random_proc(&Arc::clone(mechanic));
                let maximum =
                    mechanic_u32(mechanic, parameter_key!("executionersGrinMaximumStacks"));
                let stacks = if self.common.now_ms < self.hero.tariq().executioners_grin_until {
                    self.hero.tariq().executioners_grin_stacks
                } else {
                    0
                };
                self.hero.tariq_mut().executioners_grin_stacks =
                    stacks.saturating_add(1).min(maximum);
                self.hero.tariq_mut().executioners_grin_until =
                    self.common.now_ms.saturating_add(duration_ms);
            }
        }
    }

    pub(crate) fn schedule_tariq_heavy_strike_secondaries(&mut self, context: DamageContext) {
        let ability = self.ability(DpsAbilityKind::HeavyStrike).unwrap();
        let rate = if ability.scale_time_with_haste {
            (1.0 + self.effective_haste()).max(0.05)
        } else {
            1.0
        };
        let physical_delay = (ability.first_hit_delay_ms as f64 / rate).round() as u64;
        let lightning_delay = ((ability.first_hit_delay_ms as f64
            + ability_seconds_parameter(ability, parameter_key!("lightningDelaySeconds")) as f64)
            / rate)
            .round() as u64;
        let lightning = self.common.now_ms < self.hero.tariq().thunder_call_until
            || self.common.now_ms < self.hero.tariq().raging_tempest_until;
        if self.common.target_count > 1 {
            self.push_event(
                self.common.now_ms.saturating_add(physical_delay),
                TariqEvent::HeavyStrikeSecondary {
                    lightning: false,
                    context,
                },
            );
        }
        if lightning {
            self.push_event(
                self.common.now_ms.saturating_add(lightning_delay),
                TariqEvent::HeavyStrikeSecondary {
                    lightning: true,
                    context,
                },
            );
        }
    }

    fn tariq_heavy_strike_secondary(&mut self, lightning: bool, context: DamageContext) {
        let ability = self.ability(DpsAbilityKind::HeavyStrike).unwrap().clone();
        let window_multiplier = if self.tariq_in_hit_window() {
            1.0
        } else {
            ability_param(&ability, parameter_key!("weakDamageMultiplier"))
        };
        let coefficient = window_multiplier
            * if lightning {
                ability_param(&ability, parameter_key!("lightningPowerCoefficient"))
            } else {
                ability.power_coefficient
                    * ability_param(&ability, parameter_key!("cleaveDamageMultiplier"))
            };
        let threshold = ability_param(
            &ability,
            if lightning {
                parameter_key!("lightningTargetCountDamageScalingThreshold")
            } else {
                parameter_key!("cleaveTargetCountDamageScalingThreshold")
            },
        );
        let bonus_crit = if lightning {
            self.common
                .selected_talents
                .get("ink-talent-id-talent16")
                .map(|talent| param(talent, parameter_key!("lightningCriticalStrikeBonus")))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let additional = self.common.target_count.saturating_sub(1);
        if !self.charge_work_units(u64::from(additional) + u64::from(lightning)) {
            return;
        }
        let falloff = multi_target_damage_falloff(additional.max(1), threshold);
        for target in u32::from(!lightning)..self.common.target_count {
            // Separate recapturing specs: independent spread/critical rolls and
            // current attributes. The lightning primary always has unit falloff.
            self.damage_raw_with_spread_key(
                Some(ability.kind),
                self.ability_damage_source(&ability),
                coefficient * self.profile.power * if target == 0 { 1.0 } else { falloff },
                None,
                None,
                None,
                ability.damage_spread,
                bonus_crit,
                true,
                true,
                target,
                context.without_cast_proc(),
            );
        }
    }

    pub(crate) fn resolve_tariq_impact(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        resolved_damage: f64,
        _critical: bool,
        context: DamageContext,
    ) {
        if self.profile.contract.hero != HeroIdentity::Tariq {
            return;
        }
        if target_index == 0 {
            let (cleave_multiplier, threshold) = match ability.kind {
                DpsAbilityKind::FaceBreaker => (
                    ability_param(ability, parameter_key!("cleaveDamageMultiplier")),
                    ability_param(
                        ability,
                        parameter_key!("cleaveTargetCountDamageScalingThreshold"),
                    ),
                ),
                DpsAbilityKind::SkullCrusher => self
                    .common
                    .selected_talents
                    .get("ink-talent-id-talent4")
                    .map(|talent| {
                        (
                            param(talent, parameter_key!("cleaveDamageMultiplier")),
                            ability_param(
                                ability,
                                parameter_key!("cleaveTargetCountDamageScalingThreshold"),
                            ),
                        )
                    })
                    .unwrap_or((0.0, 0.0)),
                _ => (0.0, 0.0),
            };
            if cleave_multiplier > 0.0 && resolved_damage > 0.0 {
                let additional = self.common.target_count.saturating_sub(1);
                let falloff = multi_target_damage_falloff(additional.max(1), threshold);
                if ability.kind == DpsAbilityKind::FaceBreaker {
                    // The monitor copies resolved health loss, including noncrits;
                    // its Empty damage preset does not roll or scale it again.
                    self.push_event(
                        self.common.now_ms.saturating_add(ability_seconds_parameter(
                            ability,
                            parameter_key!("cleaveDelaySeconds"),
                        )),
                        TariqEvent::FaceBreakerCleave {
                            damage_bits: (resolved_damage * cleave_multiplier * falloff).to_bits(),
                            context,
                        },
                    );
                } else {
                    self.push_event(
                        self.common.now_ms.saturating_add(ability_seconds_parameter(
                            ability,
                            parameter_key!("cleaveDelaySeconds"),
                        )),
                        TariqEvent::SkullCleave {
                            damage_bits: (resolved_damage * cleave_multiplier * falloff).to_bits(),
                            context,
                        },
                    );
                }
            }
        }
        let attack_chain_proc_chance = self
            .common
            .selected_talents
            .get("ink-talent-id-talent3")
            .map(|talent| param(talent, parameter_key!("procChance")));
        if target_index == 0
            && ability.kind == DpsAbilityKind::HeavyStrike
            && resolved_damage > 0.0
            && self.tariq_in_hit_window()
            && attack_chain_proc_chance.is_some_and(|chance| {
                self.roll_controlled_random_bool(
                    "RandomStream.Ink.Talents.BouncyProjectile.FreeCastFromAttack",
                    chance,
                )
            })
        {
            self.record_talent_proc("ink-talent-id-talent3");
            let delay = ms_param(
                self.common
                    .selected_talents
                    .get("ink-talent-id-talent3")
                    .unwrap(),
                parameter_key!("activationDelaySeconds"),
            );
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                TariqEvent::FreeChain {
                    target: target_index,
                    context: context.as_proc(),
                },
            );
        }
        if ability.kind == DpsAbilityKind::TariqChainLightning {
            let mut fury = ability_param(ability, parameter_key!("furyPerHit"));
            if let Some(index) = self.profile.mechanic_indexes.tariq_thundering_vortex {
                let mechanic = &self.profile.mechanics[index];
                fury *= mechanic_param(mechanic, parameter_key!("chainFuryMultiplier"));
                let maximum = mechanic_u32(mechanic, parameter_key!("vortexMaximumStacks"));
                self.hero.tariq_mut().thundering_vortex_stacks = self
                    .hero
                    .tariq()
                    .thundering_vortex_stacks
                    .saturating_add(1)
                    .min(maximum);
            }
            self.add_tariq_fury(fury);
        }
        if ability.kind == DpsAbilityKind::LeapSmash
            && let Some(index) = self.profile.mechanic_indexes.tariq_slayers_mosh
        {
            let duration_ms = (mechanic_param(
                &self.profile.mechanics[index],
                parameter_key!("leapTargetDamageDurationSeconds"),
            ) * 1_000.0)
                .round()
                .max(0.0) as u64;
            if let Some(until) = self
                .hero
                .tariq_mut()
                .slayers_mosh_until
                .get_mut(target_index as usize)
            {
                *until = self.common.now_ms.saturating_add(duration_ms);
            }
        }
        self.try_tariq_motherload(ability, target_index, resolved_damage, context);
    }

    fn try_tariq_motherload(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        resolved_damage: f64,
        context: DamageContext,
    ) {
        let aoe_chain_procs_per_minute = self
            .common
            .selected_talents
            .get("ink-talent-id-talent15")
            .map(|talent| param(talent, parameter_key!("procsPerMinute")));
        if resolved_damage > 0.0
            && matches!(
                ability.kind,
                DpsAbilityKind::HammerStorm | DpsAbilityKind::SkullCrusher
            )
            && aoe_chain_procs_per_minute.is_some_and(|rate| {
                self.roll_proc_per_minute(
                    "RandomStream.Ink.Talents.BouncyProjectile.FreeCastFromAoeAttack",
                    rate,
                    true,
                )
            })
        {
            self.record_talent_proc("ink-talent-id-talent15");
            let delay = ms_param(
                self.common
                    .selected_talents
                    .get("ink-talent-id-talent15")
                    .unwrap(),
                parameter_key!("activationDelaySeconds"),
            );
            self.push_event(
                self.common.now_ms.saturating_add(delay),
                TariqEvent::FreeChain {
                    target: target_index,
                    context: context.as_proc(),
                },
            );
        }
    }

    pub(crate) fn tariq_spender_lightning(
        &mut self,
        kind: DpsAbilityKind,
        scale: f64,
        bonus_crit: f64,
        context: DamageContext,
    ) {
        let ability = self.ability(kind).unwrap().clone();
        let targets = if kind == DpsAbilityKind::HammerStorm {
            self.common.target_count
        } else {
            1
        };
        let falloff = if kind == DpsAbilityKind::HammerStorm {
            multi_target_damage_falloff(
                targets,
                ability_param(
                    &ability,
                    parameter_key!("lightningTargetCountDamageScalingThreshold"),
                ),
            )
        } else {
            1.0
        };
        let crack = self
            .common
            .selected_talents
            .get("ink-talent-id-talent16")
            .map(|t| param(t, parameter_key!("lightningCriticalStrikeBonus")))
            .unwrap_or(0.0);
        let snapshot = context.source_snapshot;
        for target in 0..targets {
            self.damage_raw_with_spread_key(
                Some(kind),
                self.ability_damage_source(&ability),
                ability_param(&ability, parameter_key!("lightningPowerCoefficient"))
                    * self.profile.power
                    * scale
                    * falloff,
                snapshot.map(|s| s.expertise),
                snapshot.map(|s| s.primary_stat_multiplier),
                snapshot.map(|s| s.critical_chance),
                HERO_DAMAGE_SPREAD_WIDTH,
                bonus_crit + crack,
                true,
                true,
                target,
                context.without_cast_proc(),
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn tariq_trigger_chain_lightning(&mut self, context: DamageContext) {
        self.schedule_tariq_chain(context, 0);
    }

    pub(crate) fn schedule_tariq_chain(&mut self, mut context: DamageContext, first_target: u32) {
        let Some(index) = self
            .profile
            .abilities
            .iter()
            .position(|a| a.kind == DpsAbilityKind::TariqChainLightning)
        else {
            return;
        };
        let ability = self.profile.abilities[index].clone();
        let hits = ability_u32_rounded(&ability, parameter_key!("chainHits")).max(1);
        let targets = self.common.target_count.min(hits).max(1);
        let increase = ability_param(&ability, parameter_key!("uniqueTargetDamageIncrease"));
        if let Some(index) = self.profile.mechanic_indexes.tariq_thundering_vortex {
            let mechanic = &self.profile.mechanics[index];
            let required = mechanic_param(mechanic, parameter_key!("vortexStacksRequired"))
                .round()
                .max(1.0) as u32;
            if self.hero.tariq().thundering_vortex_stacks >= required {
                self.hero.tariq_mut().thundering_vortex_stacks -= required;
                context.multiply_damage(mechanic_param(
                    mechanic,
                    parameter_key!("vortexDamageMultiplier"),
                ));
            }
        }
        if !self.charge_work_units(u64::from(hits)) {
            return;
        }
        for hit in 0..hits {
            // Exhausted target lists bounce through the owner for one raw tick,
            // then clear only the visited list. Unique-target history survives.
            let intervals = u64::from(hit + hit / targets);
            let at = self
                .common
                .now_ms
                .saturating_add(ability.first_hit_delay_ms)
                .saturating_add(intervals.saturating_mul(ability_seconds_parameter(
                    &ability,
                    parameter_key!("jumpDelaySeconds"),
                )));
            self.push_event(
                at,
                CoreEvent::ImpactBatch {
                    index,
                    impacts: vec![ImpactSpec {
                        single_hit: true,
                        damage_scale_bits: (1.0 + f64::from(hit.min(targets - 1)) * increase)
                            .to_bits(),
                        target_index: (first_target + hit % targets) % self.common.target_count,
                    }],
                    context: CastImpactContext::new(if hit == 0 {
                        context
                    } else {
                        context.without_cast_proc()
                    }),
                },
            );
        }
    }

    fn tariq_leap_ended(&mut self) {
        if self
            .common
            .selected_talents
            .contains_key("ink-talent-id-talent14")
        {
            self.reset_tariq_swing(true);
            let leap = self.ability(DpsAbilityKind::LeapSmash).unwrap();
            self.add_tariq_fury(leap.primary_resource_generated);
        }
        if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent5") {
            let stacks = param_u32(talent, parameter_key!("focusedWrathStacks"));
            let focused = self.ability(DpsAbilityKind::FocusedWrath).unwrap();
            let maximum = ability_u32_rounded(focused, parameter_key!("maximumStacks"));
            let until = self
                .common
                .now_ms
                .saturating_add(focused.effect_duration_ms);
            let previous = if self.common.now_ms < self.hero.tariq().focused_wrath_until {
                self.hero.tariq().focused_wrath_stacks
            } else {
                0
            };
            self.hero.tariq_mut().focused_wrath_stacks =
                previous.saturating_add(stacks).min(maximum);
            self.hero.tariq_mut().focused_wrath_until = until;
            self.activate_fixed_buff(AplBuff::FocusedWrath, until);
        }
    }

    pub(crate) fn add_tariq_fury(&mut self, amount: f64) {
        self.hero.tariq_mut().fury =
            (self.hero.tariq().fury + amount).clamp(0.0, self.profile.max_primary_resource);
    }

    pub(crate) fn start_tariq_passive_fury(&mut self) {
        if self.profile.contract.hero != HeroIdentity::Tariq {
            return;
        }
        self.hero.tariq_mut().passive_fury_generation =
            self.hero.tariq().passive_fury_generation.wrapping_add(1);
        self.push_event(
            1_000,
            TariqEvent::PassiveFury {
                generation: self.hero.tariq().passive_fury_generation,
            },
        );
    }

    pub(crate) fn tariq_passive_fury(&mut self, generation: u64) {
        if generation != self.hero.tariq().passive_fury_generation {
            return;
        }
        let mut fury = 0.0;
        if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent6") {
            fury += param(talent, parameter_key!("furyPerSecond"));
        }
        self.add_tariq_fury(fury);
        let next = self.common.now_ms.saturating_add(1_000);
        if next <= ENCOUNTER_DURATION_MS {
            self.push_event(next, TariqEvent::PassiveFury { generation });
        }
    }

    pub(crate) fn tariq_raging_current(&mut self, generation: u64, context: DamageContext) {
        if generation != self.hero.tariq().raging_tempest_generation
            || self.common.now_ms >= self.hero.tariq().raging_tempest_until
        {
            return;
        }
        let Some(ability) = self.ability(DpsAbilityKind::RagingTempest).cloned() else {
            return;
        };
        let snapshot = self.capture_damage_source_snapshot(ability.kind);
        let delay = ability_seconds_parameter(&ability, parameter_key!("pulseVisualDelaySeconds"));
        self.push_event(
            self.common.now_ms.saturating_add(delay),
            TariqEvent::RagingDamage {
                target: (self.common.sequence as u32) % self.common.target_count,
                context: context.as_proc().with_snapshot(snapshot),
            },
        );
        self.hero.tariq_mut().raging_current_stacks = self
            .hero
            .tariq()
            .raging_current_stacks
            .saturating_add(1)
            .min(ability_u32_rounded(
                &ability,
                parameter_key!("maximumStacks"),
            ));
        let next = self
            .common
            .now_ms
            .saturating_add(self.hero.tariq().raging_period_ms);
        if next < self.hero.tariq().raging_tempest_until {
            self.push_event(
                next,
                TariqEvent::RagingCurrent {
                    generation,
                    context,
                },
            );
        }
    }
    fn tariq_raging_damage(&mut self, target: u32, context: DamageContext) {
        let ability = self.ability(DpsAbilityKind::RagingTempest).unwrap().clone();
        self.damage_raw_with_spread_key(
            Some(DpsAbilityKind::RagingTempest),
            self.ability_damage_source(&ability),
            ability_param(&ability, parameter_key!("pulsePowerCoefficient")) * self.profile.power,
            context.source_snapshot.map(|s| s.expertise),
            context.source_snapshot.map(|s| s.primary_stat_multiplier),
            context.source_snapshot.map(|s| s.critical_chance),
            ability_param(&ability, parameter_key!("pulseDamageSpread")),
            if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent16") {
                param(talent, parameter_key!("lightningCriticalStrikeBonus"))
            } else {
                0.0
            },
            true,
            true,
            target,
            context,
        );
    }

    fn start_tariq_thunder_fury(&mut self) {
        let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent7") else {
            return;
        };
        let duration = ms_param(talent, parameter_key!("furyDurationSeconds"));
        let period = ms_param(talent, parameter_key!("furyPeriodSeconds"));
        self.hero.tariq_mut().thunder_fury_generation =
            self.hero.tariq().thunder_fury_generation.wrapping_add(1);
        self.hero.tariq_mut().thunder_fury_until = self.common.now_ms.saturating_add(duration);
        self.push_event(
            self.common.now_ms.saturating_add(period),
            TariqEvent::ThunderFury {
                generation: self.hero.tariq().thunder_fury_generation,
            },
        );
    }

    fn tariq_thunder_fury(&mut self, generation: u64) {
        if generation != self.hero.tariq().thunder_fury_generation
            || self.common.now_ms > self.hero.tariq().thunder_fury_until
        {
            return;
        }
        let talent = self
            .common
            .selected_talents
            .get("ink-talent-id-talent7")
            .unwrap();
        let amount = param(talent, parameter_key!("furyPerTick"));
        let period = ms_param(talent, parameter_key!("furyPeriodSeconds"));
        self.add_tariq_fury(amount);
        let next = self.common.now_ms.saturating_add(period);
        if next <= self.hero.tariq().thunder_fury_until {
            self.push_event(next, TariqEvent::ThunderFury { generation });
        }
    }
}
