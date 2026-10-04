use super::*;

#[test]
fn amethyst_stores_float_magnitudes_before_rounding_tick_damage() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemDotHotOnCrit",
        [
            ("triggerDamageFraction", 0.11),
            ("durationSeconds", 8.0),
            ("periodSeconds", 2.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    let amethyst = Arc::clone(&iteration.profile.mechanics[0]);
    iteration.apply_amethyst_splinters(&amethyst, 1343.0, 0);
    assert_eq!(
        iteration
            .test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0)
            .damage_per_tick,
        36.932498931884766
    );

    iteration.common.now_ms = 1_154;
    iteration.apply_amethyst_splinters(&amethyst, 9778.0, 0);
    assert_eq!(
        iteration
            .test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0)
            .damage_per_tick,
        300.5
    );
    iteration.process_events_through(2_000);
    // Keeping the magnitude as a double gives 300.499986875 and a 300 hit.
    assert_eq!(iteration.common.result.damage, 301.0);
}

#[test]
fn amethyst_expiry_refresh_ties_follow_existing_event_order() {
    for refresh_before_tick in [false, true] {
        let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.GemDotHotOnCrit",
            [
                ("triggerDamageFraction", 0.11),
                ("durationSeconds", 8.0),
                ("periodSeconds", 2.0),
            ],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 1);
        let amethyst = Arc::clone(&iteration.profile.mechanics[0]);
        iteration.apply_amethyst_splinters(&amethyst, 100.0, 0);
        iteration.process_events_through(if refresh_before_tick { 7_999 } else { 8_000 });
        iteration.common.now_ms = 8_000;
        iteration.apply_amethyst_splinters(&amethyst, 100.0, 0);
        let refreshed = iteration.test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0);
        assert_eq!(refreshed.next_tick_ms, 10_000);
        assert_eq!(refreshed.until_ms, 16_000);
        iteration.process_events_through(16_000);
        // Accepted timing approximation: a due final tick executes unless an
        // earlier event at the same timestamp replaces its expired generation.
        assert_eq!(
            iteration.common.result.damage,
            if refresh_before_tick { 21.0 } else { 24.0 }
        );
        assert_eq!(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemDotHotOnCrit")
                .hits,
            if refresh_before_tick { 7 } else { 8 }
        );
    }
}

#[test]
fn amethyst_tick_preserves_a_nested_emerald_critical_refresh() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.critical_strike = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemDotHotOnCrit",
        [
            ("triggerDamageFraction", 0.1),
            ("durationSeconds", 8.0),
            ("periodSeconds", 2.0),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemTargetedSpikeProc",
        [("procChance", 1.0), ("powerCoefficient", 1.0)],
    ));
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 29);
    let amethyst = Arc::clone(&iteration.profile.mechanics[0]);
    iteration.apply_amethyst_splinters(&amethyst, 100.0, 0);
    let original = iteration.test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0);

    iteration.process_events_through(2_000);

    let refreshed = iteration.test_amethyst_splinters("ItemTrait.ID.GemDotHotOnCrit", 0);
    let emerald = iteration.ability_totals("gear:ItemTrait.ID.GemTargetedSpikeProc");
    assert_eq!(emerald.crits, 1);
    assert_eq!(refreshed.generation, original.generation + 1);
    assert_eq!(refreshed.until_ms, 10_000);
    assert_eq!(refreshed.next_tick_ms, 4_000);
    assert_eq!(
        refreshed.damage_per_tick,
        ((7.5 + emerald.damage * 0.1) / 4.0) as f32 as f64
    );
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemDotHotOnCrit")
            .hits,
        1
    );

    iteration.process_events_through(4_000);
    // The old event and the refresh event must not both execute a tick.
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemDotHotOnCrit")
            .hits,
        2
    );
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemTargetedSpikeProc")
            .crits,
        2
    );
}

#[test]
fn aurastone_pulse_clears_nested_proc_damage_after_all_targets() {
    for targets in [1, 3] {
        let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.GemPulsatingOnAbilityTotemProc",
            [
                ("damageAccumulationFraction", 0.12),
                ("initialDamagePulseDelaySeconds", 3.0),
                ("pulseIntervalSeconds", 3.0),
            ],
        ));
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.GemTargetedSpikeProc",
            [("procChance", 1.0), ("powerCoefficient", 1.0)],
        ));
        let apl = apl([("infernal-wave", None)]);
        let mut iteration = Iteration::new(&profile, &apl, targets, 29);
        iteration.shared.heroism_until = 10_000;
        let aurastone = Arc::clone(&iteration.profile.mechanics[0]);
        iteration.spawn_aurastone(&aurastone);
        iteration.shared.aurastones[0]
            .as_mut()
            .unwrap()
            .accumulated_damage = 300.0;

        iteration.process_events_through(3_000);

        assert_eq!(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemTargetedSpikeProc")
                .hits,
            u64::from(targets)
        );
        assert_eq!(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemPulsatingOnAbilityTotemProc")
                .damage,
            36.0
        );
        assert_eq!(
            iteration
                .test_aurastone("ItemTrait.ID.GemPulsatingOnAbilityTotemProc")
                .accumulated_damage,
            0.0
        );
        let first_pulse_damage = iteration.common.result.targets.clone();
        iteration.process_events_through(6_000);
        assert_eq!(iteration.common.result.targets, first_pulse_damage);
    }
}

