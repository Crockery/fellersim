use super::*;

const KINDLING: &str = "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc";
const EMERALD: &str = "ItemTrait.ID.GemTargetedSpikeProc";
const COUNTER: &str = "ItemTrait.ID.CritsToIncreasedCritRating";

fn kindling() -> DynamicMechanicInstance {
    ardeos_mechanic(
        KINDLING,
        [
            ("powerCoefficientPerTick", 0.69),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("durationSeconds", 9.0),
            ("periodSeconds", 1.5),
            ("healingPowerCoefficientPerTick", 0.69),
            ("healingDurationSeconds", 9.0),
            ("healingPeriodSeconds", 1.5),
        ],
    )
}

#[test]
fn kindling_starts_inactive_without_carrying_precombat_healing() {
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]),
        mara_profile(),
        gunde_profile(),
    ] {
        profile.mechanics = vec![kindling()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 83);
        iteration.process_events_through(9_000);
        assert!(iteration.shared.kindling.iter().all(Option::is_none));
        assert!(
            iteration
                .shared
                .kindling_healing
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(iteration.test_proc_count(KINDLING), 0);
        assert_eq!(iteration.common.result.damage, 0.0);
    }
}

#[test]
fn emerald_refreshes_first_strike_after_its_hit_on_an_already_touched_target() {
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]),
        mara_profile(),
        gunde_profile(),
    ] {
        profile.mechanics = vec![
            ardeos_mechanic(
                "gem-emerald-600",
                [("expertise", 0.15), ("durationSeconds", 15.0)],
            ),
            ardeos_mechanic(EMERALD, [("procChance", 1.0), ("powerCoefficient", 8.0)]),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 71);
        iteration.trigger_first_damage_expertise(0);
        iteration.common.now_ms = 16_000;
        let base = iteration.effective_expertise();
        let source = iteration.test_damage_source("ordinary-hit");
        iteration.trigger_dynamic_on_damage(None, source, 10.0, false, 0, DamageContext::NONE);
        assert_eq!(
            iteration.test_dynamic_buff("gem-emerald-600").until_ms,
            31_000
        );
        assert_eq!(iteration.test_dynamic_buff("gem-emerald-600").stacks, 1);
        assert!((iteration.effective_expertise() - base - 0.15).abs() < 1e-12);
        assert_eq!(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemTargetedSpikeProc")
                .hits,
            1
        );
        // Its own effect is excluded, so the nested hit does not recurse.
        assert_eq!(iteration.test_proc_count(EMERALD), 1);
    }
}

#[test]
fn kindling_healing_ticks_feed_critical_dps_but_not_damage_or_diamond_recursion() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.critical_strike = 1.0;
    profile.mechanics = vec![
        kindling(),
        ardeos_mechanic(
            COUNTER,
            [
                ("requiredCriticalStrikes", 100.0),
                ("criticalRating", 20.0),
                ("durationSeconds", 12.0),
            ],
        ),
        ardeos_mechanic(
            "ItemTrait.ID.GemSingleTargetProcOnDamageHeal",
            [("procsPerMinute", 60_000.0)],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 72);
    // A Trait-sourced instant heal admits Kindling but is excluded by Diamond.
    let source = iteration.profile.mechanics[2].damage_source;
    let spirit = iteration.shared.spirit;
    iteration.apply_positive_healing(source, false);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
    iteration.process_events_through(9_000);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 6);
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
    assert_eq!(iteration.common.result.damage, 0.0);
    assert_eq!(iteration.shared.spirit, spirit);
    assert!(
        !iteration
            .shared
            .proc_per_minute_states
            .contains_key("RandomStream.Traits.GemSingleTargetProcOnDamageHeal_Proc.Heal")
    );
    iteration.process_events_through(12_000);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 6);
}

#[test]
fn kindling_healing_refresh_replaces_critical_snapshot_without_resetting_phase() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.critical_strike = 0.0;
    profile.mechanics = vec![
        kindling(),
        ardeos_mechanic(
            COUNTER,
            [
                ("requiredCriticalStrikes", 100.0),
                ("criticalRating", 20.0),
                ("durationSeconds", 12.0),
            ],
        ),
        ardeos_mechanic(
            "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff",
            [("criticalStrikeBonus", 1.0), ("durationSeconds", 15.0)],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 73);
    let source = iteration.test_damage_source("instant-heal");
    iteration.apply_positive_healing(source, false);
    iteration.common.now_ms = 500;
    let buff = Arc::clone(&iteration.profile.mechanics[2]);
    iteration.activate_dynamic_buff(&buff, 15_000, 1, 0.0);
    iteration.apply_positive_healing(source, false);
    let state = iteration.shared.kindling_healing[0].unwrap();
    assert_eq!(state.until_ms, 9_500);
    assert_eq!(state.next_tick_ms, 1_500);
    assert_eq!(state.critical_chance_snapshot, 1.0);
    iteration.process_events_through(1_499);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
    iteration.process_events_through(1_500);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 1);
}

