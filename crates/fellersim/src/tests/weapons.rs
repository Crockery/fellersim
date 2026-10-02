use super::*;

fn cleave_weapon() -> DpsAbilityModel {
    let mut weapon = ability(DpsAbilityKind::WeaponCleaveCharge, 10.0);
    weapon.mechanic_parameters = BTreeMap::from([
        ("cleaveDamageMultiplier".into(), 0.4),
        ("cleaveTargetCountDamageScalingThreshold".into(), 3.0),
        ("postHitDelaySeconds".into(), 0.0),
        ("buffDurationSeconds".into(), 6.0),
        ("buffCooldownRecoveryMultiplier".into(), 3.0),
        ("buffExpertise".into(), 0.0),
    ]);
    weapon.off_gcd = true;
    weapon
}

#[test]
fn cleave_is_shared_by_all_melee_heroes_with_secondary_only_falloff() {
    for mut profile in [tariq_profile(), mara_profile(), gunde_profile()] {
        let weapon = cleave_weapon();
        profile.abilities = vec![weapon.clone()];
        profile.power = 100.0;
        profile.critical_strike = 0.0;
        profile.expertise = 0.0;
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 5, 1);
        iteration.impact(0, true, 1.0, 0, DamageContext::NONE);
        assert_eq!(
            iteration.ability_totals(&weapon.id).hits,
            5,
            "{}",
            profile.hero_id
        );
        let expected = 1000.0 + 4.0 * (400.0 * (3.0_f64 / 4.0).sqrt()).round();
        assert_eq!(
            iteration.common.result.damage, expected,
            "{}",
            profile.hero_id
        );
        assert_eq!(iteration.shared.weapon_charge_buff_until, 6000);
    }
}

#[test]
fn cleave_resolves_independent_outcomes_instead_of_copying_primary_crit() {
    let weapon = cleave_weapon();
    let mut profile = mara_profile();
    profile.power = 100.0;
    profile.expertise = 0.0;
    profile.critical_strike = 0.5;
    profile.critical_multiplier = 2.0;
    profile.abilities = vec![weapon.clone()];
    let apl = apl([]);
    let seed = (0..1000)
        .find(|seed| {
            let mut rng = SplitMix64::new(*seed);
            let first = rng.next() as f64 / u64::MAX as f64;
            let second = rng.next() as f64 / u64::MAX as f64;
            first < 0.5 && second >= 0.5
        })
        .unwrap();
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    iteration.common.rng = SplitMix64::new(seed);
    iteration.impact(0, true, 1.0, 0, DamageContext::NONE);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 2);
    assert_eq!(iteration.common.result.damage, 2400.0);
}

#[test]
fn cleave_grants_its_expertise_buff_after_its_own_damage_and_at_arrival() {
    let mut weapon = cleave_weapon();
    weapon.first_hit_delay_ms = 200;
    weapon
        .mechanic_parameters
        .insert("buffExpertise".into(), 1.0);
    let mut profile = mara_profile();
    profile.power = 100.0;
    profile.expertise = 0.0;
    profile.critical_strike = 0.0;
    profile.abilities = vec![weapon];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    iteration.cast(0);
    assert_eq!(iteration.shared.weapon_charge_buff_until, 0);
    iteration.process_events_through(199);
    assert_eq!(iteration.common.result.damage, 0.0);
    iteration.process_events_through(200);
    assert_eq!(iteration.common.result.damage, 1400.0);
    assert_eq!(iteration.shared.weapon_charge_buff_until, 6200);
}

fn cone_weapon() -> DpsAbilityModel {
    let mut weapon = ability(DpsAbilityKind::WeaponFrontalCone, 10.0);
    weapon.first_hit_delay_ms = 400;
    weapon.max_targets = 20;
    weapon.mechanic_parameters = BTreeMap::from([
        ("initialPowerCoefficient".into(), 10.0),
        ("repeatingPowerCoefficient".into(), 1.0),
        ("finalPowerCoefficient".into(), 20.0),
        ("coneDurationSeconds".into(), 10.0),
        ("coneStunDurationSeconds".into(), 3.0),
        ("coneTickIntervalSeconds".into(), 1.5),
        ("targetCountDamageScalingThreshold".into(), 3.0),
    ]);
    weapon.off_gcd = true;
    weapon
}

