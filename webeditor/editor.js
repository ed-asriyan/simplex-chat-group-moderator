/*
 * Rules editor for the SimpleX Chat group moderator bot.
 *
 * SimpleX carries plain text only, so the whole round trip is one URL: the bot
 * sends `#bot_id=<id>&group=<name>&config=<lz>`, where the config is the
 * group's whole configuration — its settings (the mode) beside its rules. This
 * page edits it, writes it back into the hash on every change, and the owner
 * pastes the link into the chat. `bot_id` and `config` must be present or the
 * bot ignores the message; `group` is the name the page shows.
 *
 * Nothing here knows a single condition, action, trigger or group setting by
 * name. The registries are derived from rules-schema.json, so adding a rule
 * type is one `oneOf` entry in that file and no change to this code. What the
 * schema cannot express — the one-line summary, the human phrase, the execution
 * order of actions, the triggers a type works with — lives in optional
 * `options` keys on the same entry, each with a fallback when absent.
 *
 * Three pages (US-G1, US-G3, US-G14): the group's overview, one rule, and New
 * rule. Each is an entry in the browser's history, so Back goes where the
 * owner came from.
 */

/* ------------------------------------------------------------------ *
 * Layout vocabulary
 *
 * Every condition and action renders as ONE ROW titled by its type:
 *   [▾] [emoji] Title [summary when folded] [ⓘ] [⋮]
 * Its fields open under it, each with its description in view (US-G3); the
 * type's long description is behind ⓘ. Depth is a rail plus a small indent,
 * so a six-level tree still fits a phone.
 * ------------------------------------------------------------------ */

const EDITOR_URL_BASE = location.href.split("#")[0];

/* Emoji is the subject taxonomy, so it also groups the type picker. An unknown
   emoji falls into "Other" rather than breaking the list. */
const GROUPS = {
    "💬": "💬 Text",
    "🔗": "🔗 Links",
    "📏": "📏 Shape and size",
    "📎": "📎 Attachments",
    "👤": "👤 Member",
    "👥": "👥 Group",
    "🧩": "Combine",
    "🔀": "Combine",
    "✖️": "Combine",
};

/* Past this many entries a list gets a filter. */
const BIG_LIST = 20;

/* How many steps back the editor remembers. The rules are plain JSON, so a
   snapshot is a cheap clone. */
const UNDO_LIMIT = 20;

const state = {
    rules: [],
    /* The group's settings: every property of the schema root but `rules`. */
    settings: {},
    /* Snapshots of the config before each change that actually changed
       something, and the ones undone out of it, waiting to be redone. */
    history: [],
    future: [],
    /* What the bot has stored: the config as it arrived in the hash. Every diff
       is measured against this, whoever made the change. */
    baseline: { rules: [] },
    botId: null,
    groupName: null,
    /* "overview", "rule" (state.sel) or "new" (state.newTrigger). */
    view: "overview",
    sel: 0,
    newTrigger: null,
    showTemplates: false,
    howOpen: false,
    focus: null,
    collapsed: new Set(),
    helped: new Set(),
    /* Password fields the owner chose to show, by path. */
    revealed: new Set(),
    bulk: new Set(),
    q: {},
    menu: null,
    limits: { depth: 8, nodes: 64 },
};

let C = {};   // condition registry, built from the schema
let A = {};   // action registry
let S = { p: [], def: {} };   // the group settings, from the schema root
let T = [];   // the triggers, in the order of their sections
let RULE = {};   // the rule's own fields: name, trigger, enabled
let SCHEMA = null;

/* ------------------------------- helpers ------------------------------- */

const esc = (s) =>
    String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

const fmtList = (a) =>
    a.length ? a.slice(0, 2).join(", ") + (a.length > 2 ? ` +${a.length - 2}` : "") : "empty";

/* Inline markdown from the schema's own descriptions. Escaped first, so the
   only HTML that survives is what these four rules produce. */
const md = (t) =>
    esc(t)
        .replace(/`([^`]+)`/g, "<code>$1</code>")
        .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        .replace(/(^|[\s(])[_*]([^_*]+)[_*]/g, "$1<i>$2</i>")
        .replace(/\[([^\]]+)\]\((https?:[^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');

const spec = (reg, type) => reg[type] || unknownSpec(type);

const clone = (v) => structuredClone(v);

const ordinal = (n) => {
    const s = ["th", "st", "nd", "rd"], v = n % 100;
    return n + (s[(v - 20) % 10] || s[v] || s[0]);
};

/* Everything the link carries, settings and rules together: what undo
   snapshots, what Apply diffs, what the bot stores. */
const config = () => ({ ...state.settings, rules: state.rules });
const snapshot = () => clone(config());
function restore(c) {
    const { rules, ...settings } = c;
    state.rules = rules;
    state.settings = settings;
}

/* Every change goes through one of the DOM listeners below, so recording there
   catches all of them. A handler that turns out to change nothing records
   nothing. */
function recordUndo(before) {
    if (canon(before) === canon(config())) return;
    state.history.push(before);
    if (state.history.length > UNDO_LIMIT) state.history.shift();
    /* Editing after undoing forks the history: what was undone is gone. */
    state.future = [];
}

/* Undo and redo are the same move in opposite directions: take the last state
   off one stack, put the current one on the other. */
function step(from, to, word) {
    const target = from.pop();
    if (!target) return;
    to.push(snapshot());
    restore(target);
    if (state.view === "rule" && !state.rules[state.sel]) state.view = "overview";
    state.focus = null;
    state.menu = null;
    render();
    toast(word);
}

const undo = () => step(state.history, state.future, "Undone.");
const redo = () => step(state.future, state.history, "Redone.");

/* Key-order-independent identity, so two rules that differ only in how their
   JSON was written still count as the same rule. */
const canon = (v) =>
    JSON.stringify(v, (k, val) =>
        val && typeof val === "object" && !Array.isArray(val)
            ? Object.fromEntries(Object.keys(val).sort().map((k2) => [k2, val[k2]]))
            : val
    );

/* A rule saved by a newer bot than this page's schema still has to render, and
   above all has to survive being sent back untouched. */
function unknownSpec(type) {
    return {
        g: "❓",
        grp: "Other",
        t: `${type} (unknown)`,
        name: `${type} (unknown)`,
        full: `${type} (unknown)`,
        gist: "",
        help: "This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.",
        p: [],
        rank: 0,
        coveredBy: [],
        triggers: null,
        def: {},
        unknown: true,
        sum: () => "",
        say: () => type,
    };
}

const say = (n) => spec(C, n.type).say(n);

/* {field} · {field|n} · {field|list} · {field|phrase(s)} · {field?yes:no} with $ = value */
function tpl(s, v) {
    return s.replace(
        /\{([a-z_]+)(?:\?([^:}]*):([^}]*)|\|(n|list|phrase|phrases))?\}/g,
        (m, f, yes, no, mod) => {
            const val = v[f];
            if (yes !== undefined) return (val ? yes : no).replace(/\$/g, val);
            if (mod === "n") return (val || []).length;
            if (mod === "list") return fmtList(val || []);
            if (mod === "phrase") return val ? say(val) : "";
            if (mod === "phrases") return (val || []).map(say).join("; ");
            return val;
        }
    );
}

/* ------------------------- registry from schema ------------------------ */

function kindOf(pr) {
    if (pr.$ref) return "child";
    if (pr.type === "array") return pr.items && pr.items.$ref ? "children" : "strlist";
    if (pr.type === "boolean") return "bool";
    if (pr.type === "integer" || pr.type === "number") return "int";
    /* A string the owner would not want read over their shoulder. */
    if (pr.type === "string" && pr.format === "password") return "password";
    /* Prose rather than a word: several lines, and a diff that counts. */
    if (pr.type === "string" && pr.format === "textarea") return "textarea";
    /* A few named values and, optionally, a number of the owner's own:
       `oneOf` of `const`s plus at most one integer branch. */
    if (pr.oneOf) return "choice";
    return "text";
}

/* The pieces of a `choice` field: its named values and its number branch. */
function choiceParts(pr) {
    const branches = pr.oneOf || [];
    return {
        choices: branches
            .filter((b) => "const" in b)
            .map((b) => ({ v: b.const, t: b.title || String(b.const), o: b.options || {} })),
        num: branches.find((b) => b.type === "integer" || b.type === "number"),
    };
}

function defaultFor(pr) {
    if ("default" in pr) return structuredClone(pr.default);
    const k = kindOf(pr);
    if (k === "strlist" || k === "children") return [];
    if (k === "bool") return false;
    if (k === "int") return 0;
    if (k === "choice") {
        const { choices, num } = choiceParts(pr);
        return choices.length ? choices[0].v : num.minimum ?? 0;
    }
    return "";
}

/* How a `choice` value reads: the title of a named value, or the number with
   the unit its branch declares. */
function choiceText(f, v) {
    if (typeof v === "number") return `${v}${f.unit}`;
    const c = f.choices.find((x) => x.v === v);
    return c ? c.t : String(v);
}

/* Thirteen fields in one row read best grouped by value: the value most of
   them share is counted, the few that differ are named. */
function choiceSummary(fields, v) {
    const byText = new Map();
    for (const f of fields) {
        const t = choiceText(f, v[f.k]);
        if (!byText.has(t)) byText.set(t, []);
        byText.get(t).push(f.label);
    }
    return [...byText]
        .map(([t, labels]) => (labels.length > 2 ? `${t} ×${labels.length}` : labels.map((l) => `${l} ${t}`).join(" · ")))
        .join(" · ");
}

/* One parameter's control, read off its schema property. */
function fieldOf([k, pr]) {
    const { choices, num } = choiceParts(pr);
    return {
        k,
        kind: kindOf(pr),
        label: (pr.options && pr.options.label) || pr.title || k,
        hint: pr.description ? md(pr.description) : "",
        min: pr.minimum ?? (num && num.minimum),
        max: pr.maximum ?? (num && num.maximum),
        maxItems: pr.maxItems,
        maxLength: pr.items ? pr.items.maxLength : pr.maxLength,
        choices,
        num,
        unit: (num && num.options && num.options.unit) || "",
    };
}

/* The group settings are the schema root's properties other than `rules`, so
   a new one is a property there and no code here. */
function buildSettings(root) {
    const fields = Object.entries(root.properties).filter(([k]) => k !== "rules");
    return {
        full: "Group settings",
        help: "",
        p: fields.map(fieldOf),
        props: Object.fromEntries(fields),
        def: Object.fromEntries(fields.map(([k, pr]) => [k, defaultFor(pr)])),
    };
}

/* The triggers are the `oneOf` of the rule's `trigger` field: each one is a
   section of the overview, and its `options` carry the section's texts. */
function buildTriggers(pr) {
    return pr.oneOf.map((b) => {
        const o = b.options || {};
        return {
            v: b.const,
            title: b.title,
            section: o.section || b.title,
            newRule: o.new_rule || b.title,
            empty: o.empty || "No rules yet.",
            incompatible: o.incompatible || "Doesn't work here",
            incomplete: o.incomplete || "Pick a condition and at least one action.",
            prompt: o.prompt || b.title,
            optional: !!o.condition_optional,
            everyone: o.everyone || "Always",
        };
    });
}

/* A settings object from outside — the hash, an import — checked field by
   field against the schema. Returns the errors, or the settings with every
   missing field at its default. */
function validateSettings(obj) {
    const errors = [], out = clone(S.def);
    for (const k of Object.keys(obj)) if (!S.p.some((f) => f.k === k)) errors.push(`There is no setting “${k}”.`);
    for (const f of S.p) {
        if (!(f.k in obj)) continue;
        const v = obj[f.k];
        const ok =
            f.kind === "choice" ? f.choices.some((c) => c.v === v) || (f.num && Number.isInteger(v))
                : f.kind === "bool" ? typeof v === "boolean"
                    : f.kind === "int" ? Number.isInteger(v)
                        : typeof v === "string";
        if (ok) out[f.k] = v;
        else errors.push(`${f.label}: “${JSON.stringify(v)}” is not a value it can take.`);
    }
    return errors.length ? { errors } : { settings: out };
}

/* The fields behind a `data-p` path: the settings' own, or the condition's or
   action's that the path ends at. */
const fieldsAt = (p) => (p === "settings" ? S.p : [...spec(C, at(p).type).p, ...spec(A, at(p).type).p]);

function buildRegistry(entries, defaultTriggers) {
    const reg = {};
    for (const e of entries) {
        const key = e.properties.type.const;
        const emoji = e.title.split(" ")[0];
        const rest = e.title.slice(emoji.length).trim();
        const o = e.options || {};
        const fields = Object.entries(e.properties).filter(([k]) => k !== "type");
        const entry = {
            g: emoji,
            grp: GROUPS[emoji] || "Other",
            full: e.title,
            name: rest,
            help: e.description ? md(e.description) : "",
            /* The first sentence, for the type picker. */
            gist: e.description ? md(e.description.split(/(?<=[a-z)`]\.)\s+(?=[A-Z*_])/)[0]) : "",
            t: o.short || rest.replace(/^(Message|Author|This Condition)\s+/, ""),
            rank: o.execution_rank || 0,
            coveredBy: o.covered_by || [],
            triggers: o.triggers || defaultTriggers,
            p: fields.map(fieldOf),
            def: Object.fromEntries(fields.map(([k, pr]) => [k, defaultFor(pr)])),
        };
        entry.sum = (v) =>
            o.summary
                ? tpl(o.summary, v)
                : [
                    ...entry.p.filter((f) => f.kind === "int").map((f) => `${f.k} ${v[f.k]}`),
                    choiceSummary(entry.p.filter((f) => f.kind === "choice"), v),
                ]
                    .filter(Boolean)
                    .join(" · ");
        entry.say = (v) => (o.phrase ? tpl(o.phrase, v) : entry.t.toLowerCase());
        reg[key] = entry;
    }
    return reg;
}

