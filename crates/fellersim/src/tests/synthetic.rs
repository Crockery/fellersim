use super::*;
use crate::preparation::*;
use serde_json::Value;

fn fixtures() -> Vec<Value> {
    serde_json::from_reader(flate2::read::GzDecoder::new(
        &include_bytes!("fixtures/preparation.json.gz")[..],
    ))
    .unwrap()
}

fn default_apl_source(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = [
        root.join("apps/fellersim/default-apls"),
        root.join("default-apls"),
    ]
    .into_iter()
    .find(|p| p.is_dir())
    .unwrap();
    std::fs::read_to_string(directory.join(format!("{name}.apl"))).unwrap()
}

fn options(duration_seconds: u32) -> SimulationOptions {
    SimulationOptions {
        duration_seconds,
        iterations: 100,
        targets: 1,
        seed: Some("0123456789abcdef".into()),
    }
}

fn equipment(hero: &str) -> CharacterBuild {
    let mut build: CharacterBuild = serde_json::from_value(
        fixtures()
            .into_iter()
            .rev()
            .find(|f| f["character"]["heroId"] == hero)
            .unwrap()["character"]
            .clone(),
    )
    .unwrap();
    // The historical preparation corpus also includes intentionally impossible multi-legendary cases.
    // Retain a single legendary to exercise mechanically valid conversion.
    for position in &mut build.positions {
        if position.item.as_ref().is_some_and(|i| {
            i.item_id.starts_with("legendary-") && !i.item_id.starts_with("legendary-ring")
        }) {
            position.item = None;
        }
    }
    // Preserve an alternate source position, not just the catalog's default ring identity.
    let first = build
        .positions
        .iter()
        .position(|p| p.position_id == "ring-1")
        .unwrap();
    let second = build
        .positions
        .iter()
        .position(|p| p.position_id == "ring-2")
        .unwrap();
    let item = build.positions[first].item.take();
    build.positions[first].item = build.positions[second].item.take();
    build.positions[second].item = item;
    build
}

#[test]
fn equipment_corpus_resolves_to_identical_synthetic_profiles() {
    let mut checked = 0;
    for fixture in fixtures() {
        let build: CharacterBuild = serde_json::from_value(fixture["character"].clone()).unwrap();
        if build
            .positions
            .iter()
            .filter(|p| {
                p.item
                    .as_ref()
                    .is_some_and(|i| i.item_id.starts_with("legendary-"))
            })
            .count()
            > 1
        {
            continue;
        }
        let targets = fixture["targets"].as_u64().unwrap() as u32;
        let synthetic = synthetic_from_character(&build).unwrap();
        assert_eq!(
            prepare_character(&build, targets).unwrap(),
            prepare_synthetic_build(&synthetic, targets).unwrap(),
            "{} fixture {checked}",
            build.hero_id
        );
        checked += 1;
    }
    assert!(checked > 3000);
}

#[test]
fn equipped_and_synthetic_combat_match_for_every_hero_at_both_durations() {
    for (hero, name) in [
        ("firemage", "ardeos"),
        ("rime", "rime"),
        ("ink", "tariq"),
        ("bowguy", "elarion"),
        ("mara", "mara"),
        ("gunde", "gunde"),
    ] {
        let build = equipment(hero);
        let synthetic = synthetic_from_character(&build).unwrap();
        let apl = default_apl_source(name);
        for duration in [300, 660] {
            let opts = options(duration);
            let conventional = prepare(&SimulationInput {
                schema_version: INPUT_SCHEMA_VERSION,
                run_id: "equivalence".into(),
                data_build_id: fellersim_data::build_id().into(),
                character: build.clone(),
                apl_source: apl.clone(),
                options: opts.clone(),
            })
            .unwrap();
            let synthetic = prepare_synthetic(&synthetic, &apl, &opts, "equivalence").unwrap();
            assert_eq!(conventional.profile, synthetic.profile);
            assert_eq!(
                conventional.profile_fingerprint,
                synthetic.profile_fingerprint
            );
            let expected = simulate(&conventional, &AtomicBool::new(false), |_| {}).unwrap();
            let mut actual = simulate(&synthetic, &AtomicBool::new(false), |_| {}).unwrap();
            assert!(actual.exploratory);
            actual.exploratory = false;
            assert_eq!(actual, expected, "{hero} at {duration}s");
        }
    }
}

