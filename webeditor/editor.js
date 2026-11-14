"use strict";
/*
 * The rules editor of the SimpleX Chat group moderator bot.
 *
 * SimpleX carries plain text only, so the whole round trip is one URL: the bot
 * sends `#bot_id=<id>&group=<name>&config=<lz>`, the config being the group's
 * whole configuration — its settings beside its rules. This page edits it,
 * writes it back into the hash on every change, and the owner sends the link
 * back to the bot. `bot_id` and `config` must both be there or the bot ignores
 * the message; `group` only names the group on the page. A link without
 * `bot_id` is not a group's: the editor opens as a demo of itself (US-G18),
 * where nothing can be sent.
 *
 * Nothing here knows a condition, an action, a trigger or a group setting by
 * name: every one of them is read from rules-schema.json, so a new rule type is
 * an entry in that file and no code here. What the schema cannot say — a
 * summary, a phrase, the order actions run in, the triggers a type takes — is
 * in `options` keys on the same entry.
 *
 * Three pages, each an entry in the browser's history (US-G1, US-G3, US-G14):
 * the group, one rule, and New rule.
 */

const BASE_URL = location.href.split("#")[0];

/* Past this many entries a list gets a filter. */
const BIG_LIST = 20;

/* How many steps back Undo goes. A snapshot is the configuration's JSON. */
const UNDO_LIMIT = 20;

/* The subject a condition's emoji stands for, in the type picker. */
const SUBJECTS = {
    "💬": "Message text",
    "🔗": "Links",
    "📏": "Shape and size",
    "📎": "Attachments",
    "👤": "Member",
    "👥": "Group",
};

let SCHEMA = null;
let C = {}; // conditions, by type
let A = {}; // actions, by type
let S = []; // the group's settings: the schema root's fields but `rules`
let T = []; // the triggers, one section of the group page each
let R = {}; // the rule's own fields: name, trigger, enabled

const st = {
    rules: [],
    settings: {},
    /* What the bot has: the configuration the link arrived with. Every diff is
       measured from it, and it moves when a link is copied (US-G8). */
    baseline: { rules: [] },
    history: [],
    future: [],
    botId: null,
    /* No bot id: the demo (US-G18). */
    demo: false,
    group: null,
    unreadable: false,
    /* "home", "rule" (sel) or "new" (trigger). */
    view: "home",
    sel: 0,
    trigger: null,
    /* The step of the history a page was opened at: Undo on a rule's page goes
       back to there and no further, so it never changes a rule that is not on
       the screen (US-G6). */
    mark: 0,
    more: new Set(),
    menu: null,
    folded: new Set(),
    help: new Set(),
    shown: new Set(),
    bulk: new Set(),
    filter: {},
    only: null,
    limits: { depth: 8, nodes: 64 },
};

/* -------------------------------------------------------------------------- */
/* Small things                                                                */
/* -------------------------------------------------------------------------- */

const $ = (id) => document.getElementById(id);

const esc = (s) =>
    String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

/* The schema's descriptions are a little markdown: code, bold, italics, links.
   Escaped first, so these four are the only HTML that comes out. */
const md = (t) =>
    esc(t)
        .replace(/`([^`]+)`/g, "<code>$1</code>")
        .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        .replace(/(^|[\s(])[_*]([^_*]+)[_*]/g, "$1<i>$2</i>")
        .replace(/\[([^\]]+)\]\((https?:[^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');

const clone = (v) => structuredClone(v);

/* Identity regardless of key order. */
const canon = (v) =>
    JSON.stringify(v, (k, x) =>
        x && typeof x === "object" && !Array.isArray(x)
            ? Object.fromEntries(Object.keys(x).sort().map((key) => [key, x[key]]))
            : x
    );

const ordinal = (n) => {
    const s = ["th", "st", "nd", "rd"], v = n % 100;
    return n + (s[(v - 20) % 10] || s[v] || s[0]);
};

const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

/* The first sentence of a description: what a field or a type is for, in a
   line. A full stop ends it unless it belongs to "e.g." or "i.e.". */
const SENTENCE_END = /(?<=[a-z0-9)"'`”’?!]\.)(?<!\be\.g\.)(?<!\bi\.e\.)\s+(?=[A-Z*_"“(])/;
const firstSentence = (text) => String(text).split(SENTENCE_END)[0];

const ICON = {
    more: '<svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="4" cy="10" r="1.6"/><circle cx="10" cy="10" r="1.6"/><circle cx="16" cy="10" r="1.6"/></svg>',
    info: '<svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.25" fill="none" stroke="currentColor" stroke-width="1.5"/><rect x="9.25" y="8.5" width="1.5" height="5.5" rx=".75"/><circle cx="10" cy="6.2" r="1"/></svg>',
    chevron: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M6 8l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    grip: '<svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="7" cy="5" r="1.4"/><circle cx="13" cy="5" r="1.4"/><circle cx="7" cy="10" r="1.4"/><circle cx="13" cy="10" r="1.4"/><circle cx="7" cy="15" r="1.4"/><circle cx="13" cy="15" r="1.4"/></svg>',
    back: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M12.5 4.5L7 10l5.5 5.5" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    plus: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 4.5v11M4.5 10h11" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"/></svg>',
    focus: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 8V4h4M16 8V4h-4M4 12v4h4M16 12v4h-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>',
};

function toast(text) {
    const el = document.createElement("div");
    el.className = "toast";
    el.setAttribute("role", "status");
    el.textContent = text;
    document.body.appendChild(el);
    setTimeout(() => el.remove(), 3000);
}

/* -------------------------------------------------------------------------- */
/* The schema                                                                  */
/* -------------------------------------------------------------------------- */

function kindOf(pr) {
    if (pr.$ref) return "child";
    if (pr.type === "array") return pr.items && pr.items.$ref ? "children" : "list";
    if (pr.type === "boolean") return "bool";
    if (pr.type === "integer" || pr.type === "number") return "int";
    if (pr.type === "string" && pr.format === "password") return "secret";
    if (pr.type === "string" && pr.format === "textarea") return "prose";
    if (pr.oneOf) return "choice";
    return "text";
}

function defaultOf(pr) {
    if ("default" in pr) return clone(pr.default);
    const k = kindOf(pr);
    if (k === "list" || k === "children") return [];
    if (k === "bool") return false;
    if (k === "int") return pr.minimum ?? 0;
    if (k === "choice") {
        const first = (pr.oneOf || []).find((b) => "const" in b);
        return first ? first.const : 0;
    }
    return "";
}

/* A field read off its schema property: its control, label, description and
   limits. */
function fieldOf(key, pr) {
    const branches = pr.oneOf || [];
    const num = branches.find((b) => b.type === "integer" || b.type === "number");
    return {
        key,
        kind: kindOf(pr),
        label: (pr.options && pr.options.label) || pr.title || key,
        hint: pr.description || "",
        min: pr.minimum ?? (num && num.minimum),
        max: pr.maximum ?? (num && num.maximum),
        maxItems: pr.maxItems,
        maxLength: pr.items ? pr.items.maxLength : pr.maxLength,
        choices: branches.filter((b) => "const" in b).map((b) => ({ v: b.const, t: b.title || String(b.const), o: b.options || {} })),
        num,
        unit: (num && num.options && num.options.unit) || "",
        def: defaultOf(pr),
    };
}

const choiceText = (f, v) => (typeof v === "number" ? `${v}${f.unit}` : (f.choices.find((c) => c.v === v) || { t: String(v) }).t);

/* {field} · {field|n} · {field|list} · {field|phrase(s)} · {field?yes:no}, $ = the value */
function fill(template, v) {
    return template.replace(/\{([a-z_]+)(?:\?([^:}]*):([^}]*)|\|(n|list|phrase|phrases))?\}/g, (m, f, yes, no, mod) => {
        const x = v[f];
        if (yes !== undefined) return (x ? yes : no).replace(/\$/g, x);
        if (mod === "n") return (x || []).length;
        if (mod === "list") return (x || []).length ? x.slice(0, 2).join(", ") + (x.length > 2 ? ` +${x.length - 2}` : "") : "empty";
        if (mod === "phrase") return x ? phraseOf(x) : "";
        if (mod === "phrases") return (x || []).map(phraseOf).join("; ");
        return x;
    });
}

function registry(entries, triggersByDefault, isAction) {
    const out = {};
    for (const e of entries) {
        const type = e.properties.type.const;
        const emoji = e.title.split(" ")[0];
        const o = e.options || {};
        const fields = Object.entries(e.properties).filter(([k]) => k !== "type").map(([k, pr]) => fieldOf(k, pr));
        const title = e.title.slice(emoji.length).trim();
        out[type] = {
            type,
            emoji,
            title,
            subject: isAction ? "Actions" : SUBJECTS[emoji] || "Combine",
            help: e.description || "",
            gist: e.description ? firstSentence(e.description) : "",
            triggers: o.triggers || triggersByDefault,
            rank: o.execution_rank || 0,
            coveredBy: o.covered_by || [],
            joiner: ((title.match(/\(([A-Z]+)\)\s*$/) || [])[1] || "").toLowerCase(),
            fields,
            summary: (v) => (o.summary ? fill(o.summary, v) : ""),
            phrase: (v) => (o.phrase ? fill(o.phrase, v) : title.toLowerCase()),
            defaults: () => Object.fromEntries(fields.map((f) => [f.key, clone(f.def)])),
        };
    }
    return out;
}

/* A type a newer bot stored, that this page's schema does not have: it shows,
   and above all it goes back to the bot untouched. */
const unknown = (type) => ({
    type,
    emoji: "❓",
    title: `${type} (unknown)`,
    subject: "Other",
    help: "This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.",
    gist: "",
    triggers: null,
    rank: 0,
    coveredBy: [],
    joiner: "",
    fields: [],
    unknown: true,
    summary: () => "",
    phrase: () => type,
    defaults: () => ({}),
});

const cond = (type) => C[type] || unknown(type);
const act = (type) => A[type] || unknown(type);
const phraseOf = (n) => cond(n.type).phrase(n);

