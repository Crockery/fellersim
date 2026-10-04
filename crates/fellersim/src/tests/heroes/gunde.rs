use super::super::*;

#[test]
fn gunde_contract_compiles_the_complete_current_kit() {
    let request = gunde_request(gunde_profile());

    validate(&request).expect("complete Gunde request validates");
    let compiled = CompiledProfile::try_from(&request).expect("Gunde profile compiles");

    assert_eq!(compiled.contract.hero, HeroIdentity::Gunde);
    assert_eq!(compiled.contract.model_version, GUNDE_MODEL_VERSION);
    assert_eq!(compiled.abilities.len(), 18);
    assert_eq!(compiled.source.max_secondary_resource, 5);
    assert!(compiled.source.abilities.is_empty());
    assert_eq!(
        ability_category(DpsAbilityKind::BloodboundSpirit),
        AbilityCategory::Spirit
    );
    assert_eq!(
        ability_category(DpsAbilityKind::ReignInBlood),
        AbilityCategory::Major
    );
    assert_eq!(
        ability_category(DpsAbilityKind::HeartSplitter),
        AbilityCategory::Core
    );
    assert_eq!(
        ability_category(DpsAbilityKind::Rupture),
        AbilityCategory::Power
    );
    assert_eq!(
        ability_category(DpsAbilityKind::Warbound),
        AbilityCategory::Other
    );
    assert!(!is_hunters_focus_commit(DpsAbilityKind::OwedInBlood));
    assert!(is_hunters_focus_commit(DpsAbilityKind::BloodArc));
    assert!(is_hunters_focus_commit(DpsAbilityKind::ReaversEdge));
    assert!(is_hunters_focus_commit(DpsAbilityKind::GrimCarve));
    assert!(!is_hunters_focus_commit(DpsAbilityKind::GundeAttack));
    assert!(!is_hunters_focus_commit(DpsAbilityKind::ButchersHook));
}

#[test]
fn gunde_contract_rejects_a_zero_time_off_gcd_attack() {
    let mut profile = gunde_profile();
    let attack = profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::GundeAttack)
        .expect("Gunde fixture includes Attack");
    attack.manually_castable = true;
    attack.off_gcd = true;
    attack.gcd_ms = 0;
    let request = gunde_request(profile);

    let error = validate(&request).expect_err("zero-time off-GCD actions must fail validation");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
}