#[test]
fn synthetic_ratings_and_effects_are_independent_and_reversible() {
    let mut build = SyntheticBuild::new("gunde").unwrap();
    build.secondary_ratings = SecondaryRatings {
        crit: 100.0,
        haste: 250.0,
        expertise: 75.0,
        spirit: 125.0,
    };
    let (baseline, _) = prepare_synthetic_build(&build, 1).unwrap();
    let delta = SecondaryRatings {
        haste: 100.0,
        ..Default::default()
    };
    let changed = build.with_rating_delta(delta).unwrap();
    let (profile, _) = prepare_synthetic_build(&changed, 1).unwrap();
    assert_eq!(profile.haste_rating, baseline.haste_rating + 100.0);
    assert!(profile.haste > baseline.haste);
    assert_eq!(
        (
            profile.critical_strike,
            profile.expertise,
            profile.spirit,
            profile.power
        ),
        (
            baseline.critical_strike,
            baseline.expertise,
            baseline.spirit,
            baseline.power
        )
    );
    assert_eq!(
        changed
            .with_rating_delta(SecondaryRatings {
                haste: -100.0,
                ..Default::default()
            })
            .unwrap(),
        build
    );
    assert!(
        build
            .with_rating_delta(SecondaryRatings {
                haste: -251.0,
                ..Default::default()
            })
            .is_err()
    );
    let id = "ItemTrait.ID.AbilityToIncreasedMainStat";
    for rank in 1..=4 {
        build.traits.insert(id.into(), rank);
        let (profile, _) = prepare_synthetic_build(&build, 1).unwrap();
        assert_eq!(profile.mechanics.len(), 1);
        let m = &profile.mechanics[0];
        assert_eq!(m.source_id, id);
        assert_eq!(m.parameters["powerMultiplier"], fellersim_data::catalog()["traits"][id]["models"][rank.to_string()]["parameters"]["powerMultiplier"].as_f64().unwrap());
    }
    build.traits.clear();
    assert_eq!(prepare_synthetic_build(&build, 1).unwrap().0, baseline);
    // A complete legendary bundle applies its inherent multiplier exactly once.
    build.legendary = Some(SyntheticEffectSelection {
        id: "legendary-back-a-haste".into(),
        source_position_id: None,
    });
    let (legendary, _) = prepare_synthetic_build(&build, 1).unwrap();
    assert_eq!(legendary.power, baseline.power * 1.1);
    assert_eq!(legendary.haste_rating, baseline.haste_rating);
}

#[test]
fn synthetic_gems_use_existing_track_thresholds_and_set_multiplier() {
    let data = fellersim_data::catalog();
    for gem in data["gems"].as_array().unwrap() {
        let family = gem["family"].as_str().unwrap();
        let threshold = gem["requiredPower"].as_f64().unwrap();
        for offset in [-1.0, 0.0, 1.0] {
            let mut build = SyntheticBuild::new("gunde").unwrap();
            build.gem_power.insert(family.into(), threshold + offset);
            let (profile, _) = prepare_synthetic_build(&build, 1).unwrap();
            let active = profile
                .mechanics
                .iter()
                .any(|m| m.instance_id == format!("active-gem:{}", gem["id"].as_str().unwrap()));
            assert_eq!(active, offset >= 0.0, "{} {offset}", gem["id"]);
        }
    }
    let mut build = SyntheticBuild::new("gunde").unwrap();
    build.gem_power.insert("Ruby".into(), 64.0);
    for count in [0, 1, 2, 3] {
        build.set_bonuses.insert(
            "sete-percentage-hdt".into(),
            SyntheticSetBonus {
                piece_count: count,
                source_position_id: None,
            },
        );
        let (profile, _) = prepare_synthetic_build(&build, 1).unwrap();
        assert_eq!(
            profile
                .mechanics
                .iter()
                .any(|m| m.instance_id == "active-gem:gem-ruby-80"),
            count >= 2
        );
    }
}

