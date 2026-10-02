use crate::{
    catalog,
    command::*,
    discovery,
    input::*,
    output::{self, CaseStatus, Failure, Outcome},
};
use fellersim_core::{preparation::*, *};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

pub async fn execute(command: Command, quiet: bool) -> Result<Outcome, Failure> {
    let started = Instant::now();
    match command {
        Command::Version => Ok(Outcome::success(output::versions())),
        Command::Describe { command } => {
            Ok(Outcome::success(discovery::describe(command.as_deref())?))
        }
        Command::Schema { document } => Ok(Outcome::artifact(discovery::schema(&document)?, None)),
        Command::Catalog(args) => Ok(Outcome::success(catalog::search(args)?)),
        Command::Character(CharacterCommand::Init { hero }) => Ok(Outcome::artifact(
            CharacterFile::new(empty_character(&hero).map_err(Failure::from)?),
            None,
        )),
        Command::Apl(AplCommand::Default { hero }) => {
            let source = default_apl(&hero).ok_or_else(|| {
                Failure::input(
                    "unsupported-hero",
                    "Use catalog heroes to select a supported hero.",
                    "/hero",
                )
            })?;
            Ok(Outcome::artifact(
                json!({"heroId":hero,"source":source,"fingerprint":format!("{:x}",Sha256::digest(source.as_bytes()))}),
                Some(source.trim_end_matches('\n').into()),
            ))
        }
        Command::Validate(args) => {
            let request = load(&args)?.prepare()?;
            Ok(Outcome::success(
                json!({"valid":true,"dataBuildId":request.data_build_id,"modelVersion":request.model_version,"seed":request.seed,"profileFingerprint":request.profile_fingerprint}),
            ))
        }
        Command::Prepare(args) => inspect(args, false),
        Command::Apl(AplCommand::Explain(args)) => inspect(args, true),
        Command::Run(args) => {
            let request = load(&args.inspection.input)?.prepare()?;
            let (result, reason) = controlled(args.timeout, started, move |cancel| {
                simulate_with_samples(
                    &request,
                    &cancel,
                    |p| {
                        if !quiet
                            && (p.completed_iterations == 0
                                || p.completed_iterations == p.total_iterations)
                        {
                            output::progress(format_args!(
                                "Iterations: {}/{}",
                                p.completed_iterations, p.total_iterations
                            ));
                        }
                    },
                    MAX_SIMULATION_WORKERS,
                    None,
                )
            })
            .await?;
            match result {
                Ok((result, _)) => Ok(Outcome::success(output::result(
                    &result,
                    args.inspection.detail,
                ))),
                Err(e) => Err(stopped(e, reason)),
            }
        }
        Command::Trace(args) => {
            let loaded = load(&args.execution.inspection.input)?;
            let request = loaded.prepare()?;
            let source_map = parse_apl_document(loaded.apl.as_ref().unwrap())
                .map_err(Failure::from)?
                .source_map;
            let rules = explain_apl(&request).map_err(Failure::from)?;
            let options = TraceOptions {
                iteration_index: args.iteration_index,
                until_ms: u64::from(args.seconds) * 1000,
                max_decisions: args.decisions as usize,
            };
            let (result, reason) = controlled(args.execution.timeout, started, move |cancel| {
                trace(&request, options, &cancel)
            })
            .await?;
            let trace = result.map_err(|e| stopped(e, reason))?;
            Ok(Outcome::success(
                json!({"iterationIndex":trace.iteration_index,"sourceMap":source_map,"rules":rules,"decisions":trace.decisions,"truncated":trace.truncated,"capture":{"seconds":args.seconds,"maxDecisions":args.decisions},"result":output::result(&trace.result,args.execution.inspection.detail),"workUnits":trace.work_units}),
            ))
        }
        Command::Batch(args) => batch(args, None, quiet, started).await,
        Command::Compare(args) => {
            batch(
                args.batch,
                Some((args.baseline, args.metric)),
                quiet,
                started,
            )
            .await
        }
    }
}
fn inspect(args: Inspection, require_apl: bool) -> Result<Outcome, Failure> {
    let loaded = load(&args.input)?;
    if require_apl && loaded.apl.is_none() {
        return Err(Failure::input(
            "missing-apl",
            "Supply --apl or --default-apl to explain rules.",
            "/apl",
        ));
    }
    // Validate options even for character-only preparation.
    let mut issues = diagnose_character(&loaded.character);
    for d in &mut issues {
        d.path = d.path.take().map(|p| format!("/character/build{p}"));
    }
    issues.extend(diagnose_options(&loaded.options));
    if !issues.is_empty() {
        return Err(loaded.locate(SimulationError::from_diagnostics(
            SimulationErrorCode::InvalidBuild,
            issues,
        )));
    }
    let (profile, evidence) = prepare_character(&loaded.character, loaded.options.targets)
        .map_err(|e| loaded.locate(e))?;
    let mut stats = serde_json::to_value(&profile).unwrap();
    for key in [
        "abilities",
        "talents",
        "mechanics",
        "aplTargetEffects",
        "scenarioNoOpAbilities",
        "uptimeNames",
        "evidenceClaimIds",
    ] {
        stats.as_object_mut().unwrap().remove(key);
    }
    let mut value = json!({"heroId":loaded.character.hero_id,"stats":stats,"talents":profile.talents,"effects":profile.mechanics,"availableAbilities":profile.abilities.iter().map(|a| json!({"id":a.kind,"gameAbilityId":a.id,"name":a.name,"manuallyCastable":a.manually_castable})).collect::<Vec<_>>(),"targetEffects":profile.apl_target_effects,"modelingLimitations":output::limitations(&evidence),"evidenceFingerprint":evidence.evidence_fingerprint});
    if let Some(source) = &loaded.apl {
        let request = loaded.prepare()?;
        value["rules"] = json!(explain_apl(&request).map_err(Failure::from)?);
        value["sourceMap"] = json!(
            parse_apl_document(source)
                .map_err(Failure::from)?
                .source_map
        );
        value["seed"] = json!(request.seed);
        value["profileFingerprint"] = json!(request.profile_fingerprint);
        value["scenario"] = json!(request.scenario);
    }
    if args.detail == Detail::Full {
        value["profile"] = json!(profile);
        value["evidence"] = json!(evidence);
    }
    Ok(Outcome::success(value))
}