/* -------------------------------------------------------------------------- */
/* Rules                                                                       */
/* -------------------------------------------------------------------------- */

const triggerOf = (r) => T.find((t) => t.v === (r.trigger || T[0].v)) || T[0];
const isOn = (r) => r.enabled !== false;
const fits = (spec, r) => spec.unknown || spec.triggers.includes(triggerOf(r).v);

/* What keeps the bot from taking a rule (US-G14). */
function missing(r) {
    if (!r.condition && !triggerOf(r).optional) return "condition";
    if (!(r.actions || []).length) return "action";
    return null;
}

const actionWords = (r) => (r.actions || []).map((a) => `${act(a.type).emoji} ${act(a.type).phrase(a)}`).join(" · ");
const condWords = (r) => (r.condition ? phraseOf(r.condition) : triggerOf(r).everyone);
const sentence = (r) => `IF ${condWords(r)} THEN ${actionWords(r)}`;

/* A rule is called by its name, or by what it does (US-G1). */
const titleOf = (r) => r.name || (missing(r) ? "New rule" : sentence(r));

const inSection = (v) => st.rules.map((r, i) => [r, i]).filter(([r]) => triggerOf(r).v === v);

/* -------------------------------------------------------------------------- */
/* Condition trees                                                             */
/* -------------------------------------------------------------------------- */

/* A path is dotted keys from st: "rules.2.condition.conditions.0". */
const at = (path) => path.split(".").reduce((o, k) => o[k], st);
const parent = (path) => {
    const i = path.lastIndexOf(".");
    return { holder: at(path.slice(0, i)), key: path.slice(i + 1) };
};

const oneKey = (n) => (cond(n.type).fields.find((f) => f.kind === "child") || {}).key;
const manyKey = (n) => (cond(n.type).fields.find((f) => f.kind === "children") || {}).key;
const andType = () => Object.keys(C).find((k) => C[k].fields.some((f) => f.kind === "children"));

function depth(n) {
    if (!n) return 0;
    const one = oneKey(n), many = manyKey(n);
    if (one) return 1 + depth(n[one]);
    if (many) return 1 + Math.max(0, ...(n[many] || []).map(depth));
    return 1;
}

function count(n) {
    if (!n) return 0;
    const one = oneKey(n), many = manyKey(n);
    if (one) return 1 + count(n[one]);
    if (many) return 1 + (n[many] || []).reduce((s, x) => s + count(x), 0);
    return 1;
}

function foldAll(n, path, set) {
    if (!n) return;
    set.add(path);
    const one = oneKey(n), many = manyKey(n);
    if (one) foldAll(n[one], `${path}.${one}`, set);
    if (many) (n[many] || []).forEach((x, i) => foldAll(x, `${path}.${many}.${i}`, set));
}

const fresh = (spec) => ({ type: spec.type, ...spec.defaults() });

/* A new type keeps every field the old one had under the same name and kind,
   so All becomes Any with its whole subtree, and one link condition another
   with its domains (US-G4). */
function retype(old, spec, oldSpec) {
    const next = fresh(spec);
    for (const f of spec.fields) {
        const was = oldSpec.fields.find((x) => x.key === f.key);
        if (was && was.kind === f.kind && old[f.key] !== undefined) next[f.key] = clone(old[f.key]);
    }
    return next;
}

/* -------------------------------------------------------------------------- */
/* History, the link, the pages                                                */
/* -------------------------------------------------------------------------- */

const config = () => ({ ...st.settings, rules: st.rules });
const snapshot = () => clone(config());

function restore(c) {
    const { rules, ...settings } = c;
    st.rules = rules;
    st.settings = settings;
}

/* Every edit goes through `edit`, so nothing escapes Undo, and an edit that
   changes nothing leaves no step behind. */
function edit(change) {
    const before = snapshot();
    const result = change();
    if (canon(before) !== canon(config())) {
        st.history.push(before);
        if (st.history.length > UNDO_LIMIT) st.history.shift();
        st.future = [];
    }
    return result;
}

function travel(from, to, word) {
    const target = from.pop();
    if (!target) return;
    to.push(snapshot());
    restore(target);
    if (st.view === "rule" && !st.rules[st.sel]) st.view = "home";
    st.menu = null;
    render();
    toast(word);
}

/* On the overview, Undo goes through every change; on a rule's page, through
   the ones made there (US-G6). */
const canUndo = () => st.history.length > (st.view === "home" ? 0 : st.mark);
const undo = () => canUndo() && travel(st.history, st.future, "Undone.");
const redo = () => st.future.length && travel(st.future, st.history, "Redone.");

function hashOf() {
    const packed = LZString.compressToEncodedURIComponent(JSON.stringify(config()));
    return (st.botId ? `bot_id=${st.botId}&` : "") + (st.group ? `group=${encodeURIComponent(st.group)}&` : "") + `config=${packed}`;
}

/* The message the owner sends. The bot finds the URL anywhere in it. */
const linkMessage = () =>
    `[Settings for ${st.group ? st.group.replace(/[[\]]/g, "") : `group ${st.botId}`}](${BASE_URL}#${hashOf()})`;

/* A page is a history entry, so Back goes back a page (US-G3). */
function go(view, { sel, trigger, replace } = {}) {
    st.view = view;
    if (sel !== undefined) st.sel = sel;
    if (trigger !== undefined) st.trigger = trigger;
    st.menu = null;
    st.only = null;
    st.mark = st.history.length;
    st.future = [];
    history[replace ? "replaceState" : "pushState"]({ view, sel: st.sel, trigger: st.trigger }, "", location.hash);
    render();
    window.scrollTo(0, 0);
    const h = $("page-h");
    if (h) h.focus({ preventScroll: true });
}

window.addEventListener("popstate", (e) => {
    const s = e.state || {};
    st.view = s.view === "rule" && st.rules[s.sel] ? "rule" : s.view === "new" ? "new" : "home";
    if (typeof s.sel === "number") st.sel = s.sel;
    if (s.trigger) st.trigger = s.trigger;
    st.menu = null;
    st.mark = st.history.length;
    st.future = [];
    render();
});

/* The draft lives in this tab only, until its link is sent. */
window.addEventListener("beforeunload", (e) => {
    if (canon(st.baseline) !== canon(config())) e.preventDefault();
});

/* -------------------------------------------------------------------------- */
/* Rendering                                                                   */
/* -------------------------------------------------------------------------- */

const groupTitle = () => (st.demo ? "Demo group" : st.group || `Group ${st.botId}`);

function render() {
    if (st.view === "rule" && !st.rules[st.sel]) st.view = "home";
    /* The focus survives a render when its element does. */
    const focused = document.activeElement && document.activeElement.id;
    renderBar();
    const app = $("app");
    app.dataset.view = st.view;
    app.innerHTML = st.view === "rule" ? rulePage() : st.view === "new" ? newPage() : homePage();
    history.replaceState(history.state, "", "#" + hashOf());
    if (focused && $(focused) && document.activeElement === document.body) $(focused).focus({ preventScroll: true });
    placeMenu();
}

/* An open menu goes where it fits: below its button, or above it when the
   screen ends first — the bar on a phone included —, and never past a side
   (US-G12). */
function placeMenu() {
    if (!st.menu) return;
    const button = $(`m-${st.menu}`), pop = button && button.parentElement.querySelector(".pop");
    if (!pop) return;
    const fixedBar = $("bar-actions");
    const room = () => {
        const b = button.getBoundingClientRect(), bar = getComputedStyle(fixedBar).position === "fixed" && !fixedBar.hidden ? fixedBar.getBoundingClientRect().top : innerHeight;
        return { below: Math.min(innerHeight, bar) - b.bottom - 8, above: b.top - document.querySelector(".appbar").getBoundingClientRect().bottom - 8 };
    };
    pop.classList.remove("up");
    let { below, above } = room();
    /* Neither side has room: bring the button to the middle of the screen. */
    if (pop.offsetHeight > below && pop.offsetHeight > above) {
        button.scrollIntoView({ block: "center" });
        ({ below, above } = room());
    }
    if (pop.offsetHeight > below && above > below) pop.classList.add("up");
    const box = pop.getBoundingClientRect();
    if (box.left < 8) pop.style.right = `${box.left - 8}px`;
    else if (box.right > innerWidth - 8) pop.style.right = `${box.right - innerWidth + 8}px`;
}

/* A ⋯ or ⋮ button and the menu it opens; `items` are [label, attributes,
   danger?]. */
function menuInner(id, label, items, icon = ICON.more) {
    const open = st.menu === id;
    return `<button class="icon" type="button" id="m-${esc(id)}" data-op="menu" data-menu="${esc(id)}"
      aria-haspopup="menu" aria-expanded="${open}" aria-label="${esc(label)}">${icon}</button>${open
            ? `<div class="pop" role="menu" aria-label="${esc(label)}">${items
                .map(([text, attrs, danger]) => `<button type="button" role="menuitem" tabindex="-1" ${attrs}${danger ? ' class="danger"' : ""}>${esc(text)}</button>`)
                .join("")}</div>`
            : ""
        }`;
}

const menu = (id, label, items) => (items.length ? `<div class="menu">${menuInner(id, label, items)}</div>` : `<span class="icon-space"></span>`);

function renderBar() {
    $("where").innerHTML =
        st.view === "home"
            ? `<h1 id="page-h" tabindex="-1">${esc(groupTitle())}</h1>
         <p>Moderation settings · <a href="https://simplex-directory.asriyan.me/#/bots/63d110f4-9f70-4128-b0e7-5a8958299f96" target="_blank" rel="noopener">Moderator Bot</a></p>`
            : `<button class="back" type="button" data-op="home" id="back">${ICON.back}<span>Back to rules</span></button>`;
    document.title = `${groupTitle()} — Moderation settings`;
    $("undo").hidden = !canUndo();
    $("redo").hidden = !st.future.length;
    /* What applies to the whole configuration is on the overview alone. */
    $("apply").hidden = st.view !== "home";
    const n = changeCount();
    $("apply").textContent = n ? `Apply ${plural(n, "change")}` : "Apply changes";
    const g = $("gmenu");
    g.hidden = st.view !== "home";
    g.innerHTML = menuInner("global", "More", [
        ["Edit all with AI", 'data-op="ai-all"'],
        ["Import JSON", 'data-op="import"'],
        ["Export JSON", 'data-op="export"'],
    ]);
    /* An empty bar would be a strip across the phone's screen. */
    $("bar-actions").hidden = [...$("bar-actions").children].every((c) => c.hidden);
}

