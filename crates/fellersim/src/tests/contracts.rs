use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn request_validation_rejects_hostile_structure_and_execution_counts() {
    let mut request = rime_request(rime_profile());
    let frost_bolt = request
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::FrostBolt)
        .expect("Frost Bolt model");
    frost_bolt.cast_time_ms = 0;
    frost_bolt.gcd_ms = 0;
    frost_bolt.cooldown_ms = 0;
    frost_bolt.secondary_resource_cost = 0;
    frost_bolt.spirit_cost = 0.0;
    frost_bolt.channel = None;
    let error = validate(&request).expect_err("zero-time actions must be rejected");
    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
    assert_eq!(error.sources, vec!["test:frost-bolt"]);

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0].direct_hits = u32::MAX;
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    let channel = request
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::FreezingTorrent)
        .and_then(|ability| ability.channel.as_mut())
        .expect("Freezing Torrent channel");
    channel.duration_ms = 20 * 60 * 1_000;
    channel.tick_interval_ms = 1;
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0].dot = Some(DotModel {
        power_coefficient: 1.0,
        damage_spread: 0.0,
        duration_ms: 20 * 60 * 1_000,
        period_ms: 1,
        can_crit: false,
        cinder_proc_chance: 0.0,
        cinders_on_proc: 0.0,
        stack_damage_increase: 0.0,
        maximum_stacks: 1,
    });
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0].maximum_charges = 33;
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0].max_targets = 21;
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.heroism_duration_ms = 20 * 60 * 1_000 + 1;
    assert!(validate(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0]
        .mechanic_parameters
        .insert("hostileCount".into(), 1_000_001.0);
    assert!(validate(&request).is_err());
}

