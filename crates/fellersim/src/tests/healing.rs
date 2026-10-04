use super::*;

const COUNTER: &str = "ItemTrait.ID.CritsToIncreasedCritRating";
const PRIMARY: &str = "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff";
const DIAMOND: &str = "ItemTrait.ID.GemSingleTargetProcOnDamageHeal";
const HEAL_STREAM: &str = "RandomStream.Traits.GemSingleTargetProcOnDamageHeal_Proc.Heal";

#[test]
fn emerald_damage_cannot_split_the_shared_critical_listener_group() {
    for seed in 1..=128 {
        let mut outcomes = Vec::new();
        for emerald_position in [1, 2] {
            let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
            let mut seized = counter(1.0);
            seized.parameters.insert("criticalRating".into(), 2_000.0);
            profile.mechanics = vec![
                ardeos_mechanic(
                    PRIMARY,
                    [
                        ("procChance", 1.0),
                        ("durationSeconds", 8.0),
                        ("powerMultiplier", 1.1),
                    ],
                ),
                seized,
            ];
            profile.mechanics.insert(
                emerald_position,
                ardeos_mechanic(
                    "ItemTrait.ID.GemTargetedSpikeProc",
                    [("procChance", 1.0), ("powerCoefficient", 1.0)],
                ),
            );
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 1, seed);
            let source = iteration.test_damage_source("critical-hit");
            iteration.trigger_dynamic_on_damage(None, source, 100.0, true, 0, DamageContext::NONE);
            let spike = iteration.ability_totals("gear:ItemTrait.ID.GemTargetedSpikeProc");
            outcomes.push((spike.damage, spike.crits));
        }
        assert_eq!(
            outcomes[0], outcomes[1],
            "seed {seed}: finish critical peers before Emerald"
        );
    }
}

fn counter(required: f64) -> DynamicMechanicInstance {
    ardeos_mechanic(
        COUNTER,
        [
            ("criticalRating", 20.0),
            ("durationSeconds", 12.0),
            ("requiredCriticalStrikes", required),
        ],
    )
}

#[test]
fn amethyst_subscription_keeps_critical_heal_group_ahead_of_later_diamond() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    let mut seized = counter(1.0);
    seized.parameters.insert("criticalRating".into(), 2_000.0);
    profile.mechanics = vec![
        ardeos_mechanic(
            "ItemTrait.ID.GemDotHotOnCrit",
            [
                ("triggerDamageFraction", 0.1),
                ("durationSeconds", 8.0),
                ("periodSeconds", 2.0),
            ],
        ),
        diamond(),
        seized,
        ardeos_mechanic(
            PRIMARY,
            [
                ("procChance", 1.0),
                ("durationSeconds", 8.0),
                ("powerMultiplier", 1.1),
            ],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 7);
    let source = iteration.test_damage_source("critical-heal");
    iteration.emit_positive_healing(source, true);
    // Amethyst itself contributes no hostile damage, but the critical group
    // must finish before Diamond captures the now-guaranteed critical chance.
    assert_eq!(iteration.test_proc_count(PRIMARY), 2);
    assert_eq!(iteration.common.result.targets, vec![0.0]);
}

fn diamond() -> DynamicMechanicInstance {
    ardeos_mechanic(
        DIAMOND,
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
    )
}

#[test]
fn full_health_critical_healing_reaches_shared_stat_listeners_for_all_six_heroes() {
    for mut profile in [
        profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]),
        gunde_profile(),
        mara_profile(),
    ] {
        profile.mechanics = vec![
            counter(1.0),
            ardeos_mechanic(
                PRIMARY,
                [
                    ("procsPerMinute", 60_000.0),
                    ("ppmHasteScaling", 1.0),
                    ("durationSeconds", 10.0),
                    ("powerMultiplier", 1.1),
                ],
            ),
            ardeos_mechanic(
                "seta-proc-intellect",
                [
                    ("procsPerMinute", 60_000.0),
                    ("ppmCriticalScaling", 1.0),
                    ("durationSeconds", 14.0),
                    ("powerMultiplier", 1.18),
                    ("cooldownSeconds", 5.0),
                ],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 19);
        let source = iteration.test_damage_source("heal");
        iteration.emit_positive_healing(source, false);
        assert_eq!(
            iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
            0.0
        );
        iteration.emit_positive_healing(source, true);
        for id in [COUNTER, PRIMARY, "seta-proc-intellect"] {
            assert!(
                iteration.test_dynamic_buff(id).until_ms > 0,
                "{}: {id}",
                profile.hero_id
            );
        }
        assert_eq!(iteration.common.result.targets, vec![0.0]);
        assert_eq!(iteration.shared.spirit, 0.0);
    }
}