/* ---------------------------- the group ---------------------------------- */

function settingsHtml() {
    return S.map((f) => {
        const pr = SCHEMA.properties[f.key];
        const id = `set-${f.key}`;
        /* A few named values whose titles read "Name — what it does" are a row
           of choices, each with its line (US-G2). */
        const segmented = f.kind === "choice" && !f.num && f.choices.every((c) => c.t.includes(" — "));
        const body = segmented
            ? `<div class="segments" role="radiogroup" aria-labelledby="${id}-h">${f.choices
                .map((c, i) => {
                    const [label, line] = c.t.split(" — ");
                    const on = st.settings[f.key] === c.v;
                    return `<label class="segment${on ? " on" : ""}"><input type="radio" name="${id}" id="${id}-${i}" value="${i}"
                ${on ? "checked" : ""} data-op="setting" data-key="${f.key}"><b>${esc(label)}</b><span>${esc(line[0].toUpperCase() + line.slice(1))}</span></label>`;
                })
                .join("")}</div>`
            : `<div class="fields">${fieldsHtml([f], st.settings, "settings")}</div>`;
        return `<section class="block" aria-labelledby="${id}-h">
        <div class="block-head"><h2 id="${id}-h">${esc(pr.title || f.key)}</h2></div>
        ${body}</section>`;
    }).join("");
}

function rowHtml(r, i, pos, total) {
    const title = titleOf(r), gap = missing(r), on = isOn(r);
    /* A rule with a name says what it does under it; one without says it as
       its name (US-G1). */
    const words = r.name ? actionWords(r) : "";
    const sub = words || gap || !on;
    return `<li class="item${on ? "" : " off"}" data-i="${i}">
      <button class="grip" type="button" id="grip-${i}" data-drag="${i}" aria-label="Move “${esc(title)}”" aria-describedby="move-hint">${ICON.grip}</button>
      <button class="item-main" type="button" id="open-${i}" data-op="open" data-i="${i}" aria-labelledby="t-${i}"${sub ? ` aria-describedby="d-${i}"` : ""}>
        <span class="item-title${r.name ? "" : " plain"}" id="t-${i}">${esc(title)}</span>
        ${sub ? `<span class="item-sub" id="d-${i}">${words ? `<span>${esc(words)}</span>` : ""}${gap ? `<span class="tag warn">Incomplete</span>` : ""}${on ? "" : `<span class="tag">Off</span>`}</span>` : ""}
      </button>
      <button class="switch" type="button" role="switch" id="sw-${i}" aria-checked="${on}" data-op="toggle" data-i="${i}" aria-label="${esc(title)}"><span></span></button>
      ${menu(`r${i}`, `More for “${title}”`, [
        ...(pos > 0 ? [["Move up", `data-op="move" data-i="${i}" data-d="-1"`]] : []),
        ...(pos < total - 1 ? [["Move down", `data-op="move" data-i="${i}" data-d="1"`]] : []),
        [on ? "Disable" : "Enable", `data-op="toggle" data-i="${i}"`],
        ["Delete", `data-op="drop" data-i="${i}"`, true],
    ])}
    </li>`;
}

function homePage() {
    const sections = T.map((t) => {
        const rows = inSection(t.v);
        return `<section class="block" aria-labelledby="sec-${t.v}" data-trigger="${esc(t.v)}">
        <div class="block-head">
          <div><h2 id="sec-${t.v}">${esc(t.section)}</h2><p class="sub">Checked top to bottom.</p></div>
          <button class="btn" type="button" id="add-${t.v}" data-op="new" data-t="${esc(t.v)}"
            aria-label="+ Add rule ${esc(t.newRule.replace(/^When /, "when "))}">${ICON.plus}<span>Add rule</span></button>
        </div>
        ${rows.length
                ? `<ol class="list">${rows.map(([r, i], pos) => rowHtml(r, i, pos, rows.length)).join("")}</ol>`
                : `<p class="empty">${esc(t.empty)}</p>`}
      </section>`;
    }).join("");

    return `
      ${st.unreadable ? `<p class="alert bad" role="alert">The settings in this link could not be read. Starting from scratch.</p>` : ""}
      ${settingsHtml()}
      ${sections}
      <p class="sr-only" id="move-hint">Drag to move it up or down, or use the arrow keys.</p>`;
}

/* ------------------------------ fields ----------------------------------- */

/* A field's description: its first sentence, in view, and the rest behind
   More (US-G3). */
function hintHtml(f, id) {
    if (!f.hint) return "";
    const head = firstSentence(f.hint), more = f.hint.length > head.length, open = st.more.has(id);
    return `<p class="hint" id="${id}-h">${md(open || !more ? f.hint : head)}${more
        ? ` <button class="link small" type="button" id="${id}-more" data-op="more" data-key="${id}" aria-expanded="${open}" aria-controls="${id}-h">${open ? "Less" : "More"}</button>`
        : ""}</p>`;
}

function fieldsHtml(fields, value, path) {
    return fields
        .filter((f) => f.kind !== "child" && f.kind !== "children")
        .map((f) => {
            const id = `f-${path}-${f.key}`;
            const hint = hintHtml(f, id);
            const desc = f.hint ? ` aria-describedby="${id}-h"` : "";
            const label = `<label class="label" for="${id}">${esc(f.label)}</label>`;
            const v = value[f.key];
            const data = `data-path="${path}" data-key="${f.key}"`;

            if (f.kind === "int")
                return `<div class="field">${label}<input type="number" inputmode="numeric" id="${id}" value="${esc(v)}"
          ${f.min != null ? `min="${f.min}"` : ""} ${f.max != null ? `max="${f.max}"` : ""}${desc} data-op="num" ${data}>${hint}</div>`;

            if (f.kind === "bool")
                return `<div class="field wide"><label class="check"><input type="checkbox" id="${id}" ${v ? "checked" : ""}${desc}
          data-op="bool" ${data}><span>${esc(f.label)}</span></label>${hint}</div>`;

            if (f.kind === "text")
                return `<div class="field wide">${label}<input type="text" id="${id}" value="${esc(v)}" spellcheck="false"
          ${f.maxLength ? `maxlength="${f.maxLength}"` : ""}${desc} data-op="text" ${data}>${hint}</div>`;

            if (f.kind === "prose")
                return `<div class="field wide"><div class="label-row">${label}<span class="count">${String(v ?? "").length}${f.maxLength ? ` / ${f.maxLength}` : ""}</span></div>
          <textarea id="${id}" rows="5" ${f.maxLength ? `maxlength="${f.maxLength}"` : ""}${desc} data-op="text" ${data}>${esc(v)}</textarea>${hint}</div>`;

            if (f.kind === "secret") {
                const key = `${path}.${f.key}`, shown = st.shown.has(key);
                return `<div class="field wide">${label}<div class="joined"><input type="${shown ? "text" : "password"}" id="${id}" value="${esc(v)}"
          autocomplete="off" spellcheck="false" ${f.maxLength ? `maxlength="${f.maxLength}"` : ""}${desc} data-op="text" ${data}>
          <button class="btn" type="button" id="${id}-show" data-op="reveal" data-key="${esc(key)}" aria-label="${shown ? "Hide" : "Show"} ${esc(f.label)}">${shown ? "Hide" : "Show"}</button></div>${hint}</div>`;
            }

            if (f.kind === "choice") {
                const own = typeof v === "number";
                return `<div class="field">${label}<div class="joined"><select id="${id}"${desc} data-op="choice" ${data}>
          ${f.choices.map((c, i) => `<option value="${i}" ${v === c.v ? "selected" : ""}>${esc(c.t)}</option>`).join("")}
          ${f.num ? `<option value="n" ${own ? "selected" : ""}>${esc(f.num.title || "Custom")}</option>` : ""}</select>
          ${own ? `<input type="number" id="${id}-n" value="${esc(v)}" aria-label="${esc(f.label)}: ${esc(f.num.title || "value")}"
            ${f.min != null ? `min="${f.min}"` : ""} ${f.max != null ? `max="${f.max}"` : ""} data-op="num" ${data}>` : ""}</div>${hint}</div>`;
            }

            /* A list: entries one by one, or as text to paste many at once. */
            const items = v || [], key = `${path}.${f.key}`, big = items.length > BIG_LIST, bulk = st.bulk.has(key);
            const head = `<div class="label-row"><label class="label" for="${id}${bulk ? "-t" : ""}">${esc(f.label)}</label>
          <span class="count">${items.length}</span>
          <button class="link small" type="button" id="${id}-mode" data-op="bulk" data-key="${esc(key)}"
            aria-label="${bulk ? `Show ${esc(f.label)} as chips` : `Bulk edit ${esc(f.label)}`}">${bulk ? "chips" : "bulk edit"}</button></div>`;
            if (bulk)
                return `<div class="field wide">${head}${hint}
          <textarea class="code" id="${id}-t" rows="9" spellcheck="false" aria-describedby="${id}-b" data-op="lines" ${data}>${esc(items.join("\n"))}</textarea>
          <p class="hint" id="${id}-b">One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale.${f.maxItems ? ` Up to ${f.maxItems} entries${f.maxLength ? `, ${f.maxLength} characters each` : ""}.` : ""}</p></div>`;

            const q = (st.filter[key] || "").toLowerCase();
            const shown = items.map((x, j) => [x, j]).filter(([x]) => !q || String(x).toLowerCase().includes(q));
            return `<div class="field wide">${head}${hint}
        <div class="list-tools">
          <input type="text" id="${id}" placeholder="add + Enter" aria-label="Add to ${esc(f.label)}" enterkeyhint="done"
            ${f.maxLength ? `maxlength="${f.maxLength}"` : ""} data-op="entry" ${data}>
          ${big ? `<input type="search" id="${id}-q" placeholder="filter ${items.length} entries" aria-label="Filter ${esc(f.label)}" value="${esc(st.filter[key] || "")}" data-op="filter" data-key="${esc(key)}">` : ""}
        </div>
        ${items.length
                    ? `<ul class="chips${big ? " capped" : ""}" aria-label="${esc(f.label)}">${shown
                        .map(([x, j]) => `<li>${esc(x)}<button type="button" id="${id}-x${j}" data-op="unentry" ${data} data-j="${j}" aria-label="Remove ${esc(x)}">×</button></li>`)
                        .join("") || `<li class="none">Nothing matches “${esc(st.filter[key])}”.</li>`}</ul>`
                    : ""}
      </div>`;
        })
        .join("");
}