#[test]
fn damage_spirit_gain_uses_dummy_health_and_is_independent_of_spirit_stat() {
    for spirit in [0.0, 0.2, 1.0] {
        let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
        profile.spirit = spirit;
        let apl = apl([("infernal-wave", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 6);
        let hit = compiled_ability(&profile.abilities[0]);
        iteration.damage_hit(&hit, 100.0, 0.0, false, 0, DamageContext::NONE);
        let expected = f64::from((100.0_f64 / 6_637_912.0 * 11.25) as f32);
        assert_eq!(iteration.shared.spirit, expected, "Spirit stat {spirit}");

        iteration.shared.spirit = profile.max_spirit - 0.00001;
        iteration.damage_hit(&hit, 100.0, 0.0, false, 0, DamageContext::NONE);
        assert_eq!(iteration.shared.spirit, profile.max_spirit);
    }
}

#[test]
fn damage_spirit_caps_each_target_hit_including_proc_and_periodic_damage() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 6);
    let source = iteration.test_damage_source("test");
    for (target, provenance) in [
        (0, DamageProvenance::Direct),
        (1, DamageProvenance::Proc),
        (2, DamageProvenance::Periodic),
    ] {
        iteration.damage_unscaled_key(
            None,
            source,
            10_000_000.0,
            target,
            provenance,
            DamageContext::NONE,
        );
        assert_eq!(iteration.shared.spirit, f64::from(target + 1) * 11.25);
    }
    // An immortal dummy's minimum health does not cap TotalHealthChange;
    // repeated lethal-sized hits continue granting the per-hit maximum.
    iteration.damage_unscaled_key(
        None,
        source,
        6_637_912.0,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    assert_eq!(iteration.shared.spirit, 45.0);
    iteration.damage_unscaled_key(
        None,
        source,
        0.0,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    assert_eq!(iteration.shared.spirit, 45.0);
}

#[test]
fn armor_set_variants_trigger_and_expire_for_every_primary_attribute() {
    for attribute in ["intellect", "strength", "agility"] {
        let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
        profile.heroism_duration_ms = 38_000;
        profile.critical_strike = 1.0;
        profile.mechanics.push(ardeos_mechanic(
            &format!("seta-proc-{attribute}"),
            [
                ("procsPerMinute", 60_000.0),
                ("ppmCriticalScaling", 1.0),
                ("durationSeconds", 14.0),
                ("powerMultiplier", 1.18),
                ("cooldownSeconds", 5.0),
            ],
        ));
        profile.mechanics.push(ardeos_mechanic(
            &format!("setd-proc-{attribute}"),
            [("heroismPowerMultiplier", 1.2), ("durationSeconds", 20.0)],
        ));
        let apl = apl([("infernal-wave", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 6);
        let hit = compiled_ability(&profile.abilities[0]);
        iteration.damage_hit(&hit, 100.0, 0.0, false, 0, DamageContext::NONE);
        assert_eq!(
            iteration.test_proc_count(&format!("seta-proc-{attribute}")),
            0
        );
        iteration.damage_hit(&hit, 100.0, 0.0, true, 0, DamageContext::NONE);
        assert_eq!(
            iteration.test_proc_count(&format!("seta-proc-{attribute}")),
            1,
            "{attribute}"
        );
        assert!((iteration.effective_power_multiplier() - 1.18).abs() < 1e-9);
        iteration.damage_hit(&hit, 100.0, 0.0, true, 0, DamageContext::NONE);
        assert_eq!(
            iteration.test_proc_count(&format!("seta-proc-{attribute}")),
            1
        );
        iteration.common.now_ms = 14_000;
        assert_eq!(iteration.effective_power_multiplier(), 1.0);
        iteration.activate_heroism();
        assert!((iteration.effective_power_multiplier() - 1.2).abs() < 1e-9);
        iteration.common.now_ms = 34_000;
        assert!(iteration.shared.heroism_until > iteration.common.now_ms);
        assert_eq!(iteration.effective_power_multiplier(), 1.0, "{attribute}");
    }
}

#[test]
fn drakheim_applications_overlap_with_independent_expirations() {
    for attribute in ["intellect", "strength", "agility"] {
        let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
        profile.heroism_duration_ms = 38_000;
        profile.mechanics.push(ardeos_mechanic(
            &format!("setd-proc-{attribute}"),
            [("heroismPowerMultiplier", 1.2), ("durationSeconds", 20.0)],
        ));
        let apl = apl([("infernal-wave", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 6);
        iteration.activate_heroism();
        iteration.common.now_ms = 10_000;
        iteration.activate_heroism();
        assert!((iteration.effective_power_multiplier() - 1.44).abs() < 1e-9);
        iteration.common.now_ms = 20_000;
        assert!((iteration.effective_power_multiplier() - 1.2).abs() < 1e-9);
        iteration.common.now_ms = 30_000;
        assert_eq!(iteration.effective_power_multiplier(), 1.0);
        // A later activation must not retain expired applications.
        iteration.activate_heroism();
        assert!((iteration.effective_power_multiplier() - 1.2).abs() < 1e-9);
        iteration.common.now_ms = 50_000;
        assert_eq!(iteration.effective_power_multiplier(), 1.0);
    }
}

#[test]
fn flat_cooldown_reduction_preserves_work_across_missing_charges() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.maximum_charges = 3;
    let mut profile = profile(vec![fire_ball]);
    profile.cooldown_recovery = 2.0;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 2_000.0,
            used_charges: 3,
        },
    );
    // The API consumes base cooldown work, independently of live time rate.
    iteration.reduce_cooldown(DpsAbilityKind::FireBall, 13_000);
    let cooldown = &iteration.common.cooldowns[&DpsAbilityKind::FireBall];
    assert_eq!(cooldown.used_charges, 1);
    assert_eq!(cooldown.remaining_ms, 9_000.0);
    iteration.reduce_cooldown(DpsAbilityKind::FireBall, 9_000);
    assert!(
        !iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::FireBall)
    );
    iteration.reduce_cooldown(DpsAbilityKind::FireBall, 30_000);
    assert!(
        !iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::FireBall)
    );
}

#[test]
fn source_damage_rounding_follows_critical_and_cast_modifiers() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.critical_strike = 1.0;
    profile.expertise = 0.1;
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    let mut context = DamageContext::for_cast(1);
    context.damage_multiplier = 1.25;
    let outcome = iteration.damage_hit(
        &compiled_ability(&profile.abilities[0]),
        10.2,
        0.0,
        true,
        0,
        context,
    );
    assert!(outcome.critical);
    // 10.2 * 1.1 * 2 * 1.25 = 28.05, rounded once after source modifiers.
    assert_eq!(outcome.damage, 28.0);

    let ability = compiled_ability(&profile.abilities[0]);
    let source = iteration.ability_damage_source(&ability);
    let transferred = iteration.damage_unscaled_key(
        Some(ability.kind),
        source,
        7.5,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    // Empty-preset transfers round but do not reapply source stats or crit.
    assert_eq!(transferred.damage, 8.0);
    assert!(!transferred.critical);
}

#[test]
fn controlled_random_probability_changes_preserve_native_update_bucket() {
    let profile = profile(Vec::new());
    let apl = apl(std::iter::empty::<(&'static str, Option<AplExpressionNode>)>());
    let mut iteration = Iteration::new(&profile, &apl, 1, 17);
    assert!(iteration.roll_controlled_random_bool("initial-certain", 1.0));
    assert_eq!(
        iteration.shared.controlled_random_states["initial-certain"].chance_bucket,
        100
    );

    iteration.roll_controlled_random_bool("changed", 0.5);
    iteration.roll_controlled_random_bool("changed", 1.0);
    assert_eq!(
        iteration.shared.controlled_random_states["changed"].chance_bucket,
        99
    );
    // Keeping the same probability does not rerun initial insertion's endpoint handling.
    iteration.roll_controlled_random_bool("changed", 1.0);
    assert_eq!(
        iteration.shared.controlled_random_states["changed"].chance_bucket,
        99
    );
    let state = iteration.shared.controlled_random_states["changed"];
    assert!(!iteration.roll_controlled_random_bool("changed", 0.0));
    assert_eq!(
        iteration.shared.controlled_random_states["changed"].failure_threshold,
        state.failure_threshold
    );
    assert_eq!(
        iteration.shared.controlled_random_states["changed"].chance_factor,
        state.chance_factor
    );
}

#[test]
fn ordered_apl_skips_false_and_uncastable_actions() {
    let profile = profile(vec![
        ability(DpsAbilityKind::Detonate, 10.0),
        ability(DpsAbilityKind::InfernalWave, 1.0),
    ]);
    let false_condition = comparison(
        "false-condition",
        reference(AplNumericReference::FightElapsed),
        AplComparisonOperator::GreaterThan,
        number(10.0),
    );
    let apl = apl([
        ("detonate", None),
        ("detonate", Some(false_condition)),
        ("infernal-wave", None),
    ]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);

    assert_eq!(
        iteration.choose_action(),
        iteration
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::InfernalWave)
            .copied()
    );
    iteration.hero.ardeos_mut().embers = 1;
    assert_eq!(
        iteration.choose_action(),
        iteration
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::Detonate)
            .copied()
    );
}

#[test]
fn loadout_dependent_actions_skip_missing_weapons_and_report_them_not_ready() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let apl = apl([("weapon-frost-volley", None), ("infernal-wave", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 1);

    assert_eq!(
        iteration.choose_action(),
        iteration
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::InfernalWave)
            .copied()
    );
    assert!(!iteration.evaluate_expression(&boolean_reference(
        "missing-weapon-ready",
        AplBooleanReference::CooldownReady {
            ability_id: "weapon-arcane-channel".into(),
        },
    )));
}

#[test]
fn legendary_equipped_conditions_follow_the_compiled_loadout() {
    let condition = boolean_reference(
        "ring-equipped",
        AplBooleanReference::LegendaryEquipped {
            item_id: "legendary-ring-c-criticalstrike-haste".into(),
        },
    );
    let apl = apl([("fire-ball", Some(condition)), ("infernal-wave", None)]);
    let without_ring_profile = profile(vec![
        ability(DpsAbilityKind::FireBall, 1.0),
        ability(DpsAbilityKind::InfernalWave, 1.0),
    ]);
    let without_ring = Iteration::new(&without_ring_profile, &apl, 1, 1);
    assert_eq!(
        without_ring.choose_action(),
        without_ring
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::InfernalWave)
            .copied()
    );
    let mut with_ring_profile = profile(vec![
        ability(DpsAbilityKind::FireBall, 1.0),
        ability(DpsAbilityKind::InfernalWave, 1.0),
    ]);
    with_ring_profile.mechanics.push(DynamicMechanicInstance {
        instance_id: "ring".into(),
        source_id: "legendary-ring-c-criticalstrike-haste".into(),
        source_name: "Ring of Boomtastic Explosions".into(),
        mechanic_id: "test:legendary-equipped".into(),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::StaticStats,
        ability_kind: None,
        parameters: BTreeMap::new(),
        reason: None,
    });
    let with_ring = Iteration::new(&with_ring_profile, &apl, 1, 1);
    assert_eq!(
        with_ring.choose_action(),
        with_ring
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::FireBall)
            .copied()
    );
}