#[test]
fn damage_and_healing_share_critical_counter_and_set_cooldown() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![
        counter(2.0),
        ardeos_mechanic(
            "seta-proc-intellect",
            [
                ("procsPerMinute", 60_000.0),
                ("ppmCriticalScaling", 1.0),
                ("durationSeconds", 14.0),
                ("powerMultiplier", 1.18),
                ("cooldownSeconds", 5.0),
            ],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    let source = iteration.test_damage_source("heal");
    iteration.trigger_dynamic_on_damage(None, source, 100.0, true, 0, DamageContext::NONE);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 1);
    iteration.emit_positive_healing(source, true);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
        20.0
    );
    assert_eq!(iteration.test_proc_count("seta-proc-intellect"), 1);
    iteration.common.now_ms = 5_000;
    iteration.emit_positive_healing(source, true);
    assert_eq!(iteration.test_proc_count("seta-proc-intellect"), 2);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
    iteration.common.now_ms = 12_000;
    iteration.emit_positive_healing(source, true);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 1);
}

#[test]
fn noncritical_heal_can_proc_critical_diamond_without_damage_spirit_or_hostile_stacks() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.critical_strike = 1.0;
    profile.mechanics = vec![diamond(), counter(1.0)];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    let source = iteration.test_damage_source("heal");
    iteration.emit_positive_healing(source, false);
    assert_eq!(iteration.test_proc_count(DIAMOND), 1);
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
        20.0
    );
    assert!(iteration.shared.proc_per_minute_states[HEAL_STREAM].has_procced);
    let index = iteration.test_mechanic_index(DIAMOND);
    assert!(iteration.shared.dynamic_proc_per_minute_states[index].is_none());
    assert_eq!(iteration.common.result.targets, vec![0.0]);
    assert_eq!(iteration.shared.spirit, 0.0);
    assert!(
        iteration
            .shared
            .dynamic_target_buffs
            .iter()
            .all(Option::is_none)
    );
    // A Trait heal is excluded regardless of whether it crits. This also
    // proves Diamond's nested Trait heal terminates without a recursion guard.
    let trait_source = iteration.profile.mechanics[index].damage_source;
    iteration.emit_positive_healing(trait_source, false);
    iteration.emit_positive_healing(trait_source, true);
    assert_eq!(iteration.test_proc_count(DIAMOND), 1);
}

#[test]
fn usurper_healing_branch_follows_damage_and_failed_initial_proc_cannot_heal() {
    let core = ability(DpsAbilityKind::FireBall, 0.0);
    let mut profile = profile(vec![core.clone()]);
    profile.critical_strike = 1.0;
    profile.mechanics = vec![
        ardeos_mechanic(
            "DynamicItemAbilityRank.07",
            [("powerCoefficient", 1.0), ("maximumTargets", 4.0)],
        ),
        counter(2.0),
    ];
    let apl = apl([]);
    let mut damage_only = 0;
    let mut damage_and_heal = 0;
    for seed in 1..=100 {
        let mut iteration = Iteration::new(&profile, &apl, 1, seed);
        iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
        assert_eq!(
            iteration
                .ability_totals("gear:DynamicItemAbilityRank.07")
                .crits,
            1
        );
        if iteration.dynamic_rating_bonus(parameter_key!("criticalRating")) > 0.0 {
            damage_and_heal += 1;
        } else {
            damage_only += 1;
            assert_eq!(iteration.test_dynamic_counter(COUNTER), 1);
        }
        let damage = iteration.common.result.targets[0];
        assert_eq!(
            iteration.shared.spirit,
            f64::from((f64::from(damage as f32).min(6_637_912.0) / 6_637_912.0 * 11.25) as f32)
        );
    }
    assert!(damage_only > 0 && damage_and_heal > 0);
    profile.critical_strike = 0.0;
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
    assert_eq!(iteration.common.result.targets, vec![0.0]);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
    assert_eq!(iteration.test_proc_count("DynamicItemAbilityRank.07"), 0);
}

