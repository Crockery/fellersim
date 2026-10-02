use crate::*;

impl Iteration<'_> {
    pub(crate) fn resolve_impact(
        &mut self,
        index: usize,
        single_hit: bool,
        damage_scale: f64,
        target_index: u32,
        impact_context: CastImpactContext,
    ) {
        let mut context = impact_context.damage;
        let ability = self.profile.abilities[index].clone();
        match ability.kind {
            DpsAbilityKind::IceBlitz
            | DpsAbilityKind::WintersBlessing
            | DpsAbilityKind::FlightOfTheNavir => {
                self.apply_rime_ability_buff(ability.kind);
                return;
            }
            DpsAbilityKind::WrathOfWinter => {
                self.hero.rime_mut().wrath_of_winter_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::WrathOfWinter,
                    self.hero.rime().wrath_of_winter_until,
                );
                self.hero.rime_mut().wrath_generation =
                    self.hero.rime().wrath_generation.wrapping_add(1);
                self.hero.rime_mut().wrath_period_ms =
                    (ability_seconds_parameter(&ability, parameter_key!("volleyPeriodSeconds"))
                        as f64
                        / (1.0 + self.effective_haste()).max(0.05))
                    .round()
                    .max(1.0) as u64;
                self.push_event(
                    self.common.now_ms,
                    RimeEvent::WrathVolley {
                        generation: self.hero.rime().wrath_generation,
                        context,
                    },
                );
                return;
            }
            DpsAbilityKind::BurstingIce => {
                self.hero.rime_mut().bursting_generation =
                    self.hero.rime().bursting_generation.wrapping_add(1);
                self.hero.rime_mut().bursting_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                let generation = self.hero.rime().bursting_generation;
                let until = self.hero.rime().bursting_until;
                let now = self.common.now_ms;
                self.hero
                    .rime_mut()
                    .bursting_instances
                    .insert(generation, (until, now));
                self.push_event(
                    self.common
                        .now_ms
                        .saturating_add(ability_seconds_parameter(
                            &ability,
                            parameter_key!("pulsePeriodSeconds"),
                        ))
                        .min(until),
                    RimeEvent::BurstingPulse {
                        generation: self.hero.rime().bursting_generation,
                        context,
                    },
                );
                self.trigger_kindling_on_harmful_effect_application(
                    Some(ability.kind),
                    self.ability_damage_source(&ability),
                    target_index,
                );
                return;
            }
            DpsAbilityKind::ThunderCall => {
                self.hero.tariq_mut().thunder_call_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::ThunderCall,
                    self.hero.tariq().thunder_call_until,
                );
                return;
            }
            DpsAbilityKind::FocusedWrath => {
                let previous = if self.common.now_ms < self.hero.tariq().focused_wrath_until {
                    self.hero.tariq().focused_wrath_stacks
                } else {
                    0
                };
                self.hero.tariq_mut().focused_wrath_stacks = previous
                    .saturating_add(ability_u32_rounded(&ability, parameter_key!("stacks")))
                    .min(ability_u32_rounded(
                        &ability,
                        parameter_key!("maximumStacks"),
                    ));
                self.hero.tariq_mut().focused_wrath_until = self
                    .common
                    .now_ms
                    .saturating_add(ability.effect_duration_ms);
                self.activate_fixed_buff(
                    AplBuff::FocusedWrath,
                    self.hero.tariq().focused_wrath_until,
                );
                return;
            }
            DpsAbilityKind::SkystridersGrace
            | DpsAbilityKind::EventHorizon
            | DpsAbilityKind::SkystridersSupremacy
            | DpsAbilityKind::BroodingShadows
            | DpsAbilityKind::MaidenOfDeath
            | DpsAbilityKind::MatriarchMacabre
            | DpsAbilityKind::FinalStratagem => return,
            DpsAbilityKind::LunarlightMark => {
                let stacks = ability_u32_rounded(&ability, parameter_key!("stacksApplied"));
                self.apply_elarion_mark(target_index, stacks);
                if let Some((duration, resurgent_stacks)) = self
                    .common
                    .selected_talents
                    .get("bowguy-talent-id-talent3")
                    .map(|talent| {
                        (
                            ms_param(talent, parameter_key!("durationSeconds")),
                            param_u32(talent, parameter_key!("stacks")),
                        )
                    })
                {
                    self.activate_elarion_resurgent_winds(duration, resurgent_stacks);
                }
                return;
            }
            DpsAbilityKind::StarfallVolley => {
                if let Some(mut dot) = ability.dot {
                    if let Some(index) = self.profile.mechanic_indexes.elarion_astronomers_hail {
                        dot.duration_ms = dot.duration_ms.saturating_add(seconds_parameter(
                            &self.profile.mechanics[index],
                            parameter_key!("starfallDurationIncreaseSeconds"),
                        ));
                    }
                    self.apply_dot(target_index, ability.kind, &ability, dot, context);
                }
                return;
            }
            _ => {}
        }
        if ability.kind == DpsAbilityKind::Wildfire {
            let was_active = self.common.now_ms < self.hero.ardeos().wildfire_until;
            self.hero.ardeos_mut().wildfire_until = self
                .common
                .now_ms
                .saturating_add(ability.effect_duration_ms);
            self.activate_fixed_buff(AplBuff::Wildfire, self.hero.ardeos().wildfire_until);
            if !was_active {
                self.reschedule_dot_ticks();
            }
            self.push_event(
                self.hero.ardeos().wildfire_until,
                ArdeosEvent::WildfireExpire {
                    until_ms: self.hero.ardeos().wildfire_until,
                },
            );
            return;
        }
        if ability.kind == DpsAbilityKind::WeaponShadowMark {
            let duration_refreshed = self
                .shared
                .shadow_marks
                .get(&target_index)
                .is_some_and(|mark| mark.until_ms > self.common.now_ms);
            let generation = self.common.sequence.wrapping_add(1);
            let until_ms = self
                .common
                .now_ms
                .saturating_add(ability.effect_duration_ms);
            self.shared.shadow_marks.insert(
                target_index,
                ShadowMarkState {
                    generation,
                    until_ms,
                    accumulated_damage: 0.0,
                    context,
                },
            );
            self.push_event(
                until_ms,
                SharedEvent::WeaponShadowExplode {
                    generation,
                    target_index,
                },
            );
            // Shadow Mark applies its harmful accumulator debuff separately
            // from the eventual explosion damage.
            self.notify_kindling_of_harmful_effect(
                Some(ability.kind),
                self.ability_damage_source(&ability),
                target_index,
                duration_refreshed,
            );
            return;
        }
        if ability.kind == DpsAbilityKind::Detonate {
            let seconds = ability.power_coefficient;
            let target_count_scaler = self.target_count_damage_scaler(&ability);
            let hits_per_target =
                ability_u32_rounded(&ability, parameter_key!("hitsPerTarget")).max(1);
            let initial_delay_ms =
                ability_seconds_parameter(&ability, parameter_key!("initialDelaySeconds"));
            let per_target_delay_ms =
                ability_seconds_parameter(&ability, parameter_key!("perTargetHitDelaySeconds"));
            let between_hit_delay_ms =
                ability_seconds_parameter(&ability, parameter_key!("betweenHitDelaySeconds"));
            let mut target_delay_ms = initial_delay_ms;
            let mut did_detonate = false;
            let registered_targets = self
                .common
                .actor_registration_order
                .enemy_targets()
                .to_vec();
            if !self.charge_work_product(
                registered_targets.len() as u64,
                self.common.dots.len() as u64,
            ) {
                return;
            }
            for current_target in registered_targets {
                let detonate_base = self
                    .common
                    .dots
                    .iter()
                    .filter(|((target, kind), dot)| {
                        *target == current_target
                            && dot.expires_ms >= self.common.now_ms
                            && kind.is_ardeos_hero_dot()
                    })
                    .map(|((_, kind), dot)| {
                        self.approximate_dot_average_damage(*kind, dot, current_target) * seconds
                    })
                    .sum::<f64>();
                if detonate_base > 0.0 {
                    // A successful target acquisition first applies the
                    // harmful infinite controller debuff. Each of its three
                    // later damage effects supplies its own Kindling event.
                    self.trigger_kindling_on_harmful_effect_application(
                        Some(ability.kind),
                        self.ability_damage_source(&ability),
                        current_target,
                    );
                    did_detonate = true;
                    let damage_per_hit =
                        detonate_base * target_count_scaler / f64::from(hits_per_target);
                    if !self.charge_work_units(u64::from(hits_per_target)) {
                        return;
                    }
                    for hit in 0..hits_per_target {
                        self.push_event(
                            self.common
                                .now_ms
                                .saturating_add(target_delay_ms)
                                .saturating_add(
                                    between_hit_delay_ms.saturating_mul(u64::from(hit)),
                                ),
                            ArdeosEvent::DetonateHit {
                                index,
                                damage_bits: damage_per_hit.to_bits(),
                                target_index: current_target,
                                context,
                            },
                        );
                    }
                    target_delay_ms = target_delay_ms.saturating_add(per_target_delay_ms);
                }
                if let Some(talent) = self
                    .common
                    .selected_talents
                    .get("firemage-talent-id-talent10")
                {
                    self.increase_dot_duration(
                        current_target,
                        DotKind::Ability(DpsAbilityKind::SearingBlaze),
                        ms_param(talent, parameter_key!("extensionSeconds")),
                    );
                }
            }
            if did_detonate {
                self.try_reign_of_fire();
            }
            if !self
                .common
                .execution
                .charge_usize(self.profile.mechanic_indexes.intrepid.len())
            {
                return;
            }
            for index in &self.profile.mechanic_indexes.intrepid {
                self.shared.dynamic_counters[*index] = 0;
            }
            return;
        }
        if ability.kind == DpsAbilityKind::FreezingTorrent {
            self.try_rime_cold_shower(context);
        }
        let mut coefficient = ability.power_coefficient * damage_scale;
        if ability.kind == DpsAbilityKind::HeavyStrike && !self.tariq_in_hit_window() {
            coefficient *= ability_param(&ability, parameter_key!("weakDamageMultiplier"));
        }
        if ability.kind == DpsAbilityKind::FreezingTorrent && impact_context.bonus_crit > 0.0 {
            // The active Soulfrost branch multiplies by the live generic CritChance.
            coefficient *= 1.0 + self.effective_critical_strike(None);
        }
        let mut bonus_crit = impact_context.bonus_crit;
        if matches!(
            ability.kind,
            DpsAbilityKind::TariqChainLightning | DpsAbilityKind::RagingTempest
        ) && let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent16")
        {
            bonus_crit += param(talent, parameter_key!("lightningCriticalStrikeBonus"));
        }
        if ability.kind == DpsAbilityKind::HighwindArrow && target_index > 0 {
            coefficient *= ability_param(&ability, parameter_key!("bounceDamageMultiplier"));
        }
        if ability.kind == DpsAbilityKind::HeartseekerBarrage
            && target_index > 0
            && let Some(talent) = self.common.selected_talents.get("bowguy-talent-id-talent2")
        {
            coefficient *= param(talent, parameter_key!("damageMultiplier"));
        }
        let mut can_crit = true;
        if ability.kind == DpsAbilityKind::WildSwing {
            coefficient *= 1.0 + self.effective_critical_strike(None).max(0.0);
            can_crit = false;
        }
        if impact_context.frostweaver {
            bonus_crit += param(
                self.common
                    .selected_talents
                    .get("rime-talent-id-talent18")
                    .expect("selected Frostweaver talent"),
                parameter_key!("criticalStrikeBonus"),
            );
        }
        let mut cinders = ability.primary_resource_generated;
        if ability.kind == DpsAbilityKind::InfernalWave {
            if impact_context.ardeos_wave_empowered {
                cinders *= param(
                    self.common
                        .selected_talents
                        .get("firemage-talent-id-talent5")
                        .expect("selected Cascading Inferno talent"),
                    parameter_key!("cindersMultiplier"),
                );
            }
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent13")
            {
                bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
            }
        }
        if ability.kind == DpsAbilityKind::FireBall
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent16")
        {
            coefficient *= 1.0 + param(talent, parameter_key!("directDamageIncrease"));
        }
        if ability.kind == DpsAbilityKind::FireFrogs
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent6")
        {
            coefficient *= 1.0 + param(talent, parameter_key!("damageIncrease"));
        }
        coefficient *= self.target_count_damage_scaler(&ability);
        if ability.kind == DpsAbilityKind::FireFrogs {
            let mut frog_count = ability_u32_rounded(&ability, parameter_key!("frogCount"));
            let mut attacks_per_frog =
                ability_u32_rounded(&ability, parameter_key!("attacksPerFrog"));
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent6")
            {
                frog_count =
                    frog_count.saturating_add(param_u32(talent, parameter_key!("additionalFrogs")));
                attacks_per_frog = attacks_per_frog
                    .saturating_add(param_u32(talent, parameter_key!("additionalLeaps")));
            }
            self.execute_fire_frog_batch(
                &ability,
                FireFrogBatch {
                    main_target: target_index,
                    frog_count,
                    attacks_per_frog,
                    coefficient,
                    context,
                    allow_bonus_toad: true,
                },
            );
            return;
        }
        let hits = if single_hit {
            1
        } else {
            ability.direct_hits.max(1)
        };
        let mut last = DamageOutcome::default();
        let mut total_direct_damage = 0.0;
        for _ in 0..hits {
            last = self.damage_hit(
                &ability,
                coefficient * self.profile.power,
                bonus_crit,
                can_crit,
                target_index,
                context,
            );
            if ability.kind == DpsAbilityKind::InfernalWave
                && last.damage > 0.0
                && let Some(talent) = self
                    .common
                    .selected_talents
                    .get("firemage-talent-id-talent8")
            {
                self.reduce_cooldown(
                    DpsAbilityKind::EngulfingFlames,
                    ms_param(talent, parameter_key!("infernalWaveReductionSeconds")),
                );
            }
            if self.profile.contract.hero == HeroIdentity::Gunde && gunde_applies_rend(ability.kind)
            {
                self.try_gunde_spirit_refund(context);
            }

            if self.profile.contract.hero == HeroIdentity::Gunde
                && gunde_applies_rend(ability.kind)
                && ability.kind != DpsAbilityKind::BloodboundSpirit
            {
                self.hero.gunde_mut().crimson_strikes_until = 0;
                self.deactivate_fixed_buff(AplBuff::CrimsonStrikes);
            }
            total_direct_damage += last.damage;
            context = context.without_cast_proc();
            if ability.kind == DpsAbilityKind::DoubleStrike {
                // The second BP_FellowshipApplyGameplayEffect creates a fresh
                // spec after the first application's listeners have run.
                context.source_snapshot = Some(self.capture_damage_source_snapshot(ability.kind));
            }
            if self.common.execution.failed() {
                return;
            }
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::GlacialBlast | DpsAbilityKind::IceComet
        ) && target_index == 0
        {
            self.consume_rime_frostweaver();
        }
        if ability.kind == DpsAbilityKind::FrostBolt
            && let Some(chance) = self
                .common
                .selected_talents
                .get("rime-talent-id-talent2")
                .map(|talent| param(talent, parameter_key!("procChance")))
            && self.roll_controlled_random_bool(
                "RandomStream.Rime.Talent.CastedDebuffAoeDamage.TriggerRandomTick",
                chance,
            )
        {
            self.push_event(
                self.common.now_ms.saturating_add(100),
                RimeEvent::BurstingTriggered {
                    context: context.as_proc(),
                },
            );
        }
        if ability.kind == DpsAbilityKind::ColdSnap && target_index == 0 {
            self.add_rime_orbs(ability_u32_rounded(&ability, parameter_key!("orbGain")));
            self.fire_rime_anima_volley(3, 0, context);
            self.command_rime_frost_swallows(
                ability_u32_rounded(
                    self.ability(DpsAbilityKind::FlightOfTheNavir)
                        .expect("Rime profile includes Flight of the Navir"),
                    parameter_key!("projectileCount"),
                ),
                0,
                context,
            );
        }
        if ability.kind == DpsAbilityKind::FreezingTorrent && target_index == 0 {
            self.on_rime_torrent_tick(last.critical, target_index, context);
        }
        if ability.kind == DpsAbilityKind::GlacialBlast
            && target_index == 0
            && impact_context.glacial_assault
        {
            self.rime_glacial_assault_explosion(total_direct_damage, context);
        }
        let debuff_refreshed = match ability.kind {
            DpsAbilityKind::HighwindArrow => {
                self.hero.elarion().shimmer_until[target_index as usize] > self.common.now_ms
            }
            DpsAbilityKind::LeapSmash => {
                self.hero.tariq().slayers_mosh_until[target_index as usize] > self.common.now_ms
            }
            DpsAbilityKind::Rupture => {
                self.hero.gunde().open_wounds_until[target_index as usize] > self.common.now_ms
            }
            _ => false,
        };
        if ability.kind == DpsAbilityKind::WeaponCleaveCharge && target_index == 0 {
            // DoImpact applies the primary spec, creates a separate cleave
            // spec, then grants the owner buff. Never copy resolved primary
            // damage: each secondary target resolves its own crit and spread.
            let additional = self.common.target_count.saturating_sub(1);
            let falloff = multi_target_damage_falloff(
                additional.max(1),
                ability_param(
                    &ability,
                    parameter_key!("cleaveTargetCountDamageScalingThreshold"),
                ),
            );
            let cleave_context = context
                .without_cast_proc()
                .with_snapshot(self.capture_damage_source_snapshot(ability.kind));
            if !self.charge_work_units(u64::from(additional)) {
                return;
            }
            for target in 1..self.common.target_count {
                self.damage_hit(
                    &ability,
                    ability.power_coefficient
                        * self.profile.power
                        * ability_param(&ability, parameter_key!("cleaveDamageMultiplier"))
                        * falloff,
                    0.0,
                    true,
                    target,
                    cleave_context,
                );
            }
            self.shared.weapon_charge_buff_until = self.common.now_ms.saturating_add(
                ability_seconds_parameter(&ability, parameter_key!("buffDurationSeconds")),
            );
            // WaitDelayCR follows DoImpact and blocks the next action even if
            // this buff's cooldown recovery finishes the GCD first.
            let rate = if ability.scale_time_with_haste {
                (1.0 + self.effective_haste()).max(0.05)
            } else {
                1.0
            };
            self.shared.weapon_charge_lock_until = self.common.now_ms.saturating_add(
                (ability_seconds_parameter(&ability, parameter_key!("postHitDelaySeconds")) as f64
                    / rate)
                    .round()
                    .max(0.0) as u64,
            );
        }
        if self.profile.contract.hero == HeroIdentity::Gunde
            && ability.kind == DpsAbilityKind::WeaponCleaveCharge
        {
            self.hero.gunde_mut().auto_blocked_until = self.shared.weapon_charge_lock_until;
        }
        self.resolve_tariq_impact(
            &ability,
            target_index,
            total_direct_damage,
            last.critical,
            context,
        );
        self.resolve_elarion_impact(&ability, target_index, last);
        if ability.kind == DpsAbilityKind::CelestialShot && impact_context.elarion_celestial_impetus
        {
            // The projectile applies its mark after damage and synchronous damage listeners.
            let stacks = self
                .ability(DpsAbilityKind::LunarlightMark)
                .map(|mark| ability_u32_rounded(mark, parameter_key!("stacksApplied")))
                .unwrap_or(0);
            self.apply_elarion_mark(target_index, stacks);
        }
        self.resolve_mara_impact(
            &ability,
            target_index,
            last,
            total_direct_damage,
            impact_context,
            context,
        );
        self.resolve_gunde_impact(
            &ability,
            target_index,
            last,
            total_direct_damage,
            impact_context,
            context,
        );
        if ability.kind == DpsAbilityKind::BloodArc
            && target_index + 1 == self.automatic_target_count(&ability)
        {
            self.finish_gunde_blood_arc(&ability);
        }
        // Separate target-side effects also apply when the direct hit rounds
        // to zero. Their source eligibility is independent of damage magnitude.
        self.trigger_kindling_on_hero_debuffs(&ability, target_index, debuff_refreshed);
        if ability.kind == DpsAbilityKind::Rupture
            && target_index + 1 == ability.max_targets.min(self.common.target_count)
        {
            self.finish_gunde_rupture();
        }
        if ability.kind == DpsAbilityKind::InfernalWave {
            self.try_flare_up(last.damage, target_index, context);
        }
        self.try_firemage_resource_reward(
            Some(ability.kind),
            ability
                .parameters
                .get(parameter_key!("resourceProcChance"))
                .unwrap_or(0.0),
            ability
                .parameters
                .get(parameter_key!("cindersOnResourceProc"))
                .unwrap_or(0.0),
        );
        if target_index == 0 {
            match self.profile.contract.hero {
                HeroIdentity::Rime if ability.kind != DpsAbilityKind::FrostBolt => {
                    self.add_rime_anima(cinders, context)
                }
                HeroIdentity::Rime => {}
                HeroIdentity::Tariq
                    if !matches!(
                        ability.kind,
                        DpsAbilityKind::WildSwing
                            | DpsAbilityKind::FaceBreaker
                            | DpsAbilityKind::HeavyStrike
                            | DpsAbilityKind::TariqAttack
                            | DpsAbilityKind::LeapSmash
                    ) =>
                {
                    self.add_tariq_fury(cinders)
                }
                HeroIdentity::Tariq => {}
                HeroIdentity::Ardeos => self.add_cinders(cinders),
                HeroIdentity::Elarion if ability.kind != DpsAbilityKind::FocusedShot => {
                    self.hero.elarion_mut().focus = (self.hero.elarion().focus + cinders)
                        .min(self.profile.max_primary_resource);
                }
                HeroIdentity::Elarion => {}
                HeroIdentity::Mara => {}
                HeroIdentity::Gunde => {}
            }
        }
        if self.common.execution.failed() {
            return;
        }
        if ability.kind == DpsAbilityKind::InfernalWave
            && last.critical
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent13")
        {
            let total = last.damage * param(talent, parameter_key!("burnDamageFraction"));
            let model = DotModel {
                power_coefficient: 0.0,
                damage_spread: 0.0,
                duration_ms: ms_param(talent, parameter_key!("burnDurationSeconds")).max(1),
                period_ms: ms_param(talent, parameter_key!("burnTickPeriodSeconds")).max(1),
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            };
            self.apply_damage_derived_dot(
                target_index,
                DotKind::CracklingInferno,
                self.damage_source_key("talent:crackling-inferno"),
                model,
                total,
                context,
            );
        }
        if ability.kind == DpsAbilityKind::Apocalypse {
            let mechanic_count = self.profile.mechanic_indexes.apocalypse_dot.len();
            if !self.common.execution.charge_usize(mechanic_count) {
                return;
            }
            for position in 0..mechanic_count {
                let index = self.profile.mechanic_indexes.apocalypse_dot[position];
                let mechanic = Arc::clone(&self.profile.mechanics[index]);
                let model = DotModel {
                    power_coefficient: 0.0,
                    damage_spread: 0.0,
                    duration_ms: seconds_parameter(
                        &mechanic,
                        parameter_key!("apocalypseDotDurationSeconds"),
                    )
                    .max(1),
                    period_ms: seconds_parameter(
                        &mechanic,
                        parameter_key!("apocalypseDotPeriodSeconds"),
                    )
                    .max(1),
                    can_crit: true,
                    cinder_proc_chance: 0.0,
                    cinders_on_proc: 0.0,
                    stack_damage_increase: 0.0,
                    maximum_stacks: 1,
                };
                self.apply_damage_derived_dot(
                    target_index,
                    DotKind::ApocalypseBurn,
                    mechanic.damage_source,
                    model,
                    last.damage
                        * mechanic_param(&mechanic, parameter_key!("apocalypseDotFraction")),
                    context,
                );
            }
        }
        if let Some(dot) = ability.dot
            && !matches!(
                ability.kind,
                DpsAbilityKind::FireBall
                    | DpsAbilityKind::FireFrogs
                    | DpsAbilityKind::WeaponFrostVolley
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::Slaughter
            )
        {
            self.apply_dot(target_index, ability.kind, &ability, dot, context);
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::FireBall | DpsAbilityKind::FireFrogs
        ) && let Some(dot) = ability.dot
        {
            let transfer = ability_param(&ability, parameter_key!("damageToDotTransferFraction"))
                + if ability.kind == DpsAbilityKind::FireBall {
                    self.common
                        .selected_talents
                        .get("firemage-talent-id-talent16")
                        .map(|talent| param(talent, parameter_key!("additionalDotFraction")))
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
            self.apply_damage_derived_dot(
                target_index,
                DotKind::Ability(ability.kind),
                self.ability_damage_source(&ability),
                dot,
                total_direct_damage * transfer,
                context,
            );
        }
        if ability.applies_dot_kind.is_some() {
            if ability.dot_application_delay_ms > 0 {
                self.push_event(
                    self.common
                        .now_ms
                        .saturating_add(ability.dot_application_delay_ms),
                    CoreEvent::ApplyLinkedDot {
                        index,
                        targets: vec![target_index],
                        context,
                    },
                );
            } else {
                self.apply_linked_dot(&ability, target_index, context);
            }
        }
        if ability.dot_extension_ms > 0 {
            // Incinerate applies its direct and stacking DoT effects first,
            // then extends every active outgoing Firemage DoT by no more than
            // the elapsed portion of that effect's total duration.
            if !self.common.execution.charge_usize(self.common.dots.len()) {
                return;
            }
            let affected = self
                .common
                .dots
                .iter()
                .filter(|((target, kind), dot)| {
                    *target == target_index
                        && kind.is_ardeos_hero_dot()
                        && dot.expires_ms >= self.common.now_ms
                })
                .map(|((_, kind), _)| *kind)
                .collect::<Vec<_>>();
            if !self.common.execution.charge_usize(affected.len()) {
                return;
            }
            for kind in affected {
                self.increase_dot_duration_up_to_total(
                    target_index,
                    kind,
                    ability.dot_extension_ms,
                );
            }
        }
    }
}
