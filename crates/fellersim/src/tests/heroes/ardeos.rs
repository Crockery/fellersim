use super::super::*;

#[test]
fn power_chance_spirit_rolls_only_on_power_commits_and_caps_the_grant() {
    let power = ability(DpsAbilityKind::Detonate, 1.0);
    let core = ability(DpsAbilityKind::FireFrogs, 1.0);
    let mut profile = profile(vec![power.clone(), core.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.04",
        [("procChance", 1.0), ("spiritGain", 1.0)],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);
    iteration.shared.spirit = profile.max_spirit - 0.5;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::for_cast(1));
    assert!(iteration.shared.controlled_random_states.is_empty());
    assert_eq!(iteration.shared.spirit, profile.max_spirit - 0.5);
    iteration.trigger_dynamic_on_cast(&compiled_ability(&power), DamageContext::for_cast(2));
    assert_eq!(iteration.shared.spirit, profile.max_spirit);
    assert_eq!(iteration.shared.controlled_random_states.len(), 1);
    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Finesse.PowerChanceSpirit")
    );
}

#[test]
fn undying_flame_extends_only_engulfing_flames() {
    let searing = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    let engulfing = dot_ability(DpsAbilityKind::EngulfingFlames, 0.2);
    let mut profile = profile(vec![searing.clone(), engulfing.clone()]);
    profile.talents.push(DpsTalentModel {
        id: "firemage-talent-id-talent12".into(),
        name: "Undying Flame".into(),
        mechanic_id: "test:undying-flame".into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([("durationIncreaseSeconds".into(), 3.0)]),
        reason: None,
    });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);
    for ability in [&searing, &engulfing] {
        iteration.apply_dot(
            0,
            ability.kind,
            &compiled_ability(ability),
            ability.dot.unwrap(),
            DamageContext::for_cast(1),
        );
    }
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))].expires_ms,
        10_000
    );
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::EngulfingFlames))].expires_ms,
        13_000
    );
}

#[test]
fn burning_initiative_starts_a_fresh_run_with_current_resources() {
    let mut profile = profile(Vec::new());
    profile.talents.push(DpsTalentModel {
        id: "firemage-talent-id-talent15".into(),
        name: "Burning Initiative".into(),
        mechanic_id:
            "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait2".into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("startingSpirit".into(), 50.0),
            ("startingEmbers".into(), 2.0),
        ]),
        reason: None,
    });
    let apl = apl(std::iter::empty::<(&'static str, Option<AplExpressionNode>)>());

    let iteration = Iteration::new(&profile, &apl, 1, 1);

    assert_eq!(iteration.shared.spirit, 50.0);
    assert_eq!(iteration.hero.ardeos().embers, 2);
    assert_eq!(iteration.hero.ardeos().cinders, 0.0);
}

#[test]
fn sinister_applies_both_critical_modifiers_only_to_core_abilities() {
    let frogs = ability(DpsAbilityKind::FireFrogs, 1.0);
    let wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![frogs.clone(), wave.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.05",
        [
            ("criticalStrikeBonus", 0.04),
            ("criticalPowerMultiplier", 1.2),
        ],
    ));
    let apl = apl([("fire-frogs", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 2);

    assert_eq!(iteration.effective_critical_strike(Some(frogs.kind)), 0.04);
    assert_eq!(
        iteration.effective_critical_multiplier(Some(frogs.kind)),
        2.4
    );
    assert_eq!(iteration.effective_critical_strike(Some(wave.kind)), 0.0);
    assert_eq!(
        iteration.effective_critical_multiplier(Some(wave.kind)),
        2.0
    );
}

#[test]
fn fire_ball_uses_two_sequentially_recharging_charges() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.maximum_charges = 2;
    let profile = profile(vec![fire_ball.clone()]);
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 4);

    iteration.spend_ability_charge(&compiled_ability(&fire_ball), fire_ball.cooldown_ms);
    iteration.advance_to(2_000);
    iteration.spend_ability_charge(&compiled_ability(&fire_ball), fire_ball.cooldown_ms);

    let cooldown = iteration
        .common
        .cooldowns
        .get(&DpsAbilityKind::FireBall)
        .copied()
        .expect("Fire Ball cooldown");
    assert_eq!(cooldown.used_charges, 2);
    assert_eq!(cooldown.remaining_ms, 8_000.0);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::FireBall),
        8_000
    );

    iteration.advance_to(10_000);
    let cooldown = iteration
        .common
        .cooldowns
        .get(&DpsAbilityKind::FireBall)
        .copied()
        .expect("Fire Ball cooldown");
    assert_eq!(cooldown.used_charges, 1);
    assert_eq!(cooldown.remaining_ms, 10_000.0);
    assert_eq!(iteration.cooldown_remaining_ms(DpsAbilityKind::FireBall), 0);

    iteration.advance_to(20_000);
    assert!(
        !iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::FireBall)
    );
}

#[test]
fn reign_of_fire_restores_one_charge_without_refreshing_the_next_timer() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.maximum_charges = 2;
    let profile = profile(vec![fire_ball]);
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 2,
        },
    );

    iteration.restore_ability_charge(DpsAbilityKind::FireBall);

    assert_eq!(
        iteration
            .common
            .cooldowns
            .get(&DpsAbilityKind::FireBall)
            .copied()
            .expect("Fire Ball cooldown"),
        CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 1,
        }
    );
}

fn reign_of_fire_talent() -> DpsTalentModel {
    DpsTalentModel {
        id: "firemage-talent-id-talent2".into(),
        name: "Reign of Fire".into(),
        mechanic_id: "test:reign-of-fire".into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("procsPerMinute".into(), 60_000.0),
            ("criticalStrikeBonus".into(), 1.0),
            ("durationSeconds".into(), 12.0),
            ("maximumStacks".into(), 2.0),
        ]),
        reason: None,
    }
}

#[test]
fn native_real_ppm_uses_first_proc_floor_and_bad_luck_protection() {
    let initial = ProcPerMinuteState {
        last_roll_seconds: 37.0,
        last_proc_seconds: 37.0,
        has_procced: false,
    };
    assert!((real_ppm_probability(37.0, initial, 1.5) - (1.5 / 9.0)).abs() < 1e-6);

    let established = ProcPerMinuteState {
        last_roll_seconds: 100.0,
        last_proc_seconds: 0.0,
        has_procced: true,
    };
    // At 1.5 PPM and 120 seconds since the last proc, the native bad-luck
    // multiplier is 5.5. Twenty seconds since the last roll gives a base
    // chance of 0.5.
    assert!((real_ppm_probability(120.0, established, 1.5) - 2.75).abs() < 1e-6);
}

#[test]
fn native_controlled_random_uses_bucketed_failure_decay_per_tag() {
    assert_eq!(controlled_random_bucket(0.0), 0);
    assert_eq!(controlled_random_bucket(0.1), 10);
    assert_eq!(controlled_random_bucket(0.5), 50);
    assert_eq!(controlled_random_bucket(0.9), 90);
    assert_eq!(controlled_random_bucket(1.0), 100);
    assert!((CONTROLLED_RANDOM_FACTORS[10] - 0.984_358_1).abs() < f32::EPSILON);

    let profile = profile(Vec::new());
    let apl = apl(std::iter::empty::<(&'static str, Option<AplExpressionNode>)>());
    let mut iteration = Iteration::new(&profile, &apl, 1, 17);
    iteration.shared.controlled_random_states.insert(
        "changed-chance".into(),
        ControlledRandomState {
            failure_threshold: 0.5,
            chance_factor: 0.1,
            chance_bucket: 10,
        },
    );
    let did_proc = iteration.roll_controlled_random_bool("changed-chance", 0.2);
    let state = iteration.shared.controlled_random_states["changed-chance"];
    assert_eq!(state.chance_factor, 0.2);
    assert_eq!(
        state.failure_threshold,
        if did_proc {
            1.0
        } else {
            0.5 * CONTROLLED_RANDOM_FACTORS[20]
        }
    );

    let successes = (0..100_000)
        .filter(|_| iteration.roll_controlled_random_bool("ten-percent", 0.1))
        .count();
    assert!((successes as f64 / 100_000.0 - 0.1).abs() < 0.003);
}

#[test]
fn native_spirit_refund_formula_uses_scaled_spirit_and_flat_increase() {
    assert_eq!(spirit_refund_chance(0.0, 1.0, 0.0), 0.0);
    assert!((spirit_refund_chance(0.25, 1.0, 0.0) - 0.2).abs() < f64::EPSILON);
    assert!((spirit_refund_chance(0.25, 2.0, 0.1) - (1.0 / 3.0 + 0.1)).abs() < 1e-12);
}

#[test]
fn paid_detonate_spirit_refund_restores_resources_and_triggers_willful_momentum() {
    let detonate = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![detonate]);
    profile.spirit = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.IncreasedMainStatAndSpiritRating",
        [
            ("durationSeconds", 4.0),
            ("powerMultiplier", 1.048),
            ("spiritRating", 23.0),
        ],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    iteration.hero.ardeos_mut().embers = 1;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 1_000);
    assert_eq!(iteration.hero.ardeos().embers, 1);
    assert_eq!(iteration.hero.ardeos().cinders, 0.0);
    assert_eq!(iteration.shared.spirit, 1.0);
    assert_eq!(iteration.effective_power_multiplier(), 1.048);
    assert_eq!(
        iteration
            .test_dynamic_buff("ItemTrait.ID.IncreasedMainStatAndSpiritRating")
            .until_ms,
        4_000
    );
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.IncreasedMainStatAndSpiritRating"),
        1.0
    );
}

#[test]
fn free_detonate_does_not_attempt_a_spirit_refund() {
    let detonate = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![detonate]);
    profile.spirit = 1.0;
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.hero.ardeos_mut().apocalyptic_surge = 1;
    iteration.hero.ardeos_mut().apocalyptic_surge_until = 10_000;

    iteration.cast(0);

    assert_eq!(iteration.hero.ardeos().embers, 0);
    assert_eq!(iteration.shared.spirit, 0.0);
    assert!(
        !iteration
            .shared
            .controlled_random_states
            .contains_key(SPIRIT_PROC_RANDOM_STREAM_TAG)
    );
}

#[test]
fn reign_of_fire_requires_a_successful_detonation_but_not_an_active_cooldown() {
    let detonate = ability(DpsAbilityKind::Detonate, 1.0);
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.maximum_charges = 2;
    let mut profile = profile(vec![detonate, fire_ball]);
    profile.talents.push(reign_of_fire_talent());
    let apl = apl([("detonate", None), ("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 2,
        },
    );

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));

    assert_eq!(
        iteration
            .common
            .cooldowns
            .get(&DpsAbilityKind::FireBall)
            .expect("Fire Ball cooldown")
            .used_charges,
        2
    );
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 0);

    iteration.common.cooldowns.clear();
    iteration.try_reign_of_fire();

    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 1);
    assert_eq!(iteration.test_proc_count("firemage-talent-id-talent2"), 1);
    assert!(
        !iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::FireBall)
    );
    assert!(iteration.shared.proc_per_minute_states[REIGN_OF_FIRE_PPM_STREAM_TAG].has_procced);
}

#[test]
fn reign_of_fire_real_ppm_scales_with_current_haste() {
    let fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    let mut profile = profile(vec![fire_ball]);
    profile.haste = 0.5;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);

    iteration.roll_proc_per_minute(REIGN_OF_FIRE_PPM_STREAM_TAG, 0.0, true);
    iteration.common.now_ms = 10_000;
    let state = iteration.shared.proc_per_minute_states[REIGN_OF_FIRE_PPM_STREAM_TAG];
    assert!((real_ppm_probability(10.0, state, 1.5 * 1.5) - 0.375).abs() < 1e-6);
}

#[test]
fn dynamic_set_proc_uses_native_rppm_state_and_authored_cooldown() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let mechanic = ardeos_mechanic(
        "seta-proc-intellect",
        [
            ("procsPerMinute", 60_000.0),
            ("ppmCriticalScaling", 1.0),
            ("cooldownSeconds", 5.0),
        ],
    );
    profile.critical_strike = 0.5;
    profile.mechanics.push(mechanic.clone());
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    let mechanic = Arc::clone(&iteration.profile.mechanics[0]);

    assert!(iteration.roll_dynamic_proc(&mechanic));
    assert!(!iteration.roll_dynamic_proc(&mechanic));
    assert!(iteration.shared.dynamic_proc_per_minute_states[mechanic.index].is_some());

    iteration.common.now_ms = 5_000;
    assert!(iteration.roll_dynamic_proc(&mechanic));
    assert_eq!(iteration.test_uptime("proc:seta-proc-intellect"), 2.0);
    assert_eq!(iteration.test_proc_count("seta-proc-intellect"), 2);
}

#[test]
fn reign_of_fire_proc_restores_one_charge_and_grants_the_next_crit_buff() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.maximum_charges = 2;
    let mut profile = profile(vec![fire_ball]);
    profile.talents.push(reign_of_fire_talent());
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 2,
        },
    );

    iteration.try_reign_of_fire();

    assert_eq!(
        iteration
            .common
            .cooldowns
            .get(&DpsAbilityKind::FireBall)
            .expect("Fire Ball cooldown"),
        &CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 1,
        }
    );
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 1);
    assert_eq!(iteration.test_proc_count("firemage-talent-id-talent2"), 1);
}

#[test]
fn haste_and_cooldown_recovery_add_to_tagged_effect_time_rate() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_scales_with_haste = true;
    fire_ball.cooldown_scales_with_cooldown_recovery = true;
    let mut profile = profile(vec![fire_ball]);
    profile.haste = 0.5;
    profile.cooldown_recovery = 1.2;
    let apl = apl([("fire-ball", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 6);

    assert_eq!(
        iteration.cooldown_recovery_rate(DpsAbilityKind::FireBall),
        1.7
    );
}

#[test]
fn weapon_cooldowns_opt_out_of_cooldown_recovery() {
    let mut weapon = ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.cooldown_scales_with_cooldown_recovery = false;
    let mut profile = profile(vec![weapon]);
    profile.cooldown_recovery = 2.0;
    let apl = apl([("weapon-frost-volley", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 7);

    assert_eq!(
        iteration.cooldown_recovery_rate(DpsAbilityKind::WeaponFrostVolley),
        1.0
    );
}

#[test]
fn casted_ability_cooldown_starts_when_the_cast_commits() {
    let mut casted = ability(DpsAbilityKind::Apocalypse, 1.0);
    casted.cast_time_ms = 2_000;
    casted.cooldown_ms = 10_000;
    let profile = profile(vec![casted]);
    let apl = apl([("apocalypse", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 42);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 2_000);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::Apocalypse),
        10_000
    );
}

#[test]
fn detonate_style_gcd_ignores_positive_haste_but_uses_recovery() {
    let mut ability = ability(DpsAbilityKind::InfernalWave, 1.0);
    ability.gcd_haste_mode = DpsGcdHasteMode::SlowOnly;
    ability.gcd_scales_with_cooldown_recovery = true;
    let mut profile = profile(vec![ability]);
    profile.haste = 1.0;
    profile.cooldown_recovery = 2.0;
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 500);
}

#[test]
fn non_hasted_weapon_actor_offsets_keep_their_cooked_schedule() {
    let mut weapon = ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.scale_time_with_haste = false;
    weapon.first_hit_delay_ms = 1_000;
    weapon.hit_interval_ms = 1_000;
    weapon.direct_hits = 2;
    let mut profile = profile(vec![weapon]);
    profile.haste = 1.0;
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 9);

    iteration.cast(0);

    let event_times = iteration
        .common
        .queue
        .iter()
        .filter(|event| !matches!(event.0.kind, EventKind::Core(CoreEvent::PassiveSpiritRegen)))
        .map(|event| event.0.at_ms)
        .collect::<BTreeSet<_>>();
    assert_eq!(event_times, BTreeSet::from([1_000, 2_000]));
}

