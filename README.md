# Fellersim

Fellersim is an offline Fellowship character simulator. Download the Windows
or Linux archive from [Releases](https://github.com/Crockery/fellersim/releases),
extract it, and run the executable from a terminal. No game installation,
Fellership account, Node.js, or network connection is needed.

```sh
fellersim validate --character examples/ardeos.json --apl default-apls/ardeos.apl
fellersim run --character examples/ardeos.json --apl default-apls/ardeos.apl --iterations 10000
fellersim run --character examples/rime.json --apl default-apls/rime.apl --targets 5 --json > result.json
```

On Windows, use `./fellersim.exe` in PowerShell. On Linux, use `./fellersim`.
The examples start with unequipped characters; edit their JSON to describe
your equipment and talents. Incomplete equipment is allowed and contributes
only the selected stats and effects. Unknown or invalid selections are errors.

## Arch Linux and CachyOS

Install [`fellersim-bin`](https://aur.archlinux.org/packages/fellersim-bin) using
an AUR helper, or build the recipe with makepkg:

```sh
git clone https://aur.archlinux.org/fellersim-bin.git
cd fellersim-bin
makepkg -si
fellersim version --json
```

The package installs the published Linux x86-64 executable. No Rust toolchain is
required. Default APLs, examples, and the catalog are in `/usr/share/fellersim`;
documentation is in `/usr/share/doc/fellersim`. Copy files into your working
directory before editing them. For example:

```sh
cp /usr/share/fellersim/examples/ardeos.json ./character.json
fellersim run --character character.json --default-apl --json
```

Package maintenance and release retries are documented in the
[AUR maintenance guide](packaging/aur/README.md).

## Default APLs

The [default-apls](default-apls/) directory contains the action priorities used by
Fellership's default hero rotations. Read a hero's file to see its ability order
and conditions:

- [Ardeos](default-apls/ardeos.apl)
- [Rime](default-apls/rime.apl)
- [Tariq](default-apls/tariq.apl)
- [Elarion](default-apls/elarion.apl)
- [Mara](default-apls/mara.apl)
- [Gunde](default-apls/gunde.apl)

These files are included in release archives. Pass one to `--apl`, or copy and edit
it to try a different rotation. The same files are embedded in the executable. Use `--default-apl` to resolve
the character’s hero automatically, or `apl default --hero ID` to save a copy.

## Inputs and reproducibility

In Fellership's Character Planner, select **Export build** and pass the downloaded
JSON file directly to `--character`. Use `--default-apl` or supply a separate `--apl` file for the same
hero:

```sh
fellersim validate --character fellership-ardeos-character-build-v6.json --apl default-apls/ardeos.apl
```

Character files contain `format: "fellership-character-planner"`, `version: 6`,
and a `build` object. The nested build uses `schemaVersion: 6` and contains
`heroId`, `talentPoints`, `selectedTalentIds`, all
14 equipment `positions`, and `disabledConditionalContributionIds`.
An empty position has `item: null`. Equipped items specify `itemId`,
`itemLevel`, `rarity`, `appliedTempers`, `rolledModifiers`, `gems`, `traitTree`,
and `blessings`. The examples show the complete top-level structure.

The binary archive's `catalog.json` (at
`crates/fellersim-data/data/catalog.json` in source checkouts) documents the current
item IDs, configurations, talent IDs and costs, socket gems, blessing ranks,
and mechanics. Each hero's `items` map links rarity configurations to the
`configurations` map. Edit the selected IDs and values, not normalized damage
coefficients. The Rust `CharacterBuild` type describes the nested `build` object.

An APL is a list of actions the simulator checks in order. Read
[Writing an APL](docs/apl/Home.md) for a step-by-step introduction, the complete
language reference, and examples. The guide is included with downloads and is
also available on the [public wiki](https://github.com/Crockery/fellersim/wiki).
APL files use the same language as Fellership's editor.

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

`--json` writes one versioned response to stdout, including diagnostics on errors.
Human-readable output is the default. Machine results default to a compact summary;
`--detail full` includes all breakdowns and evidence. Progress uses stderr and
`--quiet` suppresses it. Ctrl+C and `--timeout SECONDS` cancel active work.
Worker count is automatically selected with the engine's four-worker maximum.

## Discovery and automation

Read the [agent workflow guide](agent-guide.md) for structured diagnostics,
request documents, partial batch results, paired comparisons, and reproducible
APL traces.

```sh
fellersim describe --json
fellersim catalog equipment --hero firemage --position weapon --json
fellersim character init --hero firemage > character.json
fellersim prepare --character character.json --default-apl --json
fellersim run --input examples/request.json --json
fellersim compare --input examples/variants.json --baseline original --json
```

`describe` generates the command reference from this executable.
`schema DOCUMENT` emits Draft 2020-12 schemas for inputs and responses.
The workflow guide explains exit codes and versioned output; no package manager
or language SDK is required.

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