/* Changing a type carries across anything the new type also has, under the
   same name and of the same kind — which is what keeps the whole subtree when
   All becomes Any, and the domains when one link condition becomes another
   (US-G4). */
function retype(oldNode, nextType, registry) {
    const next = { type: nextType, ...clone(spec(registry, nextType).def) };
    const wasFields = spec(registry, oldNode.type).p;
    for (const f of spec(registry, nextType).p) {
        const was = wasFields.find((x) => x.k === f.k);
        if (was && was.kind === f.kind && oldNode[f.k] !== undefined) next[f.k] = clone(oldNode[f.k]);
    }
    return next;
}

/* ------------------------------- rules ---------------------------------- */

const trig = (r) => r.trigger || T[0].v;
const tinfo = (r) => T.find((t) => t.v === trig(r)) || T[0];
const isOn = (r) => r.enabled !== false;
const works = (s, r) => s.unknown || s.triggers.includes(trig(r));

/* What keeps a rule from being sent: no condition where its trigger needs
   one, or no action at all (US-G14). */
function issueOf(r) {
    if (!r.condition && !tinfo(r).optional) return "condition";
    if (!r.actions || !r.actions.length) return "action";
    return null;
}

const actionWords = (r) => (r.actions || []).map((a) => `${spec(A, a.type).g} ${spec(A, a.type).say(a)}`).join(" · ");
const condWords = (r) => (r.condition ? say(r.condition) : tinfo(r).everyone);
const rulePhrase = (r) => `IF ${condWords(r)} THEN ${actionWords(r)}`;

/* How a rule is called everywhere: its name, or what it does (US-G1). */
const titleOf = (r) => r.name || (issueOf(r) ? "New rule" : rulePhrase(r));

/* The rules of one section, with their place in the whole list. */
const sectionOf = (v) => state.rules.map((r, i) => [r, i]).filter(([r]) => trig(r) === v);

/* ------------------------------ tree walking --------------------------- */

const at = (p) => p.split(".").reduce((o, k) => o[k], state);
const parentOf = (p) => ({ o: at(p.slice(0, p.lastIndexOf("."))), k: p.slice(p.lastIndexOf(".") + 1) });

/* Composites are found through the schema's own shape, not by name. */
const childKeyOf = (n) => (spec(C, n.type).p.find((f) => f.kind === "child") || {}).k;
const childrenKeyOf = (n) => (spec(C, n.type).p.find((f) => f.kind === "children") || {}).k;
const isComposite = (n) => !!(childKeyOf(n) || childrenKeyOf(n));

function depthOf(n) {
    if (!n) return 0;
    const one = childKeyOf(n), many = childrenKeyOf(n);
    if (one && n[one]) return 1 + depthOf(n[one]);
    if (many) return 1 + Math.max(0, ...(n[many] || []).map(depthOf));
    return 1;
}

function nodesOf(n) {
    if (!n) return 0;
    const one = childKeyOf(n), many = childrenKeyOf(n);
    if (one && n[one]) return 1 + nodesOf(n[one]);
    if (many) return 1 + (n[many] || []).reduce((s, x) => s + nodesOf(x), 0);
    return 1;
}

function foldAll(n, path, set) {
    set.add(path);
    const one = childKeyOf(n), many = childrenKeyOf(n);
    if (one && n[one]) foldAll(n[one], `${path}.${one}`, set);
    else if (many) (n[many] || []).forEach((x, j) => foldAll(x, `${path}.${many}.${j}`, set));
}

const fresh = (reg, type) => ({ type, ...clone(reg[type].def) });

/* The container a new condition joins when the rule has a single one already:
   the composite that holds many and is not NOT — found by shape, not name. */
const andType = () => Object.keys(C).find((k) => C[k].p.some((f) => f.kind === "children"));

/* --------------------------------- URL --------------------------------- */

/* The hash is the transport: it is rewritten on every change so the address
   bar (and therefore the copied link) is always what the bot would store. */
function hashParts() {
    const compressed = LZString.compressToEncodedURIComponent(JSON.stringify(config()));
    return (
        (state.botId ? `bot_id=${state.botId}&` : "") +
        (state.groupName ? `group=${encodeURIComponent(state.groupName)}&` : "") +
        `config=${compressed}`
    );
}

function syncHash() {
    /* Keep whatever navigation state this entry carries: the pages live in the
       history. */
    history.replaceState(history.state, "", "#" + hashParts());
}

/* Each page is an entry in the history, so the browser's Back goes back a
   page instead of leaving the editor (US-G3). */
function goTo(view, extra = {}, replace = false) {
    state.view = view;
    if (extra.sel !== undefined) state.sel = extra.sel;
    if (extra.trigger !== undefined) state.newTrigger = extra.trigger;
    state.showTemplates = !!extra.templates;
    state.focus = null;
    state.menu = null;
    history[replace ? "replaceState" : "pushState"]({ v: view, i: state.sel, t: state.newTrigger }, "", location.hash);
    window.scrollTo(0, 0);
}

window.addEventListener("popstate", (e) => {
    const entry = e.state || {};
    state.view = ["rule", "new"].includes(entry.v) ? entry.v : "overview";
    if (typeof entry.i === "number") state.sel = entry.i;
    if (entry.t) state.newTrigger = entry.t;
    if (state.view === "rule" && !state.rules[state.sel]) state.view = "overview";
    state.showTemplates = false;
    state.focus = null;
    state.menu = null;
    render();
});

/* The draft lives only in this tab: the link in the chat still holds the old
   config until the owner sends a new one. */
window.addEventListener("beforeunload", (e) => {
    if (canon(state.baseline) !== canon(config())) e.preventDefault();
});

function parseHash() {
    const out = {};
    for (const part of location.hash.slice(1).split("&")) {
        const i = part.indexOf("=");
        if (i >= 0) out[part.slice(0, i)] = part.slice(i + 1);
    }
    return out;
}

/* ------------------------------ rendering ------------------------------ */

function toast(text) {
    const el = document.createElement("div");
    el.className = "toast";
    el.setAttribute("role", "status");
    el.textContent = text;
    document.body.appendChild(el);
    setTimeout(() => el.remove(), 2800);
}

/* A ⋯ or ⋮ button and the menu it opens. `items` are [label, data-attrs,
   extra class]. */
function menuHtml(id, label, glyph, items) {
    const open = state.menu === id;
    return `<div class="menu"><button class="ico" type="button" data-op="menu" data-p="${esc(id)}"
      aria-haspopup="menu" aria-expanded="${open}" aria-label="${esc(label)}">${glyph}</button>
      ${open
            ? `<div class="pop" role="menu">${items
                .map(([text, attrs, cls]) => `<button type="button" role="menuitem" ${attrs} ${cls ? `class="${cls}"` : ""}>${esc(text)}</button>`)
                .join("")}</div>`
            : ""
        }</div>`;
}

/* ---- the overview ---- */