#[test]
fn fire_ball_projectile_uses_its_fixed_unhasted_travel_duration() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.first_hit_delay_ms = 1_000;
    fire_ball.maximum_charges = 2;
    let mut profile = profile(vec![fire_ball]);
    profile.haste = 1.0;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 10);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 500);
    assert_eq!(
        iteration.common.queue.peek().map(|event| event.0.at_ms),
        Some(1_000)
    );
}

#[test]
fn wildfire_reschedules_the_remaining_dot_tick_phase() {
    let dot = dot_ability(DpsAbilityKind::SearingBlaze, 0.2);
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.effect_duration_ms = 2_000;
    wildfire
        .mechanic_parameters
        .insert("tickRateMultiplier".into(), 2.0);
    let profile = profile(vec![dot, wildfire]);
    let apl = apl([("wildfire", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    iteration.common.now_ms = 1_000;
    iteration.common.dots.insert(
        (0, DotKind::Ability(DpsAbilityKind::SearingBlaze)),
        DotState {
            model: profile.abilities[0].dot.expect("dot model"),
            source: iteration.profile.abilities[0].damage_source,
            expertise_snapshot: 0.0,
            primary_stat_multiplier_snapshot: 1.0,
            derived_damage_per_tick: None,
            gunde_rend_buckets: None,
            critical_chance_override: None,
            bonus_crit: 0.0,
            generation: 1,
            started_ms: 0,
            expires_ms: 10_000,
            stacks: 1,
            context: DamageContext::for_cast(1),
            last_tick_ms: 1_000,
            next_tick_ms: 2_000,
            scheduled_period_ms: 1_000,
        },
    );

    iteration.impact(1, false, 1.0, 0, DamageContext::for_cast(2));

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.next_tick_ms, 1_500);
    assert_eq!(active.scheduled_period_ms, 500);
    assert_eq!(active.generation, 2);
}

#[test]
fn damage_derived_dots_transfer_resolved_damage_without_reapplying_source_stats() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.mechanic_parameters = BTreeMap::from([
        ("damageToDotTransferFraction".into(), 0.2),
        ("targetCountDamageScalingThreshold".into(), 1.0),
    ]);
    fire_ball.dot = Some(DotModel {
        power_coefficient: 0.0,
        damage_spread: 0.0,
        duration_ms: 4_000,
        period_ms: 1_000,
        can_crit: false,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    });
    let mut profile = profile(vec![fire_ball]);
    profile.expertise = 0.5;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 9);

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    iteration.process_events_through(1_000);

    // The hit resolves to 150, 20% is spread over four ticks, and the
    // derived effect's preset does not apply Expertise again; its 7.5 raw
    // damage rounds to 8 when the tick executes.
    assert!((iteration.common.result.damage - 158.0).abs() < 1e-10);
    let dot = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::FireBall))];
    assert!((dot.derived_damage_per_tick.unwrap_or_default() - 7.5).abs() < 1e-10);
    assert_eq!(dot.critical_chance_override, Some(0.0));
}

#[test]
fn damage_derived_refresh_preserves_phase_and_conserves_remaining_damage() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 0.0)]);
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 4);
    let model = DotModel {
        power_coefficient: 0.0,
        damage_spread: 0.0,
        duration_ms: 4_000,
        period_ms: 1_000,
        can_crit: true,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    };

    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        model,
        40.0,
        DamageContext::for_cast(1),
    );
    iteration.process_events_through(1_500);
    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        model,
        20.0,
        DamageContext::for_cast(2),
    );

    let refreshed = &iteration.common.dots[&(0, DotKind::CracklingInferno)];
    assert_eq!(refreshed.next_tick_ms, 2_000);
    assert_eq!(refreshed.expires_ms, 5_500);
    iteration.process_events_through(5_500);
    assert!((iteration.common.result.damage - 60.0).abs() < 1e-9);
}

#[test]
fn detonate_sampler_uses_ordinary_hits_for_standard_and_derived_dots() {
    let mut searing = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    searing.dot.as_mut().expect("dot model").can_crit = true;
    let mut profile = profile(vec![searing]);
    profile.critical_strike = 0.5;
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    let ability = profile.abilities[0].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        ability.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    let standard = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert!(
        (iteration.approximate_dot_average_damage(
            DotKind::Ability(DpsAbilityKind::SearingBlaze),
            standard,
            0,
        ) - 10.0)
            .abs()
            < 1e-10
    );

    let derived_model = DotModel {
        power_coefficient: 0.0,
        damage_spread: 0.0,
        duration_ms: 4_000,
        period_ms: 1_000,
        can_crit: true,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    };
    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        derived_model,
        40.0,
        DamageContext::for_cast(2),
    );
    let derived = &iteration.common.dots[&(0, DotKind::CracklingInferno)];
    assert!(
        (iteration.approximate_dot_average_damage(DotKind::CracklingInferno, derived, 0,) - 10.0)
            .abs()
            < 1e-10
    );

    // Fractional magnitudes round before conversion to DPS; a guaranteed
    // critical override still does not change this native ordinary-hit estimate.
    let mut fractional = derived.clone();
    fractional.derived_damage_per_tick = Some(7.5);
    fractional.critical_chance_override = Some(1.5);
    fractional.scheduled_period_ms = 500;
    assert_eq!(
        iteration.approximate_dot_average_damage(DotKind::CracklingInferno, &fractional, 0,),
        16.0
    );
}

#[test]
fn wildfire_adds_derived_dot_ticks_without_reducing_the_base_tick_damage() {
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.effect_duration_ms = 10_000;
    wildfire
        .mechanic_parameters
        .insert("tickRateMultiplier".into(), 2.0);
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 0.0), wildfire]);
    profile.haste = 1.0;
    let apl = apl([("wildfire", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);
    let model = DotModel {
        power_coefficient: 0.0,
        damage_spread: 0.0,
        duration_ms: 4_000,
        period_ms: 1_000,
        can_crit: true,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    };

    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        model,
        40.0,
        DamageContext::for_cast(1),
    );
    assert_eq!(
        iteration.common.dots[&(0, DotKind::CracklingInferno)].scheduled_period_ms,
        1_000
    );
    iteration.impact(1, false, 1.0, 0, DamageContext::for_cast(2));
    assert_eq!(
        iteration.common.dots[&(0, DotKind::CracklingInferno)].scheduled_period_ms,
        500
    );
    assert!(
        (iteration.common.dots[&(0, DotKind::CracklingInferno)]
            .derived_damage_per_tick
            .unwrap_or_default()
            - 10.0)
            .abs()
            < 1e-10
    );
    iteration.process_events_through(3_500);
    assert!((iteration.common.result.damage - 70.0).abs() < 1e-10);
}

#[test]
fn damage_derived_refresh_uses_base_period_with_wildfire_timer_phase() {
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.effect_duration_ms = 10_000;
    wildfire
        .mechanic_parameters
        .insert("tickRateMultiplier".into(), 2.0);
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 0.0), wildfire]);
    let apl = apl([("wildfire", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    let model = DotModel {
        power_coefficient: 0.0,
        damage_spread: 0.0,
        duration_ms: 4_000,
        period_ms: 1_000,
        can_crit: true,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    };

    iteration.impact(1, false, 1.0, 0, DamageContext::for_cast(1));
    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        model,
        40.0,
        DamageContext::for_cast(2),
    );
    iteration.process_events_through(750);
    iteration.apply_damage_derived_dot(
        0,
        DotKind::CracklingInferno,
        iteration.test_damage_source("talent:crackling-inferno"),
        model,
        20.0,
        DamageContext::for_cast(3),
    );

    let refreshed = &iteration.common.dots[&(0, DotKind::CracklingInferno)];
    assert_eq!(refreshed.next_tick_ms, 1_000);
    assert!((refreshed.derived_damage_per_tick.unwrap_or_default() - 60.0 / 4.75).abs() < 1e-10);
}

#[test]
fn firemage_dots_execute_proportional_partial_ticks_at_expiration() {
    let mut source = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    source.dot.as_mut().expect("dot model").duration_ms = 1_500;
    let profile = profile(vec![source]);
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let ability = profile.abilities[0].clone();

    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        ability.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    iteration.process_events_through(1_500);

    assert!((iteration.common.result.damage - 15.0).abs() < 1e-10);
    assert!(
        !iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze)))
    );
}

#[test]
fn ordinary_firemage_dot_refresh_preserves_the_current_tick_phase() {
    let source = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    let profile = profile(vec![source]);
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let ability = profile.abilities[0].clone();
    let model = ability.dot.expect("dot model");

    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        model,
        DamageContext::for_cast(1),
    );
    iteration.common.now_ms = 400;
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        model,
        DamageContext::for_cast(2),
    );

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.last_tick_ms, 0);
    assert_eq!(active.next_tick_ms, 1_000);
    assert_eq!(active.scheduled_period_ms, 1_000);
    assert_eq!(active.expires_ms, 10_400);
}

#[test]
fn backdraft_uses_the_uncapped_active_effect_duration_increase() {
    let mut detonate = ability(DpsAbilityKind::Detonate, 1.0);
    detonate.mechanic_parameters = BTreeMap::from([
        ("hitsPerTarget".into(), 1.0),
        ("initialDelaySeconds".into(), 0.0),
        ("perTargetHitDelaySeconds".into(), 0.0),
        ("betweenHitDelaySeconds".into(), 0.0),
    ]);
    let mut profile = profile(vec![
        detonate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.1),
    ]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent10".into(),
            name: "Ardeos talent 10".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent10".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([("extensionSeconds".into(), 1.5)]),
            reason: None,
        });
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let searing = profile.abilities[1].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&searing),
        searing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.model.duration_ms, 10_000);
    assert_eq!(active.expires_ms, 11_500);
}

#[test]
fn separate_fire_ball_burns_each_extend_the_same_core_dots() {
    let searing = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    let mut fire_ball = dot_ability(DpsAbilityKind::FireBall, 0.0);
    fire_ball.dot.as_mut().expect("dot model").duration_ms = 3_000;
    let mut profile = profile(vec![searing, fire_ball]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent18".into(),
            name: "Ardeos talent 18".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent9".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([("extensionPerTickSeconds".into(), 1.0)]),
            reason: None,
        });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let searing = profile.abilities[0].clone();
    let fire_ball_model = profile.abilities[1].dot.expect("dot model");
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&searing),
        searing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    iteration.apply_damage_derived_dot(
        0,
        DotKind::Ability(DpsAbilityKind::FireBall),
        iteration.test_damage_source("test:fire-ball"),
        fire_ball_model,
        30.0,
        DamageContext::for_cast(2),
    );
    iteration.process_events_through(3_000);

    iteration.common.now_ms = 4_000;
    iteration.apply_damage_derived_dot(
        0,
        DotKind::Ability(DpsAbilityKind::FireBall),
        iteration.test_damage_source("test:fire-ball"),
        fire_ball_model,
        30.0,
        DamageContext::for_cast(3),
    );
    iteration.process_events_through(7_000);

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.expires_ms, 16_000);
}

#[test]
fn incinerate_extensions_restore_but_do_not_exceed_total_duration() {
    let mut incinerate = dot_ability(DpsAbilityKind::Incinerate, 0.1);
    incinerate.power_coefficient = 0.1;
    incinerate.dot_extension_ms = 1_500;
    let profile = profile(vec![
        incinerate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.1),
    ]);
    let apl = apl([("incinerate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let searing = profile.abilities[1].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&searing),
        searing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    iteration.common.now_ms = 2_000;

    iteration.impact(0, true, 1.0, 0, DamageContext::for_cast(2));
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))].expires_ms,
        11_500
    );
    iteration.impact(0, true, 1.0, 0, DamageContext::for_cast(3));
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))].expires_ms,
        12_000
    );
    iteration.impact(0, true, 1.0, 0, DamageContext::for_cast(4));
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))].expires_ms,
        12_000
    );
}

#[test]
fn incinerate_casts_then_uses_hasted_immediate_and_full_partial_ticks() {
    let mut incinerate = ability(DpsAbilityKind::Incinerate, 1.0);
    incinerate.cast_time_ms = 1_500;
    incinerate.channel = Some(ChannelModel {
        duration_ms: 2_500,
        tick_interval_ms: 500,
        tick_immediately: true,
        scale_duration_with_ability_time_rate: false,
        enable_partial_ticks: true,
        scale_partial_tick_damage: false,
    });
    let mut profile = profile(vec![incinerate]);
    profile.spirit = 100.0;
    profile.heroism_haste = 0.3;
    profile.heroism_duration_ms = 20_000;
    let apl = apl([("incinerate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.shared.spirit = 100.0;

    iteration.cast(0);

    let result = &iteration.ability_totals("test:incinerate");
    assert_eq!(iteration.common.now_ms, 4_000);
    assert_eq!(result.hits, 8);
    assert_eq!(result.damage, 800.0);
    assert_eq!(
        iteration.shared.spirit,
        91.0 + 8.0 * f64::from((100.0_f64 / 6_637_912.0 * 11.25) as f32)
    );
    assert_eq!(iteration.shared.heroism_until, 21_500);
}

#[test]
fn incinerate_dot_stacks_stop_at_the_extracted_effect_cap() {
    let mut incinerate = dot_ability(DpsAbilityKind::Incinerate, 0.1);
    let dot = incinerate.dot.as_mut().expect("dot model");
    dot.stack_damage_increase = 0.3;
    dot.maximum_stacks = 50;
    let profile = profile(vec![incinerate]);
    let apl = apl([("incinerate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let ability = profile.abilities[0].clone();

    for cast_id in 1..=60 {
        iteration.apply_dot(
            0,
            DpsAbilityKind::Incinerate,
            &compiled_ability(&ability),
            ability.dot.expect("dot model"),
            DamageContext::for_cast(cast_id),
        );
    }

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::Incinerate))];
    assert_eq!(active.stacks, 50);
    assert!(
        (iteration.dot_raw_damage_per_tick(DotKind::Ability(DpsAbilityKind::Incinerate), active,)
            - 157.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn weapon_channel_scales_only_its_fractional_damage_tick() {
    let mut weapon = ability(DpsAbilityKind::WeaponArcaneChannel, 1.0);
    weapon.channel = Some(ChannelModel {
        duration_ms: 2_500,
        tick_interval_ms: 1_000,
        tick_immediately: true,
        scale_duration_with_ability_time_rate: false,
        enable_partial_ticks: true,
        scale_partial_tick_damage: true,
    });
    weapon.mechanic_parameters = BTreeMap::from([
        ("channelCooldownRecoveryMultiplier".into(), 8.0),
        ("targetCountDamageScalingThreshold".into(), 1.0),
    ]);
    let profile = profile(vec![weapon]);
    let apl = apl([("weapon-arcane-channel", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);

    iteration.cast(0);

    let result = &iteration.ability_totals("test:weapon-arcane-channel");
    assert_eq!(iteration.common.now_ms, 2_500);
    assert_eq!(result.hits, 4);
    assert_eq!(result.damage, 350.0);
}

#[test]
fn weapon_channel_multiplies_supported_cooldown_recovery_while_active() {
    let mut channel = ability(DpsAbilityKind::WeaponArcaneChannel, 0.0);
    channel.cast_time_ms = 1_000;
    channel.cooldown_ms = 180_000;
    channel.cooldown_scales_with_cooldown_recovery = false;
    channel.channel = Some(ChannelModel {
        duration_ms: 3_000,
        tick_interval_ms: 1_500,
        tick_immediately: true,
        scale_duration_with_ability_time_rate: false,
        enable_partial_ticks: true,
        scale_partial_tick_damage: true,
    });
    channel
        .mechanic_parameters
        .insert("channelCooldownRecoveryMultiplier".into(), 8.0);
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 0.0);
    fire_ball.cooldown_ms = 30_000;
    fire_ball.cooldown_scales_with_cooldown_recovery = true;
    let profile = profile(vec![channel, fire_ball]);
    let apl = apl([("weapon-arcane-channel", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 30_000.0,
            used_charges: 1,
        },
    );

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 4_000);
    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::FireBall].remaining_ms,
        5_000.0
    );
    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::WeaponArcaneChannel].remaining_ms,
        177_000.0
    );
}

#[test]
fn frost_volley_applies_its_non_refreshing_dot_after_the_final_damage_volley() {
    let mut weapon = dot_ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.power_coefficient = 1.0;
    weapon.direct_hits = 3;
    weapon.first_hit_delay_ms = 250;
    weapon.hit_interval_ms = 1_000;
    weapon.scale_time_with_haste = false;
    weapon.off_gcd = true;
    let profile = profile(vec![weapon]);
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);

    iteration.cast(0);
    iteration.process_events_through(2_249);
    assert!(
        !iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::WeaponFrostVolley)))
    );

    iteration.process_events_through(2_250);
    let first =
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::WeaponFrostVolley))].clone();
    assert_eq!(first.started_ms, 2_250);
    assert_eq!(first.next_tick_ms, 3_250);

    iteration.common.now_ms = 2_500;
    let ability = profile.abilities[0].clone();
    iteration.apply_dot(
        0,
        ability.kind,
        &compiled_ability(&ability),
        ability.dot.expect("frost DoT"),
        DamageContext::for_cast(2),
    );
    let retained =
        &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::WeaponFrostVolley))];
    assert_eq!(retained.generation, first.generation);
    assert_eq!(retained.started_ms, first.started_ms);
    assert_eq!(retained.next_tick_ms, first.next_tick_ms);
}