#[test]
fn kindling_haste_changes_preserve_progress_for_damage_and_healing() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![
        kindling(),
        ardeos_mechanic(
            "setb-proc-hdt",
            [("hasteBonus", 1.0), ("durationSeconds", 1.0)],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 74);
    let mechanic = Arc::clone(&iteration.profile.mechanics[0]);
    iteration.apply_kindling(&mechanic, 0);
    let source = iteration.test_damage_source("instant-heal");
    iteration.apply_positive_healing(source, false);
    iteration.process_events_through(750);
    let haste = Arc::clone(&iteration.profile.mechanics[1]);
    iteration.activate_dynamic_buff(&haste, 1_000, 1, 0.0);
    iteration.process_events_through(1_124);
    assert!(!iteration.has_ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc"));
    iteration.process_events_through(1_125);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc")
            .hits,
        1
    );
    iteration.process_events_through(1_999);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc")
            .hits,
        1
    );
    assert_eq!(
        iteration.shared.kindling_healing[0].unwrap().next_tick_ms,
        2_000
    );
    iteration.process_events_through(2_000);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc")
            .hits,
        2
    );
}

#[test]
fn kindling_tick_keeps_a_nested_emerald_application_refresh() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![
        kindling(),
        ardeos_mechanic(EMERALD, [("procChance", 1.0), ("powerCoefficient", 1.0)]),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 75);
    let mechanic = Arc::clone(&iteration.profile.mechanics[0]);
    iteration.apply_kindling(&mechanic, 0);
    iteration.process_events_through(1_500);
    let state = iteration.test_kindling(KINDLING, 0);
    assert_eq!(state.until_ms, 10_500);
    assert_eq!(state.next_tick_ms, 3_000);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.ExtraDotHotOnEffectApplicationProc")
            .hits,
        1
    );
}

#[test]
fn periodic_heal_events_do_not_count_as_new_kindling_applications() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 76);
    let source = iteration.test_damage_source("periodic-heal");
    for _ in 0..5 {
        iteration.emit_positive_healing(source, false);
    }
    assert!(iteration.shared.kindling_healing[0].is_none());
    assert!(
        !iteration
            .shared
            .proc_per_minute_states
            .contains_key("RandomStream.Traits.ExtraDotHotOnEffectApplicationProc.Hot")
    );
    iteration.apply_positive_healing(source, false);
    assert!(iteration.shared.kindling_healing[0].is_some());
    assert!(iteration.shared.dynamic_proc_per_minute_states[0].is_none());
}

#[test]
fn wolf_commits_apply_kindling_healing_for_each_dps_hero() {
    for (mut profile, trigger) in [
        (
            profile(vec![ability(DpsAbilityKind::Wildfire, 0.0)]),
            DpsAbilityKind::Wildfire,
        ),
        (rime_profile(), DpsAbilityKind::FlightOfTheNavir),
        (tariq_profile(), DpsAbilityKind::ThunderCall),
        (
            elarion_profile([elarion_ability(DpsAbilityKind::LunarlightMark)]),
            DpsAbilityKind::LunarlightMark,
        ),
        (mara_profile(), DpsAbilityKind::MaidenOfDeath),
        (gunde_profile(), DpsAbilityKind::Rupture),
    ] {
        profile.mechanics = vec![
            kindling(),
            ardeos_mechanic(
                "ItemTrait.ID.Wolf",
                [
                    ("smallHealingHealthFraction", 0.2),
                    ("mediumHealingHealthFraction", 0.25),
                    ("largeHealingHealthFraction", 0.3),
                ],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 77);
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(trigger, 0.0)),
            DamageContext::NONE,
        );
        assert!(iteration.shared.kindling_healing[0].is_some());
        assert_eq!(iteration.test_proc_count(KINDLING), 1);
        assert_eq!(iteration.common.result.damage, 0.0);
    }
}

#[test]
fn ruby_storm_uses_cooked_commit_tags_and_only_ranged_scenarios_are_out_of_reach() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    let ruby = ardeos_mechanic(
        "ItemTrait.ID.GemWhirlwindProc",
        [
            ("procsPerMinute", 60_000.0),
            ("lifetimeSeconds", 6.0),
            ("movementSpeed", 350.0),
            ("collisionRadius", 250.0),
        ],
    );
    let reach = ruby_storm_maximum_forward_overlap_distance(&ruby).unwrap();
    for distance in [
        ARDEOS_MAX_COMBAT_RANGE_UNITS,
        RIME_MAX_COMBAT_RANGE_UNITS,
        ELARION_MAX_COMBAT_RANGE_UNITS,
    ] {
        assert!(reach < distance);
    }
    for distance in [
        TARIQ_MAX_COMBAT_RANGE_UNITS,
        MARA_MAX_COMBAT_RANGE_UNITS,
        GUNDE_MAX_COMBAT_RANGE_UNITS,
    ] {
        assert!(reach >= distance);
    }
    profile.mechanics = vec![ruby];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 78);
    for kind in [
        DpsAbilityKind::MaraAttack,
        DpsAbilityKind::ElarionShoot,
        DpsAbilityKind::Detonate,
        DpsAbilityKind::WintersBlessing,
    ] {
        iteration
            .trigger_dynamic_on_cast(&compiled_ability(&ability(kind, 1.0)), DamageContext::NONE);
    }
    assert_eq!(
        iteration.test_proc_count("ItemTrait.ID.GemWhirlwindProc"),
        0
    );
    for kind in [
        DpsAbilityKind::Multishot,
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::StarfallVolley,
    ] {
        iteration.common.now_ms += 1_000;
        iteration
            .trigger_dynamic_on_cast(&compiled_ability(&ability(kind, 1.0)), DamageContext::NONE);
    }
    assert_eq!(
        iteration.test_proc_count("ItemTrait.ID.GemWhirlwindProc"),
        3
    );
    assert_eq!(iteration.common.result.damage, 0.0);
}

