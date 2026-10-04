use super::super::*;

#[test]
fn mara_contract_compiles_the_complete_current_kit() {
    let request = mara_request(mara_profile());

    validate(&request).expect("complete Mara request validates");
    let compiled = CompiledProfile::try_from(&request).expect("Mara profile compiles");

    assert_eq!(compiled.contract.hero, HeroIdentity::Mara);
    assert_eq!(compiled.contract.model_version, MARA_MODEL_VERSION);
    assert_eq!(compiled.abilities.len(), 19);
    assert_eq!(compiled.source.max_primary_resource, 200.0);
    assert_eq!(compiled.source.max_secondary_resource, 6);
}

#[test]
fn mara_backstab_uses_stationary_behind_position_and_builds_combo_points() {
    let profile = mara_profile();
    let apl = apl([("backstab", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 31);

    iteration.cast(9);

    assert!((iteration.ability_totals("test:backstab").damage - 140.0).abs() < 1e-10);
    assert_eq!(iteration.hero.mara().combo_points, 2);
    assert_eq!(iteration.hero.mara().energy, 195.0);
}

#[test]
fn mara_stealth_builder_applies_volatile_poison_and_fills_combo_points() {
    let profile = mara_profile();
    let apl = apl([("brooding-shadows", None), ("skittering-blades", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 32);

    iteration.cast(8);
    iteration.cast(0);

    assert!(!iteration.hero.mara().stealth_active);
    assert_eq!(iteration.hero.mara().combo_points, 1);
    assert!(
        iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::VolatilePoison),))
    );
}

#[test]
fn mara_brooding_shadows_applies_each_builder_poison_and_generates_widow_energy() {
    let mut profile = mara_profile();
    profile.abilities[2]
        .mechanic_parameters
        .insert("energyRegenerationPerSecond".into(), 0.0);

    let backstab_apl = apl([("brooding-shadows", None), ("backstab", None)]);
    let mut backstab = Iteration::new(&profile, &backstab_apl, 1, 36);
    backstab.cast(8);
    backstab.cast(9);
    assert_eq!(backstab.hero.mara().combo_points, 6);
    assert!(backstab.ability_totals("test:caustic-poison").damage > 0.0);

    let widow_apl = apl([("brooding-shadows", None), ("widows-bite", None)]);
    let mut widow = Iteration::new(&profile, &widow_apl, 1, 37);
    widow.hero.mara_mut().energy = 100.0;
    widow.cast(8);
    widow.cast(4);
    assert_eq!(widow.hero.mara().energy, 130.0);
    assert!(
        widow
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::SeethingPoison),))
    );
}

#[test]
fn mara_brooding_shadows_scales_with_haste_and_activates_assassins_guile() {
    let mut profile = mara_profile();
    profile.haste = 0.5;
    profile.talents.push(mara_talent(
        13,
        [("damageMultiplier", 1.4), ("durationSeconds", 5.0)],
    ));
    let apl = apl([
        ("brooding-shadows", None),
        ("backstab", None),
        ("queens-fang", None),
    ]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 38);

    iteration.cast(8);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        0
    );
    iteration.cast(9);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        9_333
    );
    assert_eq!(iteration.hero.mara().assassins_guile_until, 5_000);
    iteration.cast(7);

    assert!((iteration.ability_totals("test:queens-fang").damage - 308.0).abs() < 1e-10);
}

#[test]
fn mara_final_stratagem_refills_resources_and_resets_hero_cooldowns_and_charges() {
    let profile = mara_profile();
    let apl = apl([("final-stratagem", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 39);
    iteration.hero.mara_mut().energy = 25.0;
    iteration.hero.mara_mut().combo_points = 2;
    for (kind, remaining_ms, used_charges) in [
        (DpsAbilityKind::HemorrhagingStrike, 1_500.0, 1),
        (DpsAbilityKind::WidowsBite, 8_000.0, 3),
        (DpsAbilityKind::MaidenOfDeath, 50_000.0, 1),
        (DpsAbilityKind::BroodingShadows, 10_000.0, 1),
    ] {
        iteration.common.cooldowns.insert(
            kind,
            CooldownState {
                remaining_ms,
                used_charges,
            },
        );
    }

    iteration.cast(10);

    assert_eq!(iteration.hero.mara().energy, 200.0);
    assert_eq!(iteration.hero.mara().combo_points, 6);
    for kind in [
        DpsAbilityKind::HemorrhagingStrike,
        DpsAbilityKind::WidowsBite,
        DpsAbilityKind::MaidenOfDeath,
        DpsAbilityKind::BroodingShadows,
    ] {
        assert_eq!(iteration.cooldown_remaining_ms(kind), 0);
        assert!(!iteration.common.cooldowns.contains_key(&kind));
    }
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::FinalStratagem),
        180_000
    );
}

#[test]
fn mara_macabre_stratagem_replaces_the_refill_and_reset_behavior() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        8,
        [
            ("durationSeconds", 10.0),
            ("additionalDurationSeconds", 10.0),
        ],
    ));
    let apl = apl([("final-stratagem", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 40);
    iteration.hero.mara_mut().energy = 25.0;
    iteration.hero.mara_mut().combo_points = 2;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::BroodingShadows,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );

    iteration.cast(10);

    assert_eq!(iteration.hero.mara().energy, 25.0);
    assert_eq!(iteration.hero.mara().combo_points, 2);
    assert_eq!(iteration.hero.mara().matriarch_macabre_until, 10_000);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        10_000
    );
}

#[test]
fn mara_hemorrhaging_strike_scales_bleed_duration_from_spent_combo_points() {
    let profile = mara_profile();
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 33);
    iteration.hero.mara_mut().combo_points = 6;

    iteration.cast(3);

    let bleed = iteration
        .common
        .dots
        .get(&(0, DotKind::Ability(DpsAbilityKind::HemorrhagingStrike)))
        .expect("Hemorrhaging Strike bleed");
    assert_eq!(bleed.started_ms, 0);
    assert_eq!(bleed.expires_ms, 30_000);
    assert_eq!(iteration.hero.mara().combo_points, 0);
    assert_eq!(iteration.hero.mara().energy, 195.0);
}

#[test]
fn mara_legendary_branches_compile_into_typed_runtime_indexes() {
    let mut profile = mara_profile();
    profile.mechanics.extend([
        mara_legendary(
            2,
            [
                ("spiritRefundExpertise", 0.24),
                ("spiritRefundExpertiseDurationSeconds", 8.0),
            ],
        ),
        mara_legendary(
            4,
            [
                ("arachnidPoisonDamageFraction", 0.28),
                ("arachnidPoisonDurationSeconds", 6.0),
                ("arachnidPoisonPeriodSeconds", 2.0),
            ],
        ),
        mara_legendary(
            6,
            [
                ("fromShadowsProcChance", 0.15),
                ("fromShadowsBleedPeriodMultiplier", 0.87),
                ("fromShadowsDelaySeconds", 0.31),
            ],
        ),
    ]);

    let compiled =
        CompiledProfile::try_from(&mara_request(profile)).expect("Mara legendary profile compiles");

    assert!(compiled.mechanic_indexes.mara_drenched_in_blood.is_some());
    assert!(compiled.mechanic_indexes.mara_arachnid_poison.is_some());
    assert!(compiled.mechanic_indexes.mara_arachnid_clone.is_some());
}

