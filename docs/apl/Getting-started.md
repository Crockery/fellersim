# Getting started

This walkthrough uses Ardeos. His command-line hero ID is `firemage`; names in
commands do not always match the names shown in the game.

## Save a character and rotation

Open a terminal in a writable folder. These commands assume `fellersim` is on
your command path. When running an extracted download, use `./fellersim` on
Linux or `./fellersim.exe` in PowerShell instead.

```sh
fellersim character init --hero firemage > character.json
fellersim apl default --hero firemage > rotation.apl
```

These redirection examples assume Bash or PowerShell 7 or later. Save generated
files as UTF-8 without a byte-order mark if you use another shell or editor.

The character starts with no equipment or selected talents. You can also use a
character exported from Fellership's Character Planner. Keep its JSON file
separate from the APL.

Open `rotation.apl` in a text editor. The first action starts with `actions=/`.
Each later action starts with `actions+=/`.

## Make one change

Find the Searing Blaze line in the default rotation. Change its refresh threshold
from `3` to `2`. Leave its `actions+=/` prefix in place.

This asks the simulator to refresh Searing Blaze when less than two seconds remain.
It does not force the action to run ahead of usable actions higher in the list.

For a shorter practice rotation, replace the whole file with this complete example:

<!-- apl-test {"hero":"firemage","firstAction":"searing-blaze"} -->
```apl
# Refresh Searing Blaze, then use Infernal Wave.
actions=/searing_blaze,if=dot.searing_blaze.remains<2
actions+=/infernal_wave
```

`if=` introduces a condition. `remains` is the time left in seconds. An absent
Searing Blaze has zero seconds left, so the first line also applies it at the
start. The second line has no condition, but the action still needs to be usable.

## Check and run it

```sh
fellersim validate --character character.json --apl rotation.apl
fellersim apl explain --character character.json --apl rotation.apl --json
fellersim trace --character character.json --apl rotation.apl --seed 0123456789abcdef --seconds 10 --decisions 20 --json
fellersim run --character character.json --apl rotation.apl --iterations 1000 --json
```

`validate` checks the files and names. `apl explain` shows which rules are usable
for this character. `trace` records the choices made during one fight. `run`
simulates many fights and reports average damage.

If a check fails, read the error's line number and message. Fix the file and check
again. A successful check means the rotation is valid, not that it is optimal.

The encounter lasts five minutes. Trace limits control how much of a fight you
see; they do not shorten the fight. See [why actions run](Why-actions-run.md)
for help reading the results.

## Find names for another hero

```sh
fellersim catalog heroes --json
fellersim catalog abilities --character character.json --json
fellersim catalog apl-references --character character.json --limit 50 --offset 0 --json
```

Copy the returned `token` values. An ability must have `manuallyCastable: true`
to be an action in your list. If `nextOffset` is a number, repeat the catalog
command with that number after `--offset`. Stop when it is `null`.

Continue with the [language reference](Language-reference.md).
