# Examples and common mistakes

Each working APL below is a complete file for an unequipped Ardeos (`firemage`).
They teach individual ideas, not recommended damage rotations. Try one at a time
with the [getting started](Getting-started.md) commands.

## Spend before reaching the resource limit

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/detonate,if=resource.embers.current=resource.embers.max
actions+=/infernal_wave
```

Detonate takes priority at maximum Embers. Otherwise, try Infernal Wave.

## Refresh an effect

<!-- apl-test {"hero":"firemage","firstAction":"searing-blaze"} -->
```apl
actions=/searing_blaze,if=dot.searing_blaze.remains<3
actions+=/infernal_wave
```

An absent effect has zero seconds remaining, so this both applies and refreshes it.
The threshold is an example, not a universal best refresh time.

## Save an action until another cooldown is ready

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/fire_frogs,if=cooldown.wildfire.ready
actions+=/infernal_wave
```

Fire Frogs is tried only while Wildfire's cooldown is ready. This does not cast
Wildfire or guarantee that the rest of a burst sequence will work.

## Change priorities with target count

<!-- apl-test {"hero":"firemage","targets":3,"firstAction":"fire-ball"} -->
```apl
actions=/fire_ball,if=targets.count>=2
actions+=/infernal_wave
```

Use `--targets 3` to try the first rule. With one target it is excluded before
the fight. Target count remains fixed during a run.

## Use a time window

<!-- apl-test {"hero":"firemage","statuses":["simplified","active"],"firstAction":"infernal-wave","blocker":"outside-fight-window"} -->
```apl
actions=/searing_blaze,if=fight.elapsed>=30&fight.elapsed<60
actions+=/infernal_wave
```

The first rule applies from 30 seconds up to, but not including, 60 seconds.
It is skipped at the start of the fight. An action must still be usable when
the simulator reaches it.

## Use a talent or legendary check

<!-- apl-test {"hero":"firemage","statuses":["statically-false","statically-false","active"]} -->
```apl
actions=/fire_ball,if=talent.firemage-talent-id-talent1.enabled
actions+=/searing_blaze,if=legendary.back_a_criticalstrike.equipped
actions+=/infernal_wave
```

Both checks answer no on the starter character. Copy the exact query for your
own build from `catalog apl-references`. A talent or legendary check does not
select or equip anything.

## Check fight health and remaining time

<!-- apl-test {"hero":"firemage","firstAction":null,"seconds":151,"observedValues":[100.0,50.0],"firstCastAtMs":150000,"shortCircuit":true} -->
```apl
actions=/infernal_wave,if=target.health.pct<=50&target.time_to_die<=150
```

This waits until halfway through the five-minute fight. At the start, the health
comparison sees 100 and 50; the time comparison is skipped. At 150 seconds,
health is 50 and time remaining is 150, regardless of damage dealt. This is a
teaching example: it deliberately spends half the fight without choosing a cast.

## Common mistakes

These examples are **intentionally invalid**. Each is followed by a complete fix.

### A number without a comparison

<!-- apl-test {"hero":"firemage","error":"Expected a numeric comparison operator"} -->
```apl
actions=/detonate,if=resource.embers.current
```

Say how many Embers you mean:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/detonate,if=resource.embers.current>=4
```

### Comparing a yes/no query to a number

<!-- apl-test {"hero":"firemage","error":"Unexpected token after condition"} -->
```apl
actions=/detonate,if=buff.wildfire.up=1
```

Use the yes/no query directly:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/detonate,if=buff.wildfire.up
```

### Using two equals signs

<!-- apl-test {"hero":"firemage","error":"Unsupported state query"} -->
```apl
actions=/detonate,if=resource.embers.current==4
```

Use one equals sign:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/detonate,if=resource.embers.current=4
```

### Starting the second action as if it were the first

<!-- apl-test {"hero":"firemage","error":"Expected actions+=/"} -->
```apl
actions=/searing_blaze
actions=/infernal_wave
```

Use `actions+=/` for later action lines:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/searing_blaze
actions+=/infernal_wave
```

### Using another hero's resource

<!-- apl-test {"hero":"firemage","error":"does not belong to this hero"} -->
```apl
actions=/infernal_wave,if=resource.anima.current>0
```

Ardeos does not use Anima. Discover his supported queries and choose the one you
actually intend, for example:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/infernal_wave,if=resource.embers.deficit>0
```

### Changing the hyphens in a talent ID

<!-- apl-test {"hero":"firemage","error":"unknown hero talent"} -->
```apl
actions=/infernal_wave,if=talent.firemage_talent_id_talent1.enabled
```

Talent IDs keep their hyphens:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/infernal_wave,if=talent.firemage-talent-id-talent1.enabled
```

For a name the simulator does not recognize, use discovery rather than guessing a
replacement. For a valid rule that never runs, use
[explanations and traces](Why-actions-run.md).