#[test]
fn synthetic_validation_reports_invalid_fields_and_preserves_talent_rules() {
    let baseline = SyntheticBuild::new("gunde").unwrap();
    for bad in [-1.0, f64::NAN, f64::INFINITY, 1_000_001.0] {
        let mut b = baseline.clone();
        b.secondary_ratings.crit = bad;
        assert_eq!(
            diagnose_synthetic(&b)[0].path.as_deref(),
            Some("/secondaryRatings/crit")
        );
    }
    for (id, rank) in [
        ("unknown", 1),
        ("ItemTrait.ID.AbilityToIncreasedMainStat", 0),
        ("ItemTrait.ID.AbilityToIncreasedMainStat", 5),
    ] {
        let mut b = baseline.clone();
        b.traits.insert(id.into(), rank);
        assert!(
            prepare_synthetic_build(&b, 1)
                .unwrap_err()
                .message
                .contains(id)
        );
    }
    let mut b = baseline.clone();
    b.gem_power.insert("unknown".into(), 100.0);
    assert_eq!(diagnose_synthetic(&b)[0].code, "invalid-gem-power");
    let mut b = baseline.clone();
    b.set_bonuses.insert(
        "setc-proc-intellect".into(),
        SyntheticSetBonus {
            piece_count: 2,
            source_position_id: None,
        },
    );
    assert_eq!(diagnose_synthetic(&b)[0].code, "invalid-set-bonus");
    let mut b = baseline.clone();
    b.weapon = Some(SyntheticEffectSelection {
        id: "weapon-a-haste-rime".into(),
        source_position_id: None,
    });
    assert_eq!(diagnose_synthetic(&b)[0].code, "invalid-effect");
    let mut b = baseline;
    b.selected_talent_ids.push("invalid".into());
    assert_eq!(
        diagnose_synthetic(&b)[0].code,
        "unknown-or-duplicate-talent"
    );

    let mut duplicate = SyntheticBuild::new("gunde").unwrap();
    duplicate.relics = vec![
        SyntheticEffectSelection {
            id: "relic-a-haste-healthpotion".into(),
            source_position_id: None,
        },
        SyntheticEffectSelection {
            id: "relic-a-haste-healthpotion".into(),
            source_position_id: Some("trinket-1".into()),
        },
    ];
    assert_eq!(diagnose_synthetic(&duplicate)[0].code, "invalid-effect");
}

#[test]
fn infinite_weapon_effects_remain_active_past_five_minutes() {
    let mut build = SyntheticBuild::new("rime").unwrap();
    let hero = &fellersim_data::catalog()["heroes"]["rime"];
    let id = hero["items"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, item)| item["weapon"]["kind"] == "weapon-frost-volley")
        .unwrap()
        .0;
    build.weapon = Some(SyntheticEffectSelection {
        id: id.clone(),
        source_position_id: None,
    });
    let request = prepare_synthetic(
        &build,
        "actions=/weapon_frost_volley,if=fight.elapsed<1",
        &options(660),
        "infinite",
    )
    .unwrap();
    let profile = CompiledProfile::try_from(&request).unwrap();
    let cancelled = AtomicBool::new(false);
    let iteration =
        Iteration::from_compiled(&profile, &request.action_priority_list, 1, 1, &cancelled);
    let result = iteration.run().unwrap();
    // Only one application: the periodic effect must survive the old 300s horizon.
    let uptime = result.uptimes["dot:weapon-frost-volley"];
    assert!(uptime > 0.9, "infinite effect uptime: {uptime}");
}

#[test]
fn synthetic_batch_is_repeatable_isolated_and_has_complete_cache_identity() {
    let base = SyntheticBuild::new("gunde").unwrap();
    let variant = base
        .with_rating_delta(SecondaryRatings {
            haste: 100.0,
            ..Default::default()
        })
        .unwrap();
    let mut invalid = base.clone();
    invalid.secondary_ratings.haste = -1.0;
    let ctx = EvaluationContext::new("actions=/double_strike").unwrap();
    let opts = options(1);
    let cancel = AtomicBool::new(false);
    let results: Vec<_> = ctx
        .evaluate_batch([&base, &invalid, &variant, &base], &opts, &cancel)
        .collect();
    assert!(results[1].is_err());
    for index in [0, 2, 3] {
        let expected = ctx
            .evaluate(
                if index == 2 { &variant } else { &base },
                &opts,
                &format!("synthetic-{index}"),
                &cancel,
            )
            .unwrap();
        assert_eq!(results[index].as_ref().unwrap(), &expected);
    }
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(ctx.evaluate_batch([&base], &opts, &cancel).count(), 0);
    let json = variant.canonical_json().unwrap();
    let roundtrip: SyntheticBuild = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, variant);
    assert_eq!(roundtrip.canonical_json().unwrap(), json);
    let key = ctx.cache_key(&base, &opts).unwrap();
    for modified in [
        SimulationOptions {
            iterations: 101,
            ..opts.clone()
        },
        SimulationOptions {
            duration_seconds: 2,
            ..opts.clone()
        },
        SimulationOptions {
            seed: Some("0000000000000001".into()),
            ..opts.clone()
        },
    ] {
        assert_ne!(key, ctx.cache_key(&base, &modified).unwrap());
    }
    assert_ne!(key, ctx.cache_key(&variant, &opts).unwrap());
}