#[test]
fn heretic_noncritical_heal_activates_diamonds_independent_healing_stream() {
    let core = ability(DpsAbilityKind::FireBall, 0.0);
    let mut profile = profile(vec![core.clone()]);
    profile.critical_strike = 1.0;
    profile.mechanics = vec![
        ardeos_mechanic(
            "DynamicItemAbilityRank.14",
            [("procChance", 1.0), ("powerCoefficient", 2.0)],
        ),
        diamond(),
        counter(100.0),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
    assert_eq!(
        iteration
            .ability_totals("gear:DynamicItemAbilityRank.14")
            .crits,
        0
    );
    assert!(iteration.shared.proc_per_minute_states[HEAL_STREAM].has_procced);
    // One Diamond damage crit and one Diamond heal crit; Heretic itself has neither.
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 2);
    assert_eq!(iteration.test_proc_count(DIAMOND), 2);
}

#[test]
fn heretic_diamond_heal_captures_stats_after_exact_damage_callbacks() {
    let core = ability(DpsAbilityKind::FireBall, 0.0);
    let mut profile = profile(vec![core.clone()]);
    profile.critical_strike = 0.25;
    profile.mechanics = vec![
        ardeos_mechanic(
            "DynamicItemAbilityRank.14",
            [("procChance", 1.0), ("powerCoefficient", 2.0)],
        ),
        diamond(),
        ardeos_mechanic(
            COUNTER,
            [
                ("criticalRating", 2_000.0),
                ("durationSeconds", 12.0),
                ("requiredCriticalStrikes", 1.0),
            ],
        ),
        ardeos_mechanic(
            PRIMARY,
            [
                ("procChance", 1.0),
                ("durationSeconds", 10.0),
                ("powerMultiplier", 1.1),
            ],
        ),
    ];
    let apl = apl([]);
    let mut damage_crits = 0;
    for before_generic in [false, true] {
        for seed in 0..128 {
            let mut iteration = Iteration::new(&profile, &apl, 1, seed);
            iteration.test_heretic_before_generic = before_generic;
            iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
            assert_eq!(iteration.test_proc_count(DIAMOND), 2);
            let damage =
                iteration.ability_totals("gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal");
            if damage.crits == 1 {
                damage_crits += 1;
                // Diamond's exact damage callback and nested Seized buff finish
                // before Heretic's generic callback creates its heal. Diamond's
                // follow-up heal therefore captures guaranteed critical chance.
                // The deterministic observer sees both crits without RPPM
                // suppressing a second roll at the same timestamp.
                assert_eq!(iteration.test_proc_count(PRIMARY), 2, "seed {seed}");
            }
            assert_eq!(
                iteration
                    .ability_totals("gear:DynamicItemAbilityRank.14")
                    .crits,
                0
            );
        }
    }
    assert!(damage_crits > 0 && damage_crits < 128);
}

#[test]
fn winters_blessing_batches_direct_damage_without_haste_and_flushes_on_expiry() {
    let mut profile = rime_profile();
    profile.haste = 1.0;
    profile.critical_strike = 1.0;
    profile.mechanics = vec![diamond(), counter(100.0)];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
    let source = iteration.test_damage_source("hit");
    let periodic_source =
        iteration.profile.mechanics[iteration.test_mechanic_index(DIAMOND)].damage_source;
    iteration.damage_unscaled_key(
        None,
        periodic_source,
        100.0,
        0,
        DamageProvenance::Periodic,
        DamageContext::NONE,
    );
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 0.0);
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    assert!(iteration.hero.rime().blessing_heal_pending > 0.0);
    iteration.process_events_through(499);
    assert!(
        !iteration
            .shared
            .proc_per_minute_states
            .contains_key(HEAL_STREAM)
    );
    iteration.process_events_through(500);
    assert!(iteration.shared.proc_per_minute_states[HEAL_STREAM].has_procced);
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 0.0);
    assert!(!iteration.hero.rime().blessing_heal_scheduled);
    let until = iteration.hero.rime().winters_blessing_until;
    iteration.common.now_ms = until - 100;
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    let damage = iteration.common.result.targets.clone();
    let spirit = iteration.shared.spirit;
    iteration.process_events_through(until);
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 0.0);
    assert!(!iteration.hero.rime().blessing_heal_scheduled);
    assert_eq!(iteration.common.result.targets, damage);
    assert_eq!(iteration.shared.spirit, spirit + 1.0); // Native regen at 3 seconds.
    assert_eq!(
        iteration.shared.proc_per_minute_states[HEAL_STREAM].last_proc_seconds,
        until as f32 / 1_000.0
    );
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 0.0);
}

