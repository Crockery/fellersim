use crate::*;

impl Iteration<'_> {
    pub(crate) fn prepare_cast(&mut self, index: usize) {
        let mut ability = PreparedAbility::new(Arc::clone(&self.profile.abilities[index]));
        self.common.cast_sequence = self.common.cast_sequence.wrapping_add(1);
        let cast_id = self.common.cast_sequence;
        let mut context = DamageContext::for_cast(cast_id);
        let mut free_charge = false;
        let mut bonus_crit = 0.0;
        let mut glacial_assault_cast = false;
        let mut elarion_multishot_arrows = 0;
        let mut elarion_focus_cost = 0.0;
        let mut elarion_celestial_impetus = false;
        let mut impending_heartseeker = false;
        let mut mara_combo_points_spent = 0;
        let mut mara_copy_damage_multiplier = 1.0;
        let mut mara_from_stealth = false;
        let mut gunde_rend_transfer_bonus = 0.0;
        if self.profile.contract.hero == HeroIdentity::Gunde {
            if ability.kind == DpsAbilityKind::HeartSplitter {
                if let Some(talent) = self.common.selected_talents.get("gunde-talent-id-talent6") {
                    context
                        .multiply_damage(param(talent, parameter_key!("directDamageMultiplier")));
                }
                bonus_crit += self
                    .common
                    .selected_talents
                    .get("gunde-talent-id-talent14")
                    .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
                    .unwrap_or(0.0);
                bonus_crit += self
                    .gunde_mechanic_param(parameter_key!("heartSplitterHealthyCriticalStrikeBonus"))
                    .unwrap_or(0.0);
            }
            if ability.kind == DpsAbilityKind::GrimCarve
                && self.common.now_ms < self.hero.gunde().grim_harvest_until
            {
                bonus_crit += self
                    .common
                    .selected_talents
                    .get("gunde-talent-id-talent3")
                    .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
                    .unwrap_or(0.0);
                self.hero.gunde_mut().grim_harvest_until = 0;
                self.deactivate_fixed_buff(AplBuff::GrimHarvest);
            }
            if ability.kind == DpsAbilityKind::BloodArc
                && self.common.now_ms < self.hero.gunde().deaths_arc_until
            {
                bonus_crit += self
                    .common
                    .selected_talents
                    .get("gunde-talent-id-talent1")
                    .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
                    .unwrap_or(0.0);
                self.hero.gunde_mut().deaths_arc_until = 0;
                self.deactivate_fixed_buff(AplBuff::DeathsArc);
            }
            if ability.kind == DpsAbilityKind::BloodArc
                && self.common.now_ms < self.hero.gunde().carrion_onslaught_until
            {
                context.multiply_damage(self.hero.gunde().carrion_damage_multiplier);
                self.hero.gunde_mut().carrion_onslaught_until = 0;
                self.hero.gunde_mut().carrion_damage_multiplier = 1.0;
                self.deactivate_fixed_buff(AplBuff::CarrionOnslaught);
            } else if ability.kind == DpsAbilityKind::BloodArc {
                self.hero.gunde_mut().carrion_pending_feathers = 0;
                self.hero.gunde_mut().carrion_damage_multiplier = 1.0;
            }
            if gunde_applies_rend(ability.kind) {
                let consume_self_buffs = ability.kind != DpsAbilityKind::BloodboundSpirit;
                if consume_self_buffs && self.common.now_ms < self.hero.gunde().serrated_edge_until
                {
                    gunde_rend_transfer_bonus += self
                        .ability(DpsAbilityKind::BloodArc)
                        .map(|model| {
                            ability_param(model, parameter_key!("serratedEdgeTransferFraction"))
                        })
                        .unwrap_or(0.0);
                    gunde_rend_transfer_bonus += self
                        .common
                        .selected_talents
                        .get("gunde-talent-id-talent16")
                        .map(|talent| param(talent, parameter_key!("addedRendTransferFraction")))
                        .unwrap_or(0.0);
                    self.hero.gunde_mut().serrated_edge_until = 0;
                    self.deactivate_fixed_buff(AplBuff::SerratedEdge);
                }
            }
        }
        if self.profile.contract.hero == HeroIdentity::Mara {
            let is_attack = matches!(
                ability.kind,
                DpsAbilityKind::SkitteringBlades
                    | DpsAbilityKind::ArachnidAssault
                    | DpsAbilityKind::MaraAttack
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::WidowsBite
                    | DpsAbilityKind::QueensFang
                    | DpsAbilityKind::Backstab
            );
            mara_from_stealth = is_attack && self.hero.mara().stealth_active;
            if mara_from_stealth
                && matches!(
                    ability.kind,
                    DpsAbilityKind::Backstab | DpsAbilityKind::SkitteringBlades
                )
            {
                // Brooding's manager replaces these Basic classes with Core
                // variants. Retain that class through every delayed target hit.
                context.category_override = Some(AbilityCategory::Core);
            }

            let is_spender = matches!(
                ability.kind,
                DpsAbilityKind::ArachnidAssault
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::QueensFang
            );
            // Commit pays Energy before UseAllComboPoints broadcasts its
            // secondary-resource decrease to Efficient Killer and Sinner's Pride.
            let cost =
                ability_param(&ability, parameter_key!("energyCost")).min(self.hero.mara().energy);
            self.hero.mara_mut().energy = (self.hero.mara().energy - cost).max(0.0);
            self.accumulate_mara_deadly_scheme(cost);
            if is_spender {
                mara_combo_points_spent = std::mem::take(&mut self.hero.mara_mut().combo_points);
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent6") {
                    let energy = f64::from(mara_combo_points_spent)
                        * param(talent, parameter_key!("energyPerComboPoint"));
                    self.gain_mara_energy(energy);
                }
                if let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent12") {
                    let reduction = u64::from(mara_combo_points_spent).saturating_mul(ms_param(
                        talent,
                        parameter_key!("cooldownReductionPerComboPointSeconds"),
                    ));
                    self.reduce_cooldown(DpsAbilityKind::MaidenOfDeath, reduction);
                }
            }

            if is_spender {
                self.try_mara_spirit_refund(cost, mara_combo_points_spent);
                self.try_mara_corrosive_spill(mara_combo_points_spent, context);
            }

            let energy_gain = ability_param(&ability, parameter_key!("energyGain"));
            if energy_gain > 0.0 {
                self.hero.mara_mut().energy =
                    (self.hero.mara().energy + energy_gain).min(self.profile.max_primary_resource);
            }

            if matches!(
                ability.kind,
                DpsAbilityKind::ArachnidAssault | DpsAbilityKind::QueensFang
            ) {
                context.multiply_damage(
                    1.0 + f64::from(mara_combo_points_spent)
                        * ability_param(&ability, parameter_key!("damageMultiplierPerComboPoint")),
                );
                if self.common.now_ms < self.hero.mara().assassins_guile_until {
                    context.multiply_damage(
                        self.common
                            .selected_talents
                            .get("mara-talent-id-talent13")
                            .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                            .unwrap_or(1.0),
                    );
                }
                if self.common.now_ms < self.hero.mara().deadly_scheme_until {
                    bonus_crit += self
                        .common
                        .selected_talents
                        .get("mara-talent-id-talent3")
                        .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
                        .unwrap_or(0.0);
                }
            }
            if ability.kind == DpsAbilityKind::QueensFang
                && self.common.now_ms < self.hero.mara().feed_the_queen_until
            {
                let stacks = self.hero.mara().feed_the_queen_stacks;
                let per_stack = self
                    .common
                    .selected_talents
                    .get("mara-talent-id-talent15")
                    .map(|talent| param(talent, parameter_key!("damageIncreasePerStack")))
                    .unwrap_or(0.0);
                let feed_multiplier = 1.0 + f64::from(stacks) * per_stack;
                context.multiply_damage(feed_multiplier);
                // The cloned spec keeps its set-by-caller value, but both
                // copy tags exclude this Feed the Queen modifier.
                mara_copy_damage_multiplier /= feed_multiplier;
            }
            let malevolence_active = match ability.kind {
                DpsAbilityKind::QueensFang => {
                    self.hero.mara().malevolence_queen_stacks > 0
                        && self.common.now_ms < self.hero.mara().malevolence_queen_until
                }
                DpsAbilityKind::ArachnidAssault => {
                    self.hero.mara().malevolence_arachnid_stacks > 0
                        && self.common.now_ms < self.hero.mara().malevolence_arachnid_until
                }
                _ => false,
            };
            if malevolence_active
                && let Some(talent) = self.common.selected_talents.get("mara-talent-id-talent2")
            {
                context.multiply_damage(param(talent, parameter_key!("damageMultiplier")));
            }
            if ability.kind == DpsAbilityKind::Backstab {
                context.multiply_damage(ability_param(
                    &ability,
                    parameter_key!("behindDamageMultiplier"),
                ));
            }
            if ability.kind == DpsAbilityKind::WidowsBite
                && self
                    .common
                    .selected_talents
                    .contains_key("mara-talent-id-talent17")
            {
                bonus_crit += self
                    .common
                    .selected_talents
                    .get("mara-talent-id-talent17")
                    .map(|talent| param(talent, parameter_key!("criticalStrikeBonus")))
                    .unwrap_or(0.0);
            }
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::Multishot
                | DpsAbilityKind::HighwindArrow
                | DpsAbilityKind::HeartseekerBarrage
                | DpsAbilityKind::CelestialShot
                | DpsAbilityKind::StarfallVolley
        ) {
            let mut cost = ability_param(&ability, parameter_key!("focusCost"));
            if self.common.now_ms < self.hero.elarion().event_horizon_until {
                cost *= self
                    .ability(DpsAbilityKind::EventHorizon)
                    .map(|event_horizon| {
                        ability_param(event_horizon, parameter_key!("focusCostMultiplier"))
                    })
                    .unwrap_or(1.0);
            }
            if ability.kind == DpsAbilityKind::CelestialShot
                && self.hero.elarion().celestial_impetus_stacks > 0
                && self.common.now_ms < self.hero.elarion().celestial_impetus_until
            {
                elarion_celestial_impetus = true;
                self.hero.elarion_mut().celestial_impetus_stacks -= 1;
                if self.hero.elarion().celestial_impetus_stacks == 0 {
                    self.hero.elarion_mut().celestial_impetus_until = 0;
                    self.deactivate_fixed_buff(AplBuff::CelestialImpetus);
                }
                cost = 0.0;
            }
            if ability.kind == DpsAbilityKind::HighwindArrow
                && self.hero.elarion().resurgent_winds_stacks > 0
                && self.common.now_ms < self.hero.elarion().resurgent_winds_until
            {
                self.hero.elarion_mut().resurgent_winds_stacks -= 1;
                if self.hero.elarion().resurgent_winds_stacks == 0 {
                    self.hero.elarion_mut().resurgent_winds_until = 0;
                    self.deactivate_fixed_buff(AplBuff::ResurgentWinds);
                }
                ability.cast_time_ms = 0;
                free_charge = true;
                cost = 0.0;
                context.multiply_damage(ability_param(
                    &ability,
                    parameter_key!("resurgentDamageMultiplier"),
                ));
            }
            if ability.kind == DpsAbilityKind::Multishot {
                if self.hero.elarion().multishot_proc_stacks > 0 {
                    self.hero.elarion_mut().multishot_proc_stacks -= 1;
                    context.multiply_damage(ability_param(
                        &ability,
                        parameter_key!("procDamageMultiplier"),
                    ));
                }
                let supremacy =
                    self.common.now_ms < self.hero.elarion().skystriders_supremacy_until;
                let focused_expanse = self.hero.elarion().empowered_multishot_stacks > 0
                    && self.common.now_ms < self.hero.elarion().empowered_multishot_until;
                if supremacy || focused_expanse {
                    cost *= ability_param(&ability, parameter_key!("empoweredCostMultiplier"));
                    elarion_multishot_arrows = ability_u32_rounded(
                        &ability,
                        parameter_key!("empoweredMinimumProjectiles"),
                    )
                    .max(self.common.target_count.min(ability.max_targets));
                    if focused_expanse {
                        context.multiply_damage(
                            self.common
                                .selected_talents
                                .get("bowguy-talent-id-talent7")
                                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                                .unwrap_or(1.0),
                        );
                    }
                    if supremacy
                        && self
                            .common
                            .selected_talents
                            .contains_key("bowguy-talent-id-talent13")
                    {
                        context.multiply_damage(
                            self.common
                                .selected_talents
                                .get("bowguy-talent-id-talent13")
                                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                                .unwrap_or(1.0),
                        );
                    }
                }
            }
            if ability.kind == DpsAbilityKind::HeartseekerBarrage
                && self.common.now_ms < self.hero.elarion().impending_heartseeker_until
            {
                impending_heartseeker = true;
                self.hero.elarion_mut().impending_heartseeker_until = 0;
                self.deactivate_fixed_buff(AplBuff::ImpendingHeartseeker);
            }
            if ability.kind == DpsAbilityKind::HighwindArrow
                && (self.hero.elarion().highwind_casts + 1).is_multiple_of(3)
                && let Some(talent) = self
                    .common
                    .selected_talents
                    .get("bowguy-talent-id-talent15")
            {
                context.multiply_damage(param(talent, parameter_key!("damageMultiplier")));
                ability.max_targets = param_u32(talent, parameter_key!("maximumBounces"));
            }
            elarion_focus_cost = cost;
        }
        if ability.kind == DpsAbilityKind::HeartseekerBarrage
            && let Some(talent) = self.common.selected_talents.get("bowguy-talent-id-talent2")
        {
            ability.max_targets = 1 + param_u32(talent, parameter_key!("additionalTargets"));
        }
        if ability.kind == DpsAbilityKind::HeartseekerBarrage
            && let Some(talent) = self.common.selected_talents.get("bowguy-talent-id-talent5")
            && let Some(channel) = &mut ability.channel
        {
            channel.duration_ms = channel
                .duration_ms
                .saturating_add(ms_param(talent, parameter_key!("durationIncreaseSeconds")));
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::FocusedShot | DpsAbilityKind::CelestialShot
        ) && let Some(talent) = self
            .common
            .selected_talents
            .get("bowguy-talent-id-talent17")
        {
            context.multiply_damage(param(talent, parameter_key!("damageMultiplier")));
        }
        if ability.kind == DpsAbilityKind::Detonate {
            if self.hero.ardeos().apocalyptic_surge > 0
                && self.common.now_ms < self.hero.ardeos().apocalyptic_surge_until
            {
                self.hero.ardeos_mut().apocalyptic_surge -= 1;
            } else {
                self.hero.ardeos_mut().apocalyptic_surge = 0;
                self.hero.ardeos_mut().embers = self
                    .hero
                    .ardeos()
                    .embers
                    .saturating_sub(ability.secondary_resource_cost);
                self.try_firemage_spirit_refund(&ability);
            }
            if let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent5")
            {
                let maximum_stacks = param_u32(talent, parameter_key!("maximumStacks"));
                self.hero.ardeos_mut().cascading_stacks = self
                    .hero
                    .ardeos()
                    .cascading_stacks
                    .saturating_add(1)
                    .min(maximum_stacks);
            }
        }
        let cast_reduction_ms = if ability.kind == DpsAbilityKind::Apocalypse {
            self.common
                .selected_talents
                .get("firemage-talent-id-talent14")
                .map(|talent| ms_param(talent, parameter_key!("castTimeReductionSeconds")))
                .unwrap_or(0)
        } else {
            0
        };
        let mut ardeos_wave_empowered = false;
        if ability.kind == DpsAbilityKind::InfernalWave
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent5")
        {
            let threshold = param_u32(talent, parameter_key!("stacksThreshold")).max(1);
            if self.hero.ardeos().cascading_stacks >= threshold {
                ability.cast_time_ms = 0;
                ardeos_wave_empowered = true;
            }
        }
        let mut rime_talon_spikes = 0;
        if ability.kind == DpsAbilityKind::ColdSnap
            && self.hero.rime().navir_free_cold_snaps > 0
            && self.common.now_ms < self.hero.rime().navir_free_until
        {
            self.hero.rime_mut().navir_free_cold_snaps -= 1;
            free_charge = true;
        }
        if ability.kind == DpsAbilityKind::ColdSnap {
            let stacks = if self.common.now_ms < self.hero.rime().frostwyrm_until {
                self.hero.rime().frostwyrm_stacks
            } else {
                0
            };
            if stacks > 0
                && let Some(mechanic) =
                    self.rime_legendary_mechanic(LegendaryHeroSourceKind::Frostwyrm)
            {
                ability.max_targets = 1_u32
                    .saturating_add(stacks.saturating_mul(mechanic_u32(
                        &mechanic,
                        parameter_key!("frostwyrmTargetsPerStack"),
                    )))
                    .min(self.common.target_count);
                context.multiply_damage(
                    1.0 + f64::from(stacks)
                        * mechanic_param(
                            &mechanic,
                            parameter_key!("frostwyrmDamageIncreasePerStack"),
                        ),
                );
                self.hero.rime_mut().frostwyrm_stacks = 0;
                self.hero.rime_mut().frostwyrm_until = 0;
            }
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::GlacialBlast | DpsAbilityKind::IceComet
        ) {
            if ability.kind == DpsAbilityKind::GlacialBlast
                && let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent14")
            {
                let damage_multiplier = param(talent, parameter_key!("damageMultiplier"));
                let cast_time_increase_ms =
                    ms_param(talent, parameter_key!("castTimeIncreaseSeconds"));
                context.multiply_damage(damage_multiplier);
                ability.cast_time_ms = ability.cast_time_ms.saturating_add(cast_time_increase_ms);
            }
            let glacial_assault = ability.kind == DpsAbilityKind::GlacialBlast
                && self
                    .common
                    .selected_talents
                    .get("rime-talent-id-talent1")
                    .is_some_and(|talent| {
                        self.hero.rime().glacial_assault_stacks
                            >= param_u32(talent, parameter_key!("maximumStacks"))
                    });
            glacial_assault_cast = glacial_assault;
            let swapped = self
                .common
                .selected_talents
                .contains_key("rime-talent-id-talent17");
            if swapped {
                rime_talon_spikes = if glacial_assault {
                    self.profile.max_secondary_resource
                } else {
                    self.hero.rime().winter_orbs
                };
                ability.cast_time_ms = 0;
            } else if glacial_assault
                || (ability.kind == DpsAbilityKind::GlacialBlast
                    && self.common.now_ms < self.hero.rime().wrath_of_winter_until)
            {
                ability.cast_time_ms = 0;
            } else if ability.kind == DpsAbilityKind::GlacialBlast
                && self.buff_stacks(AplBuff::IcyFlow) > 0
            {
                // The graph grants an additive Haste effect until ability end.
                self.hero.rime_mut().icy_flow_casting_haste = param(
                    self.common
                        .selected_talents
                        .get("rime-talent-id-talent5")
                        .unwrap(),
                    parameter_key!("castHaste"),
                );
            }
        }

        if ability.kind == DpsAbilityKind::FreezingTorrent {
            if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent12") {
                let damage_multiplier = param(talent, parameter_key!("damageMultiplier"));
                let duration_increase_ms =
                    ms_param(talent, parameter_key!("durationIncreaseSeconds"));
                context.multiply_damage(damage_multiplier);
                if let Some(channel) = &mut ability.channel {
                    channel.duration_ms = channel.duration_ms.saturating_add(duration_increase_ms);
                    // Supreme writes DamageScale = 1.2, replacing the partial factor.
                    channel.scale_partial_tick_damage = false;
                }
            }
            if self.common.now_ms < self.hero.rime().soulfrost_torrent_until {
                let talent = self
                    .common
                    .selected_talents
                    .get("rime-talent-id-talent9")
                    .expect("selected Soulfrost talent");
                if let Some(channel) = &mut ability.channel {
                    channel.tick_interval_ms = ((channel.tick_interval_ms as f64)
                        / param(talent, parameter_key!("tickRateMultiplier")).max(0.05))
                    .round()
                    .max(1.0) as u64;
                }
                bonus_crit = param(talent, parameter_key!("criticalStrikeBonus"));
                self.hero.rime_mut().soulfrost_torrent_until = 0;
                self.deactivate_fixed_buff(AplBuff::SoulfrostTorrent);
            }
        }
        if matches!(
            ability.kind,
            DpsAbilityKind::HammerStorm | DpsAbilityKind::SkullCrusher
        ) {
            let mut cost = if ability.kind == DpsAbilityKind::HammerStorm {
                ability_param(&ability, parameter_key!("maximumFuryCost"))
            } else {
                ability_param(&ability, parameter_key!("furyCost"))
            };
            if self.hero.tariq().focused_wrath_stacks > 0
                && self.common.now_ms < self.hero.tariq().focused_wrath_until
            {
                let focused = self
                    .ability(DpsAbilityKind::FocusedWrath)
                    .expect("Tariq profile includes Focused Wrath");
                cost *= ability_param(focused, parameter_key!("costMultiplier"));
                context.multiply_damage(ability_param(focused, parameter_key!("damageMultiplier")));
                self.hero.tariq_mut().focused_wrath_stacks -= 1;
                if self.hero.tariq().focused_wrath_stacks == 0 {
                    self.hero.tariq_mut().focused_wrath_until = 0;
                    self.deactivate_fixed_buff(AplBuff::FocusedWrath);
                }
            }
            let spent = self.hero.tariq().fury.min(cost);
            self.hero.tariq_mut().fury -= spent;
            self.try_tariq_spirit_refund(&ability, spent);
            if let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent9") {
                let multiplier = param(talent, parameter_key!("damageMultiplier"));
                let state = self.hero.tariq_mut();
                let (stacks, until) = if ability.kind == DpsAbilityKind::HammerStorm {
                    (
                        &mut state.schism_skull_stacks,
                        &mut state.schism_skull_until,
                    )
                } else {
                    (
                        &mut state.schism_hammer_stacks,
                        &mut state.schism_hammer_until,
                    )
                };
                if self.common.now_ms < *until && *stacks > 0 {
                    *stacks -= 1;
                    context.multiply_damage(multiplier);
                    if *stacks == 0 {
                        *until = 0;
                    }
                }
            }
            if self.common.now_ms < self.hero.tariq().square_hammer_until
                && self.hero.tariq().square_hammer_stacks > 0
            {
                let talent = self
                    .common
                    .selected_talents
                    .get("ink-talent-id-talent18")
                    .expect("Square Hammer stacks require the selected talent");
                let expertise_duration =
                    ms_param(talent, parameter_key!("expertiseDurationSeconds"));
                let cooldown_reduction =
                    ms_param(talent, parameter_key!("cooldownReductionPerStackSeconds"));
                let stacks = std::mem::take(&mut self.hero.tariq_mut().square_hammer_stacks);
                self.hero.tariq_mut().square_hammer_until = 0;
                self.deactivate_fixed_buff(AplBuff::SquareHammer);
                self.hero.tariq_mut().square_hammer_expertise_until =
                    self.common.now_ms.saturating_add(expertise_duration);
                self.reduce_cooldown(
                    DpsAbilityKind::ThunderCall,
                    u64::from(stacks).saturating_mul(cooldown_reduction),
                );
                self.activate_fixed_buff(
                    AplBuff::SquareHammerExpertise,
                    self.hero.tariq().square_hammer_expertise_until,
                );
            }
        }
        if ability.kind == DpsAbilityKind::CullingStrike
            && let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent12")
        {
            bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
        }
        if ability.kind == DpsAbilityKind::FaceBreaker
            && let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent2")
        {
            bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
        }
        if ability.kind == DpsAbilityKind::SkullCrusher
            && let Some(talent) = self.common.selected_talents.get("ink-talent-id-talent10")
            && self.roll_controlled_random_bool(
                "RandomStream.Ink.Talents.HeavySingleTargetAttack.ProcAddedCritChance",
                param(talent, parameter_key!("procChance")),
            )
        {
            self.record_talent_proc("ink-talent-id-talent10");
            bonus_crit += param(talent, parameter_key!("criticalStrikeBonus"));
        }
        if ability.kind == DpsAbilityKind::HeavyStrike
            && self.hero.tariq().kill_em_all_stacks > 0
            && self.common.now_ms < self.hero.tariq().kill_em_all_until
        {
            let damage_multiplier = self
                .common
                .selected_talents
                .get("ink-talent-id-talent11")
                .map(|talent| param(talent, parameter_key!("damageMultiplier")))
                .unwrap_or(1.0);
            self.hero.tariq_mut().kill_em_all_stacks -= 1;
            context.multiply_damage(damage_multiplier);
            if self.hero.tariq().kill_em_all_stacks == 0 {
                self.deactivate_fixed_buff(AplBuff::KillEmAll);
            }
        }
        self.common.result.abilities[ability.damage_source.0].casts += 1;
        let haste = (1.0 + self.effective_haste()).max(0.05);
        let ability_time_rate = if ability.scale_time_with_haste {
            haste
        } else {
            1.0
        };
        if matches!(
            ability.kind,
            DpsAbilityKind::Multishot | DpsAbilityKind::GrimCarve
        ) {
            ability.first_hit_delay_ms +=
                (ability_param(&ability, parameter_key!("projectileSpawnDelaySeconds")) * 1_000.0
                    / ability_time_rate)
                    .round() as u64;
        }
        // ServerCasting passes the base duration through ModifyCastingDuration
        // before applying the ability time rate (build 25485623 RVA 0x5103ea0).
        let cast_ms = (ability.cast_time_ms.saturating_sub(cast_reduction_ms) as f64
            / ability_time_rate)
            .round() as u64;
        if self.profile.contract.hero == HeroIdentity::Elarion && cast_ms > 0 {
            self.hero.elarion_mut().auto_blocked_until = self.common.now_ms.saturating_add(cast_ms);
        }
        if self.profile.contract.hero == HeroIdentity::Gunde {
            let block = if ability.kind == DpsAbilityKind::Warbound {
                ability.first_hit_delay_ms
            } else {
                cast_ms
            };
            if block > 0 {
                self.hero.gunde_mut().auto_blocked_until = self.common.now_ms.saturating_add(block);
            }
        }
        let gcd_haste_rate = match ability.gcd_haste_mode {
            DpsGcdHasteMode::None => 1.0,
            DpsGcdHasteMode::Standard => haste,
            DpsGcdHasteMode::SlowOnly => haste.min(1.0),
        };
        let gcd_ms = ability.gcd_ms as f64 / gcd_haste_rate;

        if rime_talon_spikes > 0 {
            let cast_ms = 0;
            if !glacial_assault_cast {
                self.spend_rime_orbs(rime_talon_spikes);
            }
            self.commit_ability(index, &mut context, free_charge, 0.0);
            bonus_crit = self.capture_rime_spender_bonuses(glacial_assault_cast, &mut context);
            let talent = self
                .common
                .selected_talents
                .get("rime-talent-id-talent17")
                .expect("selected Icy Talons talent");
            let (source, multiplier, initial_delay_ms, period_ms) =
                if ability.kind == DpsAbilityKind::GlacialBlast {
                    (
                        if glacial_assault_cast {
                            RimeTriggeredDamageSource::GlacialAssaultTalonStrike
                        } else {
                            RimeTriggeredDamageSource::TalonStrike
                        },
                        param(talent, parameter_key!("singleTargetDamageMultiplier")),
                        ms_param(talent, parameter_key!("singleTargetInitialDelaySeconds")),
                        ms_param(talent, parameter_key!("singleTargetPulsePeriodSeconds")),
                    )
                } else {
                    (
                        RimeTriggeredDamageSource::RisingTalons,
                        param(talent, parameter_key!("multiTargetDamageMultiplier")),
                        ms_param(talent, parameter_key!("multiTargetInitialDelaySeconds")),
                        ms_param(talent, parameter_key!("multiTargetPulsePeriodSeconds")),
                    )
                };
            let targets = if ability.kind == DpsAbilityKind::IceComet {
                self.common.target_count
            } else {
                1
            };
            if !self.charge_work_product(u64::from(rime_talon_spikes), u64::from(targets)) {
                return;
            }
            let spawn_haste = (1.0 + self.effective_haste()).max(0.05);
            for spike in 0..rime_talon_spikes {
                let at_ms = self
                    .common
                    .now_ms
                    .saturating_add((initial_delay_ms as f64 / spawn_haste).round() as u64)
                    .saturating_add(period_ms.saturating_mul(
                        u64::from(spike) + u64::from(ability.kind == DpsAbilityKind::GlacialBlast),
                    ));
                self.schedule_rime_spender_pulse(
                    source,
                    ability.power_coefficient * multiplier,
                    bonus_crit,
                    targets,
                    context,
                    at_ms.saturating_sub(self.common.now_ms),
                );
            }
            if ability.kind == DpsAbilityKind::IceComet
                && self
                    .common
                    .selected_talents
                    .contains_key("rime-talent-id-talent6")
            {
                // The swapped graph invokes the utility after consuming Icy Flow.
                self.schedule_rime_comet(context.as_proc(), 0);
            }
            self.wait_for_ability_lock(cast_ms, gcd_ms, ability.gcd_scales_with_cooldown_recovery);
            return;
        }

        if ability.channel.is_some() {
            // Both current channel graphs cast first and commit the hero or
            // weapon ability only when that cast finishes. Incinerate commits
            // its GCD before casting; that distinction is DPS-neutral here
            // because its cast is longer than the GCD, but preserving it keeps
            // the task ordering faithful.
            let precast_gcd_ms = if ability.kind == DpsAbilityKind::Incinerate {
                gcd_ms
            } else {
                0.0
            };
            self.wait_for_ability_lock(
                cast_ms,
                precast_gcd_ms,
                ability.gcd_scales_with_cooldown_recovery,
            );
            if self.common.now_ms >= ENCOUNTER_DURATION_MS {
                return;
            }

            self.commit_ability(index, &mut context, free_charge, elarion_focus_cost);

            let channel_haste = (1.0 + self.effective_haste()).max(0.05);
            let channel_time_rate = if ability.scale_time_with_haste {
                channel_haste
            } else {
                1.0
            };
            let channel_duration_ms = self.schedule_channel_impacts(
                index,
                &ability,
                CastImpactContext {
                    damage: context,
                    bonus_crit,
                    frostweaver: false,
                    ardeos_wave_empowered,
                    glacial_assault: glacial_assault_cast,
                    impending_heartseeker,
                    elarion_celestial_impetus,
                    mara_combo_points_spent,
                    mara_from_stealth,
                    gunde_rend_transfer_bonus,
                    gunde_legendary_strike: false,
                },
                channel_time_rate,
            );
            if self.profile.contract.hero == HeroIdentity::Elarion {
                self.hero.elarion_mut().auto_blocked_until =
                    self.common.now_ms.saturating_add(channel_duration_ms);
            }
            if self.profile.contract.hero == HeroIdentity::Gunde {
                self.hero.gunde_mut().auto_blocked_until =
                    self.common.now_ms.saturating_add(channel_duration_ms);
            }
            let channel_gcd_ms = if ability.kind == DpsAbilityKind::Incinerate {
                0.0
            } else {
                let rate = match ability.gcd_haste_mode {
                    DpsGcdHasteMode::None => 1.0,
                    DpsGcdHasteMode::Standard => channel_haste,
                    DpsGcdHasteMode::SlowOnly => channel_haste.min(1.0),
                };
                ability.gcd_ms as f64 / rate
            };
            self.wait_for_ability_lock(
                channel_duration_ms,
                channel_gcd_ms,
                ability.gcd_scales_with_cooldown_recovery,
            );
            return;
        }

        let commit_ms = self.common.now_ms.saturating_add(cast_ms);
        let off_gcd = ability.off_gcd;
        let gcd_scales_with_cooldown_recovery = ability.gcd_scales_with_cooldown_recovery;
        self.push_event(
            commit_ms,
            CoreEvent::PreparedCastCommit {
                cast: Box::new(PreparedCast {
                    index,
                    ability,
                    context,
                    ability_time_rate,
                    free_charge,
                    bonus_crit,
                    ardeos_wave_empowered,
                    glacial_assault: glacial_assault_cast,
                    elarion_multishot_arrows,
                    elarion_focus_cost,
                    mara_copy_damage_multiplier,
                    elarion_celestial_impetus,
                    impending_heartseeker,
                    mara_combo_points_spent,
                    mara_from_stealth,
                    gunde_rend_transfer_bonus,
                }),
            },
        );
        if off_gcd {
            self.wait_for_ability_lock(cast_ms, 0.0, false);
        } else {
            self.wait_for_ability_lock(cast_ms, gcd_ms, gcd_scales_with_cooldown_recovery);
        }
    }

    pub(crate) fn commit_ability(
        &mut self,
        index: usize,
        context: &mut DamageContext,
        free_charge: bool,
        elarion_focus_cost: f64,
    ) {
        let ability = self.profile.abilities[index].clone();
        if !free_charge && ability.kind != DpsAbilityKind::BroodingShadows {
            self.spend_ability_charge(&ability, ability.cooldown_ms);
        }
        // Native CommitExecute applies cooldown, then cost, before notifying
        // commit listeners. A refund may reset the cooldown just applied.
        if elarion_focus_cost > 0.0 {
            self.hero.elarion_mut().focus =
                (self.hero.elarion().focus - elarion_focus_cost).max(0.0);
            self.try_elarion_spirit_refund(&ability, elarion_focus_cost);
        }
        // Native CommitExecute applies the cost before notifying commit listeners.
        if ability_category(ability.kind) == AbilityCategory::Spirit {
            self.shared.spirit = (self.shared.spirit - ability.spirit_cost).max(0.0);
            self.activate_heroism();
        }
        *context = self.trigger_dynamic_on_cast(&ability, *context);
        self.try_soulfrost_torrent_proc(&ability);
        if ability.kind == DpsAbilityKind::ColdSnap {
            if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent1") {
                self.hero.rime_mut().glacial_assault_stacks = self
                    .hero
                    .rime()
                    .glacial_assault_stacks
                    .saturating_add(1)
                    .min(param_u32(talent, parameter_key!("maximumStacks")));
                self.activate_fixed_buff(AplBuff::GlacialAssault, ENCOUNTER_DURATION_MS);
            }
            if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent5") {
                self.hero.rime_mut().icy_flow_stacks = self
                    .hero
                    .rime()
                    .icy_flow_stacks
                    .saturating_add(1)
                    .min(param_u32(talent, parameter_key!("maximumStacks")));
                self.hero.rime_mut().icy_flow_until = self
                    .common
                    .now_ms
                    .saturating_add(ms_param(talent, parameter_key!("durationSeconds")));
                self.activate_fixed_buff(AplBuff::IcyFlow, self.hero.rime().icy_flow_until);
            }
            if let Some(talent) = self.common.selected_talents.get("rime-talent-id-talent4") {
                self.reduce_cooldown(
                    DpsAbilityKind::FreezingTorrent,
                    ms_param(talent, parameter_key!("torrentCooldownReductionSeconds")),
                );
            }
        }
        if ability.kind == DpsAbilityKind::WeaponShadowMark {
            // The cast removes an existing debuff from the selected target
            // before spawning the replacement projectile. Removing the
            // granted monitor ends it and dispatches its accumulated hit.
            self.explode_shadow_mark(0);
        }
        if ability.kind == DpsAbilityKind::WeaponArcaneChannel
            && let Some(channel) = &ability.channel
        {
            let duration_rate =
                if channel.scale_duration_with_ability_time_rate && ability.scale_time_with_haste {
                    (1.0 + self.effective_haste()).max(0.05)
                } else {
                    1.0
                };
            self.shared.weapon_channel_cooldown_recovery_until = self.common.now_ms.saturating_add(
                (channel.duration_ms as f64 / duration_rate)
                    .round()
                    .max(0.0) as u64,
            );
        }
        if ability.kind == DpsAbilityKind::Apocalypse
            && let Some(talent) = self
                .common
                .selected_talents
                .get("firemage-talent-id-talent14")
        {
            self.hero.ardeos_mut().apocalyptic_surge = (self.hero.ardeos().apocalyptic_surge
                + param_u32(talent, parameter_key!("surgeStacks")))
            .min(param_u32(talent, parameter_key!("maximumSurgeStacks")).max(1));
            self.hero.ardeos_mut().apocalyptic_surge_until = self
                .common
                .now_ms
                .saturating_add(ms_param(talent, parameter_key!("surgeDurationSeconds")));
        }
        self.commit_tariq_ability(&ability, *context);
        self.commit_elarion_ability(&ability);
        self.commit_mara_ability(&ability);
        self.commit_gunde_ability(&ability);
        if source_damage_spec_created_at_commit(ability.kind) {
            let mut snapshot = self.capture_damage_source_snapshot(ability.kind);
            snapshot.critical_chance = self.effective_critical_strike_for_category(
                Some(ability.kind),
                context.category(Some(ability.kind)),
            );
            context.source_snapshot = Some(snapshot);
        }
        if ability.kind == DpsAbilityKind::FireBall {
            // The projectile spec captures the granted-tag crit modifier before
            // the spawn-success callback removes one stack, not on impact.
            let bonus = self.consume_reign_of_fire();
            if let Some(snapshot) = context.source_snapshot.as_mut() {
                snapshot.critical_chance += bonus;
            }
        }
    }
}