#[test]
fn gunde_bloodbound_spirit_buffs_only_heart_splitter_and_grim_carve() {
    let profile = gunde_profile();
    let apl = apl([
        ("bloodbound-spirit", None),
        ("double-strike", None),
        ("heart-splitter", None),
        ("grim-carve", None),
    ]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.shared.spirit = profile.max_spirit;

    iteration.cast(3);
    iteration.cast(0);
    iteration.cast(5);
    iteration.cast(12);

    assert_eq!(
        iteration.ability_totals("test:bloodbound-spirit").damage,
        500.0
    );
    assert_eq!(iteration.ability_totals("test:double-strike").damage, 200.0);
    assert_eq!(
        iteration.ability_totals("test:heart-splitter").damage,
        120.0
    );
    assert_eq!(iteration.ability_totals("test:grim-carve").damage, 360.0);
}

#[test]
fn gunde_owed_in_blood_converts_feathers_and_slaughter_consumes_rend() {
    let profile = gunde_profile();
    let apl = apl([("owed-in-blood", None), ("slaughter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 40);
    iteration.hero.gunde_mut().blood_feathers = 3;
    iteration.hero.gunde_mut().blood_feathers_until = 45_000;

    iteration.cast(2);

    assert_eq!(iteration.hero.gunde().blood_feathers, 0);
    assert!((iteration.gunde_rend_remaining(0) - 1_200.0).abs() < 1e-10);
    let (rend_started_ms, rend_expires_ms) = iteration
        .common
        .dots
        .get(&(0, DotKind::Ability(DpsAbilityKind::Rend)))
        .map(|rend| (rend.started_ms, rend.expires_ms))
        .expect("Rend");

    iteration.cast(7);

    assert!(
        !iteration
            .common
            .dots
            .contains_key(&(0, DotKind::Ability(DpsAbilityKind::Rend)))
    );
    let slaughter = iteration
        .common
        .dots
        .get(&(0, DotKind::Ability(DpsAbilityKind::Slaughter)))
        .expect("Slaughter damage-transfer dot");
    assert!((slaughter.derived_damage_per_tick.unwrap_or_default() - 640.0).abs() < 1e-10);
    let recorded_uptime = iteration.test_uptime("dot:rend");
    let elapsed_uptime = iteration.common.now_ms.saturating_sub(rend_started_ms) as f64
        / ENCOUNTER_DURATION_MS as f64;
    let scheduled_uptime =
        rend_expires_ms.saturating_sub(rend_started_ms) as f64 / ENCOUNTER_DURATION_MS as f64;
    assert!(recorded_uptime > 0.0);
    assert!(recorded_uptime <= elapsed_uptime);
    assert!(recorded_uptime < scheduled_uptime);
}

#[test]
fn gunde_warbound_does_not_apply_rend_and_blood_feathers_expire() {
    let profile = gunde_profile();
    let apl = apl([("warbound", None), ("owed-in-blood", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 42);

    iteration.cast(1);
    assert_eq!(iteration.gunde_rend_remaining(0), 0.0);

    iteration.hero.gunde_mut().ground_feathers = 3;
    iteration.collect_gunde_feathers(3);
    assert_eq!(iteration.gunde_blood_feathers(), 3);
    iteration.common.now_ms = iteration.hero.gunde().blood_feathers_until;
    iteration.cast(2);

    assert_eq!(iteration.hero.gunde().blood_feathers, 0);
    assert_eq!(iteration.gunde_rend_remaining(0), 0.0);
}

#[test]
fn gunde_open_wounds_is_consumed_by_slaughter() {
    let profile = gunde_profile();
    let apl = apl([("rupture", None), ("slaughter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 43);

    iteration.cast(6);
    assert!(iteration.hero.gunde().open_wounds_until[0] > iteration.common.now_ms);
    iteration.cast(7);

    assert_eq!(iteration.hero.gunde().open_wounds_until[0], 0);
}

#[test]
fn gunde_carrion_onslaught_waits_for_blood_arc() {
    let mut profile = gunde_profile();
    profile.mechanics.push(gunde_legendary(
        27,
        [
            ("carrionDamageMultiplier", 1.2),
            ("carrionAdditionalDamagePerFeather", 0.02),
            ("carrionDurationSeconds", 12.0),
            ("carrionFeathersPerPulse", 10.0),
            ("carrionPulsePeriodSeconds", 2.0),
            ("carrionStacksPerFeather", 3.0),
        ],
    ));
    let apl = apl([("owed-in-blood", None), ("blood-arc", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 44);
    iteration.hero.gunde_mut().blood_feathers = 6;
    iteration.hero.gunde_mut().blood_feathers_until = 45_000;

    iteration.cast(2);
    assert_eq!(iteration.hero.gunde().blood_feathers, 0);
    assert_eq!(iteration.hero.gunde().ground_feathers, 0);
    assert_eq!(iteration.hero.gunde().carrion_pending_feathers, 2);

    iteration.cast(8);
    assert_eq!(iteration.hero.gunde().carrion_pending_feathers, 0);
    assert_eq!(iteration.gunde_blood_feathers(), 2);
    assert!((iteration.ability_totals("test:blood-arc").damage - 132.0).abs() < 1e-10);
}

#[test]
fn gunde_carnage_reduces_cooldowns_for_every_grim_carve_spin() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        15,
        [
            ("damageMultiplier", 1.25),
            ("cooldownReductionPerSpinSeconds", 1.0),
        ],
    ));
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::ReaversEdge)
        .expect("Gunde fixture includes Reaver's Edge")
        .cooldown_ms = 10_000;
    let apl = apl([("reavers-edge", None), ("grim-carve", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 45);

    iteration.cast(9);
    iteration.cast(12);

    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::ReaversEdge),
        5_000
    );
}

#[test]
fn gunde_deaths_arc_counts_only_successful_random_resets() {
    let mut success_profile = gunde_profile();
    success_profile.talents.push(gunde_talent(
        1,
        [("procChance", 1.0), ("durationSeconds", 8.0)],
    ));
    let apl = apl([("blood-arc", None)]);
    let mut success = Iteration::new(&success_profile, &apl, 1, 46);

    success.cast(8);

    assert_eq!(success.test_proc_count("gunde-talent-id-talent1"), 1);
    assert!(success.hero.gunde().deaths_arc_until > success.common.now_ms);

    let mut failure_profile = gunde_profile();
    failure_profile.talents.push(gunde_talent(
        1,
        [("procChance", 0.0), ("durationSeconds", 8.0)],
    ));
    let mut failure = Iteration::new(&failure_profile, &apl, 1, 46);

    failure.cast(8);

    assert_eq!(failure.test_proc_count("gunde-talent-id-talent1"), 0);
}

#[test]
fn gunde_legendaries_compile_into_typed_runtime_variants() {
    let mut profile = gunde_profile();
    profile.mechanics.extend([
        gunde_legendary(
            25,
            [
                ("heartSplitterCharges", 2.0),
                ("heartSplitterAdditionalStrikeChance", 0.25),
                ("heartSplitterAdditionalStrikeDelaySeconds", 0.4),
                ("heartSplitterHealthyCriticalStrikeBonus", 1.0),
            ],
        ),
        gunde_legendary(26, [("grimCarveAdditionalSpins", 2.0)]),
        gunde_legendary(
            27,
            [
                ("carrionDamageMultiplier", 1.2),
                ("carrionAdditionalDamagePerFeather", 0.1),
                ("carrionDurationSeconds", 6.0),
                ("carrionFeathersPerPulse", 1.0),
                ("carrionPulsePeriodSeconds", 2.0),
                ("carrionStacksPerFeather", 2.0),
            ],
        ),
    ]);

    let compiled =
        CompiledProfile::try_from(&gunde_request(profile)).expect("Gunde legendaries compile");
    let kinds = compiled
        .mechanics
        .iter()
        .map(|mechanic| mechanic.kind)
        .collect::<Vec<_>>();

    assert!(kinds.iter().any(|kind| matches!(
        kind,
        CompiledMechanicKind::HeroSource(HeroSourceKind::Legendary(
            LegendaryHeroSourceKind::GundeBleedingHeartsHeart
        ))
    )));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        CompiledMechanicKind::HeroSource(HeroSourceKind::Legendary(
            LegendaryHeroSourceKind::GundeBloodsoakedCleaver
        ))
    )));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        CompiledMechanicKind::HeroSource(HeroSourceKind::Legendary(
            LegendaryHeroSourceKind::GundeCarrionOnslaught
        ))
    )));
}

#[test]
fn gunde_refund_deduplicates_failed_and_successful_activation_attempts() {
    let mut profile = gunde_profile();
    profile.spirit = 0.0;
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 37);
    iteration.try_gunde_spirit_refund(DamageContext::for_cast(1));
    iteration.hero.gunde_mut().murder_of_crows_stacks = 2;
    iteration.hero.gunde_mut().murder_of_crows_until = 12_000;
    iteration.try_gunde_spirit_refund(DamageContext::for_cast(1));
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.gunde().murder_of_crows_stacks, 2);
    iteration.try_gunde_spirit_refund(DamageContext::for_cast(2));
    assert_eq!(iteration.test_proc_count("spirit-refund"), 1);
    assert_eq!(iteration.shared.spirit, 1.0);
    assert_eq!(iteration.hero.gunde().murder_of_crows_stacks, 1);
    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key(SPIRIT_PROC_RANDOM_STREAM_TAG)
    );
    for _ in 0..5 {
        iteration.try_gunde_spirit_refund(DamageContext::for_cast(2));
    }
    assert_eq!(iteration.test_proc_count("spirit-refund"), 1);
    assert_eq!(iteration.hero.gunde().ground_feathers, 5);
}