#[test]
fn stale_winters_blessing_timer_cannot_flush_a_new_activation() {
    let profile = rime_profile();
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
    let source = iteration.test_damage_source("hit");
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    iteration.common.now_ms = 100;
    iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
    iteration.damage_unscaled_key(
        None,
        source,
        200.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    iteration.process_events_through(500);
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 60.0);
    assert!(iteration.hero.rime().blessing_heal_scheduled);
    iteration.process_events_through(600);
    assert_eq!(iteration.hero.rime().blessing_heal_pending, 0.0);
    assert!(!iteration.hero.rime().blessing_heal_scheduled);
}

#[test]
fn diamond_exact_damage_precedes_critical_stat_buffs_in_either_profile_order() {
    let mut results = Vec::new();
    for diamond_first in [false, true] {
        let hit = ability(DpsAbilityKind::FireBall, 1.0);
        let mut profile = profile(vec![hit.clone()]);
        profile.critical_strike = 1.0;
        let primary = ardeos_mechanic(
            PRIMARY,
            [
                ("procsPerMinute", 60_000.0),
                ("ppmHasteScaling", 1.0),
                ("durationSeconds", 10.0),
                ("powerMultiplier", 2.0),
            ],
        );
        profile.mechanics = if diamond_first {
            vec![diamond(), primary]
        } else {
            vec![primary, diamond()]
        };
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 19);
        iteration.damage_hit(
            &compiled_ability(&hit),
            100.0,
            0.0,
            true,
            0,
            DamageContext::NONE,
        );
        let proc_damage = iteration
            .ability_totals("gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal")
            .damage;
        // The original crit cannot double Diamond's already-created damage spec.
        // Diamond's own nested crit subsequently activates the primary buff.
        assert!((180.0..=220.0).contains(&proc_damage), "{proc_damage}");
        assert!(iteration.test_dynamic_buff(PRIMARY).until_ms > 0);
        assert_eq!(iteration.test_proc_count(DIAMOND), 1);
        results.push(proc_damage);
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn diamond_exact_damage_precedes_first_strike_and_ignores_zero_damage() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.critical_strike = 0.0;
    profile.mechanics = vec![
        ardeos_mechanic(
            "gem-emerald-80",
            [("expertise", 0.5), ("durationSeconds", 15.0)],
        ),
        diamond(),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    let source = iteration.test_damage_source("hit");
    iteration.damage_unscaled_key(
        None,
        source,
        0.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    assert_eq!(iteration.test_proc_count(DIAMOND), 0);
    iteration.damage_unscaled_key(
        None,
        source,
        100.0,
        0,
        DamageProvenance::Direct,
        DamageContext::NONE,
    );
    let proc_damage = iteration
        .ability_totals("gear:ItemTrait.ID.GemSingleTargetProcOnDamageHeal")
        .damage;
    assert!((90.0..=110.0).contains(&proc_damage), "{proc_damage}");
    assert_eq!(iteration.test_proc_count(DIAMOND), 1);
    assert!(iteration.test_dynamic_buff("gem-emerald-80").until_ms > 0);
}

#[test]
fn winters_blessing_diamond_heal_captures_crit_when_the_batch_fires() {
    for buff_at_damage_time in [false, true] {
        let mut profile = rime_profile();
        profile.critical_strike = 0.0;
        profile.mechanics = vec![
            diamond(),
            counter(1.0),
            ardeos_mechanic(
                "DynamicItemAbilityRank.02",
                [("criticalStrikeBonus", 1.0), ("durationSeconds", 4.0)],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 19);
        iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
        let buff = Arc::clone(
            &iteration.profile.mechanics
                [iteration.test_mechanic_index("DynamicItemAbilityRank.02")],
        );
        if buff_at_damage_time {
            iteration.activate_dynamic_buff(&buff, 400, 1, 0.0);
        }
        // A Trait hit still enters Winter's Blessing but cannot proc Diamond
        // damage, isolating the healing event's stat-capture boundary.
        let source =
            iteration.profile.mechanics[iteration.test_mechanic_index(DIAMOND)].damage_source;
        iteration.damage_unscaled_key(
            None,
            source,
            100.0,
            0,
            DamageProvenance::Direct,
            DamageContext::NONE,
        );
        iteration.process_events_through(250);
        if !buff_at_damage_time {
            iteration.activate_dynamic_buff(&buff, 4_000, 1, 0.0);
        }
        iteration.process_events_through(500);
        assert_eq!(iteration.test_proc_count(DIAMOND), 1);
        assert_eq!(
            iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
            if buff_at_damage_time { 0.0 } else { 20.0 }
        );
        assert_eq!(iteration.common.result.targets, vec![100.0]);
    }
}

#[test]
fn reactivated_seized_opportunity_precedes_older_draconic_listener_for_damage_and_healing() {
    for healing in [false, true] {
        for seed in 1..=128 {
            let mut outcomes = Vec::new();
            for counter_first in [false, true] {
                let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
                profile.critical_strike = 0.0;
                let mut seized = counter(1.0);
                seized.parameters.insert("criticalRating".into(), 2_000.0);
                let set = ardeos_mechanic(
                    "seta-proc-intellect",
                    [
                        ("procsPerMinute", 2.0),
                        ("ppmCriticalScaling", 1.0),
                        ("durationSeconds", 14.0),
                        ("powerMultiplier", 1.18),
                        ("cooldownSeconds", 5.0),
                    ],
                );
                profile.mechanics = if counter_first {
                    vec![seized, set]
                } else {
                    vec![set, seized]
                };
                let apl = apl([]);
                let mut iteration = Iteration::new(&profile, &apl, 1, seed);
                let counter = Arc::clone(
                    &iteration.profile.mechanics[iteration.test_mechanic_index(COUNTER)],
                );
                iteration.trigger_critical_stat_mechanic(&counter);
                iteration.common.now_ms = 12_001;
                let source = iteration.test_damage_source("critical-event");
                if healing {
                    iteration.emit_positive_healing(source, true);
                } else {
                    iteration.trigger_dynamic_on_damage(
                        None,
                        source,
                        100.0,
                        true,
                        0,
                        DamageContext::NONE,
                    );
                }
                assert_eq!(
                    iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
                    2_000.0
                );
                outcomes.push(iteration.test_proc_count("seta-proc-intellect"));
            }
            assert_eq!(outcomes[0], outcomes[1], "seed {seed}, healing {healing}");
        }
    }
}

#[test]
fn reactivated_critical_counter_counts_each_event_once_after_reordering() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![
        ardeos_mechanic(PRIMARY, [("procsPerMinute", 0.0)]),
        counter(2.0),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    let counter = Arc::clone(&iteration.profile.mechanics[iteration.test_mechanic_index(COUNTER)]);
    iteration.trigger_critical_stat_mechanic(&counter);
    iteration.trigger_critical_stat_mechanic(&counter);
    iteration.common.now_ms = 12_001;
    let source = iteration.test_damage_source("heal");
    iteration.emit_positive_healing(source, true);
    assert_eq!(iteration.test_dynamic_counter(COUNTER), 1);
    assert_eq!(
        iteration.dynamic_rating_bonus(parameter_key!("criticalRating")),
        0.0
    );
}

#[test]
fn diamond_healing_cannot_split_the_shared_critical_listener_group() {
    let mut order_sensitive_seeds = 0;
    for seed in 1..=128 {
        let mut outcomes = Vec::new();
        for diamond_position in [0, 1, 2] {
            let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
            profile.critical_strike = 0.0;
            let mut seized = counter(1.0);
            seized.parameters.insert("criticalRating".into(), 2_000.0);
            profile.mechanics = vec![
                ardeos_mechanic(
                    PRIMARY,
                    [
                        ("procsPerMinute", 3.0),
                        ("durationSeconds", 8.0),
                        ("powerMultiplier", 1.1),
                    ],
                ),
                seized,
            ];
            profile.mechanics.insert(diamond_position, diamond());
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 1, seed);
            let source = iteration.test_damage_source("critical-heal");
            iteration.emit_positive_healing(source, true);
            // With the critical group first, Seized must finish before Diamond
            // captures crit, even if Diamond separates peers in the profile.
            // Its nested critical heal can retry an unsuccessful primary proc.
            // With Diamond's group first, its heal is non-critical instead.
            outcomes.push(iteration.test_proc_count(PRIMARY));
            assert_eq!(
                iteration.common.result.mechanic_proc_counts
                    [iteration.test_mechanic_index(COUNTER)],
                1.0
            );
            assert_eq!(iteration.test_dynamic_counter(COUNTER), 0);
            assert_eq!(iteration.test_proc_count(DIAMOND), 1);
            assert_eq!(iteration.common.result.targets, vec![0.0]);
            assert_eq!(iteration.shared.spirit, 0.0);
        }
        assert_eq!(outcomes[1], outcomes[2], "seed {seed}");
        order_sensitive_seeds += usize::from(outcomes[0] != outcomes[1]);
    }
    // Group order still matters and must remain an explicit evidence boundary.
    assert!(order_sensitive_seeds > 0);
}

fn vitality(source: &str, fraction: f64) -> DynamicMechanicInstance {
    ardeos_mechanic(
        source,
        [("periodSeconds", 2.0), ("healingHealthFraction", fraction)],
    )
}

#[test]
fn unyielding_vitality_overheals_without_critical_events_unless_diamond_procs() {
    for profile in [
        profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]),
        gunde_profile(),
        mara_profile(),
    ] {
        for (source, fraction) in [("gem-ruby-220", 0.007), ("gem-ruby-1000", 0.021)] {
            for with_diamond in [false, true] {
                let mut profile = profile.clone();
                profile.haste = 0.0;
                profile.critical_strike = 1.0;
                profile.mechanics = vec![vitality(source, fraction), counter(1.0)];
                if with_diamond {
                    profile.mechanics.push(diamond());
                }
                let apl = apl([]);
                let mut iteration = Iteration::new(&profile, &apl, 1, 19);
                iteration.process_events_through(1_999);
                assert_eq!(iteration.test_uptime(&format!("proc:{source}")), 0.0);
                assert_eq!(iteration.test_uptime(&format!("proc:{COUNTER}")), 0.0);
                iteration.process_events_through(2_000);
                assert_eq!(
                    iteration.test_uptime(&format!("proc:{source}")),
                    1.0,
                    "{}: {source}",
                    profile.hero_id
                );
                assert_eq!(
                    iteration.test_uptime(&format!("proc:{COUNTER}")),
                    f64::from(with_diamond)
                );
                if with_diamond {
                    assert_eq!(iteration.test_proc_count(DIAMOND), 1);
                }
                iteration.process_events_through(4_000);
                assert_eq!(iteration.test_uptime(&format!("proc:{source}")), 2.0);
                assert_eq!(iteration.common.result.targets, vec![0.0]);
                assert_eq!(iteration.shared.spirit, 1.0); // Native regen; healing grants none.
            }
        }
    }
}

