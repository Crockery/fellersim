use super::super::*;

#[test]
fn elarion_focus_pulses_preserve_phase_across_spending_and_capping() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.mechanic_parameters.extend([
        ("focusRegenerationPerPulse".into(), 0.05),
        ("focusRegenerationIntervalSeconds".into(), 0.01),
    ]);
    let profile = elarion_profile([focused]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().focus = 0.0;
    iteration.process_events_through(9);
    assert_eq!(iteration.hero.elarion().focus, 0.0);
    iteration.process_events_through(10);
    assert_eq!(iteration.hero.elarion().focus, 0.05);
    iteration.process_events_through(29);
    assert_eq!(iteration.hero.elarion().focus, 0.1);
    iteration.hero.elarion_mut().focus = 99.98;
    iteration.process_events_through(35);
    assert_eq!(iteration.hero.elarion().focus, 100.0);
    iteration.hero.elarion_mut().focus = 0.0;
    iteration.process_events_through(39);
    assert_eq!(iteration.hero.elarion().focus, 0.0);
    iteration.process_events_through(40);
    assert_eq!(iteration.hero.elarion().focus, 0.05);
}

#[test]
fn elarion_focus_pulses_read_live_haste_including_expiration_boundaries() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.mechanic_parameters.extend([
        ("focusRegenerationPerPulse".into(), 0.05),
        ("focusRegenerationIntervalSeconds".into(), 0.01),
    ]);
    let mut grace = elarion_ability(DpsAbilityKind::SkystridersGrace);
    grace.mechanic_parameters.insert("hasteBonus".into(), 1.0);
    let profile = elarion_profile([focused, grace]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().focus = 0.0;
    iteration.process_events_through(9);
    iteration.hero.elarion_mut().skystriders_grace_until = 20;
    iteration.process_events_through(19);
    assert_eq!(iteration.hero.elarion().focus, 0.1);
    iteration.process_events_through(30);
    assert!((iteration.hero.elarion().focus - 0.2).abs() < 1e-12);
    // One large advance and split advances must account for the same pulses.
    let mut batched = Iteration::new(&profile, &apl, 1, 1);
    batched.hero.elarion_mut().focus = 0.0;
    batched.hero.elarion_mut().skystriders_grace_until = 20;
    batched.process_events_through(30);
    assert!((batched.hero.elarion().focus - iteration.hero.elarion().focus).abs() < 1e-12);
}

#[test]
fn elarion_focus_pulses_continue_during_casting_without_resetting_phase() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.cast_time_ms = 25;
    focused.off_gcd = true;
    focused.first_hit_delay_ms = 100;
    focused.primary_resource_generated = 20.0;
    focused.mechanic_parameters.extend([
        ("focusRegenerationPerPulse".into(), 0.05),
        ("focusRegenerationIntervalSeconds".into(), 0.01),
    ]);
    let profile = elarion_profile([focused]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().focus = 0.0;
    iteration.prepare_cast(0);
    assert_eq!(iteration.common.now_ms, 25);
    assert!((iteration.hero.elarion().focus - 20.1).abs() < 1e-12);
    iteration.process_events_through(30);
    assert!((iteration.hero.elarion().focus - 20.15).abs() < 1e-12);
    assert_eq!(iteration.ability_totals("test:focused-shot").hits, 0);
}

#[test]
fn elarion_focus_affordability_waits_for_the_next_whole_pulse() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.mechanic_parameters.extend([
        ("focusRegenerationPerPulse".into(), 0.05),
        ("focusRegenerationIntervalSeconds".into(), 0.01),
    ]);
    let mut multishot = elarion_ability(DpsAbilityKind::Multishot);
    multishot
        .mechanic_parameters
        .insert("focusCost".into(), 0.075);
    let profile = elarion_profile([focused, multishot]);
    let apl = apl([("multishot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().focus = 0.0;
    iteration.process_events_through(7);
    assert_eq!(iteration.next_interesting_time(), 20);
    iteration.process_events_through(19);
    assert_eq!(iteration.choose_action(), None);
    iteration.process_events_through(20);
    assert_eq!(iteration.choose_action(), Some(1));
}

#[test]
fn elarion_strikers_aim_uses_missing_ricochets_and_decays_one_stack_at_a_time() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]);
    profile.talents.extend([
        elarion_talent(
            11,
            [
                ("expertisePerStack", 0.02),
                ("maximumStacks", 3.0),
                ("decayIntervalSeconds", 2.0),
            ],
        ),
        elarion_talent(15, [("damageMultiplier", 2.0), ("maximumBounces", 9.0)]),
    ]);
    let apl = apl([("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 3);
    iteration.hero.elarion_mut().highwind_casts = 2;

    iteration.cast(0);

    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 3);
    assert!((iteration.effective_expertise() - 0.06).abs() < 1e-10);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 2);
    iteration.process_events_through(4_000);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 1);
    iteration.process_events_through(6_000);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 0);
}

#[test]
fn elarion_celestial_impetus_resets_and_empowers_heartseeker() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        elarion_ability(DpsAbilityKind::LunarlightMark),
        elarion_ability(DpsAbilityKind::HeartseekerBarrage),
    ]);
    profile.talents.push(elarion_talent(
        14,
        [
            ("damageIncreasePerProjectile", 0.1),
            ("durationSeconds", 15.0),
        ],
    ));
    let apl = apl([("celestial-shot", None), ("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 4);
    iteration.hero.elarion_mut().celestial_impetus_stacks = 1;
    iteration.hero.elarion_mut().celestial_impetus_until = 10_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::HeartseekerBarrage,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );

    iteration.cast(0);

    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::HeartseekerBarrage),
        0
    );
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 2);
    assert!(iteration.hero.elarion().impending_heartseeker_until > iteration.common.now_ms);

    iteration.cast(2);

    assert_eq!(iteration.hero.elarion().impending_heartseeker_until, 0);
    assert_eq!(
        iteration.ability_totals("test:heartseeker-barrage").damage,
        330.0
    );
}