#[test]
fn lunarlight_mark_application_can_trigger_kindling_without_damage() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::LunarlightMark)]);
    profile.mechanics = vec![kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 79);
    iteration.apply_elarion_mark(0, 1);
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
    assert_eq!(iteration.common.result.damage, 0.0);
    assert!(iteration.shared.kindling_healing[0].is_none());
    iteration.common.now_ms = 1_000;
    let mut expected_rng = iteration.common.rng;
    expected_rng.next();
    expected_rng.next();
    iteration.apply_elarion_mark(0, 1);
    // Both delegates roll, but after a success a second attempt at the same
    // timestamp has zero RPPM probability. It still consumes a random draw.
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    assert_eq!(iteration.common.rng.state, expected_rng.state);
    assert_eq!(iteration.test_kindling(KINDLING, 0).next_tick_ms, 1_500);
}

fn certain_kindling() -> DynamicMechanicInstance {
    let mut mechanic = kindling();
    mechanic.parameters.remove("procsPerMinute");
    mechanic.parameters.insert("procChance".into(), 1.0);
    mechanic
}

#[test]
fn kindling_counts_gunde_damage_and_separate_target_effect_even_for_zero_damage() {
    for kind in [DpsAbilityKind::Rupture, DpsAbilityKind::ButchersHook] {
        let mut profile = gunde_profile();
        let ability = profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == kind)
            .unwrap();
        ability.power_coefficient = 0.0;
        profile.mechanics = vec![certain_kindling()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 80);
        let index = iteration
            .profile
            .abilities
            .iter()
            .position(|a| a.kind == kind)
            .unwrap();
        iteration.impact(index, true, 1.0, 0, DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(KINDLING), 2, "{kind:?}");
        assert_eq!(iteration.common.result.damage, 0.0);
        iteration.common.now_ms = 500;
        iteration.impact(index, true, 1.0, 0, DamageContext::NONE);
        assert_eq!(
            iteration.test_proc_count(KINDLING),
            if kind == DpsAbilityKind::Rupture {
                5
            } else {
                4
            }
        );
        assert_eq!(iteration.test_kindling(KINDLING, 0).next_tick_ms, 1_500);
    }
}

#[test]
fn kindling_counts_shimmer_and_slayers_mosh_only_when_equipped() {
    for equipped in [false, true] {
        for (mut profile, kind, legendary) in [
            (
                elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]),
                DpsAbilityKind::HighwindArrow,
                ardeos_mechanic(
                    "legendary-shimmer",
                    [
                        ("shimmerDamageMultiplier", 1.08),
                        ("shimmerDurationSeconds", 9.0),
                        ("shimmerMaximumStacks", 3.0),
                    ],
                ),
            ),
            (
                tariq_profile(),
                DpsAbilityKind::LeapSmash,
                ardeos_mechanic(
                    "legendary-slayers-mosh",
                    [
                        ("leapTargetDamageMultiplier", 1.1),
                        ("leapTargetDamageDurationSeconds", 6.0),
                    ],
                ),
            ),
        ] {
            profile
                .abilities
                .iter_mut()
                .find(|a| a.kind == kind)
                .unwrap()
                .power_coefficient = 0.0;
            profile.mechanics = vec![certain_kindling()];
            if equipped {
                profile.mechanics.push(legendary);
            }
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 3, 81);
            let index = iteration
                .profile
                .abilities
                .iter()
                .position(|a| a.kind == kind)
                .unwrap();
            for target in 0..3 {
                iteration.impact(index, true, 1.0, target, DamageContext::NONE);
                assert_eq!(iteration.test_kindling(KINDLING, target).until_ms, 9_000);
            }
            assert_eq!(
                iteration.test_proc_count(KINDLING),
                if equipped { 6 } else { 3 },
                "{kind:?}"
            );
            iteration.common.now_ms = 500;
            for target in 0..3 {
                iteration.impact(index, true, 1.0, target, DamageContext::NONE);
            }
            assert_eq!(
                iteration.test_proc_count(KINDLING),
                if equipped { 15 } else { 6 }
            );
            iteration.common.now_ms = 20_000;
            iteration.impact(index, true, 1.0, 0, DamageContext::NONE);
            assert_eq!(
                iteration.test_proc_count(KINDLING),
                if equipped { 17 } else { 7 }
            );
            if equipped && kind == DpsAbilityKind::HighwindArrow {
                assert_eq!(iteration.hero.elarion().shimmer_stacks[0], 1);
            }
        }
    }
}