#[test]
fn request_validation_rejects_oversized_collections_text_and_apls() {
    let mut request = rime_request(rime_profile());
    let template = request.profile.abilities[0].clone();
    request
        .profile
        .abilities
        .extend((request.profile.abilities.len()..=64).map(|_| template.clone()));
    assert!(validate_request(&request).is_err());

    let mut request = rime_request(rime_profile());
    let claim = request.evidence.claims[0].clone();
    request.evidence.claims = vec![claim; 257];
    assert!(validate_request(&request).is_err());

    let mut request = rime_request(rime_profile());
    let rule = request.action_priority_list.rules[0].clone();
    request.action_priority_list.rules = vec![rule; 129];
    assert!(validate_request(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.action_priority_list.rules[0].condition = Some(expression(
        "wide-root",
        AplExpression::All {
            children: (0..1_025)
                .map(|index| {
                    boolean_reference(
                        &format!("wide-{index}"),
                        AplBooleanReference::TalentSelected {
                            talent_id: "unused".into(),
                        },
                    )
                })
                .collect(),
        },
    ));
    assert!(validate_request(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.profile.abilities[0].mechanic_parameters = (0..65)
        .map(|index| (format!("parameter-{index}"), 1.0))
        .collect();
    assert!(validate_request(&request).is_err());

    let mut request = rime_request(rime_profile());
    request.run_id = "x".repeat(4 * 1_024 + 1);
    assert!(validate_request(&request).is_err());
}

#[test]
fn parameter_driven_loops_exhaust_the_iteration_budget() {
    let mut request = rime_request(rime_profile());
    request
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::AnimaSpike)
        .expect("Anima Spike model")
        .mechanic_parameters
        .insert("projectileCount".into(), 1_000_000.0);
    request
        .profile
        .abilities
        .iter_mut()
        .find(|ability| ability.kind == DpsAbilityKind::FrostBolt)
        .expect("Frost Bolt model")
        .primary_resource_generated = request.profile.max_primary_resource;
    let cancelled = AtomicBool::new(false);
    let profile = CompiledProfile::try_from(&request).expect("hostile profile remains structural");
    let error = Iteration::from_compiled(
        &profile,
        &request.action_priority_list,
        request.scenario.target_count,
        1,
        &cancelled,
    )
    .run()
    .expect_err("parameter-driven projectile fanout must exhaust the work budget");

    assert_eq!(error.code, SimulationErrorCode::SimulationFailed);
    assert!(error.message.contains("execution budget"));
}

#[test]
fn haste_scaled_channel_fanout_exhausts_the_budget_before_allocation() {
    let mut request = rime_request(rime_profile());
    request.profile.haste = 999_999.0;
    request.action_priority_list = apl([("freezing-torrent", None)]);
    let cancelled = AtomicBool::new(false);
    let profile = CompiledProfile::try_from(&request).expect("hostile profile remains structural");

    let error = Iteration::from_compiled(
        &profile,
        &request.action_priority_list,
        request.scenario.target_count,
        1,
        &cancelled,
    )
    .run()
    .expect_err("haste-scaled channel fanout must exhaust the work budget");

    assert_eq!(error.code, SimulationErrorCode::SimulationFailed);
    assert!(error.message.contains("execution budget"));
}

#[test]
fn cancellation_is_observed_during_iteration_execution() {
    let request = rime_request(rime_profile());
    let cancelled = AtomicBool::new(false);

    let error = simulate(&request, &cancelled, |progress| {
        if progress.completed_iterations > 0 {
            cancelled.store(true, Ordering::Relaxed);
        }
    })
    .expect_err("cancellation must stop an active simulation");

    assert_eq!(error.code, SimulationErrorCode::Cancelled);
}

#[test]
fn apl_validation_rejects_empty_unknown_and_non_finite_contracts() {
    let profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    assert!(validate_action_priority_list(&apl([]), &profile).is_err());
    assert!(validate_action_priority_list(&apl([("fire-ball", None)]), &profile).is_err());
    assert!(
        validate_action_priority_list(
            &apl([
                ("weapon-frost-volley", None),
                ("weapon-arcane-channel", None),
                ("weapon-chain-lightning", None),
                ("weapon-shadow-mark", None),
                ("infernal-wave", None),
            ]),
            &profile,
        )
        .is_ok()
    );
    assert!(
        validate_action_priority_list(&apl([("weapon-cleave-charge", None)]), &profile).is_err()
    );
    let non_finite = comparison(
        "non-finite",
        number(f64::NAN),
        AplComparisonOperator::Equal,
        number(0.0),
    );
    assert!(
        validate_action_priority_list(&apl([("infernal-wave", Some(non_finite))]), &profile)
            .is_err()
    );
    let rime_resource = comparison(
        "rime-resource",
        reference(AplNumericReference::Resource {
            resource: AplResource::Anima,
            measure: AplResourceMeasure::Current,
        }),
        AplComparisonOperator::GreaterThan,
        number(0.0),
    );
    assert!(
        validate_action_priority_list(&apl([("infernal-wave", Some(rime_resource))]), &profile)
            .is_err()
    );

    let rime_profile = rime_profile();
    let ardeos_resource = comparison(
        "ardeos-resource",
        reference(AplNumericReference::Resource {
            resource: AplResource::Cinders,
            measure: AplResourceMeasure::Current,
        }),
        AplComparisonOperator::GreaterThan,
        number(0.0),
    );
    assert!(
            validate_action_priority_list(
                &apl([("frost-bolt", Some(ardeos_resource))]),
                &rime_profile,
            )
            .is_err()
        );
    let ardeos_buff = boolean_reference(
        "ardeos-buff",
        AplBooleanReference::BuffActive {
            buff: AplBuff::Wildfire,
        },
    );
    assert!(
        validate_action_priority_list(&apl([("frost-bolt", Some(ardeos_buff))]), &rime_profile,)
            .is_err()
    );
}

#[test]
fn apl_validation_accepts_only_ardeos_legendary_references_for_ardeos() {
    let ardeos_profile = profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]);
    for item_id in [
        "legendary-back-a-criticalstrike",
        "legendary-wrists-a-expertise",
        "legendary-ring-c-criticalstrike-haste",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(
                &apl([("infernal-wave", Some(condition))]),
                &ardeos_profile,
            )
            .is_ok(),
            "{item_id} should be a valid Ardeos legendary",
        );
    }

    let unknown = boolean_reference(
        "unknown-legendary",
        AplBooleanReference::LegendaryEquipped {
            item_id: "legendary-not-a-real-item".into(),
        },
    );
    assert!(
        validate_action_priority_list(&apl([("infernal-wave", Some(unknown))]), &ardeos_profile,)
            .is_err()
    );

    let cross_hero = boolean_reference(
        "cross-hero-legendary",
        AplBooleanReference::LegendaryEquipped {
            item_id: "legendary-ring-c-criticalstrike-haste".into(),
        },
    );
    assert!(
        validate_action_priority_list(&apl([("frost-bolt", Some(cross_hero))]), &rime_profile(),)
            .is_err()
    );
}

#[test]
fn apl_validation_accepts_only_rime_legendary_references_for_rime() {
    let profile = rime_profile();
    for item_id in [
        "legendary-back-a-haste",
        "legendary-wrists-a-haste",
        "legendary-ring-c-criticakstrike-expertise",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(&apl([("frost-bolt", Some(condition))]), &profile)
                .is_ok(),
            "{item_id} should be a valid Rime legendary",
        );
    }

    let ardeos_legendary = boolean_reference(
        "cross-hero-legendary",
        AplBooleanReference::LegendaryEquipped {
            item_id: "legendary-back-a-criticalstrike".into(),
        },
    );
    assert!(
        validate_action_priority_list(&apl([("frost-bolt", Some(ardeos_legendary))]), &profile,)
            .is_err()
    );
}

