# Checking combat state

A **query** asks about the fight, such as “How many Embers do I have?” or
“Is Wildfire active?” The answer is a number or yes/no.

This complete Ardeos example checks whether Searing Blaze is missing:

<!-- apl-test {"hero":"firemage","firstAction":"searing-blaze"} -->
```apl
actions=/searing_blaze,if=!dot.searing_blaze.up
actions+=/infernal_wave
```

## Finding valid names

Use your actual character when possible:

```sh
fellersim catalog abilities --character character.json --json
fellersim catalog apl-references --character character.json --limit 50 --offset 0 --json
fellersim catalog resources --character character.json --json
fellersim catalog buffs --character character.json --json
fellersim catalog target-effects --character character.json --json
```

Copy the `token` field exactly. `valueType: "boolean"` means yes/no;
`valueType: "number"` means you need a comparison. Follow `nextOffset` with
`--offset` until it is `null` to read all results.

Use `--hero firemage` instead of `--character character.json` to explore general
Ardeos support. Hero results can include optional weapon abilities your current
build does not provide. Character results describe the supplied build.
Validation remains the final check: an action must be manually usable, a DoT
query must name an action that creates a queryable DoT, and a target-effect query
must be supported by the build. An unselected talent query can be valid and answer no.

## Query reference

In the “Query pattern” column, `ACTION`, `RESOURCE`, `BUFF`, `EFFECT`, `TALENT`,
and `ITEM` are placeholders. Replace them using discovery. The last column
contains real Ardeos conditions. Use them after `,if=` on an action line.
All time values below are **seconds**. Percentages use **0–100**, not 0–1.

<!-- query-examples hero=firemage action=infernal_wave -->
| Query pattern | Answer | Meaning | Example condition |
| --- | --- | --- | --- |
| `cooldown.ACTION.ready` | Yes/no | At least one use is ready on its cooldown. This does not check resource costs or every other requirement. | `cooldown.fire_ball.ready` |
| `cooldown.ACTION.remains` | Seconds | Wait until a use is ready. Zero if a charge is already available, even while another recharges. | `cooldown.fire_ball.remains<2` |
| `cooldown.ACTION.charges` | Number of uses | Available charges. For an ordinary single-use action, zero or one. | `cooldown.fire_ball.charges>=1` |
| `dot.ACTION.up` | Yes/no | This action's damage-over-time effect is active on the primary target. | `dot.searing_blaze.up` |
| `dot.ACTION.remains` | Seconds | Time left on that effect; zero if absent. For independently applied copies, the longest remaining time. | `dot.searing_blaze.remains<3` |
| `resource.RESOURCE.current` | Resource amount | Amount held now. | `resource.embers.current>=4` |
| `resource.RESOURCE.max` | Resource amount | Maximum amount for this build. | `resource.embers.current=resource.embers.max` |
| `resource.RESOURCE.deficit` | Resource amount | Amount missing from maximum, never below zero. | `resource.embers.deficit=0` |
| `resource.RESOURCE.percent` | Percent | Current amount divided by maximum, times 100; zero if maximum is zero. | `resource.spirit.percent>=90` |
| `buff.BUFF.up` | Yes/no | A supported buff has active stacks. | `buff.wildfire.up` |
| `buff.BUFF.remains` | Seconds | Time left on a supported buff; zero if absent. | `buff.wildfire.remains>2` |
| `buff.BUFF.stacks` | Number of stacks | Active stack count; zero if absent. | `buff.apocalyptic_surge.stacks>=1` |
| `debuff.EFFECT.up` | Yes/no | A supported effect is active on the primary target. | `debuff.searing_blaze.up` |
| `debuff.EFFECT.remains` | Seconds | Time left on that target effect; zero if absent. | `debuff.searing_blaze.remains<3` |
| `debuff.EFFECT.stacks` | Number of stacks | Stack count reported for that target effect; zero if absent. | `debuff.searing_blaze.stacks>=1` |
| `talent.TALENT.enabled` | Yes/no | This build has selected the talent. | `talent.firemage-talent-id-talent1.enabled` |
| `legendary.ITEM.equipped` | Yes/no | This build has the supported legendary effect. | `legendary.back_a_criticalstrike.equipped` |
| `targets.count` | Number of targets | Configured target count, fixed throughout the fight. | `targets.count>=2` |
| `target.health.pct` | Percent | Simulated primary-target health, falling with elapsed fight time. | `target.health.pct<20` |
| `target.time_to_die` | Seconds | Time until the encounter ends; the same as `fight.remains`. | `target.time_to_die<10` |
| `fight.elapsed` | Seconds | Time since the fight began. | `fight.elapsed>=30` |
| `fight.remains` | Seconds | Time until the encounter ends. | `fight.remains<10` |

`dot` means damage over time. `debuff` covers the supported target effects listed
by discovery, including some DoTs and equipment effects. It is not a way to query
every effect in the game. The available properties can differ by effect.

Some stack effects, such as Glacial Assault, report the remaining fight time
while stacks are held. Use `up` or `stacks` when you only need to check whether
an effect is active.

## Targets and fight time

Fellersim uses a five-minute encounter against stationary, co-located targets.
For APL conditions, target health starts at 100 percent and falls evenly with
time. At 150 seconds it is 50 percent; changing your damage does not change it.
`target.time_to_die` and `fight.remains` both start at 300 seconds.

DoT and debuff queries inspect the primary target. They do not count all targets
with an effect, and you cannot choose a different target in an action line.

See [why actions run](Why-actions-run.md) to understand how these answers are used.
