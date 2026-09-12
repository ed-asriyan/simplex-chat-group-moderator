/*
 * Rules editor for the SimpleX Chat group moderator bot.
 *
 * SimpleX carries plain text only, so the whole round trip is one URL: the bot
 * sends `#bot_id=<id>&rules=<lz>`, this page edits the rules, writes them back
 * into the hash on every change, and the owner pastes the link into the chat.
 * Both hash parameters must always be present or the bot ignores the message.
 *
 * Nothing here knows a single condition or action type by name. The registry is
 * derived from rules-schema.json, so adding a rule type is one `oneOf` entry in
 * that file and no change to this code. What the schema cannot express — the
 * one-line summary, the human phrase, the execution order of actions — lives in
 * optional `options` keys on the same entry (summary, phrase, execution_rank,
 * covered_by, label), each with a fallback when absent.
 */

/* ------------------------------------------------------------------ *
 * Layout vocabulary
 *
 * Every condition and action renders as ONE ROW:
 *   [▾] [emoji] [type ▾] [summary] [ⓘ] [⋮]
 * Parameters open inside that row, never as a nested card, and prose is
 * always behind ⓘ. Depth is a rail plus a small indent, so a six-level
 * tree still fits a phone. Four kinds of parameter cover every type:
 * integer, boolean, list of strings, and nested condition(s).
 * ------------------------------------------------------------------ */

const EDITOR_URL_BASE = location.href.split("#")[0];

/* Emoji is the subject taxonomy (💬 text, 🔗 links, 📏 shape, 👤 author), so it
   also groups the type picker. An unknown emoji falls into "Other" rather than
   breaking the list. */
const GROUPS = {
    "💬": "Message text",
    "🔗": "Links",
    "📏": "Size & shape",
    "👤": "Author",
    "🧩": "Combine",
    "🔀": "Combine",
    "✖️": "Combine",
};

const state = {
    rules: [],
    botId: null,
    sel: 0,
    view: "list",
    focus: null,
    collapsed: new Set(),
    helped: new Set(),
    bulk: new Set(),
    q: {},
    menu: null,
    limits: { depth: 8, nodes: 64 },
};

let C = {};   // condition registry, built from the schema
let A = {};   // action registry
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

/* A rule saved by a newer bot than this page's schema still has to render, and
   above all has to survive being sent back untouched. */
function unknownSpec(type) {
    return {
        g: "❓",
        grp: "Other",
        t: type,
        full: type,
        help: "This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.",
        p: [],
        rank: 0,
        coveredBy: [],
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
    return "text";
}

function defaultFor(pr) {
    if ("default" in pr) return structuredClone(pr.default);
    const k = kindOf(pr);
    if (k === "strlist" || k === "children") return [];
    if (k === "bool") return false;
    if (k === "int") return 0;
    return "";
}

function buildRegistry(entries) {
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
            help: e.description ? md(e.description) : "",
            t: o.short || rest.replace(/^(Message|Author|This Condition)\s+/, ""),
            rank: o.execution_rank || 0,
            coveredBy: o.covered_by || [],
            p: fields.map(([k, pr]) => ({
                k,
                kind: kindOf(pr),
                label: (pr.options && pr.options.label) || pr.title || k,
                hint: pr.description ? md(pr.description) : "",
                min: pr.minimum,
                max: pr.maximum,
                maxItems: pr.maxItems,
                maxLength: pr.items && pr.items.maxLength,
            })),
            def: Object.fromEntries(fields.map(([k, pr]) => [k, defaultFor(pr)])),
        };
        entry.sum = (v) =>
            o.summary
                ? tpl(o.summary, v)
                : entry.p.filter((f) => f.kind === "int").map((f) => `${f.k} ${v[f.k]}`).join(" · ");
        entry.say = (v) => (o.phrase ? tpl(o.phrase, v) : entry.t.toLowerCase());
        reg[key] = entry;
    }
    return reg;
}

/* ------------------------------ tree walking --------------------------- */

const at = (p) => p.split(".").reduce((o, k) => o[k], state);
const parentOf = (p) => ({ o: at(p.slice(0, p.lastIndexOf("."))), k: p.slice(p.lastIndexOf(".") + 1) });

/* Composites are found through the schema's own shape, not by name. */
const childKeyOf = (n) => (spec(C, n.type).p.find((f) => f.kind === "child") || {}).k;
const childrenKeyOf = (n) => (spec(C, n.type).p.find((f) => f.kind === "children") || {}).k;