#[test]
fn chain_lightning_preserves_first_link_scaling_target_cap_and_jump_timing() {
    let mut weapon = ability(DpsAbilityKind::WeaponChainLightning, 1.0);
    weapon.off_gcd = true;
    weapon.max_targets = 4;
    weapon.mechanic_parameters = BTreeMap::from([
        ("firstTargetDamageMultiplier".into(), 2.0),
        ("jumpDelaySeconds".into(), 0.15),
    ]);
    let profile = profile(vec![weapon]);
    let apl = apl([("weapon-chain-lightning", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 6, 5);

    iteration.cast(0);

    assert_eq!(
        iteration.common.result.targets,
        vec![200.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
    iteration.process_events_through(149);
    assert_eq!(
        iteration.common.result.targets,
        vec![200.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
    iteration.process_events_through(450);
    assert_eq!(
        iteration.common.result.targets,
        vec![200.0, 100.0, 100.0, 100.0, 0.0, 0.0]
    );
    let result = &iteration.ability_totals("test:weapon-chain-lightning");
    assert_eq!(result.hits, 4);
    assert_eq!(result.damage, 500.0);
}

#[test]
fn shadow_mark_pops_at_its_live_primary_stat_cap_and_can_crit() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.mechanic_parameters = BTreeMap::from([
        ("accumulationFraction".into(), 0.5),
        ("maximumPowerCoefficient".into(), 1.0),
    ]);
    let mut profile = profile(vec![mark]);
    profile.critical_strike = 1.0;
    let apl = apl([("weapon-shadow-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.shared.shadow_marks.insert(
        0,
        ShadowMarkState {
            generation: 1,
            until_ms: 15_000,
            accumulated_damage: 0.0,
            context: DamageContext::for_cast(1),
        },
    );

    iteration.damage_unscaled_key(
        None,
        iteration.test_damage_source(&profile.abilities[0].id),
        200.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );

    assert!(!iteration.shared.shadow_marks.contains_key(&0));
    assert_eq!(iteration.common.result.targets, vec![400.0]);
    assert_eq!(iteration.ability_totals("test:weapon-shadow-mark").crits, 1);
}

#[test]
fn shadow_mark_recast_explodes_the_old_mark_before_projectile_arrival() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.off_gcd = true;
    mark.first_hit_delay_ms = 300;
    mark.effect_duration_ms = 15_000;
    mark.mechanic_parameters = BTreeMap::from([
        ("accumulationFraction".into(), 0.1),
        ("maximumPowerCoefficient".into(), 42.5),
    ]);
    let profile = profile(vec![mark]);
    let apl = apl([("weapon-shadow-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.shared.shadow_marks.insert(
        0,
        ShadowMarkState {
            generation: 7,
            until_ms: 15_000,
            accumulated_damage: 50.0,
            context: DamageContext::for_cast(1),
        },
    );

    iteration.cast(0);
    assert!(!iteration.shared.shadow_marks.contains_key(&0));
    assert_eq!(iteration.common.result.targets, vec![50.0]);

    iteration.process_events_through(300);
    assert!(iteration.shared.shadow_marks.contains_key(&0));
}

#[test]
fn agonizing_blaze_refresh_keeps_stacks_and_the_application_proc_snapshot() {
    let source = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    let mut profile = profile(vec![source]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent3".into(),
            name: "Agonizing Blaze".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-agonizingblaze".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("damagePerStack".into(), 0.04),
                ("maximumStacks".into(), 10.0),
            ]),
            reason: None,
        });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let ability = profile.abilities[0].clone();
    let model = ability.dot.expect("dot model");
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        model,
        DamageContext::for_cast(1),
    );
    let original_generation = {
        let active = iteration
            .common
            .dots
            .get_mut(&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze)))
            .expect("active dot");
        active.stacks = 6;
        active.bonus_crit = 1.0;
        active.critical_chance_override = Some(1.0);
        active.generation
    };

    iteration.common.now_ms = 400;
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        model,
        DamageContext::for_cast(2),
    );

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.generation, original_generation);
    assert_eq!(active.stacks, 6);
    assert_eq!(active.bonus_crit, 1.0);
    assert_eq!(active.critical_chance_override, Some(1.0));
    assert_eq!(active.next_tick_ms, 1_000);
    assert_eq!(active.expires_ms, 10_400);
}

#[test]
fn crash_and_burn_uses_the_effect_specific_exact_damage_event() {
    for (agonizing_blaze, critical, coefficient, expected_remaining_ms) in [
        (false, false, 0.1, 8_950.0),
        (false, true, 0.1, 9_000.0),
        (true, false, 0.1, 9_000.0),
        (true, true, 0.1, 8_950.0),
        (false, false, 0.0, 9_000.0),
        (true, true, 0.0, 9_000.0),
    ] {
        let mut searing = dot_ability(DpsAbilityKind::SearingBlaze, coefficient);
        searing.dot.as_mut().expect("dot model").can_crit = true;
        let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
        fire_ball.cooldown_ms = 10_000;
        let mut profile = profile(vec![searing, fire_ball]);
        profile.critical_strike = if critical { 1.0 } else { 0.0 };
        profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent7".into(),
            name: "Ardeos talent 7".into(),
            mechanic_id: "test:crash-and-burn".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([("cooldownReductionSeconds".into(), 0.05)]),
            reason: None,
        });
        if agonizing_blaze {
            profile.talents.push(DpsTalentModel {
                id: "firemage-talent-id-talent3".into(),
                name: "Agonizing Blaze".into(),
                mechanic_id: "test:agonizing-blaze".into(),
                classification: MechanicClassification::Modeled,
                parameters: BTreeMap::from([
                    ("damagePerStack".into(), 0.04),
                    ("maximumStacks".into(), 10.0),
                ]),
                reason: None,
            });
        }
        let apl = apl([("searing-blaze", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 5);
        let searing = profile.abilities[0].clone();
        iteration.apply_dot(
            0,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(&searing),
            searing.dot.expect("dot model"),
            DamageContext::for_cast(1),
        );
        iteration.common.cooldowns.insert(
            DpsAbilityKind::FireBall,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );

        iteration.process_events_through(1_000);

        assert_eq!(
            iteration.common.cooldowns[&DpsAbilityKind::FireBall].remaining_ms,
            expected_remaining_ms,
            "agonizing_blaze={agonizing_blaze}, critical={critical}",
        );
    }
}

#[test]
fn rolling_flames_requires_actual_damage_from_each_source() {
    for damage_coefficient in [0.0, 0.1] {
        let searing = dot_ability(DpsAbilityKind::SearingBlaze, damage_coefficient);
        let wave = ability(DpsAbilityKind::InfernalWave, damage_coefficient);
        let mut engulfing = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
        engulfing.cooldown_ms = 10_000;
        let mut profile = profile(vec![searing.clone(), wave, engulfing]);
        profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent8".into(),
            name: "Rolling Flames".into(),
            mechanic_id: "test:rolling-flames".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("searingBlazeReductionSeconds".into(), 0.25),
                ("infernalWaveReductionSeconds".into(), 1.0),
            ]),
            reason: None,
        });
        let apl = apl([("infernal-wave", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 5);
        iteration.common.cooldowns.insert(
            DpsAbilityKind::EngulfingFlames,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );
        iteration.impact(1, false, 1.0, 0, DamageContext::for_cast(1));
        let wave_reduction = if damage_coefficient > 0.0 {
            1_000.0
        } else {
            0.0
        };
        assert_eq!(
            iteration.common.cooldowns[&DpsAbilityKind::EngulfingFlames].remaining_ms,
            10_000.0 - wave_reduction
        );
        iteration.apply_dot(
            0,
            searing.kind,
            &compiled_ability(&searing),
            searing.dot.expect("dot"),
            DamageContext::for_cast(2),
        );
        iteration.process_events_through(1_000);
        let tick_reduction = if damage_coefficient > 0.0 { 250.0 } else { 0.0 };
        assert_eq!(
            iteration.common.cooldowns[&DpsAbilityKind::EngulfingFlames].remaining_ms,
            9_000.0 - wave_reduction - tick_reduction
        );
    }
}

#[test]
fn agonizing_blaze_uses_the_cooked_total_stack_limit_and_scale() {
    let source = dot_ability(DpsAbilityKind::SearingBlaze, 0.1);
    let mut profile = profile(vec![source]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent3".into(),
            name: "Agonizing Blaze".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-agonizingblaze".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("damagePerStack".into(), 0.04),
                ("maximumStacks".into(), 10.0),
            ]),
            reason: None,
        });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    let ability = profile.abilities[0].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&ability),
        ability.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );

    iteration.process_events_through(9_000);

    let active = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(active.stacks, 10);
    assert!(
        (iteration.dot_raw_damage_per_tick(DotKind::Ability(ability.kind), active) - 13.6).abs()
            < 1e-10
    );
}

#[test]
fn firestarter_applies_to_firemage_dots_but_not_weapon_dots() {
    let mut weapon = dot_ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.dot.as_mut().expect("dot model").can_crit = true;
    let mut searing = dot_ability(DpsAbilityKind::SearingBlaze, 1.0);
    searing.dot.as_mut().expect("dot model").can_crit = true;
    let mut profile = profile(vec![weapon, searing]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent11".into(),
            name: "Ardeos talent 11".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent2".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([("dotCriticalStrikeBonus".into(), 1.0)]),
            reason: None,
        });
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 5);
    for (target, index) in [0_usize, 1].into_iter().enumerate() {
        let ability = profile.abilities[index].clone();
        iteration.apply_dot(
            target as u32,
            ability.kind,
            &compiled_ability(&ability),
            ability.dot.expect("dot model"),
            DamageContext::for_cast(index as u64 + 1),
        );
    }

    iteration.process_events_through(1_000);

    assert_eq!(iteration.common.result.targets, vec![100.0, 200.0]);
}

#[test]
fn intrepid_consumes_basic_stacks_into_only_the_committed_power_cast() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let power = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![basic.clone(), power.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.01",
        [("maximumStacks", 5.0), ("damageIncreasePerStack", 0.1)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);

    for cast_id in 1..=5 {
        iteration
            .trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::for_cast(cast_id));
    }
    let power_context =
        iteration.trigger_dynamic_on_cast(&compiled_ability(&power), DamageContext::for_cast(6));

    assert_eq!(
        iteration.test_dynamic_counter("DynamicItemAbilityRank.01"),
        0
    );
    assert_eq!(power_context.power_damage_multiplier, 1.5);
    assert_eq!(
        iteration
            .damage_hit(
                &compiled_ability(&power),
                100.0,
                0.0,
                false,
                0,
                power_context,
            )
            .damage,
        150.0,
    );

    assert_eq!(
        iteration
            .damage_hit(
                &compiled_ability(&power),
                100.0,
                0.0,
                false,
                0,
                DamageContext::for_cast(7),
            )
            .damage,
        100.0,
    );
}

#[test]
fn vehement_copies_the_single_target_basic_before_registration_ordered_fanout() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![basic.clone()]);
    profile.expertise = 0.5;
    profile.critical_strike = 0.5;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.06",
        [
            ("hitThreshold", 1.0),
            ("criticalStrikeCap", 0.5),
            ("targetCountDamageScalingThreshold", 5.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 9);
    iteration.common.actor_registration_order.enemy_targets = vec![1, 0];
    let context = DamageContext::for_cast(1);
    let triggering_hit =
        iteration.damage_hit(&compiled_ability(&basic), 100.0, 0.0, false, 0, context);
    iteration.damage_hit(
        &compiled_ability(&basic),
        300.0,
        0.0,
        false,
        1,
        context.without_cast_proc(),
    );

    assert_eq!(triggering_hit.damage, 150.0);
    assert_eq!(iteration.common.result.targets, vec![375.0, 675.0]);
    let proc = &iteration.ability_totals("gear:DynamicItemAbilityRank.06");
    assert_eq!(proc.damage, 450.0);
    assert_eq!(proc.hits, 2);
}

#[test]
fn vainglorious_modifies_original_direct_and_periodic_basic_damage() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![basic.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.09",
        [("powerCoefficient", 0.5), ("periodicPowerCoefficient", 0.2)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 10);
    let context = DamageContext::for_cast(1);
    let direct = iteration.damage_hit(&compiled_ability(&basic), 100.0, 0.0, false, 0, context);
    let periodic = iteration.damage_periodic(
        Some(basic.kind),
        iteration.test_damage_source(&basic.id),
        100.0,
        0.0,
        1.0,
        0.0,
        0.0,
        false,
        0,
        context,
    );

    assert_eq!(direct.damage, 150.0);
    assert_eq!(periodic.damage, 120.0);
    assert_eq!(iteration.ability_totals(&basic.id).damage, 270.0);
    assert!(!iteration.has_ability_totals("gear:DynamicItemAbilityRank.09"));
}