#[test]
fn fated_strike_accelerates_every_cooldown_during_its_buff() {
    let mut weapon = ability(DpsAbilityKind::WeaponCleaveCharge, 1.0);
    weapon.mechanic_parameters = BTreeMap::from([
        ("cleaveDamageMultiplier".into(), 0.4),
        ("cleaveTargetCountDamageScalingThreshold".into(), 3.0),
        ("buffDurationSeconds".into(), 6.0),
        ("buffCooldownRecoveryMultiplier".into(), 3.0),
        ("buffExpertise".into(), 0.0),
    ]);
    let profile = profile(vec![weapon, ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);

    iteration.shared.weapon_charge_buff_until = 6_000;
    assert_eq!(iteration.effective_cooldown_recovery(), 3.0);
    iteration.common.now_ms = 6_000;
    assert_eq!(iteration.effective_cooldown_recovery(), 1.0);
}

#[test]
fn fated_strike_does_not_use_the_unread_mapped_maiden_cooldown_default() {
    let mut weapon = ability(DpsAbilityKind::WeaponCleaveCharge, 1.0);
    weapon.cooldown_ms = 60_000;
    weapon.mechanic_parameters = BTreeMap::from([
        ("cleaveDamageMultiplier".into(), 0.4),
        ("cleaveTargetCountDamageScalingThreshold".into(), 3.0),
        ("buffDurationSeconds".into(), 6.0),
        ("buffCooldownRecoveryMultiplier".into(), 3.0),
        ("buffExpertise".into(), 0.0),
    ]);
    let mut profile = mara_profile();
    profile.abilities.push(weapon);
    let apl = apl([("mara-attack", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::MaidenOfDeath,
        CooldownState {
            remaining_ms: 50_000.0,
            used_charges: 1,
        },
    );
    let weapon_index = iteration.common.abilities_by_kind[&DpsAbilityKind::WeaponCleaveCharge];
    let mut context = DamageContext::for_cast(1);

    iteration.commit_ability(weapon_index, &mut context, false, 0.0);

    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::MaidenOfDeath].remaining_ms,
        50_000.0
    );
}

#[test]
fn conditions_cover_resources_buffs_dots_cooldowns_and_fight_time() {
    let profile = profile(vec![
        ability(DpsAbilityKind::InfernalWave, 1.0),
        dot_ability(DpsAbilityKind::EngulfingFlames, 0.2),
    ]);
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);
    iteration.common.now_ms = 5_000;
    iteration.hero.ardeos_mut().cinders = 50.0;
    iteration.hero.ardeos_mut().wildfire_until = 8_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::InfernalWave,
        CooldownState {
            remaining_ms: 2_000.0,
            used_charges: 1,
        },
    );
    iteration.common.dots.insert(
        (0, DotKind::Ability(DpsAbilityKind::EngulfingFlames)),
        DotState {
            model: profile.abilities[1].dot.expect("dot model"),
            source: iteration.profile.abilities[1].damage_source,
            expertise_snapshot: 0.0,
            primary_stat_multiplier_snapshot: 1.0,
            derived_damage_per_tick: None,
            gunde_rend_buckets: None,
            critical_chance_override: None,
            bonus_crit: 0.0,
            generation: 1,
            started_ms: 1_000,
            expires_ms: 9_000,
            stacks: 1,
            context: DamageContext::for_cast(1),
            last_tick_ms: 5_000,
            next_tick_ms: 6_000,
            scheduled_period_ms: 1_000,
        },
    );
    let condition = expression(
        "all",
        AplExpression::All {
            children: vec![
                boolean_reference(
                    "buff",
                    AplBooleanReference::BuffActive {
                        buff: AplBuff::Wildfire,
                    },
                ),
                boolean_reference(
                    "dot",
                    AplBooleanReference::DotActive {
                        ability_id: "engulfing-flames".into(),
                    },
                ),
                comparison(
                    "resource",
                    reference(AplNumericReference::Resource {
                        resource: AplResource::Cinders,
                        measure: AplResourceMeasure::Percent,
                    }),
                    AplComparisonOperator::Equal,
                    number(50.0),
                ),
            ],
        },
    );

    assert!(iteration.evaluate_expression(&condition));
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::CooldownRemaining {
            ability_id: "infernal-wave".into()
        }),
        2.0
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::DotRemaining {
            ability_id: "engulfing-flames".into()
        }),
        4.0
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::FightElapsed),
        5.0
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::FightRemaining),
        295.0
    );
}