function depthOf(n) {
    const one = childKeyOf(n), many = childrenKeyOf(n);
    if (one && n[one]) return 1 + depthOf(n[one]);
    if (many) return 1 + Math.max(0, ...(n[many] || []).map(depthOf));
    return 1;
}

function nodesOf(n) {
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

const newCondition = () => ({ type: Object.keys(C)[0], ...structuredClone(C[Object.keys(C)[0]].def) });
const newRule = () => ({ actions: [{ type: "ModerateMessage" }], condition: newCondition() });

/* --------------------------------- URL --------------------------------- */

/* The hash is the transport: it is rewritten on every change so the address
   bar (and therefore the copied link) is always what the bot would store. */
function syncHash() {
    const compressed = LZString.compressToEncodedURIComponent(JSON.stringify(state.rules));
    const parts = (state.botId ? `bot_id=${state.botId}&` : "") + `rules=${compressed}`;
    history.replaceState(null, "", "#" + parts);
}

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
    el.textContent = text;
    document.body.appendChild(el);
    setTimeout(() => el.remove(), 2800);
}

function renderList() {
    document.getElementById("count").textContent = `${state.rules.length} total`;
    const host = document.getElementById("rlist");
    if (!state.rules.length) {
        host.innerHTML = `<div class="empty">No rules yet. Nothing is moderated until you add one.</div>`;
        return;
    }
    host.innerHTML = state.rules
        .map((r, i) => {
            const d = depthOf(r.condition);
            const acts = r.actions.map((a) => `${spec(A, a.type).g} ${spec(A, a.type).say(a)}`).join(" · ");
            const phrase = esc(say(r.condition)).replace(
                /\b(all of|any of|not)\b/g,
                '<span class="kw op">$1</span>'
            );
            return `<button class="rcard" aria-current="${i === state.sel}" data-op="sel" data-i="${i}">
        <span class="rnum tnum">${i + 1}</span>
        <span class="rsum">
          <span class="line"><span class="kw if">IF</span> ${phrase}</span>
          <span class="line"><span class="kw then">THEN</span> ${esc(acts)}</span>
          <span class="rmeta"><span>${
              d > 1 ? `${d} levels · ${nodesOf(r.condition)} nodes` : "1 condition"
          }</span></span>
        </span></button>`;
        })
        .join("");
}

/* All the prose a type carries — its own description and its fields' — in one
   place, opened on demand. */
function helpBox(s) {
    const fields = s.p.filter((f) => f.hint).map((f) => `<div><b>${esc(f.label)}</b> — ${f.hint}</div>`).join("");
    return `<div class="help"><b>${esc(s.full)}</b>${s.help ? " — " + s.help : ""}
    ${fields ? `<div class="fh">${fields}</div>` : ""}</div>`;
}

function params(s, val, path) {
    return s.p
        .filter((f) => f.kind !== "children" && f.kind !== "child")
        .map((f) => {
            if (f.kind === "int")
                return `<label class="fld"><span>${esc(f.label)}</span>
        <input type="number" id="f-${path}-${f.k}" value="${esc(val[f.k])}"
          min="${f.min ?? 0}" ${f.max != null ? `max="${f.max}"` : ""}
          data-op="num" data-p="${path}" data-k="${f.k}"></label>`;

            if (f.kind === "bool")
                return `<label class="chk"><input type="checkbox" id="f-${path}-${f.k}" ${val[f.k] ? "checked" : ""}
          data-op="bool" data-p="${path}" data-k="${f.k}"> ${esc(f.label)}</label>`;

            if (f.kind !== "strlist") return "";

            /* A list of 3 domains and a list of 347 words are the same schema
               kind but need different controls, so size picks the control. */
            const items = val[f.k] || [], key = `${path}.${f.k}`, n = items.length, big = n > 12;

            if (state.bulk.has(key))
                return `<div class="fld" style="flex:1 1 100%">
          <span>${esc(f.label)} · ${n}<button class="lnk" data-op="bulk" data-key="${key}">chips</button></span>
          <textarea class="bulk" id="f-${key}-t" rows="9" spellcheck="false"
            data-op="bulktext" data-p="${path}" data-k="${f.k}">${esc(items.join("\n"))}</textarea>
          <span class="fhint">One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale.${
              f.maxItems ? ` Up to ${f.maxItems} entries${f.maxLength ? `, ${f.maxLength} characters each` : ""}.` : ""
          }</span></div>`;

            const q = (state.q[key] || "").toLowerCase();
            const shown = items.map((v, j) => [v, j]).filter(([v]) => !q || String(v).toLowerCase().includes(q));
            return `<div class="fld" style="flex:1 1 100%">
        <span>${esc(f.label)} · ${n}${q ? ` · ${shown.length} shown` : ""}<button class="lnk" data-op="bulk" data-key="${key}">bulk edit</button></span>
        ${
            big
                ? `<input class="qbox" id="f-${key}-q" value="${esc(state.q[key] || "")}" placeholder="filter ${n} entries"
              data-op="q" data-key="${key}" aria-label="Filter ${esc(f.label)}">`
                : ""
        }
        <div class="chips ${big ? "cap" : ""}">
        ${
            shown
                .map(
                    ([v, j]) => `<span class="chip">${esc(v)}<button data-op="unchip" data-p="${path}" data-k="${f.k}"
              data-j="${j}" aria-label="Remove ${esc(v)}">×</button></span>`
                )
                .join("") || `<span class="nores">nothing matches &ldquo;${esc(q)}&rdquo;</span>`
        }
        <input class="chipin" id="f-${path}-${f.k}" placeholder="add + Enter" data-op="chip" data-p="${path}" data-k="${f.k}"></div></div>`;
        })
        .join("");
}

