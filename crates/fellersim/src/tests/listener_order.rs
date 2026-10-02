use super::*;

const AMETHYST: &str = "ItemTrait.ID.GemDotHotOnCrit";
const COUNTER: &str = "ItemTrait.ID.CritsToIncreasedCritRating";
const PRIMARY: &str = "ItemTrait.ID.CritsToIncreasedPrimaryStatBuff";
const DIAMOND: &str = "ItemTrait.ID.GemSingleTargetProcOnDamageHeal";
const AURASTONE: &str = "ItemTrait.ID.GemPulsatingOnAbilityTotemProc";
const EMERALD: &str = "ItemTrait.ID.GemTargetedSpikeProc";

// Synthetic interaction-rich profiles, not calibrated player builds. Keeping
// the inputs here makes the sensitivity experiment repeatable without logs.
fn mechanics(variant: usize) -> Vec<DynamicMechanicInstance> {
    let mut critical = vec![
        ardeos_mechanic(
            AMETHYST,
            [
                ("triggerDamageFraction", 0.11),
                ("durationSeconds", 8.0),
                ("periodSeconds", 2.0),
            ],
        ),
        ardeos_mechanic(
            COUNTER,
            [
                ("criticalRating", 100.0),
                ("durationSeconds", 12.0),
                ("requiredCriticalStrikes", 10.0),
            ],
        ),
        ardeos_mechanic(
            PRIMARY,
            [
                ("procsPerMinute", 3.0),
                ("ppmHasteScaling", 1.0),
                ("durationSeconds", 10.0),
                ("powerMultiplier", 1.1),
            ],
        ),
        ardeos_mechanic(
            "seta-proc-intellect",
            [
                ("procsPerMinute", 3.0),
                ("ppmCriticalScaling", 1.0),
                ("durationSeconds", 14.0),
                ("powerMultiplier", 1.18),
                ("cooldownSeconds", 5.0),
            ],
        ),
    ];
    let mut ordinary = vec![
        ardeos_mechanic(
            AURASTONE,
            [
                ("damageAccumulationFraction", 0.12),
                ("initialDamagePulseDelaySeconds", 3.0),
                ("pulseIntervalSeconds", 3.0),
            ],
        ),
        ardeos_mechanic(
            EMERALD,
            [
                ("powerCoefficient", 1.0),
                ("procsPerMinute", 3.0),
                ("ppmHasteScaling", 1.0),
            ],
        ),
    ];
    if matches!(variant, 1 | 6 | 7) {
        critical.reverse();
    }
    if matches!(variant, 5 | 7) {
        ordinary.reverse();
    }
    let diamond = ardeos_mechanic(
        DIAMOND,
        [
            ("powerCoefficient", 1.0),
            ("procsPerMinute", 3.0),
            ("ppmHasteScaling", 1.0),
            ("debuffDurationSeconds", 20.0),
            ("damageIncreasePerStack", 0.4),
            ("maximumStacks", 5.0),
            ("harmoniousSoulDamageIncreasePerStack", 0.35),
            ("harmoniousSoulStacks", 0.0),
        ],
    );
    // Cast-listener order is identical in every variant. Only passive combat
    // listeners move; Aurastone is the sole on-cast listener among these peers.
    let mut result = vec![
        ardeos_mechanic(
            "DynamicItemAbilityRank.07",
            [("powerCoefficient", 1.0), ("maximumTargets", 4.0)],
        ),
        ardeos_mechanic(
            "DynamicItemAbilityRank.14",
            [("procChance", 0.25), ("powerCoefficient", 1.0)],
        ),
    ];
    let diamond_first = matches!(variant, 3 | 6 | 7);
    if diamond_first {
        result.push(diamond.clone());
    }
    if matches!(variant, 2 | 6 | 7) {
        result.extend(ordinary);
        result.extend(critical);
    } else {
        result.extend(critical);
        result.extend(ordinary);
    }
    if !diamond_first {
        result.push(diamond);
    }
    result
}