#[test]
fn coalescing_frost_applies_one_kindling_trigger_even_for_two_stacks() {
    for critical in [false, true] {
        for selected in [false, true] {
            let mut profile = rime_profile();
            profile.mechanics = vec![certain_kindling()];
            if selected {
                profile.talents.push(rime_talent(
                    3,
                    [
                        ("durationSeconds", 3.0),
                        ("powerCoefficientPerStack", 0.56),
                        ("criticalExtraStackChance", 1.0),
                        ("criticalExtraStacks", 1.0),
                        ("maximumStacks", 99.0),
                        ("targetCountDamageScalingThreshold", 12.0),
                    ],
                ));
            }
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 1, 82);
            iteration.on_rime_torrent_tick(critical, 0, DamageContext::NONE);
            assert_eq!(iteration.test_proc_count(KINDLING), u64::from(selected));
            if selected {
                assert_eq!(
                    iteration.hero.rime().coalescing_stacks[0],
                    if critical { 2 } else { 1 }
                );
                assert!(iteration.shared.controlled_random_states.contains_key(
                    "RandomStream.Rime.Talent.ChanneledBeamSingleDamage.ApplyDoubleStacks"
                ));
            }
            assert_eq!(iteration.common.result.damage, 0.0);
            iteration.common.now_ms = 500;
            iteration.on_rime_torrent_tick(critical, 0, DamageContext::NONE);
            assert_eq!(iteration.test_proc_count(KINDLING), 3 * u64::from(selected));
        }
    }
}

fn certain_ruby_storm() -> DynamicMechanicInstance {
    ardeos_mechanic(
        "ItemTrait.ID.GemWhirlwindProc",
        [
            ("procChance", 1.0),
            ("flatDamage", 1_000.0),
            ("gemPowerDamageIncreasePerPoint", 0.0),
            ("lifetimeSeconds", 6.0),
            ("movementSpeed", 350.0),
            ("collisionRadius", 250.0),
        ],
    )
}

#[test]
fn ruby_storm_hits_stacked_melee_targets_once_after_travel_and_feeds_kindling() {
    for mut profile in [tariq_profile(), mara_profile(), gunde_profile()] {
        profile.mechanics = vec![certain_ruby_storm(), certain_kindling()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 84);
        let delay = if profile.hero_id == TARIQ_HERO_ID {
            858
        } else {
            715
        };
        let committed = compiled_ability(&ability(DpsAbilityKind::Multishot, 1.0));
        iteration.trigger_dynamic_on_cast(&committed, DamageContext::NONE);
        iteration.process_events_through(delay - 1);
        assert_eq!(iteration.common.result.damage, 0.0);
        iteration.process_events_through(delay);
        let totals = iteration.ability_totals("gear:ItemTrait.ID.GemWhirlwindProc");
        assert_eq!(totals.hits, 3);
        assert_eq!(totals.crits, 0);
        assert!((2_700.0..=3_300.0).contains(&totals.damage));
        assert_eq!(iteration.test_proc_count(KINDLING), 3);
        iteration.process_events_through(10_000);
        assert_eq!(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemWhirlwindProc")
                .hits,
            3
        );
    }
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
        rime_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]),
    ] {
        profile.mechanics = vec![certain_ruby_storm()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 84);
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(DpsAbilityKind::Multishot, 1.0)),
            DamageContext::NONE,
        );
        iteration.process_events_through(10_000);
        assert_eq!(iteration.common.result.damage, 0.0);
        assert_eq!(
            iteration.test_proc_count("ItemTrait.ID.GemWhirlwindProc"),
            1
        );
    }
}

#[test]
fn ruby_storm_snapshots_patient_soul_at_spawn_without_primary_stat_scaling() {
    let mut damages = Vec::new();
    for spawn_at in [2_999, 3_000] {
        let mut profile = mara_profile();
        profile.mechanics = vec![
            certain_ruby_storm(),
            ardeos_mechanic(
                "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease",
                [
                    ("expertiseRating", 0.0),
                    ("applicationDelaySeconds", 3.0),
                    ("maxHealthMultiplier", 1.05),
                ],
            ),
            ardeos_mechanic(
                "ItemTrait.ID.IncreasedMainStatAndSpiritRating",
                [("powerMultiplier", 2.0), ("durationSeconds", 10.0)],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 85);
        iteration.common.now_ms = spawn_at;
        let buff = Arc::clone(&iteration.profile.mechanics[2]);
        iteration.activate_dynamic_buff(&buff, 10_000, 1, 0.0);
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(DpsAbilityKind::Multishot, 1.0)),
            DamageContext::NONE,
        );
        iteration.process_events_through(spawn_at + 715);
        damages.push(
            iteration
                .ability_totals("gear:ItemTrait.ID.GemWhirlwindProc")
                .damage,
        );
    }
    assert!((900.0..=1_100.0).contains(&damages[0]));
    assert!((damages[1] - damages[0] * 1.05).abs() <= 1.0);
}

#[test]
fn concurrent_ruby_storms_keep_independent_snapshots_and_do_not_repeat() {
    let mut profile = gunde_profile();
    profile.mechanics = vec![certain_ruby_storm()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 86);
    let committed = compiled_ability(&ability(DpsAbilityKind::Multishot, 1.0));
    iteration.trigger_dynamic_on_cast(&committed, DamageContext::NONE);
    iteration.common.now_ms = 100;
    iteration.trigger_dynamic_on_cast(&committed, DamageContext::NONE);
    iteration.process_events_through(715);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemWhirlwindProc")
            .hits,
        1
    );
    iteration.process_events_through(815);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemWhirlwindProc")
            .hits,
        2
    );
    iteration.process_events_through(20_000);
    assert_eq!(
        iteration
            .ability_totals("gear:ItemTrait.ID.GemWhirlwindProc")
            .hits,
        2
    );
    assert_eq!(
        iteration.test_proc_count("ItemTrait.ID.GemWhirlwindProc"),
        2
    );
}

