# SimpleX Chat Group Moderator Bot — Agent Guide
Read this fully before making changes. It says what the project is and how work on it proceeds; how it is built is in [`docs/design.md`](docs/design.md).

## What this project is
A moderation bot for [SimpleX Chat](https://simplex.chat) groups, written in Rust (the crate lives in `bot/`), and its web editor (`webeditor/`). A group owner adds the bot to their group with the moderator role or higher; the bot then checks every member's message against the owner's rules and deletes the message, makes the author an observer or kicks them, as the rules say. The owner talks to the bot only in direct messages: `/groups` gives a link to the web editor, which carries the group's whole configuration — its mode and its rules — in the link's hash; the owner edits it in the browser and sends the updated link back. The full behaviour is specified in [`docs/user-stories.md`](docs/user-stories.md).

## Status
Implemented: the requirements are in `docs/user-stories.md`, the design in `docs/design.md`, the interface is the ports of the two bounded contexts, the rule types and the schema, and `bot/` and `webeditor/` implement it. The bot has unit tests for every condition, action, use case and adapter; the web editor has none, and no test cites a story yet.

## How work proceeds
Every feature, however small, goes through the same stages, and each stage is reviewed by the maintainer before the next one starts:

1. **User stories.** Change the stories first, if the behaviour changes. They are a living document: they describe the current intended behaviour only; stories that no longer apply are removed, not kept as history (git has the history).
2. **Design.** The ports (traits and the value types they exchange), a new condition's or action's type and its entry in `webeditor/rules-schema.json`, and the migration the stories need, and `docs/design.md` when the stories give a component a new responsibility or interaction, or need a new component — which most stories do not. Those files only: the rest of the code may stop compiling against the new ports, and the stage lists what the change reaches. This is what the maintainer decides on; everything behind it is the later stages'.
3. **Tests.** The code compiles again with `todo!()` bodies where it no longer fits the ports, the schema in `docs/design.md` is regenerated, and unit tests are written against the ports and pure functions. They compile and fail.
4. **Implementation.** Code that makes the tests pass.

If a later stage needs to deviate from an earlier one, the earlier stage's artefact is updated in the same change, and the maintainer is told.

Write only what is true now, in the stories, the design, code comments and test names. When something is removed or renamed, delete every mention of it: never write that a command, rule or type "no longer exists", "was replaced by", "is gone" or "used to", and never explain a decision by the old state of things. A reader who never saw the old version must not be able to tell there was one.

Each stage is a skill in `.claude/skills/`, started by the maintainer: `/stories`, `/design`, `/tests`, `/implement`. A skill does its own stage only, within the files it names, and stops for review; **never start the next stage on your own**, and never do a stage's work outside its skill. A stage with nothing to change says so instead of inventing work. `/coverage` checks, at any time, which stories the tests do not really cover. The skills hold nothing particular to this project: what they need from it is `docs/design.md` and *Workflow*. They are shared with other projects (see `.claude/skills/README.md`), so a change to a skill is made for every project, not for this one.

## Workflow
The project's values the stage skills use.

- **Stories:** `docs/user-stories.md`. A story id is `US-<epic letter><n>`, with an optional lower-case letter for a variant (`US-A1b`); its regex is `US-[A-Z][0-9]+[a-z]?`, and a story is defined as `**US-X1.**` at the start of its item. A bot story is written as the person sees the chat: what they send or do in SimpleX (`/groups`, an invitation with a role), then the bot's answer written out exactly, in its SimpleX Markdown, in a `text` block. A condition or action story gives the editor's title of the type (**💬 Message Contains Any of These Words**), real settings, a message that matches or not, and the reason line the owner is told. An editor story names the controls as the editor labels them (*Apply changes*, *Copy the prompt*) and says what the owner sees. The examples use the owner, group and member of *Conventions* in the stories.
- **Stub body:** `todo!()`.
- **Schema snapshot:** the *Schema* section of `docs/design.md`, generated, never edited by hand: `cd bot && UPDATE_SCHEMA=1 cargo test schema` rewrites it, and the same test without the variable fails while it is out of date.
- **Tests:** a test cites its story as `// US-E1` above it, or `[US-E11]` in the file's doc comment. A condition or action is tested in its own module's `tests.rs`; a use case through its inbound port with the fakes of `domain/moderator/application/tests.rs`; a storage adapter against the in-memory SQLite of `infrastructure/test_db.rs` (`test_db`). The architecture tests (`domain/architecture_tests.rs`, `domain/moderator/rules/architecture_tests.rs`) are never loosened to let a change pass. The edge cases the stories have: a member vs a moderator, admin or owner; each mode (*Notifications*, *Silent*, *Dry*); a condition alone, inside *All*/*Any* and under *Not*; an edited message; a message with an attachment, with and without a caption; the edges of a rate limit's time window; an AI provider that cannot be reached.
- **Not unit-testable by nature:** the SimpleX driver, and the web editor (the stories of epic G), which has no test suite and is checked in a browser.
- **Only the implementation writes:** the bot's texts (`domain/bot_dm/application.rs`) and the web editor's code (`webeditor/*.js`, `index.html`, `editor.css`): they are never stubbed.
- **Commands**, from `bot/`: build `cargo build --all-targets`; compile the tests `cargo test --no-run`; test `cargo test`; lint `cargo clippy` (it warns already, and `--all-targets` also reports the `unwrap()` the tests use: what counts is no new warning). Coverage, from the repository root: `.claude/skills/coverage/scripts/story_coverage.sh docs/user-stories.md bot/src tests.rs 'US-[A-Z][0-9]+[a-z]?'`.

## Language
Everything committed to this repository is in **English** — code, comments, docs, commit messages — regardless of the language the conversation with the maintainer happens in. The bot and the editor speak English only.

## Design
[`docs/design.md`](docs/design.md) is how the bot and the editor are built: the architecture and the rules it sets (checked by the architecture tests), the two bounded contexts (`bot_dm` and `moderator`) and the web editor with what each is responsible for and interacts with, where code goes, and the steps for adding a condition or an action type. Read it before changing code; its rules hold for every change, inside a stage or not.

## Migrations
The schema is changed by migrations in `bot/src/infrastructure/migrations/`, and the whole current schema is generated from them into `docs/design.md` *Schema* (*Workflow*); its conventions are in `docs/design.md` (*Persistence*).

- Files are named `NNNN_description.sql` (zero-padded, sequential). They are embedded at build time and applied in **filename sort order**.
- The applied version is tracked in SQLite's `PRAGMA user_version`; the runner (`infrastructure/migrations.rs`) skips already-applied files and runs each remaining file in its own transaction, then bumps `user_version`. The version is the file's 1-based position in the sorted list, so **never reorder, rename, renumber, or delete an existing migration file.**
- `PRAGMA foreign_keys = ON` is set for the connection, so `ON DELETE CASCADE` and FK constraints are enforced during migrations and at runtime.
- **Immutability rule:** once a migration has been merged to `master` it is **released** (see *Deployment*). Treat released migrations as frozen and add a **new** numbered migration to change the schema. Editing a migration in place is only acceptable while it is still unreleased (not yet on `master`) and has not run against any real database.
- When restructuring tables, include a data **backfill** in the same migration and drop old tables **children before parents** so FK enforcement stays satisfied.

## General conventions
- Async traits use `#[async_trait]`.
- Convert errors and types **at boundaries** (adapters/routers), not inside the domain.
- Keep tests in separate files (`#[cfg(test)] mod tests;` pointing to `<module_name>/tests.rs` or `<module_dir>/tests.rs`), never inline in implementation files.
- A module's doc comment (`//!`) says what its code cannot: the rule it keeps that its callers rely on, the outside constraint it works around (a messenger's limit, an API's behaviour), why it is built the way it is. It cites the stories it serves. It does not retell the code.
- Keep changes minimal and within the established structure; if a change seems to require breaking one of the hard rules in `docs/design.md`, stop and ask rather than working around it.

## Deployment
Every commit to `master` is the release: CI/CD builds and deploys the **bot** to production and publishes the `webeditor/` folder to **GitHub Pages**. There is no separate release step — merging to `master` ships to production. This is why a migration counts as released (and therefore frozen) the moment it lands on `master`. Pull requests run the tests and build the images (`.github/workflows/CI.yml`).