#[test]
fn gunde_murder_of_crows_starts_after_rupture_and_multihits_roll_once() {
    let mut profile = gunde_profile();
    profile.spirit = 0.0;
    profile.talents.push(gunde_talent(
        8,
        [("stacks", 2.0), ("durationSeconds", 12.0)],
    ));
    let apl = apl([("rupture", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 37);
    let rupture = iteration.common.abilities_by_kind[&DpsAbilityKind::Rupture];
    iteration.cast(rupture);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.gunde().murder_of_crows_stacks, 2);
    let double = iteration.common.abilities_by_kind[&DpsAbilityKind::DoubleStrike];
    iteration.cast(double);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 1);
    assert_eq!(iteration.hero.gunde().murder_of_crows_stacks, 1);
}

#[test]
fn bleeding_hearts_repeat_scales_delay_with_haste_and_recaptures_source_stats() {
    let mut profile = gunde_profile();
    profile.haste = 1.0;
    profile.mechanics.extend([
        ardeos_mechanic(
            "legendary-gunde-trait25",
            [
                ("powerMultiplier", 1.0),
                ("cooldownAccelerationMultiplier", 1.0),
                ("heartSplitterCharges", 2.0),
                ("heartSplitterAdditionalStrikeChance", 0.5),
                ("heartSplitterAdditionalStrikeDelaySeconds", 0.4),
                ("heartSplitterHealthyCriticalStrikeBonus", 1.0),
            ],
        ),
        ardeos_mechanic("test:repeat-expertise", [("expertiseBonus", 1.0)]),
    ]);
    let apl = apl([("heart-splitter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    let stream = "RandomStream.Gunde.Talent.HeavyMeleeDotBased.DoubleStrike";
    iteration.shared.controlled_random_states.insert(
        stream.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::HeartSplitter];
    let context = DamageContext::for_cast(1)
        .with_snapshot(iteration.capture_damage_source_snapshot(DpsAbilityKind::HeartSplitter));
    iteration.impact(index, true, 1.0, 0, context);
    let original_damage = iteration.ability_totals("test:heart-splitter").damage;
    let buff = iteration.test_mechanic_index("test:repeat-expertise");
    iteration.shared.dynamic_buffs[buff] = Some(DynamicBuffState {
        started_ms: 0,
        until_ms: 1_000,
        stacks: 1,
        value: 0.0,
    });
    iteration.process_events_through(199);
    assert_eq!(iteration.ability_totals("test:heart-splitter").hits, 1);
    iteration.shared.controlled_random_states.insert(
        stream.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    iteration.process_events_through(200);
    assert_eq!(iteration.ability_totals("test:heart-splitter").hits, 2);
    assert_eq!(
        iteration.ability_totals("test:heart-splitter").damage,
        original_damage * 3.0
    );
    assert!(iteration.shared.controlled_random_states[stream].failure_threshold > 0.0);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.ability_totals("test:heart-splitter").hits, 2);
}

fn carrion_profile() -> NormalizedDpsProfile {
    let mut profile = gunde_profile();
    profile.mechanics.push(gunde_legendary(
        27,
        [
            ("carrionDamageMultiplier", 1.2),
            ("carrionAdditionalDamagePerFeather", 0.02),
            ("carrionDurationSeconds", 12.0),
            ("carrionFeathersPerPulse", 10.0),
            ("carrionPulsePeriodSeconds", 2.0),
            ("carrionStacksPerFeather", 3.0),
        ],
    ));
    profile
}

#[test]
fn carrion_rounds_consumed_stacks_with_a_one_orb_minimum() {
    for (consumed, expected) in [(1, 1), (4, 1), (5, 2), (7, 2), (8, 3)] {
        let profile = carrion_profile();
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 304);
        iteration.hero.gunde_mut().blood_feathers = consumed;
        iteration.hero.gunde_mut().blood_feathers_until = 45_000;
        iteration.cast(2);
        assert_eq!(
            iteration.hero.gunde().carrion_pending_feathers,
            expected,
            "{consumed} consumed"
        );
    }
}

#[test]
fn carrion_replaces_emitter_on_empowered_blood_arc_not_on_a_new_proc() {
    let mut profile = carrion_profile();
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::Rend)
        .unwrap()
        .mechanic_parameters
        .insert("featherActivationDelaySeconds".into(), 0.0);
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::Rend)
        .unwrap()
        .mechanic_parameters
        .insert("maximumGroundFeathers".into(), 100.0);
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::OwedInBlood)
        .unwrap()
        .mechanic_parameters
        .insert("maximumBloodFeathers".into(), 100.0);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 305);
    iteration.hero.gunde_mut().carrion_pending_feathers = 25;
    let blood_arc = iteration.ability(DpsAbilityKind::BloodArc).unwrap().clone();
    iteration.commit_gunde_ability(&blood_arc);
    iteration.process_events_through(0);
    assert_eq!(iteration.gunde_blood_feathers(), 10);
    let generation = iteration.hero.gunde().carrion_generation;
    iteration.cast(2);
    assert_eq!(iteration.hero.gunde().carrion_generation, generation);
    iteration.process_events_through(2_000);
    assert_eq!(
        iteration.gunde_blood_feathers(),
        10,
        "the old emitter survived Owed consuming the first ten"
    );
    iteration.cast(8);
    assert_ne!(iteration.hero.gunde().carrion_generation, generation);
    assert_eq!(
        iteration.gunde_blood_feathers(),
        13,
        "replacement immediately emits round(10/3)"
    );
    iteration.process_events_through(4_000);
    assert_eq!(
        iteration.gunde_blood_feathers(),
        13,
        "cancelled emitter cannot emit its last five"
    );
}

