# Synthetic builds

Synthetic builds let you select stats and effects without equipping items. They
use the same preparation rules, rotation interpreter, and combat engine as
equipment builds. Results carry `exploratory: true`: equipment feasibility has
not been evaluated.

## Run a build

The release includes `examples/synthetic-gunde.json` (under
`apps/fellersim/examples/` in a source checkout):

```sh
fellersim prepare --synthetic examples/synthetic-gunde.json --default-apl --json
fellersim run --synthetic examples/synthetic-gunde.json --default-apl --duration-seconds 660 --iterations 100 --seed 0123456789abcdef --json
```

Use `schema synthetic` for the document schema. `catalog legendaries --hero gunde`,
`catalog weapons --hero gunde`, `catalog traits`, `catalog blessings`,
`catalog gem-tracks`, and `catalog sets` expose the bundled identifiers, effects,
ranks, and thresholds. Legendary IDs are interpreted for the selected hero.

Synthetic files have `format: "fellersim-synthetic-build"` and a `build` object
with `schemaVersion: 1`. Hero IDs are `firemage`, `rime`, `ink`, `bowguy`, `mara`,
and `gunde`. Talent IDs and the talent point budget follow the equipment rules.

Save this complete configuration as `synthetic.json` and pass it to `--synthetic`:

```json
{
  "format": "fellersim-synthetic-build",
  "build": {
    "schemaVersion": 1,
    "heroId": "gunde",
    "talentPoints": 14,
    "selectedTalentIds": [],
    "secondaryRatings": {
      "crit": 100,
      "haste": 250,
      "expertise": 75,
      "spirit": 125
    },
    "primaryStatAdd": 1500,
    "staminaAdd": 1000,
    "legendary": { "id": "legendary-back-a-haste" },
    "traits": { "ItemTrait.ID.AbilityToIncreasedMainStat": 2 },
    "gemPower": { "Ruby": 1000, "Amethyst": 500 },
    "setBonuses": { "setb-percentage-hdt": { "pieceCount": 2 } }
  }
}
```

For a `+100 Haste` experiment, copy this file and change only `haste` from 250
to 350. Use the same APL, duration, iterations, and explicit seed for both runs.
For a gem breakpoint experiment, compare Ruby power 79, 80, and 81 while holding
everything else fixed; the first unlock is at 80 before any gem-power set
multiplier. `catalog gem-tracks` lists the remaining breakpoints.

## Stats and effects

`secondaryRatings` is a complete baseline profile with `crit`, `haste`,
`expertise`, and `spirit`, all in **raw rating units**. It replaces the ratings
normally contributed by equipment. These numbers exclude passive effect bonuses
and are not percentages or final combat attributes. Zero is a valid baseline.

Hero starting stats are retained. `primaryStatAdd` adds to the hero's starting
primary attribute; `staminaAdd` adds to starting Stamina. Both default to zero.
Selected legendary, trait, blessing, gem, and set effects then apply once using
the equipment resolver. Rating conversion retains the game's rounding and
diminishing returns. Talents and temporary combat buffs work as before.

Changing baseline Haste leaves the other baseline ratings and effect selections
unchanged. Effects that select your highest stat or scale with Haste can still
respond to that change; this is part of their normal mechanics.

- `legendary` selects one complete legendary effect bundle, including its
  inherent Power and cooldown bonuses. Its ordinary fixed or rolled item stats
  are excluded. `weapon` selects the weapon ability and associated effects with
  no item-stat contribution. Each uses an object such as `{ "id": "..." }`.
- `traits` and `blessings` map IDs to aggregate ranks, from 1 through the catalog
  maximum. Remove a key to deactivate it. These are effect ranks, not initial
  runtime proc stacks; stacks accumulate through the existing combat rules.
- `gemPower` maps catalog families, such as `Ruby` or `Amethyst`, to power before
  set modifiers. The resolver applies the gem-power set multiplier once, then
  selects the highest unlocked effect in each family/slot. It does not require
  sockets. Power above the last threshold unlocks no additional effects.
- `setBonuses` maps individual bonus IDs to `{ "pieceCount": 2 }`. Each bonus
  activates at its own catalog `requiredSetCount`; zero or a sub-threshold count
  is inactive. There is no equipment budget. Hero-specific variants must match
  the hero. The universal `quickplay` bonus is also available.
- `relics` preserves reviewed relic effects. They currently have no DPS effect
  in this stationary encounter. Defensive effects and Patient Soul remain
  selectable where mechanically supported; there are no optimizer exclusions.

Numeric inputs must be finite and nonnegative, at most 1,000,000. Resolved values
also undergo the engine's existing validation. Unknown IDs, unsupported ranks,
incompatible selections, and uncovered mechanics are errors.

## Rust evaluation

The public crate is `fellersim-core`. Reuse an `EvaluationContext` to parse a
rotation once and evaluate modified builds:

