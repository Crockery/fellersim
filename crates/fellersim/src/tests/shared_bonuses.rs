use super::*;

const DARK_PROPHECY: &str = "setb-proc-hdt";
const VEHEMENT: &str = "DynamicItemAbilityRank.06";
const WAYFARER: &str = "DynamicItemAbilityRank.12";

fn profiles() -> [NormalizedDpsProfile; 6] {
    [
        profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
        rime_profile(),
        tariq_profile(),
        elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]),
        mara_profile(),
        gunde_profile(),
    ]
}

fn prophecy(ppm: f64) -> DynamicMechanicInstance {
    ardeos_mechanic(
        DARK_PROPHECY,
        [
            ("procsPerMinute", ppm),
            ("durationSeconds", 20.0),
            ("haste", 0.25),
            ("cooldownSeconds", 5.0),
        ],
    )
}

#[test]
fn dark_prophecy_rejects_auto_movement_utility_and_helpers_before_rolling() {
    let excluded = [
        tariq_ability(DpsAbilityKind::TariqAttack),
        elarion_ability(DpsAbilityKind::ElarionShoot),
        mara_ability(DpsAbilityKind::MaraAttack),
        gunde_ability(DpsAbilityKind::GundeAttack),
        tariq_ability(DpsAbilityKind::LeapSmash),
        gunde_ability(DpsAbilityKind::Warbound),
        mara_ability(DpsAbilityKind::BroodingShadows),
        gunde_ability(DpsAbilityKind::OwedInBlood),
        rime_ability(DpsAbilityKind::AnimaSpike),
    ];
    for mut profile in profiles() {
        profile.mechanics = vec![prophecy(60_000.0)];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 91);
        for (index, source) in excluded.iter().enumerate() {
            iteration.common.now_ms = 1_000 * index as u64;
            iteration.trigger_dynamic_on_cast(&compiled_ability(source), DamageContext::NONE);
        }
        assert!(iteration.shared.dynamic_proc_per_minute_states[0].is_none());
        assert_eq!(iteration.test_proc_count(DARK_PROPHECY), 0);
        assert_eq!(iteration.test_dynamic_ready_ms(DARK_PROPHECY), 0);
    }
}

#[test]
fn dark_prophecy_includes_skill_buffs_and_weapons_and_refreshes_one_buff() {
    let admitted = [
        ability(DpsAbilityKind::InfernalWave, 1.0),
        rime_ability(DpsAbilityKind::WintersBlessing),
        tariq_ability(DpsAbilityKind::ThunderCall),
        elarion_ability(DpsAbilityKind::LunarlightMark),
        mara_ability(DpsAbilityKind::MaidenOfDeath),
        gunde_ability(DpsAbilityKind::BloodboundSpirit),
        ability(DpsAbilityKind::WeaponChainLightning, 1.0),
    ];
    for source in admitted {
        let mut profile = profile(vec![source.clone()]);
        profile.mechanics = vec![prophecy(60_000.0)];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 92);
        let source = compiled_ability(&source);
        // The accepted clean start has neither an active buff nor RPPM history.
        assert!(iteration.shared.dynamic_buffs[0].is_none());
        assert!(iteration.shared.dynamic_proc_per_minute_states[0].is_none());
        iteration.trigger_dynamic_on_cast(&source, DamageContext::NONE);

        assert_eq!(iteration.test_proc_count(DARK_PROPHECY), 1);
        assert_eq!(iteration.test_dynamic_ready_ms(DARK_PROPHECY), 5_000);
        assert_eq!(iteration.test_dynamic_buff(DARK_PROPHECY).until_ms, 20_000);
        iteration.common.now_ms = 4_999;
        iteration.trigger_dynamic_on_cast(&source, DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(DARK_PROPHECY), 1);
        assert_eq!(
            iteration.shared.dynamic_proc_per_minute_states[0]
                .unwrap()
                .last_roll_seconds,
            0.0,
        );
        iteration.common.now_ms = 5_000;
        iteration.trigger_dynamic_on_cast(&source, DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(DARK_PROPHECY), 2);
        assert_eq!(iteration.test_dynamic_buff(DARK_PROPHECY).until_ms, 25_000);
        assert_eq!(iteration.test_dynamic_buff(DARK_PROPHECY).stacks, 1);
        assert_eq!(iteration.effective_haste(), 0.25);
        iteration.process_events_through(25_000);
        assert_eq!(iteration.effective_haste(), 0.0);
    }
}