function typeOptions(reg, current) {
    const groups = {};
    for (const [k, v] of Object.entries(reg)) (groups[v.grp] = groups[v.grp] || []).push([k, v]);
    let html = Object.entries(groups)
        .map(
            ([grp, items]) =>
                `<optgroup label="${esc(grp)}">` +
                items.map(([k, v]) => `<option value="${k}" ${k === current ? "selected" : ""}>${esc(v.t)}</option>`).join("") +
                "</optgroup>"
        )
        .join("");
    if (!reg[current]) html = `<option value="${esc(current)}" selected>${esc(current)} (unknown)</option>` + html;
    return html;
}

function node(n, path, d) {
    const s = spec(C, n.type);
    const one = childKeyOf(n), many = childrenKeyOf(n), composite = !!(one || many);
    const collapsed = state.collapsed.has(path);
    const sum = s.sum(n);
    const hasParams = s.p.some((f) => f.kind !== "children" && f.kind !== "child");

    let kids = "";
    if (!collapsed && many)
        kids = `<div class="kids">${(n[many] || []).map((x, j) => node(x, `${path}.${many}.${j}`, d + 1)).join("")}</div>
      <div class="kidfoot"><button class="btn sm ghost" data-op="addchild" data-p="${path}">+ Add condition</button></div>`;
    if (!collapsed && one && n[one]) kids = `<div class="kids">${node(n[one], `${path}.${one}`, d + 1)}</div>`;

    return `<div class="node" data-d="${d}">
    <div class="nrow">
      <button class="twist" data-op="fold" data-p="${path}" aria-expanded="${!collapsed}"
        aria-label="${collapsed ? "Expand" : "Collapse"}" ${composite || hasParams ? "" : "hidden"}>${collapsed ? "▸" : "▾"}</button>
      <span class="glyph" aria-hidden="true">${s.g}</span>
      <select class="ntype" data-op="type" data-p="${path}" aria-label="Condition type">${typeOptions(C, n.type)}</select>
      ${sum ? `<span class="nsum">${esc(sum)}</span>` : ""}
      <span class="sp"></span>
      ${composite && d >= 2 ? `<button class="ico" data-op="focus" data-p="${path}" title="Focus this subtree">⤢</button>` : ""}
      <button class="ico" data-op="help" data-p="${path}" aria-pressed="${state.helped.has(path)}" title="What this detects">ⓘ</button>
      <div class="menu"><button class="ico" data-op="menu" data-p="${path}" title="More">⋮</button>
        ${
            state.menu === path
                ? `<div class="pop">
          <button data-op="wrap" data-p="${path}" data-t="Not">Wrap in NOT</button>
          <button data-op="wrap" data-p="${path}" data-t="All">Wrap in AND</button>
          <button data-op="move" data-p="${path}" data-d="-1">Move up</button>
          <button data-op="move" data-p="${path}" data-d="1">Move down</button><hr>
          <button class="del" data-op="del" data-p="${path}">Delete</button></div>`
                : ""
        }</div>
    </div>
    ${state.helped.has(path) ? helpBox(s) : ""}
    ${!collapsed && hasParams ? `<div class="params">${params(s, n, path)}</div>` : ""}
    ${kids}</div>`;
}