#[test]
fn unyielding_vitality_preserves_tick_phase_through_haste_gain_and_expiration() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.haste = 0.0;
    profile.mechanics = vec![
        vitality("gem-ruby-220", 0.007),
        ardeos_mechanic(
            "DynamicItemAbilityRank.12",
            [
                ("hasteBonus", 1.0),
                ("durationSeconds", 0.5),
                ("intervalSeconds", 60.0),
                ("coreCooldownDenominatorSeconds", 30.0),
                ("coreCooldownFractionMultiplier", 4.0),
                ("minimumReductionSeconds", 0.2),
            ],
        ),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.process_events_through(500);
    let buff = Arc::clone(&iteration.profile.mechanics[1]);
    iteration.activate_dynamic_buff(&buff, 500, 1, 0.0);
    // 500 ms at normal speed, 500 at double speed, then 500 at normal speed.
    iteration.process_events_through(1_499);
    assert_eq!(iteration.test_uptime("proc:gem-ruby-220"), 0.0);
    iteration.process_events_through(1_500);
    assert_eq!(iteration.test_uptime("proc:gem-ruby-220"), 1.0);
    // Invalidated wakes at 1,250 and 2,000 must not create extra heals.
    iteration.process_events_through(3_499);
    assert_eq!(iteration.test_uptime("proc:gem-ruby-220"), 1.0);
    iteration.process_events_through(3_500);
    assert_eq!(iteration.test_uptime("proc:gem-ruby-220"), 2.0);
}

