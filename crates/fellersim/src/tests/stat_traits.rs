use super::*;

const HIDDEN: &str = "ItemTrait.ID.AbilityToIncreasedMainStat";
const ALLEGIANCE: &str = "ItemTrait.ID.CommitToHasteRatingAndWeaponCooldown";
const NAVIGATOR: &str = "ItemTrait.ID.OffensiveAbilityToHighestStatBuff";
const HUNTER: &str = "ItemTrait.ID.OffensiveAbilityHasteRatingStacking";
const ARMS: &str = "ItemTrait.ID.CooldownRecoveryOnWeaponAbility";
const MARTIAL: &str = "ItemTrait.ID.WeaponDamageReductionPrimaryStatIncrease";
const TOPAZ: &str = "ItemTrait.ID.GemCooldownRecoveryOnAbilityProc";

#[test]
fn primary_support_commits_feed_hidden_power_but_not_offensive_trait_rolls() {
    for support in [
        DpsAbilityKind::IceBlitz,
        DpsAbilityKind::WintersBlessing,
        DpsAbilityKind::WrathOfWinter,
        DpsAbilityKind::ThunderCall,
        DpsAbilityKind::FocusedWrath,
        DpsAbilityKind::LunarlightMark,
        DpsAbilityKind::SkystridersGrace,
        DpsAbilityKind::SkystridersSupremacy,
        DpsAbilityKind::MaidenOfDeath,
        DpsAbilityKind::MatriarchMacabre,
        DpsAbilityKind::FinalStratagem,
        DpsAbilityKind::BroodingShadows,
        DpsAbilityKind::BloodboundSpirit,
        DpsAbilityKind::ReignInBlood,
    ] {
        let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
        profile.mechanics = vec![
            ardeos_mechanic(
                HIDDEN,
                [
                    ("procsPerMinute", 60_000.0),
                    ("requiredStacks", 5.0),
                    ("stackDurationSeconds", 60.0),
                    ("powerMultiplier", 1.24),
                    ("durationSeconds", 15.0),
                ],
            ),
            ardeos_mechanic(
                ALLEGIANCE,
                [
                    ("procsPerMinute", 60_000.0),
                    ("hasteRating", 17.0),
                    ("durationSeconds", 8.0),
                    ("weaponCooldownReductionSeconds", 8.0),
                ],
            ),
            ardeos_mechanic(
                NAVIGATOR,
                [
                    ("procChance", 1.0),
                    ("secondaryRating", 142.0),
                    ("durationSeconds", 30.0),
                    ("cooldownSeconds", 90.0),
                ],
            ),
        ];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 8);
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(support, 0.0)),
            DamageContext::NONE,
        );
        assert_eq!(iteration.test_dynamic_counter(HIDDEN), 1, "{support:?}");
        assert_eq!(iteration.test_proc_count(ALLEGIANCE), 0, "{support:?}");
        assert_eq!(iteration.test_proc_count(NAVIGATOR), 0, "{support:?}");
        assert!(
            iteration.shared.dynamic_proc_per_minute_states
                [iteration.test_mechanic_index(ALLEGIANCE)]
            .is_none()
        );
        // Primary auto attacks carry Skill.Offensive; helper damage abilities
        // do not. Their damage/category labels must not decide this filter.
        for kind in [
            DpsAbilityKind::TariqAttack,
            DpsAbilityKind::ElarionShoot,
            DpsAbilityKind::MaraAttack,
            DpsAbilityKind::GundeAttack,
        ] {
            assert!(is_offensive_skill_commit(kind));
        }
        for kind in [
            DpsAbilityKind::AnimaSpike,
            DpsAbilityKind::FrostSwallow,
            DpsAbilityKind::LunarlightSalvo,
        ] {
            assert!(!is_primary_skill_commit(kind));
        }
        iteration.trigger_dynamic_on_cast(
            &compiled_ability(&ability(DpsAbilityKind::InfernalWave, 1.0)),
            DamageContext::NONE,
        );
        assert_eq!(iteration.test_proc_count(ALLEGIANCE), 1);
        assert_eq!(iteration.test_proc_count(NAVIGATOR), 1);
    }
}

