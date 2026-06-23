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
4. **Edit rules (the URL-hash hack).** The owner opens the link; `webeditor/index.html` decodes the rules from the hash and renders an editor (UI driven by `rules-schema.json`). After editing, they click **Apply Changes**: JavaScript writes the updated rules back into the URL hash and copies the full URL to the clipboard, then a popup tells the owner to paste it into the bot chat and send it. The bot receives that plain text, parses the rules out of the hash, and saves them.

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
- `ports.rs` — trait definitions and the domain types they exchange.
  - **Inbound ports**: (also called driving ports) how the outside world drives this context (e.g `ModerationEngine`, `BotDmReceiver`). Implemented by this context's application.
  - **Outbound ports**: (also called driven ports) what this context needs from the outside (e.g `ModerationRepository`, `GroupModerator`, `BotMessenger`, `ModerationNotifier`). Implemented by adapters in `infrastructure/`.
- `application.rs` — the use-case implementation (`ModeratorApplication`, `BotDmApplication`). Depends only on ports, never on concrete adapters.
- additional pure-domain submodules (e.g. `message_filter/`).

### Infrastructure (`infrastructure/`)
- `adapters/` — implementations of **outbound ports** (driven adapters) and the **cross-context routers**:
  - `moderator_repo_sqlite.rs` + `moderator_repo_sqlite_rules.rs` — SQLite
    persistence for the moderator context (`ModerationRepository`).
  - `simplex_adapter.rs` — implements messenger/group actions on top of the
    SimpleX driver (`BotMessenger`, `GroupModerator`).
  - `cross_domain_router.rs` — lets `bot_dm` call into `moderator` by
    implementing `bot_dm::GroupOperations` on top of `moderator::ModerationEngine`.
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
| A new capability the domain needs from outside | a new **outbound port** trait in `domain/<context>/ports.rs` |
| A new way the outside drives the domain | a new **inbound port** trait in `domain/<context>/ports.rs` |
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
Rules are owned by groups and stored as one **typed table per rule type** rather than a generic key/value registry:

- Parent rule table: `moderation_rule__<name>` with `id INTEGER PRIMARY KEY`, a `group_id` column (FK → `moderation_groups(group_id) ON DELETE CASCADE`, and indexed), a `rank INTEGER NOT NULL` column, plus any per-rule **settings** columns for that rule type.
- Optional child/detail table (lists, etc.): `moderation_rule__<name>__<subtable>` — every segment is joined by a **double underscore**, so the subtable suffix (`moderation_rule__links_whitelist__domains`, a subtable of `links_whitelist`) is never confused with `moderation_rule__links_whitelist_top100` (a different rule type, whose name simply contains a single underscore). Child tables are keyed by `rule_id` (FK → the parent's `id` `ON DELETE CASCADE`).
- A group may have **multiple rules of the same type** (each parent row has its own `id`).
- **Ordering:** `rank` stores the order the rules were supplied by the user/editor (the slice index on write). The reader merges all rule tables and sorts by `rank` (then `id` as a tiebreaker) so the original order round-trips.
- Surrogate `id`s are internal only — they are not exposed to users, so a writer may let SQLite assign them (`last_insert_rowid()`) instead of generating them.

## Adding a new moderation rule type (common task — do all of these)
`ModerationRule` (in `domain/moderator/message_filter.rs`) is the **single source of truth**: a `#[serde(tag = "type")]` enum whose struct-like variants carry the rule's parameters (e.g. `WordsBlacklist { keywords: Vec<String> }`). Each rule appears under **two naming forms that must stay aligned**: the PascalCase variant name is the serde `type` tag used in the URL hash and `rules-schema.json` (e.g. `WordsBlacklist`), while its snake_case form is the `<name>` used for DB table names (e.g. `moderation_rule__words_blacklist`).

1. **Domain:** add a variant to `ModerationRule` and implement its matching in `should_moderate_by_rule` (add a submodule under `message_filter/` if the logic is non-trivial; keep it pure and unit-tested). Evaluation short-circuits on the first rule that matches.
2. **Migration:** add `infrastructure/migrations/NNNN_*.sql` with the new `moderation_rule__<name>` table (and `__<subtable>` if it has list/detail data), following the schema conventions above.
3. **Repository read:** load the new rule in `infrastructure/adapters/moderator_repo_sqlite_rules.rs` (carry its `rank`).
4. **Repository write:** insert the new rule (with `rank`) in `set_group_rules` in `infrastructure/adapters/moderator_repo_sqlite.rs`.
5. **Web editor:** add the rule's shape to `webeditor/rules-schema.json` so owners can configure it, using the variant name as the `type` const so it matches the serde representation.
6. **Bug template:** Update `moderation-rule-bug.yml` to add the new rule's title (as it appears in `rules-schema.json`) to the `rule-type` dropdown options list so bug reporters can select it.

## General conventions
- Async traits use `#[async_trait]`.
- Convert errors and types **at boundaries** (adapters/routers), not inside the domain.
- Keep changes minimal and within the established structure; if a change seems to require breaking one of the hard rules above, stop and ask rather than working around it.

## Deployment
Every commit to `master` is the release: CI/CD builds and deploys the **bot** to production and publishes the `webeditor/` folder to **GitHub Pages**. There is no separate release step — merging to `master` ships to production. This is why a migration counts as released (and therefore frozen) the moment it lands on `master`.