#[test]
fn idle_wakeup_includes_expirations_events_and_fight_thresholds() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let condition = comparison(
        "fight-threshold",
        reference(AplNumericReference::FightElapsed),
        AplComparisonOperator::GreaterThanOrEqual,
        number(12.5),
    );
    let apl = apl([("infernal-wave", Some(condition))]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    iteration.common.now_ms = 10_000;
    iteration.hero.ardeos_mut().wildfire_until = 13_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::InfernalWave,
        CooldownState {
            remaining_ms: 4_000.0,
            used_charges: 1,
        },
    );

    assert_eq!(iteration.next_interesting_time(), 12_500);
}

#[test]
fn temporary_haste_accelerates_only_elapsed_cooldown_recovery() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.cooldown_ms = 10_000;
    fire_ball.cooldown_scales_with_haste = true;
    fire_ball.maximum_charges = 2;
    let mut profile = profile(vec![fire_ball]);
    profile.heroism_haste = 1.0;
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    iteration.shared.heroism_started_ms = Some(0);
    iteration.shared.heroism_until = 2_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 2,
        },
    );

    iteration.advance_to(5_000);

    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::FireBall),
        3_000
    );
}

#[test]
fn apl_v2_exposes_charges_target_count_and_linear_target_state() {
    let mut fire_ball = ability(DpsAbilityKind::FireBall, 1.0);
    fire_ball.maximum_charges = 2;
    fire_ball.cooldown_ms = 10_000;
    let profile = profile(vec![fire_ball]);
    let apl = apl([("fire-ball", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 7);

    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::TargetCount),
        3.0
    );

    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::CooldownCharges {
            ability_id: "fire-ball".into(),
        }),
        2.0
    );
    iteration.common.cooldowns.insert(
        DpsAbilityKind::FireBall,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::CooldownCharges {
            ability_id: "fire-ball".into(),
        }),
        1.0
    );

    for (now_ms, health, time_to_die) in [
        (0, 100.0, 300.0),
        (150_000, 50.0, 150.0),
        (300_000, 0.0, 0.0),
    ] {
        iteration.common.now_ms = now_ms;
        assert_eq!(
            iteration.evaluate_numeric_reference(&AplNumericReference::TargetHealthPercent),
            health
        );
        assert_eq!(
            iteration.evaluate_numeric_reference(&AplNumericReference::TargetTimeToDie),
            time_to_die
        );
        assert_eq!(
            iteration.evaluate_numeric_reference(&AplNumericReference::FightRemaining),
            time_to_die
        );
    }
}

#[test]
fn target_effect_references_read_registered_dot_state() {
    let dot = dot_ability(DpsAbilityKind::EngulfingFlames, 0.2);
    let mut profile = profile(vec![dot]);
    profile.apl_target_effects.push(AplTargetEffectModel {
        id: "engulfing-flames".into(),
        name: "Engulfing Flames".into(),
        source: AplTargetEffectSource::AbilityDot {
            ability_id: "engulfing-flames".into(),
        },
        effect_kind: AplTargetEffectKind::DamageOverTime,
        supported_properties: vec![
            AplTargetEffectProperty::Active,
            AplTargetEffectProperty::Remaining,
            AplTargetEffectProperty::Stacks,
        ],
    });
    let apl = apl([("engulfing-flames", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    iteration.common.now_ms = 5_000;
    iteration.common.dots.insert(
        (0, DotKind::Ability(DpsAbilityKind::EngulfingFlames)),
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
            started_ms: 1_000,
            expires_ms: 9_000,
            stacks: 3,
            context: DamageContext::for_cast(1),
            last_tick_ms: 5_000,
            next_tick_ms: 6_000,
            scheduled_period_ms: 1_000,
        },
    );

    assert!(iteration.evaluate_expression(&boolean_reference(
        "effect-active",
        AplBooleanReference::TargetEffectActive {
            effect_id: "engulfing-flames".into(),
        },
    )));
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::TargetEffectRemaining {
            effect_id: "engulfing-flames".into(),
        }),
        4.0
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::TargetEffectStacks {
            effect_id: "engulfing-flames".into(),
        }),
        3.0
    );
}

