use super::super::*;

#[test]
fn tariq_contract_compiles_the_complete_current_kit() {
    let request = tariq_request(tariq_profile());

    validate(&request).expect("complete Tariq request validates");
    let compiled = CompiledProfile::try_from(&request).expect("Tariq profile compiles");

    assert_eq!(compiled.contract.hero, HeroIdentity::Tariq);
    assert_eq!(compiled.contract.model_version, TARIQ_MODEL_VERSION);
    assert_eq!(compiled.abilities.len(), 12);
    assert!(compiled.source.abilities.is_empty());
    assert_eq!(
        compiled
            .ability(DpsAbilityKind::TariqChainLightning)
            .expect("Chain Lightning index")
            .parameters
            .get(parameter_key!("chainHits")),
        Some(3.0)
    );
}

#[test]
fn tariq_chain_lightning_preserves_jump_scaling_targets_and_fury() {
    let profile = tariq_profile();
    let apl = apl([("tariq-chain-lightning", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    iteration.hero.tariq_mut().thunder_call_until = 5_000;

    iteration.cast(2);

    let chain = iteration.ability_totals("test:tariq-chain-lightning");
    assert_eq!(chain.hits, 3);
    assert_eq!(chain.targets_hit_mask, 0b111);
    assert_eq!(chain.damage, 450.0);
    assert_eq!(iteration.hero.tariq().fury, 6.0);
}

#[test]
fn tariq_focused_wrath_discount_and_damage_are_consumed_per_spender() {
    let profile = tariq_profile();
    let apl = apl([("focused-wrath", None), ("skull-crusher", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);

    iteration.cast(5);
    iteration.hero.tariq_mut().fury = 25.0;
    iteration.cast(7);

    assert_eq!(iteration.hero.tariq().fury, 12.5);
    assert_eq!(iteration.hero.tariq().focused_wrath_stacks, 1);
    assert!((iteration.ability_totals("test:skull-crusher").damage - 110.0).abs() < 1e-10);
}

#[test]
fn tariq_raging_tempest_spends_spirit_starts_heroism_and_builds_expertise() {
    let profile = tariq_profile();
    let apl = apl([("raging-tempest", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    iteration.shared.spirit = 100.0;

    iteration.cast(6);
    assert_eq!(iteration.hero.tariq().thunder_call_until, 3_000);
    assert_eq!(iteration.buff_stacks(AplBuff::ThunderCall), 1);
    assert!(
        iteration
            .can_cast(DpsAbilityKind::TariqChainLightning)
            .is_some()
    );
    iteration.process_events_through(2_500);

    // The initial Expertise stack affects impact; each pulse captures before adding its stack.
    let damage_spirit: f64 = [101.0, 51.0, 51.0]
        .into_iter()
        .map(|damage| f64::from((damage / 1337.0_f64 * 0.75) as f32))
        .sum();
    assert_eq!(iteration.shared.spirit, damage_spirit);
    assert_eq!(iteration.shared.heroism_until, 5_000);
    assert_eq!(iteration.hero.tariq().raging_current_stacks, 3);
    assert!((iteration.effective_expertise() - 0.03).abs() < 1e-10);
    let raging = iteration.ability_totals("test:raging-tempest");
    assert_eq!(raging.hits, 3);
    assert!((raging.damage - 203.0).abs() < 1e-10);
}

#[test]
fn pneuma_grants_starting_spirit_and_spirit_on_culling_strike_commit() {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        13,
        [("startingSpirit", 50.0), ("spiritPerCast", 1.0)],
    ));
    let apl = apl([("culling-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 4);

    assert_eq!(iteration.shared.spirit, 50.0);
    iteration.hero.tariq_mut().fury = 10.0;
    let culling_strike = iteration.common.abilities_by_kind[&DpsAbilityKind::CullingStrike];
    iteration.cast(culling_strike);

    // Pneuma grants one on commit; the 140-damage strike grants ordinary Spirit too.
    assert_eq!(
        iteration.shared.spirit,
        51.0 + f64::from((140.0_f64 / 1337.0 * 0.75) as f32)
    );
}

#[test]
fn tariq_schism_procs_stack_expire_and_empower_the_opposite_spender() {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        9,
        [
            ("damageMultiplier", 2.5),
            ("procChance", 1.0),
            ("maximumStacks", 2.0),
            ("durationSeconds", 30.0),
        ],
    ));
    let apl = apl([("skull-crusher", None), ("hammer-storm", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);

    iteration.hero.tariq_mut().fury = 100.0;
    iteration.cast(7);
    assert_eq!(iteration.buff_stacks(AplBuff::SchismHammerStorm), 1);
    assert_eq!(
        iteration.buff_remaining_ms(AplBuff::SchismHammerStorm),
        29_000
    );

    iteration.cast(0);
    assert_eq!(iteration.test_proc_count("ink-talent-id-talent9"), 2);
    assert_eq!(iteration.buff_stacks(AplBuff::SchismHammerStorm), 0);
    assert_eq!(iteration.buff_stacks(AplBuff::SchismSkullCrusher), 1);
    let empowered_hammer_damage = iteration.ability_totals("test:hammer-storm").damage;
    assert!(
        (empowered_hammer_damage - 1_659.0).abs() < 1e-10,
        "empowered Hammer Storm dealt {empowered_hammer_damage}"
    );

    let schism_expires_at = iteration.hero.tariq().schism_hammer_until;
    iteration.process_events_through(schism_expires_at.saturating_add(1));
    assert_eq!(iteration.buff_stacks(AplBuff::SchismSkullCrusher), 0);
    assert_eq!(iteration.buff_remaining_ms(AplBuff::SchismSkullCrusher), 0);
}

#[test]
fn tariq_passive_fury_uses_selected_talent_parameters() {
    let mut profile = tariq_profile();
    profile
        .talents
        .push(tariq_talent(6, [("furyPerSecond", 2.0)]));
    profile.talents.push(tariq_talent(
        7,
        [
            ("furyPerTick", 0.4),
            ("furyPeriodSeconds", 0.1),
            ("furyDurationSeconds", 5.0),
            ("hasteBonus", 0.1),
        ],
    ));
    let apl = apl([("tariq-attack", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 4);
    iteration.cast(3);

    iteration.process_events_through(1_000);

    assert!((iteration.hero.tariq().fury - 6.0).abs() < 1e-10);
    assert!((iteration.effective_haste() - 0.1).abs() < 1e-10);
}

#[test]
fn tariq_spirit_refund_returns_actual_spend_after_delay_and_caps_resources() {
    let mut profile = tariq_profile();
    profile.spirit = 1.0;
    let apl = apl([("skull-crusher", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    let ability = iteration
        .ability(DpsAbilityKind::SkullCrusher)
        .unwrap()
        .clone();
    iteration.hero.tariq_mut().fury = 10.0;
    iteration.shared.spirit = profile.max_spirit - 0.5;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    iteration.try_tariq_spirit_refund(&ability, 12.5);
    assert_eq!(iteration.shared.spirit, profile.max_spirit);
    assert_eq!(iteration.hero.tariq().fury, 10.0);
    iteration.process_events_through(199);
    assert_eq!(iteration.hero.tariq().fury, 10.0);
    iteration.process_events_through(200);
    assert_eq!(iteration.hero.tariq().fury, 22.5);
    iteration.hero.tariq_mut().fury = profile.max_primary_resource - 1.0;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    iteration.try_tariq_spirit_refund(&ability, 12.5);
    iteration.process_events_through(400);
    assert_eq!(iteration.hero.tariq().fury, profile.max_primary_resource);
    let count = iteration.test_proc_count("spirit-refund");
    iteration.try_tariq_spirit_refund(&ability, 0.0);
    assert_eq!(iteration.test_proc_count("spirit-refund"), count);
}

#[test]
fn tariq_paid_spenders_roll_once_and_free_culling_does_not_roll() {
    for kind in [
        DpsAbilityKind::HammerStorm,
        DpsAbilityKind::SkullCrusher,
        DpsAbilityKind::CullingStrike,
    ] {
        let mut profile = tariq_profile();
        profile.spirit = 1.0;
        let apl = apl([("skull-crusher", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 37);
        iteration.hero.tariq_mut().fury = 100.0;
        iteration.shared.controlled_random_states.insert(
            SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
            ControlledRandomState {
                failure_threshold: 0.0,
                chance_factor: 0.5,
                chance_bucket: 50,
            },
        );
        let index = iteration.common.abilities_by_kind[&kind];
        iteration.cast(index);
        assert_eq!(iteration.test_proc_count("spirit-refund"), 1, "{kind:?}");
    }
    let mut profile = tariq_profile();
    profile.spirit = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "legendary-ink-trait3",
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("executionersGrinProcChance", 0.0),
            ("executionersGrinMaximumStacks", 2.0),
            ("executionersGrinDurationSeconds", 5.0),
            ("executionersGrinFurySpent", 50.0),
        ],
    ));
    let apl = apl([("culling-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    iteration.hero.tariq_mut().executioners_grin_until = 5_000;
    iteration.hero.tariq_mut().executioners_grin_stacks = 1;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::CullingStrike];
    iteration.cast(index);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(
        iteration.shared.controlled_random_states[SPIRIT_PROC_RANDOM_STREAM_TAG].failure_threshold,
        0.0
    );
}

#[test]
fn thundering_vortex_retains_overflow_up_to_the_effect_cap() {
    let mut profile = tariq_profile();
    profile.mechanics.push(ardeos_mechanic(
        "legendary-ink-talent1",
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("chainFuryMultiplier", 1.5),
            ("vortexStacksRequired", 20.0),
            ("vortexMaximumStacks", 40.0),
            ("vortexDamageMultiplier", 2.0),
        ],
    ));
    let apl = apl([("chain-lightning", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    let chain = iteration
        .ability(DpsAbilityKind::TariqChainLightning)
        .unwrap()
        .clone();
    for cast in 0..45 {
        iteration.resolve_tariq_impact(&chain, 0, 100.0, false, DamageContext::for_cast(cast));
    }
    assert_eq!(iteration.hero.tariq().thundering_vortex_stacks, 40);
    iteration.tariq_trigger_chain_lightning(DamageContext::for_cast(46));
    assert_eq!(iteration.hero.tariq().thundering_vortex_stacks, 20);
}

#[test]
fn executioners_grin_checks_after_commit_caps_at_two_and_consumes_one() {
    let mut profile = tariq_profile();
    profile.max_primary_resource = 200.0;
    profile.mechanics.push(ardeos_mechanic(
        "legendary-ink-trait4",
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("executionersGrinProcChance", 1.0),
            ("executionersGrinMaximumStacks", 2.0),
            ("executionersGrinDurationSeconds", 8.0),
            ("executionersGrinFurySpent", 30.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 301);
    let culling = iteration.common.abilities_by_kind[&DpsAbilityKind::CullingStrike];
    iteration.hero.tariq_mut().fury = 80.0;
    iteration.cast(culling);
    assert_eq!(
        iteration.hero.tariq().fury,
        80.0,
        "own commit grants a free strike"
    );
    assert_eq!(
        iteration.ability_totals("test:culling-strike").damage,
        220.0,
        "free damage uses 30 budget units, independent of maximum Fury"
    );
    assert_eq!(iteration.hero.tariq().executioners_grin_stacks, 0);
    assert_eq!(iteration.hero.tariq().executioners_grin_until, 0);
    let heavy = iteration
        .ability(DpsAbilityKind::HeavyStrike)
        .unwrap()
        .clone();
    for _ in 0..3 {
        iteration.commit_tariq_ability(&heavy, DamageContext::for_cast(2));
    }
    assert_eq!(iteration.hero.tariq().executioners_grin_stacks, 2);
    iteration.cast(culling);
    assert_eq!(iteration.hero.tariq().executioners_grin_stacks, 1);
    assert_eq!(iteration.hero.tariq().fury, 80.0);
    iteration.common.now_ms = iteration.hero.tariq().executioners_grin_until;
    iteration.commit_tariq_ability(&heavy, DamageContext::for_cast(3));
    assert_eq!(
        iteration.hero.tariq().executioners_grin_stacks,
        1,
        "expired stacks cannot return"
    );
}

#[test]
fn tariq_builders_grant_fury_once_before_delayed_damage() {
    for (index, id) in [(4, "test:wild-swing"), (10, "test:face-breaker")] {
        let mut profile = tariq_profile();
        profile.abilities[index].primary_resource_generated = 7.0;
        profile.abilities[index].first_hit_delay_ms = 2_000;
        let apl = apl([("wild-swing", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 53);
        iteration.cast(index);
        assert_eq!(iteration.hero.tariq().fury, 7.0);
        assert_eq!(iteration.ability_totals(id).hits, 0);
        iteration.process_events_through(2_500);
        assert_eq!(iteration.hero.tariq().fury, 7.0);
        assert!(iteration.ability_totals(id).hits > 0);
    }
}

#[test]
fn tariq_wild_swing_rolls_misses_and_converts_generic_crit_without_critting() {
    let mut profile = tariq_profile();
    profile.critical_strike = 0.25;
    profile.abilities[4].primary_resource_generated = 3.0;
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 71);
    for cast_id in 1..=2_000 {
        iteration.impact(4, true, 1.0, 0, DamageContext::for_cast(cast_id));
    }
    let totals = iteration.ability_totals("test:wild-swing");
    assert!(totals.hits > 1_850 && totals.hits < 1_950, "{totals:?}");
    assert_eq!(totals.crits, 0);
    assert_eq!(totals.damage, totals.hits as f64 * 125.0);
    // Impact-only callbacks, including misses, must not duplicate commit Fury.
    assert_eq!(iteration.hero.tariq().fury, 0.0);
}

#[test]
fn tariq_face_breaker_cleave_copies_noncritical_loss_after_delay_with_secondary_falloff() {
    let mut profile = tariq_profile();
    profile.expertise = 0.5;
    let apl = apl([("face-breaker", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 17, 73);
    let mut context = DamageContext::for_cast(1);
    context.multiply_damage(2.0);
    iteration.impact(10, true, 1.0, 0, context);
    let primary = iteration.ability_totals("test:face-breaker").damage;
    assert_eq!(iteration.ability_totals("test:face-breaker").hits, 1);
    assert_eq!(iteration.ability_totals("test:face-breaker").crits, 0);
    iteration.process_events_through(299);
    assert_eq!(iteration.ability_totals("test:face-breaker").hits, 1);
    // Changing stats during the visual delay cannot rescale copied health loss.
    iteration.shared.heroism_until = 5_000;
    iteration.process_events_through(300);
    let totals = iteration.ability_totals("test:face-breaker");
    let secondary = (primary * 0.5 * multi_target_damage_falloff(16, 12.0)).round();
    assert_eq!(totals.hits, 17);
    assert_eq!(totals.damage, primary + 16.0 * secondary);
    assert_eq!(totals.crits, 0);
}

#[test]
fn tariq_face_breaker_zero_damage_does_not_trigger_cleave() {
    let mut profile = tariq_profile();
    profile.power = 0.0;
    let apl = apl([("face-breaker", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 79);
    iteration.impact(10, true, 1.0, 0, DamageContext::for_cast(1));
    iteration.process_events_through(300);
    assert_eq!(iteration.ability_totals("test:face-breaker").hits, 1);
}

#[test]
fn tariq_far_beyond_driven_caps_refreshes_and_restarts_after_expiry() {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        8,
        [
            ("spiritPerStack", 0.015),
            ("maximumStacks", 5.0),
            ("durationSeconds", 20.0),
        ],
    ));
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 83);
    let ability = iteration
        .ability(DpsAbilityKind::HeavyStrike)
        .unwrap()
        .clone();
    for cast_id in 1..=6 {
        iteration.commit_tariq_ability(&ability, DamageContext::for_cast(cast_id));
    }
    assert_eq!(iteration.buff_stacks(AplBuff::FarBeyondDriven), 5);
    assert!((iteration.effective_spirit() - 0.075).abs() < 1e-10);
    iteration.process_events_through(19_000);
    iteration.commit_tariq_ability(&ability, DamageContext::for_cast(7));
    assert_eq!(iteration.hero.tariq().far_beyond_driven_until, 39_000);
    iteration.process_events_through(39_000);
    assert_eq!(iteration.buff_stacks(AplBuff::FarBeyondDriven), 0);
    iteration.commit_tariq_ability(&ability, DamageContext::for_cast(8));
    assert_eq!(iteration.buff_stacks(AplBuff::FarBeyondDriven), 1);
    assert!((iteration.effective_spirit() - 0.015).abs() < 1e-10);
}

fn tariq_kill_em_all_profile() -> NormalizedDpsProfile {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        11,
        [
            ("procChance", 1.0),
            ("charges", 2.0),
            ("durationSeconds", 8.0),
            ("damageMultiplier", 2.5),
        ],
    ));
    profile
}

#[test]
fn tariq_kill_em_all_filters_commits_and_does_not_roll_while_active() {
    const STREAM: &str = "RandomStream.Ink.Talents.AutoAttackBuff.AlwaysHitWindowBuff";
    let profile = tariq_kill_em_all_profile();
    let apl = apl([("wild-swing", None)]);
    for index in 0..12 {
        let mut iteration = Iteration::new(&profile, &apl, 1, 89);
        let ability = iteration.profile.abilities[index].clone();
        iteration.commit_tariq_ability(&ability, DamageContext::for_cast(1));
        let eligible = matches!(index, 0 | 2 | 4 | 6 | 7 | 8 | 10);
        assert_eq!(
            iteration.test_proc_count("ink-talent-id-talent11"),
            u64::from(eligible),
            "{index}"
        );
        assert_eq!(
            iteration
                .shared
                .controlled_random_states
                .contains_key(STREAM),
            eligible
        );
        if eligible {
            let before = format!("{:?}", iteration.shared.controlled_random_states);
            iteration.process_events_through(1_000);
            iteration.commit_tariq_ability(&ability, DamageContext::for_cast(2));
            assert_eq!(
                format!("{:?}", iteration.shared.controlled_random_states),
                before
            );
            assert_eq!(iteration.hero.tariq().kill_em_all_until, 8_000);
            iteration.process_events_through(8_000);
            iteration.commit_tariq_ability(&ability, DamageContext::for_cast(3));
            assert_eq!(iteration.test_proc_count("ink-talent-id-talent11"), 2);
            assert_eq!(iteration.hero.tariq().kill_em_all_stacks, 2);
        }
    }
}

#[test]
fn tariq_kill_em_all_resets_heavy_strike_and_empowers_exactly_two_casts() {
    let mut profile = tariq_kill_em_all_profile();
    profile.abilities[1].cooldown_ms = 8_000;
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 97);
    iteration.cast(1);
    assert!(
        iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::HeavyStrike)
    );
    let wild = iteration
        .ability(DpsAbilityKind::WildSwing)
        .unwrap()
        .clone();
    iteration.commit_tariq_ability(&wild, DamageContext::for_cast(2));
    assert!(
        !iteration
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::HeavyStrike)
    );
    iteration.cast(1);
    assert_eq!(iteration.hero.tariq().kill_em_all_stacks, 1);
    // Exercise consumption separately from the ordinary cooldown restriction.
    iteration.reset_cooldown(DpsAbilityKind::HeavyStrike);
    iteration.cast(1);
    assert_eq!(iteration.buff_stacks(AplBuff::KillEmAll), 0);
    iteration.reset_cooldown(DpsAbilityKind::HeavyStrike);
    iteration.cast(1);
    assert_eq!(iteration.ability_totals("test:heavy-strike").damage, 700.0);
}

#[test]
fn tariq_bloodline_waits_a_full_period_and_caps_fury() {
    let mut profile = tariq_profile();
    profile
        .talents
        .push(tariq_talent(6, [("furyPerSecond", 1.0)]));
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 101);
    iteration.process_events_through(999);
    assert_eq!(iteration.hero.tariq().fury, 0.0);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.hero.tariq().fury, 1.0);
    iteration.hero.tariq_mut().fury = 99.5;
    iteration.process_events_through(2_000);
    assert_eq!(iteration.hero.tariq().fury, 100.0);
}

#[test]
fn tariq_left_hand_path_adds_face_breaker_crit_and_cleave_copies_the_crit() {
    let mut profile = tariq_profile();
    profile.critical_strike = 0.6;
    profile
        .talents
        .push(tariq_talent(2, [("criticalStrikeBonus", 0.4)]));
    let apl = apl([("face-breaker", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 103);
    iteration.cast(10);
    let totals = iteration.ability_totals("test:face-breaker");
    assert_eq!(totals.crits, 1);
    assert_eq!(totals.hits, 3);
    assert_eq!(totals.damage, 400.0);
}

fn square_hammer_profile() -> NormalizedDpsProfile {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        18,
        [
            ("maximumStacks", 5.0),
            ("durationSeconds", 20.0),
            ("cooldownReductionPerStackSeconds", 1.0),
            ("expertiseBonus", 0.05),
            ("expertiseDurationSeconds", 4.0),
        ],
    ));
    profile
}

#[test]
fn tariq_square_hammer_consumes_stacks_for_one_expertise_buff_and_scaled_cooldown_reduction() {
    for stacks in [1, 5] {
        let mut profile = square_hammer_profile();
        profile.abilities[3].cooldown_ms = 60_000;
        let apl = apl([("skull-crusher", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 131);
        iteration.commit_ability(3, &mut DamageContext::for_cast(1), false, 0.0);
        iteration.hero.tariq_mut().square_hammer_stacks = stacks;
        iteration.hero.tariq_mut().square_hammer_until = 20_000;
        iteration.hero.tariq_mut().fury = 100.0;
        iteration.cast(7);
        assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 0);
        assert_eq!(iteration.buff_remaining_ms(AplBuff::SquareHammer), 0);
        assert_eq!(iteration.buff_stacks(AplBuff::SquareHammerExpertise), 1);
        assert_eq!(
            iteration.buff_remaining_ms(AplBuff::SquareHammerExpertise),
            3_000
        );
        assert!((iteration.effective_expertise() - 0.05).abs() < 1e-10);
        assert_eq!(
            iteration.cooldown_remaining_ms(DpsAbilityKind::ThunderCall),
            59_000 - u64::from(stacks) * 1_000
        );
        iteration.process_events_through(4_000);
        assert_eq!(iteration.effective_expertise(), 0.0);
        assert_eq!(iteration.buff_stacks(AplBuff::SquareHammerExpertise), 0);
    }
}

#[test]
fn tariq_square_hammer_new_stacks_do_not_extend_expertise_and_expired_stacks_restart() {
    let profile = square_hammer_profile();
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 137);
    iteration.hero.tariq_mut().square_hammer_stacks = 5;
    iteration.hero.tariq_mut().square_hammer_until = 20_000;
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.cast(7);
    let heavy = iteration
        .ability(DpsAbilityKind::HeavyStrike)
        .unwrap()
        .clone();
    iteration.commit_tariq_ability(&heavy, DamageContext::for_cast(2));
    assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 1);
    assert_eq!(iteration.hero.tariq().square_hammer_until, 21_000);
    assert_eq!(iteration.hero.tariq().square_hammer_expertise_until, 4_000);
    iteration.process_events_through(4_000);
    assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 1);
    assert_eq!(iteration.effective_expertise(), 0.0);
    for cast in 3..=8 {
        iteration.commit_tariq_ability(&heavy, DamageContext::for_cast(cast));
    }
    assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 5);
    iteration.process_events_through(24_000);
    assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 0);
    iteration.commit_tariq_ability(&heavy, DamageContext::for_cast(9));
    assert_eq!(iteration.buff_stacks(AplBuff::SquareHammer), 1);
}

#[test]
fn tariq_square_hammer_expired_stacks_do_not_grant_expertise_or_reduce_cooldown() {
    let mut profile = square_hammer_profile();
    profile.abilities[3].cooldown_ms = 60_000;
    let apl = apl([("skull-crusher", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 139);
    iteration.commit_ability(3, &mut DamageContext::for_cast(1), false, 0.0);
    iteration.hero.tariq_mut().square_hammer_stacks = 5;
    iteration.hero.tariq_mut().square_hammer_until = 20_000;
    iteration.process_events_through(20_000);
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.cast(7);
    assert_eq!(iteration.effective_expertise(), 0.0);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::ThunderCall),
        39_000
    );
}

#[test]
fn tariq_heavy_strike_cleave_rolls_critical_and_spread_independently() {
    let mut profile = tariq_profile();
    profile.critical_strike = 0.5;
    let apl = apl([("heavy-strike", None)]);
    let mut damages = BTreeSet::new();
    for seed in 1..=64 {
        let mut iteration = Iteration::new(&profile, &apl, 2, seed);
        iteration.cast(1);
        let totals = iteration.ability_totals("test:heavy-strike");
        assert_eq!(totals.hits, 2);
        damages.insert(totals.damage as u64);
    }
    // 100/200 primary plus independently rolled 30/60 secondary.
    assert_eq!(damages, BTreeSet::from([130, 160, 230, 260]));
    profile.critical_strike = 0.0;
    profile.abilities[1].damage_spread = 0.2;
    let mut damages = BTreeSet::new();
    for seed in 1..=64 {
        let mut iteration = Iteration::new(&profile, &apl, 2, seed);
        iteration.cast(1);
        let totals = iteration.ability_totals("test:heavy-strike");
        assert_eq!(totals.hits, 2);
        damages.insert(totals.damage as u64);
    }
    assert!(damages.len() > 10);
}

#[test]
fn tariq_heavy_strike_lightning_is_selected_at_commit_and_hits_primary_plus_secondaries_later() {
    let mut profile = tariq_profile();
    profile.abilities[1].gcd_ms = 1;
    profile.abilities[1].first_hit_delay_ms = 600;
    profile.abilities[1]
        .mechanic_parameters
        .insert("lightningDelaySeconds".into(), 0.25);
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 17, 149);
    iteration.hero.tariq_mut().thunder_call_until = 100;
    iteration.cast(1);
    iteration.process_events_through(599);
    assert_eq!(iteration.ability_totals("test:heavy-strike").hits, 0);
    iteration.process_events_through(600);
    let physical = 100.0 + 16.0 * (30.0 * multi_target_damage_falloff(16, 8.0)).round();
    assert_eq!(iteration.ability_totals("test:heavy-strike").hits, 17);
    assert_eq!(
        iteration.ability_totals("test:heavy-strike").damage,
        physical
    );
    iteration.process_events_through(849);
    assert_eq!(iteration.ability_totals("test:heavy-strike").hits, 17);
    iteration.process_events_through(850);
    let totals = iteration.ability_totals("test:heavy-strike");
    assert_eq!(totals.hits, 34);
    assert_eq!(
        totals.damage,
        physical + 50.0 + 16.0 * (50.0 * multi_target_damage_falloff(16, 8.0)).round()
    );
    let mut late = Iteration::new(&profile, &apl, 3, 151);
    late.cast(1);
    late.hero.tariq_mut().thunder_call_until = 5_000;
    late.process_events_through(850);
    assert_eq!(late.ability_totals("test:heavy-strike").hits, 3);
}

#[test]
fn tariq_heavy_strike_lightning_recaptures_attributes_after_physical_damage() {
    let mut profile = square_hammer_profile();
    profile.abilities[1].gcd_ms = 1;
    profile.abilities[1].first_hit_delay_ms = 600;
    profile.abilities[1]
        .mechanic_parameters
        .insert("lightningDelaySeconds".into(), 0.25);
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 157);
    iteration.hero.tariq_mut().thunder_call_until = 1_000;
    iteration.cast(1);
    iteration.hero.tariq_mut().square_hammer_expertise_until = 700;
    iteration.process_events_through(600);
    let physical = 105.0 + 2.0 * 32.0;
    assert_eq!(
        iteration.ability_totals("test:heavy-strike").damage,
        physical
    );
    iteration.process_events_through(850);
    assert_eq!(
        iteration.ability_totals("test:heavy-strike").damage,
        physical + 150.0
    );
}

#[test]
fn tariq_attack_and_heavy_strike_grant_fury_before_visual_damage_without_duplicate() {
    for (index, kind) in [(1, "heavy-strike"), (11, "tariq-attack")] {
        let mut profile = tariq_profile();
        profile.abilities[index].gcd_ms = 1;
        profile.abilities[index].first_hit_delay_ms = 600;
        profile.abilities[index].primary_resource_generated = 12.0;
        let apl = apl([(kind, None)]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 163);
        iteration.cast(index);
        assert_eq!(iteration.hero.tariq().fury, 12.0);
        assert_eq!(iteration.ability_totals(&format!("test:{}", kind)).hits, 0);
        iteration.process_events_through(600);
        assert_eq!(iteration.hero.tariq().fury, 12.0);
    }
}

#[test]
fn tariq_heavy_strike_kill_em_all_modifier_applies_once_to_each_independent_spec() {
    let profile = tariq_kill_em_all_profile();
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 167);
    iteration.hero.tariq_mut().kill_em_all_until = 1_000;
    iteration.hero.tariq_mut().kill_em_all_stacks = 1;
    iteration.hero.tariq_mut().thunder_call_until = 1_000;
    iteration.cast(1);
    assert_eq!(iteration.hero.tariq().kill_em_all_stacks, 0);
    let total = iteration.ability_totals("test:heavy-strike");
    assert_eq!(total.hits, 4);
    assert_eq!(total.damage, 2.5 * (100.0 + 30.0 + 50.0 * 2.0));
}

#[test]
fn tariq_automatic_attack_starts_without_reset_and_repeats_on_native_swing_duration() {
    let profile = tariq_profile();
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 88);
    iteration.process_events_through(5_000);
    assert!(iteration.tariq_in_hit_window());
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 0);
    // Chain starts auto-attacking but does not reset the idle 29.4-second timer.
    iteration.cast(2);
    iteration.process_events_through(34_399);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 0);
    iteration.process_events_through(34_400);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 1);
    assert!(!iteration.tariq_in_hit_window());
    iteration.process_events_through(35_000);
    assert!(iteration.tariq_in_hit_window());
    assert_eq!(iteration.ability_totals("test:tariq-attack").hits, 1);
    iteration.process_events_through(64_400);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 2);
}

#[test]
fn tariq_weak_heavy_grants_reduced_fury_but_reset_precedes_its_delayed_damage() {
    let mut profile = tariq_profile();
    profile.abilities[1].first_hit_delay_ms = 600;
    profile.abilities[1].primary_resource_generated = 20.0;
    profile.talents.push(tariq_talent(
        18,
        [
            ("maximumStacks", 5.0),
            ("durationSeconds", 20.0),
            ("expertiseBonus", 0.05),
            ("expertiseDurationSeconds", 4.0),
            ("cooldownReductionPerStackSeconds", 1.0),
        ],
    ));
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 89);
    iteration.hero.tariq_mut().swing_remaining_ms = 29_800.0;
    iteration.cast(1);
    assert_eq!(iteration.hero.tariq().fury, 4.0);
    assert_eq!(iteration.hero.tariq().square_hammer_stacks, 0);
    assert_eq!(iteration.ability_totals("test:heavy-strike").damage, 100.0);
    assert!(iteration.tariq_in_hit_window());
}

#[test]
fn tariq_heavy_damage_checks_the_live_window_after_submission() {
    let mut profile = tariq_profile();
    profile.abilities[1].gcd_ms = 1;
    profile.abilities[1].first_hit_delay_ms = 600;
    let apl = apl([("heavy-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 90);
    iteration.cast(1);
    // Model an intervening automatic activation resetting the native timer.
    iteration.hero.tariq_mut().swing_remaining_ms = 30_000.0;
    iteration.hero.tariq_mut().swing_updated_ms = iteration.common.now_ms;
    iteration.process_events_through(600);
    assert_eq!(iteration.ability_totals("test:heavy-strike").damage, 20.0);
}

#[test]
fn tariq_swing_clock_preserves_elapsed_progress_across_haste_expiry() {
    let mut profile = tariq_profile();
    profile.heroism_haste = 1.0;
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 91);
    iteration.hero.tariq_mut().auto_attacking = true;
    iteration.hero.tariq_mut().swing_remaining_ms = 1_000.0;
    iteration.shared.heroism_until = 400;
    iteration.process_events_through(400);
    assert_eq!(iteration.hero.tariq().swing_remaining_ms, 200.0);
    assert_eq!(iteration.hero.tariq().swing_rate, 1.0);
    iteration.process_events_through(599);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 0);
    iteration.process_events_through(600);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 1);
}

#[test]
fn tariq_swing_timer_continues_while_an_active_ability_blocks_activation() {
    let profile = tariq_profile();
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 92);
    iteration.hero.tariq_mut().auto_attacking = true;
    iteration.hero.tariq_mut().swing_remaining_ms = 100.0;
    iteration.hero.tariq_mut().auto_blocked_until = 1_000;
    iteration.process_events_through(999);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 0);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.ability_totals("test:tariq-attack").casts, 1);
    assert_eq!(iteration.hero.tariq().swing_remaining_ms, 29_100.0);
}