#[test]
#[ignore = "offline listener-order sensitivity experiment; run explicitly in release mode with --nocapture"]
fn listener_order_sensitivity() {
    const SEEDS: u64 = 1024;
    const NAMES: [&str; 8] = [
        "accepted-order",
        "reverse-critical-peers",
        "damage-group-first",
        "healing-group-first",
        "heretic-before-generic",
        "reverse-damage-peers",
        "combined-groups-and-critical-peers",
        "combined-all",
    ];
    let profiles = [
        ardeos_fixture_request().profile,
        rime_profile(),
        tariq_profile(),
        elarion_profile([
            elarion_ability(DpsAbilityKind::Multishot),
            elarion_ability(DpsAbilityKind::EventHorizon),
            elarion_ability(DpsAbilityKind::ElarionShoot),
        ]),
        gunde_profile(),
        mara_profile(),
    ];
    let cancelled = AtomicBool::new(false);
    for mut profile in profiles {
        profile.power = 100.0;
        profile.critical_strike = 0.25;
        profile.haste = 0.2;
        profile.heroism_duration_ms = 30_000;
        for ability in &mut profile.abilities {
            if ability_category(ability.kind) == AbilityCategory::Spirit {
                ability.spirit_cost = profile.max_spirit;
            }
        }
        let mut actions: Vec<_> = profile
            .abilities
            .iter()
            .filter(|a| {
                matches!(
                    ability_category(a.kind),
                    AbilityCategory::Spirit | AbilityCategory::Core | AbilityCategory::Basic
                ) || a.kind == DpsAbilityKind::WintersBlessing
            })
            .collect();
        actions.sort_by_key(|a| {
            if a.kind == DpsAbilityKind::WintersBlessing {
                0
            } else {
                match ability_category(a.kind) {
                    AbilityCategory::Spirit => 1,
                    AbilityCategory::Core => 2,
                    _ => 3,
                }
            }
        });
        let apl = apl(actions.iter().map(|a| {
            (
                ability_kind_id(a.kind),
                (a.kind == DpsAbilityKind::WintersBlessing).then(|| fight_window(0.0, 0.01)),
            )
        }));
        let variants: Vec<_> = (0..NAMES.len())
            .map(|variant| {
                profile.mechanics = mechanics(variant);
                CompiledProfile::compile_test(&profile)
            })
            .collect();
        for targets in [1, 5] {
            let mut damage = vec![Vec::new(); NAMES.len()];
            let mut procs = BTreeMap::<String, f64>::new();
            for seed in 1..=SEEDS {
                for (variant, compiled) in variants.iter().enumerate() {
                    let mut iteration =
                        Iteration::from_compiled(compiled, &apl, targets, seed, &cancelled);
                    iteration.test_heretic_before_generic = matches!(variant, 4 | 7);
                    iteration.shared.spirit = profile.max_spirit;
                    let result = iteration.run().expect("counterfactual iteration completes");
                    assert!(
                        result.damage.is_finite() && result.damage > 0.0,
                        "{} targets {targets} variant {variant} seed {seed}: no damage",
                        profile.hero_id
                    );
                    damage[variant].push(result.damage / (ENCOUNTER_DURATION_MS as f64 / 1000.0));
                    if variant == 0 {
                        for (mechanic, count) in
                            compiled.mechanics.iter().zip(&result.mechanic_proc_counts)
                        {
                            *procs.entry(mechanic.source_id.clone()).or_default() += count;
                        }
                    }
                }
            }
            for id in [
                "DynamicItemAbilityRank.07",
                "DynamicItemAbilityRank.14",
                COUNTER,
                PRIMARY,
                DIAMOND,
                AURASTONE,
                EMERALD,
                AMETHYST,
            ] {
                assert!(
                    procs.get(id).copied().unwrap_or_default() > 0.0,
                    "{} targets {targets}: missing {id} coverage",
                    profile.hero_id
                );
            }
            let baseline = damage[0].iter().sum::<f64>() / SEEDS as f64;
            for (variant, samples) in damage.iter().enumerate() {
                let deltas: Vec<_> = samples.iter().zip(&damage[0]).map(|(a, b)| a - b).collect();
                let mean = deltas.iter().sum::<f64>() / SEEDS as f64;
                let variance =
                    deltas.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (SEEDS - 1) as f64;
                let half_interval = 1.96 * (variance / SEEDS as f64).sqrt();
                println!(
                    "LISTENER_SENSITIVITY {}",
                    serde_json::json!({"hero":profile.hero_id,"targets":targets,"variant":NAMES[variant],"seeds":SEEDS,"durationSeconds":ENCOUNTER_DURATION_MS/1000,"baselineMeanDps":baseline,"meanDps":baseline+mean,"meanDeltaPercent":mean/baseline*100.0,"pairedMeanDelta95HalfWidthPercent":half_interval/baseline*100.0,"maximumAbsolutePairedDeltaPercent":deltas.iter().map(|d|d.abs()/baseline*100.0).fold(0.0,f64::max),"baselineProcCounts":procs})
                );
            }
        }
    }
}