#[test]
fn elarion_resurgent_winds_stacks_restores_charges_and_empowers_each_highwind() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HighwindArrow),
        elarion_ability(DpsAbilityKind::LunarlightMark),
    ]);
    profile.talents.push(elarion_talent(
        3,
        [
            ("durationSeconds", 15.0),
            ("offensiveProcChance", 0.0),
            ("stacks", 1.0),
            ("maximumStacks", 2.0),
        ],
    ));
    let apl = apl([("lunarlight-mark", None), ("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 5);
    iteration.common.cooldowns.insert(
        DpsAbilityKind::HighwindArrow,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );

    iteration.cast(1);
    assert_eq!(iteration.hero.elarion().resurgent_winds_stacks, 1);
    assert_eq!(
        iteration.cooldown_remaining_ms(DpsAbilityKind::HighwindArrow),
        0
    );
    iteration.cast(1);
    assert_eq!(iteration.hero.elarion().resurgent_winds_stacks, 2);

    iteration.cast(0);

    assert_eq!(iteration.hero.elarion().resurgent_winds_stacks, 1);
    assert_eq!(iteration.hero.elarion().focus, 100.0);
    assert_eq!(
        iteration.ability_totals("test:highwind-arrow").damage,
        150.0
    );
}

#[test]
fn elarion_heartseeker_only_erupts_after_triggering_lunarlight_salvo() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 0.0);
    mark.mechanic_parameters
        .insert("salvoCriticalProcChance".into(), 0.0);
    let heartseeker = elarion_ability(DpsAbilityKind::HeartseekerBarrage);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let profile = elarion_profile([
        heartseeker,
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
        elarion_ability(DpsAbilityKind::LunarlightEruption),
    ]);
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 7);
    iteration.apply_elarion_mark(0, 1);

    iteration.cast(0);

    assert_eq!(iteration.ability_totals("test:lunarlight-eruption").hits, 0);
}

#[test]
fn elarion_shoot_cannot_trigger_lunarlight_salvo() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::ElarionShoot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
    ]);
    let apl = apl([("elarion-shoot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 70);
    iteration.apply_elarion_mark(0, 1);
    let shoot = iteration
        .ability(DpsAbilityKind::ElarionShoot)
        .unwrap()
        .clone();

    iteration.damage_hit(&shoot, 100.0, 0.0, false, 0, DamageContext::NONE);
    iteration.process_events_through(1_000);

    assert_eq!(iteration.hero.elarion().mark_stacks[0], 1);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 0);
}

#[test]
fn elarion_lunarlight_salvo_applies_immediately_and_uses_the_cooked_random_stream() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
    ]);
    profile.haste = 1.0;
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 71);
    iteration.apply_elarion_mark(0, 1);
    let celestial = iteration
        .ability(DpsAbilityKind::CelestialShot)
        .unwrap()
        .clone();

    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);

    assert_eq!(iteration.hero.elarion().mark_stacks[0], 0);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Bowguy.IntantMarkTarget.TriggerAdditionalHit")
    );
    assert!(
        !iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Bowguy.LunarlightSalvo")
    );
    assert!(
        iteration
            .common
            .queue
            .iter()
            .all(|event| matches!(event.0.kind, EventKind::Core(CoreEvent::PassiveSpiritRegen)))
    );
    iteration.process_events_through(350);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
    assert_eq!(iteration.test_proc_count("test:lunarlight-salvo"), 1);
}

#[test]
fn elarion_lunarlight_monitor_accepts_non_auto_proc_damage() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let profile = elarion_profile([mark, elarion_ability(DpsAbilityKind::LunarlightSalvo)]);
    let apl = apl([("lunarlight-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 72);
    iteration.apply_elarion_mark(0, 1);
    let source =
        iteration.ability_damage_source(iteration.ability(DpsAbilityKind::LunarlightMark).unwrap());

    iteration.damage_unscaled_key(
        None,
        source,
        10.0,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    iteration.process_events_through(350);

    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
}

#[test]
fn elarion_lunar_fury_empowers_salvos_from_every_valid_damage_source() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
    ]);
    profile.talents.push(elarion_talent(
        18,
        [("procChanceMultiplier", 2.0), ("damageMultiplier", 1.3)],
    ));
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 73);
    iteration.apply_elarion_mark(0, 1);
    let celestial = iteration
        .ability(DpsAbilityKind::CelestialShot)
        .unwrap()
        .clone();

    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);
    iteration.process_events_through(350);

    assert_eq!(
        iteration.ability_totals("test:lunarlight-salvo").damage,
        130.0
    );
}

#[test]
fn elarion_eruption_uses_salvo_talents_and_hits_only_secondary_targets() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let heartseeker = elarion_ability(DpsAbilityKind::HeartseekerBarrage);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let mut profile = elarion_profile([
        heartseeker,
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
        elarion_ability(DpsAbilityKind::LunarlightEruption),
    ]);
    profile.talents.extend([
        elarion_talent(10, [("criticalStrikeBonus", 1.0)]),
        elarion_talent(
            18,
            [("procChanceMultiplier", 2.0), ("damageMultiplier", 1.3)],
        ),
    ]);
    let apl = apl([("heartseeker-barrage", None)]);

    let mut single = Iteration::new(&profile, &apl, 1, 74);
    single.apply_elarion_mark(0, 1);
    let heartseeker = single
        .ability(DpsAbilityKind::HeartseekerBarrage)
        .unwrap()
        .clone();
    single.damage_hit(&heartseeker, 100.0, 0.0, false, 0, DamageContext::NONE);
    single.process_events_through(350);
    assert_eq!(single.ability_totals("test:lunarlight-eruption").hits, 0);

    let mut stacked = Iteration::new(&profile, &apl, 3, 75);
    stacked.apply_elarion_mark(1, 1);
    let heartseeker = stacked
        .ability(DpsAbilityKind::HeartseekerBarrage)
        .unwrap()
        .clone();
    stacked.damage_hit(&heartseeker, 100.0, 0.0, false, 1, DamageContext::NONE);
    stacked.process_events_through(350);

    assert_eq!(
        stacked.ability_totals("test:lunarlight-salvo").damage,
        260.0
    );
    assert_eq!(stacked.ability_totals("test:lunarlight-eruption").hits, 2);
    assert_eq!(
        stacked.ability_totals("test:lunarlight-eruption").damage,
        520.0
    );
    assert!(
        stacked
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Bowguy.IntantMarkTarget.TriggerAoeHit")
    );
}

#[test]
fn elarion_non_heartseeker_salvo_advances_eruption_stream_without_eruption_damage() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
        elarion_ability(DpsAbilityKind::LunarlightEruption),
    ]);
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 76);
    iteration.apply_elarion_mark(0, 1);
    let celestial = iteration
        .ability(DpsAbilityKind::CelestialShot)
        .unwrap()
        .clone();

    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);
    iteration.process_events_through(350);

    assert!(
        iteration
            .shared
            .controlled_random_states
            .contains_key("RandomStream.Bowguy.IntantMarkTarget.TriggerAoeHit")
    );
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
    assert_eq!(iteration.ability_totals("test:lunarlight-eruption").hits, 0);
}