/* ------------------------------ one rule --------------------------------- */

const wrongPlace = (spec, r) => (fits(spec, r) ? "" : `<span class="tag bad">${esc(triggerOf(r).incompatible)}</span>`);

function conditionHtml(n, path, d, r) {
    const spec = cond(n.type), one = oneKey(n), many = manyKey(n), composite = !!(one || many);
    const folded = st.folded.has(path);
    const params = spec.fields.some((f) => f.kind !== "child" && f.kind !== "children");
    const summary = folded ? spec.summary(n) : "";
    const items = [
        ["Change type…", `data-op="pick" data-mode="retype" data-path="${path}"`],
        ["Wrap in NOT", `data-op="wrap" data-path="${path}" data-type="Not"`],
        ["Wrap in AND", `data-op="wrap" data-path="${path}" data-type="All"`],
        ...(composite ? [["Unwrap", `data-op="unwrap" data-path="${path}"`]] : []),
        ["Delete", `data-op="delete" data-path="${path}"`, true],
    ];

    let inside = "";
    if (!folded && many) {
        const kids = n[many] || [];
        inside = `<div class="kids">${kids
            .map((x, j) => (j ? `<div class="joiner" aria-hidden="true"><span>${esc(spec.joiner || "and")}</span></div>` : "") + conditionHtml(x, `${path}.${many}.${j}`, d + 1, r))
            .join("")}
          <button class="btn add" type="button" id="add-${path}" data-op="pick" data-mode="child" data-path="${path}">${ICON.plus}<span>Add to ${esc(spec.joiner.toUpperCase() || "this")}</span></button></div>`;
    } else if (!folded && one && n[one]) inside = `<div class="kids">${conditionHtml(n[one], `${path}.${one}`, d + 1, r)}</div>`;

    return `<div class="node${composite ? " group" : ""}">
      <div class="node-head">
        ${composite || params
            ? `<button class="icon fold${folded ? " folded" : ""}" type="button" id="fold-${path}" data-op="fold" data-path="${path}" aria-expanded="${!folded}" aria-label="${folded ? "Expand" : "Collapse"} “${esc(spec.title)}”">${ICON.chevron}</button>`
            : `<span class="icon-space"></span>`}
        <span class="emoji" aria-hidden="true">${spec.emoji}</span>
        <span class="node-title"><span>${esc(spec.title)}</span>${wrongPlace(spec, r)}${summary ? `<span class="summary">${esc(summary)}</span>` : ""}</span>
        ${composite && d >= 2 ? `<button class="icon" type="button" id="only-${path}" data-op="only" data-path="${path}" aria-label="Show only “${esc(spec.title)}”">${ICON.focus}</button>` : ""}
        <button class="icon" type="button" id="help-${path}" data-op="help" data-key="${path}" aria-expanded="${st.help.has(path)}" aria-label="About “${esc(spec.title)}”">${ICON.info}</button>
        ${menu(path, `More for “${spec.title}”`, items)}
      </div>
      ${st.help.has(path) ? `<div class="help">${md(spec.help)}</div>` : ""}
      ${!folded && params ? `<div class="fields">${fieldsHtml(spec.fields, n, path)}</div>` : ""}
      ${inside}
    </div>`;
}

/* `covered_by`: a type, or a type and the settings that make it cover. */
function covers(cover, a) {
    const type = typeof cover === "string" ? cover : cover.type;
    const when = typeof cover === "string" ? null : cover.when;
    return a.type === type && (!when || Object.entries(when).every(([k, v]) => a[k] === v));
}

function actionsHtml(r, i) {
    /* In the order the bot carries them out (US-G3). */
    return r.actions
        .map((a, j) => [a, j])
        .sort((x, y) => act(x[0].type).rank - act(y[0].type).rank)
        .map(([a, j]) => {
            const spec = act(a.type), path = `rules.${i}.actions.${j}`;
            const covered = spec.coveredBy.some((c) => r.actions.some((x) => covers(c, x)));
            return `<div class="node">
          <div class="node-head">
            <span class="emoji" aria-hidden="true">${spec.emoji}</span>
            <span class="node-title"><span>${esc(spec.title)}</span>${wrongPlace(spec, r)}${covered ? `<span class="tag">covered</span>` : ""}</span>
            <button class="icon" type="button" id="help-${path}" data-op="help" data-key="${path}" aria-expanded="${st.help.has(path)}" aria-label="About “${esc(spec.title)}”">${ICON.info}</button>
            ${menu(path, `More for “${spec.title}”`, [
                ["Change type…", `data-op="pick" data-mode="reaction" data-path="${path}"`],
                ["Delete", `data-op="unact" data-path="${path}"`, true],
            ])}
          </div>
          ${covered ? `<p class="hint">A stronger action on this rule already covers it, so the bot skips this one.</p>` : ""}
          ${st.help.has(path) ? `<div class="help">${md(spec.help)}</div>` : ""}
          ${spec.fields.length ? `<div class="fields">${fieldsHtml(spec.fields, a, path)}</div>` : ""}
        </div>`;
        })
        .join("");
}

function rulePage() {
    const i = st.sel, r = st.rules[i], t = triggerOf(r), gap = missing(r);
    const root = `rules.${i}.condition`;
    const only = st.only && st.only.startsWith(root + ".") ? st.only : null;
    const nodes = count(r.condition), levels = depth(r.condition);
    const condDef = SCHEMA.definitions.condition, actDef = SCHEMA.properties.rules.items.properties.actions;
    const many = r.condition && manyKey(r.condition);

    const tree = !r.condition
        ? `<p class="empty">${t.optional ? esc(t.everyone) : "No condition yet."}</p>`
        : only
            ? `<p class="crumbs"><button class="link small" type="button" id="whole" data-op="whole">Whole condition</button> › part of it</p>${conditionHtml(at(only), only, 0, r)}`
            : conditionHtml(r.condition, root, 0, r);

    return `
    <h1 class="sr-only" id="page-h" tabindex="-1">${esc(titleOf(r))}</h1>
    <div class="rule-top">
      <div class="name">
        <label class="label" for="r-name">${esc(R.name.title)}</label>
        <input type="text" id="r-name" class="title-input" maxlength="${R.name.maxLength || 60}" value="${esc(r.name)}"
          placeholder="${esc(gap ? "New rule" : sentence(r))}" aria-describedby="r-name-h" data-op="name">
        <p class="hint" id="r-name-h">${md(R.name.description || "")}</p>
      </div>
      <div class="rule-tools">
        <button class="btn" type="button" id="rule-ai" data-op="ai-rule">✨ Edit with AI</button>
        ${menu("rule", "More for this rule", [["Duplicate", 'data-op="duplicate"'], ["Delete", 'data-op="remove"', true]])}
      </div>
    </div>
    ${isOn(r) ? "" : `<p class="alert off" role="status"><span>⏸ This rule is off — the bot skips it.</span>
      <button class="btn" type="button" id="enable" data-op="toggle" data-i="${i}">Enable</button></p>`}
    ${gap ? `<p class="alert warn" role="status">${esc(t.incomplete)}</p>` : ""}

    <ol class="flow">
      <li class="step when">
        <div class="step-head"><h2 class="step-name" id="s-when">${esc(R.trigger.title)}</h2></div>
        <div class="step-body">
          <div class="segments two" role="radiogroup" aria-labelledby="s-when" aria-describedby="s-when-h">
            ${T.map((x, k) => `<label class="segment${x.v === t.v ? " on" : ""}"><input type="radio" name="when" id="when-${k}" value="${esc(x.v)}" ${x.v === t.v ? "checked" : ""} data-op="when"><b>${esc(x.title)}</b></label>`).join("")}
          </div>
          <p class="hint" id="s-when-h">${md(R.trigger.description || "")}</p>
        </div>
      </li>
      <li class="step if">
        <div class="step-head">
          <h2 class="step-name" id="s-if">${esc(condDef.title.replace(/^\S+\s/, ""))}</h2>
          <button class="icon" type="button" id="help-if" data-op="help" data-key="if" aria-expanded="${st.help.has("if")}" aria-label="About conditions">${ICON.info}</button>
          ${r.condition && (oneKey(r.condition) || many || cond(r.condition.type).fields.length)
            ? `<button class="link small push" type="button" id="fold-all" data-op="fold-all">${[...st.folded].some((p) => p.startsWith(root)) ? "Expand all" : "Collapse all"}</button>` : ""}
        </div>
        <div class="step-body">
          ${st.help.has("if") ? `<div class="help">${md(condDef.description)}</div>` : ""}
          ${nodes > st.limits.nodes || levels > st.limits.depth
            ? `<p class="alert warn">This rule has ${nodes} conditions nested ${levels} levels deep. The bot accepts at most ${st.limits.nodes} conditions and ${st.limits.depth} levels, so simplify it before applying — otherwise the rules will be rejected.</p>` : ""}
          ${tree}
          ${!r.condition || !many ? `<button class="btn add" type="button" id="add-cond" data-op="pick" data-mode="condition">${ICON.plus}<span>Add condition</span></button>` : ""}
        </div>
      </li>
      <li class="step then">
        <div class="step-head">
          <h2 class="step-name" id="s-then">${esc(actDef.title.replace(/^\S+\s/, ""))}</h2>
          <button class="icon" type="button" id="help-then" data-op="help" data-key="then" aria-expanded="${st.help.has("then")}" aria-label="About actions">${ICON.info}</button>
        </div>
        <div class="step-body">
          ${st.help.has("then") ? `<div class="help">${md(actDef.description)}</div>` : ""}
          ${r.actions.length ? `<p class="sub">The bot runs them in this order.</p>${actionsHtml(r, i)}` : `<p class="empty">No action yet.</p>`}
          <button class="btn add" type="button" id="add-act" data-op="pick" data-mode="action">${ICON.plus}<span>Add action</span></button>
        </div>
      </li>
    </ol>`;
}