#[test]
fn apl_validation_accepts_only_elarion_legendary_references_for_elarion() {
    let profile = elarion_profile([elarion_ability(DpsAbilityKind::FocusedShot)]);
    for item_id in [
        "legendary-back-a-haste",
        "legendary-wrists-a-criticalstrike",
        "legendary-ring-c-spirit-spirit",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(&apl([("focused-shot", Some(condition))]), &profile)
                .is_ok(),
            "{item_id} should be a valid Elarion legendary",
        );
    }

    let rime_legendary = boolean_reference(
        "cross-hero-legendary",
        AplBooleanReference::LegendaryEquipped {
            item_id: "legendary-wrists-a-haste".into(),
        },
    );
    assert!(
        validate_action_priority_list(&apl([("focused-shot", Some(rime_legendary))]), &profile,)
            .is_err()
    );
}

#[test]
fn apl_validation_accepts_tariq_legendary_references() {
    let profile = tariq_profile();
    for item_id in [
        "legendary-back-a-haste",
        "legendary-wrists-a-expertise",
        "legendary-ring-c-criticalstrike-haste",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(&apl([("wild-swing", Some(condition))]), &profile)
                .is_ok(),
            "{item_id} should be a valid Tariq legendary",
        );
    }
}

#[test]
fn apl_validation_accepts_gunde_legendary_references() {
    let profile = gunde_profile();
    for item_id in [
        "legendary-back-a-haste",
        "legendary-wrists-a-criticalstrike",
        "legendary-ring-c-criticakstrike-expertise",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(&apl([("double-strike", Some(condition))]), &profile)
                .is_ok(),
            "{item_id} should be a valid Gunde legendary",
        );
    }
}

#[test]
fn apl_validation_accepts_mara_legendary_references() {
    let profile = mara_profile();
    for item_id in [
        "legendary-back-a-haste",
        "legendary-wrists-a-expertise",
        "legendary-ring-c-spirit-spirit",
    ] {
        let condition = boolean_reference(
            "legendary-equipped",
            AplBooleanReference::LegendaryEquipped {
                item_id: item_id.into(),
            },
        );
        assert!(
            validate_action_priority_list(&apl([("mara-attack", Some(condition))]), &profile)
                .is_ok(),
            "{item_id} should be a valid Mara legendary",
        );
    }
}

#[test]
fn loadout_dependent_weapon_actions_follow_each_hero_weapon_pool() {
    let caster_weapons = || {
        apl([
            ("weapon-frost-volley", None),
            ("weapon-arcane-channel", None),
            ("weapon-chain-lightning", None),
            ("weapon-shadow-mark", None),
        ])
    };
    for profile in [rime_profile(), elarion_profile([])] {
        assert!(validate_action_priority_list(&caster_weapons(), &profile).is_ok());
        assert!(
            validate_action_priority_list(&apl([("weapon-instant-aoe", None)]), &profile).is_err()
        );
    }

    let melee_weapons = || {
        apl([
            ("weapon-shadow-mark", None),
            ("weapon-cleave-charge", None),
            ("weapon-frontal-cone", None),
            ("weapon-instant-aoe", None),
        ])
    };
    for profile in [tariq_profile(), mara_profile(), gunde_profile()] {
        assert!(validate_action_priority_list(&melee_weapons(), &profile).is_ok());
        assert!(
            validate_action_priority_list(&apl([("weapon-arcane-channel", None)]), &profile)
                .is_err()
        );
    }
}

#[test]
fn profile_validation_rejects_target_effects_without_runtime_sources() {
    let mut profile = rime_profile();
    profile.apl_target_effects.push(AplTargetEffectModel {
        id: "frost-bolt-debuff".into(),
        name: "Frost Bolt debuff".into(),
        source: AplTargetEffectSource::AbilityDot {
            ability_id: "frost-bolt".into(),
        },
        effect_kind: AplTargetEffectKind::DamageOverTime,
        supported_properties: vec![AplTargetEffectProperty::Active],
    });

    let error = validate(&rime_request(profile))
        .expect_err("an ability without a damage-over-time model cannot back a target effect");

    assert_eq!(error.code, SimulationErrorCode::InvalidBuild);
    assert_eq!(error.sources, vec!["frost-bolt-debuff"]);
    assert!(error.message.contains("evaluable target-effect source"));
}

