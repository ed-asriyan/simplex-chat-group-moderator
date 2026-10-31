# Feature workflow skills
Skills for Claude Code that take a feature through four reviewed stages, each started by the maintainer and each stopping for review:

| Skill | Stage | Changes |
|---|---|---|
| `/stories` | 1. user stories | the stories document |
| `/design` | 2. design | the ports, a new migration, and `docs/design.md` when a component's responsibilities change; nothing else, even if the rest stops compiling |
| `/tests` | 3. tests | stub bodies so the code compiles again, the regenerated schema, and the tests; the new tests fail |
| `/implement` | 4. implementation | everything else; all tests pass |
| `/coverage` | any time, read-only | nothing: reports the stories the tests do not really check |

The skills hold nothing particular to one project, so the same directory serves every project that provides the two files below. A change to a skill is a change for every project.

## What a project provides

### `docs/design.md`
How the project is built, in these sections; the skills refer to them by name:

- `## Overview` — what the system is, the architectural approach (hexagonal, layered, …) and the rules it sets for every change.
- `## Components` — one `### <name>` per component or bounded context, each with **Responsible for:**, **Not responsible for:** and **Interacts with:** (naming the ports or interfaces through which), and the tables it owns, if any.
- `## Source layout` — where each kind of code goes: ports, implementations, migrations, tests. `/design` changes only the ports and the migrations, so this section has to say which files they are.
- `## Schema` — for a project with a database: the whole schema, generated from the migrations and checked by a test, never edited by hand.

It describes the current design only, and is changed by `/design` when a component's responsibilities or interactions change.

### `AGENTS.md`, section `## Workflow`
The project's values the skills use:

- **Stories:** the stories document, the story id format and its regex, how a story is written there.
- **Stub body:** what a body that is not implemented yet is (`todo!()`, `raise NotImplementedError`, …).
- **Schema snapshot:** where the generated whole schema is (usually `docs/design.md` *Schema*), and the command that regenerates it.
- **Tests:** how a test cites its story, the test helpers, the edge cases the stories have.
- **Not unit-testable by nature:** what `/coverage` reports as such rather than as a gap.
- **Only the implementation writes:** the parts of the code that are never stubbed (texts, say), only written.
- **Commands:** build, compile the tests, test, lint, and the coverage script's arguments: `coverage/scripts/story_coverage.sh <stories file> <source dir> <test file glob> <story id regex>`.

`AGENTS.md` also describes the process itself (the stages, that each is reviewed, that no stage starts the next), so an agent working outside a skill knows it.