/* ------------------------------ New rule --------------------------------- */

const templatesOf = (v) => ((SCHEMA.options && SCHEMA.options.templates) || []).filter((x) => (x.rule.trigger || T[0].v) === v);

function newPage() {
    const t = T.find((x) => x.v === st.trigger) || T[0];
    const ways = [
        ["templates", "📋", "Start from a template", "A ready rule to adjust.", ' aria-haspopup="dialog"'],
        ["ai-new", "✨", "Create with AI", "Say what it should do; your AI chat writes it.", ' aria-haspopup="dialog"'],
        ["build", "🛠", "Build it myself", "Pick its condition and actions.", ""],
    ];
    return `
    <div class="page-head"><h1 id="page-h" tabindex="-1">New rule</h1><p class="sub">${esc(t.newRule)}</p></div>
    <div class="ways">${ways
            .map(([op, icon, name, line, extra]) => `<button class="way" type="button" id="way-${op}" data-op="${op}"${extra}>
          <span class="way-icon" aria-hidden="true">${icon}</span><span><span class="way-name">${name}</span><span class="way-line">${line}</span></span></button>`)
            .join("")}</div>`;
}

/* The templates of the trigger, in a sheet over the page (US-G15). */
function openTemplates() {
    const t = T.find((x) => x.v === st.trigger) || T[0];
    $("tpl-sub").textContent = t.newRule;
    $("tpl-body").innerHTML = `<ul class="templates">${templatesOf(t.v)
        .map((x, j) => `<li><button type="button" id="tpl-${j}" data-j="${j}"><span class="ttitle">${esc(x.title)}</span><span class="tline">${esc(sentence(x.rule))}</span></button></li>`)
        .join("")}</ul>`;
    $("tpldlg").showModal();
}

function useTemplate(j) {
    const x = templatesOf(st.trigger)[j];
    const k = edit(() => st.rules.push({ ...clone(x.rule), name: uniqueName(x.rule.name) }) - 1);
    go("rule", { sel: k, replace: true });
    /* A template with an empty list opens on it, to fill in (US-G15). */
    const list = x.focus && $(`f-rules.${k}.condition-${x.focus}`);
    if (list) list.focus();
}

$("tpl-body").addEventListener("click", (e) => {
    const b = e.target.closest("[data-j]");
    if (!b) return;
    $("tpldlg").close();
    useTemplate(+b.dataset.j);
});

/* -------------------------------------------------------------------------- */
/* The type picker                                                             */
/* -------------------------------------------------------------------------- */

const pick = { mode: null, path: null, tab: "All" };
const isActionPick = () => pick.mode === "action" || pick.mode === "reaction";

/* The types the rule's trigger takes, with a search and a tab for each subject
   (US-G3, US-G4). */
function openPicker(mode, path) {
    pick.mode = mode;
    pick.path = path;
    pick.tab = "All";
    $("pick-title").textContent = { retype: "Change the condition's type", reaction: "Change the action's type", action: "Pick an action" }[mode] || "Pick a condition";
    $("pick-q").value = "";
    $("pick-tools").hidden = isActionPick();
    drawPicker();
    $("pickdlg").showModal();
    if (!isActionPick()) $("pick-q").focus();
}

/* A closed picker holds nothing, so what it lists is not on the page twice. */
$("pickdlg").addEventListener("close", () => {
    $("pick-body").innerHTML = "";
});

function drawPicker() {
    const r = st.rules[st.sel];
    const current = pick.mode === "retype" || pick.mode === "reaction" ? at(pick.path).type : null;
    const taken = new Set(isActionPick() ? r.actions.map((a) => a.type) : []);
    const q = $("pick-q").value.trim().toLowerCase();
    const all = Object.values(isActionPick() ? A : C).filter((spec) => fits(spec, r) && (!taken.has(spec.type) || spec.type === current));
    const subjects = [...new Set(all.map((s) => s.subject))];
    if (pick.tab !== "All" && !subjects.includes(pick.tab)) pick.tab = "All";

    $("pick-tabs").innerHTML = ["All", ...subjects]
        .map((name, k) => `<button type="button" role="tab" id="tab-${k}" data-tab="${esc(name)}" aria-selected="${name === pick.tab}"
          aria-controls="pick-body" tabindex="${name === pick.tab ? 0 : -1}">${esc(name)}</button>`)
        .join("");
    $("pick-body").setAttribute("aria-labelledby", `tab-${["All", ...subjects].indexOf(pick.tab)}`);

    const shown = all.filter((s) => (pick.tab === "All" || s.subject === pick.tab) && (!q || `${s.title} ${s.gist}`.toLowerCase().includes(q)));
    const groups = new Map();
    for (const spec of shown) {
        if (!groups.has(spec.subject)) groups.set(spec.subject, []);
        groups.get(spec.subject).push(spec);
    }
    $("pick-body").innerHTML = groups.size
        ? [...groups]
            .map(([subject, specs]) => `<section aria-label="${esc(subject)}">${isActionPick() || pick.tab !== "All" ? "" : `<h3>${esc(subject)}</h3>`}<ul class="types">${specs
                .map((s) => `<li><button type="button" data-type="${esc(s.type)}"${s.type === current ? ' aria-current="true"' : ""}>
              <span class="emoji" aria-hidden="true">${s.emoji}</span><span><span class="ptitle">${esc(s.title)}</span><span class="pgist">${md(s.gist)}</span></span></button></li>`)
                .join("")}</ul></section>`)
            .join("")
        : `<p class="empty">${isActionPick() ? "Every action this rule can take is on it already." : "Nothing matches."}</p>`;
}

$("pick-q").addEventListener("input", drawPicker);

$("pick-tabs").addEventListener("click", (e) => {
    const b = e.target.closest("[data-tab]");
    if (!b) return;
    pick.tab = b.dataset.tab;
    drawPicker();
    $(b.id).focus();
});

/* The arrow keys, Home and End move between the tabs. */
$("pick-tabs").addEventListener("keydown", (e) => {
    const tabs = [...$("pick-tabs").querySelectorAll("[role=tab]")], k = tabs.indexOf(e.target);
    const to = { ArrowRight: (k + 1) % tabs.length, ArrowLeft: (k - 1 + tabs.length) % tabs.length, Home: 0, End: tabs.length - 1 }[e.key];
    if (k < 0 || to === undefined) return;
    e.preventDefault();
    pick.tab = tabs[to].dataset.tab;
    drawPicker();
    $(`tab-${to}`).focus();
});

$("pick-body").addEventListener("click", (e) => {
    const b = e.target.closest("[data-type]");
    if (!b) return;
    const r = st.rules[st.sel];
    const spec = (isActionPick() ? A : C)[b.dataset.type];
    let focus = null;
    edit(() => {
        if (pick.mode === "condition") {
            const n = fresh(spec);
            if (!r.condition) r.condition = n;
            else {
                const and = C[andType()];
                r.condition = { ...fresh(and), [manyKey({ type: and.type })]: [r.condition, n] };
            }
        } else if (pick.mode === "child") {
            const n = at(pick.path);
            n[manyKey(n)].push(fresh(spec));
            st.folded.delete(pick.path);
        } else if (pick.mode === "retype") {
            const { holder, key } = parent(pick.path);
            holder[key] = retype(holder[key], spec, cond(holder[key].type));
            st.folded.delete(pick.path);
            focus = `m-${pick.path}`;
        } else if (pick.mode === "action") {
            r.actions.push(fresh(spec));
        } else if (pick.mode === "reaction") {
            const { holder, key } = parent(pick.path);
            holder[key] = retype(holder[key], spec, act(holder[key].type));
            focus = `m-${pick.path}`;
        }
    });
    $("pickdlg").close();
    $("pick-body").innerHTML = "";
    render();
    if (focus && $(focus)) $(focus).focus();
});

/* -------------------------------------------------------------------------- */
/* Events                                                                      */
/* -------------------------------------------------------------------------- */

function moveRule(i, d) {
    const order = inSection(triggerOf(st.rules[i]).v).map(([, j]) => j);
    const to = order[order.indexOf(i) + d];
    if (to === undefined) return null;
    edit(() => ([st.rules[i], st.rules[to]] = [st.rules[to], st.rules[i]]));
    return to;
}

/* A template's name the rules already have gets a number (US-G15). */
function uniqueName(name) {
    const taken = new Set(st.rules.map((r) => r.name));
    if (!name || !taken.has(name)) return name;
    let n = 2;
    while (taken.has(`${name} ${n}`)) n++;
    return `${name} ${n}`;
}

