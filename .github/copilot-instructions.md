# SimpleX Chat Group Moderator Bot — Project Guide
Read this fully before making changes. It explains what the project does, how the code is organized, and the rules for *where* new code goes. Follow the existing structure — do not invent new layers or put logic in the wrong place.

## What this project is
A moderation bot for [SimpleX Chat](https://simplex.chat) groups, written in Rust (crate lives in `bot/`). A group owner adds the bot to their group as a member with moderator rights. The bot then watches group messages and **deletes** any message that violates the owner-configured moderation rules (e.g. blacklisted words, disallowed link domains).

Owners interact with the bot entirely through **direct messages**:
- Add/list their groups, toggle notifications and "dry mode".
- Edit moderation rules via an external **web editor** (`webeditor/`). The bot sends the owner a link; the rules are serialized into the URL hash, edited in the browser, and sent back to the bot as text.

There are two independent surfaces — DM conversation handling and group message moderation — and the codebase models them as two separate bounded contexts.

## Main user flows (the "why")
1. **Join a group.** A user DMs the bot and invites it to their group with a **moderator** role. The bot accepts and joins. The inviting user becomes that group's **owner**.
2. **Moderate messages.** Every message posted in a joined group is passed through **all** of that group's rules, in order. If any rule matches, the bot deletes the message (unless the group is in *dry mode*) and, if notifications are on, DMs the owner that a message was moderated.
3. **Manage groups via DM.** In private chat the owner can list their groups; for each group the bot replies with a **link to the web editor** (`webeditor/`, served from GitHub Pages). The link's URL hash carries that group's current rules encoded as JSON.
4. **Edit rules (the URL-hash hack).** The owner opens the link; `webeditor/index.html` decodes the rules from the hash and renders an editor. After editing, they click **Apply changes**: JavaScript writes the updated rules back into the URL hash and copies the full URL to the clipboard, then a popup tells the owner to paste it into the bot chat and send it. The bot receives that plain text, parses the rules out of the hash, and saves them. The hash must always carry **both** `bot_id` and `rules`, or the bot ignores the message (`parse` in `domain/bot_dm/application.rs`), so `editor.js` rewrites it on every change.

### The web editor is schema-driven
`webeditor/` is three static files published to GitHub Pages: `index.html` (a shell), `editor.css`, and `editor.js` (the renderer; its only dependency is lz-string). **`editor.js` knows no condition or action type by name.** It builds its registry from `rules-schema.json` at load: the entry's `title` gives the row label, its leading emoji gives the icon *and* the group in the type picker; `description` becomes the text behind the row's `ⓘ`; `properties` become the controls (`integer` → number field, `boolean` → checkbox, `array` of strings → chips, or a textarea in bulk mode past 12 entries, `$ref` to a condition → nested rows); `default`, `minimum`/`maximum`, `maxItems`/`maxLength` and each field's own `description` are used as they are. So **a new rule type needs no frontend code** — only the `oneOf` entry described in the steps below. A type the schema does not know (a rule stored by a newer bot) renders as a read-only unknown row and is sent back untouched.

What the schema cannot express lives in optional `options` keys on the same entry, each with a fallback when absent: `summary` (the one-line value summary in the row), `phrase` (the human wording in the rules list, mirroring `ModerationCondition::describe`), `execution_rank` and `covered_by` (action order and coverage — that knowledge belongs to `action_planner`), `label` (a short field label when the schema `title` is a whole sentence). `definitions.condition.options.max_depth` / `max_nodes` mirror `MAX_CONDITION_DEPTH` / `MAX_CONDITION_NODES` so the editor warns before the bot rejects the rules. The templates take `{field}`, `{field|n}`, `{field|list}`, `{field|phrase(s)}` and `{field?yes:no}` (with `$` for the value).

**Why this round-trip?** SimpleX Chat only supports plain **text** messages — no apps, buttons (unlike e.g. Telegram bots). Encoding rules in a shareable URL lets owners use a real GUI editor instead of hand-writing JSON/YAML into chat, while keeping the transport a single text message that the bot can parse.

## Architecture: hexagonal (ports & adapters) + two bounded contexts
The crate is split into two top-level layers (`bot/src/lib.rs`):
- `domain/` — pure business logic. **No I/O, no SQL, no network, no external crates for side effects.** Defines *ports* (traits) and implements *use cases*.
- `infrastructure/` — everything that talks to the outside world. Implements the domain's ports.

The composition root is `bot/src/bin/bot.rs` — it constructs every concrete adapter, wires them to the domain applications via `Arc<dyn Trait>`, and runs the SimpleX event loop. **Wiring happens only here.**

### The two bounded contexts (under `domain/`)
1. **`bot_dm`** (`domain/bot_dm/`) — handles direct-message conversations with users: `/start`, listing groups, notification/dry-mode toggles, generating the rules-editor link, and applying rules sent back by the user. Also receives "your message was moderated" notifications and relays them to the owner. It is responsible for bot DM conversation handling and rules-editor link generation/encoding. It manages groups at the level of id/name/toggles, but treats the moderation rules themselves as **opaque JSON** — it knows nothing about group messages or the rule-matching logic, which belong to `moderator`.

2. **`moderator`** (`domain/moderator/`) — the moderation engine: joining groups, storing/loading rules, evaluating each incoming group message against the rules, deleting violating messages, and emitting moderation notifications. The actual matching logic lives in `domain/moderator/message_filter/`. 

Each bounded context follows the same internal shape:
- `ports` — trait definitions and the domain types they exchange.
  - **Inbound ports**: (also called driving ports) how the outside world drives this context (e.g `ModerationEngine`, `GroupAdministration`, `BotDmReceiver`). Implemented by this context's applications.
  - **Outbound ports**: (also called driven ports) what this context needs from the outside (e.g `ModerationRepository`, `GroupModerator`, `BotMessenger`, `ModerationNotifier`). Implemented by adapters in `infrastructure/`.
- `application` — the use-case implementations (`MessageModerationApplication`, `GroupAdministrationApplication`, `BotDmApplication`). Depend only on ports, never on concrete adapters.
- additional pure-domain submodules (e.g. `message_filter/`).

`bot_dm` keeps both as single files (`ports.rs`, `application.rs`). `moderator` has outgrown that and splits each into a module, **one inbound port and one application per use case**:
- `ports/types.rs` — the value types both sides exchange; `ports/inbound.rs` — one trait per use case; `ports/outbound.rs` — what the context needs from outside. `ports.rs` re-exports all three, so callers keep writing `domain::moderator::ports::X` no matter which file `X` lives in.
- `application/moderation.rs` — `MessageModerationApplication`, implementing `ModerationEngine` (moderate one incoming group message); `application/groups.rs` — `GroupAdministrationApplication`, implementing `GroupAdministration` (join/remove/list groups, read and save rules, notification and dry-mode toggles); `application/member_restore.rs` — `MemberRestoreApplication`, implementing `MemberRestoreRunner` (restore the members whose timed observer restriction has expired). `application.rs` only declares and re-exports them, and its own `application/tests.rs` holds the port fakes shared by the use-case test modules below it (a fake used by one of them lives in that module's `tests.rs` instead).
- The restore runner is driven by a clock, not by a messenger event: `bin/bot.rs` ticks it once a minute and passes `Utc::now()` in, so the domain never reads a clock of its own — the same way `GroupMessage.timestamp` is the "now" of the moderation path.

### Infrastructure (`infrastructure/`)
- `adapters/` — implementations of **outbound ports** (driven adapters) and the **cross-context routers**:
  - `moderator_repo_sqlite.rs` + `moderator_repo_sqlite_rules.rs` — SQLite
    persistence for the moderator context (`ModerationRepository`).
  - `simplex_adapter.rs` — implements messenger/group actions on top of the
    SimpleX driver (`BotMessenger`, `GroupModerator`).
  - `cross_domain_router.rs` — lets `bot_dm` call into `moderator` by
    implementing `bot_dm::GroupOperations` on top of `moderator::GroupAdministration`.
  - `moderation_notification_router.rs` — lets `moderator` notify `bot_dm` by
    implementing `moderator::ModerationNotifier` on top of
    `bot_dm::ModerationNotificationReceiver`. The receiver is injected *after*
    construction (`set_receiver`) to break the wiring cycle between the contexts.
- `drivers/` — low-level clients for external systems. `drivers/simplex/` is the
  SimpleX Chat websocket client that produces `SimplexEvent`s and exposes raw
  operations. Drivers know nothing about domain types.
- `migrations/` — sequential SQL schema migrations (see DB section below).

### Where do I put new code? (decision table)
| You are adding… | Put it in… |
|-----------------|------------|
| New business rule / decision logic | `domain/<context>/` (pure, no I/O)|
| A new capability the domain needs from outside | a new **outbound port** trait in that context's ports (`domain/moderator/ports/outbound.rs`, `domain/bot_dm/ports.rs`) |
| A new way the outside drives the domain | a new **inbound port** trait in that context's ports (`domain/moderator/ports/inbound.rs`, `domain/bot_dm/ports.rs`) + its own application module |
| DB query / persistence | `infrastructure/adapters/` (a repo adapter) |
| Talking to SimpleX or another external system | `infrastructure/drivers/` (+ a thin adapter) |
| Letting one bounded context call another | a router in `infrastructure/adapters/` |
| Schema change | a new file in `infrastructure/migrations/` |
| Constructing/wiring concrete types | `bin/bot.rs` only |

### Hard rules
- `domain/` must never `use` anything from `infrastructure/`. Dependencies point inward only.
- The two bounded contexts must not import each other's types directly in domain code. They exchange data only through the cross-domain **routers**, which translate between the two contexts' own types.
- Application services depend on **ports (traits)**, not concrete adapters.
- Each context defines its own error alias `type Err = Box<dyn Error + Send + Sync>` and its own copies of shared value types (`Group`, `GroupInvitation`, etc.). Routers convert between them explicitly — do not "share" a type across contexts.

## Database principles
Persistence is **infrastructure**. It lives in two places only:
- `infrastructure/migrations/*.sql` — schema definition (DDL) and data backfills.
- `infrastructure/adapters/moderator_repo_sqlite*.rs` — all queries (DML). No SQL anywhere else; the domain never sees SQLite.

### Migrations
- Files are named `NNNN_description.sql` (zero-padded, sequential). They are embedded at build time and applied in **filename sort order**.
- The applied version is tracked in SQLite's `PRAGMA user_version`; the runner (`infrastructure/migrations.rs`) skips already-applied files and runs each remaining file in its own transaction, then bumps `user_version`. The version is the file's 1-based position in the sorted list, so **never reorder, rename, renumber, or delete an existing migration file.**
- `PRAGMA foreign_keys = ON` is set for the connection, so `ON DELETE CASCADE` and FK constraints are enforced during migrations and at runtime.
- **Immutability rule:** once a migration has been merged to `master` it is **released** (see Deployment — every commit to `master` auto-deploys to production, so a merged migration has likely already run against the production database). Treat released migrations as frozen and add a **new** numbered migration to change the schema. Editing a migration in place is only acceptable while it is still unreleased (not yet on `master`) and has not run against any real database.
- When restructuring tables, include a data **backfill** in the same migration and drop old tables **children before parents** so FK enforcement stays satisfied.

### Schema conventions (moderation rules)
The whole moderation schema is **one tree rooted at `moderation_groups`**, and every foreign key points *up* at its parent with `ON DELETE CASCADE`:

```
moderation_groups
  └── moderation_rules            (group_id, rank)
        ├── moderation_conditions (rule_id, parent_id, rank, type)
        │     └── moderation_condition__<name>[__<subtable>] (condition_id)
        └── moderation_actions    (rule_id, rank, type)
              └── moderation_action__<name> (action_id)
```

Beside the rules hangs the bot's own runtime state, a direct child of the group rather than of a rule: `moderation_set_author_observer_restores(id, group_id, member_id, execute_at)`, one row per member a timed `SetAuthorObserver` is still holding. It is deliberately *not* anchored to the action that scheduled it — saving rules deletes and reinserts every `moderation_rules` row of a group, which would cascade the obligation away and strand that member as an observer forever. Anything else the bot owes a member later belongs at this level too, for the same reason.

The invariant that follows is worth protecting: `DELETE FROM moderation_groups WHERE group_id = ?` removes every row belonging to that group, and **no adapter deletes anything by hand**. If a change makes some table reachable only through a link pointing the other way, that invariant breaks silently — a test in `infrastructure/migrations.rs` enumerates `moderation%` tables from `sqlite_master` after deleting a group and asserts all are empty, so add tables inside this tree, not beside it.

- **Rules**: `moderation_rules(id, group_id, rank)`. A rule is nothing but a rank and an owner; its condition tree and its actions hang off it. `rank` stores the order the rules were supplied by the user/editor (the slice index on write), and is unique within a group.
- **Conditions**: a rule owns a *tree* of conditions in `moderation_conditions(id, rule_id, parent_id, rank, type)`. `type` is the PascalCase serde variant name. The root of a rule's tree is its node with `parent_id IS NULL`; `rank` orders siblings within a composite. `rule_id` is denormalized onto every node, not just the root, so a whole tree loads with one indexed query and cascade-deletes flat regardless of depth — the writer maintains the "equals my parent's rule_id" invariant.
- **Condition settings**: `moderation_condition__<name>` keyed by `condition_id` (FK → `moderation_conditions(id) ON DELETE CASCADE`), with one column per setting. A condition whose only parameters are a list gets **no settings table**, just its subtable: unlike actions, the discriminator already lives in the registry row, so an empty table would buy no uniformity. For the same reason a condition with no parameters at all (`IsBlank`, `ContainsInvisibleCharacters`) and the composites (`All`, `Any`, `Not`) have no tables.
- **Condition lists**: `moderation_condition__<name>__<subtable>` keyed by `condition_id`, every segment joined by a **double underscore** so a subtable suffix (`moderation_condition__contains_links_outside_list__domains`) is never confused with a different condition type whose name merely contains an underscore (`moderation_condition__contains_links_outside_top100`).
- A group may have **multiple rules using the same condition type**, and one rule may use the same condition type several times in different branches.
- Surrogate `id`s are internal only — they are not exposed to users, so a writer may let SQLite assign them (`last_insert_rowid()`) instead of generating them.

Moderation **actions** live in their own tables and are children of the rule:

- Registry: `moderation_actions(id, rule_id, rank, type)`, where `type` is the PascalCase action variant name (e.g. `ModerateMessage`, `KickAuthor`). A rule owns **several** actions; `rank` orders them within the rule and is unique there (`idx_moderation_actions_rule_id_rank`), mirroring `moderation_rules.rank` within a group. `rank` is the slice index on write, which is the execution order `action_planner` put the list in — see the action section below.
- Per-type settings: `moderation_action__<name>` (e.g. `moderation_action__kick_author`) keyed by `action_id` (FK → `moderation_actions(id) ON DELETE CASCADE`), holding that action's **settings** columns. A zero-setting action (e.g. `moderation_action__moderate_message`) still gets a table so the reference stays uniform.
- `rule_id` is nullable purely because `ALTER TABLE ... ADD COLUMN` with a `REFERENCES` clause cannot be `NOT NULL` in SQLite. Do not "fix" this by rebuilding the table: with foreign keys enabled, `DROP TABLE` runs an implicit `DELETE FROM` that fires the `ON DELETE CASCADE` on every `moderation_action__<name>` row, and the pragma cannot be toggled inside the migration runner's transaction.

## Adding a new moderation rule (condition) type (common task — do all of these)
A rule is a `ModerationRule { actions: Vec<ModerationAction>, condition: ModerationCondition }` (each in its own module under `domain/moderator/message_filter/`); it serializes to `{ "actions": [{ "type": ... }], "condition": { "type": ... } }` — an array of `#[serde(tag = "type")]` action objects next to one `#[serde(tag = "type")]` condition (condition is **not** flattened). `ModerationCondition` is the **single source of truth** for detection types: a `#[serde(tag = "type")]` enum whose struct-like variants carry the condition's parameters (e.g. `ContainsWords { keywords: Vec<String> }`). Conditions are **pure logical predicates** (e.g. `ContainsWords`, `ContainsLinksInList`, `MatchesExactMessage`), describing what message pattern triggers the rule, not the action to take. Each condition appears under **two naming forms that must stay aligned**: the PascalCase variant name is the serde `type` tag used in the URL hash, `rules-schema.json` and the `moderation_conditions.type` column (e.g. `ContainsWords`), while its snake_case form is the `<name>` used for DB table names (e.g. `moderation_condition__contains_words__keywords`).

A rule's condition is a **tree**, not a single predicate: besides the leaves there are three composite variants, `All`, `Any` and `Not`, which carry other conditions. Anything that asks "does this rule use condition X" must therefore walk the tree (`ModerationCondition::walk`) rather than matching on the rule's root condition, or it will silently miss nested uses — `max_message_rate_limit_window` and friends exist for exactly this reason. Note that the rule list is itself a disjunction, so `Any` only adds expressiveness when nested.

- **Name a condition after what it detects, never after what the owner thinks of it.** Because any condition can sit under a `Not`, a judgement baked into the name reads backwards there ("not: contains banned words"). So `ContainsWords`, not `ContainsBannedWords`; `ContainsLinksInList` / `ContainsLinksOutsideList`, not "forbidden" / "approved" websites; `ExceedsMaxLines`, not "floods chat". Prefer precise words to relative ones, which say nothing about the threshold: `AuthorHitsMessageRateLimit`, not `AuthorSentManyMessages`. This applies to every place the condition surfaces: variant and field names, table names, the editor title and field titles, `describe()`, and the reason string. Prefer one condition per check over a bundle of checks behind checkboxes — the composites already combine them, and a bundle turns into an unreadable "none of these" under a `Not`.
- **Emoji convention for condition titles:** the emoji names the *subject* of the check, not a verdict on it.
  - 💬 message text (e.g. `💬 Message Contains Any of These Words`, `💬 Message Matches Any of These Regex Patterns`).
  - 🔗 links (e.g. `🔗 Message Contains a Link to Any of These Websites`, `🔗 Message Contains a Link Outside Top 100 Websites`).
  - 📏 the message's shape and size (e.g. `📏 Message Is Empty or Blank`, `📏 Message Exceeds Max Lines`).
  - 👤 the author (e.g. `👤 Author Hits Message Rate Limit`, `👤 Author Joined Recently`).
  - The composites are not detectors and take a shape of their own (`🧩 All of These Conditions Match (AND)`, `🔀 Any …`, `✖️ This Condition Does Not Match (NOT)`).

1. **Domain:** add a variant to `ModerationCondition` (`message_filter/moderation_condition.rs`), implement its matching in `should_moderate_by_condition` (or in `evaluate`, if it needs the repositories on `ConditionContext`), give it an arm in `normalize_and_validate_leaf` so its parameters are checked when an owner saves the rule, and a line in `describe` so a `Not` around it can explain itself. Add a submodule under `moderation_condition/` if the matching logic is non-trivial; keep it pure and unit-tested.
2. **Migration:** add `infrastructure/migrations/NNNN_*.sql`. A condition with scalar settings gets `moderation_condition__<name>` keyed by `condition_id`; a condition whose only parameter is a list gets just `moderation_condition__<name>__<subtable>`; a condition with no parameters needs no migration. No `group_id`, `rank` or `action_id` columns — those live on `moderation_rules` and `moderation_conditions`. Follow the schema conventions above.
3. **Repository read:** load the new tables into `ConditionData` in `infrastructure/adapters/moderator_repo_sqlite_rules.rs` (one group-wide query per table, via `load_condition_lists` / `load_condition_settings`) and add an arm to `build_condition` keyed on the type tag.
4. **Repository write:** add an arm to `insert_condition` in `infrastructure/adapters/moderator_repo_sqlite.rs` that writes the settings row and/or list rows against `condition_id`.
5. **Web editor:** add the condition's shape to `webeditor/rules-schema.json` as a new `oneOf` entry under `definitions.condition`, using the variant name as the `type` const so it matches the serde representation, and give it `options.summary` and `options.phrase` (see *The web editor is schema-driven*). Title, description, field types, defaults and limits all come from that entry — **do not touch `editor.js`**. The actions are a sibling of condition on every rule (`items.properties.actions`), so no per-condition action wiring is needed.
6. **Bug template:** Update `moderation-rule-bug.yml` to add the new rule's title (as it appears in `rules-schema.json`) to the `rule-type` dropdown options list so bug reporters can select it.

### Renaming or replacing a condition
Stored rules are the only thing that has to survive: the bot always sends the owner a freshly generated editor link, so old type tags never come back through a link. Do **not** add `#[serde(alias = ...)]`s or compatibility deserializers for old shapes. Instead, add a migration that rewrites the stored rows — update `moderation_conditions.type` and rename the condition's tables (`ALTER TABLE ... RENAME TO`) for a rename, or rebuild the affected nodes for a condition replaced by differently shaped ones. Precedent: `0025_neutral_condition_names.sql`.

## Adding a new moderation action type (do all of these)
`ModerationAction` (in `domain/moderator/message_filter/moderation_action.rs`) is the **single source of truth** for actions: a `#[serde(tag = "type")]` enum (`ModerateMessage`, `SetAuthorObserver { duration_minutes: u32 }`, `KickAuthor { delete_all_messages: bool }`). Like conditions, the PascalCase variant name is the serde `type` tag (URL hash, `rules-schema.json`, and the `moderation_actions.type` column), and its snake_case form is the `<name>` used for the `moderation_action__<name>` settings table.

A rule carries a **list** of actions, and every variant must stay one indivisible thing the bot can do. Do **not** add a setting to an action that means "and also do that other action" (a kick used to carry "...and moderate the message"): the owner combines actions by listing them, and a bundled sub-action cannot be deduplicated against the same action coming from another rule. The list is a **set**, not a sequence: `action_planner` drops actions a stronger one already covers (`covers`) and orders the rest by `execution_rank` — observer, then moderate, then kick, so the author is kicked last. A new variant therefore needs an arm in both of those, plus the same merge applied to a single rule's list by `ModerationRule::normalize_and_validate` when an owner saves it.

1. **Domain:** add a variant to `ModerationAction`, give it an arm in `ModerationAction::normalize_and_validate` so its own parameters are checked when an owner saves the rule (the arms are listed one by one, with no catch-all, so a new variant cannot skip validation), and execute it in `MessageModerationApplication::process_group_message` (`domain/moderator/application/moderation.rs`) via the `GroupModerator` outbound port. If the action needs a capability the messenger doesn't expose yet, add a method to `GroupModerator` in `domain/moderator/ports/outbound.rs` and implement it in `infrastructure/adapters/simplex_adapter.rs` (+ the `drivers/simplex` driver).
2. **Cross-context (owner notifications):** mirror the variant in `bot_dm::ModerationAction` (`domain/bot_dm/ports.rs`), map it in `infrastructure/adapters/moderation_notification_router.rs`, and render its notification text in `domain/bot_dm/application.rs`.
3. **Migration:** add `infrastructure/migrations/NNNN_*.sql` creating `moderation_action__<name>` (keyed by `action_id` FK → `moderation_actions(id) ON DELETE CASCADE`) with one column per setting.
4. **Repository read/write:** resolve the new action in `load_actions` (one group-wide query, keyed by `rule_id` and ordered by `rank`) and persist it in `insert_actions` (both in the `moderator_repo_sqlite*` adapters).
5. **Web editor:** add the action as a new `oneOf` entry under `definitions.action` in `webeditor/rules-schema.json`, using the variant name as the `type` const and one field per setting, plus `options.execution_rank` (matching `execution_rank` in `action_planner`), `options.covered_by` (the actions whose presence makes this one redundant, matching `covers`) and `options.summary`. `definitions.action` describes **one** action; the rule's `actions` array references it. No change to `editor.js`.
6. **Bug template:** add the action's title to the `rule-action` dropdown in `moderation-rule-bug.yml`.

## General conventions
- Async traits use `#[async_trait]`.
- Convert errors and types **at boundaries** (adapters/routers), not inside the domain.
- Keep tests in separate files (e.g. `#[cfg(test)] mod tests;` pointing to `<module_name>/tests.rs` or `<module_dir>/tests.rs`), never inline in implementation files.
- Keep changes minimal and within the established structure; if a change seems to require breaking one of the hard rules above, stop and ask rather than working around it.

## Deployment
Every commit to `master` is the release: CI/CD builds and deploys the **bot** to production and publishes the `webeditor/` folder to **GitHub Pages**. There is no separate release step — merging to `master` ships to production. This is why a migration counts as released (and therefore frozen) the moment it lands on `master`.