#[test]
fn damage_blessings_modify_resolved_hits_after_both_rounding_boundaries() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let power = ability(DpsAbilityKind::Detonate, 1.0);
    let engulfing = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
    let mut profile = profile(vec![basic.clone(), power.clone(), engulfing.clone()]);
    profile.spirit = 0.33;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.09",
        [
            ("powerCoefficient", 0.005),
            ("periodicPowerCoefficient", 0.002),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.10",
        [
            ("damageIncreasePerSpiritAmount", 0.003),
            ("spiritAmount", 0.03),
            ("spiritCap", 0.5),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "legendary-wrists-test",
        [("engulfingTargetIncomingMultiplier", 1.07)],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 11);
    iteration.apply_dot(
        0,
        engulfing.kind,
        &compiled_ability(&engulfing),
        engulfing.dot.expect("dot"),
        DamageContext::for_cast(1),
    );

    // Vainglorious adds 0.5 after round(round(9.49) * 1.07) = 10.
    let direct = iteration.damage_hit(
        &compiled_ability(&basic),
        9.49,
        0.0,
        false,
        0,
        DamageContext::for_cast(2),
    );
    assert_eq!(direct.damage, 10.5);
    let periodic = iteration.damage_periodic(
        Some(basic.kind),
        iteration.test_damage_source(&basic.id),
        9.49,
        0.0,
        1.0,
        0.0,
        0.0,
        false,
        0,
        DamageContext::for_cast(3),
    );
    assert!((periodic.damage - 10.2).abs() < 1e-12);

    let zero = iteration.damage_hit(
        &compiled_ability(&basic),
        0.49,
        0.0,
        false,
        0,
        DamageContext::for_cast(5),
    );
    assert_eq!(zero.damage, 0.0);

    // Intrepid and Mystic multiply the resolved value and preserve fractions.
    let mut context = DamageContext::for_cast(4);
    context.multiply_power_damage(1.25);
    let direct = iteration.damage_hit(&compiled_ability(&power), 10.49, 0.0, false, 0, context);
    assert!((direct.damage - 11.0 * 1.25 * 1.033).abs() < 1e-12);
    // Detonate bypasses the source-stat calculation but still invokes the
    // same post-execution Power callbacks.
    let detonate = iteration.damage_detonate(&compiled_ability(&power), 10.49, 0.0, 0, context);
    assert!((detonate.damage - 11.0 * 1.25 * 1.033).abs() < 1e-12);
}

#[test]
fn spirit_blessings_use_continuous_cooked_magnitude_formulas() {
    let power = ability(DpsAbilityKind::Detonate, 1.0);
    let major = ability(DpsAbilityKind::Apocalypse, 0.0);
    let mut profile = profile(vec![power.clone(), major.clone()]);
    profile.spirit = 0.33;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.08",
        [
            ("durationSeconds", 8.0),
            ("statIncreasePerSpirit", 0.002),
            ("spiritPerIncrease", 4.0),
            ("spiritCap", 0.5),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.10",
        [
            ("damageIncreasePerSpiritAmount", 0.003),
            ("spiritAmount", 0.03),
            ("spiritCap", 0.5),
        ],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 11);
    iteration.trigger_dynamic_on_cast(&compiled_ability(&major), DamageContext::NONE);

    assert!((iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")) - 0.0165).abs() < 1e-12);
    assert!((iteration.resolved_power_blessing_multiplier() - 1.033).abs() < 1e-12);
}

#[test]
fn monarch_caps_haste_before_applying_its_exact_acceleration_rate() {
    let major = ability(DpsAbilityKind::Apocalypse, 0.0);
    let mut profile = profile(vec![major.clone()]);
    profile.haste = 0.8;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.11",
        [
            ("cooldownAcceleration", 0.03),
            ("accelerationPerHaste", 0.1),
            ("hasteThreshold", 0.5),
        ],
    ));
    let apl = apl([("apocalypse", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 12);

    assert!((iteration.cooldown_recovery_rate(major.kind) - 1.08).abs() < 1e-12);
}

#[test]
fn monarch_rate_bonus_is_separate_from_recovery_attribute_multipliers() {
    let mut major = ability(DpsAbilityKind::Apocalypse, 0.0);
    major.cooldown_scales_with_cooldown_recovery = true;
    let mut profile = profile(vec![major.clone()]);
    profile.haste = 0.8;
    profile.cooldown_recovery = 1.2;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.11",
        [
            ("cooldownAcceleration", 0.03),
            ("accelerationPerHaste", 0.1),
            ("hasteThreshold", 0.5),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.CooldownRecoveryOnWeaponAbility",
        [("cooldownAccelerationMultiplier", 2.0)],
    ));
    let apl = apl([("apocalypse", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 12);
    let recovery = Arc::clone(&iteration.profile.mechanics[1]);
    iteration.activate_dynamic_buff(&recovery, 2_000, 1, 0.0);
    assert!((iteration.effective_cooldown_recovery() - 2.2).abs() < 1e-12);
    assert!((iteration.cooldown_recovery_rate(major.kind) - 2.28).abs() < 1e-12);
    iteration.common.now_ms = 2_000;
    assert!((iteration.cooldown_recovery_rate(major.kind) - 1.28).abs() < 1e-12);
}

#[test]
fn wayfarer_core_commits_reduce_the_live_recursive_cycle() {
    let mut core = ability(DpsAbilityKind::FireBall, 0.0);
    core.cooldown_ms = 30_000;
    let mut profile = profile(vec![core.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.12",
        [
            ("intervalSeconds", 60.0),
            ("durationSeconds", 14.0),
            ("hasteBonus", 0.2),
            ("coreCooldownDenominatorSeconds", 30.0),
            ("coreCooldownFractionMultiplier", 4.0),
            ("minimumReductionSeconds", 0.2),
        ],
    ));
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 13);

    assert_eq!(iteration.shared.wayfarer_next_ms, 60_000);
    iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
    assert_eq!(iteration.shared.wayfarer_next_ms, 56_000);
    iteration.process_events_through(55_999);
    assert_eq!(
        iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")),
        0.0
    );
    iteration.process_events_through(56_000);
    assert_eq!(
        iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")),
        0.2
    );
    assert_eq!(iteration.shared.wayfarer_next_ms, 116_000);
}

#[test]
fn usurper_samples_unique_targets_without_replacement() {
    let core = ability(DpsAbilityKind::FireBall, 0.0);
    let mut profile = profile(vec![core.clone()]);
    profile.critical_strike = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.07",
        [("powerCoefficient", 1.0), ("maximumTargets", 4.0)],
    ));
    let apl = apl([("fire-ball", None)]);
    let mut selections = BTreeSet::new();

    for seed in 1..=8 {
        let mut iteration = Iteration::new(&profile, &apl, 6, seed);
        iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
        let selected = iteration
            .common
            .result
            .targets
            .iter()
            .enumerate()
            .filter_map(|(target, damage)| (*damage > 0.0).then_some(target as u32))
            .collect::<BTreeSet<_>>();
        assert_eq!(selected.len(), 4);
        selections.insert(selected);
    }

    assert!(selections.len() > 1);
}

#[test]
fn heretic_uses_empty_preset_resolved_primary_stat_damage() {
    let core = ability(DpsAbilityKind::FireBall, 0.0);
    let mut profile = profile(vec![core.clone()]);
    profile.expertise = 1.0;
    profile.critical_strike = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.14",
        [("procChance", 1.0), ("powerCoefficient", 2.0)],
    ));
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 14);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);

    let proc = &iteration.ability_totals("gear:DynamicItemAbilityRank.14");
    assert_eq!(proc.damage, 200.0);
    assert_eq!(proc.crits, 0);
}

#[test]
fn first_strike_buffs_separately_created_specs_after_each_targets_first_hit() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "first-strike".into(),
        source_id: "gem-emerald-80".into(),
        source_name: "First Strike".into(),
        mechanic_id: "test:first-strike".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("expertise".into(), 0.5), ("durationSeconds".into(), 15.0)]),
        reason: None,
    });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 6);

    let first = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    let second = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    assert_eq!(first.damage, 100.0);
    assert_eq!(second.damage, 150.0);

    iteration.common.now_ms = 16_000;
    let already_touched = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    let new_target = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        1,
        DamageContext::NONE,
    );
    let refreshed = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );

    assert_eq!(already_touched.damage, 100.0);
    assert_eq!(new_target.damage, 100.0);
    assert_eq!(refreshed.damage, 150.0);
    assert_eq!(iteration.common.result.targets, vec![500.0, 100.0]);
    assert_eq!(iteration.test_dynamic_buff("first-strike").until_ms, 31_000);
}

#[test]
fn first_strike_uses_one_expertise_snapshot_for_a_multi_target_effect() {
    let mut infernal_wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    infernal_wave.max_targets = 2;
    let mut profile = profile(vec![infernal_wave]);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "first-strike".into(),
        source_id: "gem-emerald-80".into(),
        source_name: "First Strike".into(),
        mechanic_id: "test:first-strike".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("expertise".into(), 0.5), ("durationSeconds".into(), 15.0)]),
        reason: None,
    });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 6);

    iteration.cast(0);
    assert_eq!(iteration.common.result.targets, vec![100.0, 100.0]);

    iteration.cast(0);
    assert_eq!(iteration.common.result.targets, vec![250.0, 250.0]);
}

#[test]
fn first_strike_changes_the_next_recaptured_firemage_periodic_execution() {
    let mut profile = profile(vec![dot_ability(DpsAbilityKind::SearingBlaze, 1.0)]);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "first-strike".into(),
        source_id: "gem-emerald-80".into(),
        source_name: "First Strike".into(),
        mechanic_id: "test:first-strike".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("expertise".into(), 0.5), ("durationSeconds".into(), 15.0)]),
        reason: None,
    });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);

    iteration.cast(0);
    iteration.process_events_through(2_000);

    assert_eq!(iteration.common.result.targets, vec![250.0]);
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))]
            .expertise_snapshot,
        0.5
    );
}

#[test]
fn ancestral_surge_primary_stat_tracks_spirit_of_heroism() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "ancestral-surge".into(),
        source_id: "gem-sapphire-600".into(),
        source_name: "Ancestral Surge II".into(),
        mechanic_id: "test:ancestral-surge".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("heroismPowerMultiplier".into(), 1.24)]),
        reason: None,
    });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);

    let before = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.activate_heroism();
    let during = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = iteration.shared.heroism_until;
    let after = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );

    assert_eq!(before.damage, 100.0);
    assert_eq!(during.damage, 124.0);
    assert_eq!(after.damage, 100.0);
}

#[test]
fn drakheims_absolution_expires_independently_of_extended_heroism() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.heroism_duration_ms = 38_000;
    profile.mechanics.push(ardeos_mechanic(
        "gem-sapphire-600",
        [("heroismPowerMultiplier", 1.24)],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "setd-proc-intellect",
        [("heroismPowerMultiplier", 1.2), ("durationSeconds", 20.0)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);

    iteration.activate_heroism();
    let during_both = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 21_000;
    let during_heroism = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 39_000;
    let after = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );

    assert!((during_both.damage - 149.0).abs() < 1e-9);
    assert_eq!(during_heroism.damage, 124.0);
    assert_eq!(after.damage, 100.0);
}

#[test]
fn commit_created_damage_specs_retain_their_primary_stat_snapshot() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.off_gcd = true;
    fire_ball.first_hit_delay_ms = 2_000;
    let mut profile = profile(vec![fire_ball]);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "ancestral-surge".into(),
        source_id: "gem-sapphire-600".into(),
        source_name: "Ancestral Surge II".into(),
        mechanic_id: "test:ancestral-surge".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("heroismPowerMultiplier".into(), 1.24)]),
        reason: None,
    });
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    iteration.shared.heroism_until = 1_000;

    iteration.cast(0);
    iteration.process_events_through(2_000);

    assert_eq!(iteration.common.result.targets, vec![124.0]);
}

#[test]
fn off_gcd_abilities_with_cast_times_hold_the_cast_lock_until_commit() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.off_gcd = true;
    fire_ball.cast_time_ms = 700;
    let profile = profile(vec![fire_ball]);
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 700);
    assert_eq!(iteration.ability_totals("test:fire-ball").casts, 1);
}

#[test]
fn weapon_commit_traits_use_the_authored_default_cooldown() {
    let mut weapon = ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.cooldown_ms = 8_000;
    weapon
        .mechanic_parameters
        .insert("defaultCooldownMs".into(), 10_000.0);
    let spirit_ability = ability(DpsAbilityKind::Incinerate, 0.0);
    let mut profile = profile(vec![weapon.clone(), spirit_ability.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.CooldownRecoveryOnWeaponAbility",
        [
            ("cooldownAccelerationMultiplier", 1.2),
            ("durationWeaponCooldownFraction", 0.16),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease",
        [
            ("powerMultiplier", 1.1),
            ("durationWeaponCooldownFraction", 0.32),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.WeaponAndSpiritPoints",
        [
            ("spiritCooldownMultiplier", 2.5),
            ("spiritCooldownDivider", 30.0),
            ("weaponCooldownReductionFraction", 0.5),
        ],
    ));
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 17);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&weapon), DamageContext::NONE);

    assert_eq!(
        iteration
            .test_dynamic_buff("ItemTrait.ID.CooldownRecoveryOnWeaponAbility")
            .until_ms,
        1_600
    );
    assert_eq!(
        iteration
            .test_dynamic_buff("ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease")
            .until_ms,
        3_200
    );
    assert!((iteration.shared.spirit - 2.5 / 3.0).abs() < 1e-12);

    iteration.common.cooldowns.insert(
        weapon.kind,
        CooldownState {
            used_charges: 1,
            remaining_ms: 8_000.0,
        },
    );
    iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit_ability), DamageContext::NONE);
    assert_eq!(
        iteration.common.cooldowns[&weapon.kind].remaining_ms,
        4_000.0
    );
}

#[test]
fn brave_machinations_delays_one_base_cooldown_reduction_per_weapon_commit() {
    let mut weapon = ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    weapon.cooldown_ms = 10_000;
    let mut profile = profile(vec![weapon.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.WeaponCritChanceCooldownReduction",
        [
            ("weaponCooldownReductionPerCrit", 0.5),
            ("maximumCriticalReductionsPerCommit", 1.0),
            ("cooldownReductionDelaySeconds", 0.33333),
        ],
    ));
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 18);
    iteration.common.cooldowns.insert(
        weapon.kind,
        CooldownState {
            used_charges: 1,
            remaining_ms: 10_000.0,
        },
    );
    iteration.trigger_dynamic_on_cast(&compiled_ability(&weapon), DamageContext::NONE);

    iteration.trigger_weapon_critical_cooldown_reduction(weapon.kind);
    iteration.trigger_weapon_critical_cooldown_reduction(weapon.kind);
    iteration.process_events_through(332);
    assert_eq!(
        iteration.common.cooldowns[&weapon.kind].remaining_ms,
        9_668.0
    );
    iteration.process_events_through(333);

    assert_eq!(
        iteration.common.cooldowns[&weapon.kind].remaining_ms,
        4_667.0
    );
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.WeaponCritChanceCooldownReduction"),
        1.0
    );
}