function settingsHtml() {
    return S.p
        .map((f) => {
            const pr = S.props[f.k];
            /* A setting of a few named values whose titles read "Name — what it
               does" is a row of choices, each with its line (US-G2). */
            const segmented = f.kind === "choice" && !f.num && f.choices.every((c) => c.t.includes(" — "));
            if (!segmented)
                return `<section class="card" aria-label="${esc(f.label)}"><div class="params">${params(S, state.settings, "settings")}</div></section>`;
            const id = `set-${f.k}`;
            return `<section class="card setting" aria-labelledby="${id}-h">
          <div class="card-head"><h2 id="${id}-h">${esc(pr.title || f.k)}</h2>
            <button class="ico" type="button" data-op="help" data-p="settings" aria-expanded="${state.helped.has("settings")}"
              aria-label="About ${esc(f.label.replace(/^\S+\s/, ""))}">ⓘ</button></div>
          ${state.helped.has("settings") && pr.description ? `<div class="help">${md(pr.description)}</div>` : ""}
          <div class="seg" role="radiogroup" aria-labelledby="${id}-h">
          ${f.choices
                    .map((c, i) => {
                        const [name, line] = c.t.split(" — ");
                        const on = state.settings[f.k] === c.v;
                        return `<label class="segopt${on ? " on" : ""}">
              <input type="radio" name="${id}" id="${id}-c${i}" value="c${i}" ${on ? "checked" : ""} data-op="choice" data-p="settings" data-k="${f.k}">
              <span class="segname">${esc(name)}</span>
              <span class="segline">${esc(line.charAt(0).toUpperCase() + line.slice(1))}</span></label>`;
                    })
                    .join("")}
          </div></section>`;
        })
        .join("");
}

function ruleRowHtml(r, i, pos, count) {
    const title = titleOf(r), issue = issueOf(r), on = isOn(r);
    /* A rule without a name reads as its phrase, which names its actions. */
    const icons = r.name ? (r.actions || []).map((a) => spec(A, a.type).g).join(" ") : "";
    return `<li class="rrow${on ? "" : " off"}" data-i="${i}">
      <button class="handle" type="button" data-drag="${i}" aria-label="Move “${esc(title)}”"
        aria-describedby="move-hint">⠿</button>
      <button class="ropen" type="button" data-op="open" data-i="${i}">
        <span class="rtitle${r.name ? "" : " phrase"}">${esc(title)}</span>
        <span class="rmeta">
          ${icons ? `<span class="ricons" aria-hidden="true">${icons}</span>` : ""}
          ${issue ? `<span class="badge warn">Incomplete</span>` : ""}
          ${on ? "" : `<span class="badge">Off</span>`}
        </span>
      </button>
      <button class="switch" type="button" role="switch" aria-checked="${on}" data-op="toggle" data-i="${i}"
        aria-label="${esc(title)}"><span class="knob"></span></button>
      ${count > 1
            ? menuHtml(`r:${i}`, `More for “${title}”`, "⋯", [
                ...(pos > 0 ? [["Move up", `data-op="rmove" data-i="${i}" data-d="-1"`]] : []),
                ...(pos < count - 1 ? [["Move down", `data-op="rmove" data-i="${i}" data-d="1"`]] : []),
            ])
            : `<span class="menu-sp"></span>`}
    </li>`;
}

function renderOverview(host) {
    const sections = T.map((t) => {
        const rows = sectionOf(t.v);
        const id = `sec-${t.v}`;
        return `<section class="card section" aria-labelledby="${id}" data-trigger="${esc(t.v)}">
        <div class="card-head">
          <div><h2 id="${id}">${esc(t.section)}</h2><p class="muted">Checked top to bottom.</p></div>
          <button class="btn sm" type="button" data-op="newrule" data-t="${esc(t.v)}"
            aria-label="+ Add rule ${esc(t.newRule.replace(/^When /, "when "))}">+ Add rule</button>
        </div>
        ${rows.length
                ? `<ol class="rlist">${rows.map(([r, i], pos) => ruleRowHtml(r, i, pos, rows.length)).join("")}</ol>`
                : `<p class="empty">${esc(t.empty)}</p>`
            }
      </section>`;
    }).join("");

    host.innerHTML = `
      <div class="how">
        <button type="button" class="lnk" data-op="how" aria-expanded="${!!state.howOpen}" aria-controls="how-text">How this works</button>
        <p id="how-text" ${state.howOpen ? "" : "hidden"}>${md(SCHEMA.description)}</p>
      </div>
      ${settingsHtml()}
      ${sections}
      <p class="sr-only" id="move-hint">Drag to move it up or down, or use the arrow keys.</p>`;
}

/* ---- one rule ---- */

/* A field's own description, under it, always in view (US-G3). */
const hintHtml = (f, id) => (f.hint ? `<p class="fhint" id="${id}-h">${f.hint}</p>` : "");
const described = (f, id) => (f.hint ? `aria-describedby="${id}-h"` : "");

function params(s, val, path) {
    return s.p
        .filter((f) => f.kind !== "children" && f.kind !== "child")
        .map((f) => {
            const id = `f-${path}-${f.k}`;
            if (f.kind === "int")
                return `<div class="fld"><label for="${id}" class="flabel">${esc(f.label)}</label>
        <input type="number" id="${id}" value="${esc(val[f.k])}" inputmode="numeric"
          min="${f.min ?? 0}" ${f.max != null ? `max="${f.max}"` : ""} ${described(f, id)}
          data-op="num" data-p="${path}" data-k="${f.k}">${hintHtml(f, id)}</div>`;

            if (f.kind === "password") {
                const key = `${path}.${f.k}`, shown = state.revealed.has(key);
                return `<div class="fld wide"><label for="${id}" class="flabel">${esc(f.label)}</label>
        <div class="pw"><input type="${shown ? "text" : "password"}" id="${id}" value="${esc(val[f.k] ?? "")}"
          autocomplete="off" spellcheck="false" ${f.maxLength ? `maxlength="${f.maxLength}"` : ""} ${described(f, id)}
          data-op="text" data-p="${path}" data-k="${f.k}">
        <button type="button" class="btn sm" data-op="reveal" data-key="${key}"
          aria-label="${shown ? "Hide" : "Show"} ${esc(f.label)}">${shown ? "Hide" : "Show"}</button></div>${hintHtml(f, id)}</div>`;
            }

            if (f.kind === "text")
                return `<div class="fld wide"><label for="${id}" class="flabel">${esc(f.label)}</label>
        <input type="text" class="txt" id="${id}" value="${esc(val[f.k] ?? "")}" spellcheck="false"
          ${f.maxLength ? `maxlength="${f.maxLength}"` : ""} ${described(f, id)}
          data-op="text" data-p="${path}" data-k="${f.k}">${hintHtml(f, id)}</div>`;

            if (f.kind === "textarea") {
                const n = String(val[f.k] ?? "").length;
                return `<div class="fld wide"><label for="${id}" class="flabel">${esc(f.label)}
          <span class="count">${n}${f.maxLength ? ` / ${f.maxLength}` : ""}</span></label>
        <textarea class="prose" id="${id}" rows="6" ${f.maxLength ? `maxlength="${f.maxLength}"` : ""} ${described(f, id)}
          data-op="text" data-p="${path}" data-k="${f.k}">${esc(val[f.k] ?? "")}</textarea>${hintHtml(f, id)}</div>`;
            }

            if (f.kind === "choice") {
                const cur = val[f.k], own = typeof cur === "number";
                return `<div class="fld"><label for="${id}" class="flabel">${esc(f.label)}</label>
        <div class="choice"><select id="${id}" data-op="choice" data-p="${path}" data-k="${f.k}" ${described(f, id)}>
          ${f.choices.map((c, i) => `<option value="c${i}" ${cur === c.v ? "selected" : ""}>${esc(c.t)}</option>`).join("")}
          ${f.num ? `<option value="n" ${own ? "selected" : ""}>${esc(f.num.title || "Custom")}</option>` : ""}
        </select>${own
                        ? `<input type="number" id="${id}-n" value="${esc(cur)}" aria-label="${esc(f.label)}, ${esc(f.num.title || "value")}"
          min="${f.min ?? 0}" ${f.max != null ? `max="${f.max}"` : ""}
          data-op="num" data-p="${path}" data-k="${f.k}">`
                        : ""
                    }</div>${hintHtml(f, id)}</div>`;
            }

            if (f.kind === "bool")
                return `<div class="fld wide"><label class="chk"><input type="checkbox" id="${id}" ${val[f.k] ? "checked" : ""}
          data-op="bool" data-p="${path}" data-k="${f.k}" ${described(f, id)}> <span>${esc(f.label)}</span></label>${hintHtml(f, id)}</div>`;

            if (f.kind !== "strlist") return "";

            /* A list of 3 domains and a list of 347 words are the same schema
               kind but need different controls, so size picks the control. */
            const items = val[f.k] || [], key = `${path}.${f.k}`, n = items.length, big = n > BIG_LIST;

            if (state.bulk.has(key))
                return `<div class="fld wide">
          <div class="flabel"><label for="${id}-t">${esc(f.label)}</label> <span class="count">${n}</span>
            <button type="button" class="lnk" data-op="bulk" data-key="${key}" aria-label="Show ${esc(f.label)} as chips">chips</button></div>
          <textarea class="bulk" id="${id}-t" rows="9" spellcheck="false" aria-describedby="${id}-bh"
            data-op="bulktext" data-p="${path}" data-k="${f.k}">${esc(items.join("\n"))}</textarea>
          <p class="fhint" id="${id}-bh">One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale.${f.maxItems ? ` Up to ${f.maxItems} entries${f.maxLength ? `, ${f.maxLength} characters each` : ""}.` : ""
                    }</p></div>`;

            const q = (state.q[key] || "").toLowerCase();
            const shown = items.map((v, j) => [v, j]).filter(([v]) => !q || String(v).toLowerCase().includes(q));
            const chips = shown
                .map(
                    ([v, j]) => `<li class="chip">${esc(v)}<button type="button" data-op="unchip" data-p="${path}" data-k="${f.k}"
              data-j="${j}" aria-label="Remove ${esc(v)}">×</button></li>`
                )
                .join("");
            return `<div class="fld wide">
        <div class="flabel"><label for="${id}">${esc(f.label)}</label> <span class="count">${n}${q ? ` · ${shown.length} shown` : ""}</span>
          <button type="button" class="lnk" data-op="bulk" data-key="${key}" aria-label="Bulk edit ${esc(f.label)}">bulk edit</button></div>
        ${hintHtml(f, id)}
        <div class="listtools">
        <input class="chipin" id="${id}" placeholder="add + Enter" aria-label="Add to ${esc(f.label)}"
          data-op="chip" data-p="${path}" data-k="${f.k}" enterkeyhint="done">
        ${big
                    ? `<input class="qbox" id="${id}-q" type="search" value="${esc(state.q[key] || "")}" placeholder="filter ${n} entries"
              data-op="q" data-key="${key}" aria-label="Filter ${esc(f.label)}">`
                    : ""
                }</div>
        ${n
                    ? `<ul class="chips ${big ? "cap" : ""}" aria-label="${esc(f.label)}">${chips ||
                    `<li class="nores">Nothing matches “${esc(state.q[key])}”.</li>`}</ul>`
                    : ""
                }</div>`;
        })
        .join("");
}