#[test]
fn target_effect_references_read_registered_mechanic_debuff_state() {
    const SOURCE_ID: &str = "ItemTrait.ID.GemSingleTargetProcOnDamageHeal";
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.push(ardeos_mechanic(SOURCE_ID, []));
    profile.apl_target_effects.push(AplTargetEffectModel {
        id: "diamond-strike".into(),
        name: "Diamond Strike".into(),
        source: AplTargetEffectSource::MechanicTargetBuff {
            mechanic_instance_id: SOURCE_ID.into(),
        },
        effect_kind: AplTargetEffectKind::Debuff,
        supported_properties: vec![
            AplTargetEffectProperty::Active,
            AplTargetEffectProperty::Remaining,
            AplTargetEffectProperty::Stacks,
        ],
    });
    let apl = apl([("infernal-wave", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    iteration.common.now_ms = 2_000;
    iteration.shared.dynamic_target_buffs[0] = Some(DynamicBuffState {
        started_ms: 1_000,
        until_ms: 7_000,
        stacks: 4,
        value: 0.0,
    });

    assert!(iteration.evaluate_expression(&boolean_reference(
        "effect-active",
        AplBooleanReference::TargetEffectActive {
            effect_id: "diamond-strike".into(),
        },
    )));
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::TargetEffectRemaining {
            effect_id: "diamond-strike".into(),
        }),
        5.0
    );
    assert_eq!(
        iteration.evaluate_numeric_reference(&AplNumericReference::TargetEffectStacks {
            effect_id: "diamond-strike".into(),
        }),
        4.0
    );
}

#[test]
fn disabled_rules_are_skipped_and_traces_retain_ids_and_short_circuits() {
    let profile = profile(vec![
        ability(DpsAbilityKind::InfernalWave, 1.0),
        ability(DpsAbilityKind::FireBall, 1.0),
    ]);
    let false_group = expression(
        "trace-group",
        AplExpression::All {
            children: vec![
                comparison(
                    "trace-fail",
                    reference(AplNumericReference::FightElapsed),
                    AplComparisonOperator::GreaterThan,
                    number(10.0),
                ),
                boolean_reference(
                    "trace-skipped",
                    AplBooleanReference::CooldownReady {
                        ability_id: "infernal-wave".into(),
                    },
                ),
            ],
        },
    );
    let mut apl = apl([
        ("infernal-wave", None),
        ("infernal-wave", Some(false_group)),
        ("fire-ball", None),
    ]);
    apl.rules[0].enabled = false;
    apl.rules[0].id = "disabled".into();
    apl.rules[1].id = "conditional".into();
    apl.rules[2].id = "fallback".into();
    let iteration = Iteration::new(&profile, &apl, 1, 9);
    let mut trace = Vec::new();

    let selected = iteration.choose_action_with_trace(Some(&mut trace));
    assert_eq!(
        selected,
        iteration
            .common
            .abilities_by_kind
            .get(&DpsAbilityKind::FireBall)
            .copied()
    );
    assert_eq!(
        trace
            .iter()
            .map(|rule| rule.rule_id.as_str())
            .collect::<Vec<_>>(),
        vec!["conditional", "fallback"]
    );
    assert!(!trace[0].passed);
    assert!(!trace[0].castable);
    assert!(
        trace[0]
            .nodes
            .iter()
            .any(|node| node.node_id == "trace-skipped" && node.short_circuited)
    );
    assert_eq!(trace[0].nodes[1].observed_values, vec![0.0, 10.0]);
    assert!(trace[1].passed && trace[1].castable);
}

#[test]
fn fixed_buff_uptime_is_refresh_safe_and_preserves_separate_windows() {
    let mut uptime = FixedBuffUptimeState::default();
    uptime.activate(0, 10_000);
    uptime.activate(5_000, 15_000);
    assert_eq!(uptime.total_active_ms(15_000), 15_000);

    uptime.activate(20_000, 25_000);
    uptime.deactivate(22_500);
    assert_eq!(uptime.total_active_ms(ENCOUNTER_DURATION_MS), 17_500);
}

#[test]
fn aggregation_reports_total_primary_and_per_target_damage() {
    let mut wave = ability(DpsAbilityKind::InfernalWave, 1.0);
    wave.max_targets = 2;
    let profile = profile(vec![wave]);
    let request = request(profile.clone(), 2, DpsEvidenceClaimStatus::Verified);
    let apl = request.action_priority_list.clone();
    let compiled_profile = CompiledProfile::compile_test(&profile);
    let iterations = (0..MIN_SIMULATION_ITERATIONS).map(|index| {
        Iteration::new(&profile, &apl, 2, index as u64)
            .run()
            .expect("iteration completes")
    });

    let result = aggregate(&request, &compiled_profile, iterations).expect("aggregate result");
    assert_eq!(result.scenario.target_count, 2);
    assert_eq!(result.evidence, request.evidence);
    assert_eq!(result.targets.len(), 2);
    assert!(result.mean_dps > result.primary_target_dps);
    assert_eq!(result.mean_dps, result.primary_target_dps * 2.0);
    assert_eq!(result.abilities[0].mean_targets_hit, 2.0);
}

#[test]
fn aggregation_publishes_sorted_named_duration_percentages_only() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.uptime_names.extend([
        ("buff:alpha".into(), "Alpha".into()),
        ("buff:zeta".into(), "Zeta".into()),
        ("dot:low".into(), "Low".into()),
    ]);
    let request = request(profile.clone(), 1, DpsEvidenceClaimStatus::Verified);
    let apl = request.action_priority_list.clone();
    let compiled_profile = CompiledProfile::compile_test(&profile);
    let iterations = (0..MIN_SIMULATION_ITERATIONS).map(|index| {
        let mut result = Iteration::new(&profile, &apl, 1, index as u64)
            .run()
            .expect("iteration completes");
        result.uptimes = BTreeMap::from([
            ("dot:low".into(), 0.2),
            ("buff:zeta".into(), 0.8),
            ("buff:alpha".into(), 0.8),
            ("proc:internal-counter".into(), 12.0),
        ]);
        result
    });

    let result = aggregate(&request, &compiled_profile, iterations).expect("aggregate result");
    assert_eq!(
        result
            .uptimes
            .iter()
            .map(|uptime| uptime.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zeta", "Low"]
    );
    assert!((result.uptimes[0].mean_uptime - 0.8).abs() < f64::EPSILON * 100.0);
    assert!((result.uptimes[2].mean_uptime - 0.2).abs() < f64::EPSILON * 100.0);
    assert!(
        result
            .uptimes
            .iter()
            .all(|uptime| uptime.id.starts_with("buff:") || uptime.id.starts_with("dot:"))
    );
}

#[test]
fn aggregation_reports_sorted_source_scoped_proc_rates_and_suppresses_zeroes() {
    let mut profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    profile.mechanics.extend([
        DynamicMechanicInstance {
            instance_id: "first-copy".into(),
            source_id: "shared-proc-source".into(),
            source_name: "Shared proc".into(),
            mechanic_id: "test:first-copy".into(),
            classification: MechanicClassification::Modeled,
            handler: DynamicMechanicHandler::StaticStats,
            ability_kind: None,
            parameters: BTreeMap::new(),
            reason: None,
        },
        DynamicMechanicInstance {
            instance_id: "second-copy".into(),
            source_id: "shared-proc-source".into(),
            source_name: "Shared proc".into(),
            mechanic_id: "test:second-copy".into(),
            classification: MechanicClassification::Modeled,
            handler: DynamicMechanicHandler::StaticStats,
            ability_kind: None,
            parameters: BTreeMap::new(),
            reason: None,
        },
    ]);
    let request = request(profile.clone(), 1, DpsEvidenceClaimStatus::Verified);
    let apl = request.action_priority_list.clone();
    let compiled_profile = CompiledProfile::compile_test(&profile);
    assert_eq!(compiled_profile.mechanic_proc_sources.len(), 1);
    let ability_index = compiled_profile.ability_proc_sources[&DpsAbilityKind::InfernalWave];
    let mechanic_index = compiled_profile.mechanic_proc_sources["shared-proc-source"];
    let iterations = (0..MIN_SIMULATION_ITERATIONS).map(|index| {
        let mut result = Iteration::new(&profile, &apl, 1, index as u64)
            .run()
            .expect("iteration completes");
        result.proc_counts[ability_index] = 12 + u64::from(index < 40);
        result.proc_counts[mechanic_index] = 20;
        result
    });

    let result = aggregate(&request, &compiled_profile, iterations).expect("aggregate result");

    assert_eq!(result.procs.len(), 2);
    assert_eq!(result.procs[0].id, "proc:mechanic:shared-proc-source");
    assert_eq!(result.procs[0].source_id, "shared-proc-source");
    assert_eq!(result.procs[0].name, "Shared proc");
    assert!((result.procs[0].mean_count - 20.0).abs() < f64::EPSILON * 100.0);
    assert!((result.procs[0].mean_per_minute - 4.0).abs() < f64::EPSILON * 100.0);
    assert_eq!(result.procs[1].id, "proc:ability:test:infernal-wave");
    assert!((result.procs[1].mean_count - 12.4).abs() < f64::EPSILON * 100.0);
    assert!((result.procs[1].mean_per_minute - 2.48).abs() < f64::EPSILON * 100.0);
    assert!(
        result
            .procs
            .iter()
            .all(|proc| proc.source_id != "spirit-refund")
    );
}

#[test]
fn profile_sweep_enforces_the_quarter_budget_margin_without_a_feature() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    let request = request(profile.clone(), 1, DpsEvidenceClaimStatus::Verified);
    let apl = request.action_priority_list.clone();
    let compiled_profile = CompiledProfile::compile_test(&profile);
    let iterations = (0..MIN_SIMULATION_ITERATIONS).map(|index| {
        let mut result = Iteration::new(&profile, &apl, 1, index as u64)
            .run()
            .expect("iteration completes");
        if index == 0 {
            result.work_units = MAX_ITERATION_WORK_UNITS / 4;
        }
        result
    });

    let error = aggregate_profile_sweep(&request, &compiled_profile, iterations)
        .expect_err("profile sweep must retain work-budget headroom");
    assert_eq!(error.code, SimulationErrorCode::SimulationFailed);
    assert!(error.message.contains("browser-generated profiles"));
}
#[test]
fn real_ppm_preserves_native_float_boundaries() {
    let established = ProcPerMinuteState {
        last_roll_seconds: 100.0,
        last_proc_seconds: 0.0,
        has_procced: true,
    };
    // Native scalar instructions at 25485623 RVA 0x52a9121..0x52a915b.
    assert_eq!(
        real_ppm_probability(120.0, established, 1.5).to_bits(),
        0x4030_0002
    );

    // This seed supplies integer draw 513. Its native normalization is
    // 0x3c804100; f32 division instead gives 0x3c804101 and rejects the proc.
    let mut probe = SplitMix64::new(13_417);
    assert_eq!(probe.next() & 0x7fff, 513);
    let mut rng = SplitMix64::new(13_417);
    assert!(rng.native_15_bit_chance(f32::from_bits(0x3c80_4101)));
    let mut rng = SplitMix64::new(13_417);
    assert!(!rng.native_15_bit_chance(f32::from_bits(0x3c80_4100)));
}

