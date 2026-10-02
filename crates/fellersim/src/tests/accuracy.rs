use super::*;

fn auto_profiles() -> [(NormalizedDpsProfile, DpsAbilityModel); 4] {
    [
        (tariq_profile(), tariq_ability(DpsAbilityKind::TariqAttack)),
        (
            elarion_profile([elarion_ability(DpsAbilityKind::ElarionShoot)]),
            elarion_ability(DpsAbilityKind::ElarionShoot),
        ),
        (mara_profile(), mara_ability(DpsAbilityKind::MaraAttack)),
        (gunde_profile(), gunde_ability(DpsAbilityKind::GundeAttack)),
    ]
}

fn outcome_seed(miss: bool) -> u64 {
    (0..1000)
        .find(|seed| {
            let roll = SplitMix64::new(*seed).next() as f64 / u64::MAX as f64;
            (roll < 0.05) == miss
        })
        .unwrap()
}

#[test]
fn rear_auto_outcome_uses_one_table_with_miss_before_critical() {
    assert_eq!(hit_outcome_from_roll(0.0, 0.05, 2.0), HitOutcome::Miss);
    assert_eq!(hit_outcome_from_roll(0.04999, 0.05, 2.0), HitOutcome::Miss);
    assert_eq!(hit_outcome_from_roll(0.05, 0.05, 0.2), HitOutcome::Critical);
    assert_eq!(
        hit_outcome_from_roll(0.24999, 0.05, 0.2),
        HitOutcome::Critical
    );
    assert_eq!(hit_outcome_from_roll(0.25, 0.05, 0.2), HitOutcome::Hit);
    assert_eq!(
        hit_outcome_from_roll(0.999, 0.05, 2.0),
        HitOutcome::Critical
    );
    // This is an unconditional 20% crit band, not 95% * 20%.
    let counts = (0..10_000).fold([0; 3], |mut counts, index| {
        let outcome = hit_outcome_from_roll((index as f64 + 0.5) / 10_000.0, 0.05, 0.2);
        counts[match outcome {
            HitOutcome::Miss => 0,
            HitOutcome::Critical => 1,
            HitOutcome::Hit => 2,
        }] += 1;
        counts
    });
    assert_eq!(counts, [500, 2000, 7500]);
}

#[test]
fn all_physical_autos_miss_without_damage_hits_spirit_or_extra_random_draws() {
    for (mut profile, mut auto) in auto_profiles() {
        auto.damage_spread = 0.2;
        profile.abilities = vec![auto.clone()];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 98);
        iteration.common.rng = SplitMix64::new(outcome_seed(true));
        let mut expected = iteration.common.rng;
        expected.next(); // Outcome, even when crit is disabled.
        expected.next(); // Spread, even on a miss.
        let before_spirit = iteration.shared.spirit;
        let outcome = iteration.damage_hit(
            &compiled_ability(&auto),
            1000.0,
            0.0,
            false,
            0,
            DamageContext::NONE,
        );
        assert_eq!(outcome.damage, 0.0, "{}", profile.hero_id);
        assert!(!outcome.critical);
        assert_eq!(iteration.common.result.damage, 0.0);
        assert_eq!(iteration.ability_totals(&auto.id).hits, 0);
        assert_eq!(iteration.shared.spirit, before_spirit);
        assert_eq!(iteration.common.rng.state, expected.state);
        // Exercise the scheduled impact continuation as well as the executor.
        iteration.common.rng = SplitMix64::new(outcome_seed(true));
        iteration.resolve_impact(0, true, 1.0, 0, CastImpactContext::new(DamageContext::NONE));
        assert_eq!(iteration.common.result.damage, 0.0);
        assert_eq!(iteration.ability_totals(&auto.id).hits, 0);
        assert_eq!(iteration.shared.spirit, before_spirit);
        assert!(iteration.shared.controlled_random_states.is_empty());
    }
}