#[test]
fn carrion_expired_proc_cannot_start_an_emitter_or_amplify_blood_arc() {
    let mut profile = carrion_profile();
    profile.haste = 1.0;
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 306);
    iteration.hero.gunde_mut().blood_feathers = 6;
    iteration.hero.gunde_mut().blood_feathers_until = 45_000;
    iteration.cast(2);
    assert_eq!(
        iteration.hero.gunde().carrion_onslaught_until,
        12_000,
        "haste does not shorten the stored proc"
    );
    iteration.common.now_ms = iteration.hero.gunde().carrion_onslaught_until;
    iteration.cast(8);
    assert_eq!(iteration.gunde_blood_feathers(), 0);
    assert_eq!(iteration.ability_totals("test:blood-arc").damage, 100.0);
}

#[test]
fn gunde_rend_refresh_preserves_old_tick_amounts_and_phase() {
    let profile = gunde_profile();
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.apply_gunde_rend(0, 100.0, DamageContext::NONE);
    iteration.process_events_through(3_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 10.0);
    iteration.common.now_ms = 3_500;
    iteration.apply_gunde_rend(0, 100.0, DamageContext::NONE);
    assert_eq!(iteration.gunde_rend_remaining(0), 190.0);
    iteration.process_events_through(6_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 30.0);
    iteration.process_events_through(30_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 190.0);
    iteration.process_events_through(33_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 200.0);
    assert_eq!(iteration.gunde_rend_remaining(0), 0.0);
}