function helpBox(s) {
    return `<div class="help">${s.help || esc(s.full)}</div>`;
}

function badgeIncompatible(s, r) {
    return works(s, r) ? "" : `<span class="badge bad">${esc(tinfo(r).incompatible)}</span>`;
}

function node(n, path, d, r) {
    const s = spec(C, n.type);
    const one = childKeyOf(n), many = childrenKeyOf(n), composite = !!(one || many);
    const collapsed = state.collapsed.has(path);
    const sum = s.sum(n);
    const hasParams = s.p.some((f) => f.kind !== "children" && f.kind !== "child");

    let kids = "";
    if (!collapsed && many) {
        /* The button lives inside its own rail and borrows the tag from the
           type's title — "(AND)", "(OR)" — so it is clear which container it
           adds to. */
        const tag = (s.full.match(/\(([^)]+)\)\s*$/) || [])[1];
        kids = `<div class="kids">${(n[many] || []).map((x, j) => node(x, `${path}.${many}.${j}`, d + 1, r)).join("")}
      <div class="kidfoot"><button class="btn sm ghost" type="button" data-op="pick" data-mode="addchild" data-p="${path}">+ Add to ${tag ? esc(tag) : "this"
            }</button></div></div>`;
    }
    if (!collapsed && one && n[one]) kids = `<div class="kids">${node(n[one], `${path}.${one}`, d + 1, r)}</div>`;

    const items = [
        ["Change type…", `data-op="pick" data-mode="retype" data-p="${path}"`],
        ["Wrap in NOT", `data-op="wrap" data-p="${path}" data-t="Not"`],
        ["Wrap in AND", `data-op="wrap" data-p="${path}" data-t="All"`],
        ...(composite ? [["Unwrap", `data-op="unwrap" data-p="${path}"`]] : []),
        ["Delete", `data-op="del" data-p="${path}"`, "del"],
    ];

    return `<div class="node" data-d="${d}">
    <div class="nrow">
      ${composite || hasParams
            ? `<button class="twist" type="button" data-op="fold" data-p="${path}" aria-expanded="${!collapsed}"
        aria-label="${collapsed ? "Expand" : "Collapse"} “${esc(s.t)}”">${collapsed ? "▸" : "▾"}</button>`
            : `<span class="twist-sp"></span>`
        }
      <span class="glyph" aria-hidden="true">${s.g}</span>
      <span class="nmain"><span class="ntitle">${esc(s.t)}</span>
      ${badgeIncompatible(s, r)}
      ${collapsed && sum ? `<span class="nsum">${esc(sum)}</span>` : ""}</span>
      ${composite && d >= 2 ? `<button class="ico" type="button" data-op="focus" data-p="${path}" aria-label="Show only “${esc(s.t)}”">⤢</button>` : ""}
      <button class="ico" type="button" data-op="help" data-p="${path}" aria-expanded="${state.helped.has(path)}" aria-label="About “${esc(s.t)}”">ⓘ</button>
      ${menuHtml(`n:${path}`, `More for “${s.t}”`, "⋮", items)}
    </div>
    ${state.helped.has(path) ? helpBox(s) : ""}
    ${!collapsed && hasParams ? `<div class="params">${params(s, n, path)}</div>` : ""}
    ${kids}</div>`;
}

/* `covered_by` is either a type name or that name plus the settings that make
   the coverage true — kicking covers moderating the message only when it takes
   the member's history with it. */
function coversThis(cover, action) {
    const type = typeof cover === "string" ? cover : cover.type;
    if (action.type !== type) return false;
    const when = typeof cover === "string" ? null : cover.when;
    return !when || Object.entries(when).every(([k, v]) => action[k] === v);
}

function actionRows(r, ri) {
    /* In the order the bot carries them out, which is not the order the owner
       added them in (US-G3). */
    const order = r.actions.map((a, j) => [a, j]).sort((x, y) => spec(A, x[0].type).rank - spec(A, y[0].type).rank);
    return order
        .map(([a, j]) => {
            const s = spec(A, a.type), path = `rules.${ri}.actions.${j}`;
            const covered = s.coveredBy.some((c) => r.actions.some((x) => coversThis(c, x)));
            return `<div class="arow">
        <div class="nrow">
          <span class="glyph" aria-hidden="true">${s.g}</span>
          <span class="nmain"><span class="ntitle">${esc(s.name)}</span>
          ${badgeIncompatible(s, r)}
          ${covered ? `<span class="badge">covered</span>` : ""}</span>
          <button class="ico" type="button" data-op="help" data-p="${path}" aria-expanded="${state.helped.has(path)}" aria-label="About “${esc(s.name)}”">ⓘ</button>
          ${menuHtml(`a:${path}`, `More for “${s.name}”`, "⋮", [
                ["Change type…", `data-op="pick" data-mode="aretype" data-p="${path}"`],
                ["Delete", `data-op="adel" data-p="${path}"`, "del"],
            ])}
        </div>
        ${covered ? `<p class="fhint">A stronger action on this rule already covers it, so the bot skips this one.</p>` : ""}
        ${state.helped.has(path) ? helpBox(s) : ""}
        ${s.p.length ? `<div class="params">${params(s, a, path)}</div>` : ""}
      </div>`;
        })
        .join("");
}

function renderRule(host) {
    const ri = state.sel, r = state.rules[ri], t = tinfo(r);
    const root = `rules.${ri}.condition`;
    const focused = state.focus && state.focus.startsWith(root + ".") ? state.focus : null;
    const title = titleOf(r), issue = issueOf(r);
    const nameF = RULE.name, whenF = RULE.trigger;

    const nodes = nodesOf(r.condition), depth = depthOf(r.condition);
    const over = nodes > state.limits.nodes || depth > state.limits.depth;
    const condDef = SCHEMA.definitions.condition, actDef = SCHEMA.properties.rules.items.properties.actions;
    const back = esc(state.groupName || pageTitle());

    /* A single condition that is not a container gains a sibling by turning
       into an AND of both; a container has its own "+ Add to …". */
    const rootMany = r.condition && childrenKeyOf(r.condition);
    const addCond = !r.condition || !rootMany
        ? `<button class="btn sm" type="button" data-op="pick" data-mode="addroot">+ Add condition</button>` : "";

    const tree = !r.condition
        ? `<p class="everyone">${t.optional ? esc(t.everyone) : "No condition yet."}</p>`
        : focused
            ? `<div class="crumbs"><button type="button" class="lnk" data-op="unfocus">Whole condition</button> › <span>part of it</span></div>${node(at(focused), focused, 0, r)}`
            : node(r.condition, root, 0, r);

    host.innerHTML = `
    <nav class="pagehead"><button class="back" type="button" data-op="tooverview">‹ ${back}</button></nav>
    <div class="rulehead">
      <h2 class="${r.name ? "" : "phrase"}">${esc(title)}</h2>
      <div class="tools">
        <button class="btn" type="button" data-op="ruleai">✨ Edit with AI</button>
        ${menuHtml("rule", "More for this rule", "⋯", [
            ["Duplicate", `data-op="dup"`],
            ["Delete", `data-op="ruledel"`, "del"],
        ])}
      </div>
    </div>
    ${isOn(r) ? "" : `<div class="notice off" role="status"><span>⏸ This rule is off — the bot skips it.</span>
      <button class="btn sm" type="button" data-op="toggle" data-i="${ri}" aria-label="Turn on">Turn on</button></div>`}
    ${issue ? `<div class="notice warn" role="status">${esc(t.incomplete)}</div>` : ""}

    <section class="card" aria-label="Rule">
      <div class="params">
        <div class="fld wide"><label for="r-name" class="flabel">${esc(nameF.title)}</label>
          <input type="text" class="txt" id="r-name" maxlength="${nameF.maxLength || 60}" value="${esc(r.name || "")}"
            placeholder="${esc(issue ? "New rule" : rulePhrase(r))}" aria-describedby="r-name-h" data-op="rname">
          <p class="fhint" id="r-name-h">${md(nameF.description || "")}</p></div>
        <div class="fld wide"><label for="r-when" class="flabel">${esc(whenF.title)}</label>
          <select id="r-when" data-op="rtrigger" aria-describedby="r-when-h">
            ${T.map((x) => `<option value="${esc(x.v)}" ${x.v === trig(r) ? "selected" : ""}>${esc(x.title)}</option>`).join("")}
          </select>
          <p class="fhint" id="r-when-h">${md(whenF.description || "")}</p></div>
      </div>
    </section>

    <section class="card block" aria-labelledby="b-if">
      <div class="bhead if"><span class="tag" aria-hidden="true">If</span>
        <h3 id="b-if">${esc(condDef.title.replace(/^\S+\s/, ""))}</h3>
        <span class="tools">
          <button class="ico" type="button" data-op="help" data-p="block-if" aria-expanded="${state.helped.has("block-if")}" aria-label="About conditions">ⓘ</button>
          ${r.condition && (isComposite(r.condition) || spec(C, r.condition.type).p.length)
            ? `<button class="btn sm ghost" type="button" data-op="foldall">${[...state.collapsed].some((x) => x.startsWith(root)) ? "Expand all" : "Collapse all"}</button>`
            : ""}
        </span></div>
      ${state.helped.has("block-if") ? `<div class="help">${md(condDef.description)}</div>` : ""}
      ${over
            ? `<div class="notice warn">This rule has ${nodes} conditions nested ${depth} levels deep. The bot accepts at most ${state.limits.nodes} conditions and ${state.limits.depth} levels, so simplify it before applying — otherwise the rules will be rejected.</div>`
            : ""
        }
      ${tree}
      ${addCond ? `<div class="bfoot">${addCond}</div>` : ""}
    </section>

    <section class="card block" aria-labelledby="b-then">
      <div class="bhead then"><span class="tag" aria-hidden="true">Then</span>
        <h3 id="b-then">${esc(actDef.title.replace(/^\S+\s/, ""))}</h3>
        <span class="tools">
          <button class="ico" type="button" data-op="help" data-p="block-then" aria-expanded="${state.helped.has("block-then")}" aria-label="About actions">ⓘ</button>
        </span></div>
      ${state.helped.has("block-then") ? `<div class="help">${md(actDef.description)}</div>` : ""}
      ${r.actions.length ? `<p class="muted ordnote">The bot runs them in this order.</p>${actionRows(r, ri)}` : `<p class="everyone">No action yet.</p>`}
      <div class="bfoot"><button class="btn sm" type="button" data-op="pick" data-mode="addaction">+ Add action</button></div>
    </section>`;
}