```rust
use fellersim_core::preparation::{
    EvaluationContext, SecondaryRatings, SimulationOptions, SyntheticBuild,
    SyntheticEffectSelection,
};
use std::sync::atomic::AtomicBool;

let mut build = SyntheticBuild::new("gunde")?;
build.secondary_ratings = SecondaryRatings {
    crit: 100.0, haste: 250.0, expertise: 75.0, spirit: 125.0,
};
build.legendary = Some(SyntheticEffectSelection {
    id: "legendary-back-a-haste".into(), source_position_id: None,
});
build.traits.insert("ItemTrait.ID.AbilityToIncreasedMainStat".into(), 2);
build.gem_power.insert("Ruby".into(), 1000.0);

let extra_haste = build.with_rating_delta(SecondaryRatings {
    haste: 100.0, ..Default::default()
})?;
let options = SimulationOptions {
    duration_seconds: 660,
    iterations: 100,
    seed: Some("0123456789abcdef".into()),
    ..Default::default()
};
let context = EvaluationContext::new("actions=/double_strike")?;
let cancelled = AtomicBool::new(false);
let result = context.evaluate(&build, &options, "baseline", &cancelled)?;
let key = context.cache_key(&build, &options)?;
for result in context.evaluate_batch([&build, &extra_haste], &options, &cancelled) {
    let result = result?;
    println!("{}: {} DPS", result.run_id, result.mean_dps);
}
```

Use a bundled default APL or your own rotation for meaningful comparisons;
the single-action rotation above only demonstrates the API. `prepare_synthetic`
returns the regular `SimulationRequest`, allowing existing progress callbacks,
tracing, cancellation, and sampled comparisons. `prepare_synthetic_build`
returns the regular `NormalizedDpsProfile` and evidence snapshot.

`with_secondary_ratings` replaces all four baseline ratings. `with_rating_delta`
adds signed deltas to a cloned build and rejects negative resulting ratings.
Other selections can be modified directly on a clone.

`synthetic_from_character(&character)` converts a valid equipment document by
collecting its contributions before applying effects. It preserves source
positions and evidence identities for exact seeded equivalence. It rejects
multi-legendary inputs, including historical equipment fixtures that allowed
them. Do not copy final profile attributes back into baseline ratings: that
would apply bonuses twice.

## Reproducibility and batching

`canonical_json()` produces stable JSON with sorted maps, talents, and relics.
The evaluation cache key includes the resolved profile/evidence, game and model
versions, semantic APL, encounter settings, iterations, and seed. Run labels and
the exploratory marker do not affect combat. Use explicit matching seeds for
controlled comparisons. Changing a build normally changes its derived seed.

Optional `sourcePositionId` fields preserve effect instance identities, which
also control ordering and result labels. Omitted positions use catalog defaults;
positions do not reserve slots. Conversion supplies the original positions.
`evidenceSourceIds` retains known item/gem identities for evidence reporting and
does not grant their stats or effects. Preserve conversion metadata when testing
exact equipment equivalence.

The context reuses the parsed APL and the process-wide immutable catalog. Batch
results are lazy and ordered; invalid entries return individual errors, and
cancellation stops remaining entries. Every evaluation creates fresh combat
state. Existing simulation workers handle iterations; the batch adds no threads.

CLI request and batch documents use either
`{ "kind": "synthetic", "document": { "format": "fellersim-synthetic-build", "build": ... } }`
or `{ "kind": "synthetic-file", "path": "synthetic.json" }` as their `character`
source. They support the existing `batch` and `compare` commands, including mixed
equipment/synthetic comparisons. Cases compared together must have matching
encounter settings, including duration. CLI manifests accept up to 64 cases;
use multiple manifests or the Rust iterator for larger searches.

For example, save this as `request.json` beside `synthetic.json`:

```json
{
  "version": 1,
  "character": { "kind": "synthetic-file", "path": "synthetic.json" },
  "apl": { "kind": "default" },
  "options": {
    "durationSeconds": 660,
    "iterations": 100,
    "targets": 1,
    "seed": "0123456789abcdef"
  }
}
```

Run `fellersim run --input request.json --json`. File paths resolve relative to
the manifest. A request document contains its simulation options; do not combine
`--input` with `--duration-seconds` or other simulation-option flags.

Both build types accept `durationSeconds` in options, from 1 through 1,200 whole
seconds. The default remains 300; 660 is an ordinary configurable duration.
Changing the scenario contract to record duration changes fingerprints and
automatically derived seeds. Explicit-seed 300-second combat results are retained.

## Model limits and measurement

Synthetic builds use the existing six-hero stationary-dummy model, including its
target-health assumptions, no incoming damage, and reviewed scenario-no-op
effects. They do not add dungeon encounters, external party buffs, new game
mechanics, equipment feasibility checks, or an optimizer. Inspect preparation
evidence and modeling limitations before interpreting results.

To measure construction, cloning, resolution, simulation, and sequential batch
throughput in a release build, run:

```sh
cargo test --release -p fellersim-core synthetic_build_benchmark -- --ignored --nocapture --test-threads=1
```

The benchmark warms catalog loading, compares equivalent equipment/synthetic
builds at 300 and 660 seconds with fixed seeds and 100 iterations, and prints
three repetitions with elapsed times and allocation measurements. Run it without
other heavy workloads. Construction timing measures an empty synthetic build;
clone/resolution/simulation timings use the representative equipped build.