document.addEventListener("click", (e) => {
    if (e.target.closest("dialog")) return;
    const b = e.target.closest("[data-op]");
    const open = st.menu;
    if (!b || b.dataset.op !== "menu") st.menu = null;
    if (!b) {
        if (open) render();
        return;
    }
    const op = b.dataset.op, path = b.dataset.path, i = +b.dataset.i;
    const r = st.rules[st.sel];
    let after = null;

    switch (op) {
        case "undo": return undo();
        case "redo": return redo();
        case "home": return go("home");
        case "open": return go("rule", { sel: i });
        case "new": return go("new", { trigger: b.dataset.t });
        case "ai-all": render(); return document.dispatchEvent(new CustomEvent("ai:open", { detail: { mode: "all" } }));
        case "ai-rule": return document.dispatchEvent(new CustomEvent("ai:open", { detail: { mode: "rule", index: st.sel } }));
        case "ai-new": return document.dispatchEvent(new CustomEvent("ai:open", { detail: { mode: "create", trigger: st.trigger } }));
        case "import": render(); return document.dispatchEvent(new CustomEvent("json:import"));
        case "export": render(); return document.dispatchEvent(new CustomEvent("json:export"));
        case "menu":
            st.menu = open === b.dataset.menu ? null : b.dataset.menu;
            render();
            if (st.menu && e.detail === 0) {
                const first = $(`m-${st.menu}`).parentElement.querySelector("[role=menuitem]");
                if (first) first.focus();
            }
            return;
        case "more": st.more.has(b.dataset.key) ? st.more.delete(b.dataset.key) : st.more.add(b.dataset.key); break;
        case "help": st.help.has(b.dataset.key) ? st.help.delete(b.dataset.key) : st.help.add(b.dataset.key); break;
        case "fold": st.folded.has(path) ? st.folded.delete(path) : st.folded.add(path); break;
        case "fold-all": {
            const root = `rules.${st.sel}.condition`;
            if ([...st.folded].some((p) => p.startsWith(root))) st.folded = new Set([...st.folded].filter((p) => !p.startsWith(root)));
            else foldAll(r.condition, root, st.folded);
            break;
        }
        case "only": st.only = path; break;
        case "whole": st.only = null; break;
        case "reveal": st.shown.has(b.dataset.key) ? st.shown.delete(b.dataset.key) : st.shown.add(b.dataset.key); break;
        case "bulk":
            st.bulk.has(b.dataset.key) ? st.bulk.delete(b.dataset.key) : st.bulk.add(b.dataset.key);
            delete st.filter[b.dataset.key];
            break;
        case "templates": return openTemplates();
        case "pick":
            render();
            return openPicker(b.dataset.mode, path);
        case "build": {
            const k = edit(() => st.rules.push({ trigger: st.trigger, actions: [] }) - 1);
            go("rule", { sel: k, replace: true });
            return openPicker("condition");
        }
        case "toggle":
            edit(() => {
                const x = st.rules[i];
                if (isOn(x)) x.enabled = false;
                else delete x.enabled;
            });
            if (b.id === "enable") after = "r-name";
            else if (b.getAttribute("role") === "menuitem") after = `m-r${i}`;
            break;
        case "drop": {
            const section = triggerOf(st.rules[i]).v;
            edit(() => st.rules.splice(i, 1));
            after = `add-${section}`;
            break;
        }
        case "move": {
            const to = moveRule(i, +b.dataset.d);
            if (to !== null) after = `open-${to}`;
            break;
        }
        case "duplicate": {
            const base = titleOf(r), room = (R.name.maxLength || 60) - " (copy)".length;
            const copy = { ...clone(r), name: `${base.length > room ? base.slice(0, room - 1) + "…" : base} (copy)` };
            edit(() => st.rules.splice(st.sel + 1, 0, copy));
            return go("rule", { sel: st.sel + 1 });
        }
        /* No confirmation: Undo takes it back, and Apply shows it before
           anything reaches the bot. */
        case "remove":
            edit(() => st.rules.splice(st.sel, 1));
            return go("home");
        case "wrap": {
            const spec = C[b.dataset.type], { holder, key } = parent(path);
            edit(() => {
                const w = fresh(spec), one = oneKey(w), many = manyKey(w);
                if (one) w[one] = holder[key];
                else w[many] = [holder[key]];
                holder[key] = w;
            });
            st.only = null;
            after = `m-${path}`;
            break;
        }
        /* A container is never a one-way door: its conditions move up into its
           place. */
        case "unwrap": {
            const n = at(path), kids = oneKey(n) ? [n[oneKey(n)]] : (n[manyKey(n)] || []).slice();
            const { holder, key } = parent(path);
            if (!Array.isArray(holder) && kids.length > 1) {
                toast("This holds several conditions and sits at the top of the rule, which has room for one. Move them out first.");
                break;
            }
            edit(() => (Array.isArray(holder) ? holder.splice(+key, 1, ...kids) : (holder[key] = kids[0])));
            st.only = null;
            break;
        }
        case "delete": {
            const { holder, key } = parent(path);
            if (Array.isArray(holder) && holder.length === 1) {
                toast("A combined condition needs at least one condition inside.");
                break;
            }
            /* The rule's own condition goes too: a rule whose trigger needs one
               is incomplete until it has another (US-G14). */
            edit(() => (Array.isArray(holder) ? holder.splice(+key, 1) : delete holder[key]));
            st.only = null;
            after = "add-cond";
            break;
        }
        case "unact": {
            const { holder, key } = parent(path);
            edit(() => holder.splice(+key, 1));
            after = "add-act";
            break;
        }
        case "unentry":
            edit(() => at(path)[b.dataset.key].splice(+b.dataset.j, 1));
            after = `f-${path}-${b.dataset.key}`;
            break;
        default:
            return;
    }
    render();
    if (after && $(after)) $(after).focus();
});

document.addEventListener("change", (e) => {
    if (e.target.closest("dialog")) return;
    const b = e.target.closest("[data-op]");
    if (!b) return;
    const op = b.dataset.op, path = b.dataset.path, key = b.dataset.key;
    const r = st.rules[st.sel];
    const fields = () => (path === "settings" ? S : [...cond(at(path).type).fields, ...act(at(path).type).fields]);

    if (op === "setting") edit(() => (st.settings[key] = S.find((f) => f.key === key).choices[+b.value].v));
    else if (op === "name") edit(() => (b.value.trim() ? (r.name = b.value.trim()) : delete r.name));
    else if (op === "when") {
        /* The rule moves to the end of its new section (US-G3). */
        edit(() => {
            r.trigger = b.value;
            st.rules.splice(st.sel, 1);
            st.sel = st.rules.push(r) - 1;
        });
        history.replaceState({ view: "rule", sel: st.sel }, "", location.hash);
    } else if (op === "num") {
        const f = fields().find((x) => x.key === key);
        /* An emptied number keeps its value: 0 would mean something else. */
        if (b.value.trim() !== "") {
            let v = Math.round(+b.value);
            if (!Number.isFinite(v)) v = f.def;
            const lo = f.min ?? -Infinity, hi = f.max ?? Infinity;
            if (v < lo || v > hi) {
                v = Math.min(hi, Math.max(lo, v));
                toast(`${f.label}: kept within ${f.min ?? "…"}–${f.max ?? "…"}.`);
            }
            edit(() => (at(path)[key] = v));
        }
    } else if (op === "bool") edit(() => (at(path)[key] = b.checked));
    else if (op === "text") edit(() => (at(path)[key] = b.value));
    else if (op === "choice") {
        const f = fields().find((x) => x.key === key), now = at(path)[key];
        edit(() => (at(path)[key] = b.value === "n" ? (typeof now === "number" ? now : f.num.default ?? f.min ?? 0) : f.choices[+b.value].v));
    } else if (op === "lines") {
        const out = [...new Set(b.value.split("\n").map((x) => x.trim()).filter(Boolean))];
        edit(() => (at(path)[key] = out));
    } else return;
    /* Drawn again once the focus has moved on, and put back where it went. */
    setTimeout(() => {
        const id = document.activeElement && document.activeElement.id;
        render();
        if (id && $(id)) $(id).focus();
    });
});

document.addEventListener("input", (e) => {
    const b = e.target.closest('[data-op="filter"]');
    if (!b) return;
    st.filter[b.dataset.key] = b.value;
    render();
    const again = $(b.id);
    again.focus();
    again.setSelectionRange(again.value.length, again.value.length);
});

document.addEventListener("keydown", (e) => {
    const t = e.target;
    if (t.closest("dialog")) return;

    /* A list entry goes in with Enter (US-G3). */
    if (t.dataset.op === "entry" && e.key === "Enter") {
        e.preventDefault();
        const v = t.value.trim();
        if (!v) return;
        const list = at(t.dataset.path)[t.dataset.key];
        if (list.includes(v)) {
            t.value = "";
            return toast("That entry is already in the list.");
        }
        edit(() => list.push(v));
        const key = `${t.dataset.path}.${t.dataset.key}`;
        if (st.filter[key] && !v.toLowerCase().includes(st.filter[key].toLowerCase())) delete st.filter[key];
        render();
        return $(t.id).focus();
    }

    /* The arrow keys on a rule's handle move it (US-G12). */
    if (t.dataset.drag !== undefined && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        e.preventDefault();
        const to = moveRule(+t.dataset.drag, e.key === "ArrowUp" ? -1 : 1);
        if (to === null) return;
        render();
        $(`grip-${to}`).focus();
        return;
    }

    /* A menu: arrows, Home and End move through it; Escape and Tab leave it. */
    const pop = t.closest(".pop");
    if (pop) {
        const items = [...pop.querySelectorAll("[role=menuitem]")], k = items.indexOf(t);
        const to = { ArrowDown: (k + 1) % items.length, ArrowUp: (k - 1 + items.length) % items.length, Home: 0, End: items.length - 1 }[e.key];
        if (to !== undefined) {
            e.preventDefault();
            items[to].focus();
            return;
        }
    }
    if (st.menu && (e.key === "Escape" || (e.key === "Tab" && pop))) {
        const id = `m-${st.menu}`;
        st.menu = null;
        render();
        if (e.key === "Escape" && $(id)) $(id).focus();
        return;
    }

    /* Undo and Redo, outside a text field, where the browser's own wins. */
    if ((e.ctrlKey || e.metaKey) && !t.closest("input, textarea")) {
        const k = e.key.toLowerCase();
        if (k === "z" && !e.shiftKey) { e.preventDefault(); undo(); }
        else if ((k === "z" && e.shiftKey) || k === "y") { e.preventDefault(); redo(); }
    }
});

/* Dragging a rule by its handle, with a mouse or a finger (US-G12). */
let drag = null;
document.addEventListener("pointerdown", (e) => {
    const h = e.target.closest("[data-drag]");
    if (!h || e.button > 0) return;
    const row = h.closest(".item");
    const rows = [...row.parentElement.children];
    drag = { row, rows, from: rows.indexOf(row), to: rows.indexOf(row), y: e.clientY, moved: false, id: e.pointerId };
    h.setPointerCapture(e.pointerId);
});