#[test]
fn seized_opportunity_stops_counting_critical_hits_while_active() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.CritsToIncreasedCritRating",
        [
            ("criticalRating", 20.0),
            ("durationSeconds", 12.0),
            ("requiredCriticalStrikes", 2.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("test"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("test"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("test"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );

    assert_eq!(
        iteration.test_dynamic_counter("ItemTrait.ID.CritsToIncreasedCritRating"),
        0
    );
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
        20.0
    );
    iteration.common.now_ms = 12_000;
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("test"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );
    assert_eq!(
        iteration.test_dynamic_counter("ItemTrait.ID.CritsToIncreasedCritRating"),
        1
    );
}

#[test]
fn secondary_rating_buffs_combine_before_diminishing_returns() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.haste_rating = 60.0;
    profile.haste = secondary_rating_percentage(profile.haste_rating);
    profile.mechanics.push(ardeos_mechanic(
        "rating-a",
        [("hasteRating", 40.0), ("durationSeconds", 20.0)],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "rating-b",
        [("hasteRating", 40.0), ("durationSeconds", 20.0)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 21);
    for index in 0..iteration.profile.mechanics.len() {
        let mechanic = Arc::clone(&iteration.profile.mechanics[index]);
        iteration.activate_dynamic_buff(&mechanic, 20_000, 1, 0.0);
    }

    assert!((iteration.effective_haste() - secondary_rating_percentage(140.0)).abs() < 1e-12);
}

#[test]
fn hidden_power_expires_stacks_and_pauses_rolls_during_its_buff() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![basic.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.AbilityToIncreasedMainStat",
        [
            ("powerMultiplier", 1.2),
            ("durationSeconds", 10.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("requiredStacks", 2.0),
            ("stackDurationSeconds", 1.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 22);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::NONE);
    assert_eq!(
        iteration.test_dynamic_counter("ItemTrait.ID.AbilityToIncreasedMainStat"),
        1
    );
    iteration.common.now_ms = 1_500;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::NONE);
    assert_eq!(
        iteration.test_dynamic_counter("ItemTrait.ID.AbilityToIncreasedMainStat"),
        1
    );
    iteration.common.now_ms = 1_600;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::NONE);
    assert_eq!(
        iteration
            .test_dynamic_buff("ItemTrait.ID.AbilityToIncreasedMainStat")
            .until_ms,
        11_600
    );
    iteration.common.now_ms = 1_700;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::NONE);
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.AbilityToIncreasedMainStat"),
        3.0
    );
}

#[test]
fn navigators_intuition_selects_the_live_highest_raw_rating() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![basic.clone()]);
    profile.critical_rating = 10.0;
    profile.expertise_rating = 20.0;
    profile.haste_rating = 30.0;
    profile.spirit_rating = 25.0;
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.OffensiveAbilityToHighestStatBuff",
        [
            ("secondaryRating", 40.0),
            ("durationSeconds", 30.0),
            ("procChance", 1.0),
            ("cooldownSeconds", 90.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 23);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&basic), DamageContext::NONE);

    assert_eq!(
        iteration
            .test_dynamic_buff("ItemTrait.ID.OffensiveAbilityToHighestStatBuff")
            .value,
        2.0
    );
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("hasteRating")),
        40.0
    );
    assert_eq!(
        iteration.test_dynamic_ready_ms("ItemTrait.ID.OffensiveAbilityToHighestStatBuff"),
        90_000
    );
}

#[test]
fn navigators_intuition_uses_native_map_order_for_ties() {
    let basic = ability(DpsAbilityKind::InfernalWave, 1.0);
    let mut profile = profile(vec![basic]);
    profile.critical_rating = 20.0;
    profile.expertise_rating = 20.0;
    profile.haste_rating = 20.0;
    profile.spirit_rating = 20.0;
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 24);
    let mut selected_counts = [0_u32; 4];

    for _ in 0..20_000 {
        selected_counts[usize::from(iteration.highest_secondary_rating_code())] += 1;
    }

    // Sequential fair replacement over Crit, Expertise, Haste, Spirit
    // yields probabilities 1/8, 1/8, 1/4, and 1/2 respectively.
    for (actual, expected) in selected_counts
        .into_iter()
        .zip([2_500, 2_500, 5_000, 10_000])
    {
        assert!(actual.abs_diff(expected) < 350, "{selected_counts:?}");
    }
}

#[test]
fn hunters_focus_only_stacks_on_targeted_offensive_commits_and_refreshes() {
    let targeted = ability(DpsAbilityKind::InfernalWave, 1.0);
    let detonate = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![targeted.clone(), detonate.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.OffensiveAbilityHasteRatingStacking",
        [
            ("hasteRating", 4.0),
            ("durationSeconds", 8.0),
            ("maximumStacks", 5.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 25);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&detonate), DamageContext::NONE);
    assert!(iteration.shared.dynamic_buffs.iter().all(Option::is_none));
    for _ in 0..6 {
        iteration.trigger_dynamic_on_cast(&compiled_ability(&targeted), DamageContext::NONE);
    }
    let buff = iteration.test_dynamic_buff("ItemTrait.ID.OffensiveAbilityHasteRatingStacking");
    assert_eq!(buff.stacks, 5);
    assert_eq!(buff.until_ms, 8_000);
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("hasteRating")),
        20.0
    );

    iteration.common.now_ms = 7_000;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&targeted), DamageContext::NONE);
    let refreshed = iteration.test_dynamic_buff("ItemTrait.ID.OffensiveAbilityHasteRatingStacking");
    assert_eq!(refreshed.stacks, 5);
    assert_eq!(refreshed.until_ms, 15_000);
}

#[test]
fn amethyst_splinters_converts_each_crit_contribution_into_four_periodic_ticks() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemDotHotOnCrit",
        [
            ("triggerDamageFraction", 0.1),
            ("durationSeconds", 8.0),
            ("periodSeconds", 2.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 28);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("critical-hit"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );
    iteration.process_events_through(7_999);
    assert_eq!(iteration.common.result.targets, vec![9.0]);
    iteration.process_events_through(8_000);

    assert_eq!(iteration.common.result.targets, vec![12.0]);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemDotHotOnCrit")
            .crits,
        0
    );
}

#[test]
fn amethyst_splinters_refresh_preserves_phase_and_folds_remaining_damage_forward() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemDotHotOnCrit",
        [
            ("triggerDamageFraction", 0.1),
            ("durationSeconds", 8.0),
            ("periodSeconds", 2.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 29);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("first-crit"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 1_000;
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("second-crit"),
        100.0,
        true,
        0,
        DamageContext::NONE,
    );

    let state = iteration.test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0);
    assert_eq!(state.next_tick_ms, 2_000);
    assert_eq!(state.until_ms, 9_000);
    assert!((state.damage_per_tick - 4.6875).abs() < 1e-12);

    iteration.process_events_through(8_000);
    assert_eq!(iteration.common.result.targets, vec![20.0]);
    iteration.process_events_through(9_000);
    assert_eq!(iteration.common.result.targets, vec![20.0]);
}

#[test]
fn kindling_rolls_on_harmful_effect_application_not_periodic_execution() {
    let mut profile = profile(vec![dot_ability(DpsAbilityKind::SearingBlaze, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 30);
    let searing = profile.abilities[0].clone();

    iteration.apply_dot(
        0,
        searing.kind,
        &compiled_ability(&searing),
        searing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"),
        1.0
    );
    iteration.process_events_through(9_000);

    let kindling =
        &iteration.ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc");
    assert_eq!(kindling.hits, 6);
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"),
        1.0
    );
}

#[test]
fn kindling_refresh_preserves_its_period_phase() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 0.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 31);

    iteration.trigger_kindling_on_harmful_effect_application(
        Some(DpsAbilityKind::InfernalWave),
        iteration.test_damage_source("first-effect"),
        0,
    );
    iteration.common.now_ms = 500;
    iteration.trigger_kindling_on_harmful_effect_application(
        Some(DpsAbilityKind::InfernalWave),
        iteration.test_damage_source("refreshed-effect"),
        0,
    );

    let state = iteration.test_kindling("ItemTrait.ID.ExtraDotHotOnEffectApplicationProc", 0);
    assert_eq!(state.next_tick_ms, 1_500);
    assert_eq!(state.until_ms, 9_500);
    iteration.process_events_through(9_500);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc")
            .hits,
        6
    );
}

#[test]
fn kindling_rolls_for_detonates_controller_and_each_damage_effect() {
    let mut profile = profile(vec![
        ability(DpsAbilityKind::Detonate, 1.0),
        dot_ability(DpsAbilityKind::SearingBlaze, 1.0),
    ]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procChance", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 32);
    let searing = profile.abilities[1].clone();
    iteration.apply_dot(
        0,
        searing.kind,
        &compiled_ability(&searing),
        searing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    let before = iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc");

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    iteration.process_events_through(1_100);

    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc") - before,
        4.0
    );
}

#[test]
fn kindling_rolls_for_shadow_marks_harmful_accumulator() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.effect_duration_ms = 15_000;
    let mut profile = profile(vec![mark]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procChance", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("weapon-shadow-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 33);

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));

    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"),
        1.0
    );
}

#[test]
fn kindling_rolls_for_diamond_strikes_damage_and_amplifier() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemSingleTargetProcOnDamageHeal",
        [
            ("powerCoefficient", 1.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("debuffDurationSeconds", 20.0),
            ("damageIncreasePerStack", 0.4),
            ("maximumStacks", 5.0),
            ("harmoniousSoulDamageIncreasePerStack", 0.35),
            ("harmoniousSoulStacks", 0.0),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procChance", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 34);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("hero-hit"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );

    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"),
        2.0
    );
}

#[test]
fn kindling_rolls_for_engulfing_flames_and_devouring_flames_linked_debuff() {
    let mut profile = profile(vec![dot_ability(DpsAbilityKind::EngulfingFlames, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "legendary-wrists-test",
        [("engulfingTargetIncomingMultiplier", 1.07)],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
        [
            ("powerCoefficientPerTick", 1.0),
            ("procChance", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
        ],
    ));
    let apl = apl([("engulfing-flames", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 35);
    let engulfing = profile.abilities[0].clone();

    iteration.apply_dot(
        0,
        engulfing.kind,
        &compiled_ability(&engulfing),
        engulfing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );

    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"),
        2.0
    );
}

#[test]
fn sapphire_aurastone_accumulates_all_outgoing_damage_and_splits_each_pulse() {
    let spirit_ability = ability(DpsAbilityKind::Incinerate, 0.0);
    let mut profile = profile(vec![spirit_ability.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemPulsatingOnAbilityTotemProc",
        [
            ("damageAccumulationFraction", 0.12),
            ("initialDamagePulseDelaySeconds", 3.0),
            ("pulseIntervalSeconds", 3.0),
        ],
    ));
    let apl = apl([("incinerate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 26);
    iteration.shared.heroism_until = 10_000;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit_ability), DamageContext::NONE);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("hero-hit"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("gear:ItemTrait.ID.GemTargetedSpikeProc"),
        50.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.process_events_through(2_999);
    assert_eq!(iteration.common.result.targets, vec![0.0, 0.0]);
    iteration.process_events_through(3_000);

    assert_eq!(iteration.common.result.targets, vec![9.0, 9.0]);
    assert_eq!(
        iteration
            .test_aurastone("ItemTrait.ID.GemPulsatingOnAbilityTotemProc")
            .accumulated_damage,
        0.0
    );
}

#[test]
fn a_new_spirit_commit_replaces_aurastone_and_discards_its_store() {
    let spirit_ability = ability(DpsAbilityKind::Incinerate, 0.0);
    let mut profile = profile(vec![spirit_ability.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemPulsatingOnAbilityTotemProc",
        [
            ("damageAccumulationFraction", 0.12),
            ("initialDamagePulseDelaySeconds", 3.0),
            ("pulseIntervalSeconds", 3.0),
        ],
    ));
    let apl = apl([("incinerate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 27);
    iteration.shared.heroism_until = 10_000;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit_ability), DamageContext::NONE);
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("old-hit"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );

    iteration.common.now_ms = 1_000;
    iteration.shared.heroism_until = 11_000;
    iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit_ability), DamageContext::NONE);
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("new-hit"),
        50.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.process_events_through(3_000);
    assert_eq!(iteration.common.result.targets, vec![0.0]);
    iteration.process_events_through(4_000);

    assert_eq!(iteration.common.result.targets, vec![6.0]);
}

#[test]
fn diamond_strike_stacks_its_amplifier_per_target_and_can_crit() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.critical_strike = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemSingleTargetProcOnDamageHeal",
        [
            ("powerCoefficient", 1.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("debuffDurationSeconds", 20.0),
            ("damageIncreasePerStack", 0.4),
            ("maximumStacks", 5.0),
            ("harmoniousSoulDamageIncreasePerStack", 0.35),
            ("harmoniousSoulStacks", 0.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 24);

    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("hero-hit"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 1;
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("hero-hit"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 2;
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("hero-hit"),
        100.0,
        false,
        1,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 3;
    iteration.trigger_dynamic_on_damage(
        None,
        iteration.test_damage_source("gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal"),
        100.0,
        false,
        0,
        DamageContext::NONE,
    );

    assert_eq!(
        iteration
            .test_dynamic_target_buff("ItemTrait.ID.GemSingleTargetProcOnDamageHeal", 0)
            .stacks,
        2
    );
    assert_eq!(
        iteration
            .test_dynamic_target_buff("ItemTrait.ID.GemSingleTargetProcOnDamageHeal", 1)
            .stacks,
        1
    );
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal")
            .crits,
        3
    );
}

#[test]
fn ruby_storm_rolls_but_cannot_reach_ardeos_targets_at_maximum_range() {
    let targeted = ability(DpsAbilityKind::InfernalWave, 1.0);
    let non_targeted = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![targeted.clone(), non_targeted.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemWhirlwindProc",
        [
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("maxHealthDamageFraction", 0.07),
            ("lifetimeSeconds", 6.0),
            ("movementSpeed", 350.0),
            ("collisionRadius", 250.0),
        ],
    ));
    assert_eq!(
        ruby_storm_maximum_forward_overlap_distance(
            profile.mechanics.last().expect("Ruby Storm mechanic")
        ),
        Some(2_350.0)
    );
    assert!(
        ruby_storm_maximum_forward_overlap_distance(
            profile.mechanics.last().expect("Ruby Storm mechanic")
        )
        .expect("maximum reach")
            < ARDEOS_MAX_COMBAT_RANGE_UNITS
    );
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 32);

    iteration.trigger_dynamic_on_cast(&compiled_ability(&non_targeted), DamageContext::NONE);
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.GemWhirlwindProc"),
        0.0
    );

    iteration.trigger_dynamic_on_cast(&compiled_ability(&targeted), DamageContext::NONE);
    assert_eq!(
        iteration.test_uptime("proc:ItemTrait.ID.GemWhirlwindProc"),
        1.0
    );
    assert_eq!(iteration.common.result.damage, 0.0);
    assert!(!iteration.has_ability_totals("gear:ItemTrait.ID.GemWhirlwindProc"));
}

#[test]
fn stationary_and_weapon_modifier_traits_follow_their_exact_scenario_branches() {
    let weapon = ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    let mut profile = profile(vec![weapon.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease",
        [("expertiseRating", 20.0), ("applicationDelaySeconds", 3.0)],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.WeaponHealDamageIncrease",
        [("weaponDamageMultiplier", 1.8)],
    ));
    let apl = apl([("weapon-frost-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 20);

    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("expertiseRating")),
        0.0
    );
    iteration.common.now_ms = 3_000;
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("expertiseRating")),
        20.0
    );
    assert_eq!(iteration.source_damage_multiplier(Some(weapon.kind)), 1.8);
}

#[test]
fn devouring_flame_is_a_target_scoped_incoming_damage_multiplier() {
    let profile_abilities = vec![
        ability(DpsAbilityKind::InfernalWave, 1.0),
        dot_ability(DpsAbilityKind::EngulfingFlames, 0.1),
        ability(DpsAbilityKind::Detonate, 1.0),
    ];
    let mut profile = profile(profile_abilities);
    profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "wrists".into(),
        source_id: "legendary-wrists-test".into(),
        source_name: "Devouring Flame".into(),
        mechanic_id: "test:devouring-flame".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: BTreeMap::from([("engulfingTargetIncomingMultiplier".into(), 1.07)]),
        reason: None,
    });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 6);
    let engulfing = profile.abilities[1].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::EngulfingFlames,
        &compiled_ability(&engulfing),
        engulfing.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );

    iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        false,
        1,
        DamageContext::NONE,
    );

    assert_eq!(iteration.common.result.targets, vec![107.0, 100.0]);

    let rounded = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.5,
        0.0,
        false,
        0,
        DamageContext::NONE,
    );
    // Native rounding surrounds Devouring Flame: round(round(100.5) * 1.07).
    assert_eq!(rounded.damage, 108.0);

    let detonated = iteration.damage_detonate(
        &compiled_ability(&profile.abilities[2]),
        107.6,
        0.0,
        0,
        DamageContext::NONE,
    );
    // The incoming modifier is removed from the sampled tick before rounding.
    assert_eq!(detonated.damage, 108.0);
}

