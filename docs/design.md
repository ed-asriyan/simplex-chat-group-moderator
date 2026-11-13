# Design
How the bot and its web editor are built: the architectural approach and the rules it sets, the components and what each is responsible for, and where code goes. What they do is in [`user-stories.md`](user-stories.md); the interface of each bounded context is its ports, in code, and the database schema is *Schema* below. This is a living document: it describes the current design only.

## Overview
A moderation bot for [SimpleX Chat](https://simplex.chat) groups, written in Rust (the crate is `bot/`), and a static web editor for its rules (`webeditor/`, published to GitHub Pages). An owner talks to the bot in direct messages; the bot moderates the groups it has joined. A group's whole configuration — its mode and its rules — travels between the bot and the editor as one JSON object compressed into the hash of an editor link, because SimpleX carries only text.

### Approach: hexagonal architecture with two bounded contexts
The crate is split into two top-level layers (`bot/src/lib.rs`):
- `domain/` — pure business logic. **No I/O, no SQL, no network, no external crates for side effects.** Defines *ports* (traits) and implements *use cases*.
- `infrastructure/` — everything that talks to the outside world. Implements the domain's ports.

The composition root is `bot/src/bin/bot.rs` — it constructs every concrete adapter, wires them to the domain applications via `Arc<dyn Trait>`, and runs the SimpleX event loop. **Wiring happens only here.**

Each bounded context has the same shape:
- `ports` — **inbound ports** (driving: how the outside drives the context; one trait per use case, implemented by the context's applications), **outbound ports** (driven: what it needs from outside, implemented by adapters in `infrastructure/`), and the value types they exchange. `bot_dm` keeps them in `ports.rs`; `moderator` splits them into `ports/{types,inbound,outbound}.rs`, re-exported by `ports.rs`, so callers write `domain::<context>::ports::X` whichever file `X` lives in.
- `application` — the use-case implementations, depending only on ports.
- further pure submodules for non-trivial logic (`moderator/rules/`).

The rules this sets:
- `domain/` must never `use` anything from `infrastructure/`. Dependencies point inward only.
- The two bounded contexts must not import each other's types directly in domain code. They exchange data only through the cross-domain **routers**, which translate between the two contexts' own types.
- Application services depend on **ports (traits)**, not concrete adapters.
- Each context defines its own error alias `type Err = Box<dyn Error + Send + Sync>` and its own copies of shared value types (`Group`, `GroupInvitation`, etc.). Routers convert between them explicitly — do not "share" a type across contexts.
- Inside `domain/moderator/rules/`, a condition or action is named only by its own module and its registry lines — the leaf registry (`conditions!`, `actions!`) and the list of each trigger that takes it (`rules/trigger/<trigger>.rs`) — never by another leaf, the engine, `rules/common/`, the application or the ports. A leaf reaches only its registry (as `super`), the ports and `rules/common/`; code two leaves share moves to `common` rather than being imported from one into the other. Infrastructure may name them: the repositories store them and the notification router translates them.
- These rules are checked on the source: `domain/architecture_tests.rs` (no infrastructure, no other context, only the listed crates, no I/O and no clock) and `domain/moderator/rules/architecture_tests.rs` (the isolation above). Do not loosen them to make a change pass — a failure means the code is in the wrong place.

### Time
The domain never reads a clock. The moderation path runs at the message's own time (`GroupMessage.timestamp`), a join at the time of the join (`MemberJoin.timestamp`); the member restore runner is driven by a tick in `bin/bot.rs`, once a minute, which passes `Utc::now()` in.

### Persistence
Persistence is **infrastructure**. It lives in two places only:
- `infrastructure/migrations/*.sql` — schema definition (DDL) and data backfills.
- `infrastructure/adapters/moderator_repo_sqlite*.rs` — all queries (DML). No SQL anywhere else; the domain never sees SQLite.

The migrations' file and version rules are in `AGENTS.md` (*Migrations*); the whole schema they produce is *Schema* below.

#### Schema conventions
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

- **Rules**: `moderation_rules(id, group_id, rank, name, trigger, enabled)`. A rule is a rank, an owner, the owner's name for it (`NULL` for none), what makes the bot check it (`trigger`: `message_posted` or `member_joined`, `Trigger::name`) and whether it is on; its condition tree and its actions hang off it. `rank` stores the order the rules were supplied by the user/editor (the slice index on write), and is unique within a group: one order across both triggers, each trigger's rules keeping theirs. A rule for joins may have no condition, and then has no row in `moderation_conditions`.
- **Conditions**: a rule owns a *tree* of conditions in `moderation_conditions(id, rule_id, parent_id, rank, type)`. `type` is the PascalCase serde variant name. The root of a rule's tree is its node with `parent_id IS NULL`; `rank` orders siblings within a composite. `rule_id` is denormalized onto every node, not just the root, so a whole tree loads with one indexed query and cascade-deletes flat regardless of depth — the writer maintains the "equals my parent's rule_id" invariant.
- **Condition settings**: `moderation_condition__<name>` keyed by `condition_id` (FK → `moderation_conditions(id) ON DELETE CASCADE`), with one column per setting. A condition whose only parameters are a list gets **no settings table**, just its subtable: unlike actions, the discriminator already lives in the registry row, so an empty table would buy no uniformity. For the same reason a condition with no parameters at all (`IsBlank`, `ContainsInvisibleCharacters`) and the composites (`All`, `Any`, `Not`) have no tables.
- **Condition lists**: `moderation_condition__<name>__<subtable>` keyed by `condition_id`, every segment joined by a **double underscore** so a subtable suffix (`moderation_condition__contains_links_outside_list__domains`) is never confused with a different condition type whose name merely contains an underscore (`moderation_condition__contains_links_outside_top100`).
- A group may have **multiple rules using the same condition type**, and one rule may use the same condition type several times in different branches.
- Surrogate `id`s are internal only — they are not exposed to users, so a writer may let SQLite assign them (`last_insert_rowid()`) instead of generating them.

Moderation **actions** live in their own tables and are children of the rule:

- Registry: `moderation_actions(id, rule_id, rank, type)`, where `type` is the PascalCase action variant name (e.g. `ModerateMessage`, `KickAuthor`). A rule owns **several** actions; `rank` orders them within the rule and is unique there (`idx_moderation_actions_rule_id_rank`), mirroring `moderation_rules.rank` within a group. `rank` is the slice index on write, which is the execution order `action_planner` put the list in — see the action section below.
- Per-type settings: `moderation_action__<name>` (e.g. `moderation_action__kick_author`) keyed by `action_id` (FK → `moderation_actions(id) ON DELETE CASCADE`), holding that action's **settings** columns. A zero-setting action (e.g. `moderation_action__moderate_message`) still gets a table so the reference stays uniform.
- `rule_id` is nullable purely because `ALTER TABLE ... ADD COLUMN` with a `REFERENCES` clause cannot be `NOT NULL` in SQLite. Do not "fix" this by rebuilding the table: with foreign keys enabled, `DROP TABLE` runs an implicit `DELETE FROM` that fires the `ON DELETE CASCADE` on every `moderation_action__<name>` row, and the pragma cannot be toggled inside the migration runner's transaction.

## Components

### `bot_dm` — the conversation with owners
**Responsible for:**
- what a direct message means: `/start`, `/help`, `/groups`, `/source`, `/issue`, `/feature`, and an editor link carrying a group's configuration;
- the editor link: a group's configuration JSON, lz-compressed into the hash with the group's id and name (`#bot_id=…&group=…&config=…`), and back;
- deciding whether to join a group the bot is invited to (moderator role or higher);
- every text the bot sends to an owner, the moderation notifications included.

**Not responsible for:** group messages, rules and their matching, what a mode does — a group's configuration is **opaque JSON** to it in both directions; it knows groups only by id and name.

**Interacts with:**
- driven by the SimpleX event loop through `BotDmReceiver` (a direct message, a group invitation), and by `moderator` through `ModerationNotificationReceiver` (via `moderation_notification_router.rs`), about a message or about a join alike;
- drives `moderator` through `GroupOperations` (`cross_domain_router.rs`, over `GroupAdministration`);
- needs `BotMessenger` (the SimpleX adapter).

**Owns no table.**

### `moderator` — groups, rules and moderation
**Responsible for:**
- joining and forgetting groups, and their owners;
- a group's configuration (`GroupConfig`: its mode and its rules), validated and saved as one, with the owner's AI keys checked first;
- checking every member's message, and everyone who joins as a member or an observer, against the group's rules for that trigger that are on, carrying out the matched actions as the mode says, and telling the owner through `ModerationNotifier`;
- restoring members whose timed observer restriction is up.

**Not responsible for:** direct messages, wording, the editor link.

**Interacts with:**
- driven by the SimpleX event loop through `ModerationEngine` (a group message, someone joined) and `GroupAdministration::remove_group` (the bot removed from a group), by `bot_dm` through `GroupAdministration` (via the router), and by the tick through `MemberRestoreRunner`;
- needs `ModerationRepository` and `MemberRestoreRepository` (SQLite), `GroupModerator` (the SimpleX adapter), `ModerationNotifier` (the router into `bot_dm`), the activity and history repositories (memory), and `OpenAi` and `OpenRouter` (the AI gateway).

**Owns the tables:** all of them, the tree rooted at `moderation_groups` (*Persistence*).

It splits its applications per use case: `application/moderation.rs` (`MessageModerationApplication`, `ModerationEngine`: messages and joins), `application/groups.rs` (`GroupAdministrationApplication`, `GroupAdministration`), `application/member_restore.rs` (`MemberRestoreApplication`, `MemberRestoreRunner`); `application.rs` only declares and re-exports them, and its `application/tests.rs` holds the port fakes the use-case tests share (a fake one of them alone uses lives in its own `tests.rs`).

#### Inside `moderator/rules/`: an engine that knows no condition or action
`rules/` is an engine, its leaves and its triggers.

The **leaves** are the conditions (`rules/moderation_condition/<name>.rs`) and the actions (`rules/moderation_action/<name>.rs`), each a module of its own: the type with its settings, its trait impls, its tests, and any algorithm only it uses. A leaf belongs to no trigger. It has two traits: what it is whatever it is checked on (`Condition`: settings, `describe`, `needs`, `depends_on_other_rules`, `key_uses`; `Action`: settings, `effect`, `execution_rank`), and how it is checked or carried out on a subject (`ConditionOn<S>::should_moderate`, `ActionOn<S>::execute`). A leaf that reads the message implements the second for `GroupMessage` only; one that needs only the member — `MemberNameContainsWords`, `KickAuthor` — implements it for every subject that is a `common::member::Member`. The leaf registries (`conditions!`, `actions!`) list every leaf once and build `ModerationCondition` and `ModerationAction`: a condition or an action **as an owner writes it**, in the JSON, the editor and the database, any of them anywhere.

The **triggers** (`rules/trigger.rs`, `Trigger`) are not leaves: each is driven by its own event (a method of `ModerationEngine`) and told to the owner in its own words, so a new one is never a module and a registry line. Each has a module, `rules/trigger/<trigger>.rs`, that says what its rules are checked on — `message_posted` on the `GroupMessage`, `member_joined` on the `MemberJoin` — and lists the leaves it takes (`trigger_conditions!`, `trigger_actions!`). The lists build the trigger's own types — `MessageCondition`/`MessageAction`, `JoinCondition`/`JoinAction`: the composites `All`, `Any` and `Not` and the listed leaves only —, which convert from `ModerationCondition`/`ModerationAction` by `TryFrom`, refusing a leaf the trigger does not take (US-C2), and back. A trigger lists only leaves checkable on its subject, or the code does not compile: `ModerateMessage` cannot be in the join list.

A **rule** (`moderation_rule.rs`) exists in the same two forms. `RuleDraft` is the rule as written: `name`, `trigger`, `enabled`, an optional `ModerationCondition`, `ModerationAction`s. `ModerationRule` is the rule as kept and checked: `name`, `enabled` and `TriggeredRule`, the trigger with its condition and actions in that trigger's types — a message rule's condition required, a join rule's optional. A draft becomes a rule by `TryFrom`, which refuses what cannot be one; a `ModerationRule` serializes as its draft, so that refusal is the reason an owner reads for a configuration that does not parse.

The **engine** is everything else, generic over a trigger's types (`TriggerCondition`, `TriggerAction`) and naming no leaf and no trigger:
- `rules.rs` — `should_moderate`: the loop over one trigger's rules that are on, with the pre-pass for the conditions that `depends_on_other_rules()`, and its result `ModerationMatch`;
- `moderation_condition/tree.rs` — the composites: normalization and the tree's limits;
- `bookkeeping.rs` — records about each message what the message rules' merged `needs()` ask for;
- `action_planner/` — merges the matched rules' actions by what each `effect()` achieves and orders them by `execution_rank()`;
- `moderation_action.rs` — `execute_actions`, which carries out the plan on the subject.

`rules/common/` holds what two leaves share (rate-limit windows, domain matching, screen lines, API key checks, the `Member` a subject concerns); it knows no leaf and nothing of the engine. The application builds `ConditionPorts` and `ActionPorts` from what it was wired with and calls the engine, so it never names a condition or an action: in the domain, adding one touches its own module, its registry line and the list of each trigger that takes it, nothing else.

### `webeditor` — the rules editor
**Responsible for:** showing and editing a group's configuration in the browser, from the link's hash; the review of the changes, and the link that carries them back; editing with an AI chat the owner already uses; JSON import and export.

**Not responsible for:** storing anything, talking to the bot, the final say on validity — the bot validates whatever it receives (US-C2); the editor's checks only catch mistakes early.

**Interacts with:** the bot only through the owner, by the link (`#bot_id=…&group=…&config=…`; `group` only names the group on the page); `rules-schema.json` is its contract with `moderator`'s serde types.

#### The web editor is schema-driven
`webeditor/` is five static files published to GitHub Pages: `index.html` (a shell with the dialogs), `editor.css`, `editor.js` (the schema-driven renderer), `ai.js` (the AI handoff; see below) and `transfer.js` (JSON import/export; see below). Their only external dependency is lz-string. **`editor.js` knows no condition, action, trigger or group setting by name.** The schema root is the group's configuration: an object whose `rules` property is the rule list and whose every other property is a **group setting** (today only `mode`), rendered on the overview above the rules with the same controls as a parameter — so a new setting is a root property in the schema (plus the field on `GroupConfig`, a column and the repository), and no editor code.

A rule's own fields are the properties of `rules.items`: `name`, `enabled`, `condition`, `actions` and `trigger`, whose `oneOf` of `const`s lists the triggers. Each trigger is a section of the overview and the *When* of a rule's page, worded by the `options` on its branch: `section` (the section's heading), `new_rule` (New rule's heading), `empty` (an empty section), `incompatible` (the tag on a condition or action the trigger does not take), `prompt` (the trigger in the AI prompt), `incomplete` (what a rule that cannot be saved yet misses), and, for a trigger whose rules may have no condition, `condition_optional` and `everyone` (what such a rule's line reads). A condition or an action says which triggers it takes in `options.triggers`, mirroring the triggers whose lists name it (`rules/trigger/<trigger>.rs`): the type picker offers only those, and one that sits in a rule of another trigger is tagged with `incompatible`. So a new trigger is a `oneOf` branch (plus its `Trigger` variant and module, and what drives it), and no editor code either.

The editor builds its registry from `rules-schema.json` at load: the entry's `title` gives the row label, its leading emoji gives the icon *and* the subject tab in the type picker (`SUBJECTS` in `editor.js`; an emoji missing there falls into "Other"); `description` becomes the hint under the field, its first sentence shown and the rest behind *More*; `properties` become the controls (`integer` → number field, `boolean` → checkbox, `array` of strings → chips, or a textarea in bulk mode past `BIG_LIST` (20) entries, `$ref` to a condition → nested rows, `string` with `format: password` → a masked field with a show button (a JSON Schema annotation; the value itself travels in the link like any other, but never into the AI prompt — see below), `string` → a text field, `string` with `format: textarea` → several lines, shown in the Apply diff by length rather than in full, `oneOf` of `const`s plus at most one integer branch → a select with a number field for the integer branch, whose `options.unit` follows the number); `default`, `minimum`/`maximum`, `maxItems`/`maxLength` and each field's own `description` are used as they are. So **a new rule type needs no frontend code** — only the `oneOf` entry described in the steps below. A type the schema does not know (a rule stored by a newer bot) renders as a read-only unknown row and is sent back untouched.

What the schema cannot express lives in optional `options` keys on the same entry, each with a fallback when absent: `summary` (the one-line value summary in the row), `phrase` (the human wording of a rule's line, mirroring `ModerationCondition::describe`), `execution_rank` and `covered_by` (action order and coverage — mirroring each action's `Action::execution_rank` and the coverage `action_planner` derives from `Action::effect`; an entry is either a type name or `{type, when}` when the coverage depends on a setting, as `KickAuthor` covers `ModerateMessage` only with `delete_all_messages` on), `label` (a short field label when the schema `title` is a whole sentence), `triggers` (above), `never_under_not` (mirrors `Condition::depends_on_other_rules`, which keeps a condition out from under a `Not`). The schema root's `options.templates` are New rule's templates (US-G15): each a `title`, a whole `rule` with its `trigger`, and the field to focus once it is added. `definitions.condition.options.max_depth` / `max_nodes` mirror `MAX_CONDITION_DEPTH` / `MAX_CONDITION_NODES`, `rules.items.properties.name.maxLength` mirrors `MAX_RULE_NAME_CHARS`, the `model` field of `FlaggedByOpenRouterInstruction` lists exactly `OPENROUTER_INSTRUCTION_MODELS` (OpenRouter slugs of models that take `temperature` 0 and a strict JSON schema — OpenRouter's `structured_outputs` — and answer without reasoning first; check a new one against `https://openrouter.ai/api/v1/models?supported_parameters=structured_outputs`), the `maximum` of its `context_messages` mirrors `MAX_OPENROUTER_CONTEXT_MESSAGES` (and the column's `CHECK` in migration 0035), and the schema root's `options.message_max_bytes` mirrors `MESSAGE_MAX_LENGTH_IN_BYTES` from `drivers/simplex/consts.rs`. The `summary` and `phrase` templates take `{field}`, `{field|n}`, `{field|list}`, `{field|phrase(s)}` and `{field?yes:no}` (with `$` for the value).

#### Three pages
The editor is three pages, each an entry in the browser's history, so Back goes back a page: the **overview** (the group's settings and its rules, one section per trigger, each rule one line with its switch and its ⋯ menu), a **rule's page** (its name, *When*, *If* and *Then*), and **New rule** for one trigger (a template, *Create with AI*, or *Build it myself*). What applies to the whole configuration — *Apply changes*, *Edit all with AI*, import and export — is on the overview only; the other pages lead back to it.

Those keys are what the editor enforces *before* the bot does: it warns on a tree past the limits, a name too long or a rule its trigger cannot take, and refuses a pasted ruleset that breaks them. Keeping them in step with the Rust constants and the triggers' lists is part of changing those.

#### Editing is undoable, not confirmed
The editor asks nothing before a destructive edit and instead keeps the last `UNDO_LIMIT` (20) states of the configuration — they are plain JSON, so a snapshot is a clone. Every change goes through one function (`edit`), which snapshots before and records a step only when something changed, so no path can bypass the history. Undo and redo are the same move in opposite directions between two stacks (`history` / `future`); editing after an undo forks the timeline and discards `future`. Both buttons are **hidden**, not disabled, when there is nothing to undo or redo — a control that can do nothing is noise. Shortcuts: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z and Ctrl+Y, all ignored inside a text field where the browser's own undo belongs.

Undo is **scoped to the page** (US-G6): on the overview it goes back through every change; on a rule's page, only through the changes made since that page opened (`st.mark`), so it never changes a rule that is not on the screen. Moving to another page clears `future` for the same reason. The AI panel's *Undo the load* compares the top of the history with the snapshot the load pushed, so it can never quietly undo some later edit instead; JSON imports enter the same history. A snapshot is the whole configuration — settings and rules — so changing the mode is undone like any other edit.

Two edits that would lose data are made safe by their structure rather than by a prompt: changing a condition's or action's type carries over every field the new type also has under the same name and kind (`retype`), which is what keeps a whole subtree when `All` becomes `Any` and the domains when one link condition becomes another; and a container can be **unwrapped** — its children splice into the parent when it sits inside another container, so nothing is dropped.

#### Apply is the review point
`Apply changes` does not copy the link straight away. It opens a dialog with a diff of everything that changed since the page loaded — the baseline is simply the configuration decoded from the hash, which is what the bot has stored — and copies only after the owner confirms. A changed setting is one line above the rules (`Mode: Dry → Notifications`). Rules have no ids, so the diff pairs them within each trigger's section — a rule whose trigger changed is paired across sections by resemblance, and reads as changed with the section it came from —, identical rules first, uses a longest-increasing-subsequence over those to tell "moved" from "everything shifted", and matches the leftovers by resemblance (same name, same condition type or same actions) so a delete plus an unrelated insert does not read as an edit. The same pairing runs one level down over a composite's children — identical ones first, the rest by condition type, whatever is left reported as removed (in red) or added, each one line naming the condition with its settings behind the same click a changed list takes, so a dropped rule carrying 300 words stays one line — because comparing them by position turns one deleted child into a run of bogus "X → Y" lines down the rest of the list; order inside `All`/`Any` means nothing to the bot, so a pure reorder is silent. A list that *shrank* is called out in red: it is the one change that would otherwise pass unnoticed. A successful copy moves the baseline to what was just sent. The dialog also warns when the copied link approaches `options.message_max_bytes`: past it the owner's client truncates the message, the bot reads a broken hash, and the failure surfaces in the chat with nothing pointing at the cause.

#### Edit with AI (`ai.js`)
SimpleX carries text, and so does this: the editor writes a prompt, the owner pastes it into whatever AI chat they already use, and pastes the reply back. For the whole configuration, the prompt carries, and the reply must return, the same `{"mode": ..., "rules": [...]}` object the link and Export JSON carry — so settings and rules are edited together; for one rule, that rule's object, and its trigger's conditions and actions only. There is no API key, no request and no provider — the page stays static. One panel (`<dialog>`) serves three scopes: *Edit all with AI* in the overview's ⋯ menu (the whole configuration, US-G9), *Edit with AI* on a rule's page (that rule alone, which must come back with the same trigger, US-G17), and *Create with AI* on New rule (one new rule for that trigger, described first in the owner's words, US-G16). Copying the prompt is one paragraph and `Copy the prompt`, with nothing between them: the owner who reaches for this button is the one who understands least, so the only thing to read is what the button does. The action that loads the reply is in the panel's footer, beside *Cancel*.

`ai.js` names no condition or action type, and no group setting, either. The catalogue in the prompt is generated from `rules-schema.json` (the root's settings with their values and descriptions; for each type: type name, title, fields with their types and limits, and the full descriptions of both the type and its fields — that is where a model learns that matching is by whole word, or which message counts toward a rate limit), the container types and the phrasing about them are derived from which fields are `$ref`s, and the limits come from the `options` keys above. **A new rule type, or a new setting, appears in the prompt on its own.**

String lists longer than `AI_BIG_LIST` (100 — deliberately not the editor's `BIG_LIST` of 20: twenty entries is where a list needs a filter in the UI, while a model reproduces a hundred words without dropping any) are replaced by a marker — `<<KEEP LIST 1: 347 ENTRIES>>` — and put back when the reply is pasted. This keeps a 347-word blocklist from being rewritten by a model that would quietly drop half of it, keeps the prompt small, and keeps a list of slurs away from a provider that would refuse it. The marker carries an **id, not a path**, because the model may reorder or duplicate rules; it stands in for the entries *in place*, so `["<<KEEP LIST 1: 347 ENTRIES>>", "new word"]` appends. The entry count in the marker is redundant on purpose: a mismatch is the only sign that the model edited a list it could not see, and it is refused rather than guessed at. Owners who need the entries themselves tick that list in the panel and the prompt carries it in full — behind a collapsed `Long lists` summary, absent entirely when no list is abbreviated, because a checkbox asking about placeholders is noise to everyone else.

A secret field (`format: password`) is always replaced the same way, by `<<KEEP SECRET 1>>` — one id per distinct value, with no entry count because there is nothing for a model to edit — since the prompt goes to a third-party AI and a key has no business there. The model is told to copy the marker and to write `""` for a new one, never a value: a pasted reply is accepted only with the marker (put back) or an empty string, and a value the model wrote itself is refused.

Coverage is the one thing about actions a model cannot infer from the catalogue, and several have got it wrong the same way — advising an owner to add `ModerateMessage` beside a kick that already deletes the author's messages, which `action_planner` then drops. The prompt therefore carries a generated line per `covered_by` relation, and the editor's `covered` badge honours the same condition. When an action's `effect()` changes — and with it what `covers` in `action_planner` derives — `covered_by` in the schema changes with it.

A pasted reply is parsed liberally (last ```json fence, else the first balanced object or array, whichever opens first; a bare rule list is accepted, and a setting the reply leaves out keeps its current value) and validated strictly against the schema — settings and rules alike; nothing loads unless it is clean, and what does load is a draft with an Undo — the write path is still `Apply changes`.

#### Import and export JSON (`transfer.js`)
The **Import JSON** dialog accepts the group's configuration — the same `{"mode": ..., "rules": [...]}` object the link carries — pasted as JSON or loaded from a `.json` file. It validates the settings against the schema root and the rules as a pasted AI reply is validated, and loads them as an undoable draft; because the owner supplies this JSON directly, password fields may contain real values. Import does not send anything to the bot: **Apply changes** remains the review step that copies the updated editor link for the owner to send.

**Export JSON** offers copying the configuration to the clipboard or downloading `moderation-settings.json`. Its **Clear secret values (API keys)** checkbox is checked by default; when checked, password fields are exported as empty strings. Uncheck it only when the exported file or clipboard is meant to retain those secrets.

#### Why this round-trip
 SimpleX Chat only supports plain **text** messages — no apps, buttons (unlike e.g. Telegram bots). Encoding rules in a shareable URL lets owners use a real GUI editor instead of hand-writing JSON/YAML into chat, while keeping the transport a single text message that the bot can parse.

## Source layout
```
bot/
  src/
    bin/bot.rs                      the composition root: wiring, the SimpleX event loop, the ticks
    domain/bot_dm/
      ports.rs                      the ports and the value types they exchange
      application.rs                the application
    domain/moderator/
      ports.rs                      re-exports ports/
      ports/types.rs                the value types the ports exchange
      ports/inbound.rs              one trait per use case
      ports/outbound.rs             what the context needs from outside
      application.rs                declares application/<use case>.rs
    domain/moderator/rules/         the moderation engine, its leaves (conditions, actions) and triggers (trigger/)
    domain/architecture_tests.rs    the hard rules, checked on the source
    infrastructure/
      adapters/                     <what>_<technology>.rs: a driven adapter (moderator_repo_sqlite.rs,
                                    user_message_activity_repo_in_memory.rs, simplex_adapter.rs, ai_gateway.rs);
                                    <what>_router.rs: a router (cross_domain_router.rs, moderation_notification_router.rs)
      drivers/                      a low-level client per system state lives in, holding no domain type
      migrations/                   NNNN_description.sql, applied in order
      test_db.rs                    an in-memory database with every migration applied, for adapter tests
webeditor/                          the static editor: index.html, editor.css, editor.js, ai.js, transfer.js, rules-schema.json
```

A module's tests are in `<module>/tests.rs` (or `<module_dir>/tests.rs`), declared by `#[cfg(test)] mod tests;`, never inline. The interface `/design` changes is the ports (`domain/bot_dm/ports.rs`, `domain/moderator/ports/`), `infrastructure/migrations/` and, for a condition or action, its type in its own module (the type is what the ports and the stored JSON carry); its entry in `webeditor/rules-schema.json` is the editor's, written by `/ux`.

### Infrastructure
- `adapters/` — implementations of **outbound ports** (driven adapters) and the **cross-context routers**:
  - `moderator_repo_sqlite.rs` + `moderator_repo_sqlite_rules.rs` — SQLite
    persistence for the moderator context (`ModerationRepository`).
  - `simplex_adapter.rs` — implements messenger/group actions on top of the
    SimpleX driver (`BotMessenger`, `GroupModerator`).
  - `cross_domain_router.rs` — lets `bot_dm` call into `moderator` by
    implementing `bot_dm::GroupOperations` on top of `moderator::GroupAdministration`.
  - `ai_gateway.rs` — implements the `OpenAi` port for `FlaggedByOmniModeration`
    and the `OpenRouter` port for `FlaggedByOpenRouterInstruction` (verdicts on
    messages and key checks, each with the owner's own key). It builds
    its own HTTP drivers, so `bin/bot.rs` only calls `AiGateway::new()` and hands
    the one gateway to both ports;
    tests start it over scripted ones. A message call is tried up to the condition's own `max_attempts` times,
    `retry_delay_seconds` apart (the driver itself makes one try), each try
    paced like any other request. Every
    request, whichever provider, goes through its in-process queue: a token
    bucket per key — a provider's limits are the key's, not the endpoint's — a cap
    on what one key may have waiting, a cap on requests in flight, a rest after
    429 and a local refusal after 401/403 or no money; key checks jump the queue.
    The drivers' errors are read as one gateway-level `Failure`; a text
    OpenRouter's moderation flagged (403 with `reasons`) is no verdict but never
    held against the key. A job that cannot be taken fails at once rather than blocking,
    and a provider failure reads as "no match". The ports are request/response,
    so the queue can later move out of the process without the domain noticing.
    The earlier messages `FlaggedByOpenRouterInstruction` sends as context reach
    it with their authors' display names; the gateway turns them into the
    driver's `PriorMessage`s, an attachment into a bracketed word before its caption.
  - `group_message_history_repo_in_memory.rs` — implements
    `GroupMessageHistoryRepository`: per group, the latest messages that stayed
    in the chat, for `FlaggedByOpenRouterInstruction` to send as context. The bot
    keeps them itself rather than asking SimpleX for its history. The
    application records a message only once it has been moderated — so a
    message is never its own context — and only while some rule reads earlier
    messages, as many as the most demanding one asks for; an edit replaces the
    kept text in place, and an edit the bot deleted is forgotten. Memory only:
    the adapter caps retention at 24 hours and 50 messages per group, and
    `bin/bot.rs` purges quiet groups hourly. A history that cannot be read is no
    verdict, never a verdict without context.
  - `moderation_notification_router.rs` — lets `moderator` notify `bot_dm` by
    implementing `moderator::ModerationNotifier` on top of
    `bot_dm::ModerationNotificationReceiver`. The receiver is injected *after*
    construction (`set_receiver`) to break the wiring cycle between the contexts.
- `drivers/` — low-level clients for the systems state lives in, external or
  local. `drivers/simplex/` is the SimpleX Chat websocket client that produces
  `SimplexEvent`s and exposes raw operations. `drivers/sliding_window_counter.rs`
  is the per-key counter with TTL eviction that every "how much did this member
  do recently" adapter is built on. `drivers/openai.rs` is the one client for
  OpenAI (`OpenAiApi`, implemented over HTTP by `HttpOpenAiApi`): the
  Moderation endpoint, its URL, the POST with its retries of
  transient failures and the error mapping, all in OpenAI's own words
  (category names like `hate/threatening`, statuses, `insufficient_quota`) —
  it knows nothing about members, messages or groups. `drivers/openrouter.rs`
  is the same for OpenRouter (`OpenRouterApi` / `HttpOpenRouterApi`): the chat
  completions call with a strict JSON schema — with earlier messages, the user
  message becomes one JSON object (`earlier_messages` beside the judged
  `message`) and the instruction gains a note saying so, so no member can write
  a line that passes for another's message — routed only to providers that
  honour it (`require_parameters`) and do not train on the text
  (`data_collection: deny`), and OpenRouter's errors (402 for credits, 403 for
  a key or for a flagged text). What makes something a driver is
  not that it is remote but that it holds no domain type: a driver is where the
  storage could be swapped (memory for Redis) without any port or adapter
  moving. Retention policy is *not* the driver's — the counter honours whatever
  TTL it is handed, and each adapter caps its own.
- `migrations/` — sequential SQL schema migrations (see DB section below).

### Where does new code go?
| You are adding… | Put it in… |
|-----------------|------------|
| New business rule / decision logic | `domain/<context>/` (pure, no I/O)|
| A new capability the domain needs from outside | a new **outbound port** trait in that context's ports (`domain/moderator/ports/outbound.rs`, `domain/bot_dm/ports.rs`) |
| A new way the outside drives the domain | a new **inbound port** trait in that context's ports (`domain/moderator/ports/inbound.rs`, `domain/bot_dm/ports.rs`) + its own application module |
| DB query / persistence | `infrastructure/adapters/` (a repo adapter) |
| Talking to SimpleX or another external system | `infrastructure/drivers/` (+ a thin adapter) |
| A low-level storage mechanism several adapters share | `infrastructure/drivers/` (generic, no domain types; the adapters keep the policy) |
| Letting one bounded context call another | a router in `infrastructure/adapters/` |
| Schema change | a new file in `infrastructure/migrations/`, and the regenerated *Schema* |
| A new moderation condition or action | its own module under `domain/moderator/rules/moderation_condition/` or `domain/moderator/rules/moderation_action/` + one registry line + its name in each trigger's list that takes it (see the steps below) |
| A new trigger | its variant in `Trigger`, its module `domain/moderator/rules/trigger/<trigger>.rs` (subject and lists), a `ModerationEngine` method, its `TriggerEvent` in both contexts, and the `oneOf` branch of `trigger` in `rules-schema.json` |
| Code two conditions or actions share | `domain/moderator/rules/common/` |
| Something the conditions need recorded about every message | a field of `Needs` + `domain/moderator/rules/bookkeeping.rs` |
| Constructing/wiring concrete types | `bin/bot.rs` only |

### Adding a condition type (do all of these)
A rule is written as a `RuleDraft { name, trigger, enabled, condition: Option<ModerationCondition>, actions: Vec<ModerationAction> }` and kept as a `ModerationRule` in its trigger's types (*Inside `moderator/rules/`*); both serialize to `{ "name": ..., "trigger": "message_posted", "enabled": true, "condition": { "type": ... }, "actions": [{ "type": ... }] }` — one `#[serde(tag = "type")]` condition (**not** flattened, and absent only from a join rule that matches everyone) next to an array of `#[serde(tag = "type")]` action objects. `ModerationCondition` is the **single source of truth** for detection types: a `#[serde(tag = "type")]` enum, built by the `conditions!` registry, whose leaf variants each wrap a type of their own that carries the condition's parameters (e.g. `ContainsWords(ContainsWords { keywords: Vec<String> })`); the composites `All`, `Any` and `Not` stay plain struct variants. A trigger's condition type wraps the same leaf types under the same variant names. Code outside `rules` names a leaf type through `ports::conditions`. Conditions are **pure logical predicates** (e.g. `ContainsWords`, `ContainsLinksInList`, `MatchesExactMessage`), describing what message or member triggers the rule, not the action to take. Each condition appears under **two naming forms that must stay aligned**: the PascalCase variant name is the serde `type` tag used in the URL hash, `rules-schema.json` and the `moderation_conditions.type` column (e.g. `ContainsWords`), while its snake_case form is the `<name>` used for DB table names (e.g. `moderation_condition__contains_words__keywords`).

A rule's condition is a **tree**, not a single predicate: besides the leaves there are three composite variants, `All`, `Any` and `Not`, which carry other conditions. Anything that asks something of every condition in a rule must therefore walk the tree (`ModerationCondition::walk`) rather than look at the rule's root condition, or it will silently miss nested uses — and it asks through a `Condition` method, never by naming a variant: `needs()` (what the bot must record for it: the counters a rate limit reads, the history an AI condition sends as context — `rules/bookkeeping.rs` merges them across every rule and records each message once), `depends_on_other_rules()` (answered after the other rules by a pre-pass in `rules.rs`, and kept out from under a `Not`) and `key_uses()` (the API keys checked when an owner saves rules). Note that the rule list is itself a disjunction, so `Any` only adds expressiveness when nested.

- **Name a condition after what it detects, never after what the owner thinks of it.** Because any condition can sit under a `Not`, a judgement baked into the name reads backwards there ("not: contains banned words"). So `ContainsWords`, not `ContainsBannedWords`; `ContainsLinksInList` / `ContainsLinksOutsideList`, not "forbidden" / "approved" websites; `ExceedsMaxLines`, not "floods chat". Prefer precise words to relative ones, which say nothing about the threshold: `AuthorHitsMessageRateLimit`, not `AuthorSentManyMessages`. This applies to every place the condition surfaces: variant and field names, table names, the editor title and field titles, `describe()`, and the reason string. Prefer one condition per check over a bundle of checks behind checkboxes — the composites already combine them, and a bundle turns into an unreadable "none of these" under a `Not`.
- **Emoji convention for condition titles:** the emoji names the *subject* of the check, not a verdict on it.
  - 💬 message text (e.g. `💬 Message Contains Any of These Words`, `💬 Message Matches Any of These Regex Patterns`).
  - 🔗 links (e.g. `🔗 Message Contains a Link to Any of These Websites`, `🔗 Message Contains a Link Outside Top 100 Websites`).
  - 📏 the message's shape and size (e.g. `📏 Message Is Empty or Blank`, `📏 Message Exceeds Max Lines`).
  - 📎 what the message carries besides text (e.g. `📎 Message Contains an Image`, `📎 Message Contains a File`). An attachment's caption *is* the message text, so the text conditions still describe it — with one exception: `IsBlank` never matches a message that carries an attachment, because a caption-less picture is a picture, not an empty message.
  - 👤 the member: a message's author, or, for the name conditions, also the person who joins (e.g. `👤 Author Hits Message Rate Limit`, `👤 Author Joined Recently`, `👤 Member's Name Contains Any of These Words`).
  - 👥 the group as a whole, every member together (e.g. `👥 Group Hits Message Rate Limit`).
  - The composites are not detectors and take a shape of their own (`🧩 All of These Conditions Match (AND)`, `🔀 Any …`, `✖️ This Condition Does Not Match (NOT)`).

1. **Domain:** create the module `rules/moderation_condition/<snake_name>.rs` — with its tests in `<snake_name>/tests.rs`, and the matching algorithm in `<snake_name>/filter.rs` when it is more than a few lines — holding a `pub struct` named after the variant, holding its parameters, and implement `Condition` for it — `normalize_and_validate` (its parameters are checked when an owner saves the rule), `describe` (so a `Not` around it can explain itself), plus whichever of `needs`, `depends_on_other_rules` and `key_uses` apply to it (each defaults to "nothing") — and `ConditionOn<S>` for what it is checked on: `should_moderate` (the matching; `ctx.subject` is the `GroupMessage`, or any `Member` for a condition on the member alone, and `ctx.ports` the repositories and AI providers). Then add one `Variant => module` line to `conditions!` in `rules/moderation_condition.rs`, and its name to the list of each trigger that takes it in `rules/trigger/<trigger>.rs` — that is the whole registration. Keep the matching pure and unit-tested. A condition never imports another one: code it shares with another condition goes in `rules/common/`.
2. **Migration:** add `infrastructure/migrations/NNNN_*.sql`, and regenerate *Schema* below. A condition with scalar settings gets `moderation_condition__<name>` keyed by `condition_id`; a condition whose only parameter is a list gets just `moderation_condition__<name>__<subtable>`; a condition with no parameters needs no migration. No `group_id`, `rank` or `action_id` columns — those live on `moderation_rules` and `moderation_conditions`. Follow the schema conventions above.
3. **Repository read:** load the new tables into `ConditionData` in `infrastructure/adapters/moderator_repo_sqlite_rules.rs` (one group-wide query per table, via `load_condition_lists` / `load_condition_settings`) and add an arm to `build_condition` keyed on the type tag.
4. **Repository write:** add an arm to `insert_condition` in `infrastructure/adapters/moderator_repo_sqlite.rs` that writes the settings row and/or list rows against `condition_id`.
5. **Web editor:** add the condition's shape to `webeditor/rules-schema.json` as a new `oneOf` entry under `definitions.condition`, using the variant name as the `type` const so it matches the serde representation, and give it `options.summary`, `options.phrase` and `options.triggers` (the triggers whose lists name it; see *The web editor is schema-driven*). Title, description, field types, defaults and limits all come from that entry — **do not touch `editor.js`**. The actions are a sibling of condition on every rule (`properties.rules.items.properties.actions`), so no per-condition action wiring is needed.
6. **Bug template:** Update `moderation-rule-bug.yml` to add the new rule's title (as it appears in `rules-schema.json`) to the `rule-type` dropdown options list so bug reporters can select it.

### Renaming or replacing a condition
Stored rules are the only thing that has to survive: the bot always sends the owner a freshly generated editor link, so old type tags never come back through a link. Do **not** add `#[serde(alias = ...)]`s or compatibility deserializers for old shapes. Instead, add a migration that rewrites the stored rows — update `moderation_conditions.type` and rename the condition's tables (`ALTER TABLE ... RENAME TO`) for a rename, or rebuild the affected nodes for a condition replaced by differently shaped ones. Precedents: `0025_neutral_condition_names.sql`, and `0031_openai_instruction_and_omni_rename.sql` / `0033_openrouter_instruction.sql` for a plain rename.

### Adding an action type (do all of these)
`ModerationAction` (in `domain/moderator/rules/moderation_action.rs`) is the **single source of truth** for actions: a `#[serde(tag = "type")]` enum, built by the `actions!` registry, whose variants each wrap a type of their own carrying the action's settings (`ModerateMessage(ModerateMessage {})`, `SetAuthorObserver(SetAuthorObserver { duration_minutes: u32 })`, `KickAuthor(KickAuthor { delete_all_messages: bool })`); code outside `rules` names one through `ports::actions`. Like conditions, the PascalCase variant name is the serde `type` tag (URL hash, `rules-schema.json`, and the `moderation_actions.type` column), and its snake_case form is the `<name>` used for the `moderation_action__<name>` settings table.

A rule carries a **list** of actions, and every variant must stay one indivisible thing the bot can do. Do **not** add a setting to an action that means "and also do that other action": the owner combines actions by listing them, and a bundled sub-action cannot be deduplicated against the same action coming from another rule. The list is a **set**, not a sequence: `action_planner` drops actions a stronger one already covers and orders the rest by `execution_rank` — observer, then moderate, then kick, so the author is kicked last — and applies the same merge to a single rule's list when an owner saves it (`ModerationRule::normalize_and_validate`). The planner names no action: coverage is derived from what each one says it achieves (`Action::effect` — the message deleted, the author's history deleted, how long the author is silenced, whether they are removed), one action covering another when its effect is at least as strong on every count. So a new action is placed among the others by its `effect`, never by a new pair in `covers`; if what it achieves cannot be said in those terms, add the term to `Effect`.

1. **Domain:** create the module `rules/moderation_action/<snake_name>.rs`, with its tests in `<snake_name>/tests.rs`, holding a `pub struct` named after the variant with its settings, and implement `Action` for it — `normalize_and_validate` (its settings are checked when an owner saves the rule), `effect`, `execution_rank` — and `ActionOn<S>` for what it is carried out on: `execute` (on `ctx.subject`, the `GroupMessage` or any `Member`, through `ctx.ports` — the `GroupModerator` and `MemberRestoreRepository` outbound ports; a failure of the bot's own records goes into `ActionReport::bookkeeping_error` rather than the error, so it does not call off the moderation). Then add one `Variant => module` line to `actions!` in `rules/moderation_action.rs`, and its name to the list of each trigger that takes it in `rules/trigger/<trigger>.rs` — the application carries out whatever the planner hands it (`execute_actions`) and needs no change. An action never imports another one. If the action needs a capability the messenger doesn't expose yet, add a method to `GroupModerator` in `domain/moderator/ports/outbound.rs` and implement it in `infrastructure/adapters/simplex_adapter.rs` (+ the `drivers/simplex` driver).
2. **Cross-context (owner notifications):** mirror the variant in `bot_dm::ModerationAction` (`domain/bot_dm/ports.rs`), map it in `infrastructure/adapters/moderation_notification_router.rs`, and render its notification text in `domain/bot_dm/application.rs`.
3. **Migration:** add `infrastructure/migrations/NNNN_*.sql`, and regenerate *Schema* below, creating `moderation_action__<name>` (keyed by `action_id` FK → `moderation_actions(id) ON DELETE CASCADE`) with one column per setting.
4. **Repository read/write:** resolve the new action in `load_actions` (one group-wide query, keyed by `rule_id` and ordered by `rank`) and persist it in `insert_actions` (both in the `moderator_repo_sqlite*` adapters).
5. **Web editor:** add the action as a new `oneOf` entry under `definitions.action` in `webeditor/rules-schema.json`, using the variant name as the `type` const and one field per setting, plus `options.execution_rank` (matching the action's `execution_rank`), `options.triggers` (the triggers whose lists name it), `options.covered_by` (the actions whose presence makes this one redundant, as `covers` derives it from the effects; `test_coverage_between_every_pair_of_actions` in `action_planner/tests.rs` pins the whole relation) and `options.summary`. `definitions.action` describes **one** action; the rule's `actions` array references it. No change to `editor.js`.
6. **Bug template:** add the action's title to the `rule-action` dropdown in `moderation-rule-bug.yml`.

## Schema
The whole schema the migrations produce. Generated by `cd bot && UPDATE_SCHEMA=1 cargo test schema`, never edited by hand: the same test without the variable fails while this section is out of date.

<!-- schema:begin -->
```sql
-- user_version 36

CREATE TABLE "moderation_action__kick_author" (
    action_id           INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE,
    delete_all_messages BOOLEAN NOT NULL DEFAULT 0
);

CREATE TABLE moderation_action__moderate_message (
    action_id INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE
);

CREATE TABLE "moderation_action__set_author_observer" (
    action_id INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE
, duration_minutes INTEGER NOT NULL DEFAULT 0);

CREATE TABLE moderation_actions (
    id   INTEGER PRIMARY KEY,
    type TEXT NOT NULL
, rule_id INTEGER REFERENCES moderation_rules(id) ON DELETE CASCADE, rank INTEGER NOT NULL DEFAULT 0);

CREATE UNIQUE INDEX idx_moderation_actions_rule_id_rank ON moderation_actions (rule_id, rank);

CREATE TABLE moderation_condition__author_hits_character_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    character_count     INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE moderation_condition__author_hits_line_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    line_count          INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL,
    chars_per_line      INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__author_hits_message_rate_limit" (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__author_hits_moderation_rate_limit" (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__author_joined_recently" (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__contains_links_in_list__domains" (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE "moderation_condition__contains_links_outside_list__domains" (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE "moderation_condition__contains_links_outside_top100__domains" (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE moderation_condition__contains_repeated_sequence (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    min_repeats  INTEGER NOT NULL,
    min_length   INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__contains_words__keywords" (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    keyword      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, keyword)
);

CREATE TABLE moderation_condition__exceeds_max_characters (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_characters INTEGER NOT NULL
);

CREATE TABLE moderation_condition__exceeds_max_lines (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_lines      INTEGER NOT NULL,
    chars_per_line INTEGER NOT NULL
);

CREATE TABLE moderation_condition__exceeds_max_words (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_words    INTEGER NOT NULL
);

CREATE TABLE "moderation_condition__flagged_by_omni_moderation" (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    api_key      TEXT    NOT NULL
, max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 5), retry_delay_seconds INTEGER NOT NULL DEFAULT 1 CHECK (retry_delay_seconds BETWEEN 0 AND 10));

CREATE TABLE "moderation_condition__flagged_by_omni_moderation__categories" (
    condition_id      INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    category          TEXT    NOT NULL,
    min_score_percent INTEGER NULL CHECK (min_score_percent BETWEEN 1 AND 100),
    PRIMARY KEY (condition_id, category)
);

CREATE TABLE "moderation_condition__flagged_by_openrouter_instruction" (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    api_key      TEXT    NOT NULL,
    model        TEXT    NOT NULL,
    instruction  TEXT    NOT NULL
, max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 5), retry_delay_seconds INTEGER NOT NULL DEFAULT 1 CHECK (retry_delay_seconds BETWEEN 0 AND 10), context_messages INTEGER NOT NULL DEFAULT 0 CHECK (context_messages >= 0));

CREATE TABLE moderation_condition__group_hits_character_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    character_count     INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE moderation_condition__group_hits_line_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    line_count          INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL,
    chars_per_line      INTEGER NOT NULL
);

CREATE TABLE moderation_condition__group_hits_message_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE moderation_condition__matches_exact_message (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    case_sensitive BOOLEAN NOT NULL DEFAULT 0
);

CREATE TABLE moderation_condition__matches_exact_message__messages (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, message)
);

CREATE TABLE moderation_condition__matches_regex__patterns (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    pattern      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, pattern)
);

CREATE TABLE moderation_conditions (
    id        INTEGER PRIMARY KEY,
    rule_id   INTEGER NOT NULL REFERENCES moderation_rules(id) ON DELETE CASCADE,
    parent_id INTEGER          REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    rank      INTEGER NOT NULL,
    type      TEXT    NOT NULL
);

CREATE INDEX idx_moderation_conditions_parent_id ON moderation_conditions (parent_id);

CREATE INDEX idx_moderation_conditions_rule_id   ON moderation_conditions (rule_id);

CREATE TABLE moderation_groups (
    group_id           INTEGER PRIMARY KEY,
    messenger_group_id INTEGER NOT NULL UNIQUE,
    owner_id           INTEGER NOT NULL,
    group_name         TEXT NOT NULL DEFAULT ''
, mode TEXT NOT NULL DEFAULT 'notifications'
        CHECK (mode IN ('dry', 'silent', 'notifications')));

CREATE INDEX idx_moderation_groups_owner_id
    ON moderation_groups (owner_id);

CREATE TABLE moderation_rules (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);

CREATE INDEX idx_moderation_rules_group_id ON moderation_rules (group_id);

CREATE TABLE moderation_set_author_observer_restores (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id   INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    member_id  INTEGER NOT NULL,
    -- Unix seconds, UTC. Nobody reads this column by eye, and an integer
    -- compares and sorts without a text format to agree on first
    -- (`datetime(execute_at, 'unixepoch')` renders it when someone does look).
    execute_at INTEGER NOT NULL,
    -- One deadline per member: a later restriction of the same member replaces
    -- the pending restore instead of adding a second one.
    UNIQUE (group_id, member_id)
);

CREATE INDEX idx_moderation_set_author_observer_restores_execute_at
    ON moderation_set_author_observer_restores (execute_at);
```
<!-- schema:end -->
