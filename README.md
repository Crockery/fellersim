# Fellersim

Fellersim is an offline Fellowship character simulator. Download the Windows
or Linux archive from [Releases](https://github.com/Crockery/fellersim/releases),
extract it, and run the executable from a terminal. No game installation,
Fellership account, Node.js, or network connection is needed.

```sh
fellersim validate --character examples/ardeos.json --apl examples/ardeos.apl
fellersim run --character examples/ardeos.json --apl examples/ardeos.apl --iterations 10000
fellersim run --character examples/rime.json --apl examples/rime.apl --targets 5 --json > result.json
```

On Windows, use `./fellersim.exe` in PowerShell. On Linux, use `./fellersim`.
The examples start with unequipped characters; edit their JSON to describe
your equipment and talents. Incomplete equipment is allowed and contributes
only the selected stats and effects. Unknown or invalid selections are errors.

## Inputs and reproducibility

Character files use schema version 6, the current Fellership character-build
format. Each file contains `heroId`, `talentPoints`, `selectedTalentIds`, all
14 equipment `positions`, and `disabledConditionalContributionIds`.
An empty position has `item: null`. Equipped items specify `itemId`,
`itemLevel`, `rarity`, `appliedTempers`, `rolledModifiers`, `gems`, `traitTree`,
and `blessings`. The examples show the complete top-level structure.

The binary archive's `catalog.json` (at
`crates/fellersim-data/data/catalog.json` in source checkouts) documents the current
item IDs, configurations, talent IDs and costs, socket gems, blessing ranks,
and mechanics. Each hero's `items` map links rarity configurations to the
`configurations` map. Edit the selected IDs and values, not normalized damage
coefficients. The Rust `CharacterBuild` type is the file contract.

APL files use the same language as Fellership's editor:

```text
# First matching action wins.
actions=/detonate,if=resource.embers.current=resource.embers.max
actions+=/searing_blaze,if=dot.searing_blaze.remains<3
actions+=/infernal_wave
```

Conditions support `&`, `|`, `!`, parentheses, and numeric comparisons
(`=`, `!=`, `<`, `<=`, `>`, `>=`). Queries cover cooldowns, resources, buffs,
damage-over-time effects, debuffs, selected talents, equipped legendaries,
target health/count, and elapsed/remaining fight time. The six example APLs
show supported queries. Prefix an action line with `#` to disable it.
Arithmetic, functions, variables, and named action lists are not supported.

`--config simulation.json` accepts `iterations`, `targets`, and `seed`.
Explicit flags override the file. Defaults are 10,000 iterations, one target,
and a seed derived from the semantic inputs. Iterations must be 100–100,000;
targets must be 1–20. An explicit seed is sixteen lowercase hexadecimal digits.
Comments, editor IDs, and run IDs do not change the derived seed.

Every encounter lasts five minutes against stationary, co-located dummies.
The current six supported heroes are Ardeos, Rime, Tariq, Elarion, Mara,
and Gunde. `fellersim --version` reports the simulator and bundled data/model
versions. Results include the game build, model, seed, and evidence summaries;
these make the assumptions and approximations of a run inspectable.

`--json` writes the result to stdout. Progress/errors use stderr; errors exit
nonzero. Ctrl+C cancels an active run. Worker count is automatically selected
with the engine's existing four-worker maximum.

## Building and contributing

Install stable Rust, then run:

```sh
cargo build --locked --release -p fellersim
cargo test --locked --workspace
cargo fmt --all --check
```

The executable is in `target/release`. This public repository contains the
complete source and runtime data needed to build it independently. First-party
code is MIT licensed; see `LICENSE` and `NOTICE`.

Development is maintained in a private monorepo shared with Fellership.
This repository receives explicit source exports. Public issues and pull
requests can be reviewed and incorporated there before the next export.
The proprietary website and hosting service are not part of this repository.
