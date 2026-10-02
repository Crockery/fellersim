use crate::Iteration;
use crate::*;

#[derive(Debug, Clone)]
pub(crate) enum ElarionEvent {
    ResourceRefund {
        amount_bits: u64,
    },
    ProjectileBatch {
        index: usize,
        impacts: Vec<ImpactSpec>,
        context: CastImpactContext,
    },
    SwingTimer {
        generation: u64,
    },
    HighwindFinished {
        ability: PreparedAbility,
        targets_hit: u32,
    },
    StrikersAimDecay {
        generation: u64,
    },
}

#[derive(Debug)]
pub(crate) struct ElarionState {
    pub(crate) focus: f64,
    pub(crate) swing_remaining_ms: f64,
    pub(crate) swing_updated_ms: u64,
    pub(crate) swing_rate: f64,
    pub(crate) swing_generation: u64,
    pub(crate) swing_next_ms: Option<u64>,
    pub(crate) auto_attacking: bool,
    pub(crate) auto_blocked_until: u64,
    pub(crate) skylit_grace_active: bool,
    pub(crate) celestial_impetus_stacks: u32,
    pub(crate) celestial_impetus_until: u64,
    pub(crate) multishot_proc_stacks: u32,
    pub(crate) empowered_multishot_stacks: u32,
    pub(crate) empowered_multishot_until: u64,
    pub(crate) skystriders_grace_until: u64,
    pub(crate) event_horizon_until: u64,
    pub(crate) skystriders_supremacy_until: u64,
    pub(crate) skystriders_supremacy_stacks: u32,
    pub(crate) impending_heartseeker_until: u64,
    pub(crate) resurgent_winds_stacks: u32,
    pub(crate) resurgent_winds_until: u64,
    pub(crate) highwind_casts: u32,
    pub(crate) strikers_aim_stacks: u32,
    pub(crate) strikers_aim_generation: u64,
    pub(crate) strikers_aim_next_decay_ms: u64,
    pub(crate) mark_stacks: Vec<u32>,
    pub(crate) mark_until: Vec<u64>,
    pub(crate) previous_mark_event_was_starfall: bool,
    pub(crate) preserve_mark_stack: bool,
    pub(crate) cached_mark_target_index: u32,
    pub(crate) shimmer_stacks: Vec<u32>,
    pub(crate) shimmer_until: Vec<u64>,
}

impl Iteration<'_> {
    pub(crate) fn elarion_horizon_damage_multiplier(&self) -> f64 {
        if let HeroState::Elarion(state) = &self.hero
            && self.common.now_ms < state.event_horizon_until
        {
            return self
                .ability(DpsAbilityKind::EventHorizon)
                .map(|ability| ability_param(ability, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0);
        }
        1.0
    }

    pub(crate) fn elarion_focus_pulse(&self) -> (u64, f64) {
        self.ability(DpsAbilityKind::FocusedShot)
            .map(|ability| {
                (
                    (ability_param(ability, parameter_key!("focusRegenerationIntervalSeconds"))
                        * 1_000.0)
                        .round() as u64,
                    ability_param(ability, parameter_key!("focusRegenerationPerPulse"))
                        * (1.0 + self.effective_haste()).max(0.05),
                )
            })
            .unwrap_or((0, 0.0))
    }

    pub(crate) fn gain_elarion_passive_focus(&mut self, amount: f64) {
        self.hero.elarion_mut().focus =
            (self.hero.elarion().focus + amount).min(self.profile.max_primary_resource);
    }

    pub(crate) fn handle_elarion_event(&mut self, event: ElarionEvent) {
        match event {
            ElarionEvent::ProjectileBatch {
                index,
                impacts,
                mut context,
            } => {
                let ability = &self.profile.abilities[index];
                context.damage.source_snapshot =
                    Some(self.capture_damage_source_snapshot(ability.kind));
                self.push_event(
                    self.common
                        .now_ms
                        .saturating_add(ability.first_hit_delay_ms),
                    CoreEvent::ImpactBatch {
                        index,
                        impacts,
                        context,
                    },
                );
            }
            ElarionEvent::SwingTimer { generation } => self.elarion_swing_tick(generation),
            ElarionEvent::HighwindFinished {
                ability,
                targets_hit,
            } => {
                self.apply_elarion_strikers_aim(&ability, targets_hit);
            }
            ElarionEvent::ResourceRefund { amount_bits } => {
                self.hero.elarion_mut().focus = (self.hero.elarion().focus
                    + f64::from_bits(amount_bits))
                .min(self.profile.max_primary_resource);
            }
            ElarionEvent::StrikersAimDecay { generation } => {
                self.elarion_strikers_aim_decay(generation);
            }
        }
    }
}