function actionRows(r, ri) {
    const order = [...r.actions].sort((a, b) => spec(A, a.type).rank - spec(A, b.type).rank).map((a) => a.type);
    return r.actions
        .map((a, j) => {
            const s = spec(A, a.type), path = `rules.${ri}.actions.${j}`, sum = s.sum(a);
            const covered = s.coveredBy.some((t) => r.actions.some((x) => x.type === t));
            return `<div class="arow">
        <span class="ord tnum" title="Execution order chosen by the bot">${order.indexOf(a.type) + 1}</span>
        <span class="glyph" aria-hidden="true">${s.g}</span>
        <select class="ntype" data-op="atype" data-p="${path}" aria-label="Action">${typeOptions(A, a.type)}</select>
        ${sum ? `<span class="nsum">${esc(sum)}</span>` : ""}
        ${covered ? `<span class="cov" title="A stronger action on this rule already covers it">covered</span>` : ""}
        <span class="sp"></span>
        <button class="ico" data-op="help" data-p="${path}" aria-pressed="${state.helped.has(path)}" title="What this does">ⓘ</button>
        <button class="ico" data-op="adel" data-p="${path}" title="Remove action">✕</button>
        ${state.helped.has(path) ? `<div style="flex:1 1 100%">${helpBox(s)}</div>` : ""}
        ${s.p.length ? `<div class="params" style="flex:1 1 100%">${params(s, a, path)}</div>` : ""}
      </div>`;
        })
        .join("");
}

function renderDetail() {
    const host = document.getElementById("detail");
    if (!state.rules.length) {
        host.innerHTML = `<div class="pane-head"><span class="crumbs"><button data-op="tolist" class="back">&lsaquo; Rules</button><b>No rule selected</b></span></div>
      <div class="block"><button class="btn primary" data-op="addrule">+ Add the first rule</button></div>`;
        return;
    }
    const ri = Math.min(state.sel, state.rules.length - 1), r = state.rules[ri];
    const root = `rules.${ri}.condition`;
    const focused = state.focus && state.focus.startsWith(root + ".") ? state.focus : null;
    const shown = focused ? at(focused) : r.condition;

    const nodes = nodesOf(r.condition), depth = depthOf(r.condition);
    const over = nodes > state.limits.nodes || depth > state.limits.depth;
    const condDef = SCHEMA.definitions.condition, actDef = SCHEMA.items.properties.actions;

    const crumbs = focused
        ? `<span class="crumbs"><button data-op="tolist" class="back">&lsaquo; Rules</button>
       <button data-op="unfocus">Rule ${ri + 1}</button><span>&rsaquo;</span><span>subtree</span></span>`
        : `<span class="crumbs"><button data-op="tolist" class="back">&lsaquo; Rules</button><b>Rule ${ri + 1}</b></span>`;

    host.innerHTML = `
    <div class="pane-head">${crumbs}
      <span style="display:flex;gap:6px">
        <button class="btn sm ghost" data-op="rulemove" data-d="-1" title="Move rule up">↑</button>
        <button class="btn sm ghost" data-op="rulemove" data-d="1" title="Move rule down">↓</button>
        <button class="btn sm ghost danger" data-op="ruledel">Delete rule</button>
      </span></div>

    <div class="block">
      <div class="bhead if"><span class="tag">If</span>
        <span class="bname">${esc(condDef.title)}</span>
        <span class="meter tnum ${over ? "over" : ""}">${nodes}/${state.limits.nodes} nodes · ${depth}/${state.limits.depth} levels</span>
        <span class="tools">
          <button class="ico" data-op="help" data-p="block-if" aria-pressed="${state.helped.has("block-if")}" title="About conditions">ⓘ</button>
          <button class="btn sm ghost" data-op="foldall">Collapse all</button></span></div>
      ${state.helped.has("block-if") ? `<div class="help">${md(condDef.description)}</div>` : ""}
      ${over ? `<div class="help" style="color:var(--danger)">This tree is past what the bot accepts (max ${state.limits.nodes} conditions, ${state.limits.depth} levels). Simplify it before applying, or the bot will reject the rules.</div>` : ""}
      ${node(shown, focused || root, 0)}
    </div>

    <div class="block">
      <div class="bhead then"><span class="tag">Then</span>
        <span class="bname">${esc(actDef.title)}</span>
        <span class="tools">
          <button class="ico" data-op="help" data-p="block-then" aria-pressed="${state.helped.has("block-then")}" title="About actions">ⓘ</button>
          <button class="btn sm" data-op="addaction">+ Add action</button></span></div>
      ${state.helped.has("block-then") ? `<div class="help">${md(actDef.description)}</div>` : ""}
      ${actionRows(r, ri)}
      <div class="ordnote">Numbers are the order the bot runs them in; it skips an action a stronger one already covers.</div>
    </div>`;
}

