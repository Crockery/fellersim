use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering as AtomicOrdering},
    },
    thread,
};

use crate::{rng::split_seed, *};

const ITERATION_WINDOW_SIZE: u32 = 256;

fn simulation_worker_count(available_parallelism: usize, iterations: u32) -> usize {
    available_parallelism
        .clamp(1, MAX_SIMULATION_WORKERS)
        .min(iterations as usize)
}

pub fn simulate<F>(
    request: &SimulationRequest,
    cancelled: &AtomicBool,
    progress: F,
) -> Result<SimulationResult, SimulationError>
where
    F: Fn(SimulationProgress) + Sync,
{
    simulate_inner(
        request,
        cancelled,
        progress,
        cfg!(feature = "strict-work-budget"),
    )
}

/// Runs a simulation while enforcing the browser-profile work-budget margin.
///
/// This is used by the Fellership mechanic sweep without enabling the
/// diagnostic-only `strict-work-budget` feature in production binaries.
#[doc(hidden)]
pub fn simulate_profile_sweep<F>(
    request: &SimulationRequest,
    cancelled: &AtomicBool,
    progress: F,
) -> Result<SimulationResult, SimulationError>
where
    F: Fn(SimulationProgress) + Sync,
{
    simulate_inner(request, cancelled, progress, true)
}

fn simulate_inner<F>(
    request: &SimulationRequest,
    cancelled: &AtomicBool,
    progress: F,
    enforce_profile_budget: bool,
) -> Result<SimulationResult, SimulationError>
where
    F: Fn(SimulationProgress) + Sync,
{
    let profile = CompiledProfile::try_from(request)?;
    let seed = profile.validated_seed;
    progress(SimulationProgress {
        run_id: request.run_id.clone(),
        completed_iterations: 0,
        total_iterations: request.iterations,
        fraction: 0.0,
    });
    let workers = simulation_worker_count(
        thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        request.iterations,
    );
    let progress_stride = (request.iterations / 100).max(1);
    let completed = AtomicU32::new(0);
    let mut accumulator = SimulationAccumulator::new(request, &profile, enforce_profile_budget);
    let mut window_start = 0_u32;
    while window_start < request.iterations {
        if cancelled.load(AtomicOrdering::Relaxed) {
            return Err(cancelled_error());
        }
        let window_end = window_start
            .saturating_add(ITERATION_WINDOW_SIZE)
            .min(request.iterations);
        let next = AtomicU32::new(window_start);
        let failed = AtomicBool::new(false);
        let failure = std::sync::Mutex::new(None::<SimulationError>);
        let results = std::sync::Mutex::new(Vec::<(u32, IterationResult)>::with_capacity(
            (window_end - window_start) as usize,
        ));
        thread::scope(|scope| {
            for _ in 0..workers.min((window_end - window_start) as usize) {
                scope.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        if cancelled.load(AtomicOrdering::Relaxed)
                            || failed.load(AtomicOrdering::Relaxed)
                        {
                            break;
                        }
                        let index = next.fetch_add(1, AtomicOrdering::Relaxed);
                        if index >= window_end {
                            break;
                        }
                        let iteration_seed = split_seed(seed, index);
                        let result = Iteration::from_compiled(
                            &profile,
                            &request.action_priority_list,
                            request.scenario.target_count,
                            iteration_seed,
                            cancelled,
                        )
                        .run();
                        match result {
                            Ok(result) => local.push((index, result)),
                            Err(error) => {
                                failed.store(true, AtomicOrdering::Relaxed);
                                let mut failure =
                                    failure.lock().expect("simulation failure lock poisoned");
                                if failure.is_none() {
                                    *failure = Some(error);
                                }
                                break;
                            }
                        }
                        let done = completed.fetch_add(1, AtomicOrdering::Relaxed) + 1;
                        if done.is_multiple_of(progress_stride) || done == request.iterations {
                            progress(SimulationProgress {
                                run_id: request.run_id.clone(),
                                completed_iterations: done,
                                total_iterations: request.iterations,
                                fraction: done as f64 / request.iterations as f64,
                            });
                        }
                    }
                    results
                        .lock()
                        .expect("simulation result lock poisoned")
                        .extend(local);
                });
            }
        });
        if cancelled.load(AtomicOrdering::Relaxed) {
            return Err(cancelled_error());
        }
        if let Some(error) = failure
            .into_inner()
            .expect("simulation failure lock poisoned")
        {
            return Err(error);
        }
        let mut results = results
            .into_inner()
            .expect("simulation result lock poisoned");
        results.sort_by_key(|(index, _)| *index);
        if results.len() != (window_end - window_start) as usize {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation ended before the current iteration window completed",
            ));
        }
        for (_, result) in results {
            accumulator.push(result)?;
        }
        window_start = window_end;
    }
    accumulator.finish(request, &profile)
}