#[test]
fn all_heroes_spend_the_reduced_spirit_cost_once_without_draining_extra_capacity() {
    for (mut profile, kind) in [
        (
            profile(vec![ability(DpsAbilityKind::Incinerate, 1.0)]),
            DpsAbilityKind::Incinerate,
        ),
        (rime_profile(), DpsAbilityKind::WrathOfWinter),
        (tariq_profile(), DpsAbilityKind::RagingTempest),
        (
            elarion_profile([elarion_ability(DpsAbilityKind::EventHorizon)]),
            DpsAbilityKind::EventHorizon,
        ),
        (mara_profile(), DpsAbilityKind::MatriarchMacabre),
        (gunde_profile(), DpsAbilityKind::BloodboundSpirit),
    ] {
        profile.max_spirit = 150.0;
        profile
            .abilities
            .iter_mut()
            .find(|ability| ability.kind == kind)
            .unwrap()
            .spirit_cost = 85.0;
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 37);
        iteration.shared.spirit = 84.0;
        assert!(iteration.can_cast(kind).is_none(), "{kind:?}");
        iteration.shared.spirit = 150.0;
        let index = iteration.common.abilities_by_kind[&kind];
        iteration.commit_ability(index, &mut DamageContext::for_cast(1), false, 0.0);
        assert_eq!(iteration.shared.spirit, 65.0, "{kind:?}");
        assert_eq!(
            iteration.shared.heroism_until, profile.heroism_duration_ms,
            "{kind:?}"
        );
    }
}

#[test]
fn willful_momentum_is_shared_by_all_hero_refund_events() {
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::Detonate, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::CelestialShot)]),
        mara_profile(),
        gunde_profile(),
    ] {
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.IncreasedMainStatAndSpiritRating",
            [
                ("durationSeconds", 4.0),
                ("powerMultiplier", 1.048),
                ("spiritRating", 23.0),
            ],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 37);
        iteration.record_spirit_refund_proc();
        assert_eq!(
            iteration.dynamic_rating_bonus(parameter_key!("spiritRating")),
            0.0
        );
        assert_eq!(
            iteration.effective_power_multiplier(),
            1.048,
            "{}",
            profile.hero_id
        );
        assert_eq!(
            iteration
                .test_dynamic_buff("ItemTrait.ID.IncreasedMainStatAndSpiritRating")
                .until_ms,
            4_000
        );
        iteration.process_events_through(4_000);
        assert_eq!(iteration.effective_power_multiplier(), 1.0);
    }
}

#[test]
fn heroism_gems_switch_and_refresh_without_stacking_for_every_hero() {
    for (gem_id, capacity, cost, power, idle_haste, duration) in [
        ("gem-sapphire-80", 110.0, 95.0, 1.08, 0.02, 26_000),
        ("gem-sapphire-600", 130.0, 85.0, 1.24, 0.06, 38_000),
    ] {
        for (mut profile, kind) in [
            (
                profile(vec![ability(DpsAbilityKind::Incinerate, 1.0)]),
                DpsAbilityKind::Incinerate,
            ),
            (rime_profile(), DpsAbilityKind::WrathOfWinter),
            (tariq_profile(), DpsAbilityKind::RagingTempest),
            (
                elarion_profile([elarion_ability(DpsAbilityKind::EventHorizon)]),
                DpsAbilityKind::EventHorizon,
            ),
            (mara_profile(), DpsAbilityKind::MatriarchMacabre),
            (gunde_profile(), DpsAbilityKind::BloodboundSpirit),
        ] {
            // These are the two normalized gem tiers. Virtuoso contributes
            // idle haste but relinquishes it while ordinary Heroism is active.
            profile.max_spirit = capacity;
            profile.haste = 0.1 + idle_haste;
            profile.heroism_haste = 0.3 - idle_haste;
            profile.heroism_duration_ms = duration;
            // Isolate the shared gems from Mara's separate Spirit-ability buff.
            if kind == DpsAbilityKind::MatriarchMacabre {
                profile
                    .abilities
                    .iter_mut()
                    .find(|a| a.kind == kind)
                    .unwrap()
                    .mechanic_parameters
                    .insert("damageMultiplier".into(), 1.0);
            }
            profile
                .abilities
                .iter_mut()
                .find(|a| a.kind == kind)
                .unwrap()
                .spirit_cost = cost;
            profile
                .mechanics
                .push(ardeos_mechanic(gem_id, [("heroismPowerMultiplier", power)]));
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 1, 37);
            assert_eq!(iteration.effective_power_multiplier(), 1.0, "{kind:?}");
            assert!((iteration.effective_haste() - (0.1 + idle_haste)).abs() < 1e-10);
            iteration.shared.spirit = capacity;
            let index = iteration.common.abilities_by_kind[&kind];
            iteration.commit_ability(index, &mut DamageContext::for_cast(1), false, 0.0);
            assert_eq!(iteration.shared.spirit, capacity - cost, "{kind:?}");
            assert_eq!(iteration.shared.heroism_until, duration, "{kind:?}");
            assert_eq!(iteration.effective_power_multiplier(), power, "{kind:?}");
            assert!((iteration.effective_haste() - 0.4).abs() < 1e-10);
            iteration.advance_to(1_000);
            iteration.activate_heroism();
            iteration.advance_to(duration);
            assert_eq!(iteration.effective_power_multiplier(), power, "{kind:?}");
            assert!((iteration.effective_haste() - 0.4).abs() < 1e-10);
            iteration.advance_to(duration + 1_000);
            assert_eq!(iteration.effective_power_multiplier(), 1.0, "{kind:?}");
            assert!((iteration.effective_haste() - (0.1 + idle_haste)).abs() < 1e-10);
        }
    }
}

