# Agent workflows

Start with `fellersim describe --json`. Its command tree, schemas, defaults,
limits, and exit codes come from the executable you will run.
`fellersim version --json` identifies its bundled game and model versions.
All commands work offline.

## Discover, construct, and repair

```sh
fellersim catalog heroes --json
fellersim character init --hero firemage > character.json
fellersim catalog talents --hero firemage --json
fellersim catalog equipment --hero firemage --position weapon --rarity Epic --json
fellersim schema character > character.schema.json
fellersim validate --character character.json --default-apl --json
```

Use canonical IDs returned by `catalog`. Equipment entries expose legal levels,
rarity configurations, modifier slots, socket tiers, trait trees, and blessing
choices. Search with `--query`, resolve one choice with `--id`, and follow
`nextOffset` until it is null. A hero query includes supported optional weapon
abilities; `--character character.json` reports abilities available to that build.
Game-specific legality is checked by `validate`, in addition to JSON Schema.

The starter is an ordinary planner export and can be imported back into
Fellership. Equipment positions are already present. Edit selections inside
`build`; keep its envelope intact.

On failure, inspect each diagnostic's `code`, `engineCode`, `source`, `path`,
`line`, `column`, `identifiers`, and `help`. JSON paths use JSON Pointer;
APL locations are one-based. Fix independent issues together, then validate
again. A prerequisite failure may prevent dependent checks. At most 100
diagnostics are emitted; `diagnosticsTruncated` reports omitted issues.

## Inspect and execute

```sh
fellersim prepare --character character.json --default-apl --json
fellersim apl default --hero firemage > rotation.apl
fellersim apl explain --character character.json --apl rotation.apl --json
fellersim run --character character.json --apl rotation.apl --iterations 1000 --json --quiet
fellersim trace --character character.json --apl rotation.apl --seed 0123456789abcdef --iteration-index 0 --json
```

`prepare` also works without an APL. With an APL it includes the resolved rules,
source map, seed, and semantic fingerprint. Rule explanations come from the
compiler and distinguish disabled, unavailable, statically false, and simplified
rules. Traces observe runtime rules after compilation, check cast availability
before conditions, and record short-circuited operands and resource/cooldown/buff
state. Excluded source rules remain visible in the accompanying explanations.

Trace capture defaults to 10 simulated seconds and 200 decisions. Set
`--seconds` and `--decisions` to adjust it within the limits in `describe`.
The complete five-minute encounter still executes after capture stops.
`truncated` refers to observation capture, not encounter duration.
Use the same character, APL, options, seed, and iteration index to reproduce it.

Summary output includes total and primary-target DPS, Monte Carlo uncertainty,
scenario, iterations, seed, fingerprints, and applicable model limitations.
`--detail full` adds damage/proc/target/uptime breakdowns and full evidence.
Treat model limitations separately from the sampling confidence interval.

## Requests, batches, and comparisons

The ready-to-run `examples/request.json` and `examples/variants.json` demonstrate
the versioned input documents. Inspect their current schemas with:

```sh
fellersim schema request
fellersim schema batch
fellersim run --input examples/request.json --json
fellersim batch --input examples/variants.json --jobs 2 --json
fellersim compare --input examples/variants.json --baseline original --json
```

A complete request uses a tagged `character` source (`file` with `path`, or
`inline` with the planner `document`) and an APL source (`file`, `inline`
with `source`, or `default`). It cannot be mixed with individual input or
simulation-option flags. `--input -` reads stdin. File references resolve
relative to the containing manifest; stdin references resolve relative to
the working directory. Character and APL documents are limited to 1 MiB each;
manifests to 64 MiB and batches to 64 cases.

Batch options apply to every case unless overridden in that case. Cases have
unique IDs and results retain input order. Invalid cases do not prevent valid
cases from running. `--jobs` defaults to one and accepts up to four; active
cases share a total four-worker budget.

Comparisons require one hero, game/model version, scenario, target count, and
iteration count. Put an explicit seed in shared `options.seed`, or leave it
unset to use the baseline's deterministic seed for every case. Case-specific
seed overrides are rejected. `--metric primary-target-dps` selects the alternate
metric; total DPS is the default.

The interval uses paired iteration differences, Student's t, and a Bonferroni
adjustment for every requested baseline comparison, including failed variants.
It has 95% family confidence under independent-iteration and t-approximation
assumptions. The calculation measures covariance; equal seeds alone do not
guarantee variance reduction. Classification is higher, lower, or inconclusive
according to the adjusted interval. A zero baseline has no defined percentage
change. These intervals measure Monte Carlo uncertainty, not game-model error.
See the [paired-interval treatment](https://www.itl.nist.gov/div898/handbook/prc/section3/prc312.htm)
and [Bonferroni method](https://www.itl.nist.gov/div898/handbook/prc/section4/prc463.htm).

## Process contract

`--json` selects one versioned response on stdout for successes, errors, help,
version, cancellation, and timeout. Check `ok` and the process exit code before
using `data`. Diagnostics are in the response. Progress uses stderr;
`--quiet` suppresses progress. Without `--json`, artifact commands
(`character init`, `schema`, and `apl default`) emit ready-to-save documents.
With `--json`, their documents are inside `data`.

Exit codes are 0 for success, 2 for arguments or invalid inputs, 1 for execution
or mixed batch failure, 124 for timeout, and 130 for cancellation.
`--timeout SECONDS` applies to execution commands. Ctrl+C cooperatively cancels
work. Batch responses preserve completed cases and explicitly label active
cancelled/timed-out cases and queued cases that were not started.
Inspect the individual outcomes even when the overall exit code is nonzero.

Generate response references with `schema run-response`,
`schema compare-response`, or another schema listed by `describe`.
The CLI protocol version is independent of planner-envelope, character-build,
and engine schemas.

## Reproducible manual model evaluation

Give a model only the executable path, an empty working directory, and one task
below. Keep networking disabled. Record the executable's `version --json`,
model identifier, task text, commands, exit codes, and final answer in your
evaluation environment; no model-provider integration is required.

| Task | Acceptance criteria |
| --- | --- |
| “Find Ardeos, create a valid character, select one legal talent, and simulate it.” | Uses discovery IDs, saves a planner export, validates it, reports positive DPS and reproducibility metadata. |
| “Repair a build with two unknown talent IDs and targets set to zero.” | Reads structured diagnostics, repairs both selections and targets, validates successfully without inventing IDs. |
| “Explain why an unequipped character skips its default weapon rule.” | Uses preparation or explanation; identifies unavailable ability rather than changing combat rules. |
| “Compare an original build and one talent variant at 1,000 iterations.” | Uses a shared seed and correct baseline; reports adjusted paired interval and separates model limitations from uncertainty. |
| “Run three labeled cases, one invalid, and recover the successful results.” | Reads all ordered case statuses despite exit 1; preserves completed results and explains the invalid case. |
| “Trace iteration 7, capturing two seconds, and reproduce its result.” | Keeps semantic inputs/seed/index fixed; recognizes capture truncation and the full encounter result. |

A task passes only when saved artifacts validate and the reported numbers and
status match CLI output. Track unnecessary retries, invented identifiers, missed
diagnostics, token use, and wall-clock time to compare model usability across
CLI changes. The automated offline workflows in the CLI test suite cover the
same protocol without a model.