#[test]
fn elarion_mark_preservation_observes_immediate_salvo_reentry() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::StarfallVolley),
        elarion_ability(DpsAbilityKind::CelestialShot),
        elarion_ability(DpsAbilityKind::ElarionShoot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
    ]);
    profile
        .talents
        .push(elarion_talent(10, [("criticalStrikeBonus", 0.2)]));
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 77);
    iteration.apply_elarion_mark(0, 5);
    let starfall = iteration
        .ability(DpsAbilityKind::StarfallVolley)
        .unwrap()
        .clone();
    let celestial = iteration
        .ability(DpsAbilityKind::CelestialShot)
        .unwrap()
        .clone();
    let shoot = iteration
        .ability(DpsAbilityKind::ElarionShoot)
        .unwrap()
        .clone();

    // Salvo re-enters the monitor before stack removal, seeing Starfall tags.
    iteration.damage_hit(&starfall, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 5);
    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 4);
    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 3);
    iteration.damage_hit(&starfall, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 3);
    // Auto-attacks cannot proc Salvo but still replace the monitor's cached tags.
    iteration.damage_hit(&shoot, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 3);
    iteration.damage_hit(&celestial, 100.0, 0.0, false, 0, DamageContext::NONE);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 2);
}

#[test]
fn elarion_lunarlight_mark_uses_only_its_guaranteed_resurgent_winds_grant() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HighwindArrow),
        elarion_ability(DpsAbilityKind::LunarlightMark),
    ]);
    profile.talents.push(elarion_talent(
        3,
        [
            ("durationSeconds", 15.0),
            ("offensiveProcChance", 1.0),
            ("stacks", 1.0),
            ("maximumStacks", 2.0),
        ],
    ));
    let apl = apl([("lunarlight-mark", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);

    iteration.cast(1);

    assert_eq!(iteration.hero.elarion().resurgent_winds_stacks, 1);
}

#[test]
fn elarion_ability_categories_follow_current_cooked_tags() {
    assert_eq!(
        ability_category(DpsAbilityKind::Multishot),
        AbilityCategory::Core
    );
    assert_eq!(
        ability_category(DpsAbilityKind::HighwindArrow),
        AbilityCategory::Power
    );
    assert_eq!(
        ability_category(DpsAbilityKind::HeartseekerBarrage),
        AbilityCategory::Power
    );
    assert_eq!(
        ability_category(DpsAbilityKind::CelestialShot),
        AbilityCategory::Basic
    );
}

#[test]
fn elarion_highwind_grants_and_multishot_consumes_its_authored_proc_charge() {
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HighwindArrow),
        elarion_ability(DpsAbilityKind::Multishot),
    ]);
    let apl = apl([("highwind-arrow", None), ("multishot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 10);

    iteration.cast(0);
    assert_eq!(iteration.hero.elarion().multishot_proc_stacks, 1);
    assert_eq!(iteration.test_proc_count("test:highwind-arrow"), 0);

    iteration.cast(1);

    assert_eq!(iteration.hero.elarion().multishot_proc_stacks, 0);
    assert_eq!(iteration.ability_totals("test:multishot").hits, 3);
    assert_eq!(iteration.ability_totals("test:multishot").damage, 600.0);
}

#[test]
fn elarion_multishot_proc_requires_three_highwind_targets_and_caps_at_five() {
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HighwindArrow),
        elarion_ability(DpsAbilityKind::Multishot),
    ]);
    let apl = apl([("highwind-arrow", None), ("multishot", None)]);
    let mut single_target = Iteration::new(&profile, &apl, 1, 11);

    single_target.cast(0);
    assert_eq!(single_target.hero.elarion().multishot_proc_stacks, 0);

    let mut stacked_targets = Iteration::new(&profile, &apl, 3, 12);
    for _ in 0..6 {
        stacked_targets.cast(0);
    }
    assert_eq!(stacked_targets.hero.elarion().multishot_proc_stacks, 5);
}

#[test]
fn elarion_focused_expanse_accumulates_up_to_its_authored_stack_cap() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]);
    profile.talents.push(elarion_talent(
        7,
        [
            ("procChance", 1.0),
            ("damageMultiplier", 1.2),
            ("stacks", 1.0),
            ("maximumStacks", 2.0),
            ("durationSeconds", 15.0),
        ],
    ));
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 9);

    iteration.cast(0);
    iteration.cast(0);
    iteration.cast(0);

    assert_eq!(iteration.hero.elarion().empowered_multishot_stacks, 2);
    assert_eq!(iteration.test_proc_count("bowguy-talent-id-talent7"), 3);
}

#[test]
fn elarion_deadly_focus_changes_both_attacks_but_only_focused_shot_resource() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.primary_resource_generated = 20.0;
    let celestial = elarion_ability(DpsAbilityKind::CelestialShot);
    let mut profile = elarion_profile([focused, celestial]);
    profile.talents.push(elarion_talent(
        17,
        [("damageMultiplier", 2.0), ("resourceMultiplier", 0.5)],
    ));
    let apl = apl([("focused-shot", None), ("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 6);
    iteration.hero.elarion_mut().focus = 0.0;

    iteration.cast(0);
    iteration.cast(1);

    assert_eq!(iteration.ability_totals("test:focused-shot").damage, 200.0);
    assert_eq!(
        iteration.ability_totals("test:celestial-shot").damage,
        200.0
    );
    assert_eq!(iteration.hero.elarion().focus, 10.0);
}

#[test]
fn elarion_refund_delays_focus_and_marks_only_native_selected_targets() {
    for targets in [1, 2, 3, 5] {
        let mut profile = elarion_profile([
            elarion_ability(DpsAbilityKind::CelestialShot),
            elarion_ability(DpsAbilityKind::LunarlightMark),
        ]);
        profile.spirit = 1.0;
        let apl = apl([("celestial-shot", None)]);
        let mut iteration = Iteration::new(&profile, &apl, targets, 37);
        let ability = iteration
            .ability(DpsAbilityKind::CelestialShot)
            .unwrap()
            .clone();
        iteration.hero.elarion_mut().focus = 40.0;
        iteration.shared.controlled_random_states.insert(
            SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
            ControlledRandomState {
                failure_threshold: 0.0,
                chance_factor: 0.5,
                chance_bucket: 50,
            },
        );
        iteration.try_elarion_spirit_refund(&ability, 30.0);
        assert_eq!(iteration.shared.spirit, 1.0);
        assert_eq!(iteration.hero.elarion().focus, 40.0);
        assert_eq!(iteration.hero.elarion().mark_stacks[0], 5);
        for target in 1..targets {
            assert_eq!(
                iteration.hero.elarion().mark_stacks[target as usize],
                if targets > 2 && target < 3 { 2 } else { 0 }
            );
        }
        iteration.process_events_through(199);
        assert_eq!(iteration.hero.elarion().focus, 40.0);
        iteration.process_events_through(200);
        assert_eq!(iteration.hero.elarion().focus, 70.0);
        iteration.hero.elarion_mut().focus = profile.max_primary_resource - 1.0;
        iteration.shared.controlled_random_states.insert(
            SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
            ControlledRandomState {
                failure_threshold: 0.0,
                chance_factor: 0.5,
                chance_bucket: 50,
            },
        );
        iteration.try_elarion_spirit_refund(&ability, 30.0);
        iteration.process_events_through(400);
        assert_eq!(iteration.hero.elarion().focus, profile.max_primary_resource);
    }
}

#[test]
fn elarion_refund_does_not_revive_expired_mark_stacks_or_roll_without_spending() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        elarion_ability(DpsAbilityKind::LunarlightMark),
    ]);
    profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::LunarlightMark)
        .unwrap()
        .mechanic_parameters
        .insert("maximumStacks".into(), 20.0);
    profile.spirit = 1.0;
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 37);
    let ability = iteration
        .ability(DpsAbilityKind::CelestialShot)
        .unwrap()
        .clone();
    iteration.hero.elarion_mut().mark_stacks[0] = 8;
    iteration.hero.elarion_mut().mark_until[0] = 0;
    iteration.shared.controlled_random_states.insert(
        SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
        ControlledRandomState {
            failure_threshold: 0.0,
            chance_factor: 0.5,
            chance_bucket: 50,
        },
    );
    iteration.try_elarion_spirit_refund(&ability, 0.0);
    assert_eq!(iteration.test_proc_count("spirit-refund"), 0);
    iteration.try_elarion_spirit_refund(&ability, 30.0);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 5);
}

