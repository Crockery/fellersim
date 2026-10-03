# Language reference

## Action lines

This complete Ardeos example spends Embers when full, then uses Infernal Wave:

<!-- apl-test {"hero":"firemage"} -->
```apl
actions=/detonate,if=resource.embers.current=resource.embers.max
actions+=/infernal_wave
```

Read the first line as: “Try Detonate if current Embers equal maximum Embers.”

- Start the first action with `actions=/` and every later action with `actions+=/`.
- Write one action per line. Blank lines are allowed.
- Add `,if=` followed by a condition when needed. Without it, the action has no
  extra condition. The simulator still checks whether it can be used.
- Order matters. Each new choice starts at the top, rather than moving to the
  next line after a cast. You can list the same action more than once.

## Comments and disabled actions

A comment starts with `#`. It can occupy a line or follow an action.
Put `#` before an action line to disable it:

<!-- apl-test {"hero":"firemage","statuses":["disabled","active"],"firstAction":"infernal-wave"} -->
```apl
# Practice rotation: leave Detonate switched off.
# actions=/detonate,if=resource.embers.current>=4
actions+=/infernal_wave # Keep attacking.
```

The disabled action is still the first action line, so Infernal Wave still uses
`actions+=/`. Disabled actions must have readable action/condition syntax, though
the simulator does not check whether their action and query names fit the build.
To leave an unfinished line as an ordinary note, write something like
`# Note: try a different Detonate condition` instead.

## Conditions

Queries return either a number or a yes/no answer. A yes/no query can stand alone,
such as `buff.wildfire.up`. A number needs a comparison, such as
`resource.embers.current>=4`. There is no automatic conversion between the two.

| Symbol | Meaning | Example condition |
| --- | --- | --- |
| `=` | Equal | `resource.embers.current=4` |
| `!=` | Not equal | `resource.embers.current!=0` |
| `<` | Less than | `dot.searing_blaze.remains<3` |
| `<=` | Less than or equal | `fight.remains<=5` |
| `>` | Greater than | `targets.count>1` |
| `>=` | Greater than or equal | `cooldown.fire_ball.charges>=1` |

You can compare two numbers, two numeric queries, or one of each. Equality uses
the exact values; a regenerating resource can pass a threshold between choices.
Use `>=` when you mean “at least.”

### Combining conditions

| Symbol | Meaning |
| --- | --- |
| `&` | Both conditions must pass. |
| `\|` | At least one condition must pass. |
| `!` | Reverse a condition: yes becomes no, and no becomes yes. |
| `(...)` | Group conditions together. |

Comparisons form single conditions. `!` applies to the condition after it.
Then `&` groups more tightly than `|`. Parentheses make your intended grouping
clear. For example, `A|B&C` means `A|(B&C)`, not `(A|B)&C`.
Here `A`, `B`, and `C` are explanation placeholders, not valid APL names.

These complete Ardeos examples show the difference. With one target, the first
condition passes; the grouped version does not:

<!-- apl-test {"hero":"firemage","statuses":["simplified","active"],"firstAction":"searing-blaze"} -->
```apl
actions=/searing_blaze,if=targets.count=1|targets.count=2&targets.count=3
actions+=/infernal_wave
```

<!-- apl-test {"hero":"firemage","statuses":["statically-false","active"],"firstAction":"infernal-wave"} -->
```apl
actions=/searing_blaze,if=(targets.count=1|targets.count=2)&targets.count=3
actions+=/infernal_wave
```

Use `!(...)` to reverse a whole group. Conditions do not spend resources or
perform actions; they only check state. Checks can stop once the answer is known.

## Names and spacing

Copy exact `token` values from [discovery commands](Checking-combat-state.md#finding-valid-names).
Display names such as “Infernal Wave” are not action names.

- Keywords and names are read without regard to ASCII letter case. Lowercase is
  easiest to read and matches discovery output.
- Action names accept underscores or hyphens: `infernal_wave` and `infernal-wave`
  identify the same action. Most query names use the same rule.
- Talent IDs are the exception: keep their hyphens exactly as returned. For
  example, `talent.firemage-talent-id-talent1.enabled`.
- Legendary queries omit the item's initial `legendary-` part. Copy the complete
  query from `catalog apl-references` instead of constructing it yourself.
- Spaces at the start/end of lines and between condition parts are allowed.
  Keep `actions=/`, `actions+=/`, `,if=`, names, and operators intact. Do not
  put a space between an action name and `,if=`.
- Windows and Linux line endings are accepted. Save files as UTF-8.

This complete Ardeos example uses capital letters, a hyphenated action name,
spaces inside the condition, and grouped negation. It can cast at the start of a
one-target fight:

<!-- apl-test {"hero":"firemage","firstAction":"infernal-wave"} -->
```apl
ACTIONS=/INFERNAL-WAVE,IF=!(fight.elapsed < +0.0 | targets.count != 1e0)
```

## Numbers and unsupported features

Numbers can be whole numbers or decimals, including signed values and scientific
notation: `4`, `2.5`, `-1`, `+2`, and `1e2`. They must be finite. Write seconds as
numbers such as `2.5`, not `2.5s`; write 25 percent as `25`, not `25%`.

Use `=`, `&`, and `|`, not `==`, `&&`, or `||`. There are no `true`/`false`
literals, strings, arithmetic, functions, variables, named action lists, target
cycling, or extra action options. APLs from other simulators are not directly
interchangeable with Fellersim APLs.

## Size limits

Ordinary rotations should be well below these limits:

| Check | Limit |
| --- | --- |
| APL file size | 1 MiB (1,048,576 bytes) |
| Action lines, including disabled lines | 128; at least one is required |
| Parts read within one condition | 4,096 names, numbers, or symbols |
| Condition nodes | 1,024 per condition while reading; 1,024 across the whole APL when validating |
| Nesting | Depth 64 for nested groups and negations; validation also checks the resulting condition tree |
| Comments | 1,024 across the APL |
| One comment or other text field | 4,096 bytes |
| Numeric literal range | −1,000,000 through 1,000,000 when validating |

A “node” is one comparison, yes/no query, negation, or group joined by `&` or `|`.
These are size safeguards, not targets to write toward. A file can be read
successfully and still fail validation, so always use `validate`.