#[test]
fn rating_conversion_rounds_aggregate_float_ties_to_even_before_brackets() {
    for (rating, rounded) in [
        (0.49, 0.0),
        (0.5, 0.0),
        (1.5, 2.0),
        (2.5, 2.0),
        (62.5, 62.0),
        (63.5, 64.0),
        (94.5, 94.0),
        (95.5, 96.0),
        (200.49, 200.0),
    ] {
        assert_eq!(
            secondary_rating_percentage(rating),
            secondary_rating_percentage(rounded)
        );
    }
    assert!((secondary_rating_percentage(68.0) - 0.108624).abs() < 1e-12);
    assert!(
        (secondary_rating_percentage(60.25) + secondary_rating_delta(60.25, 8.0) - 0.108624).abs()
            < 1e-12
    );
    // Rounding each source independently would erase these two contributions.
    assert!((secondary_rating_delta(60.0, 0.3 + 0.3) - 0.0016).abs() < 1e-12);
}

#[test]
fn first_strike_refreshes_only_for_new_targets_without_stacking_for_every_hero() {
    for (source_id, bonus) in [("gem-emerald-80", 0.05), ("gem-emerald-600", 0.15)] {
        for mut profile in [
            profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
            rime_profile(),
            tariq_profile(),
            elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]),
            mara_profile(),
            gunde_profile(),
        ] {
            profile.mechanics = vec![ardeos_mechanic(
                source_id,
                [("expertise", bonus), ("durationSeconds", 15.0)],
            )];
            let source = compiled_ability(&profile.abilities[0]);
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 3, 41);
            let initial = iteration.effective_expertise();
            let hit = |iteration: &mut Iteration<'_>, target, amount| {
                iteration.damage_hit(&source, amount, 0.0, false, target, DamageContext::NONE);
            };
            hit(&mut iteration, 0, 0.0);
            assert_eq!(iteration.effective_expertise(), initial);
            iteration.common.now_ms = 100;
            hit(&mut iteration, 0, 100.0);
            assert!((iteration.effective_expertise() - initial - bonus).abs() < 1e-12);
            assert_eq!(iteration.test_dynamic_buff(source_id).until_ms, 15_100);

            iteration.common.now_ms = 10_000;
            hit(&mut iteration, 0, 100.0);
            assert_eq!(iteration.test_dynamic_buff(source_id).until_ms, 15_100);
            iteration.common.now_ms = 14_999;
            hit(&mut iteration, 1, 100.0);
            assert_eq!(iteration.test_dynamic_buff(source_id).until_ms, 29_999);
            assert_eq!(iteration.test_dynamic_buff(source_id).stacks, 1);
            assert!((iteration.effective_expertise() - initial - bonus).abs() < 1e-12);

            iteration.common.now_ms = 15_100;
            assert!((iteration.effective_expertise() - initial - bonus).abs() < 1e-12);
            iteration.common.now_ms = 29_999;
            assert_eq!(iteration.effective_expertise(), initial);
            hit(&mut iteration, 0, 100.0);
            assert_eq!(iteration.effective_expertise(), initial);
            hit(&mut iteration, 1, 100.0);
            assert_eq!(iteration.effective_expertise(), initial);
            hit(&mut iteration, 2, 100.0);
            assert_eq!(iteration.test_dynamic_buff(source_id).until_ms, 44_999);
            assert!((iteration.effective_expertise() - initial - bonus).abs() < 1e-12);
        }
    }
}

#[test]
fn availability_first_apl_matches_reference_with_free_casts_and_failed_conditions() {
    let profile = rime_profile();
    let condition = boolean_reference(
        "fallback-ready",
        AplBooleanReference::CooldownReady {
            ability_id: "frost-bolt".into(),
        },
    );
    let apl = apl([
        ("fire-ball", None),
        ("cold-snap", Some(condition)),
        ("frost-bolt", None),
    ]);
    for free_casts in [0, 1] {
        for fallback_on_cooldown in [false, true] {
            let mut iteration = Iteration::new(&profile, &apl, 1, 104);
            iteration.common.cooldowns.insert(
                DpsAbilityKind::ColdSnap,
                CooldownState {
                    remaining_ms: 1_000.0,
                    used_charges: 1,
                },
            );
            iteration.hero.rime_mut().navir_free_cold_snaps = free_casts;
            iteration.hero.rime_mut().navir_free_until = 12_000;
            assert_eq!(iteration.common.runtime_action_priority_list.len(), 2);
            if fallback_on_cooldown {
                iteration.common.cooldowns.insert(
                    DpsAbilityKind::FrostBolt,
                    CooldownState {
                        remaining_ms: 1_000.0,
                        used_charges: 1,
                    },
                );
            }
            assert_eq!(
                iteration.choose_action(),
                iteration.choose_action_with_trace(None)
            );
        }
    }
}

#[test]
fn all_heroes_regenerate_spirit_on_a_fixed_timer_without_damage_or_refund_procs() {
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::Incinerate, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::EventHorizon)]),
        mara_profile(),
        gunde_profile(),
    ] {
        profile.max_spirit = 130.0;
        profile.spirit = 2.0; // Secondary Spirit does not multiply the grant.
        profile.haste = 3.0; // Haste does not accelerate the native timer.
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 37);
        iteration.shared.spirit = 0.0;
        iteration.process_events_through(2_999);
        assert_eq!(iteration.shared.spirit, 0.0, "{}", profile.hero_id);
        iteration.process_events_through(3_000);
        assert_eq!(iteration.shared.spirit, 1.0, "{}", profile.hero_id);
        iteration.process_events_through(9_000);
        assert_eq!(iteration.shared.spirit, 3.0, "{}", profile.hero_id);
        assert_eq!(iteration.common.result.damage, 0.0);
        assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
        iteration.shared.spirit = 129.75;
        iteration.process_events_through(12_000);
        assert_eq!(iteration.shared.spirit, 130.0);
        iteration.process_events_through(15_000);
        iteration.shared.spirit = 45.0; // Spending does not reset tick phase.
        iteration.process_events_through(17_999);
        assert_eq!(iteration.shared.spirit, 45.0);
        iteration.process_events_through(18_000);
        assert_eq!(iteration.shared.spirit, 46.0);
    }
}

#[test]
fn all_heroes_use_the_fixed_boss_health_benchmark_for_damage_spirit() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dummy-target.json")).unwrap();
    assert_eq!(
        STATIONARY_DUMMY_MAX_HEALTH,
        reference["maxHealth"].as_f64().unwrap()
    );
    assert_eq!(
        STATIONARY_DUMMY_SPIRIT_VALUE,
        reference["spiritPointValue"].as_f64().unwrap()
    );
    for profile in [
        profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::EventHorizon)]),
        mara_profile(),
        gunde_profile(),
    ] {
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 37);
        iteration.shared.spirit = 0.0;
        let source = iteration.test_damage_source("benchmark-hit");
        iteration.damage_unscaled_key(
            None,
            source,
            10_000.0,
            0,
            DamageProvenance::Direct,
            DamageContext::NONE,
        );
        // Shared reference vector: no party income or dungeon modifiers.
        assert_eq!(
            iteration.shared.spirit, 0.016948100179433823,
            "{}",
            profile.hero_id
        );
        assert_eq!(iteration.common.result.damage, 10_000.0);
    }
}