#[test]
fn dark_prophecy_real_ppm_does_not_scale_with_haste_or_critical_chance() {
    let source = compiled_ability(&ability(DpsAbilityKind::InfernalWave, 1.0));
    for seed in 1..=32 {
        let mut slow = profile(vec![]);
        slow.mechanics = vec![prophecy(0.8)];
        let mut fast = slow.clone();
        fast.haste = 1.0;
        fast.critical_strike = 1.0;
        let apl = apl([]);
        let mut a = Iteration::new(&slow, &apl, 1, seed);
        let mut b = Iteration::new(&fast, &apl, 1, seed);
        for second in 0..120 {
            for iteration in [&mut a, &mut b] {
                iteration.common.now_ms = second * 1_000;
                iteration.trigger_dynamic_on_cast(&source, DamageContext::NONE);
            }
            assert_eq!(
                a.test_proc_count(DARK_PROPHECY),
                b.test_proc_count(DARK_PROPHECY)
            );
            assert_eq!(
                a.test_dynamic_ready_ms(DARK_PROPHECY),
                b.test_dynamic_ready_ms(DARK_PROPHECY)
            );
        }
    }
}

#[test]
fn wayfarer_uses_base_core_cooldown_and_refreshes_without_stacking() {
    let cores = [
        ability(DpsAbilityKind::FireBall, 0.0),
        rime_ability(DpsAbilityKind::BurstingIce),
        tariq_ability(DpsAbilityKind::HeavyStrike),
        elarion_ability(DpsAbilityKind::Multishot),
        mara_ability(DpsAbilityKind::WidowsBite),
        gunde_ability(DpsAbilityKind::HeartSplitter),
    ];
    for (mut profile, mut core) in profiles().into_iter().zip(cores) {
        profile.haste = 1.0;
        profile.cooldown_recovery = 2.0;
        profile.mechanics = vec![ardeos_mechanic(
            WAYFARER,
            [
                ("intervalSeconds", 60.0),
                ("durationSeconds", 14.0),
                ("hasteBonus", 0.2),
                ("coreCooldownDenominatorSeconds", 30.0),
                ("coreCooldownFractionMultiplier", 4.0),
                ("minimumReductionSeconds", 0.2),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 93);
        assert_eq!(iteration.shared.wayfarer_next_ms, 60_000);
        assert!(iteration.shared.dynamic_buffs[0].is_none());
        core.cooldown_ms = 0;
        iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
        assert_eq!(iteration.shared.wayfarer_next_ms, 59_800);
        core.cooldown_ms = 30_000;
        core.cooldown_scales_with_haste = true;
        iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
        assert_eq!(iteration.shared.wayfarer_next_ms, 55_800);
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(DpsAbilityKind::InfernalWave, 1.0)),
            DamageContext::NONE,
        );
        assert_eq!(iteration.shared.wayfarer_next_ms, 55_800);
        iteration.process_events_through(55_800);
        assert_eq!(iteration.common.result.mechanic_proc_counts[0], 1.0);
        assert_eq!(iteration.shared.wayfarer_next_ms, 115_800);
        iteration.process_events_through(60_000);
        assert_eq!(iteration.common.result.mechanic_proc_counts[0], 1.0); // Stale timer events cannot proc.
        for _ in 0..14 {
            iteration.trigger_dynamic_on_cast(&compiled_ability(&core), DamageContext::NONE);
        }
        iteration.process_events_through(60_000);
        assert_eq!(iteration.common.result.mechanic_proc_counts[0], 2.0);
        assert_eq!(iteration.test_dynamic_buff(WAYFARER).until_ms, 74_000);
        assert_eq!(iteration.test_dynamic_buff(WAYFARER).stacks, 1);
        assert!((iteration.effective_haste() - 1.2).abs() < 1e-12);
    }
}