#[test]
fn cone_weapon_applies_kindling_stun_triggers_only_on_first_and_final_hits() {
    let mut cone = ability(DpsAbilityKind::WeaponFrontalCone, 1.0);
    cone.off_gcd = true;
    cone.gcd_ms = 0;
    cone.first_hit_delay_ms = 400;
    cone.max_targets = 3;
    cone.mechanic_parameters = BTreeMap::from([
        ("initialPowerCoefficient".into(), 1.0),
        ("repeatingPowerCoefficient".into(), 1.0),
        ("finalPowerCoefficient".into(), 1.0),
        ("coneDurationSeconds".into(), 3.0),
        ("coneStunDurationSeconds".into(), 4.0),
        ("coneTickIntervalSeconds".into(), 1.0),
    ]);
    let mut profile = profile(vec![cone]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 87);
    iteration.cast(0);
    for (time, applications) in [(399, 0), (400, 6), (1_400, 9), (2_400, 12), (3_400, 21)] {
        iteration.process_events_through(time);
        assert_eq!(
            iteration.test_proc_count(KINDLING),
            applications,
            "at {time}"
        );
    }
}

#[test]
fn frost_volley_reapplication_triggers_kindling_without_resetting_periodic_state() {
    let weapon = dot_ability(DpsAbilityKind::WeaponFrostVolley, 1.0);
    let mut profile = profile(vec![weapon.clone()]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 88);
    let compiled = Arc::clone(&iteration.profile.abilities[0]);
    iteration.apply_dot(
        0,
        compiled.kind,
        &compiled,
        weapon.dot.unwrap(),
        DamageContext::NONE,
    );
    let key = (0, DotKind::Ability(DpsAbilityKind::WeaponFrostVolley));
    let before = iteration.common.dots[&key].clone();
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    iteration.common.now_ms = 500;
    iteration.apply_dot(
        0,
        compiled.kind,
        &compiled,
        weapon.dot.unwrap(),
        DamageContext::NONE.with_snapshot(DamageSourceSnapshot {
            hero_damage_scale: 1.0,
            expertise: 0.3,
            primary_stat_multiplier: 2.0,
            critical_chance: 1.0,
        }),
    );
    let after = &iteration.common.dots[&key];
    assert_eq!(after.next_tick_ms, before.next_tick_ms);
    assert_eq!(after.expires_ms, before.expires_ms);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.expertise_snapshot, 0.3);
    assert_eq!(after.primary_stat_multiplier_snapshot, 2.0);
    assert_eq!(after.critical_chance_override, Some(1.0));
    assert_eq!(iteration.test_proc_count(KINDLING), 3);
    assert_eq!(iteration.shared.kindling[0].unwrap().until_ms, 9_500);
    iteration.process_events_through(before.next_tick_ms);
    assert_eq!(
        iteration.test_proc_count(KINDLING),
        3,
        "periodic execution is not a new application"
    );
}

#[test]
fn timed_stacking_dots_notify_kindling_for_refresh_and_application() {
    for kind in [
        DpsAbilityKind::SearingBlaze,
        DpsAbilityKind::Incinerate,
        DpsAbilityKind::HemorrhagingStrike,
        DpsAbilityKind::SeethingPoison,
        DpsAbilityKind::VolatilePoison,
        DpsAbilityKind::Hemotoxin,
    ] {
        let source = dot_ability(kind, 1.0);
        let mut profile = if matches!(
            kind,
            DpsAbilityKind::SearingBlaze | DpsAbilityKind::Incinerate
        ) {
            profile(vec![source.clone()])
        } else {
            let mut profile = mara_profile();
            profile.abilities = vec![source.clone()];
            profile
        };
        profile.mechanics = vec![certain_kindling()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 2, 89);
        let compiled = Arc::clone(&iteration.profile.abilities[0]);
        iteration.apply_dot(0, kind, &compiled, source.dot.unwrap(), DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(KINDLING), 1, "{kind:?}");
        let phase = iteration.common.dots[&(0, DotKind::Ability(kind))].next_tick_ms;
        iteration.common.now_ms = 500;
        iteration.apply_dot(0, kind, &compiled, source.dot.unwrap(), DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(KINDLING), 3, "{kind:?}");
        assert_eq!(
            iteration.common.dots[&(0, DotKind::Ability(kind))].next_tick_ms,
            phase
        );
        iteration.apply_dot(1, kind, &compiled, source.dot.unwrap(), DamageContext::NONE);
        assert_eq!(
            iteration.test_proc_count(KINDLING),
            4,
            "a new target has no refresh"
        );
    }
}

#[test]
fn manual_dot_refresh_notifies_kindling_once_and_duration_extension_does_not() {
    let source = dot_ability(DpsAbilityKind::SearingBlaze, 1.0);
    let mut profile = profile(vec![source.clone()]);
    profile.mechanics = vec![certain_kindling()];
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
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 90);
    let compiled = Arc::clone(&iteration.profile.abilities[0]);
    iteration.apply_dot(
        0,
        source.kind,
        &compiled,
        source.dot.unwrap(),
        DamageContext::NONE,
    );
    iteration.common.now_ms = 500;
    iteration.apply_dot(
        0,
        source.kind,
        &compiled,
        source.dot.unwrap(),
        DamageContext::NONE,
    );
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    iteration.increase_dot_duration(0, DotKind::Ability(source.kind), 1_000);
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
}