#[test]
fn deterministic_output_changes_with_the_apl() {
    let profile = profile(vec![
        ability(DpsAbilityKind::InfernalWave, 1.0),
        ability(DpsAbilityKind::FireBall, 2.0),
    ]);
    let wave_apl = apl([("infernal-wave", None)]);
    let fire_ball_apl = apl([("fire-ball", None)]);

    let first = Iteration::new(&profile, &wave_apl, 1, 99)
        .run()
        .expect("first iteration completes");
    let repeated = Iteration::new(&profile, &wave_apl, 1, 99)
        .run()
        .expect("repeated iteration completes");
    let different = Iteration::new(&profile, &fire_ball_apl, 1, 99)
        .run()
        .expect("different iteration completes");

    assert_eq!(first.damage, repeated.damage);
    assert!(different.damage > first.damage);
}

#[test]
fn cinder_tick_rewards_are_seeded_discrete_rolls() {
    let mut source = dot_ability(DpsAbilityKind::SearingBlaze, 0.2);
    let model = source.dot.as_mut().expect("dot model");
    model.cinder_proc_chance = 0.5;
    model.cinders_on_proc = 20.0;
    let profile = profile(vec![source]);
    let apl = apl([("searing-blaze", None)]);

    let tick = |seed| {
        let mut iteration = Iteration::new(&profile, &apl, 1, seed);
        iteration.common.now_ms = 1_000;
        iteration.common.dots.insert(
            (0, DotKind::Ability(DpsAbilityKind::SearingBlaze)),
            DotState {
                model: profile.abilities[0].dot.expect("dot model"),
                source: iteration.profile.abilities[0].damage_source,
                expertise_snapshot: 0.0,
                primary_stat_multiplier_snapshot: 1.0,
                derived_damage_per_tick: None,
                gunde_rend_buckets: None,
                critical_chance_override: None,
                bonus_crit: 0.0,
                generation: 1,
                started_ms: 0,
                expires_ms: 2_000,
                stacks: 1,
                context: DamageContext::for_cast(1),
                last_tick_ms: 1_000,
                next_tick_ms: 2_000,
                scheduled_period_ms: 1_000,
            },
        );
        iteration.dot_tick(DotKind::Ability(DpsAbilityKind::SearingBlaze), 1, 0);
        iteration.hero.ardeos().cinders
    };

    let first = tick(17);
    assert_eq!(first, tick(17));
    assert!(first == 0.0 || first == 20.0);
    assert_ne!(first, 10.0);
}

#[test]
fn firemage_resource_rewards_scale_with_inverse_haste_and_each_target_execution() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball
        .mechanic_parameters
        .insert("resourceProcChance".into(), 1.0);
    fire_ball
        .mechanic_parameters
        .insert("cindersOnResourceProc".into(), 20.0);
    let mut profile = profile(vec![fire_ball]);
    profile.haste = 1.0;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 17);

    for target_index in 0..3 {
        iteration.impact(
            0,
            false,
            1.0,
            target_index,
            DamageContext::for_cast(u64::from(target_index) + 1),
        );
    }

    assert_eq!(iteration.hero.ardeos().cinders, 30.0);
}

#[test]
fn resolved_periodic_and_proc_damage_feed_source_owned_shadow_mark() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.mechanic_parameters
        .insert("accumulationFraction".into(), 0.5);
    mark.mechanic_parameters
        .insert("maximumPowerCoefficient".into(), 10.0);
    let profile = profile(vec![mark]);
    let apl = apl([("weapon-shadow-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    iteration.shared.shadow_marks.insert(
        0,
        ShadowMarkState {
            generation: 1,
            until_ms: 5_000,
            accumulated_damage: 0.0,
            context: DamageContext::for_cast(1),
        },
    );

    iteration.damage_unscaled_key(
        None,
        iteration.test_damage_source(&profile.abilities[0].id),
        100.0,
        0,
        DamageProvenance::Periodic,
        DamageContext::NONE,
    );
    iteration.damage_unscaled_key(
        None,
        iteration.test_damage_source(&profile.abilities[0].id),
        100.0,
        1,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );

    assert_eq!(iteration.shared.shadow_marks[&0].accumulated_damage, 100.0);
    assert_eq!(iteration.common.result.targets, vec![100.0, 100.0]);
}

#[test]
fn automatic_targeting_is_deterministic_and_isolated_per_dummy() {
    let mut wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    wave.max_targets = 3;
    let profile = profile(vec![wave]);
    let apl = apl([("infernal-wave", None)]);

    let single = Iteration::new(&profile, &apl, 1, 42)
        .run()
        .expect("single-target iteration completes");
    let pack = Iteration::new(&profile, &apl, 3, 42)
        .run()
        .expect("multi-target iteration completes");
    let repeated = Iteration::new(&profile, &apl, 3, 42)
        .run()
        .expect("repeated iteration completes");

    assert_eq!(pack.damage, repeated.damage);
    assert_eq!(pack.targets, repeated.targets);
    assert_eq!(pack.targets.len(), 3);
    assert!(pack.targets.iter().all(|damage| *damage > 0.0));
    assert_eq!(pack.targets[0], single.targets[0]);
    assert_eq!(pack.damage, single.damage * 3.0);
}

#[test]
fn pyromania_spreads_to_missing_dots_before_refreshing_active_targets() {
    let mut pyromania = ability(DpsAbilityKind::Pyromania, 0.0);
    pyromania.direct_hits = 0;
    pyromania.max_targets = 3;
    pyromania.cooldown_ms = 90_000;
    pyromania.off_gcd = true;
    pyromania.applies_dot_kind = Some(DpsAbilityKind::EngulfingFlames);
    let engulfing = dot_ability(DpsAbilityKind::EngulfingFlames, 0.2);
    let profile = profile(vec![pyromania, engulfing]);
    let apl = apl([("pyromania", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 5, 42);
    let engulfing = profile.abilities[1].clone();
    for target in [1, 2] {
        iteration.apply_dot(
            target,
            DpsAbilityKind::EngulfingFlames,
            &compiled_ability(&engulfing),
            engulfing.dot.expect("dot model"),
            DamageContext::for_cast(1),
        );
    }
    let existing_generations = [
        iteration.common.dots[&(1, DotKind::Ability(DpsAbilityKind::EngulfingFlames))].generation,
        iteration.common.dots[&(2, DotKind::Ability(DpsAbilityKind::EngulfingFlames))].generation,
    ];

    assert_eq!(iteration.pyromania_targets(0, 3), vec![3, 4, 0]);
    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));

    for target in [0, 1, 2, 3, 4] {
        assert!(
            iteration
                .common
                .dots
                .contains_key(&(target, DotKind::Ability(DpsAbilityKind::EngulfingFlames)))
        );
    }
    assert_eq!(
        existing_generations,
        [
            iteration.common.dots[&(1, DotKind::Ability(DpsAbilityKind::EngulfingFlames))]
                .generation,
            iteration.common.dots[&(2, DotKind::Ability(DpsAbilityKind::EngulfingFlames))]
                .generation,
        ]
    );
}

#[test]
fn pyromania_waits_for_its_cooked_effect_delay_before_applying_dots() {
    let mut pyromania = ability(DpsAbilityKind::Pyromania, 0.0);
    pyromania.direct_hits = 0;
    pyromania.max_targets = 3;
    pyromania.cooldown_ms = 90_000;
    pyromania.first_hit_delay_ms = 200;
    pyromania.off_gcd = true;
    pyromania.applies_dot_kind = Some(DpsAbilityKind::EngulfingFlames);
    let profile = profile(vec![
        pyromania,
        dot_ability(DpsAbilityKind::EngulfingFlames, 0.2),
    ]);
    let apl = apl([("pyromania", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 42);

    iteration.cast(0);
    assert!(iteration.common.dots.is_empty());
    iteration.process_events_through(199);
    assert!(iteration.common.dots.is_empty());
    iteration.process_events_through(200);

    assert_eq!(iteration.common.dots.len(), 3);
    assert!(
        iteration
            .common
            .dots
            .values()
            .all(|dot| dot.started_ms == 200)
    );
}

#[test]
fn apocalypse_applies_searing_after_its_separate_visual_delay() {
    let mut apocalypse = ability(DpsAbilityKind::Apocalypse, 1.0);
    apocalypse.applies_dot_kind = Some(DpsAbilityKind::SearingBlaze);
    apocalypse.dot_application_delay_ms = 100;
    let profile = profile(vec![
        apocalypse,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.2),
    ]);
    let apl = apl([("apocalypse", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 42);

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    assert!(iteration.common.dots.is_empty());
    iteration.process_events_through(99);
    assert!(iteration.common.dots.is_empty());
    iteration.process_events_through(100);

    let dot = &iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))];
    assert_eq!(dot.started_ms, 100);
}

#[test]
fn wildfire_starts_after_its_cooked_hit_delay() {
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.first_hit_delay_ms = 180;
    wildfire.effect_duration_ms = 1_000;
    wildfire.off_gcd = true;
    let profile = profile(vec![wildfire]);
    let apl = apl([("wildfire", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 42);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 0);
    assert_eq!(iteration.hero.ardeos().wildfire_until, 0);
    iteration.process_events_through(179);
    assert_eq!(iteration.hero.ardeos().wildfire_until, 0);
    iteration.process_events_through(180);
    assert_eq!(iteration.hero.ardeos().wildfire_until, 1_180);
}

#[test]
fn multi_target_damage_falloff_matches_the_native_scaler() {
    assert_eq!(multi_target_damage_falloff(0, 1.0), 1.0);
    assert_eq!(multi_target_damage_falloff(3, 3.9), 1.0);
    assert!((multi_target_damage_falloff(4, 1.0) - 0.5).abs() < f64::EPSILON);
    assert!((multi_target_damage_falloff(8, 2.0) - 0.5).abs() < f64::EPSILON);
}

#[test]
fn stationary_targets_register_primary_then_synthetic_indices() {
    assert_eq!(
        ActorRegistrationOrder::primary_then_synthetic(5).enemy_targets(),
        &[0, 1, 2, 3, 4]
    );
}

#[test]
fn native_damage_spread_is_symmetric_and_grievous_overflow_is_linear() {
    let rolls = (0_u64..=0x7fff)
        .map(damage_spread_roll_from_random)
        .collect::<Vec<_>>();
    assert_eq!(rolls.first().copied(), Some(-0.5));
    assert_eq!(rolls.last().copied(), Some(0.5));
    assert!((rolls.iter().sum::<f64>() / rolls.len() as f64).abs() < 1e-12);
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.critical_strike = 1.25;
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    let outcome = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        100.0,
        0.0,
        true,
        0,
        DamageContext::NONE,
    );
    assert!(outcome.critical);
    assert_eq!(outcome.damage, 225.0);
}

#[test]
fn ability_target_count_falloff_scales_each_affected_dummy() {
    let mut apocalypse = ability(DpsAbilityKind::Apocalypse, 1.0);
    apocalypse.max_targets = 4;
    apocalypse
        .mechanic_parameters
        .insert("targetCountDamageScalingThreshold".into(), 1.0);
    let profile = profile(vec![apocalypse]);
    let apl = apl([("apocalypse", None)]);
    let mut single = Iteration::new(&profile, &apl, 1, 1);
    single.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    let mut pack = Iteration::new(&profile, &apl, 4, 1);
    for target in 0..4 {
        pack.impact(0, false, 1.0, target, DamageContext::for_cast(1));
    }

    assert_eq!(single.common.result.targets, vec![100.0]);
    assert_eq!(pack.common.result.targets, vec![50.0; 4]);
    assert_eq!(pack.common.result.damage, 200.0);
}

#[test]
fn detonate_target_count_falloff_scales_sampled_dots() {
    let mut detonate = ability(DpsAbilityKind::Detonate, 1.0);
    detonate.max_targets = 4;
    detonate.mechanic_parameters = BTreeMap::from([
        ("initialDelaySeconds".into(), 0.5),
        ("perTargetHitDelaySeconds".into(), 0.4),
        ("hitsPerTarget".into(), 3.0),
        ("betweenHitDelaySeconds".into(), 0.3),
        ("targetCountDamageScalingThreshold".into(), 1.0),
    ]);
    let profile = profile(vec![
        detonate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.2),
    ]);
    let apl = apl([("detonate", None)]);
    let searing_blaze = profile.abilities[1].clone();
    let dot = searing_blaze.dot.expect("dot model");

    let mut single = Iteration::new(&profile, &apl, 1, 1);
    single.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&searing_blaze),
        dot,
        DamageContext::for_cast(1),
    );
    single.common.queue.clear();
    single.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    single.process_events_through(3_000);

    let mut pack = Iteration::new(&profile, &apl, 4, 1);
    for target in 0..4 {
        pack.apply_dot(
            target,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(&searing_blaze),
            dot,
            DamageContext::for_cast(1),
        );
    }
    pack.common.queue.clear();
    pack.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    pack.process_events_through(3_000);

    // Each of three hits rounds separately: 20/3 -> 7, (20 * 0.5)/3 -> 3.
    assert!((single.common.result.targets[0] - 21.0).abs() < 1e-10);
    assert!(
        pack.common
            .result
            .targets
            .iter()
            .all(|damage| (*damage - 9.0).abs() < 1e-10)
    );
    assert!((pack.common.result.damage - 36.0).abs() < 1e-10);
}

#[test]
fn detonate_uses_the_granted_controller_hit_schedule() {
    let mut detonate = ability(DpsAbilityKind::Detonate, 1.0);
    detonate.max_targets = 2;
    detonate.mechanic_parameters = BTreeMap::from([
        ("initialDelaySeconds".into(), 0.5),
        ("perTargetHitDelaySeconds".into(), 0.4),
        ("hitsPerTarget".into(), 3.0),
        ("betweenHitDelaySeconds".into(), 0.3),
        ("targetCountDamageScalingThreshold".into(), 2.0),
    ]);
    let profile = profile(vec![
        detonate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.3),
    ]);
    let apl = apl([("detonate", None)]);
    let searing_blaze = profile.abilities[1].clone();
    let dot = searing_blaze.dot.expect("dot model");
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    for target in 0..2 {
        iteration.apply_dot(
            target,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(&searing_blaze),
            dot,
            DamageContext::for_cast(1),
        );
    }
    iteration.common.queue.clear();

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    iteration.process_events_through(499);
    assert_eq!(iteration.common.result.targets, vec![0.0, 0.0]);

    iteration.process_events_through(500);
    assert_eq!(iteration.common.result.targets, vec![10.0, 0.0]);

    iteration.process_events_through(899);
    assert_eq!(iteration.common.result.targets, vec![20.0, 0.0]);

    iteration.process_events_through(1_100);
    assert_eq!(iteration.common.result.targets, vec![30.0, 10.0]);

    iteration.process_events_through(1_500);
    assert_eq!(iteration.common.result.targets, vec![30.0, 30.0]);
}