#[test]
fn gunde_deep_rend_scales_execution_without_inflating_stored_damage() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        12,
        [
            ("tickRateMultiplier", 1.1),
            ("extraFeathersChance", 0.0),
            ("extraFeathersAmount", 3.0),
        ],
    ));
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.apply_gunde_rend(0, 1_100.0, DamageContext::NONE);
    assert_eq!(iteration.gunde_rend_remaining(0), 1_100.0);
    iteration.process_events_through(2_726);
    assert_eq!(iteration.ability_totals("test:rend").damage, 0.0);
    iteration.process_events_through(2_727);
    assert!((iteration.ability_totals("test:rend").damage - 110.0).abs() < 1e-9);
    assert_eq!(iteration.gunde_rend_remaining(0), 1_000.0);
    iteration.process_events_through(30_000);
    assert!((iteration.ability_totals("test:rend").damage - 1_210.0).abs() < 1e-9);
}

#[test]
fn gunde_rend_terminal_signal_consumes_a_whole_bucket() {
    let mut profile = gunde_profile();
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::Rend)
        .unwrap()
        .dot
        .as_mut()
        .unwrap()
        .duration_ms = 10_500;
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.apply_gunde_rend(0, 400.0, DamageContext::NONE);
    iteration.process_events_through(9_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 300.0);
    iteration.process_events_through(10_500);
    assert_eq!(iteration.ability_totals("test:rend").damage, 400.0);
    assert_eq!(iteration.gunde_rend_remaining(0), 0.0);
}

#[test]
fn gunde_heart_splitter_samples_rend_after_its_own_direct_hit() {
    let profile = gunde_profile();
    let apl = apl([("heart-splitter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::HeartSplitter];
    iteration.impact(index, true, 1.0, 0, DamageContext::NONE);
    assert_eq!(
        iteration.ability_totals("test:heart-splitter").damage,
        100.0
    );
    assert_eq!(iteration.gunde_rend_remaining(0), 100.0);
    assert_eq!(iteration.ability_totals("test:exsanguinate").damage, 30.0);
}

#[test]
fn gunde_massacre_uses_consumed_rend_in_strength_units_and_additive_spirit() {
    let mut profile = gunde_profile();
    profile.spirit = 0.2;
    profile.talents.push(gunde_talent(
        9,
        [
            ("spiritPerStack", 0.0025),
            ("maximumStacks", 100.0),
            ("durationSeconds", 8.0),
        ],
    ));
    let apl = apl([("slaughter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 41);
    iteration.apply_gunde_rend(0, 2_500.0, DamageContext::NONE);
    iteration.apply_gunde_rend(1, 150.0, DamageContext::NONE);
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::Slaughter];
    iteration.impact(index, false, 1.0, 0, DamageContext::NONE);
    iteration.impact(index, false, 1.0, 1, DamageContext::NONE);
    assert_eq!(iteration.hero.gunde().massacre_stacks, 27);
    assert!((iteration.effective_spirit() - 0.2675).abs() < 1e-9);
    iteration.common.now_ms = 8_000;
    assert!((iteration.effective_spirit() - 0.2).abs() < 1e-9);
}

#[test]
fn gunde_open_wounds_does_not_bypass_its_ungranted_brimborn_requirement() {
    let profile = gunde_profile();
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.apply_gunde_rend(0, 400.0, DamageContext::NONE);
    iteration.hero.gunde_mut().open_wounds_until[0] = 5_000;
    iteration.process_events_through(3_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 40.0);
    assert_eq!(iteration.gunde_rend_remaining(0), 360.0);
    iteration.process_events_through(6_000);
    assert_eq!(iteration.ability_totals("test:rend").damage, 80.0);
}

#[test]
fn gunde_slaughter_converts_deep_rend_once_and_scales_its_terminal_tick() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        12,
        [
            ("tickRateMultiplier", 1.1),
            ("extraFeathersChance", 0.0),
            ("extraFeathersAmount", 3.0),
        ],
    ));
    let slaughter = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::Slaughter)
        .unwrap();
    slaughter.dot.as_mut().unwrap().duration_ms = 3_500;
    let apl = apl([("slaughter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 41);
    iteration.apply_gunde_rend(0, 1_100.0, DamageContext::NONE);
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::Slaughter];
    iteration.impact(index, false, 1.0, 0, DamageContext::NONE);
    iteration.process_events_through(3_500);
    assert!((iteration.ability_totals("test:slaughter").damage - 1_936.0).abs() < 1e-9);
    assert_eq!(iteration.gunde_rend_remaining(0), 0.0);
}

#[test]
fn gunde_attack_starts_from_selected_melee_attacks_and_uses_its_own_clock() {
    let mut profile = gunde_profile();
    for ability in &mut profile.abilities {
        ability.gcd_ms = 0;
    }
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 13);
    iteration.process_events_through(1_000);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::BloodArc]);
    assert!(!iteration.hero.gunde().auto_attacking);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 0);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::DoubleStrike]);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 1);
    iteration.process_events_through(2_499);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 1);
    iteration.process_events_through(2_500);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 2);
}