#[test]
fn physical_auto_misses_preserve_harmful_application_kindling() {
    for (mut profile, auto) in auto_profiles() {
        profile.abilities = vec![auto.clone()];
        let kindling = "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc";
        profile.mechanics = vec![ardeos_mechanic(
            kindling,
            [
                ("procsPerMinute", 60_000.0),
                ("powerCoefficientPerTick", 0.69),
                ("durationSeconds", 9.0),
                ("periodSeconds", 1.5),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 98);
        iteration.common.rng = SplitMix64::new(outcome_seed(true));
        let outcome = iteration.damage_hit(
            &compiled_ability(&auto),
            1000.0,
            0.0,
            true,
            0,
            DamageContext::NONE,
        );
        assert_eq!(outcome.damage, 0.0);
        assert_eq!(iteration.ability_totals(&auto.id).hits, 0);
        assert_eq!(iteration.test_proc_count(kindling), 1);
        assert_eq!(iteration.test_kindling(kindling, 0).until_ms, 9_000);
    }
}

#[test]
fn missed_shoot_dispatches_parent_lunarlight_listener_without_proccing() {
    let auto = elarion_ability(DpsAbilityKind::ElarionShoot);
    let profile = elarion_profile([auto.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 98);
    iteration
        .hero
        .elarion_mut()
        .previous_mark_event_was_starfall = true;
    iteration.common.rng = SplitMix64::new(outcome_seed(true));
    iteration.damage_hit(
        &compiled_ability(&auto),
        1000.0,
        0.0,
        true,
        0,
        DamageContext::NONE,
    );
    assert!(!iteration.hero.elarion().previous_mark_event_was_starfall);
    assert_eq!(iteration.common.result.damage, 0.0);
    iteration.common.rng = SplitMix64::new(outcome_seed(false));
    iteration.damage_hit(
        &compiled_ability(&auto),
        1000.0,
        0.0,
        true,
        0,
        DamageContext::NONE,
    );
    assert!(!iteration.hero.elarion().previous_mark_event_was_starfall);
}

#[test]
fn missed_tariq_auto_retains_blueprint_resource_grant() {
    let mut auto = tariq_ability(DpsAbilityKind::TariqAttack);
    auto.primary_resource_generated = 7.0;
    auto.first_hit_delay_ms = 600;
    auto.gcd_ms = 1;
    let mut profile = tariq_profile();
    profile.abilities = vec![auto];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 98);
    iteration.hero.tariq_mut().fury = 0.0;
    iteration.cast(0);
    assert_eq!(iteration.hero.tariq().fury, 7.0);
    iteration.common.rng = SplitMix64::new(outcome_seed(true));
    iteration.process_events_through(600);
    assert_eq!(iteration.common.result.damage, 0.0);
    assert_eq!(iteration.hero.tariq().fury, 7.0);
}

#[test]
fn physical_autos_never_count_as_basic_for_vehement() {
    for (mut profile, auto) in auto_profiles() {
        assert_eq!(ability_category(auto.kind), AbilityCategory::Other);
        profile.abilities = vec![auto.clone()];
        let vehement = "DynamicItemAbilityRank.06";
        profile.mechanics = vec![ardeos_mechanic(
            vehement,
            [
                ("hitThreshold", 1.0),
                ("criticalStrikeCap", 0.5),
                ("targetCountDamageScalingThreshold", 5.0),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 98);
        iteration.common.rng = SplitMix64::new(outcome_seed(false));
        let outcome = iteration.damage_hit(
            &compiled_ability(&auto),
            1000.0,
            0.0,
            true,
            0,
            DamageContext::NONE,
        );
        assert!(outcome.damage > 0.0);
        assert_eq!(iteration.test_proc_count(vehement), 0);
        assert_eq!(iteration.test_dynamic_counter(vehement), 0);
    }
}

#[test]
fn landed_mara_auto_has_no_unused_energy_poison_proc() {
    let mut profile = mara_profile();
    profile.abilities = vec![mara_ability(DpsAbilityKind::MaraAttack)];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 98);
    iteration.hero.mara_mut().energy = 0.0;
    iteration.common.rng = SplitMix64::new(outcome_seed(false));
    iteration.resolve_impact(0, true, 1.0, 0, CastImpactContext::new(DamageContext::NONE));
    assert!(iteration.common.result.damage > 0.0);
    assert_eq!(iteration.hero.mara().energy, 0.0);
    assert!(iteration.shared.controlled_random_states.is_empty());
}

#[test]
fn magical_hits_and_transferred_auto_magnitudes_do_not_inherit_miss_chance() {
    let source = ability(DpsAbilityKind::FireBall, 1.0);
    let profile = profile(vec![source.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 98);
    iteration.common.rng = SplitMix64::new(outcome_seed(true));
    let outcome = iteration.damage_hit(
        &compiled_ability(&source),
        1000.0,
        2.0,
        true,
        0,
        DamageContext::NONE,
    );
    assert!(outcome.critical && outcome.damage > 0.0);
    let key = iteration.ability_damage_source(&compiled_ability(&source));
    let state = iteration.common.rng.state;
    iteration.damage_unscaled_key(
        Some(DpsAbilityKind::TariqAttack),
        key,
        100.0,
        0,
        DamageProvenance::Proc,
        DamageContext::NONE,
    );
    assert_eq!(iteration.common.rng.state, state);
    assert_eq!(iteration.ability_totals(&source.id).hits, 2);
}

#[test]
fn missed_auto_can_proc_emerald_through_parent_damage_event() {
    for (mut profile, auto) in auto_profiles() {
        profile.abilities = vec![auto.clone()];
        let emerald = "ItemTrait.ID.GemTargetedSpikeProc";
        let diamond = "ItemTrait.ID.GemSingleTargetProcOnDamageHeal";
        profile.mechanics = vec![
            ardeos_mechanic(
                emerald,
                [("procsPerMinute", 60_000.0), ("powerCoefficient", 1.0)],
            ),
            ardeos_mechanic(
                diamond,
                [("procsPerMinute", 60_000.0), ("powerCoefficient", 1.0)],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 98);
        iteration.common.rng = SplitMix64::new(outcome_seed(true));
        let outcome = iteration.damage_hit(
            &compiled_ability(&auto),
            1000.0,
            0.0,
            true,
            0,
            DamageContext::NONE,
        );
        assert_eq!(outcome.damage, 0.0);
        assert_eq!(iteration.ability_totals(&auto.id).hits, 0);
        assert_eq!(iteration.test_proc_count(emerald), 1);
        assert_eq!(iteration.test_proc_count(diamond), 0);
        assert!(
            iteration.common.result.damage > 0.0,
            "Emerald itself still hits"
        );
    }
}