/* ---- New rule ---- */

const templatesFor = (v) => ((SCHEMA.options && SCHEMA.options.templates) || []).filter((x) => (x.rule.trigger || T[0].v) === v);

function renderNew(host) {
    const t = T.find((x) => x.v === state.newTrigger) || T[0];
    const back = esc(state.groupName || pageTitle());
    const temps = templatesFor(t.v);
    host.innerHTML = `
    <nav class="pagehead"><button class="back" type="button" data-op="tooverview">‹ ${back}</button></nav>
    <div class="rulehead"><div><h2>New rule</h2><p class="muted">${esc(t.newRule)}</p></div></div>
    <div class="ways">
      <button class="way" type="button" data-op="templates" aria-expanded="${state.showTemplates}">
        <span class="wayicon" aria-hidden="true">📋</span><span><b>Start from a template</b>
        <span class="muted">A ready rule to adjust.</span></span></button>
      <button class="way" type="button" data-op="createai">
        <span class="wayicon" aria-hidden="true">✨</span><span><b>Describe it to AI</b>
        <span class="muted">Say what it should do; your AI chat writes it.</span></span></button>
      <button class="way" type="button" data-op="build">
        <span class="wayicon" aria-hidden="true">🛠</span><span><b>Build it myself</b>
        <span class="muted">Pick its condition and actions.</span></span></button>
    </div>
    ${state.showTemplates
            ? `<section class="card" aria-labelledby="tpl-h"><div class="card-head"><h2 id="tpl-h">Templates</h2></div>
        <ul class="tlist">${temps
                .map(
                    (x, j) => `<li><button class="tpl" type="button" data-op="usetpl" data-j="${j}">
            <span class="ttitle">${esc(x.title)}</span>
            <span class="tphrase">${esc(rulePhrase(x.rule))}</span></button></li>`
                )
                .join("")}</ul></section>`
            : ""
        }`;
}

/* ---- the frame ---- */

const pageTitle = () => (state.groupName ? state.groupName : state.botId ? `Group ${state.botId}` : "Group settings");

/* The changes the Apply button counts: each changed setting and each rule
   added, removed, moved or changed (US-G7). */
function changeCount() {
    const d = diffConfig(state.baseline, config());
    return d.settings.length + d.rows.filter((x) => x.status !== "same").length;
}

function renderFrame() {
    document.getElementById("gtitle").textContent = pageTitle();
    /* On a rule's page and New rule, the way back names the group. */
    document.querySelector(".titles").hidden = state.view !== "overview";
    document.title = `${pageTitle()} — Moderation settings`;
    /* Hidden rather than greyed out: a control that can do nothing is noise, and
       the phone's action bar has room for three buttons, not five. */
    document.getElementById("undo").hidden = !state.history.length;
    document.getElementById("redo").hidden = !state.future.length;
    const n = changeCount();
    document.getElementById("apply").textContent = n ? `Apply ${n} change${n === 1 ? "" : "s"}` : "Apply changes";
    document.getElementById("gmenu-wrap").hidden = state.view !== "overview";
    const gm = document.getElementById("gmenu");
    gm.hidden = state.menu !== "global";
    document.querySelector('[data-p="global"]').setAttribute("aria-expanded", state.menu === "global");

    /* A setting value may carry a banner shown on every page while it holds
       (US-G2: Dry mode). */
    const banners = S.p
        .filter((f) => f.kind === "choice")
        .map((f) => (f.choices.find((c) => c.v === state.settings[f.k]) || { o: {} }).o.banner)
        .filter(Boolean);
    const banner = document.getElementById("banner");
    banner.hidden = !banners.length;
    banner.textContent = banners.join(" ");
}

function render() {
    if (state.view === "rule" && !state.rules[state.sel]) state.view = "overview";
    const host = document.getElementById("app");
    host.dataset.view = state.view;
    renderFrame();
    if (state.view === "rule") renderRule(host);
    else if (state.view === "new") renderNew(host);
    else renderOverview(host);
    syncHash();
}

/* After a render, put the focus back where it was, by id. */
function refocus(id, caretEnd) {
    const again = id && document.getElementById(id);
    if (!again) return;
    again.focus();
    if (caretEnd && again.setSelectionRange) again.setSelectionRange(again.value.length, again.value.length);
}

/* ------------------------------- picker -------------------------------- */

const pick = { mode: null, p: null };
const pickDlg = document.getElementById("pickdlg");

/* Lists the types a rule's trigger takes, grouped by subject (US-G3, US-G4). */
function openPicker(mode, p) {
    const r = state.rules[state.sel];
    const actions = mode === "addaction" || mode === "aretype";
    const reg = actions ? A : C;
    pick.mode = mode;
    pick.p = p;
    const current = mode === "retype" || mode === "aretype" ? at(p).type : null;
    const taken = actions ? new Set(r.actions.map((a) => a.type)) : new Set();
    const groups = {};
    for (const [k, s] of Object.entries(reg)) {
        if (!works(s, r)) continue;
        if (actions && taken.has(k) && k !== current) continue;
        (groups[s.grp] = groups[s.grp] || []).push([k, s]);
    }
    document.getElementById("pickdlg-title").textContent =
        mode === "retype" ? "Change the condition's type" : mode === "aretype" ? "Change the action's type"
            : actions ? "Pick an action" : "Pick a condition";
    const body = Object.entries(groups)
        .map(
            ([grp, items]) => `<section class="pgroup" aria-label="${esc(grp)}"><h3>${esc(grp)}</h3>
        <ul>${items
                    .map(
                        ([k, s]) => `<li><button type="button" class="ptype${k === current ? " cur" : ""}" data-type="${esc(k)}"
            ${k === current ? 'aria-current="true"' : ""}>
            <span class="glyph" aria-hidden="true">${s.g}</span>
            <span><span class="pname">${esc(s.name)}</span><span class="pgist">${s.gist}</span></span></button></li>`
                    )
                    .join("")}</ul></section>`
        )
        .join("");
    document.getElementById("pickdlg-body").innerHTML =
        body || `<p class="empty">Every action this rule can take is on it already.</p>`;
    pickDlg.showModal();
}

document.getElementById("pickdlg-x").addEventListener("click", () => pickDlg.close());

pickDlg.addEventListener("click", (e) => {
    const b = e.target.closest("[data-type]");
    if (!b) return;
    const type = b.dataset.type, ri = state.sel, r = state.rules[ri];
    const before = snapshot();
    const root = `rules.${ri}.condition`;
    switch (pick.mode) {
        case "addroot": {
            const n = fresh(C, type);
            if (!r.condition) r.condition = n;
            else {
                const and = andType();
                r.condition = { type: and, ...clone(C[and].def), [childrenKeyOf({ type: and })]: [r.condition, n] };
            }
            state.collapsed.delete(root);
            break;
        }
        case "addchild": {
            const n = at(pick.p);
            n[childrenKeyOf(n)].push(fresh(C, type));
            state.collapsed.delete(pick.p);
            break;
        }
        case "retype": {
            const { o, k } = parentOf(pick.p);
            o[k] = retype(o[k], type, C);
            state.collapsed.delete(pick.p);
            break;
        }
        case "addaction":
            r.actions.push(fresh(A, type));
            break;
        case "aretype": {
            const { o, k } = parentOf(pick.p);
            o[k] = retype(o[k], type, A);
            break;
        }
    }
    pickDlg.close();
    recordUndo(before);
    render();
});

/* -------------------------------- events ------------------------------- */

function moveRule(i, d) {
    const sec = sectionOf(trig(state.rules[i])).map(([, j]) => j);
    const pos = sec.indexOf(i), to = sec[pos + d];
    if (to === undefined) return null;
    [state.rules[i], state.rules[to]] = [state.rules[to], state.rules[i]];
    return to;
}

/* Puts a section's rules in a new order, in the places its rules held. */
function reorderSection(v, order) {
    const places = sectionOf(v).map(([, j]) => j);
    const rules = order.map((j) => state.rules[j]);
    places.forEach((p, k) => (state.rules[p] = rules[k]));
}

function addRule(rule) {
    state.rules.push(rule);
    return state.rules.length - 1;
}