#[test]
fn gunde_attack_waits_without_accumulating_overdue_swings() {
    let profile = gunde_profile();
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 14);
    iteration.hero.gunde_mut().auto_attacking = true;
    iteration.process_events_through(0);
    iteration.hero.gunde_mut().auto_blocked_until = 10_000;
    iteration.process_events_through(10_000);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 2);
    iteration.process_events_through(11_499);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 2);
    iteration.process_events_through(11_500);
    assert_eq!(iteration.ability_totals("test:gunde-attack").casts, 3);
}

#[test]
fn gunde_harvesters_toll_buffs_ruptures_own_hit_and_survives_in_a_created_spec() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        4,
        [("damageMultiplier", 2.0), ("durationSeconds", 0.5)],
    ));
    let rupture = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::Rupture)
        .unwrap();
    rupture.gcd_ms = 0;
    let grim = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::GrimCarve)
        .unwrap();
    grim.gcd_ms = 0;
    grim.first_hit_delay_ms = 1_000;
    grim.direct_hits = 1;
    let apl = apl([("rupture", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 15);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::Rupture]);
    assert_eq!(iteration.ability_totals("test:rupture").damage, 200.0);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::GrimCarve]);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.ability_totals("test:grim-carve").damage, 200.0);
    assert_eq!(
        iteration.hero_damage_scale(Some(DpsAbilityKind::GrimCarve)),
        1.0
    );
}

#[test]
fn gunde_crimson_strikes_is_granted_after_blood_arc_and_consumed_by_the_first_spec() {
    let mut profile = gunde_profile();
    profile
        .talents
        .push(gunde_talent(5, [("damageMultiplier", 2.0)]));
    let apl = apl([("blood-arc", None), ("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 16);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::BloodArc]);
    assert_eq!(iteration.ability_totals("test:blood-arc").damage, 100.0);
    assert!(iteration.hero.gunde().crimson_strikes_until > iteration.common.now_ms);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::DoubleStrike]);
    assert_eq!(iteration.ability_totals("test:double-strike").damage, 300.0);
    assert_eq!(iteration.hero.gunde().crimson_strikes_until, 0);
}

#[test]
fn gunde_deep_rend_extra_feathers_replace_the_normal_roll_only_for_one_rend() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        12,
        [
            ("tickRateMultiplier", 1.1),
            ("extraFeathersChance", 1.0),
            ("extraFeathersAmount", 3.0),
        ],
    ));
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::Rend)
        .unwrap()
        .mechanic_parameters
        .insert("featherChance2".into(), 0.5);
    let apl = apl([("double-strike", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 17);
    iteration.apply_gunde_rend(0, 100.0, DamageContext::NONE);
    iteration.roll_gunde_rend_feathers();
    assert_eq!(iteration.hero.gunde().ground_feathers, 3);
    assert!(
        !iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Gunde.DotTransfer.OrbDropper.PeriodSpawnChance")
    );
    iteration.apply_gunde_rend(1, 100.0, DamageContext::NONE);
    iteration.roll_gunde_rend_feathers();
    assert_eq!(iteration.test_proc_count("gunde-talent-id-talent12"), 1);
    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Gunde.DotTransfer.OrbDropper.PeriodSpawnChance")
    );
}