#[test]
fn duration_extensions_notify_kindling_only_when_the_timer_returns_to_full() {
    for capped in [false, true] {
        for (elapsed, extension, expected_refresh) in [
            (0, 0, true),
            (500, 0, false),
            (500, 499, false),
            (500, 500, true),
            (500, 501, capped),
        ] {
            let source = dot_ability(DpsAbilityKind::SearingBlaze, 1.0);
            let mut profile = profile(vec![source.clone()]);
            profile.mechanics = vec![certain_kindling()];
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 2, 92);
            let compiled = Arc::clone(&iteration.profile.abilities[0]);
            let kind = DotKind::Ability(source.kind);
            iteration.apply_dot(
                0,
                source.kind,
                &compiled,
                source.dot.unwrap(),
                DamageContext::NONE,
            );
            // A later application resets the duration but retains uptime's
            // original start and the periodic clock.
            iteration.common.now_ms = 2_000;
            iteration.apply_dot(
                0,
                source.kind,
                &compiled,
                source.dot.unwrap(),
                DamageContext::NONE,
            );
            let before = iteration.common.dots[&(0, kind)].clone();
            let procs = iteration.test_proc_count(KINDLING);
            iteration.common.now_ms += elapsed;
            if capped {
                iteration.increase_dot_duration_up_to_total(0, kind, extension);
            } else {
                iteration.increase_dot_duration(0, kind, extension);
            }
            let after = &iteration.common.dots[&(0, kind)];
            assert_eq!(after.started_ms, 0);
            assert_eq!(after.next_tick_ms, before.next_tick_ms);
            assert_eq!(after.generation, before.generation);
            assert_eq!(
                iteration.test_proc_count(KINDLING),
                procs + u64::from(expected_refresh),
                "capped={capped}, elapsed={elapsed}, extension={extension}"
            );
            let procs = iteration.test_proc_count(KINDLING);
            iteration.increase_dot_duration(1, kind, extension);
            iteration.increase_dot_duration_up_to_total(1, kind, extension);
            iteration.common.now_ms = 100_000;
            iteration.increase_dot_duration(0, kind, extension);
            iteration.increase_dot_duration_up_to_total(0, kind, extension);
            assert_eq!(
                iteration.test_proc_count(KINDLING),
                procs,
                "missing/expired effects"
            );
        }
    }
}

#[test]
fn incinerate_notifies_kindling_when_extending_its_fresh_dot_and_an_older_dot_to_full() {
    let searing = dot_ability(DpsAbilityKind::SearingBlaze, 1.0);
    let mut incinerate = dot_ability(DpsAbilityKind::Incinerate, 1.0);
    incinerate.dot_extension_ms = 1_000;
    let mut profile = profile(vec![searing.clone(), incinerate]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 95);
    let compiled = Arc::clone(&iteration.profile.abilities[0]);
    iteration.apply_dot(
        0,
        searing.kind,
        &compiled,
        searing.dot.unwrap(),
        DamageContext::NONE,
    );
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
    iteration.common.now_ms = 500;
    iteration.impact(1, true, 1.0, 0, DamageContext::NONE);
    // Direct hit, new DoT, Searing Blaze's reset and Incinerate's zero extension.
    assert_eq!(iteration.test_proc_count(KINDLING), 5);
    iteration.common.now_ms = 1_000;
    iteration.impact(1, true, 1.0, 0, DamageContext::NONE);
    // Reapplying Incinerate's existing DoT also emits its stacking refresh.
    assert_eq!(iteration.test_proc_count(KINDLING), 10);
}

#[test]
fn area_pulses_notify_kindling_per_application_without_a_spurious_timer_application() {
    for kind in [
        DpsAbilityKind::StarfallVolley,
        DpsAbilityKind::BloodboundSpirit,
    ] {
        for coefficient in [0.0, 1.0] {
            let mut source = dot_ability(kind, coefficient);
            source.power_coefficient = coefficient;
            source.direct_hits = u32::from(kind == DpsAbilityKind::BloodboundSpirit);
            let mut profile = if kind == DpsAbilityKind::StarfallVolley {
                elarion_profile([source.clone()])
            } else {
                let mut profile = gunde_profile();
                profile.abilities = vec![source.clone()];
                profile
            };
            profile.mechanics = vec![certain_kindling()];
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 2, 93);
            let initial = u64::from(source.direct_hits);
            for target in 0..2 {
                iteration.impact(0, false, 1.0, target, DamageContext::NONE);
            }
            assert_eq!(iteration.test_proc_count(KINDLING), initial * 2, "{kind:?}");
            for tick in 1..=2 {
                iteration.common.now_ms = tick * 1_000;
                for target in 0..2 {
                    let dot = iteration.common.dots[&(target, DotKind::Ability(kind))].clone();
                    iteration.execute_dot_tick(DotKind::Ability(kind), &dot, target, 1.0);
                }
                assert_eq!(
                    iteration.test_proc_count(KINDLING),
                    (initial + tick) * 2,
                    "{kind:?}"
                );
            }
            let procs = iteration.test_proc_count(KINDLING);
            // Extending an actor's life is not a duration refresh on a target GE.
            iteration.increase_dot_duration(0, DotKind::Ability(kind), 2_000);
            iteration.increase_dot_duration_up_to_total(1, DotKind::Ability(kind), 2_000);
            assert_eq!(iteration.test_proc_count(KINDLING), procs);
        }
    }
}

