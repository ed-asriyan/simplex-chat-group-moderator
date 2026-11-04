# Feature workflow skills
Skills for Claude Code that take a feature through six reviewed stages, each started by the maintainer and each stopping for review:

| Skill | Stage | Changes |
|---|---|---|
| `/stories` | 1. user stories | the stories document |
| `/ux` | 2. user experience | the UX document: the screens the stories are played on, their elements and states; nothing to do for a project without a user interface |
| `/e2e-tests` | 3. end-to-end tests | the end-to-end tests and their harness: each changed story played by a user against the whole running system; the new tests fail |
| `/design` | 4. design | the ports, a new migration, and `docs/design.md` when a component's responsibilities change; nothing else, even if the rest stops compiling |
| `/unit-tests` | 5. unit tests | stub bodies so the code compiles again, the regenerated schema, and the unit tests; the new tests fail |
| `/implement` | 6. implementation | everything else; all unit and end-to-end tests pass |
| `/coverage` | any time, read-only | nothing: reports the stories the tests do not really check, per level |

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
- **UX:** the UX document and where the user interface's code is and how to open it, or that the project has no user interface.
- **Stub body:** what a body that is not implemented yet is (`todo!()`, `raise NotImplementedError`, …).
- **Schema snapshot:** where the generated whole schema is (usually `docs/design.md` *Schema*), and the command that regenerates it.
- **Tests:** how a unit test cites its story, the test helpers, the edge cases the stories have.
- **End-to-end tests:** where they and their harness live, how a test cites its story, how the harness starts the system (the production images, locally), plays a user and moves the clock, and the command that runs them.
- **Not testable by nature:** per level (unit, end-to-end), what `/coverage` reports as such rather than as a gap.
- **Only the implementation writes:** the parts of the code that are never stubbed (texts, say), only written.
- **Commands:** build, compile the tests, test, run the end-to-end tests, lint, and the coverage script's arguments: `coverage/scripts/story_coverage.sh <stories file> <story id regex> <level>=<source dir>:<test file glob> …`.

`AGENTS.md` also describes the process itself (the six stages, that each is reviewed, that no stage starts the next), so an agent working outside a skill knows it.