fn cancelled_error() -> SimulationError {
    SimulationError::new(SimulationErrorCode::Cancelled, "Simulation cancelled")
}

pub fn simulate_owned<F>(
    request: SimulationRequest,
    cancelled: Arc<AtomicBool>,
    progress: F,
) -> Result<SimulationResult, SimulationError>
where
    F: Fn(SimulationProgress) + Sync,
{
    simulate(&request, &cancelled, progress)
}

#[cfg(test)]
pub(crate) fn aggregate(
    request: &SimulationRequest,
    profile: &CompiledProfile,
    iterations: impl Iterator<Item = IterationResult>,
) -> Result<SimulationResult, SimulationError> {
    let mut accumulator =
        SimulationAccumulator::new(request, profile, cfg!(feature = "strict-work-budget"));
    for iteration in iterations {
        accumulator.push(iteration)?;
    }
    accumulator.finish(request, profile)
}

#[cfg(test)]
pub(crate) fn aggregate_profile_sweep(
    request: &SimulationRequest,
    profile: &CompiledProfile,
    iterations: impl Iterator<Item = IterationResult>,
) -> Result<SimulationResult, SimulationError> {
    let mut accumulator = SimulationAccumulator::new(request, profile, true);
    for iteration in iterations {
        accumulator.push(iteration)?;
    }
    accumulator.finish(request, profile)
}

struct SimulationAccumulator {
    dps: Vec<f64>,
    abilities: Vec<AbilityTotals>,
    mechanic_buff_uptimes: Vec<f64>,
    proc_counts: Vec<u64>,
    uptimes: BTreeMap<String, f64>,
    target_damage: Vec<f64>,
    max_iteration_work_units: u32,
    enforce_profile_budget: bool,
    #[cfg(feature = "strict-work-budget")]
    max_iteration_work_breakdown: Vec<(&'static str, u32, u32)>,
}

impl SimulationAccumulator {
    fn new(
        request: &SimulationRequest,
        profile: &CompiledProfile,
        enforce_profile_budget: bool,
    ) -> Self {
        Self {
            dps: Vec::with_capacity(request.iterations as usize),
            abilities: vec![AbilityTotals::default(); profile.damage_sources.len()],
            mechanic_buff_uptimes: vec![0.0; profile.mechanics.len()],
            proc_counts: vec![0; profile.proc_sources.len()],
            uptimes: BTreeMap::new(),
            target_damage: vec![0.0; request.scenario.target_count as usize],
            max_iteration_work_units: 0,
            enforce_profile_budget,
            #[cfg(feature = "strict-work-budget")]
            max_iteration_work_breakdown: Vec::new(),
        }
    }

