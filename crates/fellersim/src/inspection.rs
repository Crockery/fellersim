//! Read-only compiler and execution observations, excluded from semantic fingerprints.
use crate::*;
use serde::Serialize;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AplRuleExplanation {
    pub rule_id: String,
    pub ability_id: String,
    pub status: String,
    pub original_condition: Option<AplExpressionNode>,
    pub resolved_condition: Option<AplExpressionNode>,
    pub active_from_ms: Option<u64>,
    pub active_until_ms: Option<u64>,
}
pub fn explain_apl(
    request: &SimulationRequest,
) -> Result<Vec<AplRuleExplanation>, SimulationError> {
    let profile = CompiledProfile::try_from(request)?;
    Ok(compile_runtime_apl_report(
        &request.action_priority_list,
        &profile,
        request.scenario.target_count,
    )
    .2)
}

/// Enumerate the same enum variants and hero legality predicates used by validation.
fn enum_values<T: schemars::JsonSchema + serde::de::DeserializeOwned>() -> Vec<T> {
    let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    schema["enum"]
        .as_array()
        .expect("string enum schema")
        .iter()
        .map(|v| serde_json::from_value(v.clone()).unwrap())
        .collect()
}
pub fn supported_resources(hero_id: &str) -> Vec<AplResource> {
    enum_values()
        .into_iter()
        .filter(|r| validation::resource_supported(*r, hero_id))
        .collect()
}
pub fn supported_buffs(profile: &NormalizedDpsProfile) -> Vec<AplBuff> {
    enum_values()
        .into_iter()
        .filter(|r| validation::validate_apl_buff(*r, profile).is_ok())
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct TraceOptions {
    pub iteration_index: u32,
    pub until_ms: u64,
    pub max_decisions: usize,
}
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceRule {
    pub rule_id: String,
    pub blocker: Option<String>,
    pub nodes: Vec<AplNodeEvaluationTrace>,
    pub selected: bool,
}
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceDecision {
    pub time_ms: u64,
    pub rules: Vec<TraceRule>,
    pub state: BTreeMap<String, f64>,
    pub selected_ability: Option<String>,
}
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulationTrace {
    pub iteration_index: u32,
    pub decisions: Vec<TraceDecision>,
    pub truncated: bool,
    pub result: SimulationResult,
    pub work_units: u32,
}
pub fn trace(
    request: &SimulationRequest,
    options: TraceOptions,
    cancelled: &AtomicBool,
) -> Result<SimulationTrace, SimulationError> {
    if options.until_ms > ENCOUNTER_DURATION_MS
        || !(1..=10000).contains(&options.max_decisions)
        || options.iteration_index >= MAX_SIMULATION_ITERATIONS
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "Trace requires a duration within the encounter, 1–10000 decisions, and an iteration index below 100000.",
        ));
    }
    let profile = CompiledProfile::try_from(request)?;
    let mut iteration = Iteration::from_compiled(
        &profile,
        &request.action_priority_list,
        request.scenario.target_count,
        split_seed(profile.validated_seed, options.iteration_index),
        cancelled,
    )
    .run_observed(Some(options))?;
    let decisions = std::mem::take(&mut iteration.trace);
    let truncated = iteration.trace_truncated;
    let work_units = iteration.work_units;
    let mut single = request.clone();
    single.iterations = 1;
    let result = aggregate::aggregate(&single, &profile, std::iter::once(iteration))?;
    Ok(SimulationTrace {
        iteration_index: options.iteration_index,
        decisions,
        truncated,
        result,
        work_units,
    })
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AplReferenceInfo {
    pub token: String,
    pub value_type: String,
    pub reference: serde_json::Value,
}
/// Discovery passes each token through the actual parser and semantic validator.
pub fn apl_references(profile: &NormalizedDpsProfile) -> Vec<AplReferenceInfo> {
    let mut tokens = Vec::new();
    for token in [
        "targets.count",
        "target.health.pct",
        "target.time_to_die",
        "fight.elapsed",
        "fight.remains",
    ] {
        tokens.push(token.to_string());
    }
    for ability in &profile.abilities {
        let id = serde_json::to_value(ability.kind)
            .unwrap()
            .as_str()
            .unwrap()
            .replace('-', "_");
        for (prefix, properties) in [
            ("cooldown", &["ready", "remains", "charges"][..]),
            ("dot", &["up", "remains"][..]),
        ] {
            for property in properties {
                tokens.push(format!("{prefix}.{id}.{property}"));
            }
        }
    }
    for resource in supported_resources(&profile.hero_id) {
        let id = serde_json::to_value(resource)
            .unwrap()
            .as_str()
            .unwrap()
            .replace('-', "_");
        for property in ["current", "max", "deficit", "percent"] {
            tokens.push(format!("resource.{id}.{property}"));
        }
    }
    for buff in supported_buffs(profile) {
        let id = serde_json::to_value(buff)
            .unwrap()
            .as_str()
            .unwrap()
            .replace('-', "_");
        for property in ["up", "remains", "stacks"] {
            tokens.push(format!("buff.{id}.{property}"));
        }
    }
    for effect in &profile.apl_target_effects {
        for property in ["up", "remains", "stacks"] {
            tokens.push(format!("debuff.{}.{property}", effect.id.replace('-', "_")));
        }
    }
    for (id, _) in fellersim_data::catalog()["heroes"][&profile.hero_id]["talents"]
        .as_object()
        .unwrap()
    {
        tokens.push(format!("talent.{id}.enabled"));
    }
    for (id, _) in fellersim_data::catalog()["heroes"][&profile.hero_id]["items"]
        .as_object()
        .unwrap()
    {
        if let Some(id) = id.strip_prefix("legendary-") {
            tokens.push(format!("legendary.{}.equipped", id.replace('-', "_")));
        }
    }
    tokens.into_iter().filter_map(|token| {
        for boolean in [true,false] {
            let Some(reference) = apl_source::reference(&token, boolean) else { continue; };
            let expression = if boolean { serde_json::json!({"kind":"boolean-reference", "reference":reference}) }
                else { serde_json::json!({"kind":"comparison", "left":{"kind":"reference","reference":reference},"operator":"greater-than","right":{"kind":"number","value":0.0}}) };
            let mut node = expression; node["id"] = serde_json::json!("discovery");
            let Ok(node) = serde_json::from_value(node) else { continue; };
            if validation::validate_apl_expression(&node, profile, &mut Default::default(), 0).is_ok() {
                return Some(AplReferenceInfo { token, value_type: if boolean { "boolean" } else { "number" }.into(), reference });
            }
        }
        None
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use preparation::*;
    fn request(hero: &str, name: &str) -> SimulationRequest {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let apl_path = [
            root.join("apps/fellersim/default-apls"),
            root.join("default-apls"),
        ]
        .into_iter()
        .find(|p| p.is_dir())
        .unwrap()
        .join(format!("{name}.apl"));
        prepare(&SimulationInput {
            schema_version: 1,
            run_id: "inspection-test".into(),
            data_build_id: fellersim_data::build_id().into(),
            character: empty_character(hero).unwrap(),
            apl_source: std::fs::read_to_string(apl_path).unwrap(),
            options: SimulationOptions {
                iterations: 100,
                targets: 3,
                seed: Some("0123456789abcdef".into()),
            },
        })
        .unwrap()
    }
    #[test]
    fn tracing_preserves_all_hero_results_rng_and_work_with_capture_limits() {
        for (hero, name) in [
            ("firemage", "ardeos"),
            ("rime", "rime"),
            ("ink", "tariq"),
            ("bowguy", "elarion"),
            ("mara", "mara"),
            ("gunde", "gunde"),
        ] {
            let request = request(hero, name);
            let profile = CompiledProfile::try_from(&request).unwrap();
            let cancel = AtomicBool::new(false);
            let seed = split_seed(profile.validated_seed, 7);
            let normal =
                Iteration::from_compiled(&profile, &request.action_priority_list, 3, seed, &cancel)
                    .run()
                    .unwrap();
            for limits in [(0, 1), (10_000, 200), (ENCOUNTER_DURATION_MS, 10000)] {
                let observed = trace(
                    &request,
                    TraceOptions {
                        iteration_index: 7,
                        until_ms: limits.0,
                        max_decisions: limits.1,
                    },
                    &cancel,
                )
                .unwrap();
                let mut single = request.clone();
                single.iterations = 1;
                let expected =
                    aggregate::aggregate(&single, &profile, std::iter::once(normal.clone()))
                        .unwrap();
                assert_eq!(observed.result, expected, "{hero}");
                assert_eq!(observed.work_units, normal.work_units, "{hero}");
                assert!(!observed.decisions.is_empty());
                if limits.1 == 1 {
                    assert!(observed.truncated);
                }
            }
        }
    }
    #[test]
    fn compiler_exclusions_and_short_circuit_observations_use_runtime_rules() {
        let mut request = request("firemage", "ardeos");
        let source = "# actions=/detonate\nactions+=/weapon_frost_volley\nactions+=/fire_ball,if=targets.count<0\nactions+=/fire_ball,if=targets.count>0&resource.spirit.current<0&cooldown.fire_ball.ready\nactions+=/fire_ball";
        request.action_priority_list = parse_apl(source).unwrap();
        let explained = explain_apl(&request).unwrap();
        assert_eq!(explained[0].status, "disabled");
        assert_eq!(explained[1].status, "unavailable");
        assert_eq!(explained[2].status, "statically-false");
        assert_eq!(explained[3].status, "simplified");
        let observed = trace(
            &request,
            TraceOptions {
                iteration_index: 0,
                until_ms: 10_000,
                max_decisions: 200,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            observed
                .decisions
                .iter()
                .flat_map(|d| &d.rules)
                .flat_map(|r| &r.nodes)
                .any(|n| n.short_circuited)
        );
        assert!(
            observed
                .decisions
                .iter()
                .flat_map(|d| &d.rules)
                .any(|r| r.blocker.as_deref() == Some("condition-false"))
        );
    }
    #[test]
    fn worker_budgets_and_metric_samples_are_reproducible() {
        let request = request("mara", "mara");
        let cancel = AtomicBool::new(false);
        let (one, samples) = simulate_with_samples(
            &request,
            &cancel,
            |_| {},
            1,
            Some(ComparisonMetric::TotalDps),
        )
        .unwrap();
        let (four, parallel) = simulate_with_samples(
            &request,
            &cancel,
            |_| {},
            4,
            Some(ComparisonMetric::TotalDps),
        )
        .unwrap();
        assert_eq!(one, four);
        assert_eq!(samples, parallel);
        assert_eq!(
            samples.iter().sum::<f64>() / samples.len() as f64,
            one.mean_dps
        );
        let (primary, primary_samples) = simulate_with_samples(
            &request,
            &cancel,
            |_| {},
            2,
            Some(ComparisonMetric::PrimaryTargetDps),
        )
        .unwrap();
        assert_eq!(one, primary);
        assert!(
            (primary_samples.iter().sum::<f64>() / primary_samples.len() as f64
                - primary.primary_target_dps)
                .abs()
                < 1e-8
        );
    }
}

#[cfg(test)]
mod weapon_trace_tests {
    use super::*;
    use preparation::*;
    #[test]
    fn equipped_optional_weapons_use_the_observed_cast_path_without_changing_results() {
        for (hero_id, hero) in fellersim_data::catalog()["heroes"].as_object().unwrap() {
            let (item_id, item) = hero["items"]
                .as_object()
                .unwrap()
                .iter()
                .find(|(_, item)| !item["weapon"].is_null())
                .unwrap();
            let (rarity, levels) = item["validItemLevelsByRarity"]
                .as_object()
                .unwrap()
                .iter()
                .next()
                .unwrap();
            let mut character = empty_character(hero_id).unwrap();
            character
                .positions
                .iter_mut()
                .find(|p| p.position_id == "weapon")
                .unwrap()
                .item = Some(CharacterItem {
                item_id: item_id.clone(),
                item_level: levels[0].as_u64().unwrap() as u32,
                rarity: rarity.clone(),
                applied_tempers: 0,
                rolled_modifiers: vec![],
                gems: vec![],
                trait_tree: None,
                blessings: vec![],
            });
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let directory = [
                root.join("default-apls"),
                root.join("apps/fellersim/default-apls"),
            ]
            .into_iter()
            .find(|p| p.is_dir())
            .unwrap();
            let apl_source = std::fs::read_to_string(directory.join(format!(
                "{}.apl",
                hero["name"].as_str().unwrap().to_lowercase()
            )))
            .unwrap();
            let request = prepare(&SimulationInput {
                schema_version: 1,
                run_id: "weapon-trace".into(),
                data_build_id: fellersim_data::build_id().into(),
                character,
                apl_source,
                options: SimulationOptions {
                    iterations: 100,
                    targets: 1,
                    seed: None,
                },
            })
            .unwrap();
            let cancel = AtomicBool::new(false);
            let compiled = CompiledProfile::try_from(&request).unwrap();
            let ordinary = Iteration::from_compiled(
                &compiled,
                &request.action_priority_list,
                1,
                split_seed(compiled.validated_seed, 0),
                &cancel,
            )
            .run()
            .unwrap();
            let traced = trace(
                &request,
                TraceOptions {
                    iteration_index: 0,
                    until_ms: ENCOUNTER_DURATION_MS,
                    max_decisions: 10000,
                },
                &cancel,
            )
            .unwrap();
            let mut single = request.clone();
            single.iterations = 1;
            assert_eq!(
                traced.result,
                aggregate::aggregate(&single, &compiled, std::iter::once(ordinary.clone()))
                    .unwrap()
            );
            assert_eq!(traced.work_units, ordinary.work_units);
            let weapon_kind = item["weapon"]["kind"].as_str().unwrap();
            let rules = explain_apl(&request).unwrap();
            assert!(
                rules
                    .iter()
                    .any(|r| r.ability_id == weapon_kind && r.status != "unavailable")
            );
        }
    }
}