#[test]
fn hunters_focus_uses_cooked_tags_instead_of_single_target_damage_categories() {
    for kind in [
        DpsAbilityKind::FlightOfTheNavir,
        DpsAbilityKind::Multishot,
        DpsAbilityKind::EventHorizon,
        DpsAbilityKind::StarfallVolley,
        DpsAbilityKind::SkitteringBlades,
        DpsAbilityKind::WeaponArcaneChannel,
        DpsAbilityKind::WeaponChainLightning,
        DpsAbilityKind::WeaponShadowMark,
    ] {
        let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
        profile.mechanics = vec![ardeos_mechanic(
            HUNTER,
            [
                ("hasteRating", 16.0),
                ("durationSeconds", 8.0),
                ("maximumStacks", 5.0),
            ],
        )];
        let apl = apl([]);
        let mut iteration = Iteration::new(&profile, &apl, 1, 8);
        for excluded in [
            DpsAbilityKind::TariqAttack,
            DpsAbilityKind::ElarionShoot,
            DpsAbilityKind::MaraAttack,
            DpsAbilityKind::GundeAttack,
            DpsAbilityKind::OwedInBlood,
            DpsAbilityKind::ButchersHook,
            DpsAbilityKind::Detonate,
        ] {
            iteration.trigger_dynamic_on_cast(
                &compiled_ability(&ability(excluded, 1.0)),
                DamageContext::NONE,
            );
        }
        assert!(iteration.shared.dynamic_buffs.iter().all(Option::is_none));
        iteration
            .trigger_dynamic_on_cast(&compiled_ability(&ability(kind, 1.0)), DamageContext::NONE);
        assert_eq!(iteration.test_dynamic_buff(HUNTER).stacks, 1, "{kind:?}");
        assert_eq!(
            iteration.dynamic_rating_bonus(parameter_key!("hasteRating")),
            16.0
        );
    }
}

#[test]
fn primary_attribute_trait_bonuses_add_before_compound_effects() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![
        ardeos_mechanic(HIDDEN, [("powerMultiplier", 1.24)]),
        ardeos_mechanic(MARTIAL, [("powerMultiplier", 1.1)]),
        ardeos_mechanic(
            "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff",
            [("powerMultiplier", 1.16)],
        ),
        ardeos_mechanic(
            "ItemTrait.ID.IncreasedMainStatAndSpiritRating",
            [("powerMultiplier", 1.3)],
        ),
        ardeos_mechanic("seta-proc-intellect", [("powerMultiplier", 1.18)]),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    for index in 0..5 {
        let mechanic = Arc::clone(&iteration.profile.mechanics[index]);
        iteration.activate_dynamic_buff(&mechanic, if index == 0 { 1000 } else { 2000 }, 1, 0.0);
    }
    assert!(
        (iteration.effective_power_multiplier() - (1.0 + 0.24 + 0.1 + 0.16 + 0.3) * 1.18).abs()
            < 1e-12
    );
    iteration.common.now_ms = 1000;
    assert!(
        (iteration.effective_power_multiplier() - (1.0 + 0.1 + 0.16 + 0.3) * 1.18).abs() < 1e-12
    );
    iteration.common.now_ms = 2000;
    assert_eq!(iteration.effective_power_multiplier(), 1.0);
}

#[test]
fn cooldown_recovery_buffs_share_the_native_additive_multiplier_bucket() {
    let mut channel = ability(DpsAbilityKind::WeaponArcaneChannel, 0.0);
    channel
        .mechanic_parameters
        .insert("channelCooldownRecoveryMultiplier".into(), 8.0);
    let mut cleave = ability(DpsAbilityKind::WeaponCleaveCharge, 0.0);
    cleave
        .mechanic_parameters
        .insert("buffCooldownRecoveryMultiplier".into(), 1.3);
    let mut profile = profile(vec![channel, cleave]);
    profile.cooldown_recovery = 1.1;
    profile.mechanics = vec![
        ardeos_mechanic(ARMS, [("cooldownAccelerationMultiplier", 1.2)]),
        ardeos_mechanic(TOPAZ, [("cooldownAccelerationMultiplier", 1.3)]),
    ];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    iteration.shared.weapon_channel_cooldown_recovery_until = 1000;
    iteration.shared.weapon_charge_buff_until = 2000;
    for index in 0..2 {
        let mechanic = Arc::clone(&iteration.profile.mechanics[index]);
        iteration.activate_dynamic_buff(&mechanic, 3000, 1, 0.0);
    }
    for (now, expected) in [(0, 8.9), (1000, 1.9), (2000, 1.6), (3000, 1.1)] {
        iteration.common.now_ms = now;
        assert!(
            (iteration.effective_cooldown_recovery() - expected).abs() < 1e-12,
            "{now}"
        );
    }
}

#[test]
fn topaz_winds_waits_out_its_internal_cooldown_before_rolling_again() {
    let mut profile = profile(vec![ability(DpsAbilityKind::FireBall, 1.0)]);
    profile.mechanics = vec![ardeos_mechanic(
        TOPAZ,
        [
            ("cooldownAccelerationMultiplier", 1.3),
            ("durationSeconds", 8.0),
            ("cooldownSeconds", 20.0),
            ("procsPerMinute", 60_000.0),
        ],
    )];
    let apl = apl([]);
    let mut iteration = Iteration::new(&profile, &apl, 1, 8);
    let support = compiled_ability(&ability(DpsAbilityKind::IceBlitz, 0.0));
    for (now, procs) in [(0, 1), (19_999, 1), (20_000, 2)] {
        iteration.common.now_ms = now;
        iteration.trigger_dynamic_on_cast(&support, DamageContext::NONE);
        assert_eq!(iteration.test_proc_count(TOPAZ), procs);
    }
}
