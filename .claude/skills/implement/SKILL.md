---
name: implement
description: Stage 4 of a feature — write the code that makes the reviewed tests pass, without changing the tests or the design, then stop for review.
argument-hint: what to focus on
disable-model-invocation: true
---

# Stage 4: implementation

You are doing **only** the implementation stage of the process in `AGENTS.md`. $ARGUMENTS

Read `docs/design.md` first: the rules of its *Overview* hold for every line you write, and *Source layout* says where each kind of code goes. The project's own values — the commands, the stub body, which parts of the code only this stage writes — are in `AGENTS.md` *Workflow*.

## Input
The stories, the design (`docs/design.md`, the ports, the migration), the stubs and the tests: the uncommitted diff. Start from the project's test command to see what fails.

## Scope
- Fill the stub bodies and whatever the stories require behind them, outside the test files.
- Everything the design left out is yours: applications, pure rule modules, adapters, routers, wiring in the composition root, texts and their wording, and the removal of code a removed feature no longer needs.
- **Don't change tests.** If a test looks wrong, or cannot pass without breaking the stories or the rules of `docs/design.md`, stop and explain; the maintainer decides whether the test or the story changes.
- **Don't change the design.** No new or changed component responsibility in `docs/design.md`, port, value type or migration beyond what `/design` wrote. If the code needs one, stop and say why; the maintainer decides, and the change is made to the ports or the migration in the same change, the regenerated schema included.
- A module's doc comment says what its code cannot: the rule it keeps that its callers rely on, the outside constraint it works around, why it is built the way it is. It does not retell the code. Update it when that changes.

## Exit check
1. The project's test command: all tests pass, old and new.
2. The project's lint command: no new warnings in the code you wrote.
3. A search of the source for the stub body: nothing left from this feature.
4. You edited no test file in this stage. If you had to, say which and why.

## Finish
Summarise the implementation per component, the exit check results, and **stop**. Suggest `/coverage` if the feature changed many stories. Do not commit unless the maintainer asks.