document.addEventListener("click", (e) => {
    if (e.target.closest("dialog")) return;
    const b = e.target.closest("[data-op]");
    const wasOpen = state.menu;
    if (!b || b.dataset.op !== "menu") state.menu = null;
    if (!b) {
        if (wasOpen) render();
        return;
    }
    const op = b.dataset.op, p = b.dataset.p;
    const before = snapshot();

    switch (op) {
        case "undo":
            undo();
            return;
        case "redo":
            redo();
            return;
        case "open":
            goTo("rule", { sel: +b.dataset.i });
            break;
        case "tooverview":
            goTo("overview");
            break;
        case "newrule":
            goTo("new", { trigger: b.dataset.t });
            break;
        case "how":
            state.howOpen = !state.howOpen;
            break;
        case "templates":
            state.showTemplates = !state.showTemplates;
            break;
        case "usetpl": {
            const x = templatesFor(state.newTrigger)[+b.dataset.j];
            const i = addRule(clone(x.rule));
            recordUndo(before);
            goTo("rule", { sel: i }, true);
            render();
            /* A template with an empty list opens on it, to fill in (US-G15). */
            if (x.focus) refocus(`f-rules.${i}.condition-${x.focus}`);
            return;
        }
        case "build": {
            const i = addRule({ trigger: state.newTrigger, actions: [] });
            recordUndo(before);
            goTo("rule", { sel: i }, true);
            render();
            openPicker("addroot");
            return;
        }
        case "createai":
            document.dispatchEvent(new CustomEvent("ai:create", { detail: { trigger: state.newTrigger } }));
            return;
        case "ruleai":
            document.dispatchEvent(new CustomEvent("ai:rule", { detail: { index: state.sel } }));
            return;
        case "toggle": {
            const r = state.rules[+b.dataset.i];
            if (isOn(r)) r.enabled = false;
            else delete r.enabled;
            break;
        }
        case "rmove": {
            moveRule(+b.dataset.i, +b.dataset.d);
            break;
        }
        case "dup": {
            const r = state.rules[state.sel];
            /* The copy's name stays within the 60 characters a name can have. */
            const base = titleOf(r), max = (RULE.name.maxLength || 60) - " (copy)".length;
            const copy = { ...clone(r), name: `${base.length > max ? base.slice(0, max - 1) + "…" : base} (copy)` };
            state.rules.splice(state.sel + 1, 0, copy);
            recordUndo(before);
            goTo("rule", { sel: state.sel + 1 });
            render();
            return;
        }
        /* No confirmation: Undo takes it back, and the Apply dialog shows the
           deletion before anything reaches the bot. */
        case "ruledel":
            state.rules.splice(state.sel, 1);
            recordUndo(before);
            goTo("overview");
            render();
            return;
        case "fold":
            state.collapsed.has(p) ? state.collapsed.delete(p) : state.collapsed.add(p);
            break;
        case "foldall": {
            const root = `rules.${state.sel}.condition`, set = new Set(state.collapsed);
            if ([...set].some((x) => x.startsWith(root))) [...set].forEach((x) => x.startsWith(root) && set.delete(x));
            else foldAll(state.rules[state.sel].condition, root, set);
            state.collapsed = set;
            break;
        }
        case "help":
            state.helped.has(p) ? state.helped.delete(p) : state.helped.add(p);
            break;
        case "menu":
            state.menu = wasOpen === p ? null : p;
            break;
        case "focus":
            state.focus = p;
            break;
        case "unfocus":
            state.focus = null;
            break;
        case "reveal": {
            const k = b.dataset.key;
            state.revealed.has(k) ? state.revealed.delete(k) : state.revealed.add(k);
            break;
        }
        case "bulk": {
            const k = b.dataset.key;
            state.bulk.has(k) ? state.bulk.delete(k) : state.bulk.add(k);
            delete state.q[k];
            break;
        }
        case "pick":
            render();
            openPicker(b.dataset.mode, p);
            return;
        /* Wrapping is never a one-way door: a container can be unwrapped. */
        case "unwrap": {
            const node = at(p);
            const one = childKeyOf(node), many = childrenKeyOf(node);
            const kids = one ? [node[one]] : (node[many] || []).slice();
            if (!kids.length) break;
            const { o, k } = parentOf(p);
            if (Array.isArray(o)) o.splice(+k, 1, ...kids);
            else if (kids.length === 1) o[k] = kids[0];
            else {
                toast("This holds several conditions and sits at the top of the rule, which has room for one. Move them out first.");
                break;
            }
            state.focus = null;
            break;
        }
        case "wrap": {
            const t = b.dataset.t, { o, k } = parentOf(p);
            const wrapper = { type: t, ...structuredClone(C[t].def) };
            const one = (C[t].p.find((f) => f.kind === "child") || {}).k;
            const many = (C[t].p.find((f) => f.kind === "children") || {}).k;
            if (one) wrapper[one] = o[k];
            else if (many) wrapper[many] = [o[k]];
            o[k] = wrapper;
            state.focus = null;
            break;
        }
        case "del": {
            const { o, k } = parentOf(p);
            /* The rule's own condition: without it, a rule whose trigger needs
               one is incomplete until it gets another (US-G14). */
            if (!Array.isArray(o)) {
                delete o[k];
                state.focus = null;
                break;
            }
            if (o.length === 1) {
                toast("A combined condition needs at least one condition inside.");
                break;
            }
            o.splice(+k, 1);
            state.focus = null;
            break;
        }
        case "adel": {
            const { o, k } = parentOf(p);
            o.splice(+k, 1);
            break;
        }
        case "unchip":
            at(b.dataset.p)[b.dataset.k].splice(+b.dataset.j, 1);
            break;
        default:
            return;
    }
    recordUndo(before);
    render();
    /* A menu opened by keyboard puts the focus on its first item. */
    if (op === "menu" && state.menu) {
        const btn = document.querySelector(`[data-op="menu"][data-p="${CSS.escape(state.menu)}"]`);
        const first = btn && btn.parentElement.querySelector(".pop [role=menuitem]");
        if (first && e.detail === 0) first.focus();
    }
});

document.addEventListener("change", (e) => {
    if (e.target.closest("dialog")) return;
    const b = e.target.closest("[data-op]");
    if (!b) return;
    const op = b.dataset.op, p = b.dataset.p, key = b.dataset.k;
    const before = snapshot();
    const r = state.rules[state.sel];

    if (op === "rname") {
        const v = b.value.trim();
        if (v) r.name = v;
        else delete r.name;
    } else if (op === "rtrigger") {
        /* The rule moves to the end of its new section (US-G3). */
        r.trigger = b.value;
        state.rules.splice(state.sel, 1);
        state.sel = addRule(r);
        history.replaceState({ v: "rule", i: state.sel }, "", location.hash);
    } else if (op === "num") {
        const f = fieldsAt(p).find((x) => x.k === key) || {};
        let v = Math.round(+b.value || 0);
        if (f.min != null) v = Math.max(f.min, v);
        if (f.max != null) v = Math.min(f.max, v);
        at(p)[key] = v;
    } else if (op === "bool") {
        at(p)[key] = b.checked;
    } else if (op === "text") {
        at(p)[key] = b.value;
    } else if (op === "choice") {
        const f = fieldsAt(p).find((x) => x.k === key) || {};
        const cur = at(p)[key];
        /* Switching to a number starts from the branch's default. */
        if (b.value === "n") at(p)[key] = typeof cur === "number" ? cur : f.num.default ?? f.min ?? 0;
        else at(p)[key] = f.choices[+b.value.slice(1)].v;
    } else if (op === "bulktext") {
        const seen = new Set(), out = [];
        b.value.split("\n").map((x) => x.trim()).filter(Boolean).forEach((x) => {
            if (!seen.has(x)) { seen.add(x); out.push(x); }
        });
        at(p)[key] = out;
    } else return;
    recordUndo(before);
    /* Rendered once the focus has moved on — a Tab out of the field — and
       put back on whatever took it. */
    setTimeout(() => {
        const id = document.activeElement && document.activeElement.id;
        render();
        refocus(id);
    });
});

/* Filtering a long list re-renders it, so put the caret back where it was. */
document.addEventListener("input", (e) => {
    const b = e.target.closest('[data-op="q"]');
    if (!b) return;
    state.q[b.dataset.key] = b.value;
    const id = b.id;
    render();
    refocus(id, true);
});

document.addEventListener("keydown", (e) => {
    const b = e.target.closest('[data-op="chip"]');
    if (!b || e.key !== "Enter") return;
    e.preventDefault();
    const v = b.value.trim();
    if (!v) return;
    const list = at(b.dataset.p)[b.dataset.k];
    if (list.includes(v)) {
        b.value = "";
        toast("That entry is already in the list.");
        return;
    }
    const before = snapshot();
    list.push(v);
    /* A filter that hides the entry just added looks like nothing happened. */
    const key = `${b.dataset.p}.${b.dataset.k}`;
    if (state.q[key] && !v.toLowerCase().includes(state.q[key].toLowerCase())) delete state.q[key];
    const id = b.id;
    recordUndo(before);
    render();
    refocus(id);
});

/* Escape closes an open menu and gives the focus back to its button. */
document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape" || !state.menu) return;
    const id = state.menu;
    state.menu = null;
    render();
    const btn = document.querySelector(`[data-op="menu"][data-p="${CSS.escape(id)}"]`);
    if (btn) btn.focus();
});

/* Ctrl/Cmd+Z and its redo spellings, outside a text field: inside one the
   browser's own undo wins. */
document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    const isUndo = key === "z" && !e.shiftKey;
    const isRedo = (key === "z" && e.shiftKey) || key === "y";
    if (!isUndo && !isRedo) return;
    if (e.target.closest("input, textarea")) return;
    e.preventDefault();
    isUndo ? undo() : redo();
});

/* ----- moving a rule on the overview: its handle, by pointer or keys ----- */

/* The arrow keys on a focused handle move the rule (US-G12). */
document.addEventListener("keydown", (e) => {
    const h = e.target.closest("[data-drag]");
    if (!h || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    const before = snapshot();
    const to = moveRule(+h.dataset.drag, e.key === "ArrowUp" ? -1 : 1);
    if (to === null) return;
    recordUndo(before);
    render();
    const again = document.querySelector(`[data-drag="${to}"]`);
    if (again) again.focus();
});

/* Dragging works with a mouse and a finger alike: pointer events, the row
   following the pointer, and the order settled when it is let go. */
let drag = null;
document.addEventListener("pointerdown", (e) => {
    const h = e.target.closest("[data-drag]");
    if (!h || e.button > 0) return;
    const row = h.closest(".rrow"), list = row.parentElement;
    const rows = [...list.children];
    drag = { row, list, rows, startY: e.clientY, from: rows.indexOf(row), to: rows.indexOf(row), moved: false, id: e.pointerId };
    h.setPointerCapture(e.pointerId);
});

document.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const dy = e.clientY - drag.startY;
    if (Math.abs(dy) > 4) drag.moved = true;
    if (!drag.moved) return;
    e.preventDefault();
    drag.row.classList.add("dragging");
    drag.row.style.transform = `translateY(${dy}px)`;
    const mid = drag.row.getBoundingClientRect().top + drag.row.offsetHeight / 2;
    let to = 0;
    drag.rows.forEach((r, k) => {
        if (r === drag.row) return;
        const box = r.getBoundingClientRect();
        if (mid > box.top + box.height / 2) to = k < drag.from ? k + 1 : k;
    });
    if (mid < drag.rows[0].getBoundingClientRect().top + drag.rows[0].offsetHeight / 2) to = 0;
    drag.to = Math.max(0, Math.min(drag.rows.length - 1, to));
    drag.rows.forEach((r, k) => r.classList.toggle("drop-before", k === drag.to && drag.to < drag.from && r !== drag.row));
    drag.rows.forEach((r, k) => r.classList.toggle("drop-after", k === drag.to && drag.to > drag.from && r !== drag.row));
});