#[test]
fn ordinary_dot_executions_do_not_notify_kindling_again() {
    let source = dot_ability(DpsAbilityKind::SearingBlaze, 1.0);
    let mut profile = profile(vec![source.clone()]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 94);
    let compiled = Arc::clone(&iteration.profile.abilities[0]);
    iteration.apply_dot(
        0,
        source.kind,
        &compiled,
        source.dot.unwrap(),
        DamageContext::NONE,
    );
    let dot = iteration.common.dots[&(0, DotKind::Ability(source.kind))].clone();
    iteration.common.now_ms = 1_000;
    iteration.execute_dot_tick(DotKind::Ability(source.kind), &dot, 0, 1.0);
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
}

#[test]
fn diamond_amplifier_refresh_and_shadow_mark_reapplication_supply_extra_kindling_attempts() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.effect_duration_ms = 15_000;
    let mut profile = profile(vec![mark]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 91);
    iteration.impact(0, false, 1.0, 0, DamageContext::NONE);
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
    iteration.common.now_ms = 500;
    iteration.impact(0, false, 1.0, 0, DamageContext::NONE);
    assert_eq!(iteration.test_proc_count(KINDLING), 3);

    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.GemSingleTargetProcOnDamageHeal",
        [
            ("powerCoefficient", 1.0),
            ("procsPerMinute", 60_000.0),
            ("ppmHasteScaling", 1.0),
            ("debuffDurationSeconds", 20.0),
            ("damageIncreasePerStack", 0.4),
            ("maximumStacks", 1.0),
            ("harmoniousSoulDamageIncreasePerStack", 0.0),
            ("harmoniousSoulStacks", 0.0),
        ],
    ));
    let mut iteration = Iteration::new(&profile, &apl, 1, 92);
    let source = iteration.test_damage_source("hero-hit");
    iteration.trigger_dynamic_on_damage(None, source, 100.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    iteration.common.now_ms = 500;
    iteration.trigger_dynamic_on_damage(None, source, 100.0, false, 0, DamageContext::NONE);
    assert_eq!(
        iteration.test_proc_count(KINDLING),
        5,
        "capped stacks still refresh and apply"
    );
}

#[test]
fn engulfing_instances_keep_their_phase_expiry_and_union_uptime() {
    let source = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
    let mut profile = profile(vec![source.clone()]);
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 96);
    let compiled = compiled_ability(&source);
    let mut model = source.dot.unwrap();
    model.duration_ms = 2_000;
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    iteration.process_events_through(500);
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    let instances = iteration
        .dot_instances(0, source.kind)
        .map(|(_, d)| (d.next_tick_ms, d.expires_ms))
        .collect::<Vec<_>>();
    assert!(instances.contains(&(1_000, 2_000)));
    assert!(instances.contains(&(1_500, 2_500)));
    iteration.process_events_through(2_500);
    assert_eq!(iteration.ability_totals("test:engulfing-flames").hits, 4);
    assert_eq!(iteration.dot_instances(0, source.kind).count(), 0);
    assert!(
        (iteration.common.result.uptimes["dot:engulfing-flames"]
            - 2_500.0 / ENCOUNTER_DURATION_MS as f64)
            .abs()
            < 1e-12
    );
    assert_eq!(iteration.dot_remaining(source.kind), 0);
}

#[test]
fn engulfing_extensions_reach_every_instance_and_surviving_older_effect() {
    let source = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
    let profile = profile(vec![source.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 97);
    let compiled = compiled_ability(&source);
    let kind = DotKind::Ability(source.kind);
    let mut model = source.dot.unwrap();
    model.duration_ms = 4_000;
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    iteration.process_events_through(500);
    model.duration_ms = 1_000;
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    iteration.increase_dot_duration(0, kind, 1_000);
    let expiries = iteration
        .dot_instances(0, source.kind)
        .map(|(_, d)| d.expires_ms)
        .collect::<Vec<_>>();
    assert!(expiries.contains(&5_000));
    assert!(expiries.contains(&2_500));
    iteration.process_events_through(2_501);
    assert_eq!(iteration.dot_instances(0, source.kind).count(), 1);
    assert_eq!(iteration.dot_remaining(source.kind), 2_499);
}

#[test]
fn slaughter_reapplication_keeps_separate_damage_budgets() {
    let source = dot_ability(DpsAbilityKind::Slaughter, 1.0);
    let mut profile = gunde_profile();
    profile.abilities = vec![source.clone()];
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 98);
    let damage_source = iteration.ability_damage_source(&compiled_ability(&source));
    let kind = DotKind::Ability(source.kind);
    let mut model = source.dot.unwrap();
    model.duration_ms = 3_000;
    iteration.apply_damage_derived_dot(0, kind, damage_source, model, 300.0, DamageContext::NONE);
    iteration.process_events_through(500);
    iteration.apply_damage_derived_dot(0, kind, damage_source, model, 600.0, DamageContext::NONE);
    let budgets = iteration
        .dot_instances(0, source.kind)
        .map(|(_, d)| d.derived_damage_per_tick.unwrap())
        .collect::<Vec<_>>();
    assert!(budgets.contains(&100.0));
    assert!(budgets.contains(&200.0));
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
}