#[test]
fn wolf_heals_only_on_its_hero_commit_and_can_feed_diamond_at_full_health() {
    for (base, trigger) in [
        (
            profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]),
            DpsAbilityKind::Wildfire,
        ),
        (rime_profile(), DpsAbilityKind::FlightOfTheNavir),
        (tariq_profile(), DpsAbilityKind::ThunderCall),
        (
            elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]),
            DpsAbilityKind::LunarlightMark,
        ),
        (gunde_profile(), DpsAbilityKind::Rupture),
        (mara_profile(), DpsAbilityKind::MaidenOfDeath),
    ] {
        for with_diamond in [false, true] {
            let mut profile = base.clone();
            profile.critical_strike = 1.0;
            profile.mechanics = vec![
                ardeos_mechanic(
                    "ItemTrait.ID.Wolf",
                    [
                        ("smallHealingHealthFraction", 0.2),
                        ("mediumHealingHealthFraction", 0.25),
                        ("largeHealingHealthFraction", 0.3),
                    ],
                ),
                counter(1.0),
            ];
            if with_diamond {
                profile.mechanics.push(diamond());
            }
            let apl = apl([]);
            let mut iteration = Iteration::new(&profile, &apl, 1, 19);
            let spirit = iteration.shared.spirit;
            iteration.trigger_dynamic_on_cast(
                &compiled_ability(&ability(DpsAbilityKind::FireBall, 1.0)),
                DamageContext::NONE,
            );
            assert_eq!(
                iteration.common.result.mechanic_proc_counts[0], 0.0,
                "{}",
                profile.hero_id
            );
            // Use two distinct commits: the buff's duration does not suppress a
            // repeated application and its conditional heal.
            for expected in [1, 2] {
                iteration.common.now_ms = expected * 10_000;
                iteration.trigger_dynamic_on_cast(
                    &compiled_ability(&ability(trigger, 1.0)),
                    DamageContext::NONE,
                );
                assert_eq!(
                    iteration.common.result.mechanic_proc_counts[0],
                    expected as f64
                );
                assert_eq!(
                    iteration.test_proc_count(DIAMOND),
                    if with_diamond { expected } else { 0 }
                );
            }
            assert_eq!(
                iteration.common.result.mechanic_proc_counts
                    [iteration.test_mechanic_index(COUNTER)]
                    > 0.0,
                with_diamond,
                "Wolf itself cannot crit"
            );
            assert_eq!(iteration.common.result.targets, vec![0.0]);
            assert_eq!(iteration.shared.spirit, spirit);
        }
    }
}