#[test]
fn cone_haste_scales_only_repeating_timer_and_all_specs_keep_commit_snapshot() {
    let weapon = cone_weapon();
    let mut buff = ability(DpsAbilityKind::WeaponInstantAoe, 1.0);
    buff.mechanic_parameters
        .insert("criticalStrikeBonusPerStack".into(), 1.0);
    let mut profile = profile(vec![weapon.clone(), buff]);
    profile.haste = 0.5;
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 1);
    iteration.cast(0);
    iteration.shared.weapon_critical_buff_stacks = 1;
    iteration.shared.weapon_critical_buff_until = 100_000;
    iteration.process_events_through(399);
    assert_eq!(iteration.common.result.damage, 0.0);
    iteration.process_events_through(400);
    assert_eq!(iteration.common.result.damage, 1000.0);
    iteration.process_events_through(999);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 1);
    iteration.process_events_through(1000);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 2);
    iteration.process_events_through(9999);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 10);
    iteration.process_events_through(10_000);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 11);
    assert_eq!(iteration.common.result.damage, 3900.0);
}

#[test]
fn cone_applies_three_target_falloff_to_every_pulse_and_final_hit() {
    let weapon = cone_weapon();
    let profile = profile(vec![weapon.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 12, 1);
    iteration.cast(0);
    iteration.process_events_through(10_000);
    assert_eq!(iteration.ability_totals(&weapon.id).hits, 12 * 8);
    assert_eq!(
        iteration.common.result.damage,
        12.0 * (500.0 + 6.0 * 50.0 + 1000.0)
    );
}

#[test]
fn instant_aoe_applies_one_capped_buff_after_damage_and_discards_expired_stacks() {
    let mut weapon = ability(DpsAbilityKind::WeaponInstantAoe, 1.0);
    weapon.max_targets = 20;
    weapon.mechanic_parameters = BTreeMap::from([
        ("targetCountDamageScalingThreshold".into(), 20.0),
        ("criticalStrikeBonusPerStack".into(), 1.0),
        ("maximumCriticalStrikeStacks".into(), 5.0),
        ("criticalStrikeBuffDurationSeconds".into(), 2.0),
    ]);
    let profile = profile(vec![weapon.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 3, 1);
    iteration.shared.weapon_critical_buff_stacks = 5;
    iteration.shared.weapon_critical_buff_until = 0; // Expired previous cast.
    iteration.cast(0);
    iteration.process_events_through(0);
    assert_eq!(iteration.common.result.damage, 300.0);
    assert_eq!(iteration.shared.weapon_critical_buff_stacks, 3);
    assert_eq!(iteration.shared.weapon_critical_buff_until, 2000);
    iteration.common.now_ms = 1000;
    iteration.cast(0);
    iteration.process_events_through(1000);
    assert_eq!(iteration.shared.weapon_critical_buff_stacks, 5);
    assert_eq!(iteration.shared.weapon_critical_buff_until, 3000);
    iteration.common.now_ms = 3000;
    iteration.cast(0);
    iteration.process_events_through(3000);
    assert_eq!(iteration.shared.weapon_critical_buff_stacks, 3);
}

#[test]
fn shadow_mark_accumulates_other_target_explosions_and_removes_itself_before_dispatch() {
    let mut mark = ability(DpsAbilityKind::WeaponShadowMark, 0.0);
    mark.mechanic_parameters = BTreeMap::from([
        ("accumulationFraction".into(), 0.5),
        ("maximumPowerCoefficient".into(), 1.0),
    ]);
    let profile = profile(vec![mark.clone()]);
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 2, 1);
    iteration.shared.shadow_marks.insert(
        0,
        ShadowMarkState {
            generation: 1,
            until_ms: 5000,
            accumulated_damage: 0.0,
            context: DamageContext::for_cast(1),
        },
    );
    iteration.damage_unscaled_key(
        None,
        iteration.test_damage_source(&mark.id),
        200.0,
        1,
        DamageProvenance::Explosion,
        DamageContext::NONE,
    );
    assert!(iteration.shared.shadow_marks.is_empty());
    assert_eq!(iteration.common.result.targets, vec![100.0, 200.0]);
    assert_eq!(iteration.ability_totals(&mark.id).hits, 2);
}

#[test]
fn cleave_post_hit_lock_outlasts_the_accelerated_gcd_without_delaying_damage() {
    for (haste, expected_unlock) in [(0.0, 650), (1.0, 325)] {
        let mut weapon = cleave_weapon();
        weapon.cast_time_ms = 150;
        weapon.off_gcd = false;
        weapon.gcd_scales_with_cooldown_recovery = true;
        weapon
            .mechanic_parameters
            .insert("postHitDelaySeconds".into(), 0.5);
        let mut profile = profile(vec![weapon]);
        profile.haste = haste;
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 1);
        iteration.cast(0);
        assert_eq!(iteration.common.now_ms, expected_unlock);
        assert_eq!(
            iteration.shared.weapon_charge_buff_until,
            6000 + (150.0 / (1.0 + haste)) as u64
        );
        assert_eq!(iteration.common.result.damage, 1000.0);
    }
}