    fn push(&mut self, iteration: IterationResult) -> Result<(), SimulationError> {
        if iteration.work_units > self.max_iteration_work_units {
            self.max_iteration_work_units = iteration.work_units;
            #[cfg(feature = "strict-work-budget")]
            {
                self.max_iteration_work_breakdown = iteration.work_breakdown.clone();
            }
        }
        if iteration.targets.len() != self.target_damage.len() {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation produced an invalid target result",
            ));
        }
        self.dps
            .push(iteration.damage / (ENCOUNTER_DURATION_MS as f64 / 1_000.0));
        for (total, current) in self.target_damage.iter_mut().zip(iteration.targets) {
            *total += current;
        }
        if iteration.abilities.len() != self.abilities.len() {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation produced an invalid ability result",
            ));
        }
        for (total, current) in self.abilities.iter_mut().zip(iteration.abilities) {
            total.damage += current.damage;
            total.hits += current.hits;
            total.crits += current.crits;
            total.grievous += current.grievous;
            total.casts += current.casts;
            total.targets_hit_total += u64::from(current.targets_hit_mask.count_ones());
        }
        if iteration.mechanic_proc_counts.len() != self.mechanic_buff_uptimes.len()
            || iteration.mechanic_buff_uptimes.len() != self.mechanic_buff_uptimes.len()
        {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation produced an invalid mechanic result",
            ));
        }
        for (total, current) in self
            .mechanic_buff_uptimes
            .iter_mut()
            .zip(iteration.mechanic_buff_uptimes)
        {
            *total += current;
        }
        if iteration.proc_counts.len() != self.proc_counts.len() {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation produced an invalid proc result",
            ));
        }
        for (total, current) in self.proc_counts.iter_mut().zip(iteration.proc_counts) {
            *total = total.saturating_add(current);
        }
        for (id, uptime) in iteration.uptimes {
            *self.uptimes.entry(id).or_default() += uptime;
        }
        Ok(())
    }

    fn finish(
        mut self,
        request: &SimulationRequest,
        profile: &CompiledProfile,
    ) -> Result<SimulationResult, SimulationError> {
        if self.dps.len() != request.iterations as usize {
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                "simulation ended before all iterations completed",
            ));
        }
        debug_assert!(self.max_iteration_work_units <= MAX_ITERATION_WORK_UNITS);
        if self.enforce_profile_budget
            && self.max_iteration_work_units >= MAX_ITERATION_WORK_UNITS / 4
        {
            let message = {
                let base = format!(
                    "simulation used {} work units; browser-generated profiles must remain below {}",
                    self.max_iteration_work_units,
                    MAX_ITERATION_WORK_UNITS / 4,
                );
                #[cfg(feature = "strict-work-budget")]
                {
                    format!(
                        "{base}; largest charges: {:?}",
                        self.max_iteration_work_breakdown
                            .iter()
                            .take(12)
                            .collect::<Vec<_>>()
                    )
                }
                #[cfg(not(feature = "strict-work-budget"))]
                {
                    base
                }
            };
            return Err(SimulationError::new(
                SimulationErrorCode::SimulationFailed,
                message,
            ));
        }
        let count = self.dps.len() as f64;
        let mean = self.dps.iter().sum::<f64>() / count;
        let variance = if self.dps.len() > 1 {
            self.dps
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / (count - 1.0)
        } else {
            0.0
        };
        let standard_deviation = variance.sqrt();
        let margin = 1.96 * standard_deviation / count.sqrt();
        for (index, total) in self.mechanic_buff_uptimes.into_iter().enumerate() {
            if total > 0.0 {
                self.uptimes.insert(
                    format!("buff:{}", profile.mechanics[index].instance_id),
                    total,
                );
            }
        }
        let mut ability_results = self
            .abilities
            .into_iter()
            .enumerate()
            .filter(|(source_index, totals)| {
                *source_index < profile.abilities.len() || totals.hits > 0 || totals.casts > 0
            })
            .map(|(source_index, totals)| {
                let source = &profile.damage_sources[source_index];
                let mean_damage = totals.damage / count;
                let mean_dps = mean_damage / (ENCOUNTER_DURATION_MS as f64 / 1_000.0);
                AbilityDamageResult {
                    ability_id: source.id.clone(),
                    ability_name: source.name.clone(),
                    mean_damage,
                    mean_dps,
                    share: if mean > 0.0 { mean_dps / mean } else { 0.0 },
                    mean_hits: totals.hits as f64 / count,
                    mean_crits: totals.crits as f64 / count,
                    mean_grievous_crits: totals.grievous as f64 / count,
                    mean_casts: totals.casts as f64 / count,
                    mean_targets_hit: totals.targets_hit_total as f64 / count,
                }
            })
            .collect::<Vec<_>>();
        ability_results.sort_by(|left, right| {
            right
                .mean_damage
                .total_cmp(&left.mean_damage)
                .then(left.ability_id.cmp(&right.ability_id))
        });
        let mut proc_results = self
            .proc_counts
            .into_iter()
            .zip(&profile.proc_sources)
            .filter(|(total, _)| *total > 0)
            .map(|(total, source)| {
                let mean_count = total as f64 / count;
                let mean_per_minute = mean_count / (ENCOUNTER_DURATION_MS as f64 / 60_000.0);
                if !mean_count.is_finite()
                    || !mean_per_minute.is_finite()
                    || mean_count < 0.0
                    || mean_per_minute < 0.0
                {
                    return Err(SimulationError {
                        code: SimulationErrorCode::SimulationFailed,
                        message: "simulation produced an invalid proc result".into(),
                        sources: vec![source.id.clone()],
                    });
                }
                Ok(ProcResult {
                    id: source.id.clone(),
                    source_id: source.source_id.clone(),
                    name: source.name.clone(),
                    mean_count,
                    mean_per_minute,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        proc_results.sort_by(|left, right| {
            right
                .mean_per_minute
                .total_cmp(&left.mean_per_minute)
                .then(left.name.cmp(&right.name))
                .then(left.id.cmp(&right.id))
        });
        let mut uptime_results = self
            .uptimes
            .into_iter()
            .filter(|(id, total)| *total > 0.0 && !id.starts_with("proc:"))
            .map(|(id, total)| {
                if !id.starts_with("dot:") && !id.starts_with("buff:") {
                    return Err(SimulationError {
                        code: SimulationErrorCode::SimulationFailed,
                        message: "simulation produced a non-duration uptime result".into(),
                        sources: vec![id],
                    });
                }
                let mean_uptime = total / count;
                if !mean_uptime.is_finite() || !(0.0..=1.0).contains(&mean_uptime) {
                    return Err(SimulationError {
                        code: SimulationErrorCode::SimulationFailed,
                        message: "simulation produced an invalid uptime result".into(),
                        sources: vec![id],
                    });
                }
                let Some(name) = profile.uptime_names.get(&id) else {
                    return Err(SimulationError {
                        code: SimulationErrorCode::SimulationFailed,
                        message: "simulation produced an unnamed uptime result".into(),
                        sources: vec![id],
                    });
                };
                Ok(UptimeResult {
                    id,
                    name: name.clone(),
                    mean_uptime,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        uptime_results.sort_by(|left, right| {
            right
                .mean_uptime
                .total_cmp(&left.mean_uptime)
                .then(left.name.cmp(&right.name))
                .then(left.id.cmp(&right.id))
        });
        let target_results = self
            .target_damage
            .into_iter()
            .enumerate()
            .map(|(index, damage)| {
                let mean_damage = damage / count;
                let mean_dps = mean_damage / (ENCOUNTER_DURATION_MS as f64 / 1_000.0);
                TargetDamageResult {
                    target_index: index as u32,
                    primary: index == 0,
                    mean_damage,
                    mean_dps,
                    share: if mean > 0.0 { mean_dps / mean } else { 0.0 },
                }
            })
            .collect::<Vec<_>>();
        let primary_target_dps = target_results
            .first()
            .map(|target| target.mean_dps)
            .unwrap_or(0.0);
        Ok(SimulationResult {
            schema_version: SIMULATOR_SCHEMA_VERSION,
            run_id: request.run_id.clone(),
            data_build_id: request.data_build_id.clone(),
            model_version: request.model_version.clone(),
            profile_fingerprint: request.profile_fingerprint.clone(),
            scenario_id: request.scenario_id.clone(),
            scenario: request.scenario.clone(),
            evidence: request.evidence.clone(),
            iterations: request.iterations,
            seed: request.seed.clone(),
            mean_dps: mean,
            primary_target_dps,
            standard_deviation,
            confidence_interval_95: ConfidenceInterval {
                low: mean - margin,
                high: mean + margin,
            },
            abilities: ability_results,
            procs: proc_results,
            targets: target_results,
            uptimes: uptime_results,
        })
    }
}