#[test]
fn profile_compilation_builds_typed_indexes_and_discards_serialized_parameter_maps() {
    let mut profile = rime_profile();
    profile.talents.push(rime_talent(7, [("procChance", 0.25)]));
    profile.mechanics.push(ardeos_mechanic(
        "ItemTrait.ID.WeaponHealDamageIncrease",
        [("weaponDamageMultiplier", 1.15)],
    ));
    let request = rime_request(profile);

    let compiled = CompiledProfile::try_from(&request).expect("valid profile compiles");

    assert_eq!(compiled.contract.hero, HeroIdentity::Rime);
    assert_eq!(compiled.contract.model_version, RIME_MODEL_VERSION);
    assert_eq!(compiled.validated_seed, 1);
    assert_eq!(compiled.source.abilities.len(), 0);
    assert_eq!(compiled.source.talents.len(), 0);
    assert_eq!(compiled.source.mechanics.len(), 0);
    assert_eq!(
        compiled
            .talent("rime-talent-id-talent7")
            .map(|talent| talent.kind),
        Some(CompiledTalentKind::Rime(7))
    );
    assert_eq!(
        compiled.mechanics[0].kind,
        CompiledMechanicKind::HeroSource(HeroSourceKind::Trait(
            TraitHeroSourceKind::WeaponHealDamageIncrease,
        ))
    );
    assert!(
        compiled
            .ability(DpsAbilityKind::BurstingIce)
            .expect("Bursting Ice index")
            .parameters
            .contains(parameter_key!("pulsePeriodSeconds"))
    );
    assert!(
        compiled
            .abilities
            .iter()
            .all(|ability| ability.source.mechanic_parameters.is_empty())
    );
    assert!(
        compiled
            .talents
            .iter()
            .all(|talent| talent.source.parameters.is_empty())
    );
    assert!(
        compiled
            .mechanics
            .iter()
            .all(|mechanic| mechanic.source.parameters.is_empty())
    );
}

#[test]
fn profile_compilation_rejects_missing_typed_shared_mechanic_parameters() {
    let mut profile = rime_profile();
    profile
        .mechanics
        .push(ardeos_mechanic("ItemTrait.ID.WeaponHealDamageIncrease", []));
    let request = rime_request(profile);

    let error = CompiledProfile::try_from(&request)
        .expect_err("typed shared mechanics require their executable parameters");

    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec!["ItemTrait.ID.WeaponHealDamageIncrease"]);
    assert!(error.message.contains("weaponDamageMultiplier"));
}

#[test]
fn modeled_shared_repeating_effects_require_positive_complete_cadences() {
    let cases = [
        (
            "gem-ruby-220",
            vec![("healingHealthFraction", 0.007), ("periodSeconds", 0.0)],
            SimulationErrorCode::InvalidBuild,
        ),
        (
            "gem-ruby-1000",
            vec![("healingHealthFraction", 0.0), ("periodSeconds", 2.0)],
            SimulationErrorCode::InvalidBuild,
        ),
        (
            "DynamicItemAbilityRank.12",
            vec![("durationSeconds", 5.0), ("intervalSeconds", 0.0)],
            SimulationErrorCode::InvalidBuild,
        ),
        (
            "ItemTrait.ID.ExtraDotHotOnEffectApplicationProc",
            vec![("durationSeconds", 5.0)],
            SimulationErrorCode::UncoveredMechanics,
        ),
        (
            "ItemTrait.ID.GemDotHotOnCrit",
            vec![("durationSeconds", 5.0), ("periodSeconds", -1.0)],
            SimulationErrorCode::InvalidBuild,
        ),
        (
            "ItemTrait.ID.GemPulsatingOnAbilityTotemProc",
            vec![
                ("initialDamagePulseDelaySeconds", 1.0),
                ("pulseIntervalSeconds", 0.0),
            ],
            SimulationErrorCode::InvalidBuild,
        ),
    ];
    for (source_id, parameters, expected_code) in cases {
        let mut request = rime_request(rime_profile());
        request
            .profile
            .mechanics
            .push(ardeos_mechanic(source_id, parameters));
        let error = validate(&request).expect_err("invalid shared cadence must fail");
        assert_eq!(error.code, expected_code, "{source_id}");
        assert_eq!(error.sources, vec![source_id.to_string()]);
    }
}

#[test]
fn modeled_unknown_hero_source_mechanics_are_rejected() {
    let mut request = rime_request(rime_profile());
    request.profile.mechanics.push(ardeos_mechanic(
        "test:unknown-modeled-mechanic",
        [("damageIncrease", 1.0)],
    ));

    let error = validate(&request).expect_err("unknown modeled mechanic must fail");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec!["test:unknown-modeled-mechanic"]);
}