impl Iteration<'_> {
    pub(crate) fn commit_elarion_ability(&mut self, ability: &CompiledAbility) {
        if self.profile.contract.hero != HeroIdentity::Elarion {
            return;
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::FocusedShot
                | DpsAbilityKind::HighwindArrow
                | DpsAbilityKind::CelestialShot
                | DpsAbilityKind::HeartseekerBarrage
                | DpsAbilityKind::Multishot
        ) {
            self.hero.elarion_mut().auto_attacking = true;
        }
        match ability.kind {
            DpsAbilityKind::StarfallVolley => {
                self.hero.elarion_mut().skylit_grace_active = self
                    .common
                    .selected_talents
                    .contains_key("bowguy-talent-id-talent4");
            }
            DpsAbilityKind::SkystridersGrace => {
                self.hero.elarion_mut().skystriders_grace_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::SkystridersGrace,
                    self.hero.elarion().skystriders_grace_until,
                );
            }
            DpsAbilityKind::EventHorizon => {
                self.hero.elarion_mut().event_horizon_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::EventHorizon,
                    self.hero.elarion().event_horizon_until,
                );
            }
            DpsAbilityKind::SkystridersSupremacy => {
                let talent = self
                    .common
                    .selected_talents
                    .get("bowguy-talent-id-talent13");
                let duration = talent
                    .map(|talent| ms_param(talent, parameter_key!("durationSeconds")))
                    .unwrap_or(ability.effect_duration_ms);
                let stacks = talent
                    .map(|talent| param_u32(talent, parameter_key!("maximumStacks")))
                    .unwrap_or(1);
                self.hero.elarion_mut().skystriders_supremacy_stacks = stacks;
                self.hero.elarion_mut().skystriders_supremacy_until =
                    self.common.now_ms.saturating_add(duration);
                self.activate_fixed_buff(
                    AplBuff::SkystridersSupremacy,
                    self.hero.elarion().skystriders_supremacy_until,
                );
            }
            DpsAbilityKind::HighwindArrow => {
                self.hero.elarion_mut().highwind_casts =
                    self.hero.elarion().highwind_casts.saturating_add(1);
                if let Some(reduction) = self
                    .common
                    .selected_talents
                    .get("bowguy-talent-id-talent8")
                    .map(|talent| ms_param(talent, parameter_key!("cooldownReductionSeconds")))
                {
                    self.reduce_cooldown(DpsAbilityKind::LunarlightMark, reduction);
                }
            }
            DpsAbilityKind::Multishot => {
                if let Some(reduction) = self
                    .common
                    .selected_talents
                    .get("bowguy-talent-id-talent1")
                    .map(|talent| {
                        let key = if self.common.now_ms
                            < self.hero.elarion().skystriders_supremacy_until
                            || (self.common.now_ms < self.hero.elarion().empowered_multishot_until
                                && self.hero.elarion().empowered_multishot_stacks > 0)
                        {
                            parameter_key!("empoweredCooldownReductionSeconds")
                        } else {
                            parameter_key!("cooldownReductionSeconds")
                        };
                        ms_param(talent, key)
                    })
                {
                    self.reduce_cooldown(DpsAbilityKind::StarfallVolley, reduction);
                }
                if let Some(index) = self.profile.mechanic_indexes.elarion_astronomers_hail {
                    let extension = seconds_parameter(
                        &self.profile.mechanics[index],
                        parameter_key!("starfallExtensionPerMultishotSeconds"),
                    );
                    if !self.charge_work_units(u64::from(self.common.target_count)) {
                        return;
                    }
                    for target_index in 0..self.common.target_count {
                        self.increase_dot_duration(
                            target_index,
                            DotKind::Ability(DpsAbilityKind::StarfallVolley),
                            extension,
                        );
                    }
                }
            }
            _ => {}
        }
        if self
            .common
            .selected_talents
            .contains_key("bowguy-talent-id-talent9")
            && matches!(
                ability.kind,
                DpsAbilityKind::CelestialShot | DpsAbilityKind::Multishot
            )
        {
            let reduction = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent9")
                .map(|talent| ms_param(talent, parameter_key!("cooldownReductionSeconds")))
                .unwrap_or(0);
            self.reduce_cooldown(DpsAbilityKind::HighwindArrow, reduction);
            self.reduce_cooldown(DpsAbilityKind::HeartseekerBarrage, reduction);
        }
        if ability.kind == DpsAbilityKind::FocusedShot
            && let Some((chance, duration, stacks, maximum)) = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent7")
                .map(|talent| {
                    (
                        param(talent, parameter_key!("procChance")),
                        ms_param(talent, parameter_key!("durationSeconds")),
                        param_u32(talent, parameter_key!("stacks")),
                        param_u32(talent, parameter_key!("maximumStacks")).max(1),
                    )
                })
            && self.roll_controlled_random_bool(
                "RandomStream.Bowguy.Talent.CastedProjectileDamage.AoeProjectileDamageEmpowered",
                chance,
            )
        {
            self.record_talent_proc("bowguy-talent-id-talent7");
            let current = if self.common.now_ms < self.hero.elarion().empowered_multishot_until {
                self.hero.elarion().empowered_multishot_stacks
            } else {
                0
            };
            self.hero.elarion_mut().empowered_multishot_stacks =
                current.saturating_add(stacks.max(1)).min(maximum);
            self.hero.elarion_mut().empowered_multishot_until =
                self.common.now_ms.saturating_add(duration);
            self.activate_fixed_buff(
                AplBuff::EmpoweredMultishot,
                self.hero.elarion().empowered_multishot_until,
            );
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::FocusedShot
                | DpsAbilityKind::HeartseekerBarrage
                | DpsAbilityKind::CelestialShot
                | DpsAbilityKind::Multishot
                | DpsAbilityKind::StarfallVolley
        ) && let Some((chance, duration, stacks)) = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent3")
            .map(|talent| {
                (
                    param(talent, parameter_key!("offensiveProcChance")),
                    ms_param(talent, parameter_key!("durationSeconds")),
                    param_u32(talent, parameter_key!("stacks")),
                )
            })
            && self.roll_controlled_random_bool(
                "RandomStream.Bowguy.Talent.CastedProjectileHeavyDamage.InstantNoCooldownProc",
                chance,
            )
        {
            self.record_talent_proc("bowguy-talent-id-talent3");
            self.activate_elarion_resurgent_winds(duration, stacks);
        }
    }

    pub(crate) fn apply_elarion_mark(&mut self, target_index: u32, stacks: u32) {
        let Some(mark) = self.ability(DpsAbilityKind::LunarlightMark).cloned() else {
            return;
        };
        let maximum = ability_u32_rounded(&mark, parameter_key!("maximumStacks"));
        let Some(current) = self
            .hero
            .elarion()
            .mark_stacks
            .get(target_index as usize)
            .copied()
        else {
            return;
        };
        let current = if self.common.now_ms < self.hero.elarion().mark_until[target_index as usize]
        {
            current
        } else {
            0
        };
        let state = self.hero.elarion_mut();
        state.mark_stacks[target_index as usize] = current.saturating_add(stacks).min(maximum);
        state.mark_until[target_index as usize] =
            self.common.now_ms.saturating_add(mark.effect_duration_ms);
        // The mark is itself a Harmful GE, even before it deals damage.
        self.notify_kindling_of_harmful_effect(
            Some(mark.kind),
            self.ability_damage_source(&mark),
            target_index,
            current > 0,
        );
    }

    pub(crate) fn activate_elarion_resurgent_winds(&mut self, duration_ms: u64, stacks: u32) {
        let maximum = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent3")
            .map(|talent| param_u32(talent, parameter_key!("maximumStacks")))
            .unwrap_or(1)
            .max(1);
        let current = if self.common.now_ms < self.hero.elarion().resurgent_winds_until {
            self.hero.elarion().resurgent_winds_stacks
        } else {
            0
        };
        self.hero.elarion_mut().resurgent_winds_stacks =
            current.saturating_add(stacks.max(1)).min(maximum);
        self.hero.elarion_mut().resurgent_winds_until =
            self.common.now_ms.saturating_add(duration_ms);
        self.restore_ability_charge(DpsAbilityKind::HighwindArrow);
        self.activate_fixed_buff(
            AplBuff::ResurgentWinds,
            self.hero.elarion().resurgent_winds_until,
        );
    }

    pub(crate) fn apply_elarion_strikers_aim(
        &mut self,
        ability: &PreparedAbility,
        targets_hit: u32,
    ) {
        let Some(talent) = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent11")
        else {
            return;
        };
        let missing_ricochets = ability.max_targets.saturating_sub(targets_hit);
        let stacks = missing_ricochets.min(param_u32(talent, parameter_key!("maximumStacks")));
        if stacks == 0 || stacks < self.hero.elarion().strikers_aim_stacks {
            return;
        }
        let decay_ms = ms_param(talent, parameter_key!("decayIntervalSeconds"));
        self.hero.elarion_mut().strikers_aim_stacks = stacks;
        self.hero.elarion_mut().strikers_aim_generation =
            self.hero.elarion().strikers_aim_generation.wrapping_add(1);
        self.hero.elarion_mut().strikers_aim_next_decay_ms =
            self.common.now_ms.saturating_add(decay_ms);
        self.push_event(
            self.hero.elarion().strikers_aim_next_decay_ms,
            ElarionEvent::StrikersAimDecay {
                generation: self.hero.elarion().strikers_aim_generation,
            },
        );
    }

    pub(crate) fn elarion_strikers_aim_decay(&mut self, generation: u64) {
        if generation != self.hero.elarion().strikers_aim_generation
            || self.common.now_ms != self.hero.elarion().strikers_aim_next_decay_ms
        {
            return;
        }
        self.hero.elarion_mut().strikers_aim_stacks =
            self.hero.elarion().strikers_aim_stacks.saturating_sub(1);
        if self.hero.elarion().strikers_aim_stacks == 0 {
            self.hero.elarion_mut().strikers_aim_next_decay_ms = 0;
            return;
        }
        let decay_ms = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent11")
            .map(|talent| ms_param(talent, parameter_key!("decayIntervalSeconds")))
            .unwrap_or(0);
        self.hero.elarion_mut().strikers_aim_next_decay_ms =
            self.common.now_ms.saturating_add(decay_ms);
        self.push_event(
            self.hero.elarion().strikers_aim_next_decay_ms,
            ElarionEvent::StrikersAimDecay { generation },
        );
    }

    pub(crate) fn try_elarion_spirit_refund(
        &mut self,
        ability: &CompiledAbility,
        focus_spent: f64,
    ) {
        if focus_spent <= 0.0 {
            return;
        }
        let chance = spirit_refund_chance(self.effective_spirit(), 1.0, 0.0);
        if !self.roll_controlled_random_bool(SPIRIT_PROC_RANDOM_STREAM_TAG, chance) {
            return;
        }
        self.record_spirit_refund_proc();
        if let Some(index) = self.profile.mechanic_indexes.elarion_starstrikers_ascent {
            let mechanic = Arc::clone(&self.profile.mechanics[index]);
            if self.roll_controlled_random_bool(
                "RandomStream.Bowguy.SpiritProcToInstantNoCooldownProc",
                mechanic_param(&mechanic, parameter_key!("starstrikerProcChance")),
            ) {
                self.record_dynamic_random_proc(&mechanic);
                self.common
                    .cooldowns
                    .remove(&DpsAbilityKind::HeartseekerBarrage);
                self.hero.elarion_mut().impending_heartseeker_until =
                    self.common.now_ms.saturating_add(seconds_parameter(
                        &mechanic,
                        parameter_key!("impendingHeartseekerDurationSeconds"),
                    ));
                self.activate_fixed_buff(
                    AplBuff::ImpendingHeartseeker,
                    self.hero.elarion().impending_heartseeker_until,
                );
            }
        }
        self.push_event(
            self.common.now_ms.saturating_add(ability_seconds_parameter(
                ability,
                parameter_key!("spiritRefundDelaySeconds"),
            )),
            ElarionEvent::ResourceRefund {
                amount_bits: focus_spent.to_bits(),
            },
        );
        self.shared.spirit = (self.shared.spirit
            + ability_param(ability, parameter_key!("spiritRefundSpiritGain")))
        .min(self.profile.max_spirit);
        self.apply_elarion_mark(
            0,
            ability_u32_rounded(ability, parameter_key!("spiritRefundMainTargetStacks")),
        );
        // The current Blueprint deliberately tests additional-target count > 1.
        if self.common.target_count > 2 {
            for target in 1..self.common.target_count.min(3) {
                self.apply_elarion_mark(
                    target,
                    ability_u32_rounded(
                        ability,
                        parameter_key!("spiritRefundAdditionalTargetStacks"),
                    ),
                );
            }
        }
    }

    pub(crate) fn resolve_elarion_impact(
        &mut self,
        ability: &CompiledAbility,
        target_index: u32,
        outcome: DamageOutcome,
    ) {
        if self.profile.contract.hero != HeroIdentity::Elarion {
            return;
        }
        if ability.kind == DpsAbilityKind::HighwindArrow
            && let Some(index) = self.profile.mechanic_indexes.elarion_shimmer
        {
            let mechanic = &self.profile.mechanics[index];
            let maximum = mechanic_u32(mechanic, parameter_key!("shimmerMaximumStacks"));
            let state = self.hero.elarion_mut();
            if state.shimmer_until[target_index as usize] <= self.common.now_ms {
                state.shimmer_stacks[target_index as usize] = 0;
            }
            state.shimmer_stacks[target_index as usize] = state.shimmer_stacks
                [target_index as usize]
                .saturating_add(1)
                .min(maximum);
            state.shimmer_until[target_index as usize] = self.common.now_ms.saturating_add(
                seconds_parameter(mechanic, parameter_key!("shimmerDurationSeconds")),
            );
        }
        // The projectile rolls after TargetReached without testing its damage result.
        if ability.kind == DpsAbilityKind::FocusedShot
            && target_index == 0
            && self.roll_proc_per_minute(
                "RandomStream.Bowguy.RangedAutoAttack.ProcChance",
                ability_param(ability, parameter_key!("celestialImpetusProcsPerMinute")),
                true,
            )
        {
            self.record_ability_proc(DpsAbilityKind::FocusedShot);
            let maximum =
                ability_u32_rounded(ability, parameter_key!("celestialImpetusMaximumStacks"));
            let current = if self.common.now_ms < self.hero.elarion().celestial_impetus_until {
                self.hero.elarion().celestial_impetus_stacks
            } else {
                0
            };
            self.hero.elarion_mut().celestial_impetus_stacks =
                current.saturating_add(1).min(maximum);
            self.hero.elarion_mut().celestial_impetus_until =
                self.common.now_ms.saturating_add(ability_seconds_parameter(
                    ability,
                    parameter_key!("celestialImpetusDurationSeconds"),
                ));
            self.activate_fixed_buff(
                AplBuff::CelestialImpetus,
                self.hero.elarion().celestial_impetus_until,
            );
        }
        if ability.kind == DpsAbilityKind::HighwindArrow {
            if let Some(reduction) = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent12")
                .map(|talent| ms_param(talent, parameter_key!("cooldownReductionSeconds")))
            {
                self.reduce_cooldown(DpsAbilityKind::HighwindArrow, reduction);
            }
            if target_index + 1
                == ability_u32_rounded(ability, parameter_key!("minimumTargetsForMultishotProc"))
                && let Some(multishot) = self.ability(DpsAbilityKind::Multishot)
            {
                let maximum = ability_u32_rounded(multishot, parameter_key!("maximumProcStacks"));
                self.hero.elarion_mut().multishot_proc_stacks = self
                    .hero
                    .elarion()
                    .multishot_proc_stacks
                    .saturating_add(1)
                    .min(maximum);
            }
        }
        if outcome.damage <= 0.0 {
            return;
        }
        if ability.kind == DpsAbilityKind::HighwindArrow
            && self.common.now_ms < self.hero.elarion().event_horizon_until
        {
            let reduction = self
                .ability(DpsAbilityKind::EventHorizon)
                .map(|event_horizon| {
                    ability_seconds_parameter(
                        event_horizon,
                        parameter_key!("heartseekerCooldownReductionPerHighwindHitSeconds"),
                    )
                })
                .unwrap_or(0);
            self.reduce_cooldown(DpsAbilityKind::HeartseekerBarrage, reduction);
        }
        if ability.kind == DpsAbilityKind::HeartseekerBarrage
            && target_index == 0
            && self.common.now_ms < self.hero.elarion().event_horizon_until
        {
            let reduction = self
                .ability(DpsAbilityKind::EventHorizon)
                .map(|event_horizon| {
                    ability_seconds_parameter(
                        event_horizon,
                        parameter_key!("starfallCooldownReductionPerHeartseekerHitSeconds"),
                    )
                })
                .unwrap_or(0);
            self.reduce_cooldown(DpsAbilityKind::StarfallVolley, reduction);
        }
    }

    pub(crate) fn try_elarion_mark_proc(
        &mut self,
        source: DamageSourceKey,
        target_index: u32,
        critical: bool,
        damage: f64,
        context: DamageContext,
    ) {
        if self.profile.contract.hero != HeroIdentity::Elarion {
            return;
        }
        let is_source = |iteration: &Self, kind| {
            iteration
                .ability(kind)
                .is_some_and(|ability| iteration.ability_damage_source(ability) == source)
        };
        let is_shoot = is_source(self, DpsAbilityKind::ElarionShoot);
        let is_salvo = is_source(self, DpsAbilityKind::LunarlightSalvo);
        let is_eruption = is_source(self, DpsAbilityKind::LunarlightEruption);
        let is_heartseeker = is_source(self, DpsAbilityKind::HeartseekerBarrage);
        let is_starfall = is_source(self, DpsAbilityKind::StarfallVolley);
        // The monitor computes its preservation flag before replacing its
        // cached Effect Asset Tags with this event's tags (bytecode 3261..3469).
        self.hero.elarion_mut().preserve_mark_stack =
            self.hero.elarion().previous_mark_event_was_starfall
                && self
                    .common
                    .selected_talents
                    .contains_key("bowguy-talent-id-talent10");
        self.hero.elarion_mut().previous_mark_event_was_starfall = is_starfall;
        if damage <= 0.0 || is_shoot || is_salvo || is_eruption {
            return;
        }
        // The native instance field is written before checking for a mark.
        // A nested gear hit can redirect the outer monitor continuation.
        self.hero.elarion_mut().cached_mark_target_index = target_index;
        if self
            .hero
            .elarion()
            .mark_stacks
            .get(target_index as usize)
            .copied()
            .unwrap_or(0)
            == 0
            || self
                .hero
                .elarion()
                .mark_until
                .get(target_index as usize)
                .copied()
                .unwrap_or(0)
                <= self.common.now_ms
        {
            return;
        }
        let Some(mark) = self.ability(DpsAbilityKind::LunarlightMark).cloned() else {
            return;
        };
        let mut chance = ability_param(
            &mark,
            if critical {
                parameter_key!("salvoCriticalProcChance")
            } else {
                parameter_key!("salvoProcChance")
            },
        );
        if is_heartseeker
            && let Some(talent) = self
                .common
                .selected_talents
                .get("bowguy-talent-id-talent18")
        {
            chance *= param(talent, parameter_key!("procChanceMultiplier"));
        }
        if !self.roll_controlled_random_bool(
            "RandomStream.Bowguy.IntantMarkTarget.TriggerAdditionalHit",
            chance,
        ) {
            return;
        }

        // Kismet evaluates both BooleanAND inputs: every successful Salvo
        // advances this stream, even when its triggering hit is not Heartseeker.
        let eruption_roll = self.roll_controlled_random_bool(
            "RandomStream.Bowguy.IntantMarkTarget.TriggerAoeHit",
            ability_param(&mark, parameter_key!("eruptionProcChance")),
        );
        let eruption = is_heartseeker && eruption_roll;
        let Some(salvo) = self.ability(DpsAbilityKind::LunarlightSalvo).cloned() else {
            return;
        };
        self.record_ability_proc(DpsAbilityKind::LunarlightSalvo);
        if eruption {
            self.record_ability_proc(DpsAbilityKind::LunarlightEruption);
        }
        let bonus_crit = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent10")
            .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
            .unwrap_or(0.0);
        let damage_multiplier = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent18")
            .map(|talent| param(talent, parameter_key!("damageMultiplier")))
            .unwrap_or(1.0);
        // Both specs are constructed before either is applied. Visual Delay
        // only sets ServerVisualizationTime; native application is synchronous.
        let salvo_context = context
            .as_proc()
            .with_snapshot(self.capture_damage_source_snapshot(salvo.kind));
        let eruption_context = context
            .as_proc()
            .with_snapshot(self.capture_damage_source_snapshot(DpsAbilityKind::LunarlightEruption));
        if eruption {
            self.apply_elarion_lunarlight_eruption(
                target_index,
                damage_multiplier,
                bonus_crit,
                eruption_context,
            );
        }
        self.damage_hit(
            &salvo,
            salvo.power_coefficient * self.profile.power * damage_multiplier,
            bonus_crit,
            true,
            self.hero.elarion().cached_mark_target_index,
            salvo_context,
        );
        // The shared Blueprint frame is re-entered by the immediate hits.
        // Even excluded Salvo/Eruption events update its preservation flag.
        // Read that flag after applying effects, as bytecode 2798 does.
        if !self.hero.elarion().preserve_mark_stack {
            let target_index = self.hero.elarion().cached_mark_target_index;
            let stacks = &mut self.hero.elarion_mut().mark_stacks[target_index as usize];
            *stacks = stacks.saturating_sub(1);
        }
    }

    fn apply_elarion_lunarlight_eruption(
        &mut self,
        target_index: u32,
        damage_multiplier: f64,
        bonus_crit: f64,
        context: DamageContext,
    ) {
        let Some(eruption) = self.ability(DpsAbilityKind::LunarlightEruption).cloned() else {
            return;
        };
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return;
        }
        let targets = self
            .common
            .actor_registration_order
            .enemy_targets()
            .iter()
            .copied()
            .filter(|candidate| *candidate != target_index)
            .take(eruption.max_targets as usize)
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return;
        }
        if !self.common.execution.charge_usize(targets.len()) {
            return;
        }
        let scaler = eruption
            .parameters
            .get(parameter_key!("targetCountDamageScalingThreshold"))
            .map(|threshold| multi_target_damage_falloff(targets.len() as u32, threshold))
            .unwrap_or(1.0);
        for current_target in targets {
            self.damage_hit(
                &eruption,
                eruption.power_coefficient * self.profile.power * damage_multiplier * scaler,
                bonus_crit,
                true,
                current_target,
                context,
            );
        }
    }
}