#[test]
fn all_heroes_apply_ranked_visions_once_per_commit_for_each_weapon() {
    for base in [
        profile(vec![ability(DpsAbilityKind::Incinerate, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::EventHorizon)]),
        mara_profile(),
        gunde_profile(),
    ] {
        let spirit = base
            .abilities
            .iter()
            .find(|a| ability_category(a.kind) == AbilityCategory::Spirit)
            .unwrap()
            .clone();
        for kind in [
            DpsAbilityKind::WeaponArcaneChannel,
            DpsAbilityKind::WeaponFrostVolley,
            DpsAbilityKind::WeaponChainLightning,
            DpsAbilityKind::WeaponShadowMark,
        ] {
            for rank in 1..=4 {
                let mut profile = base.clone();
                let mut weapon = ability(kind, 0.0);
                weapon.cooldown_ms = 90_000;
                weapon
                    .mechanic_parameters
                    .insert("defaultCooldownMs".into(), 180_000.0);
                profile.abilities.push(weapon.clone());
                profile.max_spirit = 130.0;
                profile.mechanics.push(ardeos_mechanic(
                    "ItemTrait.ID.WeaponAndSpiritPoints",
                    [
                        ("spiritCooldownMultiplier", 2.5),
                        ("spiritCooldownDivider", 30.0),
                        ("weaponCooldownReductionFraction", rank as f64 * 0.25),
                    ],
                ));
                let apl = apl([]);
                let mut iteration = Iteration::new(&profile, &apl, 1, 37);
                iteration.shared.spirit = 62.0;
                iteration.trigger_dynamic_on_cast(&compiled_ability(&weapon), DamageContext::NONE);
                assert_eq!(
                    iteration.shared.spirit, 77.0,
                    "{} rank {rank}",
                    profile.hero_id
                );
                iteration.common.cooldowns.insert(
                    kind,
                    CooldownState {
                        used_charges: 1,
                        remaining_ms: 80_000.0,
                    },
                );
                iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit), DamageContext::NONE);
                assert_eq!(
                    iteration
                        .common
                        .cooldowns
                        .get(&kind)
                        .map_or(0.0, |c| c.remaining_ms),
                    80_000.0 * (1.0 - rank as f64 * 0.25)
                );
                iteration.shared.spirit = 125.0;
                iteration.trigger_dynamic_on_cast(&compiled_ability(&weapon), DamageContext::NONE);
                assert_eq!(iteration.shared.spirit, 130.0);
            }
        }
        // A Spirit commit with the trait but no equipped weapon is harmless.
        let mut profile = base.clone();
        profile.mechanics.push(ardeos_mechanic(
            "ItemTrait.ID.WeaponAndSpiritPoints",
            [
                ("spiritCooldownMultiplier", 2.5),
                ("spiritCooldownDivider", 30.0),
                ("weaponCooldownReductionFraction", 0.75),
            ],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 37);
        iteration.shared.spirit = 62.0;
        iteration.trigger_dynamic_on_cast(&compiled_ability(&spirit), DamageContext::NONE);
        assert_eq!(iteration.shared.spirit, 62.0);
    }
}

#[test]
fn prepared_equipment_has_one_ranked_counter_and_one_starting_grant_for_all_heroes() {
    use crate::preparation::{empty_character, prepare_character};
    for hero in ["firemage", "rime", "ink", "bowguy", "mara", "gunde"] {
        let mut build = empty_character(hero).unwrap();
        for (pos, item_id) in [
            ("ring-1", "ring-sete-c-haste-spirit"),
            ("ring-2", "ring-setb-c-haste-expertise"),
            ("feet", "feet-a-crit"),
            ("trinket-2", "relic-c-crit-spirit-polymorph"),
        ] {
            let ring = pos.starts_with("ring-");
            let item = serde_json::json!({
                "itemId": item_id, "itemLevel": 315, "rarity": "Heroic", "appliedTempers": 0,
                "rolledModifiers": if ring { (0..2).map(|i| serde_json::json!({
                    "slotId": format!("random:{i}:ItemTrait"), "kind": "item-trait",
                    "choiceId": "ItemTrait.ID.WeaponCritChanceCooldownReduction"
                })).collect::<Vec<_>>() } else { vec![] },
                "blessings": if ring {vec![]} else {vec![serde_json::json!({
                    "slotId": if pos == "feet" {"random:2:AbilityRank"} else {"random:0:AbilityRank"},
                    "blessingId": "DynamicItemAbilityRank.13", "rank": 1
                })]},
                "gems": [], "traitTree": null
            });
            build
                .positions
                .iter_mut()
                .find(|p| p.position_id == pos)
                .unwrap()
                .item = Some(serde_json::from_value(item).unwrap());
        }
        let (mut prepared, _) = prepare_character(&build, 1).unwrap();
        let brave: Vec<_> = prepared
            .mechanics
            .iter()
            .filter(|m| m.source_id == "ItemTrait.ID.WeaponCritChanceCooldownReduction")
            .collect();
        assert_eq!(brave.len(), 1);
        assert_eq!(brave[0].parameters["weaponCriticalStrikeBonus"], 0.32);
        assert_eq!(brave[0].parameters["weaponCooldownReductionPerCrit"], 0.3);
        let mut weapon = ability(DpsAbilityKind::WeaponFrostVolley, 0.0);
        weapon.cooldown_ms = 10_000;
        prepared.abilities.push(weapon.clone());
        prepared.cooldown_recovery = 1.0;
        prepared.spirit = 0.0;
        let apl = apl([]);
        let mut iteration = Iteration::new(&prepared, &apl, 1, 18);
        assert_eq!(
            iteration.shared.spirit, 20.0,
            "{hero}: one rank-two Herald grant"
        );
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
        iteration.process_events_through(333);
        assert_eq!(
            iteration.common.cooldowns[&weapon.kind].remaining_ms,
            6_667.0
        );
        assert_eq!(
            iteration.test_uptime(
                "proc:unsupported:traits:equipment:ItemTrait.ID.WeaponCritChanceCooldownReduction"
            ),
            1.0
        );
    }
}

#[test]
fn logged_starting_loadouts_grant_62_and_70_spirit() {
    use crate::preparation::{empty_character, prepare_character};
    for source in [
        include_str!("fixtures/apl-builds/spirit/primary-channels.json"),
        include_str!("fixtures/apl-builds/spirit/secondary-channels.json"),
    ] {
        let fixture: serde_json::Value = serde_json::from_str(source).unwrap();
        let selected = &fixture["startingLoadout"];
        let mut build = empty_character(selected["heroId"].as_str().unwrap()).unwrap();
        build.selected_talent_ids =
            serde_json::from_value(selected["selectedTalentIds"].clone()).unwrap();
        for position in selected["positions"].as_array().unwrap() {
            let p = build
                .positions
                .iter_mut()
                .find(|p| p.position_id == position["positionId"].as_str().unwrap())
                .unwrap();
            p.item = serde_json::from_value(position["item"].clone()).unwrap();
        }
        let (profile, _) = prepare_character(&build, 1).unwrap();
        let apl = apl([]);
        let iteration = Iteration::new(&profile, &apl, 1, 37);
        assert_eq!(
            iteration.shared.spirit,
            fixture["initial"]["amount"].as_f64().unwrap()
        );
    }
}