document.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const dy = e.clientY - drag.y;
    if (!drag.moved && Math.abs(dy) < 5) return;
    drag.moved = true;
    drag.row.classList.add("dragging");
    drag.row.style.transform = `translateY(${dy}px)`;
    const mid = drag.row.getBoundingClientRect().top + drag.row.offsetHeight / 2;
    let to = drag.from;
    drag.rows.forEach((x, k) => {
        if (x === drag.row) return;
        const box = x.getBoundingClientRect(), centre = box.top + box.height / 2;
        if (k < drag.from && mid < centre) to = Math.min(to, k);
        if (k > drag.from && mid > centre) to = Math.max(to, k);
    });
    drag.to = to;
    drag.rows.forEach((x, k) => {
        x.classList.toggle("drop-above", k === to && to < drag.from);
        x.classList.toggle("drop-below", k === to && to > drag.from);
    });
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
    const v = d.row.closest("[data-trigger]").dataset.trigger;
    const places = inSection(v).map(([, j]) => j);
    const order = places.slice();
    order.splice(d.to, 0, order.splice(d.from, 1)[0]);
    edit(() => {
        const rules = order.map((j) => st.rules[j]);
        places.forEach((p, k) => (st.rules[p] = rules[k]));
    });
    render();
});

/* Every dialog closes by its ×, its Close or Cancel. */
document.addEventListener("click", (e) => {
    const x = e.target.closest("dialog [data-close]");
    if (x) x.closest("dialog").close();
});

/* -------------------------------------------------------------------------- */
/* What changed (US-G7)                                                        */
/* -------------------------------------------------------------------------- */

/* Rules have no ids, so they are paired by what they are: the same rule first,
   then the same name, or the same condition and actions. Each section is its
   own list, and a rule that changed section is found across them. */
function changes(before, after) {
    const settings = S.filter((f) => canon(before[f.key]) !== canon(after[f.key]));
    const rows = [], gone = [], come = [];
    for (const t of T) {
        const was = before.rules.filter((r) => triggerOf(r).v === t.v);
        const now = after.rules.filter((r) => triggerOf(r).v === t.v);
        for (const row of pairUp(was, now, t.v)) (row.status === "removed" ? gone : row.status === "added" ? come : rows).push(row);
    }
    const used = new Set();
    for (const c of come) {
        const k = gone.findIndex((g, j) => !used.has(j) && g.section !== c.section && alike(g.rule, c.rule) >= 3);
        if (k >= 0) {
            used.add(k);
            rows.push({ ...c, status: "changed", marks: ["changed"], was: gone[k].rule, notes: [`was ${sectionName(gone[k].section)}`], details: details(gone[k].rule, c.rule) });
        } else rows.push(c);
    }
    gone.forEach((g, j) => !used.has(j) && rows.push(g));
    return { settings, rows };
}

const sectionName = (v) => (T.find((t) => t.v === v) || T[0]).section;

function alike(a, b) {
    const sameCond = a.condition && b.condition && canon(a.condition) === canon(b.condition);
    const sameType = a.condition && b.condition && a.condition.type === b.condition.type;
    return (a.name && a.name === b.name ? 5 : 0) + (sameCond ? 4 : sameType ? 2 : 0) + (actionWords(a) === actionWords(b) ? 2 : 0);
}

function pairUp(was, now, section) {
    const left = was.map((r, i) => ({ r, i, key: canon(r) }));
    const right = now.map((r, i) => ({ r, i, key: canon(r) }));
    const takenL = new Set(), takenR = new Set(), pairs = [];
    for (const b of right) {
        const a = left.find((x) => !takenL.has(x.i) && x.key === b.key);
        if (a) { takenL.add(a.i); takenR.add(b.i); pairs.push([a, b]); }
    }
    for (const b of right) {
        if (takenR.has(b.i)) continue;
        let best = null, score = 0;
        for (const a of left) if (!takenL.has(a.i) && alike(a.r, b.r) > score) { best = a; score = alike(a.r, b.r); }
        if (best && score >= 3) { takenL.add(best.i); takenR.add(b.i); pairs.push([best, b]); }
    }
    /* What kept its order among the paired is what stayed; the rest moved. */
    pairs.sort((x, y) => x[1].i - y[1].i);
    const stayed = longestRun(pairs.map(([a]) => a.i));
    const rows = pairs.map(([a, b], k) => {
        const notes = [], marks = [];
        if (a.key !== b.key) {
            const { name: n1, enabled: e1, ...rest1 } = a.r, { name: n2, enabled: e2, ...rest2 } = b.r;
            if (canon(rest1) !== canon(rest2)) marks.push("changed");
            if ((n1 || "") !== (n2 || "")) { marks.push("renamed"); notes.push(`was “${n1 || ""}”`); }
            if (isOn(a.r) !== isOn(b.r)) marks.push(isOn(b.r) ? "turned on" : "turned off");
        }
        if (!stayed.has(k)) { marks.push("moved"); notes.push(`was ${ordinal(a.i + 1)}`); }
        return { status: marks[0] || "same", marks, index: b.i, rule: b.r, was: a.r, section, notes, details: marks.includes("changed") ? details(a.r, b.r) : [] };
    });
    right.filter((b) => !takenR.has(b.i)).forEach((b) => rows.push({ status: "added", marks: ["added"], index: b.i, rule: b.r, section, notes: [] }));
    left.filter((a) => !takenL.has(a.i)).forEach((a) => rows.push({ status: "removed", marks: ["removed"], index: a.i, rule: a.r, section, notes: [] }));
    return rows;
}

/* Positions of a longest increasing run, as a Set. */
function longestRun(seq) {
    const len = seq.map(() => 1), prev = seq.map(() => -1);
    let end = -1;
    seq.forEach((x, i) => {
        for (let j = 0; j < i; j++) if (seq[j] < x && len[j] + 1 > len[i]) { len[i] = len[j] + 1; prev[i] = j; }
        if (end < 0 || len[i] > len[end]) end = i;
    });
    const out = new Set();
    for (let i = end; i >= 0; i = prev[i]) out.add(i);
    return out;
}

function changeCount() {
    const d = changes(st.baseline, config());
    return d.settings.length + d.rows.filter((x) => x.status !== "same").length;
}

const secretTail = (v) => (v ? `…${String(v).slice(-4)}` : "empty");

function details(a, b) {
    const out = [];
    if (actionWords(a) !== actionWords(b)) out.push({ text: `actions: ${actionWords(a) || "none"} → ${actionWords(b) || "none"}` });
    if (!a.condition !== !b.condition) out.push({ text: `condition: ${condWords(a)} → ${condWords(b)}` });
    else if (a.condition) treeDetails(a.condition, b.condition, out, "");
    return out;
}

/* Two trees side by side. A list that lost entries is called out in red: it is
   the change that would otherwise slip through. */
function treeDetails(a, b, out, where) {
    if (out.length > 8) return;
    if (a.type !== b.type) return out.push({ text: `${where}${cond(a.type).title} → ${cond(b.type).title}` });
    for (const f of cond(a.type).fields) {
        const x = a[f.key], y = b[f.key];
        if (f.kind === "child") treeDetails(x, y, out, where);
        else if (f.kind === "children") childDetails(cond(a.type).title, x || [], y || [], out);
        else if (f.kind === "list") {
            if (canon(x) === canon(y)) continue;
            const removed = (x || []).filter((v) => !(y || []).includes(v)), added = (y || []).filter((v) => !(x || []).includes(v));
            if (!removed.length && !added.length) out.push({ text: `${where}${f.key}: same ${(x || []).length} entries, reordered` });
            else out.push({
                text: `${where}${f.key}: ${[removed.length && `${removed.length} removed`, added.length && `${added.length} added`].filter(Boolean).join(", ")} (${(x || []).length} → ${(y || []).length})`,
                alarm: removed.length > 0, removed, added,
            });
        } else if (f.kind === "secret") { if (x !== y) out.push({ text: `${where}${f.key}: replaced (${secretTail(x)} → ${secretTail(y)})` }); }
        else if (f.kind === "prose") { if (x !== y) out.push({ text: `${where}${f.key}: rewritten (${String(x ?? "").length} → ${String(y ?? "").length} characters)` }); }
        else if (f.kind === "choice") { if (x !== y) out.push({ text: `${where}${f.label}: ${choiceText(f, x)} → ${choiceText(f, y)}` }); }
        else if (x !== y) out.push({ text: `${where}${f.key}: ${x} → ${y}` });
    }
}

/* A container's children are paired like rules: identical first, then by
   type; their order means nothing to the bot. */
function childDetails(title, xs, ys, out) {
    const left = xs.map((n, i) => ({ n, i, key: canon(n) })), same = new Set(), edited = new Set(), pairs = [], added = [];
    const rest = ys.filter((n) => {
        const m = left.find((a) => !same.has(a.i) && a.key === canon(n));
        if (m) same.add(m.i);
        return !m;
    });
    for (const n of rest) {
        const m = left.find((a) => !same.has(a.i) && !edited.has(a.i) && a.n.type === n.type);
        if (m) { edited.add(m.i); pairs.push([m.n, n]); } else added.push(n);
    }
    const removed = left.filter((a) => !same.has(a.i) && !edited.has(a.i)).map((a) => a.n);
    if (xs.length !== ys.length) out.push({ text: `${title}: ${xs.length} → ${ys.length} conditions` });
    removed.forEach((n) => out.push({ text: `removed: ${cond(n.type).emoji} ${cond(n.type).title}`, node: n, alarm: true }));
    added.forEach((n) => out.length <= 8 && out.push({ text: `added: ${cond(n.type).emoji} ${cond(n.type).title}`, node: n }));
    pairs.forEach(([a, b]) => treeDetails(a, b, out, `${cond(b.type).title} · `));
}

const PREVIEW = 40;
const chipsOf = (xs, cls) =>
    xs.slice(0, PREVIEW).map((x) => `<span class="${cls}">${esc(x)}</span>`).join("") + (xs.length > PREVIEW ? `<span class="more-n">+${xs.length - PREVIEW} more</span>` : "");