#[test]
fn encounter_duration_controls_apl_cutoffs_and_damage_denominators() {
    let b = SyntheticBuild::new("gunde").unwrap();
    for duration in [1, 300, 660, 1200] {
        let request = prepare_synthetic(
            &b,
            "actions=/double_strike,if=fight.remains<2",
            &options(duration),
            "duration",
        )
        .unwrap();
        let result = trace(
            &request,
            TraceOptions {
                iteration_index: 0,
                until_ms: u64::from(duration) * 1000,
                max_decisions: 10000,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            result.result.scenario.duration_ms,
            u64::from(duration) * 1000
        );
        let damage: f64 = result.result.abilities.iter().map(|a| a.mean_damage).sum();
        assert!((damage / f64::from(duration) - result.result.mean_dps).abs() < 1e-9);
        let casts: Vec<_> = result
            .decisions
            .iter()
            .filter(|d| d.selected_ability.is_some())
            .collect();
        assert!(!casts.is_empty());
        assert!(casts.iter().all(|d| d.time_ms <= u64::from(duration) * 1000
            && d.time_ms > u64::from(duration.saturating_sub(2)) * 1000
            || duration == 1));
        assert!(
            result
                .result
                .uptimes
                .iter()
                .all(|u| u.mean_uptime >= 0.0 && u.mean_uptime <= 1.0)
        );
    }
    for duration in [0, 1201] {
        assert!(
            prepare_synthetic(&b, "actions=/double_strike", &options(duration), "bad").is_err()
        );
    }
}

#[test]
#[ignore = "run explicitly in release mode with --test-threads=1"]
fn synthetic_build_benchmark() {
    use std::{hint::black_box, time::Instant};
    let character = equipment("gunde");
    let build = synthetic_from_character(&character).unwrap();
    let apl = default_apl_source("gunde");
    let ctx = EvaluationContext::new(&apl).unwrap();
    for repeat in 0..3 {
        for stage in [
            "construct",
            "clone",
            "resolve-synthetic",
            "resolve-equipment",
        ] {
            allocation_counter::reset();
            let start = Instant::now();
            for _ in 0..1000 {
                match stage {
                    "construct" => {
                        black_box(SyntheticBuild::new("gunde").unwrap());
                    }
                    "clone" => {
                        black_box(build.clone());
                    }
                    "resolve-synthetic" => {
                        black_box(prepare_synthetic_build(&build, 1).unwrap());
                    }
                    _ => {
                        black_box(prepare_character(&character, 1).unwrap());
                    }
                }
            }
            eprintln!(
                "SYNTHETIC_BENCH repeat={repeat} stage={stage} count=1000 elapsed_us={} allocations={:?}",
                start.elapsed().as_micros(),
                allocation_counter::metrics()
            );
        }
        for duration in [300, 660] {
            let opts = options(duration);
            let conventional = prepare(&SimulationInput {
                schema_version: INPUT_SCHEMA_VERSION,
                run_id: "benchmark".into(),
                data_build_id: fellersim_data::build_id().into(),
                character: character.clone(),
                apl_source: apl.clone(),
                options: opts.clone(),
            })
            .unwrap();
            let synthetic = ctx.prepare(&build, &opts, "benchmark").unwrap();
            for (name, request) in [("equipment", &conventional), ("synthetic", &synthetic)] {
                allocation_counter::reset();
                let start = Instant::now();
                black_box(simulate(request, &AtomicBool::new(false), |_| {}).unwrap());
                eprintln!(
                    "SYNTHETIC_BENCH repeat={repeat} stage=simulate-{name} duration={duration} iterations=100 elapsed_us={} allocations={:?}",
                    start.elapsed().as_micros(),
                    allocation_counter::metrics()
                );
            }
            let start = Instant::now();
            for result in ctx.evaluate_batch(
                std::iter::repeat_n(&build, 10),
                &opts,
                &AtomicBool::new(false),
            ) {
                black_box(result.unwrap());
            }
            eprintln!(
                "SYNTHETIC_BENCH repeat={repeat} stage=batch duration={duration} count=10 iterations=100 elapsed_us={}",
                start.elapsed().as_micros()
            );
        }
    }
}