#[test]
fn elarion_eruption_precedes_salvo_and_scales_only_secondary_target_count() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let mut salvo = elarion_ability(DpsAbilityKind::LunarlightSalvo);
    salvo.damage_spread = 0.2;
    let mut eruption = elarion_ability(DpsAbilityKind::LunarlightEruption);
    eruption.damage_spread = 0.2;
    eruption
        .mechanic_parameters
        .insert("targetCountDamageScalingThreshold".into(), 1.0);
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HeartseekerBarrage),
        mark,
        salvo,
        eruption,
    ]);
    profile.critical_strike = 0.3;
    let apl = apl([("heartseeker-barrage", None)]);
    for seed in 1..=20 {
        let mut actual = Iteration::new(&profile, &apl, 3, seed);
        let mut expected = Iteration::new(&profile, &apl, 3, seed);
        actual.apply_elarion_mark(1, 5);
        let source = actual
            .ability_damage_source(actual.ability(DpsAbilityKind::HeartseekerBarrage).unwrap());
        actual.try_elarion_mark_proc(source, 1, false, 100.0, DamageContext::NONE);

        expected.roll_controlled_random_bool(
            "RandomStream.Bowguy.IntantMarkTarget.TriggerAdditionalHit",
            1.0,
        );
        expected
            .roll_controlled_random_bool("RandomStream.Bowguy.IntantMarkTarget.TriggerAoeHit", 1.0);
        let eruption = expected
            .ability(DpsAbilityKind::LunarlightEruption)
            .unwrap()
            .clone();
        let salvo = expected
            .ability(DpsAbilityKind::LunarlightSalvo)
            .unwrap()
            .clone();
        for target in [0, 2] {
            expected.damage_hit(
                &eruption,
                100.0 / 2_f64.sqrt(),
                0.0,
                true,
                target,
                DamageContext::NONE,
            );
        }
        expected.damage_hit(&salvo, 100.0, 0.0, true, 1, DamageContext::NONE);
        assert_eq!(actual.common.result.targets, expected.common.result.targets);
        assert_eq!(actual.hero.elarion().mark_stacks, vec![0, 4, 0]);
        assert_eq!(actual.test_proc_count("test:lunarlight-salvo"), 1);
        assert_eq!(actual.test_proc_count("test:lunarlight-eruption"), 1);
        assert!(
            actual.common.queue.iter().all(|event| matches!(
                event.0.kind,
                EventKind::Core(CoreEvent::PassiveSpiritRegen)
            ))
        );
    }
}

#[test]
fn elarion_lunarlight_requires_live_marks_positive_damage_and_uses_critical_chance() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 0.0);
    mark.mechanic_parameters
        .insert("salvoCriticalProcChance".into(), 1.0);
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::CelestialShot),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
    ]);
    let apl = apl([("celestial-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 82);
    let source =
        iteration.ability_damage_source(iteration.ability(DpsAbilityKind::CelestialShot).unwrap());
    iteration.apply_elarion_mark(0, 2);
    iteration.try_elarion_mark_proc(source, 1, true, 100.0, DamageContext::NONE);
    iteration.try_elarion_mark_proc(source, 0, true, 0.0, DamageContext::NONE);
    iteration.try_elarion_mark_proc(source, 0, false, 100.0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 0);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 2);
    iteration.try_elarion_mark_proc(source, 0, true, 100.0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
    assert_eq!(iteration.hero.elarion().mark_stacks[0], 1);
    iteration.common.now_ms = 10_000;
    iteration.try_elarion_mark_proc(source, 0, true, 100.0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
}

#[test]
fn elarion_both_lunarlight_specs_snapshot_before_eruption_activates_first_strike() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HeartseekerBarrage),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
        elarion_ability(DpsAbilityKind::LunarlightEruption),
    ]);
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
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 83);
    iteration.apply_elarion_mark(1, 5);
    let source = iteration.ability_damage_source(
        iteration
            .ability(DpsAbilityKind::HeartseekerBarrage)
            .unwrap(),
    );
    iteration.try_elarion_mark_proc(source, 1, false, 100.0, DamageContext::NONE);
    assert_eq!(iteration.common.result.targets, vec![100.0, 100.0, 100.0]);
    assert_eq!(iteration.effective_expertise(), 0.5);
    assert!((iteration.shared.spirit - 300.0 / 6_637_912.0 * 11.25).abs() < 1e-7);
    assert_eq!(iteration.test_proc_count("test:lunarlight-salvo"), 1);
}

#[test]
fn elarion_nested_damage_updates_the_lunarlight_continuation_target_even_without_a_mark() {
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.mechanic_parameters
        .insert("salvoProcChance".into(), 1.0);
    mark.mechanic_parameters
        .insert("eruptionProcChance".into(), 1.0);
    let mut shadow = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    shadow.mechanic_parameters = BTreeMap::from([
        ("accumulationFraction".into(), 1.0),
        ("maximumPowerCoefficient".into(), 1.0),
    ]);
    let profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HeartseekerBarrage),
        mark,
        elarion_ability(DpsAbilityKind::LunarlightSalvo),
        elarion_ability(DpsAbilityKind::LunarlightEruption),
        shadow,
    ]);
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 84);
    iteration.apply_elarion_mark(1, 5);
    iteration.shared.shadow_marks.insert(
        0,
        ShadowMarkState {
            generation: 1,
            until_ms: 15_000,
            accumulated_damage: 0.0,
            context: DamageContext::for_cast(1),
        },
    );
    let source = iteration.ability_damage_source(
        iteration
            .ability(DpsAbilityKind::HeartseekerBarrage)
            .unwrap(),
    );
    iteration.try_elarion_mark_proc(source, 1, false, 100.0, DamageContext::NONE);
    // Eruption's hit on target 0 explodes Shadow Mark. That nested event updates
    // CachedDamagedTargetCharacter before its missing-Lunarlight-mark rejection.
    // Salvo and the later mark lookup therefore use 0, not the original target 1.
    assert_eq!(iteration.common.result.targets, vec![300.0, 0.0, 100.0]);
    assert_eq!(iteration.hero.elarion().mark_stacks, vec![0, 5, 0]);
    assert_eq!(iteration.ability_totals("test:lunarlight-salvo").hits, 1);
    assert_eq!(iteration.ability_totals("test:weapon-shadow-mark").hits, 1);
}