#[test]
fn bloodbound_period_is_unhasted_and_recasts_preserve_older_actor() {
    let source = dot_ability(DpsAbilityKind::BloodboundSpirit, 1.0);
    let mut profile = gunde_profile();
    profile.abilities = vec![source.clone()];
    profile.haste = 1.0;
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 99);
    let compiled = compiled_ability(&source);
    iteration.apply_dot(
        0,
        source.kind,
        &compiled,
        source.dot.unwrap(),
        DamageContext::NONE,
    );
    iteration.process_events_through(500);
    iteration.apply_dot(
        0,
        source.kind,
        &compiled,
        source.dot.unwrap(),
        DamageContext::NONE,
    );
    iteration.process_events_through(1_500);
    assert_eq!(iteration.ability_totals("test:bloodbound-spirit").hits, 2);
    assert!(
        iteration
            .dot_instances(0, source.kind)
            .all(|(_, d)| d.scheduled_period_ms == 1_000)
    );
}

#[test]
fn starfall_delayed_hits_survive_recasts_and_preserve_the_spawn_clock() {
    let mut source = dot_ability(DpsAbilityKind::StarfallVolley, 1.0);
    source
        .mechanic_parameters
        .insert("visualDelaySeconds".into(), 0.2);
    let mut profile = elarion_profile([source.clone()]);
    profile.haste = 1.0;
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 100);
    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(1));
    iteration.process_events_through(50);
    assert_eq!(iteration.test_proc_count(KINDLING), 0);
    iteration.impact(0, false, 1.0, 0, DamageContext::for_cast(2));
    iteration.process_events_through(99);
    assert_eq!(
        iteration.test_proc_count(KINDLING),
        0,
        "initial minimum is 200ms / 2"
    );
    iteration.process_events_through(499);
    assert!(iteration.test_proc_count(KINDLING) >= 1);
    iteration.process_events_through(550);
    assert!(
        iteration.test_proc_count(KINDLING) >= 2,
        "both first pulses apply after moving the earlier actor"
    );
    assert_eq!(iteration.dot_instances(0, source.kind).count(), 2);
    assert!(
        iteration
            .dot_instances(0, source.kind)
            .all(|(_, d)| d.scheduled_period_ms == 500)
    );
    iteration.common.now_ms = 20_000;
    let procs = iteration.test_proc_count(KINDLING);
    let generations = iteration
        .dot_instances(0, source.kind)
        .map(|(_, d)| d.generation)
        .collect::<Vec<_>>();
    for generation in generations {
        iteration.starfall_hit(generation, 0);
    }
    assert_eq!(
        iteration.test_proc_count(KINDLING),
        procs,
        "expired actors cannot apply queued hits"
    );
}

#[test]
fn bursting_ice_recasts_keep_both_pulse_series_and_notify_the_initial_debuff() {
    let mut profile = rime_profile();
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::BurstingIce)
        .unwrap()
        .effect_duration_ms = 1_000;
    profile.mechanics = vec![certain_kindling()];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 101);
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::BurstingIce];
    iteration.impact(index, false, 1.0, 0, DamageContext::for_cast(1));
    assert_eq!(iteration.test_proc_count(KINDLING), 1);
    iteration.process_events_through(250);
    iteration.impact(index, false, 1.0, 0, DamageContext::for_cast(2));
    assert_eq!(iteration.test_proc_count(KINDLING), 2);
    iteration.process_events_through(1_250);
    assert_eq!(iteration.ability_totals("test:bursting-ice").hits, 4);
    assert_eq!(iteration.test_proc_count(KINDLING), 6);
    assert!(iteration.hero.rime().bursting_instances.is_empty());
}

#[test]
fn each_engulfing_parent_contributes_and_removes_one_devouring_stack() {
    let source = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
    let mut profile = profile(vec![source.clone()]);
    profile.mechanics.push(ardeos_mechanic(
        "legendary-devouring-flame",
        [("engulfingTargetIncomingMultiplier", 1.07)],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 102);
    let compiled = compiled_ability(&source);
    let mut model = source.dot.unwrap();
    model.duration_ms = 2_000;
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    iteration.process_events_through(500);
    iteration.apply_dot(0, source.kind, &compiled, model, DamageContext::NONE);
    assert!((iteration.target_damage_multiplier(0) - 1.07_f64.powi(2)).abs() < 1e-12);
    iteration.process_events_through(2_001);
    assert!((iteration.target_damage_multiplier(0) - 1.07).abs() < 1e-12);
    iteration.process_events_through(2_501);
    assert_eq!(iteration.target_damage_multiplier(0), 1.0);
}

#[test]
fn extension_after_last_full_tick_preserves_the_next_periodic_wake() {
    let source = dot_ability(DpsAbilityKind::EngulfingFlames, 1.0);
    let profile = profile(vec![source.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 103);
    let mut model = source.dot.unwrap();
    model.duration_ms = 1_500;
    iteration.apply_dot(
        0,
        source.kind,
        &compiled_ability(&source),
        model,
        DamageContext::NONE,
    );
    iteration.process_events_through(1_200);
    iteration.increase_dot_duration(0, DotKind::Ability(source.kind), 1_000);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:engulfing-flames").hits, 2);
    iteration.process_events_through(2_500);
    assert_eq!(
        iteration.ability_totals("test:engulfing-flames").hits,
        3,
        "final half tick remains"
    );
}