function detailHtml(x) {
    const line = `<span class="d${x.alarm ? " alarm" : ""}">${esc(x.text)}</span>`;
    if (x.node) {
        const lists = cond(x.node.type).fields.filter((f) => f.kind === "list" && (x.node[f.key] || []).length);
        return `<details class="d-more"><summary>${line}</summary><div><p>${esc(phraseOf(x.node))}</p>${lists
            .map((f) => `<p class="d-list"><b>${esc(f.key)}</b>${chipsOf(x.node[f.key], x.alarm ? "rm" : "ad")}</p>`).join("")}</div></details>`;
    }
    if (!x.removed) return line;
    return `<details class="d-more"><summary>${line}</summary><div>
      ${x.removed.length ? `<p class="d-list"><b class="rm-k">removed</b>${chipsOf(x.removed, "rm")}</p>` : ""}
      ${x.added.length ? `<p class="d-list"><b class="ad-k">added</b>${chipsOf(x.added, "ad")}</p>` : ""}</div></details>`;
}

function ruleLine(r) {
    const name = r.name || (missing(r) ? "New rule" : "");
    const words = !r.condition && !(r.actions || []).length ? ""
        : `<span class="kw if">IF</span> ${esc(condWords(r))} <span class="kw then">THEN</span> ${esc(actionWords(r) || "—")}`;
    return `${name ? `<b>${esc(name)}</b> ` : ""}${words}`;
}

/* The link travels in one chat message, and that is its budget:
   `MESSAGE_MAX_LENGTH_IN_BYTES` in drivers/simplex/consts.rs, mirrored in the
   schema. */
function sizeWarning() {
    const limit = (SCHEMA.options && SCHEMA.options.message_max_bytes) || 15000;
    const bytes = new TextEncoder().encode(linkMessage()).length;
    if (bytes <= limit * 0.7) return "";
    const n = (x) => x.toLocaleString("en-US");
    return bytes > limit
        ? `<p class="alert bad">This link is ${n(bytes)} bytes — past the ${n(limit)} a single chat message can carry. The bot will not be able to read it. Shorten a long list before sending.</p>`
        : `<p class="alert warn">This link is ${n(bytes)} bytes, ${Math.round((bytes / limit) * 100)}% of what a single chat message can carry. Once it passes ${n(limit)}, the bot will no longer be able to read it.</p>`;
}

function settingText(f, v) {
    if (f.kind === "choice") return choiceText(f, v);
    if (f.kind === "bool") return v ? "on" : "off";
    return String(v);
}

function openApply() {
    const d = changes(st.baseline, config());
    const shown = d.rows.filter((x) => x.status !== "same");
    const same = d.rows.filter((x) => x.status === "same").map((x) => titleOf(x.rule));
    $("apply-meta").textContent = `${st.baseline.rules.length} → ${st.rules.length} rules`;

    const settings = d.settings.map((f) => `<div class="change"><span class="marks"><span class="mark changed">changed</span></span>
      <span class="what">${esc(f.label)}: ${esc(settingText(f, st.baseline[f.key]))} → <b>${esc(settingText(f, st.settings[f.key]))}</b></span></div>`).join("");
    const sections = T.map((t) => {
        const rows = shown.filter((x) => x.section === t.v).sort((a, b) => a.index - b.index);
        return rows.length ? `<h3>${esc(t.section)}</h3>` + rows.map((x) => `<div class="change">
          <span class="marks">${x.marks.map((m) => `<span class="mark ${m.replace(" ", "-")}">${m}</span>`).join("")}</span>
          <span class="what">${ruleLine(x.rule)}${x.notes.map((n) => `<span class="d">${esc(n)}</span>`).join("")}${(x.details || []).map(detailHtml).join("")}</span></div>`).join("") : "";
    }).join("");

    $("apply-body").innerHTML = sizeWarning() + settings + (shown.length
        ? sections + (same.length ? `<p class="quiet">Unchanged: ${esc(same.join(", "))}</p>` : "")
        : settings ? "" : `<p class="quiet">Nothing has changed since you opened this page.${st.demo ? "" : " You can still copy the link and send it again."}</p>`);

    /* A rule the bot would refuse is not sent at all (US-G14). */
    const k = st.rules.findIndex(missing);
    $("apply-foot").innerHTML = k >= 0
        ? `<p class="blocked" role="alert">“${esc(titleOf(st.rules[k]))}” has no ${missing(st.rules[k])} yet. Finish it or delete it before sending.</p>
       <button class="btn" type="button" data-close>Cancel</button>
       <button class="btn primary" type="button" id="apply-open" data-i="${k}">Open “${esc(titleOf(st.rules[k]))}”</button>`
        : st.demo
            ? `<p class="foot-note">This is a demo, so there is nothing to send. Invite the bot to your group, and the link it sends you opens this editor on your group's rules.</p>
       <button class="btn primary" type="button" data-close>Close</button>`
            : `<button class="btn" type="button" data-close>Cancel</button><button class="btn primary" type="button" id="apply-copy">Copy the link</button>`;
    $("applydlg").showModal();
}

$("apply").addEventListener("click", openApply);

$("applydlg").addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    if (b.id === "apply-open") {
        $("applydlg").close();
        go("rule", { sel: +b.dataset.i });
        return;
    }
    if (b.id !== "apply-copy") return;
    /* Copying is the last step, so the baseline moves to what was copied. */
    navigator.clipboard.writeText(linkMessage())
        .then(() => {
            st.baseline = snapshot();
            renderBar();
            $("apply-meta").textContent = "";
            $("apply-body").innerHTML = `<p class="done" role="status"><b>Link copied.</b> Paste it into your chat with the bot and send it — that is what applies the changes.</p>`;
            $("apply-foot").innerHTML = `<button class="btn primary" type="button" data-close>Close</button>`;
        })
        .catch(() => {
            $("apply-body").innerHTML = `<p class="alert bad" role="alert"><b>Could not reach the clipboard.</b> Copy the whole address from the address bar and send that to the bot.</p>`;
        });
});

/* -------------------------------------------------------------------------- */
/* Start                                                                       */
/* -------------------------------------------------------------------------- */

function readSettings(obj) {
    const errors = [], out = Object.fromEntries(S.map((f) => [f.key, clone(f.def)]));
    for (const k of Object.keys(obj)) if (!S.some((f) => f.key === k)) errors.push(`There is no setting “${k}”.`);
    for (const f of S) {
        if (!(f.key in obj)) continue;
        const v = obj[f.key];
        const ok = f.kind === "choice" ? f.choices.some((c) => c.v === v) || (f.num && Number.isInteger(v))
            : f.kind === "bool" ? typeof v === "boolean" : f.kind === "int" ? Number.isInteger(v) : typeof v === "string";
        if (ok) out[f.key] = v;
        else errors.push(`${f.label}: “${JSON.stringify(v)}” is not a value it can take.`);
    }
    return errors.length ? { errors } : { settings: out };
}

async function start() {
    if (typeof LZString === "undefined") throw new Error("lz-string failed to load");
    SCHEMA = await fetch("rules-schema.json").then((r) => {
        if (!r.ok) throw new Error(`rules-schema.json: HTTP ${r.status}`);
        return r.json();
    });
    R = SCHEMA.properties.rules.items.properties;
    T = R.trigger.oneOf.map((b) => {
        const o = b.options || {};
        return {
            v: b.const, title: b.title, section: o.section || b.title, newRule: o.new_rule || b.title,
            empty: o.empty || "No rules yet.", incompatible: o.incompatible || "Doesn't work here",
            incomplete: o.incomplete || "Pick a condition and at least one action.", prompt: o.prompt || b.title,
            optional: !!o.condition_optional, everyone: o.everyone || "Always",
        };
    });
    const firstOnly = [R.trigger.default || T[0].v];
    C = registry(SCHEMA.definitions.condition.oneOf, firstOnly, false);
    A = registry(SCHEMA.definitions.action.oneOf, firstOnly, true);
    S = Object.entries(SCHEMA.properties).filter(([k]) => k !== "rules").map(([k, pr]) => fieldOf(k, pr));
    st.settings = Object.fromEntries(S.map((f) => [f.key, clone(f.def)]));
    const lim = SCHEMA.definitions.condition.options || {};
    st.limits = { depth: lim.max_depth || 8, nodes: lim.max_nodes || 64 };

    const hash = Object.fromEntries(location.hash.slice(1).split("&").filter((p) => p.includes("=")).map((p) => [p.slice(0, p.indexOf("=")), p.slice(p.indexOf("=") + 1)]));
    st.botId = hash.bot_id || null;
    st.demo = !st.botId;
    try { st.group = hash.group ? decodeURIComponent(hash.group) : null; } catch (e) { st.group = null; }
    if (hash.config) {
        try {
            const { rules, ...settings } = JSON.parse(LZString.decompressFromEncodedURIComponent(hash.config));
            const read = readSettings(settings);
            if (!Array.isArray(rules) || read.errors) throw new Error((read.errors || ["no rules"]).join(" "));
            /* Every rule names its trigger, so a link that leaves the default
               out compares equal to one that writes it. */
            st.rules = rules.map((r) => ({ ...r, trigger: triggerOf(r).v }));
            st.settings = read.settings;
        } catch (err) {
            console.warn("Could not read the settings in the link:", err);
            st.unreadable = true;
        }
    }
    /* The demo with no settings of its own shows one rule of each template, so
       a person sees what a group's rules look like (US-G18). */
    if (st.demo && !hash.config) {
        st.rules = ((SCHEMA.options && SCHEMA.options.templates) || []).map((x) => ({ ...clone(x.rule), trigger: triggerOf(x.rule).v }));
    }
    st.baseline = snapshot();

    $("demo").hidden = !st.demo;
    $("boot").hidden = true;
    $("app").hidden = false;
    history.replaceState({ view: "home" }, "", location.hash);
    render();
    document.dispatchEvent(new CustomEvent("editor:ready"));
}

start().catch((err) => {
    $("boot").textContent = `Failed to load the editor: ${err.message}`;
    $("boot").classList.add("failed");
});