#[test]
fn gunde_carnage_counts_separate_spins_once_and_keeps_the_raw_actor_period() {
    let mut profile = gunde_profile();
    profile.haste = 1.0;
    profile.talents.push(gunde_talent(
        15,
        [
            ("damageMultiplier", 1.0),
            ("cooldownReductionPerSpinSeconds", 1.0),
        ],
    ));
    for ability in &mut profile.abilities {
        ability.gcd_ms = 0;
    }
    let edge = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::ReaversEdge)
        .unwrap();
    edge.cooldown_ms = 20_000;
    edge.cooldown_scales_with_haste = false;
    let grim = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::GrimCarve)
        .unwrap();
    grim.scale_time_with_haste = true;
    grim.first_hit_delay_ms = 200; // Fixed flight time.
    grim.hit_interval_ms = 500;
    grim.mechanic_parameters
        .insert("projectileSpawnDelaySeconds".into(), 0.14);
    let apl = apl([("grim-carve", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 18);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::ReaversEdge]);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::GrimCarve]);
    iteration.process_events_through(269);
    assert_eq!(iteration.ability_totals("test:grim-carve").hits, 0);
    iteration.process_events_through(270);
    assert_eq!(iteration.ability_totals("test:grim-carve").hits, 3);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::ReaversEdge),
        18_730
    );
    iteration.process_events_through(769);
    assert_eq!(iteration.ability_totals("test:grim-carve").hits, 3);
    iteration.process_events_through(1_270);
    assert_eq!(iteration.ability_totals("test:grim-carve").hits, 9);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::ReaversEdge),
        15_730
    );
}

#[test]
fn gunde_deaths_arc_reset_occurs_after_the_triggering_hit() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        1,
        [
            ("procChance", 1.0),
            ("durationSeconds", 8.0),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let arc = profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::BloodArc)
        .unwrap();
    arc.gcd_ms = 0;
    arc.cooldown_ms = 10_000;
    let apl = apl([("blood-arc", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 19);
    iteration.cast(iteration.common.abilities_by_kind[&DpsAbilityKind::BloodArc]);
    assert_eq!(iteration.hero.gunde().deaths_arc_until, 0);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::BloodArc),
        10_000
    );
    assert_eq!(iteration.ability_totals("test:blood-arc").damage, 100.0);
    iteration.process_events_through(1);
    assert_eq!(iteration.cooldown_remaining_ms(DpsAbilityKind::BloodArc), 0);
    assert_eq!(iteration.hero.gunde().deaths_arc_until, 8_001);
}