#[test]
fn starstriker_self_refund_resets_committed_cooldown_and_empowers_next_channel() {
    let mut heartseeker = elarion_ability(DpsAbilityKind::HeartseekerBarrage);
    heartseeker
        .mechanic_parameters
        .insert("focusCost".into(), 10.0);
    let mut profile = elarion_profile([heartseeker]);
    profile.spirit = 1.0;
    profile.mechanics.push(ardeos_mechanic(
        "legendary-bowguy-trait2",
        [
            ("powerMultiplier", 1.0),
            ("cooldownAccelerationMultiplier", 1.0),
            ("starstrikerProcChance", 1.0),
            ("impendingHeartseekerDurationSeconds", 15.0),
        ],
    ));
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 302);
    for expected_damage in [300.0, 630.0] {
        iteration.shared.controlled_random_states.insert(
            SPIRIT_PROC_RANDOM_STREAM_TAG.into(),
            ControlledRandomState {
                failure_threshold: 0.0,
                chance_factor: 1.0,
                chance_bucket: 50,
            },
        );
        iteration.cast(0);
        assert_eq!(
            iteration.cooldown_remaining_ms(DpsAbilityKind::HeartseekerBarrage),
            0
        );
        assert!(iteration.hero.elarion().impending_heartseeker_until > iteration.common.now_ms);
        assert_eq!(
            iteration.ability_totals("test:heartseeker-barrage").damage,
            expected_damage
        );
    }
}

#[test]
fn astronomers_hail_extends_each_live_actor_and_only_doubles_primary_damage() {
    for targets in [1, 3] {
        let source = dot_ability(DpsAbilityKind::StarfallVolley, 1.0);
        let mut profile = elarion_profile([source, elarion_ability(DpsAbilityKind::Multishot)]);
        profile.mechanics.push(ardeos_mechanic(
            "legendary-bowguy-trait5",
            [
                ("powerMultiplier", 1.0),
                ("cooldownAccelerationMultiplier", 1.0),
                ("starfallMainTargetDamageMultiplier", 2.0),
                ("starfallDurationIncreaseSeconds", 3.0),
                ("starfallExtensionPerMultishotSeconds", 0.25),
            ],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, targets, 303);
        for at in [0, 500] {
            iteration.common.now_ms = at;
            for target in 0..targets {
                iteration.impact(0, false, 1.0, target, DamageContext::for_cast(at + 1));
            }
        }
        let multishot = iteration
            .ability(DpsAbilityKind::Multishot)
            .unwrap()
            .clone();
        iteration.commit_elarion_ability(&multishot);
        for target in 0..targets {
            let mut expiries = iteration
                .dot_instances(target, DpsAbilityKind::StarfallVolley)
                .map(|(_, d)| d.expires_ms)
                .collect::<Vec<_>>();
            expiries.sort();
            assert_eq!(expiries, [13_250, 13_750]);
        }
        let generations = (0..targets)
            .flat_map(|target| {
                iteration
                    .dot_instances(target, DpsAbilityKind::StarfallVolley)
                    .map(move |(_, d)| (target, d.generation))
            })
            .collect::<Vec<_>>();
        for (target, generation) in generations {
            iteration.starfall_hit(generation, target);
        }
        assert_eq!(
            iteration.ability_totals("test:starfall-volley").damage,
            200.0 * f64::from(targets + 1)
        );
        iteration.common.now_ms = 13_300;
        iteration.commit_elarion_ability(&multishot);
        for target in 0..targets {
            let mut expiries = iteration
                .dot_instances(target, DpsAbilityKind::StarfallVolley)
                .map(|(_, d)| d.expires_ms)
                .collect::<Vec<_>>();
            expiries.sort();
            assert_eq!(
                expiries,
                [13_250, 14_000],
                "expired actors cannot be revived by an extension"
            );
        }
    }
}

#[test]
fn shimmer_stacks_linearly_per_target_caps_and_resets_after_expiration() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]);
    profile.mechanics.push(ardeos_mechanic(
        "legendary-shimmer",
        [
            ("shimmerDamageMultiplier", 1.08),
            ("shimmerDurationSeconds", 9.0),
            ("shimmerMaximumStacks", 3.0),
        ],
    ));
    let apl = apl([("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 35);
    for expected in [1.08, 1.16, 1.24, 1.24] {
        iteration.impact(0, true, 1.0, 0, DamageContext::for_cast(1));
        assert!((iteration.target_damage_multiplier(0) - expected).abs() < 1e-9);
        assert_eq!(iteration.target_damage_multiplier(1), 1.0);
    }
    iteration.process_events_through(9_000);
    assert_eq!(iteration.target_damage_multiplier(0), 1.0);
    iteration.impact(0, true, 1.0, 0, DamageContext::for_cast(2));
    assert!((iteration.target_damage_multiplier(0) - 1.08).abs() < 1e-9);
}

#[test]
fn elarion_focused_shot_grants_focus_and_expanse_before_projectile_arrival() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.cast_time_ms = 200;
    focused.first_hit_delay_ms = 2_000;
    focused.primary_resource_generated = 20.0;
    let mut profile = elarion_profile([focused]);
    profile.talents.extend([
        elarion_talent(
            7,
            [
                ("procChance", 1.0),
                ("damageMultiplier", 1.2),
                ("stacks", 1.0),
                ("maximumStacks", 2.0),
                ("durationSeconds", 15.0),
            ],
        ),
        elarion_talent(17, [("damageMultiplier", 2.0), ("resourceMultiplier", 0.5)]),
    ]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 71);
    iteration.hero.elarion_mut().focus = 0.0;

    iteration.cast(0);

    assert_eq!(iteration.hero.elarion().focus, 10.0);
    assert_eq!(iteration.hero.elarion().empowered_multishot_stacks, 1);
    assert_eq!(iteration.ability_totals("test:focused-shot").hits, 0);
    assert!(iteration.shared.controlled_random_states.contains_key(
        "RandomStream.Bowguy.Talent.CastedProjectileDamage.AoeProjectileDamageEmpowered"
    ));
    iteration.process_events_through(2_200);
    assert_eq!(iteration.ability_totals("test:focused-shot").hits, 1);
    assert_eq!(iteration.hero.elarion().focus, 10.0);
    assert_eq!(iteration.test_proc_count("bowguy-talent-id-talent7"), 1);
}

#[test]
fn elarion_focused_projectile_rolls_on_zero_damage_and_restarts_expired_impetus() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.mechanic_parameters.extend([
        ("celestialImpetusProcsPerMinute".into(), 600.0),
        ("celestialImpetusDurationSeconds".into(), 15.0),
        ("celestialImpetusMaximumStacks".into(), 2.0),
    ]);
    let profile = elarion_profile([focused]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 72);
    let focused = iteration
        .ability(DpsAbilityKind::FocusedShot)
        .unwrap()
        .clone();
    let stream = "RandomStream.Bowguy.RangedAutoAttack.ProcChance";
    iteration.hero.elarion_mut().celestial_impetus_stacks = 2;
    iteration.hero.elarion_mut().celestial_impetus_until = 1;
    for (time, stacks) in [(1_000, 1), (2_000, 2), (3_000, 2)] {
        iteration.advance_to(time);
        iteration.shared.proc_per_minute_states.insert(
            stream.into(),
            ProcPerMinuteState {
                last_roll_seconds: 0.0,
                last_proc_seconds: 0.0,
                has_procced: true,
            },
        );
        iteration.resolve_elarion_impact(&focused, 0, DamageOutcome::default());
        assert_eq!(iteration.hero.elarion().celestial_impetus_stacks, stacks);
        assert_eq!(
            iteration.hero.elarion().celestial_impetus_until,
            time + 15_000
        );
    }
    assert_eq!(iteration.test_proc_count("test:focused-shot"), 3);
    assert!(
        !iteration
            .shared
            .proc_per_minute_states
            .contains_key("RandomStream.Bowguy.FocusedShot.CelestialImpetus")
    );
}

