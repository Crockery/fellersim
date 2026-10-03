# Why an action runs or gets skipped

## Each choice starts at the top

The simulator checks the list in order whenever it can choose another action.
It picks the first usable action whose conditions pass. It does not cast every
action in the list in sequence.

A rule can be skipped because it is disabled, the build lacks the action, its
time window has not arrived, its cooldown or resource cost blocks it, or its
condition does not pass. An action lower down cannot interrupt an action already
being performed.

If nothing can run, time advances to the next relevant event or check and the
simulator tries again. It does not invent an extra filler action. Automatic
attacks and ongoing effects still follow their own rules.

## Ready does not always mean usable

A ready cooldown only answers a cooldown question. Detonate can have a ready
cooldown while Ardeos has too few Embers to cast it. The simulator checks whether
an action can be used **before** checking its condition. You do not need to repeat
its normal resource and cooldown requirements in every rule.

## Inspect the rules before running

```sh
fellersim apl explain --character character.json --apl rotation.apl --json
```

The returned rule statuses mean:

| Status | Meaning |
| --- | --- |
| `disabled` | You commented out this action. |
| `unavailable` | This is a supported optional action, but this build does not provide it. |
| `statically-false` | The condition cannot pass with these inputs, such as asking for two targets in a one-target run. |
| `simplified` | Some checks already have known answers, or a fight-time condition was turned into a time window. The rule keeps its meaning. |
| `active` | The rule is available for checks during the fight. It is not a promise that the action will run. |

For an unequipped Ardeos with one target, this complete example produces those
five statuses in order:

<!-- apl-test {"hero":"firemage","statuses":["disabled","unavailable","statically-false","simplified","active"],"firstAction":"infernal-wave","shortCircuit":true} -->
```apl
# actions=/detonate
actions+=/weapon_frost_volley
actions+=/fire_ball,if=targets.count<0
actions+=/fire_ball,if=targets.count>0&resource.spirit.current<0&cooldown.fire_ball.ready
actions+=/infernal_wave
```

The weapon is missing. The third rule can never pass. In the fourth, “at least
one target” is known before the fight. The remaining Spirit check fails, so the
later cooldown check is skipped. Infernal Wave is the first usable action.

Unknown names or names from the wrong hero can be validation errors, rather
than `unavailable` rules. A false condition does not hide an invalid query.

## See a choice during the fight

```sh
fellersim trace --character character.json --apl rotation.apl --seed 0123456789abcdef --iteration-index 0 --seconds 10 --decisions 20 --json
```

A trace records each observed choice, the rules checked, the action selected,
and relevant combat state. `blocker` explains a skipped rule: for example,
`cooldown`, `resource:embers`, `condition-false`, or `outside-fight-window`.
The selected action uses its game ability ID. Match that against `gameAbilityId`
in `catalog abilities` to find its readable name and APL token.

With `A&B`, the second check is skipped if the first fails. With `A|B`, it is
skipped if the first passes. In JSON this is called `shortCircuited`. A skipped
check has not been measured; do not read its `passed: false` field as a failed
condition. A rule blocked before its conditions are checked can have no condition
observations at all. Lower-priority rules after the chosen action are not checked.

Rules excluded before the fight appear in the accompanying explanations, not
among runtime checks. Use the source map to match rule IDs to file lines.
APL file line and column numbers start at one. Trace times ending in `Ms` are
milliseconds; APL time queries are seconds.

The default capture is ten seconds and 200 choices. `--seconds` and `--decisions`
change the capture. The full five-minute encounter still runs. `truncated` means
the capture stopped early, not that the fight stopped early.

Keep the same character, APL, options, seed, iteration index, and simulator version
to reproduce a trace. The [agent guide](../../agent-guide.md) explains repeatable
runs and fair comparisons between rotations.

## Fix an error

Run `validate --character character.json --apl rotation.apl --json`.
Check `ok` before using the result. Read each diagnostic's message, line, column,
and `help`. Syntax errors can prevent later checks, so fix those first and retry.
The simulator reports up to 100 diagnostics at once; `diagnosticsTruncated`
means more were omitted. Examples of fixes are on the
[common mistakes page](Examples-and-common-mistakes.md#common-mistakes).