#[test]
fn detonate_assigns_unequal_target_damage_in_actor_registration_order() {
    let mut detonate = ability(DpsAbilityKind::Detonate, 1.0);
    detonate.max_targets = 2;
    detonate.mechanic_parameters.extend([
        ("hitsPerTarget".into(), 1.0),
        ("betweenHitDelaySeconds".into(), 0.0),
        ("targetCountDamageScalingThreshold".into(), 2.0),
    ]);
    let profile = profile(vec![
        detonate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.1),
    ]);
    let apl = apl([("detonate", None)]);
    let searing_blaze = profile.abilities[1].clone();
    let dot = searing_blaze.dot.expect("dot model");
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    for target in 0..2 {
        iteration.apply_dot(
            target,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(&searing_blaze),
            dot,
            DamageContext::for_cast(1),
        );
    }
    iteration
        .common
        .dots
        .get_mut(&(1, DotKind::Ability(DpsAbilityKind::SearingBlaze)))
        .expect("second target dot")
        .model
        .power_coefficient = 0.3;
    iteration.common.queue.clear();

    // Reverse the test registry to prove Detonate consumes the explicit
    // actor view rather than falling back to target-index iteration.
    iteration.common.actor_registration_order.enemy_targets = vec![1, 0];
    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    iteration.process_events_through(500);
    assert_eq!(iteration.common.result.targets, vec![0.0, 30.0]);

    iteration.process_events_through(900);
    assert_eq!(iteration.common.result.targets, vec![10.0, 30.0]);
}

#[test]
fn flare_up_hits_only_nearby_targets_with_searing_blaze() {
    let mut profile = profile(vec![
        ability(DpsAbilityKind::InfernalWave, 1.0),
        dot_ability(DpsAbilityKind::SearingBlaze, 0.2),
    ]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent17".into(),
            name: "Bursting Flames".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-burstingflames".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("damageFraction".into(), 0.5),
                ("maximumRadius".into(), 1_500.0),
                ("targetCountDamageScalingThreshold".into(), 3.0),
                ("visualDelaySeconds".into(), 0.3),
            ]),
            reason: None,
        });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 4, 1);
    let searing_blaze = profile.abilities[1].clone();
    let dot = searing_blaze.dot.expect("dot model");
    for target in 0..3 {
        iteration.apply_dot(
            target,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(&searing_blaze),
            dot,
            DamageContext::for_cast(1),
        );
    }

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));

    assert_eq!(iteration.common.result.targets, vec![100.0, 0.0, 0.0, 0.0]);
    iteration.process_events_through(299);
    assert_eq!(iteration.common.result.targets, vec![100.0, 0.0, 0.0, 0.0]);
    iteration.process_events_through(300);
    assert_eq!(
        iteration.common.result.targets,
        vec![150.0, 50.0, 50.0, 0.0]
    );
    let flare = &iteration.ability_totals("firemage-talent-id-talent17");
    assert_eq!(flare.damage, 150.0);
    assert_eq!(flare.hits, 3);
}

#[test]
fn spontaneous_combustion_rolls_when_a_relevant_dot_is_applied() {
    let mut profile = profile(vec![
        dot_ability(DpsAbilityKind::SearingBlaze, 0.2),
        dot_ability(DpsAbilityKind::FireBall, 0.2),
    ]);
    profile.talents.push(DpsTalentModel {
        id: "firemage-talent-id-talent9".into(),
        name: "Spontaneous Combustion".into(),
        mechanic_id:
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-warmth"
                .into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("baseProcChance".into(), 1.0),
            ("criticalStrikeBonus".into(), 1.0),
            ("criticalChanceStep".into(), 0.05),
            ("procChancePerStep".into(), 0.01),
        ]),
        reason: None,
    });
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    let searing_blaze = profile.abilities[0].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::SearingBlaze,
        &compiled_ability(&searing_blaze),
        searing_blaze.dot.expect("dot model"),
        DamageContext::for_cast(1),
    );
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::SearingBlaze))].bonus_crit,
        1.0
    );

    let fire_ball = profile.abilities[1].clone();
    iteration.apply_dot(
        0,
        DpsAbilityKind::FireBall,
        &compiled_ability(&fire_ball),
        fire_ball.dot.expect("dot model"),
        DamageContext::for_cast(2),
    );
    assert_eq!(
        iteration.common.dots[&(0, DotKind::Ability(DpsAbilityKind::FireBall))].bonus_crit,
        0.0
    );
}

#[test]
fn spontaneous_combustion_critical_steps_have_no_minimum_bonus() {
    for (critical_strike, expected_chance) in
        [(0.0, 0.10), (0.049, 0.10), (0.05, 0.11), (0.11, 0.12)]
    {
        let mut profile = profile(vec![dot_ability(DpsAbilityKind::SearingBlaze, 0.2)]);
        profile.critical_strike = critical_strike;
        profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent9".into(),
            name: "Spontaneous Combustion".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-warmth".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("baseProcChance".into(), 0.10), ("criticalStrikeBonus".into(), 1.0),
                ("criticalChanceStep".into(), 0.05), ("procChancePerStep".into(), 0.01),
            ]), reason: None,
        });
        let apl = apl([("searing-blaze", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 1);
        let ability = &profile.abilities[0];
        iteration.apply_dot(
            0,
            DpsAbilityKind::SearingBlaze,
            &compiled_ability(ability),
            ability.dot.unwrap(),
            DamageContext::for_cast(1),
        );
        let state =
            iteration.shared.controlled_random_states[SPONTANEOUS_COMBUSTION_RANDOM_STREAM_TAG];
        assert!(
            (state.chance_factor - expected_chance).abs() < f32::EPSILON,
            "critical chance {critical_strike}: {}",
            state.chance_factor
        );
    }
}

#[test]
fn pyrophibian_spawns_the_extracted_number_of_full_attack_frogs() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireFrogs, 1.0)]);
    profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent1".into(),
            name: "Pyrophibian Frenzy".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-pyrophibianfrenzy".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("numberOfFrogs".into(), 3.0),
                ("procChance".into(), 1.0),
                ("criticalProcChance".into(), 1.0),
            ]),
            reason: None,
        });
    let apl = apl([("fire-frogs", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);

    iteration.try_pyrophibian(false, 0, DamageContext::for_cast(1));
    iteration.process_events_through(20_000);

    let frogs = &iteration.ability_totals("test:fire-frogs");
    assert_eq!(frogs.hits, 9);
    assert_eq!(frogs.damage, 900.0);
}

#[test]
fn fire_toad_replaces_frogs_and_adds_the_ability_only_bonus_toad() {
    let frogs = ability(DpsAbilityKind::FireFrogs, 1.0);
    let mut profile = profile(vec![frogs.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "legendary-back-fire-toad",
        [
            ("fireToadSpawnChance", 1.0),
            ("fireToadDamageMultiplier", 9.0),
            ("fireToadAoeDamageMultiplier", 5.0),
            ("fireToadTargetCountThreshold", 1.0),
            ("fireToadBonusCount", 1.0),
        ],
    ));
    let apl = apl([("fire-frogs", None)]);

    let mut replacement = Iteration::new(&profile, &apl, 5, 1);
    replacement.execute_fire_frog_batch(
        &compiled_ability(&frogs),
        FireFrogBatch {
            main_target: 0,
            frog_count: 1,
            attacks_per_frog: 3,
            coefficient: 1.0,
            context: DamageContext::for_cast(1),
            allow_bonus_toad: false,
        },
    );
    replacement.process_events_through(20_000);
    let replacement_damage = &replacement.ability_totals("test:fire-frogs");
    assert_eq!(replacement_damage.hits, 5);
    assert_eq!(replacement_damage.damage, 1_900.0);
    assert_eq!(
        replacement.common.result.targets,
        vec![900.0, 250.0, 250.0, 250.0, 250.0]
    );

    let mut ability_cast = Iteration::new(&profile, &apl, 5, 1);
    ability_cast.execute_fire_frog_batch(
        &compiled_ability(&frogs),
        FireFrogBatch {
            main_target: 0,
            frog_count: 1,
            attacks_per_frog: 3,
            coefficient: 1.0,
            context: DamageContext::for_cast(1),
            allow_bonus_toad: true,
        },
    );
    ability_cast.process_events_through(20_000);
    let ability_damage = &ability_cast.ability_totals("test:fire-frogs");
    assert_eq!(ability_damage.hits, 10);
    assert_eq!(ability_damage.damage, 3_800.0);
}

#[test]
fn apocalyptic_surge_is_granted_on_commit_and_consumed_by_free_detonates() {
    let mut apocalypse = ability(DpsAbilityKind::Apocalypse, 1.0);
    apocalypse.cast_time_ms = 2_000;
    let detonate = ability(DpsAbilityKind::Detonate, 1.0);
    let mut profile = profile(vec![apocalypse, detonate]);
    profile.talents.push(DpsTalentModel {
        id: "firemage-talent-id-talent14".into(),
        name: "Ardeos talent 14".into(),
        mechanic_id:
            "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait1".into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("castTimeReductionSeconds".into(), 0.5),
            ("surgeStacks".into(), 2.0),
            ("maximumSurgeStacks".into(), 4.0),
            ("surgeDurationSeconds".into(), 18.0),
        ]),
        reason: None,
    });
    let apl = apl([("apocalypse", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);

    iteration.cast(0);

    assert_eq!(iteration.common.now_ms, 1_500);
    assert_eq!(iteration.hero.ardeos().apocalyptic_surge, 2);
    assert_eq!(iteration.hero.ardeos().apocalyptic_surge_until, 19_500);
    assert_eq!(iteration.hero.ardeos().embers, 0);
    assert_eq!(iteration.can_cast(DpsAbilityKind::Detonate), Some(1));

    iteration.cast(1);
    assert_eq!(iteration.hero.ardeos().apocalyptic_surge, 1);
    assert_eq!(iteration.hero.ardeos().embers, 0);

    iteration.common.now_ms = iteration.hero.ardeos().apocalyptic_surge_until;
    assert_eq!(iteration.buff_stacks(AplBuff::ApocalypticSurge), 0);
    assert_eq!(iteration.can_cast(DpsAbilityKind::Detonate), None);
}

#[test]
fn apocalyptic_surge_reduces_base_cast_duration_before_haste() {
    let mut apocalypse = ability(DpsAbilityKind::Apocalypse, 1.0);
    apocalypse.cast_time_ms = 3_000;
    let mut profile = profile(vec![apocalypse]);
    profile.haste = 0.5;
    profile.talents.push(DpsTalentModel {
        id: "firemage-talent-id-talent14".into(),
        name: "Ardeos talent 14".into(),
        mechanic_id: "test:apocalyptic-surge".into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("castTimeReductionSeconds".into(), 1.5),
            ("surgeStacks".into(), 2.0),
            ("maximumSurgeStacks".into(), 4.0),
            ("surgeDurationSeconds".into(), 18.0),
        ]),
        reason: None,
    });
    let apl = apl([("apocalypse", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);

    iteration.cast(0);

    // (3 - 1.5) / 1.5 seconds, rather than 3 / 1.5 - 1.5.
    assert_eq!(iteration.common.now_ms, 1_000);
    assert_eq!(iteration.hero.ardeos().apocalyptic_surge_until, 19_000);
}

#[test]
fn fire_frogs_spawn_one_batch_and_reserve_targets_across_the_pack() {
    let mut frogs = ability(DpsAbilityKind::FireFrogs, 1.0);
    frogs.direct_hits = 15;
    frogs.max_targets = 1;
    let profile = profile(vec![frogs]);
    let apl = apl([("fire-frogs", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    iteration.process_events_through(20_000);

    let frogs = &iteration.ability_totals("test:fire-frogs");
    assert_eq!(frogs.hits, 15);
    assert_eq!(frogs.damage, 1_500.0);
    assert_eq!(iteration.common.result.targets, vec![800.0, 700.0]);
}

#[test]
fn fire_frog_first_hit_uses_the_accepted_thirty_hertz_travel_model() {
    let mut frogs = ability(DpsAbilityKind::FireFrogs, 1.0);
    frogs.mechanic_parameters.extend([
        ("frogCount".into(), 1.0),
        ("attacksPerFrog".into(), 1.0),
        ("spawnRadiusUnits".into(), 0.0),
        ("minimumJumpLengthUnits".into(), 300.0),
        ("maximumJumpLengthUnits".into(), 300.0),
        ("minimumJumpPeriodSeconds".into(), 0.5),
        ("maximumJumpPeriodSeconds".into(), 0.5),
    ]);
    let profile = profile(vec![frogs]);
    let apl = apl([("fire-frogs", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);

    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    iteration.process_events_through(5_599);
    assert_eq!(iteration.ability_totals("test:fire-frogs").hits, 0);

    iteration.process_events_through(5_600);
    assert_eq!(iteration.ability_totals("test:fire-frogs").hits, 1);
}

#[test]
fn flare_up_is_modeled_for_single_and_stacked_target_scenarios() {
    let talent = DpsTalentModel {
            id: "firemage-talent-id-talent17".into(),
            name: "Bursting Flames".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-burstingflames".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([
                ("damageFraction".into(), 0.5),
                ("maximumRadius".into(), 1_500.0),
                ("targetCountDamageScalingThreshold".into(), 3.0),
                ("visualDelaySeconds".into(), 0.3),
            ]),
            reason: None,
        };

    assert!(validate_talent(&talent, ARDEOS_HERO_ID, 1).is_ok());
    assert!(validate_talent(&talent, ARDEOS_HERO_ID, 5).is_ok());
}

#[test]
fn stationary_dummy_scenario_requires_ardeos_maximum_combat_range() {
    let mut request = request(
        profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
        1,
        DpsEvidenceClaimStatus::Partial,
    );
    request.scenario.target_distance_units = ARDEOS_MAX_COMBAT_RANGE_UNITS - 1.0;

    let error = validate(&request).expect_err("off-contract range must be rejected");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
    assert!(error.message.contains("3000"));
}

#[test]
fn reign_of_fire_caps_and_refreshes_stacks_and_consumes_at_projectile_launch() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.max_targets = 2;
    let mut profile = profile(vec![fire_ball]);
    profile.talents.push(reign_of_fire_talent());
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 6);
    for time in [0, 1_000, 2_000] {
        iteration.advance_to(time);
        iteration.try_reign_of_fire();
    }
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 2);
    assert_eq!(iteration.hero.ardeos().reign_fireball_until, 14_000);

    iteration.advance_to(13_999);
    let mut boosted = DamageContext::for_cast(1);
    iteration.commit_ability(0, &mut boosted, false, 0.0);
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 1);
    assert_eq!(boosted.source_snapshot.unwrap().critical_chance, 1.0);

    // A launch at expiry cannot use the remaining stack. The projectile
    // already in flight keeps its captured bonus for every target.
    iteration.advance_to(14_000);
    let mut unboosted = DamageContext::for_cast(2);
    iteration.commit_ability(0, &mut unboosted, false, 0.0);
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 0);
    assert_eq!(unboosted.source_snapshot.unwrap().critical_chance, 0.0);
    iteration.impact(0, false, 1.0, 0, boosted);
    iteration.impact(0, false, 1.0, 1, boosted);
    assert_eq!(iteration.common.result.targets, vec![200.0, 200.0]);
    iteration.common.queue.clear();

    // A later proc cannot empower the unboosted projectile retroactively,
    // and its impact must not consume the new proc's stack.
    iteration.try_reign_of_fire();
    iteration.impact(0, false, 1.0, 0, unboosted);
    iteration.impact(0, false, 1.0, 1, unboosted);
    assert_eq!(iteration.common.result.targets, vec![300.0, 300.0]);
    assert_eq!(iteration.hero.ardeos().reign_fireball_stacks, 1);
}