#[test]
fn elarion_celestial_proc_marks_after_damage_and_retains_the_launch_proc() {
    for previous_stacks in [0, 1] {
        let mut celestial = elarion_ability(DpsAbilityKind::CelestialShot);
        celestial.first_hit_delay_ms = 2_000;
        let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
        mark.mechanic_parameters
            .insert("salvoProcChance".into(), 1.0);
        let profile = elarion_profile([
            celestial,
            mark,
            elarion_ability(DpsAbilityKind::LunarlightSalvo),
        ]);
        let apl = apl([("celestial-shot", None)]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 73);
        if previous_stacks > 0 {
            iteration.apply_elarion_mark(0, previous_stacks);
        }
        iteration.hero.elarion_mut().celestial_impetus_stacks = 1;
        iteration.hero.elarion_mut().celestial_impetus_until = 1_000;

        iteration.cast(0);
        assert_eq!(iteration.hero.elarion().celestial_impetus_stacks, 0);
        assert_eq!(iteration.hero.elarion().mark_stacks[0], previous_stacks);
        iteration.process_events_through(2_000);
        assert_eq!(
            iteration.ability_totals("test:lunarlight-salvo").hits,
            previous_stacks as u64
        );
        assert_eq!(iteration.hero.elarion().mark_stacks[0], 2);
    }
}

#[test]
fn elarion_resurgent_offensive_monitor_uses_its_five_ability_query() {
    for kind in [
        DpsAbilityKind::FocusedShot,
        DpsAbilityKind::HeartseekerBarrage,
        DpsAbilityKind::CelestialShot,
        DpsAbilityKind::Multishot,
        DpsAbilityKind::StarfallVolley,
        DpsAbilityKind::HighwindArrow,
        DpsAbilityKind::ElarionShoot,
        DpsAbilityKind::LunarlightMark,
    ] {
        let mut profile = elarion_profile([elarion_ability(kind)]);
        profile.talents.push(elarion_talent(
            3,
            [
                ("durationSeconds", 15.0),
                ("offensiveProcChance", 1.0),
                ("stacks", 1.0),
                ("maximumStacks", 2.0),
            ],
        ));
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 74);
        let ability = iteration.ability(kind).unwrap().clone();
        iteration.commit_elarion_ability(&ability);
        let eligible = !matches!(
            kind,
            DpsAbilityKind::HighwindArrow
                | DpsAbilityKind::ElarionShoot
                | DpsAbilityKind::LunarlightMark
        );
        assert_eq!(
            iteration.hero.elarion().resurgent_winds_stacks,
            u32::from(eligible),
            "{kind:?}"
        );
        assert_eq!(
            iteration.shared.controlled_random_states.contains_key(
                "RandomStream.Bowguy.Talent.CastedProjectileHeavyDamage.InstantNoCooldownProc"
            ),
            eligible,
            "{kind:?}"
        );
    }
}

#[test]
fn elarion_shoot_starts_after_targeted_cast_and_keeps_its_own_clock() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.cast_time_ms = 500;
    focused.gcd_ms = 0;
    let mut shoot = elarion_ability(DpsAbilityKind::ElarionShoot);
    shoot.first_hit_delay_ms = 395;
    let profile = elarion_profile([focused, shoot]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.process_events_through(1_000);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 0);
    iteration.cast(0);
    assert_eq!(iteration.common.now_ms, 1_500);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 1);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").hits, 0);
    iteration.process_events_through(3_899);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 1);
    iteration.process_events_through(3_900);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 2);
}

#[test]
fn elarion_shoot_waits_for_channels_without_accumulating_overdue_attacks() {
    let mut barrage = elarion_ability(DpsAbilityKind::HeartseekerBarrage);
    barrage.channel.as_mut().unwrap().duration_ms = 10_000;
    barrage.channel.as_mut().unwrap().tick_interval_ms = 2_000;
    barrage.gcd_ms = 0;
    let profile = elarion_profile([barrage, elarion_ability(DpsAbilityKind::ElarionShoot)]);
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().auto_attacking = true;
    iteration.process_events_through(0);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 1);
    iteration.cast(0);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 2);
    iteration.process_events_through(12_399);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 2);
    iteration.process_events_through(12_400);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 3);
}