fn stopped(error: SimulationError, reason: u8) -> Failure {
    if reason == 124 {
        Failure::execution(
            "timeout",
            "The command exceeded its wall-clock deadline.",
            124,
        )
    } else if reason == 130 {
        Failure::execution("cancelled", "The command was cancelled.", 130)
    } else {
        Failure::from(error)
    }
}
async fn controlled<T: Send + 'static>(
    timeout: Option<u64>,
    started: Instant,
    work: impl FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
) -> Result<(T, u8), Failure> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let remaining =
        timeout.map(|seconds| Duration::from_secs(seconds).saturating_sub(started.elapsed()));
    let expired = remaining.is_some_and(|r| r.is_zero());
    if expired {
        cancelled.store(true, Ordering::Release);
    }
    let flag = cancelled.clone();
    let mut worker = tokio::task::spawn_blocking(move || work(flag));
    let deadline = async {
        if let Some(remaining) = remaining {
            tokio::time::sleep(remaining).await
        } else {
            std::future::pending::<()>().await
        }
    };
    if expired {
        return Ok((
            worker
                .await
                .map_err(|e| Failure::execution("worker-failed", e.to_string(), 1))?,
            124,
        ));
    }
    tokio::pin!(deadline);
    let (result, reason) = tokio::select! {
        result = &mut worker => (result,0),
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|e| Failure::execution("signal-handler",e.to_string(),1))?;
            cancelled.store(true,Ordering::Release);
            (worker.await,130)
        }
        _ = &mut deadline => {
            cancelled.store(true,Ordering::Release);
            (worker.await,124)
        }
    };
    Ok((
        result.map_err(|e| Failure::execution("worker-failed", e.to_string(), 1))?,
        reason,
    ))
}
struct Job {
    id: String,
    request: Option<SimulationRequest>,
    failure: Option<Failure>,
}
struct Finished {
    result: Result<(SimulationResult, Vec<f64>), SimulationError>,
}
async fn batch(
    args: Batch,
    comparison: Option<(String, Metric)>,
    quiet: bool,
    started: Instant,
) -> Result<Outcome, Failure> {
    let manifest: Manifest = decode(&read(&args.input, MANIFEST_LIMIT)?, &args.input)?;
    if manifest.version != 1 {
        return Err(Failure::input(
            "unsupported-batch-version",
            "Expected batch version 1.",
            "/version",
        )
        .source(&args.input));
    }
    if manifest.cases.is_empty() || manifest.cases.len() > 64 {
        return Err(
            Failure::input("batch-size", "Supply 1–64 cases.", "/cases").source(&args.input)
        );
    }
    let directory = base(&args.input);
    let mut jobs = Vec::new();
    let mut ids = BTreeSet::new();
    let mut duplicate_ids = BTreeSet::new();
    for v in &manifest.cases {
        if let Some(id) = v["id"].as_str()
            && !ids.insert(id.to_owned())
        {
            duplicate_ids.insert(id.to_owned());
        }
    }
    for (index, value) in manifest.cases.iter().enumerate() {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let request = (|| {
            let case: Case = decode(&value.to_string(), &args.input)?;
            if case.id.is_empty() || case.id.len() > 128 || duplicate_ids.contains(&case.id) {
                return Err(Failure::input(
                    "invalid-case-id",
                    "Case IDs must be unique and contain 1–128 bytes.",
                    "/id",
                ));
            }
            if comparison.is_some() && case.options.seed.is_some() {
                return Err(Failure::input(
                    "case-seed",
                    "Comparisons use one seed; place it in shared options.seed.",
                    "/options/seed",
                ));
            }
            let mut options = manifest.options.clone();
            case.options.apply(&mut options);
            let loaded = resolve(
                &case.character,
                Some(&case.apl),
                options,
                &directory,
                &args.input,
            )?;
            loaded.prepare()
        })();
        let mut job = Job {
            id,
            request: None,
            failure: None,
        };
        match request {
            Ok(r) => job.request = Some(r),
            Err(mut f) => {
                for d in &mut f.diagnostics {
                    // Preserve source-relative locations for external files.
                    if d.source.is_none()
                        || d.source.as_deref() == Some(&args.input.display().to_string())
                    {
                        d.source = Some(args.input.display().to_string());
                        let path = d.path.as_deref().unwrap_or("");
                        let shared_option = path
                            .strip_prefix("/options/")
                            .is_some_and(|field| value["options"].get(field).is_none());
                        d.path = Some(if shared_option {
                            path.to_owned()
                        } else {
                            format!("/cases/{index}{path}")
                        });
                    }
                }
                job.failure = Some(f);
            }
        }
        jobs.push(job);
    }
    let baseline_index = if let Some((baseline, _)) = &comparison {
        let index = jobs.iter().position(|j| j.id == *baseline).ok_or_else(|| {
            Failure::input(
                "missing-baseline",
                "Baseline must name a unique case.",
                "/baseline",
            )
        })?;
        let Some(base) = jobs[index].request.clone() else {
            return Err(jobs[index].failure.take().unwrap());
        };
        for job in &mut jobs {
            if let Some(r) = &mut job.request {
                if r.hero_id != base.hero_id
                    || r.data_build_id != base.data_build_id
                    || r.model_version != base.model_version
                    || r.scenario_id != base.scenario_id
                    || r.scenario != base.scenario
                    || r.iterations != base.iterations
                {
                    job.request = None;
                    job.failure = Some(Failure::input(
                        "incompatible-comparison",
                        "Cases must share hero, game/model version, scenario, target count, and iterations.",
                        "/options",
                    ));
                } else {
                    r.seed = base.seed.clone();
                }
            }
        }
        Some(index)
    } else {
        None
    };
    let metric = comparison.as_ref().map(|(_, metric)| match metric {
        Metric::TotalDps => ComparisonMetric::TotalDps,
        Metric::PrimaryTargetDps => ComparisonMetric::PrimaryTargetDps,
    });
    let requests = jobs.iter().map(|j| j.request.clone()).collect::<Vec<_>>();
    let labels = jobs.iter().map(|j| j.id.clone()).collect::<Vec<_>>();
    let concurrency = args.jobs as usize;
    let (finished, reason) = controlled(args.timeout, started, move |cancel| {
        let next = AtomicUsize::new(0);
        let results = Mutex::new(
            (0..requests.len())
                .map(|_| None)
                .collect::<Vec<Option<Finished>>>(),
        );
        std::thread::scope(|scope| {
            for _ in 0..concurrency {
                let cancel = &cancel;
                let requests = &requests;
                let next = &next;
                let results = &results;
                let labels = &labels;
                scope.spawn(move || {
                    loop {
                        if cancel.load(Ordering::Acquire) {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= requests.len() {
                            break;
                        }
                        let Some(request) = &requests[index] else {
                            continue;
                        };
                        if cancel.load(Ordering::Acquire) {
                            break;
                        }
                        if !quiet {
                            output::progress(format_args!("Starting case {}", labels[index]));
                        }
                        let result = simulate_with_samples(
                            request,
                            cancel,
                            |_| {},
                            MAX_SIMULATION_WORKERS / concurrency,
                            metric,
                        );
                        if !quiet {
                            output::progress(format_args!("Finished case {}", labels[index]));
                        }
                        results.lock().unwrap()[index] = Some(Finished { result });
                    }
                });
            }
        });
        results.into_inner().unwrap()
    })
    .await?;
    let comparisons_count = jobs.len().saturating_sub(1);
    let baseline_samples = baseline_index
        .and_then(|i| finished[i].as_ref())
        .and_then(|f| f.result.as_ref().ok())
        .map(|(_, s)| s.clone());
    let mut cases = Vec::new();
    let mut any_failed = false;
    for (index, (job, finished)) in jobs.into_iter().zip(finished).enumerate() {
        let mut value = json!({"id":job.id,"status":CaseStatus::NotStarted,"result":null,"diagnostics":[],"diagnosticsTruncated":false});
        if let Some(failure) = job.failure {
            any_failed = true;
            value["status"] = json!(CaseStatus::Invalid);
            value["diagnostics"] = json!(failure.diagnostics);
            value["diagnosticsTruncated"] = json!(failure.truncated);
        } else if let Some(finished) = finished {
            match finished.result {
                Ok((result, samples)) => {
                    value["status"] = json!(CaseStatus::Completed);
                    value["result"] = output::result(&result, args.detail);
                    if let Some(base) = &baseline_samples
                        && Some(index) != baseline_index
                    {
                        value["comparison"] = json!(
                            compare_paired(base, &samples, comparisons_count)
                                .map_err(Failure::from)?
                        );
                    }
                }
                Err(error) => {
                    any_failed = true;
                    let failure = stopped(error, reason);
                    value["status"] = json!(match failure.exit {
                        124 => CaseStatus::TimedOut,
                        130 => CaseStatus::Cancelled,
                        _ => CaseStatus::Failed,
                    });
                    value["diagnostics"] = json!(failure.diagnostics);
                }
            }
        } else {
            any_failed = true;
        }
        cases.push(value);
    }
    let mut data = json!({"cases":cases});
    if let Some((baseline, _)) = comparison {
        data["baseline"] = json!(baseline);
        data["metric"] = json!(metric);
        data["familyConfidence"] = json!(0.95);
        data["requestedComparisons"] = json!(comparisons_count);
        data["uncertainty"] = json!(
            "Paired Monte Carlo uncertainty assumes independent iterations and an adequate t approximation. It does not measure game-model error."
        );
    }
    let failure = if reason != 0 {
        Some(Failure::execution(
            if reason == 124 {
                "timeout"
            } else {
                "cancelled"
            },
            "Completed results are preserved; queued cases were not started.",
            reason,
        ))
    } else if any_failed {
        Some(Failure::execution(
            "partial-batch-failure",
            "One or more cases failed; inspect each case's diagnostics.",
            1,
        ))
    } else {
        None
    };
    Ok(Outcome {
        data: Some(data),
        failure,
        artifact: None,
    })
}