document.addEventListener("pointerup", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const d = drag;
    drag = null;
    if (!d.moved || d.to === d.from) {
        d.row.style.transform = "";
        d.row.classList.remove("dragging");
        return;
    }
    const before = snapshot();
    const v = d.list.closest("[data-trigger]").dataset.trigger;
    const order = sectionOf(v).map(([, j]) => j);
    const [moved] = order.splice(d.from, 1);
    order.splice(d.to, 0, moved);
    reorderSection(v, order);
    recordUndo(before);
    render();
});

/* --------------------------------- diff --------------------------------- */

/* Rules carry no id, so comparing by position lies: insert one rule at the top
   and everything below it looks changed. Identical rules are paired first, and
   only what is left is matched up — by resemblance, not by position. Each
   section is its own list (US-D1, US-D6), and a rule that changed sections is
   found across them (US-G7). */
function diffConfig(before, after) {
    const settings = S.p.filter((f) => canon(before[f.k]) !== canon(after[f.k]));
    const rows = [];
    const left = [], right = [];
    for (const t of T) {
        const l = before.rules.filter((r) => trig(r) === t.v), rr = after.rules.filter((r) => trig(r) === t.v);
        const d = diffRules(l, rr, t.v);
        rows.push(...d.rows.filter((x) => x.status !== "added" && x.status !== "removed"));
        left.push(...d.rows.filter((x) => x.status === "removed"));
        right.push(...d.rows.filter((x) => x.status === "added"));
    }
    /* What left one section and arrived in another is the same rule, moved by
       its When, when the two resemble each other. */
    const claimed = new Set();
    for (const b of right) {
        let best = -1, score = 0;
        left.forEach((a, ix) => {
            if (claimed.has(ix) || a.trigger === b.trigger) return;
            const s = resemblance(a.rule, b.rule);
            if (s > score) { score = s; best = ix; }
        });
        if (best >= 0 && score >= 2) {
            claimed.add(best);
            const was = left[best].rule;
            rows.push({ ...b, status: "changed", was, wasTrigger: trig(was), details: ruleDetails(was, b.rule) });
        } else rows.push(b);
    }
    left.forEach((a, ix) => !claimed.has(ix) && rows.push(a));
    return { settings, rows };
}

function resemblance(a, b) {
    return (
        (a.name && a.name === b.name ? 5 : 0) +
        (a.condition && b.condition && a.condition.type === b.condition.type ? 3 : 0) +
        (actionWords(a) === actionWords(b) ? 2 : 0)
    );
}

/* What a paired rule's change is called: a rename, an on/off switch, or a
   change of anything else (US-G7). */
function changeKind(was, now) {
    const strip = (r) => {
        const { name, enabled, ...rest } = r;
        return canon(rest);
    };
    if (strip(was) !== strip(now)) return "changed";
    const renamed = (was.name || "") !== (now.name || ""), switched = isOn(was) !== isOn(now);
    if (renamed && switched) return "changed";
    if (switched) return isOn(now) ? "turned on" : "turned off";
    return "renamed";
}

function diffRules(before, after, trigger) {
    const left = before.map((r, i) => ({ r, i, key: canon(r) }));
    const right = after.map((r, i) => ({ r, i, key: canon(r) }));
    const usedL = new Set(), pairs = [];
    for (const b of right) {
        const m = left.find((a) => !usedL.has(a.i) && a.key === b.key);
        if (m) {
            usedL.add(m.i);
            pairs.push({ l: m, r: b });
        }
    }

    /* Of the untouched rules, the longest run that kept its relative order is
       what "stayed put"; the rest are what actually moved. */
    const inPlace = longestIncreasing(pairs.map((p) => p.l.i));
    const rows = pairs.map((p, idx) => ({
        status: inPlace.has(idx) ? "same" : "moved",
        index: p.r.i,
        from: p.l.i,
        rule: p.r.r,
        trigger,
    }));

    const usedR = new Set(pairs.map((p) => p.r.i));
    const restL = left.filter((a) => !usedL.has(a.i));
    const restR = right.filter((b) => !usedR.has(b.i));
    const claimed = new Set();
    for (const b of restR) {
        let best = -1, score = 0;
        restL.forEach((a, ix) => {
            if (claimed.has(ix)) return;
            const s = resemblance(a.r, b.r);
            if (s > score) {
                score = s;
                best = ix;
            }
        });
        /* Same name, same detector or same actions means this is the same rule,
           edited. Anything less is a different rule that simply took its place. */
        if (best >= 0 && score >= 2) {
            claimed.add(best);
            const was = restL[best].r;
            rows.push({ status: changeKind(was, b.r), index: b.i, rule: b.r, was, trigger, details: ruleDetails(was, b.r) });
        } else rows.push({ status: "added", index: b.i, rule: b.r, trigger });
    }
    restL.forEach((a, ix) => {
        if (!claimed.has(ix)) rows.push({ status: "removed", index: a.i, rule: a.r, trigger });
    });

    rows.sort((a, b) => a.index - b.index);
    return { rows };
}

/* Indices of a longest increasing subsequence, as a Set. */
function longestIncreasing(seq) {
    const best = seq.map(() => 1), prev = seq.map(() => -1);
    let endAt = -1, longest = 0;
    for (let i = 0; i < seq.length; i++) {
        for (let j = 0; j < i; j++)
            if (seq[j] < seq[i] && best[j] + 1 > best[i]) {
                best[i] = best[j] + 1;
                prev[i] = j;
            }
        if (best[i] > longest) {
            longest = best[i];
            endAt = i;
        }
    }
    const out = new Set();
    for (let i = endAt; i >= 0; i = prev[i]) out.add(i);
    return out;
}

/* A secret as the diff shows it: its last four characters. */
const secretHint = (v) => (v ? `…${String(v).slice(-4)}` : "empty");

const label = (n) => `${spec(C, n.type).g} ${spec(C, n.type).t}`;

function ruleDetails(a, b) {
    const out = [];
    if ((a.name || "") !== (b.name || "") && changeKind(a, b) === "changed")
        out.push({ text: `name: “${a.name || ""}” → “${b.name || ""}”` });
    if (isOn(a) !== isOn(b) && changeKind(a, b) === "changed") out.push({ text: isOn(b) ? "turned on" : "turned off" });
    if (actionWords(a) !== actionWords(b)) out.push({ text: `actions: ${actionWords(a)} → ${actionWords(b)}` });
    if (!a.condition !== !b.condition) out.push({ text: `condition: ${condWords(a)} → ${condWords(b)}` });
    else if (a.condition) conditionDetails(a.condition, b.condition, out);
    return out;
}

/* Walks two condition trees side by side. A shrinking list is called out
   separately: it is the one change that would otherwise slip through. */
function conditionDetails(a, b, out, where = "") {
    if (!a || !b || out.length > 8) return;
    if (a.type !== b.type) {
        out.push({ text: `${spec(C, a.type).t} → ${spec(C, b.type).t}` });
        return;
    }
    for (const f of spec(C, a.type).p) {
        const x = a[f.k], y = b[f.k];
        if (f.kind === "child") conditionDetails(x, y, out, where);
        else if (f.kind === "children") childrenDetails(spec(C, a.type).t, x || [], y || [], out);
        else if (f.kind === "strlist") {
            /* A net "+1" can hide five entries leaving and six arriving, so the
               two directions are counted separately. */
            const was = x || [], now = y || [];
            if (canon(was) === canon(now)) continue;
            const wasSet = new Set(was), nowSet = new Set(now);
            const removed = was.filter((v) => !nowSet.has(v));
            const added = now.filter((v) => !wasSet.has(v));
            if (!removed.length && !added.length) {
                out.push({ text: `${where}${f.k}: same ${was.length} entries, reordered` });
                continue;
            }
            const parts = [];
            if (removed.length) parts.push(`${removed.length} removed`);
            if (added.length) parts.push(`${added.length} added`);
            out.push({
                text: `${where}${f.k}: ${parts.join(", ")} (${was.length} → ${now.length})`,
                alarm: removed.length > 0,
                removed,
                added,
            });
        } else if (f.kind === "password") {
            if (x !== y) out.push({ text: `${where}${f.k}: replaced (${secretHint(x)} → ${secretHint(y)})` });
        } else if (f.kind === "textarea") {
            if (x !== y) out.push({ text: `${where}${f.k}: rewritten (${String(x ?? "").length} → ${String(y ?? "").length} characters)` });
        } else if (f.kind === "choice") {
            if (x !== y) out.push({ text: `${where}${f.label}: ${choiceText(f, x)} → ${choiceText(f, y)}` });
        } else if (x !== y) out.push({ text: `${where}${f.k}: ${x} → ${y}` });
    }
}

/* A composite's children carry no id either, so the rules' own pairing applies
   one level down: identical children first, then what is left, by type. Order
   inside All/Any changes nothing the bot does, so a pure reorder says
   nothing. */
function childrenDetails(title, xs, ys, out) {
    const left = xs.map((n, i) => ({ n, i, key: canon(n) }));
    const right = ys.map((n, i) => ({ n, i, key: canon(n) }));
    const same = new Set();
    const restR = right.filter((b) => {
        const m = left.find((a) => !same.has(a.i) && a.key === b.key);
        if (m) same.add(m.i);
        return !m;
    });

    const edited = new Set(), pairs = [], added = [];
    for (const b of restR) {
        const m = left.find((a) => !same.has(a.i) && !edited.has(a.i) && a.n.type === b.n.type);
        if (m) {
            edited.add(m.i);
            pairs.push([m.n, b.n]);
        } else added.push(b.n);
    }
    const removed = left.filter((a) => !same.has(a.i) && !edited.has(a.i)).map((a) => a.n);

    if (xs.length !== ys.length) out.push({ text: `${title}: ${xs.length} → ${ys.length} conditions` });
    for (const n of removed) out.push({ text: `removed: ${label(n)}`, node: n, alarm: true });
    for (const n of added) {
        if (out.length > 8) break;
        out.push({ text: `added: ${label(n)}`, node: n });
    }
    for (const [a, b] of pairs) conditionDetails(a, b, out, `${spec(C, b.type).t} · `);
}

/* ----------------------------- apply dialog ----------------------------- */

const dlg = document.getElementById("applydlg");

/* How many entries one side of a list change shows before it stops listing. */
const LIST_PREVIEW = 40;