#[test]
fn elarion_shoot_integrates_live_haste_without_resetting_phase() {
    let mut grace = elarion_ability(DpsAbilityKind::SkystridersGrace);
    grace.mechanic_parameters.insert("hasteBonus".into(), 1.0);
    let profile = elarion_profile([elarion_ability(DpsAbilityKind::ElarionShoot), grace]);
    let apl = apl([("skystriders-grace", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().auto_attacking = true;
    iteration.process_events_through(600);
    iteration.hero.elarion_mut().skystriders_grace_until = 1_000;
    iteration.process_events_through(1_999);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 1);
    iteration.process_events_through(2_000);
    assert_eq!(iteration.ability_totals("test:elarion-shoot").casts, 2);
}

#[test]
fn elarion_highwind_rewards_arrival_and_equal_strikers_aim_refreshes_decay() {
    let mut highwind = elarion_ability(DpsAbilityKind::HighwindArrow);
    highwind.first_hit_delay_ms = 395;
    highwind.gcd_ms = 0;
    let mut profile = elarion_profile([highwind]);
    profile.talents.push(elarion_talent(
        11,
        [
            ("expertisePerStack", 0.02),
            ("maximumStacks", 3.0),
            ("decayIntervalSeconds", 2.0),
        ],
    ));
    let apl = apl([("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.cast(0);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 0);
    iteration.process_events_through(395);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 2);
    iteration.process_events_through(1_000);
    iteration.cast(0);
    iteration.process_events_through(2_395);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 2);
    iteration.process_events_through(3_395);
    assert_eq!(iteration.hero.elarion().strikers_aim_stacks, 1);
}

#[test]
fn elarion_final_crescendo_counts_total_targets_including_the_primary() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]);
    profile.talents.push(elarion_talent(
        15,
        [("damageMultiplier", 2.0), ("maximumBounces", 9.0)],
    ));
    let apl = apl([("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 20, 1);
    iteration.hero.elarion_mut().highwind_casts = 2;
    iteration.cast(0);
    assert_eq!(iteration.ability_totals("test:highwind-arrow").hits, 9);
}

#[test]
fn elarion_repeating_stars_observes_the_final_focused_expanse_charge() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::Multishot),
        elarion_ability(DpsAbilityKind::StarfallVolley),
    ]);
    profile.talents.push(elarion_talent(
        1,
        [
            ("cooldownReductionSeconds", 1.0),
            ("empoweredCooldownReductionSeconds", 3.0),
        ],
    ));
    let apl = apl([("multishot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 2);
    iteration.hero.elarion_mut().empowered_multishot_stacks = 1;
    iteration.hero.elarion_mut().empowered_multishot_until = 10_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::StarfallVolley,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    iteration.cast(0);
    assert_eq!(iteration.hero.elarion().empowered_multishot_stacks, 0);
    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::StarfallVolley].remaining_ms,
        7_000.0 - iteration.common.now_ms as f64
    );
}