#[test]
fn hero_contracts_reject_cross_hero_abilities_and_talents() {
    let mut ability_request = rime_request(rime_profile());
    ability_request
        .profile
        .abilities
        .push(ability(DpsAbilityKind::InfernalWave, 1.0));
    let error = validate(&ability_request).expect_err("Ardeos ability must fail for Rime");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec!["test:infernal-wave"]);

    let mut talent_request = rime_request(rime_profile());
    talent_request.profile.talents.push(DpsTalentModel {
            id: "firemage-talent-id-talent18".into(),
            name: "Ardeos talent 18".into(),
            mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-passivetalent9".into(),
            classification: MechanicClassification::Modeled,
            parameters: BTreeMap::from([("extensionPerTickSeconds".into(), 1.0)]),
            reason: None,
        });
    let error = validate(&talent_request).expect_err("Ardeos talent must fail for Rime");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec!["firemage-talent-id-talent18"]);
}

#[test]
fn partial_evidence_blocks_before_simulation() {
    let request = request(
        profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
        1,
        DpsEvidenceClaimStatus::Partial,
    );

    let error = validate(&request).expect_err("partial evidence must block");
    assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
    assert_eq!(error.sources, vec!["Test source"]);
}

#[test]
fn explicitly_accepted_approximate_evidence_passes_the_evidence_gate() {
    let request = request(
        profile(vec![ability(DpsAbilityKind::InfernalWave, 1.0)]),
        1,
        DpsEvidenceClaimStatus::Approximate,
    );

    let error = validate(&request).expect_err("minimal profile remains incomplete");
    assert_ne!(error.sources, vec!["Test source"]);
    assert_ne!(
        error.message,
        "The loadout contains outgoing-DPS mechanics without current evidence."
    );
}

#[test]
fn current_refund_profiles_reject_missing_grants_and_delay_inputs() {
    for (request, kind, parameters) in [
        (
            tariq_request(tariq_profile()),
            DpsAbilityKind::SkullCrusher,
            vec!["spiritRefundDelaySeconds", "spiritRefundSpiritGain"],
        ),
        (
            gunde_request(gunde_profile()),
            DpsAbilityKind::Rend,
            vec![
                "spiritRefundDelaySeconds",
                "spiritRefundSpiritGain",
                "spiritRefundFeathers",
            ],
        ),
    ] {
        validate(&request).expect("complete refund profile validates");
        for parameter in parameters {
            let mut missing = request.clone();
            let ability = missing
                .profile
                .abilities
                .iter_mut()
                .find(|ability| ability.kind == kind)
                .unwrap();
            let id = ability.id.clone();
            assert!(ability.mechanic_parameters.remove(parameter).is_some());
            let error = validate(&missing).expect_err("missing refund inputs must not become zero");
            assert_eq!(error.code, SimulationErrorCode::UncoveredMechanics);
            assert_eq!(error.sources, vec![id]);
        }
    }
}

#[test]
fn ruby_storm_accepts_reviewed_melee_approximation_but_rejects_changed_scaling() {
    for mut request in [
        tariq_request(tariq_profile()),
        mara_request(mara_profile()),
        gunde_request(gunde_profile()),
    ] {
        request.profile.mechanics = vec![ardeos_mechanic(
            "ItemTrait.ID.GemWhirlwindProc",
            [
                ("flatDamage", 1_000.0),
                ("gemPowerDamageIncreasePerPoint", 0.0),
                ("procsPerMinute", 1.3),
                ("ppmHasteScaling", 1.0),
                ("lifetimeSeconds", 6.0),
                ("movementSpeed", 350.0),
                ("collisionRadius", 250.0),
                ("oscillationAmplitude", 300.0),
                ("oscillationFrequencyDegreesPerSecond", 120.0),
            ],
        )];
        validate(&request).expect("reviewed melee geometry is supported");
        request.profile.mechanics[0]
            .parameters
            .insert("gemPowerDamageIncreasePerPoint".into(), 0.1);
        assert_eq!(
            validate(&request).unwrap_err().code,
            SimulationErrorCode::UncoveredMechanics
        );
        request.profile.mechanics[0]
            .parameters
            .insert("gemPowerDamageIncreasePerPoint".into(), 0.0);
        request.profile.mechanics[0]
            .parameters
            .insert("movementSpeed".into(), 0.0);
        assert_eq!(
            validate(&request).unwrap_err().code,
            SimulationErrorCode::UncoveredMechanics
        );
    }
}