/* The payload sits under both panes, because it belongs to the whole list:
   this is the JSON the link carries, not the rule that happens to be open. */
function renderPayload() {
    const host = document.getElementById("payload");
    const json = JSON.stringify(state.rules, null, 2);
    const kb = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} KB`);
    document.getElementById("payload-meta").textContent =
        `· ${state.rules.length} rule${state.rules.length === 1 ? "" : "s"} · ${kb(
            new Blob([json]).size
        )} · link ${kb(location.href.length)}`;
    /* Only fill it when it is open — the JSON of a long word list is big. */
    document.getElementById("payload-json").textContent = host.open ? json : "";
}

document.getElementById("payload").addEventListener("toggle", renderPayload);

/* The button lives inside <summary>, so swallow the click or it would toggle
   the block as well. Copying works whether the JSON is shown or not. */
document.getElementById("payload-copy").addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    navigator.clipboard
        .writeText(JSON.stringify(state.rules, null, 2))
        .then(() => toast("JSON copied to clipboard."))
        .catch(() => toast("Could not copy — open the block and select the text."));
});

function render() {
    document.getElementById("app").dataset.view = state.view;
    renderList();
    renderDetail();
    syncHash();
    renderPayload();
}

/* -------------------------------- events ------------------------------- */

document.addEventListener("click", (e) => {
    const b = e.target.closest("[data-op]");
    const wasOpen = state.menu;
    if (!b || b.dataset.op !== "menu") state.menu = null;
    if (!b) {
        if (wasOpen) render();
        return;
    }
    const op = b.dataset.op, p = b.dataset.p;

    switch (op) {
        case "sel":
            state.sel = +b.dataset.i;
            state.view = "rule";
            state.focus = null;
            break;
        case "tolist":
            state.view = "list";
            break;
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
        case "bulk": {
            const k = b.dataset.key;
            state.bulk.has(k) ? state.bulk.delete(k) : state.bulk.add(k);
            delete state.q[k];
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
        case "addchild": {
            const n = at(p), many = childrenKeyOf(n);
            n[many].push(newCondition());
            state.collapsed.delete(p);
            break;
        }
        case "move": {
            const { o, k } = parentOf(p);
            if (!Array.isArray(o)) {
                toast("This condition has no siblings to swap with.");
                break;
            }
            const i = +k, j = i + +b.dataset.d;
            if (j < 0 || j >= o.length) break;
            [o[i], o[j]] = [o[j], o[i]];
            state.focus = null;
            break;
        }
        case "del": {
            const { o, k } = parentOf(p);
            if (!Array.isArray(o)) {
                toast("Change this condition's type instead — a rule always needs one.");
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
        case "addaction": {
            const r = state.rules[state.sel];
            const free = Object.keys(A).find((k) => !r.actions.some((a) => a.type === k));
            if (!free) {
                toast("Every action is already on this rule.");
                break;
            }
            r.actions.push({ type: free, ...structuredClone(A[free].def) });
            break;
        }
        case "adel": {
            const { o, k } = parentOf(p);
            if (o.length === 1) {
                toast("A rule with no actions would detect something and then do nothing.");
                break;
            }
            o.splice(+k, 1);
            break;
        }
        case "addrule":
            state.rules.push(newRule());
            state.sel = state.rules.length - 1;
            state.view = "rule";
            break;
        case "ruledel":
            if (!confirm(`Delete rule ${state.sel + 1}?`)) return;
            state.rules.splice(state.sel, 1);
            state.sel = Math.max(0, state.sel - 1);
            state.focus = null;
            break;
        case "rulemove": {
            const i = state.sel, j = i + +b.dataset.d;
            if (j < 0 || j >= state.rules.length) break;
            [state.rules[i], state.rules[j]] = [state.rules[j], state.rules[i]];
            state.sel = j;
            break;
        }
        case "unchip":
            at(b.dataset.p)[b.dataset.k].splice(+b.dataset.j, 1);
            break;
        default:
            return;
    }
    render();
});

document.addEventListener("change", (e) => {
    const b = e.target.closest("[data-op]");
    if (!b) return;
    const op = b.dataset.op, p = b.dataset.p, key = b.dataset.k;

    if (op === "num") {
        const f = [...spec(C, at(p).type).p, ...spec(A, at(p).type).p].find((x) => x.k === key) || {};
        let v = Math.round(+b.value || 0);
        if (f.min != null) v = Math.max(f.min, v);
        if (f.max != null) v = Math.min(f.max, v);
        at(p)[key] = v;
        render();
        return;
    }
    if (op === "bool") {
        at(p)[key] = b.checked;
        render();
        return;
    }
    if (op === "bulktext") {
        const seen = new Set(), out = [];
        b.value.split("\n").map((x) => x.trim()).filter(Boolean).forEach((x) => {
            if (!seen.has(x)) { seen.add(x); out.push(x); }
        });
        at(p)[key] = out;
        render();
        return;
    }
    if (op === "type") {
        const { o, k } = parentOf(p);
        o[k] = { type: b.value, ...structuredClone(spec(C, b.value).def) };
        state.collapsed.delete(p);
        state.focus = null;
        render();
        return;
    }
    if (op === "atype") {
        const { o, k } = parentOf(p);
        if (o.some((a, i) => i !== +k && a.type === b.value)) {
            toast("That action is already on this rule.");
            render();
            return;
        }
        o[k] = { type: b.value, ...structuredClone(spec(A, b.value).def) };
        render();
    }
});

/* Filtering a long list re-renders it, so put the caret back where it was. */
document.addEventListener("input", (e) => {
    const b = e.target.closest('[data-op="q"]');
    if (!b) return;
    state.q[b.dataset.key] = b.value;
    const id = b.id;
    render();
    const again = document.getElementById(id);
    if (again) {
        again.focus();
        again.setSelectionRange(again.value.length, again.value.length);
    }
});

document.addEventListener("keydown", (e) => {
    const b = e.target.closest('[data-op="chip"]');
    if (!b || e.key !== "Enter") return;
    e.preventDefault();
    const v = b.value.trim();
    if (!v) return;
    at(b.dataset.p)[b.dataset.k].push(v);
    const id = b.id;
    render();
    const again = document.getElementById(id);
    if (again) again.focus();
});

/* ------------------------------- startup ------------------------------- */

function renderHeader() {
    const instructions = document.getElementById("instructions");
    instructions.innerHTML = md(SCHEMA.description);
    if (state.botId) {
        document.getElementById("apply").hidden = false;
    } else {
        document.getElementById("nobot").hidden = false;
    }
}

document.getElementById("apply").addEventListener("click", () => {
    /* The bot finds this URL anywhere in the message and stops at ")" or a
       space, which is why a markdown link is safe to paste. */
    const link = `[Rules for group ${state.botId}](${location.href})`;
    navigator.clipboard
        .writeText(link)
        .then(() => alert("The link is copied to clipboard. Send it to the bot to apply the changes."))
        .catch(() => alert("Please copy the URL from the address bar and send it to the bot."));
});

async function init() {
    if (typeof LZString === "undefined") throw new Error("lz-string failed to load");
    SCHEMA = await fetch("rules-schema.json").then((r) => {
        if (!r.ok) throw new Error(`rules-schema.json: HTTP ${r.status}`);
        return r.json();
    });

    C = buildRegistry(SCHEMA.definitions.condition.oneOf);
    A = buildRegistry(SCHEMA.definitions.action.oneOf);
    const lim = SCHEMA.definitions.condition.options || {};
    state.limits = { depth: lim.max_depth || 8, nodes: lim.max_nodes || 64 };

    const { bot_id, rules } = parseHash();
    state.botId = bot_id || null;
    if (rules) {
        try {
            const parsed = JSON.parse(LZString.decompressFromEncodedURIComponent(rules));
            if (Array.isArray(parsed)) state.rules = parsed;
        } catch (err) {
            console.warn("Could not restore rules from the URL hash:", err);
            toast("The rules in this link could not be read. Starting from an empty list.");
        }
    }

    renderHeader();
    document.getElementById("boot").hidden = true;
    document.getElementById("app").hidden = false;
    document.getElementById("payload").hidden = false;
    render();
}

init().catch((err) => {
    const boot = document.getElementById("boot");
    boot.className = "boot failed";
    boot.textContent = `Failed to load the editor: ${err.message}`;
});