#[test]
fn mara_spirit_refund_activates_drenched_in_blood_expertise() {
    let mut profile = mara_profile();
    profile.spirit = 1e300;
    profile.mechanics.push(mara_legendary(
        2,
        [
            ("spiritRefundExpertise", 0.24),
            ("spiritRefundExpertiseDurationSeconds", 8.0),
        ],
    ));
    let apl = apl([("queens-fang", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 34);

    iteration.try_mara_spirit_refund(40.0, 6);

    assert_eq!(iteration.test_proc_count("spirit-refund"), 1);
    assert_eq!(iteration.hero.mara().drenched_in_blood_until, 8_000);
    assert!((iteration.effective_expertise() - 0.24).abs() < 1e-10);
    iteration.process_events_through(8_000);
    assert_eq!(iteration.effective_expertise(), 0.0);
}

#[test]
fn mara_zero_cost_spender_still_rolls_spirit_without_fabricating_resources() {
    let mut profile = mara_profile();
    profile.spirit = 1.0;
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::QueensFang)
        .unwrap()
        .mechanic_parameters
        .insert("energyCost".into(), 0.0);
    let apl = apl([("queens-fang", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::QueensFang];
    iteration.cast(index);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 1);
    assert_eq!(iteration.hero.mara().combo_points, 0);
    assert_eq!(iteration.hero.mara().energy, profile.max_primary_resource);
}

#[test]
fn mara_refund_restores_both_resource_channels_after_delay_with_caps() {
    let mut profile = mara_profile();
    profile.spirit = 1.0;
    let apl = apl([("queens-fang", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    iteration.hero.mara_mut().energy = 20.0;
    iteration.hero.mara_mut().combo_points = 0;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    iteration.try_mara_spirit_refund(40.0, 6);
    assert_eq!(iteration.shared.spirit, 1.0);
    iteration.process_events_through(199);
    assert_eq!(iteration.hero.mara().combo_points, 0);
    assert!(iteration.hero.mara().energy < 30.0);
    iteration.hero.mara_mut().energy = profile.max_primary_resource - 1.0;
    iteration.hero.mara_mut().combo_points = profile.max_secondary_resource - 1;
    iteration.process_events_through(200);
    assert_eq!(
        iteration.hero.mara().combo_points,
        profile.max_secondary_resource
    );
    assert_eq!(iteration.hero.mara().energy, profile.max_primary_resource);
}

#[test]
fn vexiras_venom_requires_a_critical_queen_or_arachnid_hit() {
    for kind in [
        DpsAbilityKind::QueensFang,
        DpsAbilityKind::ArachnidAssault,
        DpsAbilityKind::SkitteringBlades,
    ] {
        for critical in [false, true] {
            let mut profile = mara_profile();
            profile.critical_strike = f64::from(critical);
            profile.mechanics.push(mara_legendary(
                4,
                [
                    ("arachnidPoisonDamageFraction", 0.28),
                    ("arachnidPoisonDurationSeconds", 6.0),
                    ("arachnidPoisonPeriodSeconds", 2.0),
                ],
            ));
            let apl = apl([("queens-fang", None)]);
            let mut iteration = Iteration::new(&profile, &apl, 1, 35);
            let index = iteration.common.abilities_by_kind[&kind];
            iteration.impact(index, true, 1.0, 0, DamageContext::for_cast(1));
            let poison = iteration.common.dots.get(&(0, DotKind::MaraArachnidPoison));
            assert_eq!(
                poison.is_some(),
                critical && kind != DpsAbilityKind::SkitteringBlades,
                "{kind:?}, crit={critical}"
            );
            if let Some(poison) = poison {
                assert!(
                    (poison.derived_damage_per_tick.unwrap() - 200.0 * 0.28 / 3.0).abs() < 1e-9
                );
            }
        }
    }
}

#[test]
fn from_shadows_uses_bleed_ticks_six_points_and_a_fresh_delayed_spec() {
    let mut profile = mara_profile();
    profile.mechanics.extend([
        mara_legendary(
            6,
            [
                ("fromShadowsProcChance", 1.0),
                ("fromShadowsBleedPeriodMultiplier", 0.87),
                ("fromShadowsDelaySeconds", 0.31),
            ],
        ),
        mara_legendary(
            4,
            [
                ("arachnidPoisonDamageFraction", 0.28),
                ("arachnidPoisonDurationSeconds", 6.0),
                ("arachnidPoisonPeriodSeconds", 2.0),
            ],
        ),
    ]);
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 35);
    let arachnid = iteration.common.abilities_by_kind[&DpsAbilityKind::ArachnidAssault];
    iteration.impact(arachnid, true, 1.0, 0, DamageContext::for_cast(1));
    assert_eq!(iteration.test_proc_count("legendary-mara-trait6"), 0);
    let bleed = iteration.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
    iteration.impact(bleed, true, 1.0, 0, DamageContext::for_cast(2));
    let key = DotKind::Ability(DpsAbilityKind::HemorrhagingStrike);
    let mut dot = iteration.common.dots[&(0, key)].clone();
    assert_eq!(dot.model.period_ms, 2_610);
    // A bleed's damage scale must not leak into the freshly constructed Queen spec.
    dot.context.multiply_damage(9.0);
    iteration.execute_dot_tick(key, &dot, 0, 1.0);
    assert_eq!(iteration.test_proc_count("legendary-mara-trait6"), 1);
    iteration.process_events_through(309);
    assert_eq!(
        iteration.ability_totals("gear:legendary-mara-trait6").hits,
        0
    );
    iteration.process_events_through(310);
    assert_eq!(
        iteration.ability_totals("gear:legendary-mara-trait6").hits,
        1
    );
    assert_eq!(
        iteration
            .ability_totals("gear:legendary-mara-trait6")
            .damage,
        220.0
    );
    assert!(
        !iteration
            .common
            .dots
            .contains_key(&(0, DotKind::MaraArachnidPoison))
    );
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
}

#[test]
fn matriarch_copies_are_delayed_separate_hits_and_cannot_feed_vexira() {
    for kind in [DpsAbilityKind::QueensFang, DpsAbilityKind::ArachnidAssault] {
        let mut profile = mara_profile();
        profile.critical_strike = 1.0;
        for ability in &mut profile.abilities {
            ability.gcd_ms = 0;
        }
        profile.mechanics.push(mara_legendary(
            4,
            [
                ("arachnidPoisonDamageFraction", 0.28),
                ("arachnidPoisonDurationSeconds", 6.0),
                ("arachnidPoisonPeriodSeconds", 2.0),
            ],
        ));
        profile.talents.push(mara_talent(
            15,
            [
                ("damageIncreasePerStack", 0.1),
                ("maximumStacks", 5.0),
                ("durationSeconds", 10.0),
            ],
        ));
        let apl = apl([("queens-fang", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 35);
        iteration.hero.mara_mut().combo_points = 6;
        iteration.hero.mara_mut().matriarch_macabre_until = 10_000;
        iteration.hero.mara_mut().feed_the_queen_stacks = 2;
        iteration.hero.mara_mut().feed_the_queen_until = 10_000;
        let index = iteration.common.abilities_by_kind[&kind];
        iteration.cast(index);
        let source = if kind == DpsAbilityKind::QueensFang {
            "test:queens-fang"
        } else {
            "test:arachnid-assault"
        };
        // Matriarch's owner buff is 1.2; Feed the Queen adds 1.2 only to Queen's original.
        let original = (440.0_f64
            * 1.2
            * if kind == DpsAbilityKind::QueensFang {
                1.2
            } else {
                1.0
            })
        .round();
        assert!(
            (iteration.ability_totals(source).damage - original).abs() < 1e-9,
            "{kind:?}: actual={}, expected={original}",
            iteration.ability_totals(source).damage
        );
        let poison = iteration.common.dots[&(0, DotKind::MaraArachnidPoison)]
            .derived_damage_per_tick
            .unwrap();
        assert!((poison - original * 0.28 / 3.0).abs() < 1e-9);
        assert_eq!(iteration.hero.mara().combo_points, 0);
        iteration.process_events_through(309);
        assert_eq!(iteration.ability_totals("test:matriarch-macabre").hits, 0);
        iteration.process_events_through(310);
        assert_eq!(iteration.ability_totals("test:matriarch-macabre").hits, 1);
        // The original spec's source capture survives expiration before copy two.
        iteration.hero.mara_mut().matriarch_macabre_until = 0;
        iteration.process_events_through(619);
        assert_eq!(iteration.ability_totals("test:matriarch-macabre").hits, 1);
        iteration.process_events_through(620);
        assert_eq!(iteration.ability_totals("test:matriarch-macabre").hits, 2);
        assert!((iteration.ability_totals("test:matriarch-macabre").damage - 528.0).abs() < 1e-9);
        assert_eq!(
            iteration.common.dots[&(0, DotKind::MaraArachnidPoison)]
                .derived_damage_per_tick
                .unwrap(),
            poison
        );
        assert_eq!(iteration.hero.mara().combo_points, 0);
        assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    }
}

#[test]
fn from_shadows_excludes_feed_the_queen_but_captures_assassins_guile() {
    let mut profile = mara_profile();
    profile.mechanics.push(mara_legendary(
        6,
        [
            ("fromShadowsProcChance", 1.0),
            ("fromShadowsBleedPeriodMultiplier", 0.87),
            ("fromShadowsDelaySeconds", 0.31),
        ],
    ));
    profile.talents.extend([
        mara_talent(
            15,
            [
                ("damageIncreasePerStack", 0.1),
                ("maximumStacks", 5.0),
                ("durationSeconds", 10.0),
            ],
        ),
        mara_talent(13, [("damageMultiplier", 1.4), ("durationSeconds", 5.0)]),
    ]);
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 35);
    iteration.hero.mara_mut().feed_the_queen_stacks = 5;
    iteration.hero.mara_mut().feed_the_queen_until = 10_000;
    iteration.hero.mara_mut().assassins_guile_until = 100;
    iteration.try_mara_from_shadows(0, DamageContext::for_cast(1));
    iteration.process_events_through(310);
    assert!(
        (iteration
            .ability_totals("gear:legendary-mara-trait6")
            .damage
            - 308.0)
            .abs()
            < 1e-9
    );
    assert_eq!(iteration.hero.mara().feed_the_queen_stacks, 5);
}

#[test]
fn hemorrhaging_refresh_carries_capped_duration_without_compounding_from_shadows_period() {
    for refresh_at in [1_000, 14_000, 16_000] {
        let mut profile = mara_profile();
        profile.mechanics.push(mara_legendary(
            6,
            [
                ("fromShadowsProcChance", 0.0),
                ("fromShadowsBleedPeriodMultiplier", 0.87),
                ("fromShadowsDelaySeconds", 0.31),
            ],
        ));
        let apl = apl([("hemorrhaging-strike", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 35);
        let index = iteration.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
        let key = (0, DotKind::Ability(DpsAbilityKind::HemorrhagingStrike));
        iteration.impact(index, true, 1.0, 0, DamageContext::for_cast(1));
        assert_eq!(iteration.common.dots[&key].expires_ms, 15_000);
        iteration.process_events_through(refresh_at);
        let previous_next = iteration.common.dots.get(&key).map(|d| d.next_tick_ms);
        iteration.impact(index, true, 1.0, 0, DamageContext::for_cast(2));
        let dot = &iteration.common.dots[&key];
        assert_eq!(dot.model.period_ms, 2_610);
        let carry = 15_000_u64.saturating_sub(refresh_at).min(4_500);
        assert_eq!(dot.expires_ms, refresh_at + 15_000 + carry);
        if refresh_at < 15_000 {
            assert_eq!(Some(dot.next_tick_ms), previous_next);
        } else {
            assert_eq!(dot.next_tick_ms, refresh_at + 2_610);
        }
    }
}

#[test]
fn matriarch_dispatches_each_stacked_target_before_starting_the_next_copy() {
    let mut profile = mara_profile();
    for ability in &mut profile.abilities {
        ability.gcd_ms = 0;
    }
    let apl = apl([("arachnid-assault", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 35);
    iteration.hero.mara_mut().combo_points = 6;
    iteration.hero.mara_mut().matriarch_macabre_until = 10_000;
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::ArachnidAssault];
    iteration.cast(index);
    for (at, hits) in [
        (309, 0),
        (310, 1),
        (320, 2),
        (330, 3),
        (639, 3),
        (640, 4),
        (650, 5),
        (660, 6),
    ] {
        iteration.process_events_through(at);
        assert_eq!(
            iteration.ability_totals("test:matriarch-macabre").hits,
            hits
        );
    }
    assert_eq!(
        iteration.ability_totals("test:matriarch-macabre").damage,
        792.0
    );
    assert_eq!(
        iteration.ability_totals("test:arachnid-assault").damage,
        792.0
    );
}

#[test]
fn hemorrhaging_carry_cap_uses_the_new_spenders_combo_point_duration() {
    let mut profile = mara_profile();
    for ability in &mut profile.abilities {
        ability.gcd_ms = 0;
    }
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 35);
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
    iteration.hero.mara_mut().combo_points = 6;
    iteration.cast(index);
    let key = (0, DotKind::Ability(DpsAbilityKind::HemorrhagingStrike));
    assert_eq!(iteration.common.dots[&key].expires_ms, 30_000);
    iteration.process_events_through(2_000);
    iteration.hero.mara_mut().combo_points = 1;
    iteration.cast(index);
    assert_eq!(
        iteration.common.dots[&key].expires_ms,
        2_000 + 15_000 + 4_500
    );
    assert_eq!(iteration.common.dots[&key].next_tick_ms, 3_000);
}

#[test]
fn malevolence_charges_have_flat_damage_consume_one_and_refresh_the_opposite_buff() {
    let mut profile = mara_profile();
    for a in &mut profile.abilities {
        a.gcd_ms = 0;
    }
    profile.talents.push(mara_talent(
        2,
        [
            ("damageMultiplier", 2.0),
            ("maximumStacks", 2.0),
            ("durationSeconds", 20.0),
        ],
    ));
    let apl = apl([("queens-fang", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    let queen = it.common.abilities_by_kind[&DpsAbilityKind::QueensFang];
    let arachnid = it.common.abilities_by_kind[&DpsAbilityKind::ArachnidAssault];
    // Two Queen casts stock two Arachnid charges. Their presence must not
    // triple Arachnid damage or survive unlimited casts of the same spender.
    for _ in 0..2 {
        it.hero.mara_mut().combo_points = 6;
        it.hero.mara_mut().energy = 200.0;
        it.cast(queen);
    }
    assert_eq!(it.hero.mara().malevolence_arachnid_stacks, 2);
    for expected in [440.0, 880.0, 1100.0] {
        it.hero.mara_mut().combo_points = 6;
        it.hero.mara_mut().energy = 200.0;
        it.cast(arachnid);
        assert_eq!(it.ability_totals("test:arachnid-assault").damage, expected);
    }
    assert_eq!(it.hero.mara().malevolence_arachnid_stacks, 0);
    assert_eq!(it.hero.mara().malevolence_queen_stacks, 2);
    it.process_events_through(20_000);
    it.hero.mara_mut().combo_points = 6;
    it.hero.mara_mut().energy = 200.0;
    it.cast(queen);
    assert_eq!(it.ability_totals("test:queens-fang").damage, 660.0);
    assert_eq!(it.hero.mara().malevolence_arachnid_stacks, 1);
}

#[test]
fn efficient_killer_follows_energy_cost_and_uses_maiden_scaling_for_each_point() {
    for kind in [
        DpsAbilityKind::QueensFang,
        DpsAbilityKind::ArachnidAssault,
        DpsAbilityKind::HemorrhagingStrike,
    ] {
        for maiden in [false, true] {
            let mut profile = mara_profile();
            for a in &mut profile.abilities {
                a.gcd_ms = 0;
            }
            profile
                .talents
                .push(mara_talent(6, [("energyPerComboPoint", 1.0)]));
            profile.talents.push(mara_talent(
                12,
                [("cooldownReductionPerComboPointSeconds", 0.6)],
            ));
            let apl = apl([("queens-fang", None)]);
            let mut it = Iteration::new(&profile, &apl, 1, 42);
            it.hero.mara_mut().combo_points = 6;
            if maiden {
                it.hero.mara_mut().maiden_of_death_until = 10_000;
            }
            it.common.cooldowns.insert(
                DpsAbilityKind::MaidenOfDeath,
                CooldownState {
                    remaining_ms: 50_000.0,
                    used_charges: 1,
                },
            );
            let index = it.common.abilities_by_kind[&kind];
            let cost = ability_param(&it.profile.abilities[index], parameter_key!("energyCost"));
            it.cast(index);
            let expected = 200.0 - cost + 6.0 * if maiden { 1.2 } else { 1.0 };
            assert!(
                (it.hero.mara().energy - expected).abs() < 1e-9,
                "{kind:?}, maiden={maiden}"
            );
            assert_eq!(
                it.cooldown_remaining_ms(DpsAbilityKind::MaidenOfDeath),
                46_400
            );
            assert_eq!(it.hero.mara().combo_points, 0);
        }
    }
}

#[test]
fn deadly_scheme_keeps_partial_energy_and_promotes_only_after_the_threshold_attack() {
    let mut profile = mara_profile();
    for a in &mut profile.abilities {
        a.gcd_ms = 0;
    }
    profile.talents.push(mara_talent(
        3,
        [
            ("energyPerStack", 5.0),
            ("maximumStacks", 40.0),
            ("durationSeconds", 12.0),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let apl = apl([("queens-fang", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    it.accumulate_mara_deadly_scheme(2.0);
    it.accumulate_mara_deadly_scheme(3.0);
    assert_eq!(it.hero.mara().deadly_scheme_stacks, 1);
    it.accumulate_mara_deadly_scheme(160.0);
    assert_eq!(it.hero.mara().deadly_scheme_stacks, 33);
    let queen = it.common.abilities_by_kind[&DpsAbilityKind::QueensFang];
    it.hero.mara_mut().combo_points = 6;
    it.cast(queen);
    assert_eq!(it.ability_totals("test:queens-fang").damage, 220.0);
    assert_eq!(it.hero.mara().deadly_scheme_until, 12_000);
    assert_eq!(it.hero.mara().deadly_scheme_overflow, 1);
    it.hero.mara_mut().combo_points = 6;
    it.cast(queen);
    assert_eq!(it.ability_totals("test:queens-fang").damage, 660.0);
    assert_eq!(it.hero.mara().deadly_scheme_until, 0);
    assert_eq!(it.hero.mara().deadly_scheme_stacks, 9);
    assert_eq!(it.hero.mara().deadly_scheme_overflow, 0);
}

#[test]
fn feed_the_queen_counts_each_builder_target_and_original_queen_damage_consumes_it() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        15,
        [
            ("damageIncreasePerStack", 0.1),
            ("maximumStacks", 5.0),
            ("durationSeconds", 10.0),
        ],
    ));
    let apl = apl([("skittering-blades", None)]);
    let mut it = Iteration::new(&profile, &apl, 3, 42);
    let builder = it.common.abilities_by_kind[&DpsAbilityKind::SkitteringBlades];
    for target in 0..3 {
        it.impact(builder, true, 1.0, target, DamageContext::for_cast(1));
    }
    assert_eq!(it.hero.mara().feed_the_queen_stacks, 3);
    for target in 0..3 {
        it.impact(builder, true, 1.0, target, DamageContext::for_cast(2));
    }
    assert_eq!(it.hero.mara().feed_the_queen_stacks, 5);
    let queen = it.common.abilities_by_kind[&DpsAbilityKind::QueensFang];
    it.impact(queen, true, 1.0, 0, DamageContext::for_cast(3));
    assert_eq!(it.hero.mara().feed_the_queen_stacks, 0);
    it.impact(builder, true, 1.0, 0, DamageContext::for_cast(4));
    it.process_events_through(10_000);
    it.impact(builder, true, 1.0, 1, DamageContext::for_cast(5));
    assert_eq!(it.hero.mara().feed_the_queen_stacks, 1);
}

#[test]
fn bloodrush_and_from_shadows_compose_without_accelerating_each_refresh_again() {
    let mut profile = mara_profile();
    profile
        .talents
        .push(mara_talent(19, [("tickRateMultiplier", 1.2)]));
    profile.mechanics.push(mara_legendary(
        6,
        [
            ("fromShadowsProcChance", 0.0),
            ("fromShadowsBleedPeriodMultiplier", 0.87),
            ("fromShadowsDelaySeconds", 0.31),
        ],
    ));
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    let bleed = it.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
    for at in [0, 1000, 2000] {
        it.process_events_through(at);
        it.impact(bleed, true, 1.0, 0, DamageContext::for_cast(at));
        let dot = &it.common.dots[&(0, DotKind::Ability(DpsAbilityKind::HemorrhagingStrike))];
        assert_eq!(dot.model.period_ms, 2175);
        assert_eq!(dot.next_tick_ms, 2175);
    }
}

#[test]
fn arachnid_onslaught_checks_each_original_target_and_does_not_leak_to_copy_contexts() {
    let mut profile = mara_profile();
    for a in &mut profile.abilities {
        a.gcd_ms = 0;
    }
    profile
        .talents
        .push(mara_talent(18, [("damageMultiplier", 1.3)]));
    let apl = apl([("arachnid-assault", None)]);
    let mut it = Iteration::new(&profile, &apl, 3, 42);
    let bleed = it.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
    // The primary target is not bleeding. Only secondary target one qualifies.
    it.impact(bleed, true, 1.0, 1, DamageContext::for_cast(1));
    let arachnid = it.common.abilities_by_kind[&DpsAbilityKind::ArachnidAssault];
    it.hero.mara_mut().combo_points = 6;
    it.hero.mara_mut().matriarch_macabre_until = 10_000;
    it.cast(arachnid);
    assert_eq!(
        it.ability_totals("test:arachnid-assault").damage,
        264.0 + 343.0 + 264.0
    );
    it.process_events_through(660);
    assert_eq!(it.ability_totals("test:matriarch-macabre").damage, 792.0);
    it.process_events_through(15_001);
    let before = it.ability_totals("test:arachnid-assault").damage;
    it.impact(arachnid, true, 1.0, 1, DamageContext::for_cast(3));
    assert_eq!(
        it.ability_totals("test:arachnid-assault").damage - before,
        100.0
    );
}

#[test]
fn macabre_stratagem_uses_separate_initial_and_extension_durations() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        8,
        [("durationSeconds", 7.0), ("additionalDurationSeconds", 3.0)],
    ));
    let apl = apl([("final-stratagem", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    let ability = it.ability(DpsAbilityKind::FinalStratagem).unwrap().clone();
    it.commit_mara_ability(&ability);
    assert_eq!(it.hero.mara().matriarch_macabre_until, 7_000);
    it.process_events_through(2_000);
    it.commit_mara_ability(&ability);
    assert_eq!(it.hero.mara().matriarch_macabre_until, 10_000);
    it.process_events_through(10_000);
    it.commit_mara_ability(&ability);
    assert_eq!(it.hero.mara().matriarch_macabre_until, 17_000);
    // The extension also applies to an existing spirit-ability buff.
    it.hero.mara_mut().matriarch_macabre_until = 30_000;
    it.commit_mara_ability(&ability);
    assert_eq!(it.hero.mara().matriarch_macabre_until, 33_000);
}

#[test]
fn maiden_scales_bleed_energy_but_not_direct_widow_or_spirit_refunds() {
    let mut profile = mara_profile();
    for a in &mut profile.abilities {
        a.gcd_ms = 0;
    }
    profile.abilities[2]
        .mechanic_parameters
        .insert("energyRegenerationPerSecond".into(), 0.0);
    let apl = apl([("hemorrhaging-strike", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    it.hero.mara_mut().maiden_of_death_until = 5_000;
    let bleed = it.common.abilities_by_kind[&DpsAbilityKind::HemorrhagingStrike];
    let per_tick = ability_param(
        &it.profile.abilities[bleed],
        parameter_key!("energyPerBleedTick"),
    );
    it.impact(bleed, true, 1.0, 0, DamageContext::for_cast(1));
    it.hero.mara_mut().energy = 50.0;
    it.process_events_through(3_000);
    assert!((it.hero.mara().energy - (50.0 + per_tick * 1.2)).abs() < 1e-9);
    it.hero.mara_mut().energy = 50.0;
    let widow = it.common.abilities_by_kind[&DpsAbilityKind::WidowsBite];
    it.cast(widow);
    assert_eq!(it.hero.mara().energy, 80.0);
    it.handle_mara_event(MaraEvent::ResourceRefund {
        energy_bits: 10.0_f64.to_bits(),
        combo_points: 1,
    });
    assert_eq!(it.hero.mara().energy, 90.0);
    it.process_events_through(6_000);
    assert_eq!(it.hero.mara().energy, 90.0 + per_tick);
    it.hero.mara_mut().maiden_of_death_until = 10_000;
    it.hero.mara_mut().energy = 199.0;
    it.gain_mara_energy(10.0);
    assert_eq!(it.hero.mara().energy, 200.0);
}

#[test]
fn deadly_scheme_end_filter_includes_widow_and_excludes_auto_attacks_and_buffs() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        3,
        [
            ("energyPerStack", 5.0),
            ("maximumStacks", 40.0),
            ("durationSeconds", 12.0),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let apl = apl([("widows-bite", None)]);
    let mut it = Iteration::new(&profile, &apl, 1, 42);
    it.accumulate_mara_deadly_scheme(200.0);
    for kind in [
        DpsAbilityKind::MaraAttack,
        DpsAbilityKind::MaidenOfDeath,
        DpsAbilityKind::FinalStratagem,
    ] {
        it.finish_mara_ability(kind);
        assert_eq!(it.hero.mara().deadly_scheme_until, 0);
    }
    it.finish_mara_ability(DpsAbilityKind::WidowsBite);
    assert_eq!(it.hero.mara().deadly_scheme_until, 12_000);
    it.accumulate_mara_deadly_scheme(200.0);
    // Consuming an old charge and reaching the threshold on the same attack
    // rearms the proc. Expiration removes the bonus without discarding progress.
    it.finish_mara_ability(DpsAbilityKind::QueensFang);
    assert_eq!(it.hero.mara().deadly_scheme_until, 12_000);
    it.accumulate_mara_deadly_scheme(12.0);
    it.process_events_through(12_000);
    assert_eq!(it.buff_stacks(AplBuff::DeadlyScheme), 2);
    assert_eq!(it.hero.mara().deadly_scheme_energy_remainder, 2.0);
}

#[test]
fn mara_guile_buffs_the_next_spender_not_the_stealth_breaking_spec() {
    for index in [1, 7] {
        let mut profile = mara_profile();
        profile.talents.push(mara_talent(
            13,
            [("damageMultiplier", 1.4), ("durationSeconds", 5.0)],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 111);
        iteration.cast(8);
        iteration.hero.mara_mut().combo_points = 6;
        iteration.cast(index);
        let id = &profile.abilities[index].id;
        let opening = iteration.ability_totals(id).damage;
        assert_eq!(opening, 220.0);
        assert!(!iteration.hero.mara().stealth_active);
        assert!(iteration.hero.mara().assassins_guile_until > iteration.common.now_ms);
        iteration.hero.mara_mut().combo_points = 6;
        iteration.cast(index);
        assert_eq!(iteration.ability_totals(id).damage - opening, 308.0);
        iteration.process_events_through(5_000);
        iteration.hero.mara_mut().combo_points = 6;
        iteration.cast(index);
        assert_eq!(iteration.ability_totals(id).damage - opening - 308.0, 220.0);
    }
}

#[test]
fn mara_stealth_cooldown_begins_on_health_loss_and_manual_toggle_grants_no_guile() {
    let mut profile = mara_profile();
    profile.haste = 0.5;
    profile.talents.push(mara_talent(
        13,
        [("damageMultiplier", 1.4), ("durationSeconds", 5.0)],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 112);
    iteration.cast(8);
    iteration.process_events_through(2_000);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        0
    );
    iteration.cast(8);
    assert!(!iteration.hero.mara().stealth_active);
    assert_eq!(iteration.hero.mara().assassins_guile_until, 0);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        0
    );
    iteration.cast(8);
    let hit = iteration.ability(DpsAbilityKind::Backstab).unwrap().clone();
    iteration.damage_hit(&hit, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert!(!iteration.hero.mara().stealth_active);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BroodingShadows),
        10_000
    );
    assert_eq!(
        iteration.hero.mara().assassins_guile_until,
        iteration.common.now_ms + 5_000
    );
}

#[test]
fn mara_stealth_ignores_poison_periodic_copy_and_missed_damage() {
    let mut profile = mara_profile();
    profile.mechanics.push(mara_legendary(
        6,
        [
            ("fromShadowsProcChance", 0.0),
            ("fromShadowsBleedPeriodMultiplier", 0.87),
            ("fromShadowsDelaySeconds", 0.31),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 113);
    iteration.cast(8);
    for kind in [DpsAbilityKind::CausticPoison, DpsAbilityKind::SeethingBurst] {
        let poison = iteration.ability(kind).unwrap().clone();
        iteration.damage_hit(&poison, 100.0, 0.0, true, 0, DamageContext::NONE);
        assert!(iteration.hero.mara().stealth_active);
    }
    iteration.apply_mara_dot(
        0,
        DpsAbilityKind::HemorrhagingStrike,
        DamageContext::NONE,
        1.0,
    );
    let kind = DotKind::Ability(DpsAbilityKind::HemorrhagingStrike);
    let dot = iteration.common.dots[&(0, kind)].clone();
    iteration.execute_dot_tick(kind, &dot, 0, 1.0);
    assert!(iteration.hero.mara().stealth_active);
    let queen = iteration
        .ability(DpsAbilityKind::QueensFang)
        .unwrap()
        .clone();
    let matriarch = iteration
        .ability(DpsAbilityKind::MatriarchMacabre)
        .unwrap()
        .damage_source;
    let clone = iteration
        .profile
        .mechanic_indexes
        .mara_arachnid_clone
        .unwrap();
    for source in [matriarch, iteration.profile.mechanics[clone].damage_source] {
        iteration.damage_raw_key(
            Some(queen.kind),
            source,
            100.0,
            0.0,
            true,
            false,
            0,
            DamageContext::NONE,
        );
        assert!(iteration.hero.mara().stealth_active);
    }
    let seed = (0..1000)
        .find(|seed| SplitMix64::new(*seed).next() as f64 / (u64::MAX as f64) < 0.05)
        .unwrap();
    iteration.common.rng = SplitMix64::new(seed);
    let auto = iteration
        .ability(DpsAbilityKind::MaraAttack)
        .unwrap()
        .clone();
    assert_eq!(
        iteration
            .damage_hit(&auto, 100.0, 0.0, false, 0, DamageContext::NONE)
            .damage,
        0.0
    );
    assert!(iteration.hero.mara().stealth_active);
    iteration.damage_hit(&queen, 0.0, 0.0, false, 0, DamageContext::NONE);
    assert!(iteration.hero.mara().stealth_active);
}

#[test]
fn venomous_delight_refunds_once_per_poison_health_loss_without_maiden_scaling() {
    let mut profile = mara_profile();
    profile
        .talents
        .push(mara_talent(5, [("procChance", 1.0), ("energyGain", 10.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 114);
    iteration.hero.mara_mut().energy = 100.0;
    iteration.hero.mara_mut().maiden_of_death_until = 10_000;
    let poison = iteration
        .ability(DpsAbilityKind::CausticPoison)
        .unwrap()
        .clone();
    iteration.damage_hit(&poison, 100.0, 1.0, true, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.mara().energy, 110.0);
    iteration.damage_hit(&poison, 0.0, 0.0, false, 0, DamageContext::NONE);
    let queen = iteration
        .ability(DpsAbilityKind::QueensFang)
        .unwrap()
        .clone();
    iteration.damage_hit(&queen, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.mara().energy, 110.0);
    iteration.apply_mara_dot(0, DpsAbilityKind::SeethingPoison, DamageContext::NONE, 1.0);
    let kind = DotKind::Ability(DpsAbilityKind::SeethingPoison);
    let dot = iteration.common.dots[&(0, kind)].clone();
    iteration.execute_dot_tick(kind, &dot, 0, 1.0);
    assert_eq!(iteration.hero.mara().energy, 120.0);
    assert_eq!(iteration.test_proc_count("mara-talent-id-talent5"), 2);
    iteration.hero.mara_mut().energy = 195.0;
    iteration.damage_hit(&poison, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.mara().energy, 200.0);
}

#[test]
fn finesse_poison_refunds_venomous_delight_and_breaks_stealth() {
    let mut profile = mara_profile();
    profile
        .talents
        .push(mara_talent(5, [("procChance", 1.0), ("energyGain", 10.0)]));
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.06",
        [
            ("hitThreshold", 1.0),
            ("criticalStrikeCap", 0.5),
            ("targetCountDamageScalingThreshold", 5.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 115);
    iteration.cast(8);
    iteration.hero.mara_mut().energy = 100.0;
    let source = iteration.profile.mechanics[iteration.profile.mechanic_indexes.basic_to_aoe[0]]
        .damage_source;
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    assert!(!iteration.hero.mara().stealth_active);
    assert_eq!(iteration.hero.mara().energy, 110.0);
    assert_eq!(iteration.test_proc_count("mara-talent-id-talent5"), 1);
}

#[test]
fn caustic_wounds_requires_owned_target_bleed_and_adds_to_stealth_poison() {
    for (bleed_target, stealth, chance, expected_hits) in [
        (None, false, 1.0, 0),
        (Some(1), false, 1.0, 0),
        (Some(0), false, 0.0, 0),
        (Some(0), false, 1.0, 1),
        (Some(0), true, 1.0, 2),
    ] {
        let mut profile = mara_profile();
        profile
            .talents
            .push(mara_talent(11, [("procChance", chance)]));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 2, 116);
        if let Some(target) = bleed_target {
            iteration.apply_mara_dot(
                target,
                DpsAbilityKind::HemorrhagingStrike,
                DamageContext::NONE,
                1.0,
            );
        }
        if stealth {
            iteration.cast(8);
        }
        iteration.cast(9);
        assert_eq!(
            iteration.ability_totals("test:caustic-poison").hits,
            expected_hits
        );
        assert_eq!(
            iteration.ability_totals("test:caustic-poison").damage,
            expected_hits as f64 * 200.0
        );
    }
}

#[test]
fn seething_burst_uses_fresh_stats_falloff_and_each_target_can_refund_energy() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(10, [("procChance", 1.0)]));
    profile
        .talents
        .push(mara_talent(5, [("procChance", 1.0), ("energyGain", 10.0)]));
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::SeethingBurst)
        .unwrap()
        .mechanic_parameters
        .insert("targetCountDamageScalingThreshold".into(), 1.0);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 4, 117);
    let mut old_context = DamageContext::for_cast(1)
        .with_snapshot(iteration.capture_damage_source_snapshot(DpsAbilityKind::SeethingPoison));
    old_context.multiply_damage(7.0);
    iteration.apply_mara_dot(0, DpsAbilityKind::SeethingPoison, old_context, 1.0);
    iteration.hero.mara_mut().maiden_of_death_until = 10_000;
    iteration.hero.mara_mut().energy = 100.0;
    let key = DotKind::Ability(DpsAbilityKind::SeethingPoison);
    let dot = iteration.common.dots[&(0, key)].clone();
    iteration.execute_dot_tick(key, &dot, 0, 1.0);
    assert_eq!(iteration.ability_totals("test:seething-burst").hits, 4);
    assert_eq!(
        iteration.ability_totals("test:seething-burst").damage,
        240.0
    );
    assert_eq!(iteration.hero.mara().energy, 150.0);
    assert_eq!(iteration.test_proc_count("mara-talent-id-talent10"), 1);
    assert_eq!(iteration.test_proc_count("mara-talent-id-talent5"), 5);
    assert_eq!(iteration.common.dots[&(0, key)].expires_ms, dot.expires_ms);
}

#[test]
fn puncture_guarantees_both_widow_crits_without_buffing_its_poison() {
    let mut profile = mara_profile();
    profile.abilities[4].hit_interval_ms = 100;
    profile
        .talents
        .push(mara_talent(17, [("criticalStrikeBonus", 1.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 118);
    iteration.cast(8);
    iteration.cast(4);
    assert_eq!(iteration.ability_totals("test:widows-bite").hits, 2);
    assert_eq!(iteration.ability_totals("test:widows-bite").crits, 2);
    assert_eq!(iteration.ability_totals("test:widows-bite").damage, 350.0);
    let key = DotKind::Ability(DpsAbilityKind::SeethingPoison);
    let dot = iteration.common.dots[&(0, key)].clone();
    iteration.execute_dot_tick(key, &dot, 0, 1.0);
    assert_eq!(iteration.ability_totals("test:seething-poison").crits, 0);
}

#[test]
fn mara_excluded_gem_damage_preserves_stealth_while_a_weapon_hit_breaks_it() {
    let mut profile = mara_profile();
    // Distinct registered sources let this exercise the monitor's cooked
    // effect-tag exclusions without depending on the gems' random triggers.
    for source in [
        "ItemTrait.ID.GemSingleTargetProcOnDamageHeal",
        "ItemTrait.ID.GemTargetedSpikeProc",
    ] {
        let mut effect = ability(DpsAbilityKind::WeaponInstantAoe, 1.0);
        effect.id = format!("gear:{source}");
        profile.abilities.push(effect);
    }
    profile
        .abilities
        .push(ability(DpsAbilityKind::WeaponInstantAoe, 1.0));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 119);
    iteration.cast(8);
    for model in &profile.abilities[19..21] {
        iteration.damage_hit(
            &compiled_ability(model),
            100.0,
            0.0,
            false,
            0,
            DamageContext::NONE,
        );
        assert!(iteration.hero.mara().stealth_active);
    }
    let weapon = compiled_ability(&ability(DpsAbilityKind::WeaponInstantAoe, 1.0));
    iteration.damage_hit(&weapon, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert!(!iteration.hero.mara().stealth_active);
}

#[test]
fn caustic_wounds_observes_backstab_application_even_without_health_loss() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(11, [("procChance", 1.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 120);
    iteration.apply_mara_dot(
        0,
        DpsAbilityKind::HemorrhagingStrike,
        DamageContext::NONE,
        1.0,
    );
    iteration.impact(9, true, 0.0, 0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:backstab").damage, 0.0);
    assert_eq!(iteration.ability_totals("test:caustic-poison").hits, 1);
    assert_eq!(iteration.hero.mara().combo_points, 0);
}

#[test]
fn venomous_delight_receives_vexiras_pool_ticks() {
    let mut profile = mara_profile();
    profile.critical_strike = 1.0;
    profile
        .talents
        .push(mara_talent(5, [("procChance", 1.0), ("energyGain", 10.0)]));
    profile.mechanics.push(mara_legendary(
        4,
        [
            ("arachnidPoisonDamageFraction", 0.28),
            ("arachnidPoisonDurationSeconds", 6.0),
            ("arachnidPoisonPeriodSeconds", 2.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 121);
    iteration.hero.mara_mut().energy = 100.0;
    iteration.impact(7, true, 1.0, 0, DamageContext::for_cast(1));
    assert_eq!(iteration.hero.mara().energy, 100.0);
    let dot = iteration.common.dots[&(0, DotKind::MaraArachnidPoison)].clone();
    iteration.execute_dot_tick(DotKind::MaraArachnidPoison, &dot, 0, 1.0);
    assert_eq!(iteration.hero.mara().energy, 110.0);
    assert_eq!(iteration.test_proc_count("mara-talent-id-talent5"), 1);
}

#[test]
fn hemotoxin_rolls_once_on_commit_even_when_the_auto_misses() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(14, [("procChance", 1.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 191);
    let auto = iteration
        .ability(DpsAbilityKind::MaraAttack)
        .unwrap()
        .clone();
    iteration.commit_mara_ability(&auto);
    let key = (0, DotKind::Ability(DpsAbilityKind::Hemotoxin));
    assert_eq!(iteration.common.dots[&key].stacks, 1);
    let miss_seed = (0..1000)
        .find(|seed| SplitMix64::new(*seed).next() as f64 / (u64::MAX as f64) < 0.05)
        .unwrap();
    iteration.common.rng = SplitMix64::new(miss_seed);
    iteration.impact(2, true, 1.0, 0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:mara-attack").hits, 0);
    assert_eq!(iteration.common.dots[&key].stacks, 1);
    iteration.cast(4);
    iteration.process_events_through(2_000);
    assert_eq!(
        iteration.common.dots[&key].stacks, 2,
        "two Widow hits are one commit"
    );
    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Mara.Talent.BleedAttackSpender.ExplosivePoison")
    );
}

#[test]
fn hemotoxin_stack_cap_refresh_preserves_phase_and_does_not_inherit_creeping_death() {
    let mut profile = mara_profile();
    profile.haste = 0.5;
    profile.talents.push(mara_talent(14, [("procChance", 1.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 192);
    let auto = iteration
        .ability(DpsAbilityKind::MaraAttack)
        .unwrap()
        .clone();
    iteration.commit_mara_ability(&auto);
    let key = (0, DotKind::Ability(DpsAbilityKind::Hemotoxin));
    let next_tick = iteration.common.dots[&key].next_tick_ms;
    iteration.advance_to(100);
    for _ in 0..4 {
        iteration.commit_mara_ability(&auto);
    }
    let dot = &iteration.common.dots[&key];
    assert_eq!(dot.stacks, 3);
    assert_eq!(dot.expires_ms, 11_800);
    assert_eq!(dot.next_tick_ms, next_tick);
    assert_eq!(dot.context.damage_multiplier, 1.0);
}

#[test]
fn hemotoxin_eruption_samples_refreshed_bleed_and_scales_only_secondary_targets() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(14, [("procChance", 0.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 193);
    let bleed = iteration
        .ability(DpsAbilityKind::HemorrhagingStrike)
        .unwrap()
        .clone();
    let mut model = bleed.dot.unwrap();
    model.duration_ms = 10_000;
    model.period_ms = 1_000;
    model.power_coefficient = 1.0;
    iteration.apply_dot(0, bleed.kind, &bleed, model, DamageContext::NONE);
    iteration.advance_to(100);
    iteration.apply_dot(0, bleed.kind, &bleed, model, DamageContext::NONE);
    iteration.apply_mara_dot(0, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
    let dot = &iteration.common.dots[&(0, DotKind::Ability(bleed.kind))];
    assert_eq!(dot.expires_ms - iteration.common.now_ms, 13_000);
    iteration.trigger_mara_hemotoxin(0, DamageContext::NONE);
    // 100 DPS * 13 seconds * .5, then .25 of that with sqrt(1/2) falloff per secondary.
    assert_eq!(
        iteration.ability_totals("test:hemotoxin-eruption").damage,
        650.0 + 115.0 * 2.0
    );
    assert!(
        !iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::Hemotoxin)))
    );
}

#[test]
fn gushing_blood_replaces_capped_targets_without_carryover_and_consumes_each_poison() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        16,
        [
            ("seethingBleedDamageMultiplier", 1.5),
            ("additionalTargets", 2.0),
        ],
    ));
    profile.talents.push(mara_talent(14, [("procChance", 0.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 4, 194);
    let bleed = iteration
        .ability(DpsAbilityKind::HemorrhagingStrike)
        .unwrap()
        .clone();
    for target in 0..4 {
        iteration.apply_dot(
            target,
            bleed.kind,
            &bleed,
            bleed.dot.unwrap(),
            DamageContext::NONE,
        );
        iteration.apply_mara_dot(target, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
    }
    let untouched = iteration.common.dots[&(3, DotKind::Ability(bleed.kind))].clone();
    iteration.advance_to(100);
    iteration.hero.mara_mut().maiden_of_death_until = 20_000;
    iteration.hero.mara_mut().combo_points = 2;
    iteration.cast(3);
    for target in 0..3 {
        let dot = &iteration.common.dots[&(target, DotKind::Ability(bleed.kind))];
        assert_eq!(
            dot.expires_ms,
            100 + bleed.dot.unwrap().duration_ms
                + 2 * ability_seconds_parameter(
                    &bleed,
                    parameter_key!("bleedDurationPerComboPointSeconds")
                )
        );
        assert_eq!(dot.next_tick_ms, 100 + bleed.dot.unwrap().period_ms);
        assert!(
            !iteration
                .common
                .dots
                .contains_key(&(target, DotKind::Ability(DpsAbilityKind::Hemotoxin)))
        );
    }
    assert_eq!(
        iteration.common.dots[&(3, DotKind::Ability(bleed.kind))].generation,
        untouched.generation
    );
    assert!(
        iteration
            .common
            .dots
            .contains_key(&(3, DotKind::Ability(DpsAbilityKind::Hemotoxin)))
    );
    let eruption_hits = iteration.ability_totals("test:hemotoxin-eruption").hits;
    iteration.process_events_through(2_000);
    assert_eq!(
        iteration.ability_totals("test:hemotoxin-eruption").hits,
        eruption_hits,
        "periodic ticks do not notify either application listener"
    );
}

#[test]
fn gushing_blood_checks_current_seething_target_and_bleed_recaptures_source_stats() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        16,
        [
            ("seethingBleedDamageMultiplier", 2.0),
            ("additionalTargets", 4.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 195);
    let bleed = iteration
        .ability(DpsAbilityKind::HemorrhagingStrike)
        .unwrap()
        .clone();
    let mut model = bleed.dot.unwrap();
    model.power_coefficient = 1.0;
    let mut context = DamageContext::NONE;
    context.source_snapshot = Some(DamageSourceSnapshot {
        hero_damage_scale: 1.0,
        expertise: 4.0,
        primary_stat_multiplier: 3.0,
        critical_chance: 1.0,
    });
    iteration.apply_dot(0, bleed.kind, &bleed, model, context);
    let kind = DotKind::Ability(bleed.kind);
    let dot = iteration.common.dots[&(0, kind)].clone();
    iteration.hero.mara_mut().seething_poison_until[0] = 1_000;
    iteration.execute_dot_tick(kind, &dot, 0, 1.0);
    assert_eq!(
        iteration.ability_totals("test:hemorrhaging-strike").damage,
        200.0
    );
    iteration.common.now_ms = 1_000;
    iteration.execute_dot_tick(kind, &dot, 0, 1.0);
    assert_eq!(
        iteration.ability_totals("test:hemorrhaging-strike").damage,
        300.0
    );
}

#[test]
fn corrosive_spill_overlap_keeps_one_clock_and_ends_with_last_area() {
    let mut profile = mara_profile();
    profile
        .talents
        .push(mara_talent(4, [("procChancePerComboPoint", 1.0)]));
    profile.abilities[16].dot.as_mut().unwrap().duration_ms = 3_000;
    profile.abilities[16].dot.as_mut().unwrap().period_ms = 1_500;
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 196);
    iteration.try_mara_corrosive_spill(0, DamageContext::NONE);
    assert!(iteration.common.dots.is_empty());
    iteration.try_mara_corrosive_spill(6, DamageContext::NONE);
    assert_eq!(
        iteration.ability_totals("test:corrosive-spill").hits,
        2,
        "one initial tick per target, not per point"
    );
    let key = (0, DotKind::Ability(DpsAbilityKind::CorrosiveSpill));
    let generation = iteration.common.dots[&key].generation;
    iteration.advance_to(1_000);
    iteration.try_mara_corrosive_spill(6, DamageContext::NONE);
    assert_eq!(
        iteration.ability_totals("test:corrosive-spill").hits,
        2,
        "overlap is not a second application"
    );
    assert_eq!(iteration.common.dots[&key].generation, generation);
    assert_eq!(iteration.common.dots[&key].next_tick_ms, 1_500);
    assert_eq!(iteration.common.dots[&key].expires_ms, 4_000);
    iteration.process_events_through(3_500);
    assert_eq!(iteration.ability_totals("test:corrosive-spill").hits, 6);
    iteration.process_events_through(4_000);
    iteration.advance_to(4_000);
    assert!(!iteration.common.dots.contains_key(&key));
    iteration.try_mara_corrosive_spill(1, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:corrosive-spill").hits, 8);
}

#[test]
fn corrosive_spill_recaptures_stats_and_creeping_death_without_triggering_hit_multiplier() {
    let mut profile = mara_profile();
    profile.haste = 0.5;
    profile
        .talents
        .push(mara_talent(4, [("procChancePerComboPoint", 1.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 197);
    let mut context = DamageContext::NONE;
    context.multiply_damage(100.0);
    iteration.try_mara_corrosive_spill(1, context);
    let expected = (100.0 * iteration.mara_creeping_death_multiplier()).round();
    assert_eq!(
        iteration.ability_totals("test:corrosive-spill").damage,
        expected
    );
    let key = (0, DotKind::Ability(DpsAbilityKind::CorrosiveSpill));
    let mut dot = iteration.common.dots[&key].clone();
    dot.expertise_snapshot = 100.0;
    dot.primary_stat_multiplier_snapshot = 100.0;
    iteration.execute_dot_tick(key.1, &dot, 0, 1.0);
    assert_eq!(
        iteration.ability_totals("test:corrosive-spill").damage,
        expected * 2.0
    );
}

#[test]
fn bloodrush_changes_bleed_clock_after_hemotoxins_application_sample() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(14, [("procChance", 0.0)]));
    profile
        .talents
        .push(mara_talent(19, [("tickRateMultiplier", 2.0)]));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 198);
    iteration.apply_mara_dot(0, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
    iteration.hero.mara_mut().combo_points = 2;
    iteration.cast(3);
    let bleed = iteration
        .ability(DpsAbilityKind::HemorrhagingStrike)
        .unwrap();
    let dot = &iteration.common.dots[&(0, DotKind::Ability(bleed.kind))];
    let expected = (100.0 * dot.model.power_coefficient).round() * dot.model.duration_ms as f64
        / bleed.dot.unwrap().period_ms as f64
        * 0.5;
    assert_eq!(
        iteration.ability_totals("test:hemotoxin-eruption").damage,
        expected.round()
    );
    assert_eq!(dot.scheduled_period_ms, bleed.dot.unwrap().period_ms / 2);
}

#[test]
fn gushing_requires_maiden_and_bleed_application_does_not_require_health_loss() {
    let mut profile = mara_profile();
    profile.talents.push(mara_talent(
        16,
        [
            ("seethingBleedDamageMultiplier", 1.5),
            ("additionalTargets", 4.0),
        ],
    ));
    profile.abilities[3].power_coefficient = 0.0;
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 199);
    iteration.hero.mara_mut().combo_points = 2;
    iteration.cast(3);
    assert_eq!(
        iteration.ability_totals("test:hemorrhaging-strike").damage,
        0.0
    );
    assert_eq!(iteration.common.dots.len(), 1);
    assert!(
        iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::HemorrhagingStrike)))
    );
}

#[test]
fn widow_applies_one_poison_after_both_strikes_and_replaces_the_old_target_clock() {
    let mut profile = mara_profile();
    profile.abilities[4].gcd_ms = 0;
    profile.abilities[4].first_hit_delay_ms = 190;
    profile.abilities[4].hit_interval_ms = 70;
    profile.abilities[4]
        .mechanic_parameters
        .insert("poisonAdditionalDelaySeconds".into(), 0.2);
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 3, 201);
    it.apply_mara_dot(2, DpsAbilityKind::SeethingPoison, DamageContext::NONE, 1.0);
    it.hero.mara_mut().seething_poison_until[2] = 9_000;
    it.hero.mara_mut().seething_poison_max_until = 9_000;
    it.cast(8);
    it.cast(4);
    it.process_events_through(389);
    assert_eq!(it.ability_totals("test:widows-bite").hits, 2);
    assert_eq!(
        it.hero.mara().combo_points,
        4,
        "stealth does not refill Widow combo points"
    );
    let key = (0, DotKind::Ability(DpsAbilityKind::SeethingPoison));
    assert!(!it.common.dots.contains_key(&key));
    it.process_events_through(390);
    assert!(!it.common.dots.contains_key(&(2, key.1)));
    assert_eq!(it.hero.mara().seething_poison_until[2], 0);
    assert_eq!(it.common.dots[&key].started_ms, 390);
    assert_eq!(it.common.dots[&key].next_tick_ms, 1_390);
    it.process_events_through(800);
    it.apply_mara_stealth_poison(DpsAbilityKind::WidowsBite, DamageContext::NONE);
    assert_eq!(it.common.dots[&key].started_ms, 800);
    assert_eq!(it.common.dots[&key].next_tick_ms, 1_800);
    assert_eq!(it.common.dots[&key].expires_ms, 9_800);
}

#[test]
fn volatile_rolls_one_duration_for_the_batch_and_refresh_rescales_remaining_phase() {
    let profile = mara_profile();
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 3, 202);
    it.apply_mara_stealth_poison(DpsAbilityKind::SkitteringBlades, DamageContext::NONE);
    let key = (0, DotKind::Ability(DpsAbilityKind::VolatilePoison));
    let old = it.common.dots[&key].clone();
    assert!((6_000..=8_000).contains(&old.expires_ms));
    assert_eq!(
        old.model.period_ms,
        (old.model.duration_ms as f64 / 6.0).round() as u64
    );
    for target in 1..3 {
        assert_eq!(it.common.dots[&(target, key.1)].expires_ms, old.expires_ms);
    }
    it.process_events_through(500);
    it.apply_mara_stealth_poison(DpsAbilityKind::SkitteringBlades, DamageContext::NONE);
    let refreshed = &it.common.dots[&key];
    assert!((6_500..=8_500).contains(&refreshed.expires_ms));
    let fraction = (old.next_tick_ms - 500) as f64 / old.scheduled_period_ms as f64;
    assert_eq!(
        refreshed.next_tick_ms,
        500 + (fraction * refreshed.scheduled_period_ms as f64).round() as u64
    );
    assert_eq!(refreshed.started_ms, 0);
}

#[test]
fn volatile_expiry_hits_only_its_owner_once_and_stale_expiry_does_not_erupt() {
    let mut profile = mara_profile();
    profile.abilities[13]
        .mechanic_parameters
        .insert("minimumDurationSeconds".into(), 6.0);
    profile.abilities[13]
        .mechanic_parameters
        .insert("maximumDurationSeconds".into(), 6.0);
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 3, 203);
    it.apply_mara_stealth_poison(DpsAbilityKind::SkitteringBlades, DamageContext::NONE);
    it.process_events_through(500);
    it.apply_mara_stealth_poison(DpsAbilityKind::SkitteringBlades, DamageContext::NONE);
    it.process_events_through(6_200);
    assert_eq!(it.ability_totals("test:volatile-poison-eruption").hits, 0);
    it.process_events_through(6_700);
    assert_eq!(
        it.ability_totals("test:volatile-poison-eruption").hits,
        3,
        "one hit per expired poison, not three AoEs"
    );
    assert_eq!(
        it.ability_totals("test:volatile-poison-eruption").damage,
        300.0
    );
}

#[test]
fn stealth_core_variants_do_not_trigger_basic_blessings_after_stealth_breaks() {
    for index in [0, 9] {
        let mut profile = mara_profile();
        profile.mechanics = vec![ardeos_mechanic(
            "DynamicItemAbilityRank.06",
            [
                ("hitThreshold", 1.0),
                ("criticalStrikeCap", 0.5),
                ("targetCountDamageScalingThreshold", 5.0),
            ],
        )];
        let apl = apl([]);
        let mut it = Iteration::new(&profile, &apl, 3, 204);
        it.cast(8);
        it.cast(index);
        assert!(!it.hero.mara().stealth_active);
        assert_eq!(it.test_proc_count("DynamicItemAbilityRank.06"), 0);
        it.cast(index);
        assert_eq!(it.test_proc_count("DynamicItemAbilityRank.06"), 1);
    }
}

#[test]
fn stealth_combo_rules_distinguish_builders_and_maiden_refills_each() {
    for (index, expected) in [(0, 1), (4, 4), (9, 6)] {
        for maiden in [false, true] {
            let mut profile = mara_profile();
            profile.abilities[4].hit_interval_ms = 70;
            let apl = apl([]);
            let mut it = Iteration::new(&profile, &apl, 1, 205);
            if maiden {
                it.cast(5);
            }
            it.cast(8);
            it.cast(index);
            assert_eq!(
                it.hero.mara().combo_points,
                if maiden { 6 } else { expected }
            );
        }
    }
}

#[test]
fn seething_and_volatile_recapture_stats_and_creeping_death_each_tick() {
    for kind in [
        DpsAbilityKind::SeethingPoison,
        DpsAbilityKind::VolatilePoison,
    ] {
        let profile = mara_profile();
        let apl = apl([]);
        let mut it = Iteration::new(&profile, &apl, 1, 206);
        it.apply_mara_dot(0, kind, DamageContext::NONE, 1.0);
        let dot = it.common.dots[&(0, DotKind::Ability(kind))].clone();
        it.shared.heroism_until = 10_000;
        it.execute_dot_tick(DotKind::Ability(kind), &dot, 0, 1.0);
        let id = &it.ability(kind).unwrap().id;
        let expected = (100.0 * it.mara_creeping_death_multiplier()).round();
        assert_eq!(it.ability_totals(id).damage, expected);
    }
}

#[test]
fn hemotoxin_stack_count_is_consumable_charges_not_a_tick_damage_multiplier() {
    let mut profile = mara_profile();
    profile.critical_strike = 1.0;
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 1, 207);
    it.apply_mara_dot(0, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
    let key = (0, DotKind::Ability(DpsAbilityKind::Hemotoxin));
    let single = it.common.dots[&key].clone();
    it.execute_dot_tick(key.1, &single, 0, 1.0);
    let damage = it.ability_totals("test:hemotoxin").damage;
    for _ in 0..2 {
        it.apply_mara_dot(0, DpsAbilityKind::Hemotoxin, DamageContext::NONE, 1.0);
    }
    let stacked = it.common.dots[&key].clone();
    assert_eq!(stacked.stacks, 3);
    it.execute_dot_tick(key.1, &stacked, 0, 1.0);
    assert_eq!(it.ability_totals("test:hemotoxin").damage, damage * 2.0);
    assert_eq!(
        it.approximate_dot_average_damage(key.1, &stacked, 0),
        100.0,
        "ordinary-hit sampler excludes crit weighting"
    );
}

#[test]
fn timed_mara_dots_execute_terminal_fraction_and_obey_minimum_partial_time() {
    for kind in [
        DpsAbilityKind::HemorrhagingStrike,
        DpsAbilityKind::SeethingPoison,
        DpsAbilityKind::VolatilePoison,
        DpsAbilityKind::Hemotoxin,
    ] {
        for (duration, expected) in [(1_050, 100.0), (1_500, 150.0), (2_000, 200.0)] {
            let profile = mara_profile();
            let apl = apl([]);
            let mut it = Iteration::new(&profile, &apl, 1, 208);
            let ability = it.ability(kind).unwrap().clone();
            let mut model = ability.dot.unwrap();
            model.duration_ms = duration;
            model.period_ms = 1_000;
            it.apply_dot(0, kind, &ability, model, DamageContext::NONE);
            it.process_events_through(duration);
            assert_eq!(
                it.ability_totals(&ability.id).damage,
                expected,
                "{kind:?} {duration}"
            );
            assert!(!it.common.dots.contains_key(&(0, DotKind::Ability(kind))));
        }
    }
}

#[test]
fn current_poisons_neither_double_spenders_nor_get_consumed_by_queens_fang() {
    let profile = mara_profile();
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 1, 209);
    for kind in [
        DpsAbilityKind::SeethingPoison,
        DpsAbilityKind::VolatilePoison,
        DpsAbilityKind::Hemotoxin,
    ] {
        it.apply_mara_dot(0, kind, DamageContext::NONE, 1.0);
    }
    it.hero.mara_mut().combo_points = 6;
    it.cast(7);
    assert_eq!(it.ability_totals("test:queens-fang").damage, 220.0);
    for kind in [
        DpsAbilityKind::SeethingPoison,
        DpsAbilityKind::VolatilePoison,
        DpsAbilityKind::Hemotoxin,
    ] {
        assert!(it.common.dots.contains_key(&(0, DotKind::Ability(kind))));
    }
}

#[test]
fn stealth_core_critical_bonus_survives_the_opening_hits_stealth_removal() {
    for index in [0, 9] {
        let mut profile = mara_profile();
        profile.mechanics = vec![ardeos_mechanic(
            "DynamicItemAbilityRank.05",
            [("criticalStrikeBonus", 1.0)],
        )];
        let apl = apl([]);
        let mut it = Iteration::new(&profile, &apl, 3, 210);
        it.cast(8);
        it.cast(index);
        let id = &profile.abilities[index].id;
        let expected = if index == 0 { 3 } else { 1 };
        assert_eq!(it.ability_totals(id).crits, expected);
        it.cast(index);
        assert_eq!(
            it.ability_totals(id).crits,
            expected,
            "ordinary Basic variant does not receive Core CritChance"
        );
    }
}

#[test]
fn terminal_mara_tick_uses_rescaled_timer_progress() {
    let mut profile = mara_profile();
    profile.abilities[13].dot.as_mut().unwrap().duration_ms = 600;
    let apl = apl([]);
    let mut it = Iteration::new(&profile, &apl, 1, 211);
    it.apply_mara_dot(0, DpsAbilityKind::VolatilePoison, DamageContext::NONE, 1.0);
    let key = (0, DotKind::Ability(DpsAbilityKind::VolatilePoison));
    // At 500 ms a 1-second timer becomes a half-second timer with half
    // its interval left. Expiry at 600 has 70% of the new interval elapsed.
    let dot = it.common.dots.get_mut(&key).unwrap();
    dot.model.period_ms = 500;
    dot.scheduled_period_ms = 500;
    dot.next_tick_ms = 750;
    it.process_events_through(600);
    assert_eq!(it.ability_totals("test:volatile-poison").damage, 70.0);
}

#[test]
fn default_mara_spends_actual_spirit_cost_without_overwriting_matriarch() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/apl-builds/mara/character.json")).unwrap();
    let mut build: crate::preparation::CharacterBuild =
        serde_json::from_value(document["build"].clone()).unwrap();
    build
        .positions
        .iter_mut()
        .find(|p| p.position_id == "weapon")
        .unwrap()
        .item = None;
    let (profile, _) = crate::preparation::prepare_character(&build, 1).unwrap();
    let apl = crate::parse_apl(&shipped_apl_source("mara")).unwrap();
    for cost in [85.0, 100.0] {
        let mut profile = profile.clone();
        profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == DpsAbilityKind::MatriarchMacabre)
            .unwrap()
            .spirit_cost = cost;
        let mut it = Iteration::new(&profile, &apl, 1, 211);
        it.apply_mara_dot(0, DpsAbilityKind::SeethingPoison, DamageContext::NONE, 1.0);
        let matriarch = it.common.abilities_by_kind[&DpsAbilityKind::MatriarchMacabre];
        it.shared.spirit = cost - 0.01;
        assert_ne!(it.choose_action(), Some(matriarch));
        it.shared.spirit = cost;
        assert!(cost < profile.max_spirit);
        assert_eq!(it.choose_action(), Some(matriarch));
        it.hero.mara_mut().matriarch_macabre_until = 1_000;
        assert_ne!(it.choose_action(), Some(matriarch));
        it.common.now_ms = 1_000;
        assert_eq!(it.choose_action(), Some(matriarch));
    }
}

#[test]
fn default_mara_consumes_active_malevolence_and_falls_back_after_expiry() {
    let profile = mara_profile();
    let apl = crate::parse_apl(&shipped_apl_source("mara")).unwrap();
    for targets in [1, 3, 5] {
        let mut it = Iteration::new(&profile, &apl, targets, 212);
        it.shared.spirit = 0.0;
        it.hero.mara_mut().seething_poison_max_until = 30_000;
        it.hero.mara_mut().combo_points = if targets == 1 { 5 } else { 4 };
        for kind in [
            DpsAbilityKind::HemorrhagingStrike,
            DpsAbilityKind::MaidenOfDeath,
            DpsAbilityKind::FinalStratagem,
            DpsAbilityKind::BroodingShadows,
        ] {
            it.common.cooldowns.insert(
                kind,
                CooldownState {
                    remaining_ms: 30_000.0,
                    used_charges: 1,
                },
            );
        }
        let queen = it.common.abilities_by_kind[&DpsAbilityKind::QueensFang];
        let arachnid = it.common.abilities_by_kind[&DpsAbilityKind::ArachnidAssault];
        let (usual, empowered) = if targets == 1 {
            (queen, arachnid)
        } else {
            (arachnid, queen)
        };
        assert_eq!(it.choose_action(), Some(usual));
        if targets == 1 {
            it.hero.mara_mut().malevolence_arachnid_until = 1_000;
            it.hero.mara_mut().malevolence_arachnid_stacks = 1;
        } else {
            it.hero.mara_mut().malevolence_queen_until = 1_000;
            it.hero.mara_mut().malevolence_queen_stacks = 1;
        }
        assert_eq!(it.choose_action(), Some(empowered));
        it.common.now_ms = 1_000;
        assert_eq!(it.choose_action(), Some(usual));
    }
}