function chipList(items, kind) {
    return (
        items
            .slice(0, LIST_PREVIEW)
            .map((v) => `<span class="dlchip ${kind}">${esc(v)}</span>`)
            .join("") +
        (items.length > LIST_PREVIEW ? `<span class="dlmore">+${items.length - LIST_PREVIEW} more</span>` : "")
    );
}

/* A list change is a line like any other until you click it. */
function detailHtml(x) {
    if (x.node) return nodeHtml(x);
    if (!x.removed && !x.added) return `<span class="d${x.alarm ? " alarm" : ""}">${esc(x.text)}</span>`;
    return `<details class="dlist"><summary class="d${x.alarm ? " alarm" : ""}">${esc(x.text)}</summary>
    <div class="dlbody">
      ${x.removed.length
            ? `<div class="dlgroup"><span class="dlk rm">removed</span><span class="dlchips">${chipList(x.removed, "rm")}</span></div>`
            : ""
        }
      ${x.added.length
            ? `<div class="dlgroup"><span class="dlk ad">added</span><span class="dlchips">${chipList(x.added, "ad")}</span></div>`
            : ""
        }
    </div></details>`;
}

function nodeHtml(x) {
    const n = x.node, kind = x.alarm ? "rm" : "ad";
    const fields = spec(C, n.type).p;
    const head = `<span class="d${x.alarm ? " alarm" : ""}">${esc(x.text)}</span>`;
    if (!fields.length) return head;
    const lists = fields
        .filter((f) => f.kind === "strlist" && (n[f.k] || []).length)
        .map(
            (f) =>
                `<div class="dlgroup"><span class="dlk ${kind}">${esc(f.k)}</span><span class="dlchips">${chipList(n[f.k], kind)}</span></div>`
        )
        .join("");
    return `<details class="dlist"><summary class="d${x.alarm ? " alarm" : ""}">${esc(x.text)}</summary>
    <div class="dlbody"><span class="d">${esc(say(n))}</span>${lists}</div></details>`;
}

function ruleLine(r) {
    const name = r.name || (issueOf(r) ? "New rule" : "");
    const phrase = issueOf(r) && !r.condition && !(r.actions || []).length
        ? ""
        : `<span class="kw if">IF</span> ${esc(condWords(r))} <span class="kw then">THEN</span> ${esc(actionWords(r) || "—")}`;
    return `${name ? `<b class="dname">${esc(name)}</b> ` : ""}${phrase}`;
}

/* The bot reads the link out of one chat message, so the message is the budget:
   `MESSAGE_MAX_LENGTH_IN_BYTES` in drivers/simplex/consts.rs, mirrored into the
   schema. */
function linkWarning() {
    const limit = (SCHEMA.options && SCHEMA.options.message_max_bytes) || 15000;
    const bytes = new TextEncoder().encode(linkMessage()).length;
    if (bytes <= limit * 0.7) return "";
    const pct = Math.round((bytes / limit) * 100);
    return bytes > limit
        ? `<div class="dwarn over">This link is ${bytes.toLocaleString("en-US")} bytes — past the ${limit.toLocaleString("en-US")} a single chat message can carry. The bot will not be able to read it. Shorten a long list before sending.</div>`
        : `<div class="dwarn">This link is ${bytes.toLocaleString("en-US")} bytes, ${pct}% of what a single chat message can carry. Once it passes ${limit.toLocaleString("en-US")}, the bot will no longer be able to read it.</div>`;
}

/* How a setting's value reads in the diff. */
function settingText(f, v) {
    if (f.kind === "choice") return choiceText(f, v);
    if (f.kind === "bool") return v ? "on" : "off";
    return String(v);
}

/* The message the owner sends: the bot finds the URL anywhere in it and stops
   at ")" or a space, which is why a markdown link is safe to paste. */
const linkMessage = () =>
    `[Settings for ${state.groupName ? state.groupName.replace(/[[\]]/g, "") : `group ${state.botId}`}](${EDITOR_URL_BASE}#${hashParts()})`;

const STATUS = { changed: "changed", added: "added", removed: "removed", moved: "moved", renamed: "renamed", "turned off": "turned off", "turned on": "turned on" };

function openApplyDialog() {
    const d = diffConfig(state.baseline, config());
    const body = document.getElementById("applydlg-body");
    const foot = document.getElementById("applydlg-foot");
    document.getElementById("applydlg-meta").textContent = `${state.baseline.rules.length} → ${state.rules.length} rules`;

    const shown = d.rows.filter((r) => r.status !== "same");
    const same = d.rows.filter((r) => r.status === "same").map((r) => titleOf(r.rule));
    /* A setting is one value, so its change is one line, above the rules it
       applies to. */
    const settings = d.settings
        .map(
            (f) => `<div class="drow"><span class="st changed">changed</span>
        <span class="txt">${esc(f.label)}: ${esc(settingText(f, state.baseline[f.k]))} → <b>${esc(settingText(f, state.settings[f.k]))}</b></span></div>`
        )
        .join("");

    const sections = T.map((t) => {
        const rows = shown.filter((r) => r.trigger === t.v).sort((a, b) => a.index - b.index);
        if (!rows.length) return "";
        return `<h3 class="dsec">${esc(t.section)}</h3>` + rows
            .map((r) => {
                const notes = [];
                if (r.status === "moved") notes.push(`was ${ordinal(r.from + 1)}`);
                if (r.status === "renamed") notes.push(`was “${r.was.name || ""}”`);
                if (r.wasTrigger) notes.push(`was ${(T.find((x) => x.v === r.wasTrigger) || T[0]).section}`);
                return `<div class="drow">
          <span class="st ${r.status.replace(" ", "-")}">${STATUS[r.status]}</span>
          <span class="txt">${ruleLine(r.rule)}${notes.map((n) => `<span class="d">${esc(n)}</span>`).join("")}${(r.status === "changed" ? r.details || [] : []).map(detailHtml).join("")
                    }</span></div>`;
            })
            .join("");
    }).join("");

    body.innerHTML =
        linkWarning() +
        settings +
        (shown.length
            ? sections + (same.length ? `<div class="dquiet">Unchanged: ${esc(same.join(", "))}</div>` : "")
            : settings
                ? ""
                : `<div class="dempty">Nothing has changed since you opened this page. You can still copy the link and send it again.</div>`);

    /* A rule the bot would refuse is not sent at all (US-G14). */
    const bad = state.rules.find(issueOf);
    foot.innerHTML = bad
        ? `<p class="dblock" role="alert">“${esc(titleOf(bad))}” has no ${issueOf(bad)} yet. Finish it or delete it before sending.</p>
       <button class="btn" id="applydlg-cancel" type="button">Cancel</button>`
        : `<button class="btn" id="applydlg-cancel" type="button">Cancel</button>
       <button class="btn primary" id="applydlg-ok" type="button">Copy the link</button>`;
    dlg.showModal();
}

/* Copying is the last step of the flow, so it is also where the baseline moves:
   what the owner just sent becomes the new "before". */
function copyEditorLink() {
    return navigator.clipboard.writeText(linkMessage()).then(() => {
        state.baseline = snapshot();
        renderFrame();
    });
}

dlg.addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    if (b.id === "applydlg-cancel") {
        dlg.close();
        return;
    }
    if (b.id === "applydlg-ok") {
        copyEditorLink()
            .then(() => {
                document.getElementById("applydlg-body").innerHTML =
                    `<div class="dsuccess" role="status"><b>Link copied.</b> Paste it into your chat with the bot and send it — that is what applies the changes.</div>`;
                document.getElementById("applydlg-meta").textContent = "";
                document.getElementById("applydlg-foot").innerHTML =
                    `<button class="btn primary" id="applydlg-cancel" type="button">Close</button>`;
            })
            .catch(() => {
                document.getElementById("applydlg-body").innerHTML =
                    `<div class="dsuccess bad" role="alert"><b>Could not reach the clipboard.</b> Copy the whole address from the address bar and send that to the bot.</div>`;
            });
    }
});

/* ------------------------------- startup ------------------------------- */

document.getElementById("apply").addEventListener("click", openApplyDialog);

async function init() {
    if (typeof LZString === "undefined") throw new Error("lz-string failed to load");
    SCHEMA = await fetch("rules-schema.json").then((r) => {
        if (!r.ok) throw new Error(`rules-schema.json: HTTP ${r.status}`);
        return r.json();
    });

    const ruleProps = SCHEMA.properties.rules.items.properties;
    RULE = ruleProps;
    T = buildTriggers(ruleProps.trigger);
    const firstTrigger = [ruleProps.trigger.default || T[0].v];
    C = buildRegistry(SCHEMA.definitions.condition.oneOf, firstTrigger);
    A = buildRegistry(SCHEMA.definitions.action.oneOf, firstTrigger);
    S = buildSettings(SCHEMA);
    state.settings = clone(S.def);
    const lim = SCHEMA.definitions.condition.options || {};
    state.limits = { depth: lim.max_depth || 8, nodes: lim.max_nodes || 64 };

    const { bot_id, group, config: packed } = parseHash();
    state.botId = bot_id || null;
    try {
        state.groupName = group ? decodeURIComponent(group) : null;
    } catch (e) {
        state.groupName = null;
    }
    let unreadable = false;
    if (packed) {
        try {
            const { rules, ...settings } = JSON.parse(LZString.decompressFromEncodedURIComponent(packed));
            const checked = validateSettings(settings);
            if (!Array.isArray(rules) || checked.errors) throw new Error((checked.errors || ["no rules"]).join(" "));
            /* Every rule says its trigger, so two links that differ only in
               leaving the default out compare equal. */
            state.rules = rules.map((r) => ({ ...r, trigger: trig(r) }));
            state.settings = checked.settings;
        } catch (err) {
            console.warn("Could not restore the config from the URL hash:", err);
            unreadable = true;
        }
    }

    state.baseline = snapshot();

    if (!state.botId) document.getElementById("nobot").hidden = false;
    else document.getElementById("apply").hidden = false;
    document.getElementById("boot").hidden = true;
    document.getElementById("app").hidden = false;
    history.replaceState({ v: "overview" }, "", location.hash);
    render();
    if (unreadable) toast("The settings in this link could not be read. Starting from scratch.");
    /* ai.js and transfer.js wait for this: the registry and the rules have to
       exist first. */
    document.dispatchEvent(new CustomEvent("editor:ready"));
}

init().catch((err) => {
    const boot = document.getElementById("boot");
    boot.className = "boot failed";
    boot.textContent = `Failed to load the editor: ${err.message}`;
});