#[test]
fn vehement_counts_one_first_hit_per_basic_cast_across_hero_impact_batches() {
    let basics = [
        ability(DpsAbilityKind::InfernalWave, 1.0),
        rime_ability(DpsAbilityKind::FrostBolt),
        tariq_ability(DpsAbilityKind::WildSwing),
        elarion_ability(DpsAbilityKind::FocusedShot),
        mara_ability(DpsAbilityKind::SkitteringBlades),
        gunde_ability(DpsAbilityKind::DoubleStrike),
    ];
    for (mut profile, mut basic) in profiles().into_iter().zip(basics) {
        // Exercise the shared scheduler with repeated hits and several targets.
        basic.direct_hits = 2;
        basic.hit_interval_ms = 100;
        basic.max_targets = 5;
        basic.cast_time_ms = 0;
        profile.abilities = vec![basic];
        if profile.hero_id == TARIQ_HERO_ID {
            profile
                .abilities
                .push(tariq_ability(DpsAbilityKind::TariqAttack));
        }
        profile.mechanics = vec![ardeos_mechanic(
            VEHEMENT,
            [
                ("hitThreshold", 2.0),
                ("criticalStrikeCap", 0.5),
                ("targetCountDamageScalingThreshold", 5.0),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 5, 94);
        assert_eq!(iteration.test_dynamic_counter(VEHEMENT), 0);
        iteration.prepare_cast(0);
        iteration.process_events_through(5_000);
        assert_eq!(
            iteration.test_dynamic_counter(VEHEMENT),
            1,
            "{}",
            profile.hero_id
        );
        assert_eq!(iteration.test_proc_count(VEHEMENT), 0);
        iteration.prepare_cast(0);
        iteration.process_events_through(10_000);
        assert_eq!(
            iteration.test_dynamic_counter(VEHEMENT),
            0,
            "{}",
            profile.hero_id
        );
        assert_eq!(iteration.test_proc_count(VEHEMENT), 1);
        assert_eq!(
            iteration
                .ability_totals("gear:DynamicItemAbilityRank.06")
                .hits,
            5
        );
    }
}

#[test]
fn vehement_copies_resolved_damage_with_live_generic_crit_and_falloff() {
    for mut profile in profiles() {
        profile
            .abilities
            .push(ability(DpsAbilityKind::InfernalWave, 1.0));
        profile.critical_strike = 0.1;
        profile.expertise = 0.5;
        profile.mechanics = vec![ardeos_mechanic(
            VEHEMENT,
            [
                ("hitThreshold", 1.0),
                ("criticalStrikeCap", 0.5),
                ("targetCountDamageScalingThreshold", 5.0),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 10, 95);
        let basic = compiled_ability(&ability(DpsAbilityKind::InfernalWave, 1.0));
        // A spec-only critical bonus gives a grievous critical hit. The copied
        // hit uses live 10% CritChance, then sqrt(5 / 10) falloff and rounding.
        let hit = iteration.damage_hit(&basic, 100.0, 1.0, true, 0, DamageContext::for_cast(1));
        assert_eq!(hit.damage, 315.0); // Includes the source spec's 10% grievous bonus.
        assert_eq!(
            iteration
                .ability_totals("gear:DynamicItemAbilityRank.06")
                .damage,
            2450.0
        );
        assert_eq!(
            iteration
                .ability_totals("gear:DynamicItemAbilityRank.06")
                .crits,
            0
        );
        assert_eq!(iteration.test_proc_count(VEHEMENT), 1);
    }
}

#[test]
fn shared_damage_rounding_and_empty_transfers_hold_for_all_hero_contracts() {
    for mut profile in profiles() {
        profile
            .abilities
            .push(ability(DpsAbilityKind::InfernalWave, 1.0));
        profile.critical_strike = 1.5;
        profile.expertise = 0.1;
        profile.critical_multiplier = 2.0;
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 96);
        let source = compiled_ability(&ability(DpsAbilityKind::InfernalWave, 1.0));
        let hit = iteration.damage_hit(&source, 10.2, 0.0, true, 0, DamageContext::NONE);
        assert!(hit.critical);
        assert_eq!(hit.damage, 28.0); // 10.2 * 1.1 * (2 + 0.5), rounded.
        let hit = iteration.damage_hit(&source, 10.2, 0.0, false, 0, DamageContext::NONE);
        assert!(!hit.critical);
        assert_eq!(hit.damage, 11.0);
        let key = iteration.test_damage_source("empty-transfer");
        for (raw, expected) in [(7.499, 7.0), (7.5, 8.0), (0.499, 0.0), (0.5, 1.0)] {
            let hit = iteration.damage_unscaled_key(
                None,
                key,
                raw,
                0,
                DamageProvenance::Proc,
                DamageContext::NONE,
            );
            assert_eq!(hit.damage, expected);
            assert!(!hit.critical);
        }
    }
}