#[test]
fn tariq_leap_grants_high_road_fury_and_mouth_for_war_only_after_landing() {
    for (high_road, mouth) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut profile = tariq_profile();
        profile.abilities[9].first_hit_delay_ms = 800;
        profile.abilities[9].gcd_ms = 1;
        profile.abilities[9].primary_resource_generated = 20.0;
        if high_road {
            profile.talents.push(tariq_talent(14, []));
        }
        if mouth {
            profile
                .talents
                .push(tariq_talent(5, [("focusedWrathStacks", 1.0)]));
        }
        let apl = apl([("leap-smash", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 3, 93);
        iteration.cast(9);
        assert_eq!(iteration.hero.tariq().fury, 0.0);
        assert_eq!(iteration.hero.tariq().focused_wrath_stacks, 0);
        iteration.process_events_through(800);
        assert_eq!(
            iteration.hero.tariq().fury,
            if high_road { 20.0 } else { 0.0 }
        );
        assert_eq!(
            iteration.hero.tariq().focused_wrath_stacks,
            u32::from(mouth)
        );
        assert!(!iteration.hero.tariq().auto_attacking);
    }
}

#[test]
fn tariq_schism_spends_one_charge_with_a_fixed_multiplier() {
    let mut profile = tariq_profile();
    profile.talents.push(tariq_talent(
        9,
        [
            ("damageMultiplier", 2.5),
            ("procChance", 0.0),
            ("maximumStacks", 2.0),
            ("durationSeconds", 30.0),
        ],
    ));
    let apl = apl([("skull-crusher", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 94);
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.hero.tariq_mut().schism_hammer_stacks = 2;
    iteration.hero.tariq_mut().schism_hammer_until = 30_000;
    iteration.cast(7);
    assert_eq!(iteration.hero.tariq().schism_hammer_stacks, 1);
    assert_eq!(iteration.hero.tariq().schism_hammer_until, 30_000);
    assert_eq!(iteration.ability_totals("test:skull-crusher").damage, 250.0);
    iteration.cast(7);
    assert_eq!(iteration.hero.tariq().schism_hammer_stacks, 0);
    assert_eq!(iteration.ability_totals("test:skull-crusher").damage, 500.0);
}

#[test]
fn tariq_chain_owner_returns_delay_enemy_hits_without_granting_fury() {
    let mut profile = tariq_profile();
    profile.abilities[2].first_hit_delay_ms = 300;
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 171);
    iteration.tariq_trigger_chain_lightning(DamageContext::for_cast(1));
    iteration.process_events_through(299);
    assert_eq!(iteration.hero.tariq().fury, 0.0);
    iteration.process_events_through(300);
    assert_eq!(iteration.hero.tariq().fury, 2.0);
    iteration.process_events_through(499);
    assert_eq!(
        iteration.ability_totals("test:tariq-chain-lightning").hits,
        1
    );
    iteration.process_events_through(500);
    assert_eq!(iteration.hero.tariq().fury, 4.0);
    iteration.process_events_through(700);
    assert_eq!(iteration.hero.tariq().fury, 6.0);
    assert_eq!(
        iteration
            .ability_totals("test:tariq-chain-lightning")
            .damage,
        300.0
    );
    assert!(!iteration.hero.tariq().auto_attacking);
}

#[test]
fn tariq_free_chain_captures_vortex_at_activation_and_preserves_event_target() {
    let mut profile = tariq_profile();
    profile.abilities[2].first_hit_delay_ms = 300;
    profile.mechanics.push(ardeos_mechanic(
        "legendary-ink-talent1",
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("chainFuryMultiplier", 1.5),
            ("vortexStacksRequired", 20.0),
            ("vortexMaximumStacks", 40.0),
            ("vortexDamageMultiplier", 2.0),
        ],
    ));
    let apl = apl([("wild-swing", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 172);
    iteration.push_event(
        200,
        TariqEvent::FreeChain {
            target: 2,
            context: DamageContext::for_cast(1).as_proc(),
        },
    );
    iteration.process_events_through(199);
    iteration.hero.tariq_mut().thundering_vortex_stacks = 20;
    iteration.process_events_through(200);
    assert_eq!(iteration.hero.tariq().thundering_vortex_stacks, 0);
    assert_eq!(
        iteration.ability_totals("test:tariq-chain-lightning").hits,
        0
    );
    iteration.process_events_through(500);
    let totals = iteration.ability_totals("test:tariq-chain-lightning");
    assert_eq!(totals.damage, 200.0);
    assert_eq!(totals.targets_hit_mask, 0b100);
    assert_eq!(iteration.hero.tariq().fury, 3.0);
}

#[test]
fn tariq_hammer_pays_once_and_preserves_submitted_lightning_after_thunder_expires() {
    let mut profile = tariq_profile();
    let hammer = &mut profile.abilities[0];
    hammer.gcd_ms = 1;
    hammer.channel = Some(ChannelModel {
        duration_ms: 430,
        tick_interval_ms: 200,
        tick_immediately: true,
        scale_duration_with_ability_time_rate: true,
        enable_partial_ticks: false,
        scale_partial_tick_damage: false,
    });
    hammer
        .mechanic_parameters
        .insert("maximumFuryCost".into(), 50.0);
    hammer
        .mechanic_parameters
        .insert("lightningDelaySeconds".into(), 0.05);
    let apl = apl([("hammer-storm", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 173);
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.hero.tariq_mut().thunder_call_until = 1;
    iteration.cast(0);
    assert_eq!(iteration.hero.tariq().fury, 50.0);
    iteration.process_events_through(550);
    let totals = iteration.ability_totals("test:hammer-storm");
    assert_eq!(totals.hits, 6);
    // Physical fixtures have zero spread; the three lightning specs retain their native 0.2 spread.
    let physical = 100.0 + 135.0 + 182.0;
    let lightning = 50.0 * (1.0 + 1.35 + 1.35_f64.powi(2));
    assert!(
        (physical + 0.9 * lightning - 2.0..=physical + 1.1 * lightning + 2.0)
            .contains(&totals.damage)
    );
    assert_eq!(iteration.hero.tariq().fury, 50.0);
}

#[test]
fn tariq_ride_the_lightning_has_its_own_phase_lifetime_and_terminal_tick() {
    let mut profile = tariq_profile();
    profile.abilities[3].gcd_ms = 1;
    profile.abilities[3].effect_duration_ms = 150;
    profile.talents.push(tariq_talent(
        7,
        [
            ("hasteBonus", 0.0),
            ("furyPerTick", 1.0),
            ("furyPeriodSeconds", 0.1),
            ("furyDurationSeconds", 0.3),
        ],
    ));
    let apl = apl([("thunder-call", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 174);
    iteration.process_events_through(37);
    iteration.cast(3);
    iteration.process_events_through(136);
    assert_eq!(iteration.hero.tariq().fury, 0.0);
    iteration.process_events_through(137);
    assert_eq!(iteration.hero.tariq().fury, 1.0);
    iteration.process_events_through(337);
    assert_eq!(iteration.hero.tariq().fury, 3.0);
    assert_eq!(iteration.buff_stacks(AplBuff::ThunderCall), 0);
    iteration.process_events_through(500);
    assert_eq!(iteration.hero.tariq().fury, 3.0);
}

#[test]
fn tariq_tempest_captures_pulse_period_and_pre_increment_snapshot_until_expiry() {
    let mut profile = tariq_profile();
    profile.heroism_haste = 1.0;
    profile.abilities[6].gcd_ms = 1;
    profile.abilities[6].effect_duration_ms = 1_100;
    profile.abilities[6]
        .mechanic_parameters
        .insert("pulseVisualDelaySeconds".into(), 0.2);
    let apl = apl([("raging-tempest", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 175);
    iteration.shared.spirit = 100.0;
    iteration.cast(6);
    assert_eq!(iteration.hero.tariq().raging_period_ms, 500);
    iteration.shared.heroism_until = 250;
    iteration.process_events_through(699);
    assert_eq!(iteration.hero.tariq().raging_current_stacks, 2);
    assert_eq!(
        iteration.ability_totals("test:raging-tempest").damage,
        101.0
    );
    iteration.process_events_through(700);
    assert_eq!(
        iteration.ability_totals("test:raging-tempest").damage,
        152.0
    );
    iteration.process_events_through(1_200);
    assert_eq!(
        iteration.ability_totals("test:raging-tempest").damage,
        203.0
    );
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:raging-tempest").hits, 3);
}

#[test]
fn tariq_healthy_dummy_never_unlocks_culling_by_elapsed_time() {
    let profile = tariq_profile();
    let apl = apl([("culling-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 176);
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.process_events_through(ENCOUNTER_DURATION_MS - 1);
    assert!(iteration.can_cast(DpsAbilityKind::CullingStrike).is_none());
    iteration.hero.tariq_mut().executioners_grin_stacks = 1;
    iteration.hero.tariq_mut().executioners_grin_until = ENCOUNTER_DURATION_MS;
    assert!(iteration.can_cast(DpsAbilityKind::CullingStrike).is_some());
}

#[test]
fn tariq_skull_lightning_keeps_them_bones_and_sledgehammer_copies_only_physical_loss() {
    let mut profile = tariq_profile();
    profile.abilities[7].gcd_ms = 1;
    profile.abilities[7].first_hit_delay_ms = 100;
    profile.abilities[7]
        .mechanic_parameters
        .insert("lightningDelaySeconds".into(), 0.1);
    for (number, parameters) in [
        (4, vec![("cleaveDamageMultiplier", 0.5)]),
        (10, vec![("procChance", 1.0), ("criticalStrikeBonus", 1.0)]),
    ] {
        let mut talent = tariq_talent(2, parameters);
        talent.id = format!("ink-talent-id-talent{number}");
        profile.talents.push(talent);
    }
    let apl = apl([("skull-crusher", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 177);
    iteration.hero.tariq_mut().fury = 100.0;
    iteration.hero.tariq_mut().thunder_call_until = 50;
    iteration.cast(7);
    iteration.process_events_through(100);
    let primary = iteration.ability_totals("test:skull-crusher");
    assert_eq!(primary.hits, 1);
    assert_eq!(primary.crits, 1);
    let physical_damage = primary.damage;
    iteration.process_events_through(200);
    let lightning = iteration.ability_totals("test:skull-crusher");
    assert_eq!(lightning.hits, 2);
    assert_eq!(lightning.crits, 2);
    let before_cleave = lightning.damage;
    iteration.process_events_through(599);
    assert_eq!(iteration.ability_totals("test:skull-crusher").hits, 2);
    iteration.process_events_through(600);
    let total = iteration.ability_totals("test:skull-crusher");
    assert_eq!(total.hits, 4);
    assert_eq!(total.crits, 2);
    assert_eq!(total.damage - before_cleave, physical_damage);
}
