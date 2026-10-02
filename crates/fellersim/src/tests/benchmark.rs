use super::*;

#[test]
fn representative_hero_profiles_use_less_than_one_quarter_of_the_iteration_budget() {
    let ardeos = ardeos_fixture_request();
    let rime = rime_fixture_request();
    let cases = [
        ("ardeos", ardeos.profile, ardeos.action_priority_list),
        ("rime", rime.profile, rime.action_priority_list),
        ("tariq", tariq_profile(), apl([("wild-swing", None)])),
        (
            "elarion",
            elarion_profile(std::iter::empty::<DpsAbilityModel>()),
            apl([("elarion-shoot", None)]),
        ),
        ("mara", mara_profile(), apl([("mara-attack", None)])),
        ("gunde", gunde_profile(), apl([("double-strike", None)])),
    ];

    let mut observations = Vec::new();
    for (hero, profile, apl) in cases {
        // Keep this aligned with Fellership's browser-generated mechanic sweep.
        for target_count in [1, 5] {
            let result = Iteration::new(&profile, &apl, target_count, 1)
                .run()
                .unwrap_or_else(|error| panic!("{hero} failed its work-budget sweep: {error}"));
            observations.push((hero, target_count, result.work_units));
        }
    }
    assert!(
        observations
            .iter()
            .all(|(_, _, work)| *work < MAX_ITERATION_WORK_UNITS / 4),
        "hero work-budget sweep exceeded one quarter of the ceiling: {observations:?}",
    );
}

#[test]
#[ignore = "run explicitly in release mode to verify the maximum-iteration memory ceiling"]
fn simulator_max_iterations_stay_below_one_gibibyte_peak_live_memory() {
    let mut request = ardeos_fixture_request();
    request.iterations = MAX_SIMULATION_ITERATIONS;
    allocation_counter::reset();

    let result = simulate(&request, &AtomicBool::new(false), |_| {})
        .expect("maximum-iteration request simulates");
    let allocations = allocation_counter::metrics();
    std::hint::black_box(result);

    assert!(
        allocations.peak_live_bytes < 1024 * 1024 * 1024,
        "maximum-iteration simulation reached {} peak live bytes",
        allocations.peak_live_bytes,
    );
}

#[test]
#[ignore = "run explicitly in release mode to enforce the default-workload baseline"]
fn simulator_default_iteration_benchmark() {
    use std::{hint::black_box, time::Instant};

    #[derive(Debug, Clone, Copy)]
    struct Baseline {
        elapsed_ms: u128,
        allocations: u64,
        allocated_bytes: u64,
        peak_live_bytes: usize,
    }

    const ARDEOS_BASELINE: Baseline = Baseline {
        elapsed_ms: 2_262,
        allocations: 32_125_492,
        allocated_bytes: 2_050_495_576,
        peak_live_bytes: 19_337_740,
    };
    const RIME_BASELINE: Baseline = Baseline {
        elapsed_ms: 1_222,
        allocations: 15_686_617,
        allocated_bytes: 1_557_387_452,
        peak_live_bytes: 20_621_064,
    };
    const WALL_TIME_TOLERANCE_PERCENT: u128 = 25;
    const PEAK_MEMORY_TOLERANCE_PERCENT: usize = 5;

    for (hero, mut request, baseline) in [
        ("ardeos", ardeos_fixture_request(), ARDEOS_BASELINE),
        ("rime", rime_fixture_request(), RIME_BASELINE),
    ] {
        request.iterations = DEFAULT_SIMULATION_ITERATIONS;
        allocation_counter::reset();
        let started = Instant::now();
        let result = simulate(&request, &AtomicBool::new(false), |_| {})
            .expect("benchmark request simulates");
        let elapsed = started.elapsed();
        let allocations = allocation_counter::metrics();
        black_box(result);
        eprintln!(
            "SIMULATOR_BENCHMARK hero={hero} iterations={} elapsed_ms={} allocations={} allocated_bytes={} peak_live_bytes={}",
            request.iterations,
            elapsed.as_millis(),
            allocations.allocations,
            allocations.allocated_bytes,
            allocations.peak_live_bytes,
        );
        let maximum_elapsed_ms = baseline
            .elapsed_ms
            .saturating_mul(100 + WALL_TIME_TOLERANCE_PERCENT)
            / 100;
        let maximum_peak_live_bytes = baseline
            .peak_live_bytes
            .saturating_mul(100 + PEAK_MEMORY_TOLERANCE_PERCENT)
            / 100;
        assert!(
            elapsed.as_millis() <= maximum_elapsed_ms,
            "{hero} default simulation took {} ms; baseline is {} ms with a {WALL_TIME_TOLERANCE_PERCENT}% noise allowance",
            elapsed.as_millis(),
            baseline.elapsed_ms,
        );
        assert!(
            allocations.allocations <= baseline.allocations,
            "{hero} default simulation performed {} allocations; baseline is {}",
            allocations.allocations,
            baseline.allocations,
        );
        assert!(
            allocations.allocated_bytes <= baseline.allocated_bytes,
            "{hero} default simulation allocated {} bytes; baseline is {}",
            allocations.allocated_bytes,
            baseline.allocated_bytes,
        );
        assert!(
            allocations.peak_live_bytes <= maximum_peak_live_bytes,
            "{hero} default simulation reached {} peak live bytes; baseline is {} with a {PEAK_MEMORY_TOLERANCE_PERCENT}% scheduling allowance",
            allocations.peak_live_bytes,
            baseline.peak_live_bytes,
        );
    }
}