#[test]
fn elarion_event_horizon_multiplies_recovery_without_scaling_unflagged_cooldowns() {
    let mut grace = elarion_ability(DpsAbilityKind::SkystridersGrace);
    grace.cooldown_scales_with_cooldown_recovery = true;
    let mut mark = elarion_ability(DpsAbilityKind::LunarlightMark);
    mark.cooldown_scales_with_cooldown_recovery = false;
    mark.cooldown_scales_with_haste = false;
    let mut profile = elarion_profile([grace, mark]);
    profile.haste = 0.5;
    profile.cooldown_recovery = 2.0;
    let apl = apl([("skystriders-grace", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().event_horizon_until = 1_000;
    assert_eq!(iteration.effective_cooldown_recovery(), 3.0);
    assert_eq!(
        iteration.cooldown_recovery_rate(DpsAbilityKind::LunarlightMark),
        1.0
    );
    iteration.process_events_through(1_000);
    assert_eq!(iteration.effective_cooldown_recovery(), 2.0);
}

#[test]
fn elarion_piercing_seekers_hits_extra_targets_but_only_primary_reduces_starfall() {
    let mut profile = elarion_profile([
        elarion_ability(DpsAbilityKind::HeartseekerBarrage),
        elarion_ability(DpsAbilityKind::StarfallVolley),
        elarion_ability(DpsAbilityKind::EventHorizon),
    ]);
    profile.talents.push(elarion_talent(
        2,
        [("additionalTargets", 2.0), ("damageMultiplier", 0.5)],
    ));
    profile.abilities[2].mechanic_parameters.extend([
        ("damageMultiplier".into(), 1.0),
        (
            "starfallCooldownReductionPerHeartseekerHitSeconds".into(),
            1.0,
        ),
    ]);
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 5, 1);
    iteration.hero.elarion_mut().event_horizon_until = 10_000;
    iteration.common.cooldowns.insert(
        DpsAbilityKind::StarfallVolley,
        CooldownState {
            remaining_ms: 10_000.0,
            used_charges: 1,
        },
    );
    iteration.cast(0);
    assert_eq!(iteration.ability_totals("test:heartseeker-barrage").hits, 9);
    assert_eq!(
        iteration.ability_totals("test:heartseeker-barrage").damage,
        600.0
    );
    assert_eq!(
        iteration.common.cooldowns[&DpsAbilityKind::StarfallVolley].remaining_ms,
        7_000.0 - iteration.common.now_ms as f64
    );
}

#[test]
fn elarion_projectiles_retain_event_horizon_damage_after_the_buff_expires() {
    let mut focused = elarion_ability(DpsAbilityKind::FocusedShot);
    focused.first_hit_delay_ms = 395;
    focused.gcd_ms = 0;
    let mut horizon = elarion_ability(DpsAbilityKind::EventHorizon);
    horizon
        .mechanic_parameters
        .insert("damageMultiplier".into(), 1.5);
    let profile = elarion_profile([focused, horizon]);
    let apl = apl([("focused-shot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.hero.elarion_mut().event_horizon_until = 100;
    iteration.cast(0);
    iteration.process_events_through(395);
    assert_eq!(iteration.ability_totals("test:focused-shot").damage, 150.0);
}

#[test]
fn elarion_empowered_multishot_pads_the_selected_target() {
    let mut multishot = elarion_ability(DpsAbilityKind::Multishot);
    multishot.first_hit_delay_ms = 395;
    multishot.gcd_ms = 0;
    let profile = elarion_profile([multishot]);
    let apl = apl([("multishot", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    iteration.hero.elarion_mut().skystriders_supremacy_until = 1_000;
    iteration.cast(0);
    let targets = iteration
        .common
        .queue
        .iter()
        .find_map(|event| match &event.0.kind {
            EventKind::Core(CoreEvent::ImpactBatch { impacts, .. }) => Some(
                impacts
                    .iter()
                    .map(|hit| hit.target_index)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap();
    assert_eq!(targets, [0, 1, 0]);
}

#[test]
fn elarion_fusillade_extends_the_channel_without_granting_last_lights_crit() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::HeartseekerBarrage)]);
    profile
        .talents
        .push(elarion_talent(5, [("durationIncreaseSeconds", 0.2)]));
    let apl = apl([("heartseeker-barrage", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.cast(0);
    assert_eq!(iteration.ability_totals("test:heartseeker-barrage").hits, 5);
    assert_eq!(
        iteration.ability_totals("test:heartseeker-barrage").damage,
        500.0
    );
}

#[test]
fn elarion_lethal_shots_uses_the_cooked_stream_and_one_roll_per_projectile() {
    let mut profile = elarion_profile([elarion_ability(DpsAbilityKind::HighwindArrow)]);
    profile.talents.push(elarion_talent(
        6,
        [("procChance", 1.0), ("criticalStrikeBonus", 1.0)],
    ));
    let apl = apl([("highwind-arrow", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    iteration.cast(0);
    assert!(iteration.shared.controlled_random_states.contains_key(
        "RandomStream.Bowguy.Talent.CastedProjectileHeavyDamage.ChanceToHaveIncreasedCritChance"
    ));
    assert_eq!(iteration.test_proc_count("bowguy-talent-id-talent6"), 1);
    assert_eq!(iteration.ability_totals("test:highwind-arrow").hits, 3);
}

#[test]
fn elarion_skylit_grace_does_not_require_grace_and_any_actor_end_removes_it() {
    let mut starfall = dot_ability(DpsAbilityKind::StarfallVolley, 1.0);
    starfall.dot.as_mut().unwrap().duration_ms = 2_000;
    starfall.gcd_ms = 0;
    let mut profile =
        elarion_profile([starfall, elarion_ability(DpsAbilityKind::SkystridersGrace)]);
    profile
        .talents
        .push(elarion_talent(4, [("cooldownAcceleration", 1.2)]));
    let apl = apl([("starfall-volley", None)]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.cast(0);
    assert_eq!(iteration.hero.elarion().skystriders_grace_until, 0);
    assert_eq!(
        iteration.cooldown_recovery_rate(DpsAbilityKind::SkystridersGrace),
        2.2
    );
    iteration.process_events_through(500);
    iteration.cast(0);
    iteration.process_events_through(2_000);
    assert!(
        iteration
            .dot_instances(0, DpsAbilityKind::StarfallVolley)
            .any(|(_, dot)| dot.expires_ms > 2_000)
    );
    assert_eq!(
        iteration.cooldown_recovery_rate(DpsAbilityKind::SkystridersGrace),
        1.0
    );
}

#[test]
fn default_elarion_spends_actual_spirit_cost_but_preserves_active_horizon() {
    let document: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/apl-builds/elarion/character.json"
    ))
    .unwrap();
    let mut build: crate::preparation::CharacterBuild =
        serde_json::from_value(document["build"].clone()).unwrap();
    // The shared default must skip unequipped weapon actions.
    build
        .positions
        .iter_mut()
        .find(|p| p.position_id == "weapon")
        .unwrap()
        .item = None;
    let (profile, _) = crate::preparation::prepare_character(&build, 1).unwrap();
    let apl = crate::parse_apl(&shipped_apl_source("elarion")).unwrap();
    for cost in [85.0, 100.0] {
        let mut profile = profile.clone();
        profile
            .abilities
            .iter_mut()
            .find(|a| a.kind == DpsAbilityKind::EventHorizon)
            .unwrap()
            .spirit_cost = cost;
        let mut iteration = Iteration::new(&profile, &apl, 1, 51);
        iteration.common.cooldowns.insert(
            DpsAbilityKind::SkystridersGrace,
            CooldownState {
                remaining_ms: 30_000.0,
                used_charges: 1,
            },
        );
        let horizon = iteration.common.abilities_by_kind[&DpsAbilityKind::EventHorizon];
        iteration.shared.spirit = cost - 0.01;
        assert_ne!(iteration.choose_action(), Some(horizon));
        iteration.shared.spirit = cost;
        assert!(cost < profile.max_spirit);
        assert_eq!(iteration.choose_action(), Some(horizon));
        iteration.hero.elarion_mut().event_horizon_until = 1_000;
        assert_ne!(iteration.choose_action(), Some(horizon));
        iteration.common.now_ms = 1_000;
        assert_eq!(iteration.choose_action(), Some(horizon));
    }
}

fn ranked_elarion_profile(highwind: bool) -> NormalizedDpsProfile {
    let source = if highwind {
        include_str!("../fixtures/apl-builds/elarion/variants/highwind.json")
    } else {
        include_str!("../fixtures/apl-builds/elarion/character.json")
    };
    let document: serde_json::Value = serde_json::from_str(source).unwrap();
    let build = serde_json::from_value(document["build"].clone()).unwrap();
    crate::preparation::prepare_character(&build, 1).unwrap().0
}

fn elarion_test_cooldown(iteration: &mut Iteration<'_>, kind: DpsAbilityKind) {
    iteration.common.cooldowns.insert(
        kind,
        CooldownState {
            remaining_ms: 30_000.0,
            used_charges: iteration.ability(kind).unwrap().maximum_charges,
        },
    );
}

#[test]
fn default_elarion_uses_mark_for_resurgent_winds_without_waiting_for_barrage() {
    let apl = crate::parse_apl(&shipped_apl_source("elarion")).unwrap();
    for resurgent in [false, true] {
        let profile = ranked_elarion_profile(resurgent);
        let mut iteration = Iteration::new(&profile, &apl, 1, 52);
        iteration.shared.spirit = 0.0;
        for kind in [
            DpsAbilityKind::SkystridersGrace,
            DpsAbilityKind::SkystridersSupremacy,
            DpsAbilityKind::HeartseekerBarrage,
        ] {
            elarion_test_cooldown(&mut iteration, kind);
        }
        let mark = iteration.common.abilities_by_kind[&DpsAbilityKind::LunarlightMark];
        assert_eq!(iteration.choose_action() == Some(mark), resurgent);
        iteration
            .common
            .cooldowns
            .remove(&DpsAbilityKind::HeartseekerBarrage);
        assert_eq!(iteration.choose_action(), Some(mark));
    }
}

#[test]
fn default_elarion_resets_committed_cooldowns_early_only_outside_crescendo_on_one_target() {
    let apl = crate::parse_apl(&shipped_apl_source("elarion")).unwrap();
    for crescendo in [false, true] {
        let profile = ranked_elarion_profile(crescendo);
        for targets in [1, 3, 5] {
            let mut iteration = Iteration::new(&profile, &apl, targets, 53);
            iteration.shared.spirit = 0.0;
            for kind in [
                DpsAbilityKind::SkystridersGrace,
                DpsAbilityKind::SkystridersSupremacy,
                DpsAbilityKind::LunarlightMark,
                DpsAbilityKind::StarfallVolley,
            ] {
                elarion_test_cooldown(&mut iteration, kind);
            }
            let chrono = iteration.common.abilities_by_kind[&DpsAbilityKind::WeaponArcaneChannel];
            let highwind = iteration.common.abilities_by_kind[&DpsAbilityKind::HighwindArrow];
            assert_eq!(
                iteration.choose_action(),
                Some(if targets == 1 && !crescendo {
                    chrono
                } else {
                    highwind
                })
            );
            iteration
                .common
                .cooldowns
                .remove(&DpsAbilityKind::StarfallVolley);
            assert_ne!(iteration.choose_action(), Some(chrono));
        }
    }
}