#[test]
fn gunde_damage_scale_adds_toll_and_carnage_before_compound_buffs() {
    let mut profile = gunde_profile();
    profile.talents.push(gunde_talent(
        4,
        [("damageMultiplier", 1.1), ("durationSeconds", 8.0)],
    ));
    profile.talents.push(gunde_talent(
        15,
        [
            ("damageMultiplier", 1.25),
            ("cooldownReductionPerSpinSeconds", 1.0),
        ],
    ));
    let apl = apl([("grim-carve", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 20);
    iteration.hero.gunde_mut().harvesters_toll_until = 1_000;
    iteration.hero.gunde_mut().bloodbound_spirit_until = 500;
    // The fixture's Bloodbound multiplier is 1.2.
    assert!(
        (iteration.hero_damage_scale(Some(DpsAbilityKind::GrimCarve)) - 1.35 * 1.2).abs() < 1e-10
    );
    iteration.process_events_through(500);
    assert!((iteration.hero_damage_scale(Some(DpsAbilityKind::GrimCarve)) - 1.35).abs() < 1e-10);
    assert!((iteration.hero_damage_scale(Some(DpsAbilityKind::HeartSplitter)) - 1.1).abs() < 1e-10);
}

#[test]
fn gunde_oathshatter_excludes_its_trigger_target_and_uses_secondary_count() {
    let mut profile = gunde_profile();
    profile.critical_strike = 1.0;
    profile.talents.push(gunde_talent(
        14,
        [
            ("explosionDamageFraction", 0.5),
            ("targetCountDamageScalingThreshold", 8.0),
        ],
    ));
    let apl = apl([("heart-splitter", None)]);
    for targets in [1, 9, 17] {
        let mut iteration = Iteration::new(&profile, &apl, targets, 41);
        let index = iteration.common.abilities_by_kind[&DpsAbilityKind::HeartSplitter];
        iteration.impact(index, true, 1.0, targets - 1, DamageContext::NONE);
        let secondary_count = targets - 1;
        let expected = if secondary_count == 0 {
            0.0
        } else {
            f64::from(secondary_count)
                * (30.0 * multi_target_damage_falloff(secondary_count, 8.0)).round()
        };
        assert_eq!(
            iteration.ability_totals("test:oathshatter").damage,
            expected
        );
        assert_eq!(
            iteration.ability_totals("test:oathshatter").hits,
            u64::from(secondary_count)
        );
    }
}

#[test]
fn default_gunde_spends_affordable_spirit_without_waiting_for_reign() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/apl-builds/gunde/character.json")).unwrap();
    let mut build: crate::preparation::CharacterBuild =
        serde_json::from_value(document["build"].clone()).unwrap();
    build
        .positions
        .iter_mut()
        .find(|p| p.position_id == "weapon")
        .unwrap()
        .item = None;
    let (profile, _) = crate::preparation::prepare_character(&build, 1).unwrap();
    let apl = crate::parse_apl(&shipped_apl_source("gunde")).unwrap();
    for cost in [95.0, 100.0] {
        let mut profile = profile.clone();
        profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == DpsAbilityKind::BloodboundSpirit)
            .unwrap()
            .spirit_cost = cost;
        let mut it = Iteration::new(&profile, &apl, 1, 231);
        for kind in [
            DpsAbilityKind::Rupture,
            DpsAbilityKind::BloodArc,
            DpsAbilityKind::ReignInBlood,
        ] {
            it.common.cooldowns.insert(
                kind,
                CooldownState {
                    remaining_ms: 30_000.0,
                    used_charges: 1,
                },
            );
        }
        it.hero.gunde_mut().serrated_edge_until = 10_000;
        let spirit = it.common.abilities_by_kind[&DpsAbilityKind::BloodboundSpirit];
        let reign = it.common.abilities_by_kind[&DpsAbilityKind::ReignInBlood];
        it.shared.spirit = cost - 0.01;
        assert_ne!(it.choose_action(), Some(spirit));
        it.shared.spirit = cost;
        assert!(cost < profile.max_spirit);
        assert_eq!(it.choose_action(), Some(spirit));
        it.hero.gunde_mut().bloodbound_spirit_until = 1_000;
        assert_ne!(it.choose_action(), Some(spirit));
        it.common.now_ms = 1_000;
        assert_eq!(it.choose_action(), Some(spirit));
        it.shared.spirit = 0.0;
        it.common.cooldowns.remove(&DpsAbilityKind::ReignInBlood);
        assert_eq!(it.choose_action(), Some(reign));
    }
}

#[test]
fn default_gunde_carrion_setup_only_waits_for_selected_talent_buffs() {
    let apl = crate::parse_apl(&shipped_apl_source("gunde")).unwrap();
    for document in [
        include_str!("../fixtures/apl-builds/gunde/variants/wrists-bloodcraze.json"),
        include_str!("../fixtures/apl-builds/gunde/variants/bloodcraze-no-deaths-arc.json"),
        include_str!("../fixtures/apl-builds/gunde/variants/bloodcraze-no-toll.json"),
    ] {
        let document: serde_json::Value = serde_json::from_str(document).unwrap();
        let mut build: crate::preparation::CharacterBuild =
            serde_json::from_value(document["build"].clone()).unwrap();
        build
            .positions
            .iter_mut()
            .find(|p| p.position_id == "weapon")
            .unwrap()
            .item = None;
        let has_arc = build
            .selected_talent_ids
            .iter()
            .any(|t| t == "gunde-talent-id-talent1");
        let has_toll = build
            .selected_talent_ids
            .iter()
            .any(|t| t == "gunde-talent-id-talent4");
        let (profile, _) = crate::preparation::prepare_character(&build, 3).unwrap();
        let mut it = Iteration::new(&profile, &apl, 3, 232);
        for kind in [DpsAbilityKind::Rupture, DpsAbilityKind::BloodArc] {
            it.common.cooldowns.insert(
                kind,
                CooldownState {
                    remaining_ms: 30_000.0,
                    used_charges: 1,
                },
            );
        }
        it.shared.spirit = profile.max_spirit;
        it.hero.gunde_mut().serrated_edge_until = 30_000;
        let spirit = it.common.abilities_by_kind[&DpsAbilityKind::BloodboundSpirit];
        assert_ne!(it.choose_action(), Some(spirit));
        if has_arc {
            it.hero.gunde_mut().deaths_arc_until = 10_000;
        }
        if has_toll {
            it.hero.gunde_mut().harvesters_toll_until = 10_000;
        }
        assert_eq!(it.choose_action(), Some(spirit));
    }
}