#[test]
fn detonate_captures_critical_chance_per_delayed_hit_without_resampling_damage() {
    let mut detonate = ability(DpsAbilityKind::Detonate, 1.0);
    detonate.mechanic_parameters.extend([
        ("initialDelaySeconds".into(), 0.5),
        ("hitsPerTarget".into(), 3.0),
        ("betweenHitDelaySeconds".into(), 0.3),
    ]);
    let mut profile = profile(vec![
        detonate,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.3),
    ]);
    profile.mechanics.push(ardeos_mechanic(
        "test:temporary-stats",
        [
            ("criticalStrikeBonus", 1.0),
            ("powerMultiplier", 2.0),
            ("durationSeconds", 0.3),
        ],
    ));
    let apl = apl([("detonate", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    let dot = &profile.abilities[1];
    iteration.apply_dot(
        0,
        dot.kind,
        &compiled_ability(dot),
        dot.dot.unwrap(),
        DamageContext::for_cast(1),
    );
    iteration.common.queue.clear();
    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    iteration.process_events_through(500);
    assert_eq!(iteration.common.result.damage, 10.0);
    let buff = Arc::clone(&iteration.profile.mechanics[0]);
    iteration.advance_to(600);
    iteration.activate_dynamic_buff(&buff, 300, 1, 0.0);
    // Remove the sampled DoT: delayed hits retain their original magnitude.
    iteration.common.dots.clear();
    iteration.process_events_through(800);
    assert_eq!(iteration.common.result.damage, 30.0);
    iteration.process_events_through(1_100);
    assert_eq!(iteration.common.result.damage, 40.0);
}

fn spontaneous_combustion_talent(chance: f64) -> DpsTalentModel {
    DpsTalentModel {
        id: "firemage-talent-id-talent9".into(),
        name: "Spontaneous Combustion".into(),
        mechanic_id:
            "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-warmth"
                .into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("baseProcChance".into(), chance),
            ("criticalStrikeBonus".into(), 1.0),
            ("criticalChanceStep".into(), 0.05),
            ("procChancePerStep".into(), 0.01),
        ]),
        reason: None,
    }
}

#[test]
fn spontaneous_combustion_rerolls_full_and_terminal_fractional_ticks() {
    let mut dot = dot_ability(DpsAbilityKind::SearingBlaze, 0.2);
    dot.dot.as_mut().unwrap().can_crit = true;
    dot.dot.as_mut().unwrap().duration_ms = 2_500;
    let mut profile = profile(vec![dot.clone()]);
    profile.talents.push(spontaneous_combustion_talent(1.0));
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.apply_dot(
        0,
        dot.kind,
        &compiled_ability(&dot),
        dot.dot.unwrap(),
        DamageContext::for_cast(1),
    );
    assert_eq!(iteration.test_proc_count("firemage-talent-id-talent9"), 1);
    iteration.process_events_through(2_500);
    assert_eq!(iteration.test_proc_count("firemage-talent-id-talent9"), 4);
    assert_eq!(iteration.common.result.damage, 100.0);
    assert!(iteration.common.dots.is_empty());
}

#[test]
fn spontaneous_combustion_failed_periodic_roll_clears_only_the_next_ticks_bonus() {
    let mut dot = dot_ability(DpsAbilityKind::SearingBlaze, 0.2);
    dot.dot.as_mut().unwrap().can_crit = true;
    let mut profile = profile(vec![dot.clone()]);
    profile.talents.push(spontaneous_combustion_talent(0.0));
    let apl = apl([("searing-blaze", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.apply_dot(
        0,
        dot.kind,
        &compiled_ability(&dot),
        dot.dot.unwrap(),
        DamageContext::for_cast(1),
    );
    let key = (0, DotKind::Ability(dot.kind));
    // Start with a successful previous roll. The next callback is guaranteed
    // to fail, making its position relative to damage observable.
    let active = iteration.common.dots.get_mut(&key).unwrap();
    active.bonus_crit = 1.0;
    active.critical_chance_override = Some(1.0);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.common.result.damage, 40.0);
    assert_eq!(iteration.common.dots[&key].bonus_crit, 0.0);
    assert_eq!(
        iteration.common.dots[&key].critical_chance_override,
        Some(0.0)
    );
    iteration.process_events_through(2_000);
    assert_eq!(iteration.common.result.damage, 60.0);
}

fn infernal_wave_talent(id: &str, parameters: &[(&str, f64)]) -> DpsTalentModel {
    DpsTalentModel {
        id: id.into(),
        name: id.into(),
        mechanic_id: format!("test:{id}"),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .iter()
            .map(|(key, value)| ((*key).into(), *value))
            .collect(),
        reason: None,
    }
}

#[test]
fn infernal_wave_captures_active_dot_instances_at_commit_before_flight() {
    let mut wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    wave.off_gcd = true;
    wave.cast_time_ms = 500;
    wave.first_hit_delay_ms = 1_000;
    let engulfing = dot_ability(DpsAbilityKind::EngulfingFlames, 0.0);
    let mut profile = profile(vec![wave, engulfing.clone()]);
    profile.talents.push(infernal_wave_talent(
        "firemage-talent-id-talent4",
        &[("damagePerUniqueDot", 0.15), ("maximumDamageIncrease", 2.2)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    // Independent Engulfing instances both count even though their kind matches.
    for _ in 0..2 {
        iteration.apply_dot(
            0,
            engulfing.kind,
            &compiled_ability(&engulfing),
            engulfing.dot.unwrap(),
            DamageContext::NONE,
        );
    }
    iteration.cast(0);
    iteration.common.dots.clear();
    iteration.process_events_through(1_500);
    assert_eq!(iteration.ability_totals("test:infernal-wave").damage, 130.0);

    // A DoT added during the next flight cannot strengthen that projectile.
    iteration.cast(0);
    iteration.apply_dot(
        0,
        engulfing.kind,
        &compiled_ability(&engulfing),
        engulfing.dot.unwrap(),
        DamageContext::NONE,
    );
    iteration.process_events_through(3_000);
    assert_eq!(iteration.ability_totals("test:infernal-wave").damage, 230.0);
}

#[test]
fn cascading_inferno_proc_belongs_to_its_launch_with_overlapping_projectiles() {
    let mut wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    wave.off_gcd = true;
    wave.first_hit_delay_ms = 2_000;
    wave.primary_resource_generated = 40.0;
    let mut profile = profile(vec![wave]);
    profile.talents.push(infernal_wave_talent(
        "firemage-talent-id-talent5",
        &[
            ("stacksThreshold", 3.0),
            ("maximumStacks", 6.0),
            ("criticalStrikeBonus", 1.0),
            ("cindersMultiplier", 2.0),
        ],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    iteration.cast(0);
    iteration.process_events_through(500);
    iteration.hero.ardeos_mut().cascading_stacks = 3;
    iteration.cast(0);
    iteration.process_events_through(500);
    assert_eq!(iteration.hero.ardeos().cascading_stacks, 0);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:infernal-wave").damage, 100.0);
    assert_eq!(iteration.hero.ardeos().cinders, 40.0);
    iteration.process_events_through(2_500);
    assert_eq!(iteration.ability_totals("test:infernal-wave").damage, 300.0);
    assert_eq!(iteration.hero.ardeos().embers, 1);
    assert_eq!(iteration.hero.ardeos().cinders, 20.0);
}

#[test]
fn slow_burn_adds_one_resource_roll_per_fire_ball_tick_without_core_dots() {
    for (slow_burn, expected) in [(false, 10.0), (true, 20.0)] {
        let mut fire_ball = dot_ability(DpsAbilityKind::FireBall, 0.0);
        let model = fire_ball.dot.as_mut().unwrap();
        model.duration_ms = 1_000;
        model.cinder_proc_chance = 1.0;
        model.cinders_on_proc = 10.0;
        let mut profile = profile(vec![fire_ball.clone()]);
        if slow_burn {
            profile.talents.push(infernal_wave_talent(
                "firemage-talent-id-talent18",
                &[("extensionPerTickSeconds", 1.0)],
            ));
        }
        let apl = apl([("fire-ball", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 6);
        iteration.apply_damage_derived_dot(
            0,
            DotKind::Ability(DpsAbilityKind::FireBall),
            iteration.test_damage_source("test:fire-ball"),
            fire_ball.dot.unwrap(),
            100.0,
            DamageContext::NONE,
        );
        iteration.process_events_through(1_000);
        assert_eq!(
            iteration.hero.ardeos().cinders,
            expected,
            "slow_burn={slow_burn}"
        );
    }
}

#[test]
fn transferred_burn_does_not_reapply_original_cast_damage_multiplier() {
    let mut burn = dot_ability(DpsAbilityKind::FireBall, 0.0).dot.unwrap();
    burn.duration_ms = 1_000;
    let profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    let mut context = DamageContext::for_cast(1);
    context.multiply_damage(1.5);
    iteration.apply_damage_derived_dot(
        0,
        DotKind::Ability(DpsAbilityKind::FireBall),
        iteration.test_damage_source("test:fire-ball"),
        burn,
        150.0,
        context,
    );
    iteration.process_events_through(1_000);
    assert_eq!(iteration.ability_totals("test:fire-ball").damage, 150.0);
}

#[test]
fn firemage_periodic_recapture_retains_proc_bonus_and_updates_detonate_sample() {
    for kind in [
        DpsAbilityKind::SearingBlaze,
        DpsAbilityKind::EngulfingFlames,
        DpsAbilityKind::Incinerate,
    ] {
        let mut source = dot_ability(kind, 1.0);
        source.dot.as_mut().unwrap().can_crit = true;
        let mut profile = profile(vec![source.clone()]);
        profile.talents.push(infernal_wave_talent(
            "firemage-talent-id-talent11",
            &[("dotCriticalStrikeBonus", 1.0)],
        ));
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.CooldownRecoveryOnWeaponAbility",
            [("powerMultiplier", 1.5)],
        ));
        let apl = apl([(ability_kind_id(kind), None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 6);
        iteration.apply_dot(
            0,
            kind,
            &compiled_ability(&source),
            source.dot.unwrap(),
            DamageContext::NONE,
        );
        let buff = Arc::clone(&iteration.profile.mechanics[0]);
        iteration.activate_dynamic_buff(&buff, 1_500, 1, 0.0);
        iteration.process_events_through(1_000);
        assert_eq!(
            iteration.ability_totals(&source.id).damage,
            300.0,
            "{kind:?}"
        );
        let dot = &iteration.common.dots[&(0, DotKind::Ability(kind))];
        assert_eq!(
            iteration.approximate_dot_average_damage(DotKind::Ability(kind), dot, 0),
            150.0
        );
        iteration.process_events_through(2_000);
        assert_eq!(
            iteration.ability_totals(&source.id).damage,
            500.0,
            "{kind:?}"
        );
    }
}

fn ranked_default_profile(rolling_flames: bool) -> NormalizedDpsProfile {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/apl-builds/ardeos/character.json")).unwrap();
    let mut build: crate::preparation::CharacterBuild =
        serde_json::from_value(document["build"].clone()).unwrap();
    if !rolling_flames {
        build
            .selected_talent_ids
            .retain(|id| id != "firemage-talent-id-talent8");
    }
    let (mut profile, _) = crate::preparation::prepare_character(&build, 1).unwrap();
    // The default must skip all unequipped weapon rules, including cooldown references.
    profile.abilities.retain(|ability| {
        !matches!(
            ability.kind,
            DpsAbilityKind::WeaponArcaneChannel
                | DpsAbilityKind::WeaponFrostVolley
                | DpsAbilityKind::WeaponShadowMark
                | DpsAbilityKind::WeaponChainLightning
        )
    });
    profile
}

fn shipped_ardeos_apl() -> ActionPriorityListV2 {
    crate::parse_apl(&shipped_apl_source("ardeos")).unwrap()
}

#[test]
fn default_ardeos_spends_actual_spirit_cost_without_waiting_for_cap_or_dot_expiry() {
    let apl = shipped_ardeos_apl();
    for cost in [85.0, 100.0] {
        let mut profile = ranked_default_profile(true);
        let spirit = profile
            .abilities
            .iter_mut()
            .find(|ability| ability.kind == DpsAbilityKind::Incinerate)
            .unwrap();
        spirit.spirit_cost = cost;
        let spirit = spirit.clone();
        let mut iteration = Iteration::new(&profile, &apl, 1, 31);
        iteration.apply_dot(
            0,
            spirit.kind,
            &compiled_ability(&spirit),
            spirit.dot.unwrap(),
            DamageContext::for_cast(1),
        );
        iteration.hero.ardeos_mut().wildfire_until = 10_000;
        let index = iteration.common.abilities_by_kind[&DpsAbilityKind::Incinerate];
        iteration.shared.spirit = cost - 0.01;
        assert_ne!(iteration.choose_action(), Some(index));
        iteration.shared.spirit = cost;
        assert!(cost < profile.max_spirit);
        assert_eq!(iteration.choose_action(), Some(index));
    }
}

#[test]
fn default_ardeos_pools_the_frog_opener_only_with_rolling_flames() {
    let apl = shipped_ardeos_apl();
    for rolling_flames in [false, true] {
        let profile = ranked_default_profile(rolling_flames);
        let iteration = Iteration::new(&profile, &apl, 1, 32);
        let expected = if rolling_flames {
            DpsAbilityKind::SearingBlaze
        } else {
            DpsAbilityKind::FireFrogs
        };
        assert_eq!(
            iteration.choose_action(),
            Some(iteration.common.abilities_by_kind[&expected]),
            "Rolling Flames: {rolling_flames}"
        );
    }
}

#[test]
fn default_ardeos_uses_spare_engulfing_and_single_target_fire_ball_charges() {
    let apl = shipped_ardeos_apl();
    for rolling_flames in [false, true] {
        let profile = ranked_default_profile(rolling_flames);
        let mut iteration = Iteration::new(&profile, &apl, 1, 33);
        iteration.shared.spirit = 0.0;
        for kind in [DpsAbilityKind::Wildfire, DpsAbilityKind::FireFrogs] {
            iteration.common.cooldowns.insert(
                kind,
                CooldownState {
                    remaining_ms: 20_000.0,
                    used_charges: 1,
                },
            );
        }
        let searing = iteration
            .ability(DpsAbilityKind::SearingBlaze)
            .unwrap()
            .clone();
        iteration.apply_dot(
            0,
            searing.kind,
            &searing,
            searing.dot.unwrap(),
            DamageContext::for_cast(1),
        );
        let engulfing = iteration.common.abilities_by_kind[&DpsAbilityKind::EngulfingFlames];
        let fire_ball = iteration.common.abilities_by_kind[&DpsAbilityKind::FireBall];
        assert_eq!(
            iteration.choose_action(),
            Some(if rolling_flames { engulfing } else { fire_ball })
        );
        iteration.common.cooldowns.insert(
            DpsAbilityKind::EngulfingFlames,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );
        assert_eq!(iteration.choose_action(), Some(fire_ball));
        iteration.common.cooldowns.insert(
            DpsAbilityKind::FireBall,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );
        assert_eq!(
            iteration.choose_action(),
            Some(iteration.common.abilities_by_kind[&DpsAbilityKind::InfernalWave])
        );
    }
}
