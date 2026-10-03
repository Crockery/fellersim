# Writing an APL

An **APL** (action priority list) is a list of actions the simulator checks in
order. It picks the first usable action whose conditions pass. You can change
the list to try a different rotation.

You do not need programming experience. Start with a default rotation and make
one small change at a time.

## Where to start

1. [Getting started](Getting-started.md): save, edit, check, and run a rotation.
2. [Language reference](Language-reference.md): learn how to write action lines and conditions.
3. [Checking combat state](Checking-combat-state.md): find out what you can ask about the fight.
4. [Why an action runs or gets skipped](Why-actions-run.md): understand the simulator's choices.
5. [Examples and common mistakes](Examples-and-common-mistakes.md): copy a pattern or fix a problem.

The [default rotations](../../default-apls/) are full examples for each supported
hero. The examples in this guide teach individual ideas; they are not claims
about the best rotation.

## A checklist for people and models

1. Run `fellersim version --json` to identify the simulator you are using.
2. Use `catalog heroes` to find the hero's ID. Use `catalog abilities` and
   `catalog apl-references` with your character to get exact action and query names.
   Read every results page; `nextOffset` tells you whether more results remain.
3. Save a default APL, then edit it. Do not guess names or borrow unsupported
   syntax from another simulator.
4. Run `validate`. Fix the reported problems before simulating.
5. Use `apl explain` to inspect the rules and `trace` to see actual choices.
6. Run the simulation. Keep the character, APL, options, seed, and version with
   results you want to reproduce.

Commands work offline. Add `--json` when a program or model needs to read the
output. The [agent workflow guide](../../agent-guide.md) explains that output
and how to compare rotations fairly.

## Which version does this describe?

The [public wiki](https://github.com/Crockery/fellersim/wiki) follows the public
repository's `main` branch. Its footer identifies the published revision.
The Markdown included with a download describes that release. If they differ,
use the guide shipped with your executable and its discovery commands.
