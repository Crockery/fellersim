use super::super::*;

fn frostweaver_profile() -> NormalizedDpsProfile {
    let mut profile = rime_profile();
    profile.talents.push(rime_talent(
        18,
        [
            ("procChance", 1.0),
            ("durationSeconds", 12.0),
            ("criticalStrikeBonus", 1.0),
            ("maximumStacks", 2.0),
        ],
    ));
    profile
}

#[test]
fn frostweaver_caps_charges_and_does_not_proc_on_overflow() {
    let profile = frostweaver_profile();
    let apl = apl([("frost-bolt", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.add_rime_orbs(3);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 2);
    iteration.common.now_ms = 12_000;
    iteration.hero.rime_mut().winter_orbs = profile.max_secondary_resource;
    iteration.add_rime_orbs(1);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 0);
    iteration.hero.rime_mut().winter_orbs -= 1;
    iteration.add_rime_orbs(1);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 1);
}

#[test]
fn frostweaver_rolls_once_for_an_aggregate_orb_refund() {
    let profile = frostweaver_profile();
    let apl = apl([("frost-bolt", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.handle_rime_event(RimeEvent::OrbRefund { orbs: 3 });
    assert_eq!(iteration.hero.rime().winter_orbs, 3);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 1);
}

#[test]
fn frostweaver_is_checked_at_impact_and_shared_by_the_entire_comet_pulse() {
    let profile = frostweaver_profile();
    let apl = apl([("ice-comet", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    // Both actors launch before the proc exists; only the first pulse gets it.
    for delay in [500, 600] {
        iteration.schedule_rime_spender_pulse(
            RimeTriggeredDamageSource::IceComet,
            1.0,
            0.0,
            3,
            DamageContext::for_cast(delay),
            delay,
        );
    }
    iteration.add_rime_orbs(1);
    iteration.process_events_through(500);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 0);
    assert_eq!(iteration.ability_totals("test:ice-comet").crits, 3);
    iteration.process_events_through(600);
    assert_eq!(iteration.ability_totals("test:ice-comet").hits, 6);
    assert_eq!(iteration.ability_totals("test:ice-comet").crits, 3);
}

#[test]
fn frostweaver_expiration_during_flight_does_not_empower_a_later_hit() {
    let profile = frostweaver_profile();
    let apl = apl([("glacial-blast", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.add_rime_orbs(1);
    iteration.common.now_ms = 11_900;
    iteration.schedule_rime_spender_pulse(
        RimeTriggeredDamageSource::TalonStrike,
        1.0,
        0.0,
        1,
        DamageContext::for_cast(1),
        200,
    );
    iteration.process_events_through(12_100);
    assert_eq!(iteration.ability_totals("talent:talon-strike").hits, 1);
    assert_eq!(iteration.ability_totals("talent:talon-strike").crits, 0);
}

#[test]
fn frost_swallow_recaptures_attributes_after_launch() {
    let mut profile = rime_profile();
    profile.mechanics.push(ardeos_mechanic(
        "test:live-expertise",
        [("expertiseBonus", 1.0)],
    ));
    let apl = apl([("frost-bolt", None)]);
    let mut baseline = Iteration::new(&profile, &apl, 1, 1);
    baseline.schedule_rime_projectile(
        RimeTriggeredDamageSource::FrostSwallow,
        1.0,
        0.0,
        0,
        DamageContext::for_cast(1),
        500,
    );
    baseline.process_events_through(500);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.schedule_rime_projectile(
        RimeTriggeredDamageSource::FrostSwallow,
        1.0,
        0.0,
        0,
        DamageContext::for_cast(1),
        500,
    );
    let index = iteration.test_mechanic_index("test:live-expertise");
    iteration.shared.dynamic_buffs[index] = Some(DynamicBuffState {
        started_ms: 100,
        until_ms: 1_000,
        stacks: 1,
        value: 0.0,
    });
    iteration.process_events_through(500);
    assert_eq!(
        iteration
            .ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
            .damage,
        baseline
            .ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
            .damage
            * 2.0
    );
}

#[test]
fn glacial_blast_uses_a_frostweaver_charge_gained_during_flight() {
    let profile = frostweaver_profile();
    let apl = apl([("glacial-blast", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().winter_orbs = 2;
    let glacial = iteration.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    iteration.cast(glacial);
    assert_eq!(iteration.ability_totals("test:glacial-blast").hits, 0);
    iteration.add_rime_orbs(1);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:glacial-blast").crits, 1);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 0);
}

#[test]
fn paid_ice_comet_shares_frostweaver_bonus_across_its_target_batch() {
    let profile = frostweaver_profile();
    let apl = apl([("ice-comet", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    iteration.hero.rime_mut().winter_orbs = 2;
    iteration.add_rime_orbs(1);
    let comet = iteration.common.abilities_by_kind[&DpsAbilityKind::IceComet];
    iteration.cast(comet);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:ice-comet").crits, 3);
    assert_eq!(iteration.buff_stacks(AplBuff::FrostweaversWrath), 0);
}

#[test]
fn philosopher_tracks_live_spirit_during_its_existing_buff() {
    let mut profile = rime_profile();
    profile.spirit = 0.4;
    profile.haste = 0.0;
    let cooldown = profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::IceBlitz)
        .unwrap();
    cooldown.cooldown_ms = 10_000;
    cooldown.cooldown_scales_with_haste = true;
    cooldown.cooldown_scales_with_cooldown_recovery = false;
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::WintersBlessing)
        .unwrap()
        .mechanic_parameters
        .insert("spiritMultiplier".into(), 0.2);
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.08",
        [
            ("durationSeconds", 8.0),
            ("statIncreasePerSpirit", 0.002),
            ("spiritPerIncrease", 4.0),
            ("spiritCap", 0.5),
        ],
    ));
    let apl = apl([("cold-snap", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 11);
    let major = compiled_ability(&rime_ability(DpsAbilityKind::IceBlitz));
    iteration.trigger_dynamic_on_cast(&major, DamageContext::NONE);
    assert!((iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")) - 0.02).abs() < 1e-12);

    iteration.hero.rime_mut().winters_blessing_until = 2_000;
    for parameter in [
        parameter_key!("hasteBonus"),
        parameter_key!("criticalStrikeBonus"),
        parameter_key!("expertiseBonus"),
    ] {
        assert!((iteration.dynamic_stat_bonus(parameter) - 0.024).abs() < 1e-12);
    }
    iteration.common.cooldowns.insert(
        DpsAbilityKind::IceBlitz,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    iteration.advance_to(4_000);
    // Two seconds at 1.024, then two at 1.020 after Winter's Blessing expires.
    assert!(
        (iteration.common.cooldowns[&DpsAbilityKind::IceBlitz].remaining_ms - 5_912.0).abs() < 1e-9
    );
    assert!((iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")) - 0.02).abs() < 1e-12);
    iteration.common.now_ms = 8_000;
    assert_eq!(
        iteration.dynamic_stat_bonus(parameter_key!("hasteBonus")),
        0.0
    );
}

#[test]
fn biting_cold_compounds_with_sinister_only_for_core_damage() {
    let mut profile = rime_profile();
    profile.critical_multiplier = 2.04;
    profile
        .talents
        .push(rime_talent(16, [("criticalPowerMultiplier", 1.1)]));
    profile.mechanics.push(ardeos_mechanic(
        "DynamicItemAbilityRank.05",
        [
            ("criticalStrikeBonus", 0.04),
            ("criticalPowerMultiplier", 1.2),
        ],
    ));
    let apl = apl([("cold-snap", None)]);
    let iteration = Iteration::new(&profile, &apl, 1, 2);
    assert!(
        (iteration.effective_critical_multiplier(Some(DpsAbilityKind::ColdSnap)) - 2.04 * 1.1)
            .abs()
            < 1e-12
    );
    assert!(
        (iteration.effective_critical_multiplier(Some(DpsAbilityKind::FreezingTorrent))
            - 2.04 * 1.1 * 1.2)
            .abs()
            < 1e-12
    );
}

#[test]
fn rime_contract_rejects_missing_parameters_and_non_positive_cadences() {
    let request = rime_request(rime_profile());
    validate(&request).expect("complete Rime profile must validate");

    let mut missing = request.clone();
    let bursting = missing
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::BurstingIce)
        .expect("Bursting Ice model");
    bursting.mechanic_parameters.remove("pulsePowerCoefficient");
    let bursting_id = bursting.id.clone();
    let error = validate(&missing).expect_err("missing pulse coefficient must fail");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec![bursting_id]);

    let mut negative_bursting = request.clone();
    let bursting = negative_bursting
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::BurstingIce)
        .expect("Bursting Ice model");
    bursting
        .mechanic_parameters
        .insert("pulsePeriodSeconds".into(), -0.5);
    let error = validate(&negative_bursting).expect_err("negative pulse period must fail");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);

    let mut zero_wrath = request;
    let wrath = zero_wrath
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::WrathOfWinter)
        .expect("Wrath of Winter model");
    wrath
        .mechanic_parameters
        .insert("volleyPeriodSeconds".into(), 0.0);
    let error = validate(&zero_wrath).expect_err("zero volley period must fail");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
}

#[test]
fn rime_contract_rejects_invalid_effect_durations_rates_and_counts() {
    for (kind, parameter) in [
        (DpsAbilityKind::FrostBolt, "serverTickRateCapHz"),
        (DpsAbilityKind::FlightOfTheNavir, "projectileCount"),
        (DpsAbilityKind::WrathOfWinter, "volleyProjectiles"),
    ] {
        let mut request = rime_request(rime_profile());
        let ability = request
            .profile
            .abilities
            .iter_mut()
            .find(|ability| ability.kind == kind)
            .expect("current Rime ability");
        ability.mechanic_parameters.insert(parameter.into(), 0.0);
        let ability_id = ability.id.clone();
        let error = validate(&request).expect_err("zero executable parameter must fail");
        assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
        assert_eq!(error.sources, vec![ability_id]);
    }

    let mut request = rime_request(rime_profile());
    let flight = request
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::FlightOfTheNavir)
        .expect("Flight of the Navir model");
    flight.effect_duration_ms = 0;
    let flight_id = flight.id.clone();
    let error = validate(&request).expect_err("zero effect duration must fail");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
    assert_eq!(error.sources, vec![flight_id]);

    let mut profile = rime_profile();
    profile.mechanics.push(rime_legendary(
        1,
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("startingSpirit", 50.0),
            ("wintersBlessingCharges", 2.0),
            ("undulatingSpiritDurationSeconds", 12.0),
        ],
    ));
    let request = rime_request(profile);
    validate(&request).expect("current Undulating Spirit parameters must validate");

    let mut missing_duration = request.clone();
    missing_duration.profile.mechanics[0]
        .parameters
        .remove("undulatingSpiritDurationSeconds");
    let error = validate(&missing_duration).expect_err("missing duration must fail");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);

    let mut zero_stacks = request;
    zero_stacks.profile.mechanics[0]
        .parameters
        .insert("wintersBlessingCharges".into(), 0.0);
    let error = validate(&zero_stacks).expect_err("zero stacks must fail");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
}

#[test]
fn rime_talent_contract_requires_the_current_mechanic_and_parameters() {
    let talent = DpsTalentModel {
        id: "rime-talent-id-talent17".into(),
        name: "Rime talent 17".into(),
        mechanic_id: "augmentation:fellowship-content-abilities-talents-rime-caa-rime-trait1"
            .into(),
        classification: MechanicClassification::Modeled,
        parameters: BTreeMap::from([
            ("singleTargetDamageMultiplier".into(), 0.4),
            ("multiTargetDamageMultiplier".into(), 0.4),
            ("singleTargetPulsePeriodSeconds".into(), 0.15),
            ("multiTargetPulsePeriodSeconds".into(), 0.15),
            ("singleTargetInitialDelaySeconds".into(), 0.35),
            ("multiTargetInitialDelaySeconds".into(), 1.0),
        ]),
        reason: None,
    };
    validate_talent(&talent, RIME_HERO_ID, 1).expect("current Icy Talons contract");

    let mut unknown_mechanic = talent.clone();
    unknown_mechanic.mechanic_id = "augmentation:test:unknown".into();
    let error = validate_talent(&unknown_mechanic, RIME_HERO_ID, 1)
        .expect_err("unknown mechanic must fail");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);

    let mut missing = talent.clone();
    missing.parameters.remove("singleTargetPulsePeriodSeconds");
    let error = validate_talent(&missing, RIME_HERO_ID, 1).expect_err("missing cadence must fail");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);

    let mut zero_cadence = talent;
    zero_cadence
        .parameters
        .insert("singleTargetPulsePeriodSeconds".into(), 0.0);
    let error =
        validate_talent(&zero_cadence, RIME_HERO_ID, 1).expect_err("zero cadence must fail");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
}

#[test]
fn rime_anima_rollover_and_delayed_orb_refunds_preserve_resources() {
    let profile = rime_profile();
    let apl = apl([("frost-bolt", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().anima = 8.0;
    iteration.hero.rime_mut().winter_orbs = 2;

    iteration.add_rime_anima(10.0, DamageContext::for_cast(1));

    assert_eq!(iteration.hero.rime().anima, 0.0);
    assert_eq!(iteration.hero.rime().winter_orbs, 4);

    iteration.hero.rime_mut().winter_orbs = 0;
    iteration.shared.spirit = 0.0;
    iteration.hero.rime_mut().undulating_spirit_stacks = 1;
    iteration.hero.rime_mut().undulating_spirit_until = u64::MAX;
    iteration.try_rime_spirit_refund(2);
    assert_eq!(iteration.shared.spirit, 2.0);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.rime().winter_orbs, 0);

    iteration.process_events_through(199);
    assert_eq!(iteration.hero.rime().winter_orbs, 0);
    iteration.process_events_through(200);
    assert_eq!(iteration.hero.rime().winter_orbs, 2);
}

#[test]
fn undulating_spirit_grants_two_timed_refunds_and_starting_spirit() {
    let mut profile = rime_profile();
    profile.spirit = 0.0;
    profile.mechanics.push(rime_legendary(
        1,
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("startingSpirit", 50.0),
            ("wintersBlessingCharges", 2.0),
            ("undulatingSpiritDurationSeconds", 12.0),
        ],
    ));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.IncreasedMainStatAndSpiritRating",
        [
            ("durationSeconds", 4.0),
            ("powerMultiplier", 1.048),
            ("spiritRating", 23.0),
        ],
    ));
    let apl = apl([("frost-bolt", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);

    assert_eq!(iteration.shared.spirit, 50.0);
    iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
    assert_eq!(iteration.hero.rime().undulating_spirit_stacks, 2);
    assert_eq!(iteration.hero.rime().undulating_spirit_until, 12_000);

    iteration.shared.spirit = 0.0;
    iteration.try_rime_spirit_refund(1);
    assert_eq!(iteration.shared.spirit, 2.0);
    assert_eq!(iteration.effective_power_multiplier(), 1.048);
    assert!(
        !iteration
            .shared
            .controlled_random_states
            .contains_key(SPIRIT_PROC_RANDOM_STREAM_TAG)
    );

    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.rime().undulating_spirit_stacks, 1);
    iteration.process_events_through(200);
    assert_eq!(iteration.hero.rime().winter_orbs, 1);

    iteration.try_rime_spirit_refund(1);
    assert_eq!(iteration.shared.spirit, 4.0);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.rime().undulating_spirit_stacks, 0);
    assert_eq!(iteration.hero.rime().undulating_spirit_until, 0);
    iteration.process_events_through(400);
    assert_eq!(iteration.hero.rime().winter_orbs, 2);

    iteration.apply_rime_ability_buff(DpsAbilityKind::WintersBlessing);
    assert_eq!(iteration.hero.rime().undulating_spirit_stacks, 2);
    assert_eq!(iteration.hero.rime().undulating_spirit_until, 12_400);
    iteration.process_events_through(12_400);
    iteration.try_rime_spirit_refund(1);
    assert_eq!(iteration.shared.spirit, 4.0);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    assert_eq!(iteration.hero.rime().undulating_spirit_stacks, 0);
    assert_eq!(iteration.hero.rime().undulating_spirit_until, 0);
}

#[test]
fn rime_spender_talents_apply_and_consume_their_cast_local_state() {
    let mut assault_profile = rime_profile();
    assault_profile.talents.push(rime_talent(
        1,
        [
            ("maximumStacks", 2.0),
            ("damageMultiplier", 1.4),
            ("explosionDamageFraction", 0.3),
            ("explosionRadius", 1_000.0),
            ("targetCountDamageScalingThreshold", 3.0),
        ],
    ));
    let assault_apl = apl([("glacial-blast", None)]);
    let mut assault = Iteration::new(&assault_profile, &assault_apl, 3, 1);
    let cold_snap = assault.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    for cast_id in 1..=2 {
        assault.commit_ability(cold_snap, &mut DamageContext::for_cast(cast_id), false, 0.0);
    }
    assert_eq!(assault.hero.rime().glacial_assault_stacks, 2);
    assert_eq!(assault.buff_stacks(AplBuff::GlacialAssault), 2);

    let glacial = assault.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    assault.cast(glacial);
    assault.process_events_through(2_000);
    assert_eq!(assault.hero.rime().glacial_assault_stacks, 0);
    assert_eq!(assault.buff_stacks(AplBuff::GlacialAssault), 0);
    assert!((assault.ability_totals("talent:glacial-assault").damage - 84.0).abs() < 1e-9);

    let mut icy_flow_profile = rime_profile();
    icy_flow_profile.talents.push(rime_talent(
        5,
        [
            ("durationSeconds", 15.0),
            ("maximumStacks", 2.0),
            ("castHaste", 0.5),
            ("cometInitialDelaySeconds", 0.5),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let icy_flow_apl = apl([("glacial-blast", None)]);
    let mut icy_flow = Iteration::new(&icy_flow_profile, &icy_flow_apl, 1, 2);
    let cold_snap = icy_flow.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    icy_flow.commit_ability(cold_snap, &mut DamageContext::for_cast(1), false, 0.0);
    icy_flow.hero.rime_mut().winter_orbs = 2;
    let glacial = icy_flow.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    icy_flow.cast(glacial);
    icy_flow.process_events_through(2_000);
    assert_eq!(icy_flow.hero.rime().icy_flow_stacks, 0);
    assert_eq!(icy_flow.ability_totals("test:glacial-blast").crits, 1);

    let mut talons_profile = rime_profile();
    talons_profile.talents.push(rime_talent(
        17,
        [
            ("singleTargetDamageMultiplier", 0.4),
            ("multiTargetDamageMultiplier", 0.4),
            ("singleTargetPulsePeriodSeconds", 0.15),
            ("multiTargetPulsePeriodSeconds", 0.15),
            ("singleTargetInitialDelaySeconds", 0.35),
            ("multiTargetInitialDelaySeconds", 1.0),
        ],
    ));
    let talons_apl = apl([("glacial-blast", None)]);
    let mut talons = Iteration::new(&talons_profile, &talons_apl, 1, 3);
    talons.hero.rime_mut().winter_orbs = 3;
    let glacial = talons.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    talons.cast(glacial);
    assert_eq!(talons.hero.rime().winter_orbs, 0);
    assert_eq!(talons.ability_totals("talent:talon-strike").hits, 3);
    assert!((talons.ability_totals("talent:talon-strike").damage - 120.0).abs() < 1e-9);

    let mut frostweaver_profile = rime_profile();
    frostweaver_profile.talents.push(rime_talent(
        18,
        [
            ("procChance", 1.0),
            ("maximumStacks", 2.0),
            ("durationSeconds", 12.0),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let frostweaver_apl = apl([("glacial-blast", None)]);
    let mut frostweaver = Iteration::new(&frostweaver_profile, &frostweaver_apl, 1, 4);
    frostweaver.add_rime_orbs(2);
    assert_eq!(frostweaver.buff_stacks(AplBuff::FrostweaversWrath), 2);
    let glacial = frostweaver.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    frostweaver.cast(glacial);
    frostweaver.process_events_through(2_000);
    assert_eq!(frostweaver.buff_stacks(AplBuff::FrostweaversWrath), 1);
    assert_eq!(frostweaver.ability_totals("test:glacial-blast").crits, 1);
}

#[test]
fn rime_bursting_talents_preserve_pulse_cadence_thresholds_and_triggers() {
    let mut baseline_profile = rime_profile();
    let bursting = baseline_profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::BurstingIce)
        .expect("Bursting Ice model");
    bursting.effect_duration_ms = 3_000;
    let baseline_apl = apl([("bursting-ice", None)]);
    let mut baseline = Iteration::new(&baseline_profile, &baseline_apl, 1, 1);
    let bursting = baseline.common.abilities_by_kind[&DpsAbilityKind::BurstingIce];
    baseline.impact(bursting, false, 1.0, 0, DamageContext::for_cast(1));
    baseline.process_events_through(3_000);
    assert_eq!(baseline.ability_totals("test:bursting-ice").hits, 6);
    assert_eq!(baseline.hero.rime().anima, 6.0);

    let mut burstbolter_profile = rime_profile();
    burstbolter_profile.talents.push(rime_talent(
        2,
        [("burstingDamageMultiplier", 2.0), ("procChance", 1.0)],
    ));
    let burstbolter_apl = apl([("frost-bolt", None)]);
    let mut burstbolter = Iteration::new(&burstbolter_profile, &burstbolter_apl, 1, 2);
    let frost_bolt = burstbolter.common.abilities_by_kind[&DpsAbilityKind::FrostBolt];
    burstbolter.impact(frost_bolt, false, 1.0, 0, DamageContext::for_cast(1));
    burstbolter.process_events_through(100);
    assert_eq!(burstbolter.ability_totals("test:bursting-ice").hits, 1);
    assert!((burstbolter.ability_totals("test:bursting-ice").damage - 100.0).abs() < 1e-9);

    let mut harrowing_profile = rime_profile();
    harrowing_profile.talents.push(rime_talent(
        11,
        [
            ("durationSeconds", 12.0),
            ("maximumStacks", 3.0),
            ("damageIncreasePerStack", 0.1),
            ("flightCooldownReductionSeconds", 5.0),
        ],
    ));
    let harrowing_apl = apl([("bursting-ice", None)]);
    let mut harrowing = Iteration::new(&harrowing_profile, &harrowing_apl, 1, 3);
    harrowing.common.cooldowns.insert(
        DpsAbilityKind::FlightOfTheNavir,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    harrowing.rime_bursting_damage_pulse(DamageContext::for_cast(1));
    harrowing.rime_bursting_damage_pulse(DamageContext::for_cast(1));
    assert_eq!(harrowing.buff_stacks(AplBuff::HarrowingIce), 2);
    assert!((harrowing.ability_totals("test:bursting-ice").damage - 105.0).abs() < 1e-9);
    harrowing.rime_bursting_damage_pulse(DamageContext::for_cast(1));
    assert_eq!(harrowing.buff_stacks(AplBuff::HarrowingIce), 0);
    assert_eq!(harrowing.ability_totals("test:bursting-ice").hits, 3);
    assert!((harrowing.ability_totals("test:bursting-ice").damage - 165.0).abs() < 1e-9);
    assert_eq!(
        harrowing.common.cooldowns[&DpsAbilityKind::FlightOfTheNavir].remaining_ms,
        5_000.0
    );

    let mut swallows_profile = rime_profile();
    swallows_profile
        .talents
        .push(rime_talent(15, [("procChance", 1.0)]));
    let swallows_apl = apl([("flight-of-the-navir", None)]);
    let mut swallows = Iteration::new(&swallows_profile, &swallows_apl, 1, 4);
    swallows.rime_triggered_damage(
        RimeTriggeredDamageSource::FrostSwallow,
        0.469,
        0.0,
        0,
        DamageContext::for_cast(1),
    );
    assert_eq!(swallows.ability_totals("test:bursting-ice").hits, 1);
}

#[test]
fn rime_wrath_volleys_scale_their_cadence_with_haste() {
    let mut profile = rime_profile();
    profile.haste = 1.0;
    let wrath = profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::WrathOfWinter)
        .expect("Wrath of Winter model");
    wrath.effect_duration_ms = 5_000;
    let apl = apl([("wrath-of-winter", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    let wrath = iteration.common.abilities_by_kind[&DpsAbilityKind::WrathOfWinter];
    iteration.impact(wrath, false, 1.0, 0, DamageContext::for_cast(1));

    iteration.process_events_through(1_999);
    assert_eq!(
        iteration
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        3
    );
    iteration.process_events_through(3_999);
    assert_eq!(
        iteration
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        6
    );
    iteration.process_events_through(5_000);
    assert_eq!(
        iteration
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        9
    );
    assert_eq!(
        iteration
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .targets_hit_mask,
        0b111
    );
}

#[test]
fn flight_commands_swallows_and_navir_and_cascading_blitz_add_their_projectiles() {
    let mut flight_profile = rime_profile();
    flight_profile.talents.push(rime_talent(
        10,
        [("freeColdSnapCharges", 2.0), ("durationSeconds", 12.0)],
    ));
    let flight_apl = apl([("flight-of-the-navir", None)]);
    let mut flight = Iteration::new(&flight_profile, &flight_apl, 1, 2);
    flight.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
    assert_eq!(flight.buff_stacks(AplBuff::FlightOfTheNavir), 1);
    assert_eq!(flight.hero.rime().navir_free_cold_snaps, 2);

    let cold_snap = flight.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    flight.impact(cold_snap, false, 1.0, 0, DamageContext::for_cast(1));
    flight.process_events_through(1_000);
    assert_eq!(
        flight
            .ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
            .hits,
        5
    );
    let torrent = flight.common.abilities_by_kind[&DpsAbilityKind::FreezingTorrent];
    flight.impact(torrent, true, 1.0, 0, DamageContext::for_cast(2));
    flight.process_events_through(1_500);
    assert_eq!(
        flight
            .ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
            .hits,
        6
    );

    let mut cascading_profile = rime_profile();
    cascading_profile
        .talents
        .push(rime_talent(13, [("spikesPerAnima", 1.0)]));
    let cascading_apl = apl([("ice-blitz", None)]);
    let mut cascading = Iteration::new(&cascading_profile, &cascading_apl, 1, 3);
    cascading.apply_rime_ability_buff(DpsAbilityKind::IceBlitz);
    cascading.add_rime_anima(2.0, DamageContext::for_cast(1));
    cascading.process_events_through(500);
    assert_eq!(
        cascading
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        2
    );
}

#[test]
fn rime_torrent_and_comet_talents_use_their_authored_proc_branches() {
    let mut coalescing_profile = rime_profile();
    coalescing_profile.talents.push(rime_talent(
        3,
        [
            ("durationSeconds", 3.0),
            ("powerCoefficientPerStack", 0.56),
            ("targetCountDamageScalingThreshold", 12.0),
            ("criticalExtraStackChance", 1.0),
            ("criticalExtraStacks", 1.0),
            ("maximumStacks", 99.0),
        ],
    ));
    let coalescing_apl = apl([("freezing-torrent", None)]);
    let mut coalescing = Iteration::new(&coalescing_profile, &coalescing_apl, 1, 1);
    coalescing.on_rime_torrent_tick(true, 0, DamageContext::for_cast(1));
    assert_eq!(coalescing.hero.rime().coalescing_stacks[0], 2);
    coalescing.process_events_through(3_000);
    assert_eq!(coalescing.hero.rime().coalescing_stacks[0], 0);
    assert_eq!(coalescing.ability_totals("talent:coalescing-frost").hits, 1);
    assert!((100.8..=123.2).contains(&coalescing.ability_totals("talent:coalescing-frost").damage));

    let mut shower_profile = rime_profile();
    shower_profile
        .talents
        .push(rime_talent(7, [("procChance", 1.0)]));
    let shower_apl = apl([("freezing-torrent", None)]);
    let mut shower = Iteration::new(&shower_profile, &shower_apl, 1, 2);
    shower.try_rime_cold_shower(DamageContext::for_cast(1));
    shower.process_events_through(500);
    assert_eq!(shower.ability_totals("test:ice-comet").hits, 1);

    let mut avalanche_profile = rime_profile();
    avalanche_profile.talents.push(rime_talent(
        6,
        [("oneExtraChance", 0.0), ("twoExtraChance", 1.0)],
    ));
    let avalanche_apl = apl([("ice-comet", None)]);
    let mut avalanche = Iteration::new(&avalanche_profile, &avalanche_apl, 1, 3);
    avalanche.hero.rime_mut().winter_orbs = 2;
    let comet = avalanche.common.abilities_by_kind[&DpsAbilityKind::IceComet];
    avalanche.cast(comet);
    avalanche.process_events_through(2_000);
    assert_eq!(avalanche.ability_totals("test:ice-comet").hits, 3);

    let mut soulfrost_profile = rime_profile();
    soulfrost_profile.talents.push(rime_talent(
        9,
        [
            ("procsPerMinute", 0.000_001),
            ("durationSeconds", 18.0),
            ("tickRateMultiplier", 2.0),
            ("criticalStrikeBonus", 1.0),
        ],
    ));
    let soulfrost_apl = apl([("freezing-torrent", None)]);
    let mut soulfrost = Iteration::new(&soulfrost_profile, &soulfrost_apl, 1, 4);
    soulfrost.hero.rime_mut().soulfrost_torrent_until = 18_000;
    let torrent = soulfrost.common.abilities_by_kind[&DpsAbilityKind::FreezingTorrent];
    soulfrost.cast(torrent);
    let torrent_totals = &soulfrost.ability_totals("test:freezing-torrent");
    assert_eq!(torrent_totals.hits, 11);
    assert_eq!(torrent_totals.crits, 11);
    assert_eq!(soulfrost.hero.rime().soulfrost_torrent_until, 0);

    let mut supreme_profile = rime_profile();
    supreme_profile.talents.push(rime_talent(
        12,
        [("damageMultiplier", 1.2), ("durationIncreaseSeconds", 0.8)],
    ));
    let supreme_apl = apl([("freezing-torrent", None)]);
    let mut supreme = Iteration::new(&supreme_profile, &supreme_apl, 1, 5);
    let torrent = supreme.common.abilities_by_kind[&DpsAbilityKind::FreezingTorrent];
    supreme.cast(torrent);
    let torrent_totals = &supreme.ability_totals("test:freezing-torrent");
    assert_eq!(torrent_totals.hits, 8);
    assert!((torrent_totals.damage - 960.0).abs() < 1e-9);
}

#[test]
fn rime_cooldown_talents_reduce_only_their_authored_abilities() {
    let mut finesse_profile = rime_profile();
    finesse_profile.talents.push(rime_talent(
        4,
        [
            ("burstingCooldownReductionSeconds", 0.2),
            ("torrentCooldownReductionSeconds", 1.5),
        ],
    ));
    let finesse_apl = apl([("cold-snap", None)]);
    let mut finesse = Iteration::new(&finesse_profile, &finesse_apl, 1, 1);
    finesse.common.cooldowns.insert(
        DpsAbilityKind::BurstingIce,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    finesse.common.cooldowns.insert(
        DpsAbilityKind::FreezingTorrent,
        CooldownState {
            remaining_ms: 15_000.0,
            used_charges: 1,
        },
    );
    finesse.on_rime_torrent_tick(false, 0, DamageContext::for_cast(1));
    assert_eq!(
        finesse.common.cooldowns[&DpsAbilityKind::BurstingIce].remaining_ms,
        9_800.0
    );
    let cold_snap = finesse.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    finesse.commit_ability(cold_snap, &mut DamageContext::for_cast(1), false, 0.0);
    assert_eq!(
        finesse.common.cooldowns[&DpsAbilityKind::FreezingTorrent].remaining_ms,
        13_500.0
    );

    let mut wisdom_profile = rime_profile();
    wisdom_profile
        .talents
        .push(rime_talent(8, [("cooldownReductionPerOrbSeconds", 0.3)]));
    let wisdom_apl = apl([("cold-snap", None)]);
    let mut wisdom = Iteration::new(&wisdom_profile, &wisdom_apl, 1, 2);
    for kind in [
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::WintersBlessing,
    ] {
        wisdom.common.cooldowns.insert(
            kind,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );
    }
    wisdom.reduce_rime_major_cooldowns(2);
    for kind in [
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::WintersBlessing,
    ] {
        assert_eq!(wisdom.common.cooldowns[&kind].remaining_ms, 9_400.0);
    }
    assert!(
        !wisdom
            .common
            .cooldowns
            .contains_key(&DpsAbilityKind::FreezingTorrent)
    );
}

#[test]
fn greater_glacial_blast_and_biting_cold_apply_their_damage_contracts() {
    let mut glacial_profile = rime_profile();
    glacial_profile.talents.push(rime_talent(
        14,
        [("damageMultiplier", 1.4), ("castTimeIncreaseSeconds", 0.5)],
    ));
    glacial_profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::GlacialBlast)
        .expect("Glacial Blast model")
        .gcd_ms = 0;
    let glacial_apl = apl([("glacial-blast", None)]);
    let mut glacial = Iteration::new(&glacial_profile, &glacial_apl, 1, 1);
    glacial.hero.rime_mut().winter_orbs = 2;
    let glacial_index = glacial.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    glacial.cast(glacial_index);
    assert_eq!(glacial.common.now_ms, 500);
    glacial.process_events_through(2_000);
    assert!((glacial.ability_totals("test:glacial-blast").damage - 140.0).abs() < 1e-9);

    let mut biting_profile = rime_profile();
    biting_profile.critical_strike = 1.0;
    biting_profile
        .talents
        .push(rime_talent(16, [("criticalPowerMultiplier", 1.1)]));
    let biting_apl = apl([("frost-bolt", None)]);
    let mut biting = Iteration::new(&biting_profile, &biting_apl, 1, 2);
    let frost_bolt = biting.common.abilities_by_kind[&DpsAbilityKind::FrostBolt];
    biting.impact(frost_bolt, false, 1.0, 0, DamageContext::for_cast(1));
    assert_eq!(biting.ability_totals("test:frost-bolt").crits, 1);
    assert!((biting.ability_totals("test:frost-bolt").damage - 220.0).abs() < 1e-9);
}

#[test]
fn frostwyrm_consumes_owner_stacks_and_falloff_excludes_primary() {
    let mut profile = rime_profile();
    profile.mechanics.push(rime_legendary(
        2,
        [
            ("frostwyrmMaximumStacks", 30.0),
            ("frostwyrmDurationSeconds", 15.0),
            ("frostwyrmTargetsPerStack", 1.0),
            ("frostwyrmDamageIncreasePerStack", 0.3),
            ("frostwyrmTargetCountThreshold", 3.0),
        ],
    ));
    let apl = apl([("cold-snap", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 5, 1);
    for cast_id in 1..=3 {
        iteration.on_rime_torrent_tick(false, 0, DamageContext::for_cast(cast_id));
    }
    assert_eq!(iteration.hero.rime().frostwyrm_stacks, 3);

    let cold_snap = iteration.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    iteration.cast(cold_snap);
    assert_eq!(iteration.hero.rime().frostwyrm_stacks, 0);
    let totals = &iteration.ability_totals("test:cold-snap");
    assert_eq!(totals.hits, 4);
    assert_eq!(totals.targets_hit_mask, 0b1111);
    let expected = 100.0 * 1.9 * 4.0;
    assert!((totals.damage - expected).abs() < 1e-9);

    let mut larger = Iteration::new(&profile, &apl, 6, 1);
    larger.hero.rime_mut().frostwyrm_stacks = 5;
    larger.hero.rime_mut().frostwyrm_until = 15_000;
    larger.cast(cold_snap);
    let secondary = (250.0 * (3.0_f64 / 5.0).sqrt()).round();
    assert_eq!(
        larger.ability_totals("test:cold-snap").damage,
        250.0 + 5.0 * secondary
    );
}

#[test]
fn rime_anima_recaptures_at_impact_and_procs_ignore_cast_modifiers() {
    let mut snapshot_profile = rime_profile();
    snapshot_profile.mechanics.push(ardeos_mechanic(
        "test:rime-snapshot-expertise",
        [("expertiseBonus", 1.0)],
    ));
    let snapshot_apl = apl([("frost-bolt", None)]);
    let mut snapshot = Iteration::new(&snapshot_profile, &snapshot_apl, 1, 1);
    let snapshot_mechanic = snapshot.test_mechanic_index("test:rime-snapshot-expertise");
    snapshot.shared.dynamic_buffs[snapshot_mechanic] = Some(DynamicBuffState {
        started_ms: 0,
        until_ms: 1_000,
        stacks: 1,
        value: 0.0,
    });
    snapshot.schedule_rime_projectile(
        RimeTriggeredDamageSource::AnimaSpike,
        1.0,
        0.0,
        0,
        DamageContext::for_cast(1),
        500,
    );
    snapshot.shared.dynamic_buffs[snapshot_mechanic] = None;
    snapshot.process_events_through(500);
    assert!(
        (snapshot
            .ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .damage
            - 100.0)
            .abs()
            < 1e-9
    );

    let mut proc_profile = rime_profile();
    proc_profile.talents.push(rime_talent(
        3,
        [
            ("durationSeconds", 3.0),
            ("powerCoefficientPerStack", 0.56),
            ("targetCountDamageScalingThreshold", 12.0),
            ("criticalExtraStackChance", 0.0),
            ("criticalExtraStacks", 1.0),
            ("maximumStacks", 99.0),
        ],
    ));
    let proc_apl = apl([("freezing-torrent", None)]);
    let mut baseline = Iteration::new(&proc_profile, &proc_apl, 1, 2);
    baseline.on_rime_torrent_tick(false, 0, DamageContext::for_cast(7));
    baseline.process_events_through(3_000);
    let baseline_damage = baseline.ability_totals("talent:coalescing-frost").damage;

    let mut modified = Iteration::new(&proc_profile, &proc_apl, 1, 2);
    let mut modified_context = DamageContext::for_cast(7);
    modified_context.multiply_damage(5.0);
    modified.on_rime_torrent_tick(false, 0, modified_context);
    modified.process_events_through(3_000);
    assert_eq!(
        modified.ability_totals("talent:coalescing-frost").damage,
        baseline_damage
    );
}

#[test]
fn iteration_results_include_wildfire_and_queryable_rime_buff_uptimes() {
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.effect_duration_ms = 10_000;
    let wildfire_profile = profile(vec![wildfire]);
    let wildfire_result = Iteration::new(&wildfire_profile, &apl([("wildfire", None)]), 1, 1)
        .run()
        .expect("wildfire iteration completes");
    assert_eq!(wildfire_result.uptimes["buff:wildfire"], 1.0);

    let ice_blitz_profile = rime_profile();
    let ice_blitz_result = Iteration::new(&ice_blitz_profile, &apl([("ice-blitz", None)]), 1, 1)
        .run()
        .expect("ice blitz iteration completes");
    assert_eq!(ice_blitz_result.uptimes["buff:ice-blitz"], 1.0);
}

#[test]
fn frostwyrm_expired_owner_stacks_do_not_revive_on_a_new_target() {
    let mut profile = rime_profile();
    profile.mechanics.push(rime_legendary(
        2,
        [
            ("frostwyrmMaximumStacks", 30.0),
            ("frostwyrmDurationSeconds", 15.0),
            ("frostwyrmTargetsPerStack", 1.0),
            ("frostwyrmDamageIncreasePerStack", 0.3),
            ("frostwyrmTargetCountThreshold", 3.0),
        ],
    ));
    let apl = apl([("cold-snap", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 5, 1);
    iteration.on_rime_torrent_tick(false, 0, DamageContext::for_cast(1));
    iteration.advance_to(15_000);
    iteration.on_rime_torrent_tick(false, 1, DamageContext::for_cast(2));
    assert_eq!(iteration.hero.rime().frostwyrm_stacks, 1);
    iteration.on_rime_torrent_tick(false, 0, DamageContext::for_cast(3));
    assert_eq!(iteration.hero.rime().frostwyrm_stacks, 2);
}

fn spender_flow_talent() -> DpsTalentModel {
    rime_talent(
        5,
        [
            ("durationSeconds", 15.0),
            ("maximumStacks", 2.0),
            ("castHaste", 1.0),
            ("cometInitialDelaySeconds", 0.5),
            ("criticalStrikeBonus", 1.0),
        ],
    )
}

fn spender_swap_talent() -> DpsTalentModel {
    rime_talent(
        17,
        [
            ("singleTargetDamageMultiplier", 0.4),
            ("multiTargetDamageMultiplier", 0.4),
            ("singleTargetPulsePeriodSeconds", 0.15),
            ("multiTargetPulsePeriodSeconds", 0.15),
            ("singleTargetInitialDelaySeconds", 0.35),
            ("multiTargetInitialDelaySeconds", 0.5),
        ],
    )
}

#[test]
fn avalanche_selects_exclusive_one_or_two_extra_pulses() {
    let apl = apl([("ice-comet", None)]);
    for (one, two, expected) in [(0.0, 0.0, 0), (1.0, 0.0, 1), (0.0, 1.0, 2)] {
        let mut profile = rime_profile();
        profile.talents.push(rime_talent(
            6,
            [("oneExtraChance", one), ("twoExtraChance", two)],
        ));
        let mut iteration = Iteration::new(&profile, &apl, 1, 42);
        for _ in 0..10 {
            assert_eq!(iteration.roll_rime_avalanche(), expected);
        }
    }
    let mut profile = rime_profile();
    profile.talents.push(rime_talent(
        6,
        [("oneExtraChance", 0.18), ("twoExtraChance", 0.09)],
    ));
    let mut iteration = Iteration::new(&profile, &apl, 1, 42);
    let mut counts = [0; 3];
    for _ in 0..1000 {
        counts[iteration.roll_rime_avalanche() as usize] += 1;
    }
    assert!(counts.iter().all(|count| *count > 0));
}

#[test]
fn comet_pulses_keep_their_spawn_snapshot_and_raw_interval() {
    let mut profile = rime_profile();
    profile.haste = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "test:late-expertise",
        [("expertiseBonus", 1.0)],
    ));
    profile.talents.push(rime_talent(
        6,
        [("oneExtraChance", 0.0), ("twoExtraChance", 1.0)],
    ));
    let apl = apl([("ice-comet", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.schedule_rime_comet(DamageContext::for_cast(1), 1000);
    iteration.process_events_through(799);
    assert_eq!(iteration.ability_totals("test:ice-comet").hits, 0);
    let index = iteration.test_mechanic_index("test:late-expertise");
    iteration.shared.dynamic_buffs[index] = Some(DynamicBuffState {
        started_ms: 799,
        until_ms: 2000,
        stacks: 1,
        value: 0.0,
    });
    for (at, hits) in [(800, 1), (999, 1), (1000, 2), (1199, 2), (1200, 3)] {
        iteration.process_events_through(at);
        assert_eq!(iteration.ability_totals("test:ice-comet").hits, hits);
    }
    assert_eq!(iteration.ability_totals("test:ice-comet").damage, 300.0);
}

#[test]
fn cold_shower_uses_icy_flow_delay_and_avalanche_without_spending_orbs() {
    let mut profile = rime_profile();
    profile.talents.extend([
        spender_flow_talent(),
        rime_talent(7, [("procChance", 1.0)]),
        rime_talent(6, [("oneExtraChance", 0.0), ("twoExtraChance", 1.0)]),
    ]);
    let apl = apl([("freezing-torrent", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().winter_orbs = 3;
    iteration.hero.rime_mut().icy_flow_stacks = 2;
    iteration.hero.rime_mut().icy_flow_until = 10_000;
    let mut context = DamageContext::for_cast(1);
    context.multiply_damage(10.0);
    iteration.try_rime_cold_shower(context);
    assert_eq!(iteration.buff_stacks(AplBuff::IcyFlow), 1);
    assert_eq!(iteration.hero.rime().winter_orbs, 3);
    iteration.process_events_through(799);
    assert_eq!(iteration.ability_totals("test:ice-comet").hits, 0);
    iteration.process_events_through(1200);
    assert_eq!(iteration.ability_totals("test:ice-comet").hits, 3);
    assert_eq!(iteration.ability_totals("test:ice-comet").crits, 3);
    assert_eq!(iteration.ability_totals("test:ice-comet").damage, 600.0);
}

#[test]
fn swapped_cold_shower_distinguishes_absent_avalanche_from_failed_roll() {
    let apl = apl([("freezing-torrent", None)]);
    for (avalanche, expected_hits, remaining_flow) in [(false, 1, 0), (true, 0, 1)] {
        let mut profile = rime_profile();
        profile
            .talents
            .extend([spender_flow_talent(), spender_swap_talent()]);
        if avalanche {
            profile.talents.push(rime_talent(
                6,
                [("oneExtraChance", 0.0), ("twoExtraChance", 0.0)],
            ));
        }
        let mut iteration = Iteration::new(&profile, &apl, 1, 1);
        iteration.hero.rime_mut().icy_flow_stacks = 1;
        iteration.hero.rime_mut().icy_flow_until = 10_000;
        iteration.schedule_rime_comet(DamageContext::for_cast(1), 0);
        iteration.process_events_through(2000);
        assert_eq!(
            iteration.ability_totals("test:ice-comet").hits,
            expected_hits
        );
        assert_eq!(iteration.buff_stacks(AplBuff::IcyFlow), remaining_flow);
    }
}

#[test]
fn glacial_assault_grants_a_free_full_talon_volley_with_both_damage_modifiers() {
    let mut profile = rime_profile();
    profile.max_secondary_resource = 5;
    profile.talents.extend([
        spender_swap_talent(),
        rime_talent(
            1,
            [
                ("maximumStacks", 4.0),
                ("damageMultiplier", 1.4),
                ("explosionDamageFraction", 0.3),
                ("explosionRadius", 700.0),
                ("targetCountDamageScalingThreshold", 12.0),
            ],
        ),
        rime_talent(
            14,
            [("damageMultiplier", 1.4), ("castTimeIncreaseSeconds", 0.5)],
        ),
    ]);
    let apl = apl([("glacial-blast", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    iteration.hero.rime_mut().glacial_assault_stacks = 4;
    iteration.hero.rime_mut().winter_orbs = 2;
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    iteration.cast(index);
    iteration.process_events_through(2000);
    assert_eq!(iteration.hero.rime().winter_orbs, 2);
    assert_eq!(iteration.hero.rime().glacial_assault_stacks, 0);
    assert_eq!(iteration.ability_totals("talent:talon-strike").hits, 5);
    assert!((iteration.ability_totals("talent:talon-strike").damage - 390.0).abs() < 1e-9);
    assert_eq!(iteration.ability_totals("talent:glacial-assault").hits, 10);
    assert!((iteration.ability_totals("talent:glacial-assault").damage - 230.0).abs() < 1e-9);
}

#[test]
fn glacial_cast_spends_orbs_at_completion_after_intervening_gains() {
    let mut profile = rime_profile();
    profile.max_secondary_resource = 5;
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::GlacialBlast)
        .unwrap()
        .cast_time_ms = 2000;
    let apl = apl([("glacial-blast", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().winter_orbs = 5;
    iteration.push_event(500, RimeEvent::OrbRefund { orbs: 1 });
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    iteration.cast(index);
    assert_eq!(iteration.hero.rime().winter_orbs, 3);
}

#[test]
fn icy_flow_haste_adds_to_existing_haste_but_expired_crit_is_not_captured() {
    let mut profile = rime_profile();
    profile.haste = 1.0;
    profile.talents.push(spender_flow_talent());
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::GlacialBlast)
        .unwrap()
        .cast_time_ms = 2000;
    let apl = apl([("glacial-blast", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().winter_orbs = 2;
    iteration.hero.rime_mut().icy_flow_stacks = 1;
    iteration.hero.rime_mut().icy_flow_until = 500;
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::GlacialBlast];
    iteration.cast(index);
    assert_eq!(iteration.common.now_ms, 667);
    assert_eq!(iteration.effective_haste(), 1.0);
    iteration.process_events_through(2200);
    assert_eq!(iteration.ability_totals("test:glacial-blast").crits, 0);
}

#[test]
fn rising_talons_consumes_flow_before_its_separate_avalanche_actor() {
    let mut profile = rime_profile();
    profile.talents.extend([
        spender_swap_talent(),
        spender_flow_talent(),
        rime_talent(6, [("oneExtraChance", 1.0), ("twoExtraChance", 0.0)]),
    ]);
    let apl = apl([("ice-comet", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.rime_mut().winter_orbs = 3;
    iteration.hero.rime_mut().icy_flow_stacks = 1;
    iteration.hero.rime_mut().icy_flow_until = 10_000;
    let index = iteration.common.abilities_by_kind[&DpsAbilityKind::IceComet];
    iteration.cast(index);
    iteration.process_events_through(2000);
    assert_eq!(iteration.ability_totals("talent:rising-talons").hits, 3);
    assert_eq!(iteration.ability_totals("talent:rising-talons").crits, 3);
    assert_eq!(iteration.ability_totals("test:ice-comet").hits, 1);
    assert_eq!(iteration.ability_totals("test:ice-comet").crits, 0);
    assert_eq!(iteration.hero.rime().winter_orbs, 0);
}

#[test]
fn wisdom_observes_aggregate_capped_refunds_once_and_only_tagged_cooldowns() {
    let mut profile = rime_profile();
    profile
        .talents
        .push(rime_talent(8, [("cooldownReductionPerOrbSeconds", 0.3)]));
    let apl = apl([("frost-bolt", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    for kind in [
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::WintersBlessing,
        DpsAbilityKind::FreezingTorrent,
    ] {
        iteration.common.cooldowns.insert(
            kind,
            CooldownState {
                remaining_ms: 10_000.0,
                used_charges: 1,
            },
        );
    }
    iteration.hero.rime_mut().winter_orbs = profile.max_secondary_resource;
    iteration.handle_rime_event(RimeEvent::OrbRefund { orbs: 3 });
    for kind in [
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::WintersBlessing,
    ] {
        assert_eq!(iteration.common.cooldowns[&kind].remaining_ms, 9700.0);
    }
    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::FreezingTorrent].remaining_ms,
        10_000.0
    );
}

#[test]
fn talon_actors_scale_only_startup_and_use_distinct_first_pulse_rules() {
    for (kind, source, first_hit) in [
        (DpsAbilityKind::GlacialBlast, "talent:talon-strike", 325),
        (DpsAbilityKind::IceComet, "talent:rising-talons", 250),
    ] {
        let mut profile = rime_profile();
        profile.haste = 1.0;
        profile.talents.push(spender_swap_talent());
        profile
            .abilities
            .iter_mut()
            .find(|ability| ability.kind == kind)
            .unwrap()
            .gcd_ms = 0;
        let apl = apl([("frost-bolt", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 1);
        iteration.hero.rime_mut().winter_orbs = 2;
        let index = iteration.common.abilities_by_kind[&kind];
        iteration.cast(index);
        iteration.process_events_through(first_hit - 1);
        assert_eq!(iteration.ability_totals(source).hits, 0);
        iteration.process_events_through(first_hit);
        assert_eq!(iteration.ability_totals(source).hits, 1);
        iteration.process_events_through(first_hit + 149);
        assert_eq!(iteration.ability_totals(source).hits, 1);
        iteration.process_events_through(first_hit + 150);
        assert_eq!(iteration.ability_totals(source).hits, 2);
    }
}

#[test]
fn frost_bolt_grants_anima_on_spawn_before_projectile_damage() {
    let mut profile = rime_profile();
    profile
        .abilities
        .iter_mut()
        .find(|a| a.kind == DpsAbilityKind::FrostBolt)
        .unwrap()
        .cast_time_ms = 1_500;
    let apl = apl([("frost-bolt", None)]);
    let mut run = Iteration::new(&profile, &apl, 1, 41);
    let index = run.common.abilities_by_kind[&DpsAbilityKind::FrostBolt];
    run.cast(index);
    assert_eq!(run.common.now_ms, 1_500);
    assert_eq!(run.hero.rime().anima, 3.0);
    assert_eq!(run.ability_totals("test:frost-bolt").hits, 0);
    run.process_events_through(3_000);
    assert_eq!(run.hero.rime().anima, 3.0);
    assert_eq!(run.ability_totals("test:frost-bolt").hits, 1);
}

#[test]
fn cold_snap_launches_a_volley_without_anima_or_flight() {
    let profile = rime_profile();
    let apl = apl([("cold-snap", None)]);
    let mut run = Iteration::new(&profile, &apl, 1, 42);
    let index = run.common.abilities_by_kind[&DpsAbilityKind::ColdSnap];
    run.impact(index, false, 1.0, 0, DamageContext::for_cast(1));
    assert_eq!(run.hero.rime().winter_orbs, 1);
    assert_eq!(run.hero.rime().anima, 0.0);
    run.process_events_through(800);
    assert_eq!(
        run.ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        3
    );
}

#[test]
fn anima_volley_shuffles_primary_and_pads_only_short_lists() {
    let profile = rime_profile();
    let apl = apl([("frost-bolt", None)]);
    let mut omitted_primary = false;
    for seed in 1..32 {
        let mut run = Iteration::new(&profile, &apl, 6, seed);
        run.fire_rime_anima_volley(3, 0, DamageContext::for_cast(1));
        run.process_events_through(800);
        let totals = run.ability_totals("GA_Rime_Helper_AutoDamageProjectile");
        assert_eq!(totals.hits, 3);
        assert_eq!(totals.targets_hit_mask.count_ones(), 3);
        omitted_primary |= totals.targets_hit_mask & 1 == 0;
    }
    assert!(omitted_primary);
    let mut run = Iteration::new(&profile, &apl, 2, 43);
    run.fire_rime_anima_volley(3, 1, DamageContext::for_cast(1));
    let mut selected = vec![];
    for event in &run.common.queue {
        if let EventKind::Rime(RimeEvent::TriggeredDamage { target_index, .. }) = event.0.kind {
            selected.push(target_index);
        }
    }
    selected.sort();
    assert_eq!(selected, [0, 1, 1]);
}

#[test]
fn wrath_grants_orbs_and_keeps_activation_haste_for_later_volleys() {
    let profile = rime_profile();
    let apl = apl([("wrath-of-winter", None)]);
    let mut run = Iteration::new(&profile, &apl, 1, 44);
    // This additive source Haste expires after the passive snapshots its interval.
    run.hero.rime_mut().icy_flow_casting_haste = 1.0;
    let index = run.common.abilities_by_kind[&DpsAbilityKind::WrathOfWinter];
    run.impact(index, false, 1.0, 0, DamageContext::for_cast(1));
    run.process_events_through(0);
    assert_eq!(run.hero.rime().winter_orbs, 1);
    run.hero.rime_mut().icy_flow_casting_haste = 0.0;
    run.process_events_through(4_000);
    assert_eq!(run.hero.rime().winter_orbs, 3);
}

#[test]
fn flight_busy_birds_are_retargeted_without_duplicate_hits() {
    let profile = rime_profile();
    let apl = apl([("flight-of-the-navir", None)]);
    let mut run = Iteration::new(&profile, &apl, 3, 45);
    run.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
    run.command_rime_frost_swallows(5, 0, DamageContext::for_cast(1));
    run.common.now_ms = 100;
    run.command_rime_frost_swallows(5, 2, DamageContext::for_cast(2));
    // Torrent cannot command a sixth bird while all five are busy.
    run.on_rime_torrent_tick(false, 0, DamageContext::for_cast(3));
    run.process_events_through(500);
    let totals = run.ability_totals("GA_Rime_TargetedPeriodicProjectileAoe");
    assert_eq!(totals.hits, 5);
    assert_eq!(totals.targets_hit_mask, 0b100);
    assert!(run.hero.rime().flight_birds.iter().all(Option::is_none));
    run.on_rime_torrent_tick(false, 0, DamageContext::for_cast(4));
    run.process_events_through(1_000);
    assert_eq!(
        run.ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
            .hits,
        6
    );
}

#[test]
fn replacing_or_expiring_flight_despawns_its_in_flight_birds() {
    let profile = rime_profile();
    let apl = apl([("flight-of-the-navir", None)]);
    for replace in [false, true] {
        let mut run = Iteration::new(&profile, &apl, 1, 46);
        run.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
        run.command_rime_frost_swallows(5, 0, DamageContext::for_cast(1));
        run.common.now_ms = 100;
        if replace {
            run.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
        } else {
            run.hero.rime_mut().flight_of_the_navir_until = 200;
        }
        run.process_events_through(1_000);
        assert_eq!(
            run.ability_totals("GA_Rime_TargetedPeriodicProjectileAoe")
                .hits,
            0
        );
    }
}

#[test]
fn navir_charges_cap_refresh_and_expire_without_spending_real_charges() {
    let mut profile = rime_profile();
    profile.talents.push(rime_talent(
        10,
        [("freeColdSnapCharges", 2.0), ("durationSeconds", 12.0)],
    ));
    let apl = apl([("cold-snap", None)]);
    let mut run = Iteration::new(&profile, &apl, 1, 47);
    run.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
    run.common.now_ms = 1_000;
    run.apply_rime_ability_buff(DpsAbilityKind::FlightOfTheNavir);
    assert_eq!(run.hero.rime().navir_free_cold_snaps, 2);
    run.common.cooldowns.insert(
        DpsAbilityKind::ColdSnap,
        CooldownState {
            remaining_ms: 20_000.0,
            used_charges: 2,
        },
    );
    let index = run.can_cast(DpsAbilityKind::ColdSnap).unwrap();
    run.cast(index);
    assert_eq!(run.hero.rime().navir_free_cold_snaps, 1);
    assert_eq!(
        run.common.cooldowns[&DpsAbilityKind::ColdSnap].used_charges,
        2
    );
    run.common.now_ms = 13_000;
    assert!(run.can_cast(DpsAbilityKind::ColdSnap).is_none());
}

#[test]
fn ice_blitz_requires_room_for_the_entire_extension() {
    let profile = rime_profile();
    let apl = apl([("ice-blitz", None)]);
    let mut run = Iteration::new(&profile, &apl, 1, 48);
    run.apply_rime_ability_buff(DpsAbilityKind::IceBlitz);
    let until = run.hero.rime().ice_blitz_until;
    run.common.now_ms = 50;
    run.extend_rime_ice_blitz();
    assert_eq!(run.hero.rime().ice_blitz_until, until);
    run.common.now_ms = 100;
    run.extend_rime_ice_blitz();
    assert_eq!(run.hero.rime().ice_blitz_until, until + 100);
}

#[test]
fn cascading_blitz_keeps_the_resource_events_target_and_single_spike_timing() {
    let mut profile = rime_profile();
    profile
        .talents
        .push(rime_talent(13, [("spikesPerAnima", 1.0)]));
    let apl = apl([("ice-blitz", None)]);
    let mut run = Iteration::new(&profile, &apl, 3, 49);
    run.apply_rime_ability_buff(DpsAbilityKind::IceBlitz);
    run.add_rime_anima_at(3.0, 2, DamageContext::for_cast(1));
    run.process_events_through(499);
    assert_eq!(
        run.ability_totals("GA_Rime_Helper_AutoDamageProjectile")
            .hits,
        0
    );
    run.process_events_through(500);
    let totals = run.ability_totals("GA_Rime_Helper_AutoDamageProjectile");
    assert_eq!(totals.hits, 3);
    assert_eq!(totals.targets_hit_mask, 0b100);
}

fn torrent_soulfrost_talent() -> DpsTalentModel {
    rime_talent(
        9,
        [
            ("procsPerMinute", 0.000_001),
            ("durationSeconds", 18.0),
            ("tickRateMultiplier", 1.4),
            ("criticalStrikeBonus", 1.0),
        ],
    )
}

#[test]
fn torrent_scales_terminal_damage_unless_supreme_overwrites_the_factor() {
    for (supreme, expected_hits, expected_damage) in [(false, 7, 625.0), (true, 9, 1080.0)] {
        let mut profile = rime_profile();
        let torrent = profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == DpsAbilityKind::FreezingTorrent)
            .unwrap();
        torrent.channel.as_mut().unwrap().duration_ms = 2_100;
        torrent.primary_resource_generated = 1.0;
        if supreme {
            profile.talents.push(rime_talent(
                12,
                [("damageMultiplier", 1.2), ("durationIncreaseSeconds", 0.8)],
            ));
        }
        let apl = apl([]);
        let mut run = Iteration::new(&profile, &apl, 1, 50);
        run.cast(run.common.abilities_by_kind[&DpsAbilityKind::FreezingTorrent]);
        let totals = run.ability_totals("test:freezing-torrent");
        assert_eq!(totals.hits, expected_hits);
        assert!((totals.damage - expected_damage).abs() < 1e-9);
        // Even the fractional callback grants one whole resource event.
        assert_eq!(run.hero.rime().anima, if supreme { 0.0 } else { 7.0 });
        assert_eq!(run.hero.rime().winter_orbs, u32::from(supreme));
    }
}

#[test]
fn soulfrost_ignores_excluded_commits_and_does_not_refresh_a_pending_proc() {
    let mut profile = rime_profile();
    profile.talents.push(torrent_soulfrost_talent());
    let apl = apl([]);
    let mut run = Iteration::new(&profile, &apl, 1, 51);
    for kind in [
        DpsAbilityKind::FreezingTorrent,
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::WintersBlessing,
        DpsAbilityKind::WrathOfWinter,
        DpsAbilityKind::AnimaSpike,
    ] {
        run.try_soulfrost_torrent_proc(&run.ability(kind).unwrap().clone());
    }
    assert!(run.shared.proc_per_minute_states.is_empty());
    run.hero.rime_mut().soulfrost_torrent_until = 18_000;
    run.common.now_ms = 5_000;
    run.try_soulfrost_torrent_proc(&run.ability(DpsAbilityKind::FrostBolt).unwrap().clone());
    assert_eq!(run.hero.rime().soulfrost_torrent_until, 18_000);
    assert!(run.shared.proc_per_minute_states.is_empty());
    run.hero.rime_mut().soulfrost_torrent_until = 0;
    // Flight carries the Offensive tag despite applying a buff.
    run.try_soulfrost_torrent_proc(
        &run.ability(DpsAbilityKind::FlightOfTheNavir)
            .unwrap()
            .clone(),
    );
    assert!(
        run.shared
            .proc_per_minute_states
            .contains_key("RandomStream.Rime.Talent.ChanneledBeamSingleDamage.BoostByCritProc")
    );
}

#[test]
fn soulfrost_damage_multiplier_reads_live_generic_critical_chance_each_tick() {
    let mut profile = rime_profile();
    profile.critical_strike = 0.2;
    profile.mechanics.push(ardeos_mechanic(
        "test:live-critical",
        [("criticalStrikeBonus", 0.3)],
    ));
    let apl = apl([]);
    let mut run = Iteration::new(&profile, &apl, 1, 52);
    let torrent = run.common.abilities_by_kind[&DpsAbilityKind::FreezingTorrent];
    let mut context = CastImpactContext::new(DamageContext::for_cast(1));
    context.bonus_crit = 1.0;
    run.resolve_impact(torrent, true, 1.0, 0, context);
    assert_eq!(run.ability_totals("test:freezing-torrent").damage, 264.0);
    let index = run.test_mechanic_index("test:live-critical");
    run.shared.dynamic_buffs[index] = Some(DynamicBuffState {
        started_ms: 0,
        until_ms: 10_000,
        stacks: 1,
        value: 0.0,
    });
    run.resolve_impact(torrent, true, 1.0, 0, context);
    assert_eq!(run.ability_totals("test:freezing-torrent").damage, 639.0);
}

#[test]
fn bursting_recaptures_source_and_discards_the_triggering_cast_multiplier() {
    let mut profile = rime_profile();
    profile.mechanics.push(ardeos_mechanic(
        "test:bursting-expertise",
        [("expertiseBonus", 1.0)],
    ));
    let apl = apl([]);
    let mut run = Iteration::new(&profile, &apl, 1, 53);
    let index = run.common.abilities_by_kind[&DpsAbilityKind::BurstingIce];
    let mut context = DamageContext::for_cast(1)
        .with_snapshot(run.capture_damage_source_snapshot(DpsAbilityKind::BurstingIce));
    context.multiply_damage(10.0);
    run.impact(index, false, 1.0, 0, context);
    let buff = run.test_mechanic_index("test:bursting-expertise");
    run.shared.dynamic_buffs[buff] = Some(DynamicBuffState {
        started_ms: 0,
        until_ms: 10_000,
        stacks: 1,
        value: 0.0,
    });
    run.process_events_through(500);
    assert_eq!(run.ability_totals("test:bursting-ice").damage, 100.0);
    run.shared.dynamic_buffs[buff] = None;
    run.process_events_through(1_000);
    assert_eq!(run.ability_totals("test:bursting-ice").damage, 150.0);
}

#[test]
fn bursting_uses_nineteen_random_targets_and_one_resource_event() {
    let profile = rime_profile();
    let apl = apl([]);
    let mut omitted_primary = false;
    for seed in 1..=100 {
        let mut run = Iteration::new(&profile, &apl, 20, seed);
        run.rime_bursting_damage_pulse(DamageContext::for_cast(1));
        let totals = run.ability_totals("test:bursting-ice");
        assert_eq!(totals.hits, 19);
        assert_eq!(totals.targets_hit_mask.count_ones(), 19);
        assert!((totals.damage - 19.0 * (49.8 * (5.0_f64 / 19.0).sqrt()).round()).abs() < 1e-9);
        omitted_primary |= totals.targets_hit_mask & 1 == 0;
        assert_eq!(run.hero.rime().anima, 1.0);
    }
    assert!(omitted_primary);
}

#[test]
fn bursting_independent_instances_keep_terminal_fraction_and_minimum_interval() {
    for (duration, hits, damage) in [(1_150, 6, 230.0), (1_099, 4, 200.0), (99, 0, 0.0)] {
        let mut profile = rime_profile();
        profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == DpsAbilityKind::BurstingIce)
            .unwrap()
            .effect_duration_ms = duration;
        let apl = apl([]);
        let mut run = Iteration::new(&profile, &apl, 1, 54);
        let index = run.common.abilities_by_kind[&DpsAbilityKind::BurstingIce];
        for id in 1..=2 {
            run.impact(index, false, 1.0, 0, DamageContext::for_cast(id));
        }
        run.process_events_through(duration);
        assert_eq!(run.ability_totals("test:bursting-ice").hits, hits);
        assert!((run.ability_totals("test:bursting-ice").damage - damage).abs() < 1e-9);
        assert_eq!(run.hero.rime().anima, hits as f64);
        assert!(run.hero.rime().bursting_instances.is_empty());
    }
}

fn coalescing_talent() -> DpsTalentModel {
    rime_talent(
        3,
        [
            ("durationSeconds", 3.0),
            ("powerCoefficientPerStack", 0.56),
            ("targetCountDamageScalingThreshold", 12.0),
            ("criticalExtraStackChance", 1.0),
            ("criticalExtraStacks", 1.0),
            ("maximumStacks", 99.0),
        ],
    )
}

#[test]
fn coalescing_caps_stacks_and_refreshes_one_expiration() {
    let mut profile = rime_profile();
    profile.talents.push(coalescing_talent());
    let apl = apl([]);
    let mut run = Iteration::new(&profile, &apl, 1, 55);
    run.hero.rime_mut().coalescing_stacks[0] = 98;
    run.on_rime_torrent_tick(true, 0, DamageContext::for_cast(1));
    assert_eq!(run.hero.rime().coalescing_stacks[0], 99);
    run.common.now_ms = 100;
    run.on_rime_torrent_tick(true, 0, DamageContext::for_cast(1));
    run.process_events_through(3_000);
    assert_eq!(run.ability_totals("talent:coalescing-frost").hits, 0);
    run.process_events_through(3_100);
    assert_eq!(run.ability_totals("talent:coalescing-frost").hits, 1);
    assert_eq!(run.hero.rime().coalescing_stacks[0], 0);
}

#[test]
fn coalescing_primary_is_full_damage_and_secondary_spec_keeps_expiration_capture() {
    let mut profile = rime_profile();
    profile.talents.push(coalescing_talent());
    profile.mechanics.push(ardeos_mechanic(
        "test:coalescing-expertise",
        [("expertiseBonus", 1.0)],
    ));
    let apl = apl([]);
    let mut primary_damage = None;
    for targets in [1, 20] {
        let mut run = Iteration::new(&profile, &apl, targets, 56);
        run.on_rime_torrent_tick(false, 0, DamageContext::for_cast(1));
        let buff = run.test_mechanic_index("test:coalescing-expertise");
        run.shared.dynamic_buffs[buff] = Some(DynamicBuffState {
            started_ms: 0,
            until_ms: 10_000,
            stacks: 1,
            value: 0.0,
        });
        run.process_events_through(3_000);
        let damage = run.ability_totals("talent:coalescing-frost").damage;
        assert!((100.8..=123.2).contains(&damage));
        assert_eq!(run.ability_totals("talent:coalescing-frost").hits, 1);
        if let Some(previous) = primary_damage {
            assert_eq!(damage, previous);
        } else {
            primary_damage = Some(damage);
        }
        run.shared.dynamic_buffs[buff] = None;
        run.process_events_through(3_199);
        assert_eq!(run.ability_totals("talent:coalescing-frost").hits, 1);
        run.process_events_through(3_200);
        assert_eq!(
            run.ability_totals("talent:coalescing-frost").hits,
            targets as u64
        );
        if targets == 20 {
            let secondary = run.ability_totals("talent:coalescing-frost").damage - damage;
            let expected = 19.0 * 112.0 * (12.0_f64 / 19.0).sqrt();
            assert!((expected * 0.9..=expected * 1.1).contains(&secondary));
        }
    }
}

#[test]
fn burstbolter_rolls_its_own_stream_and_waits_for_the_visual_delay() {
    for chance in [0.0, 1.0] {
        let mut profile = rime_profile();
        profile.talents.push(rime_talent(
            2,
            [("burstingDamageMultiplier", 1.3), ("procChance", chance)],
        ));
        let apl = apl([]);
        let mut run = Iteration::new(&profile, &apl, 1, 57);
        let index = run.common.abilities_by_kind[&DpsAbilityKind::FrostBolt];
        let mut context = DamageContext::for_cast(1);
        context.multiply_damage(10.0);
        run.impact(index, false, 1.0, 0, context);
        assert_eq!(
            run.shared
                .controlled_random_states
                .contains_key("RandomStream.Rime.Talent.CastedDebuffAoeDamage.TriggerRandomTick"),
            chance > 0.0
        );
        run.process_events_through(99);
        assert_eq!(run.ability_totals("test:bursting-ice").hits, 0);
        run.process_events_through(100);
        assert_eq!(run.ability_totals("test:bursting-ice").hits, chance as u64);
        assert_eq!(
            run.ability_totals("test:bursting-ice").damage,
            65.0 * chance
        );
    }
}

#[test]
fn bursting_cast_applies_one_debuff_before_its_area_pulses() {
    let profile = rime_profile();
    let apl = apl([]);
    let mut run = Iteration::new(&profile, &apl, 3, 58);
    run.cast(run.common.abilities_by_kind[&DpsAbilityKind::BurstingIce]);
    assert_eq!(run.hero.rime().bursting_generation, 1);
    run.process_events_through(3_000);
    assert_eq!(run.ability_totals("test:bursting-ice").hits, 18);
    assert_eq!(run.hero.rime().anima, 6.0);
}