#[test]
fn patient_soul_uses_the_documented_pull_delay_and_live_rating_conversion() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![ardeos_mechanic(
        "ItemTrait.ID.StandingStillStaminaExpertiseRatingIncrease",
        [("expertiseRating", 42.0), ("applicationDelaySeconds", 3.0)],
    )];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    for (now, rating) in [(0, 0.0), (2999, 0.0), (3000, 42.0), (300_000, 42.0)] {
        iteration.common.now_ms = now;
        assert_eq!(
            iteration.dynamic_rating_bonus(parameter_key!("expertiseRating")),
            rating
        );
        assert_eq!(iteration.common.result.targets, vec![0.0]);
    }
}

#[test]
fn heroic_brand_scales_weapon_damage_without_scaling_hero_or_gear_sources() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    let apl = apl([]);
    for multiplier in [1.24, 1.42, 1.6, 1.8] {
        profile.mechanics = vec![ardeos_mechanic(
            "ItemTrait.ID.WeaponHealDamageIncrease",
            [("weaponDamageMultiplier", multiplier)],
        )];
        let iteration = Iteration::new(&profile, &apl, 1, 19);
        for kind in [
            None,
            Some(DpsAbilityKind::FireBall),
            Some(DpsAbilityKind::Wildfire),
        ] {
            assert_eq!(iteration.source_damage_multiplier(kind), 1.0);
        }
        // Representative direct, channeled and periodic weapon source kinds.
        for kind in [
            DpsAbilityKind::WeaponFrostVolley,
            DpsAbilityKind::WeaponArcaneChannel,
            DpsAbilityKind::WeaponShadowMark,
        ] {
            assert_eq!(iteration.source_damage_multiplier(Some(kind)), multiplier);
        }
    }
}
