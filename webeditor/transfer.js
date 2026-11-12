"use strict";
/*
 * Import and export of the group's whole configuration as plain JSON (US-G11),
 * for owners who keep it in a file or write it by hand. An import is checked
 * as an AI's answer is, and lands as one step on the undo stack; the way to the
 * bot is still Apply changes.
 */

function setupTransfer() {
    const text = $("imp-text"), file = $("imp-file");

    const say = (kind, html) => {
        $("imp-result").className = `note ${kind}`;
        $("imp-result").innerHTML = html;
        $("imp-result").hidden = false;
    };

    document.addEventListener("json:import", () => {
        $("imp-result").hidden = true;
        $("impdlg").showModal();
        text.focus();
    });
    document.addEventListener("json:export", () => $("expdlg").showModal());

    /* The configuration, with every secret emptied unless the owner keeps them. */
    const exported = () => {
        const clear = $("exp-clear-secrets").checked;
        const walk = (v) => {
            if (Array.isArray(v)) return v.map(walk);
            if (!v || typeof v !== "object") return v;
            return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, clear && isSecret(v, k) ? "" : walk(x)]));
        };
        return JSON.stringify(walk(config()), null, 2);
    };

    $("exp-download").addEventListener("click", () => {
        const a = document.createElement("a");
        a.href = URL.createObjectURL(new Blob([exported()], { type: "application/json" }));
        a.download = "moderation-settings.json";
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 1000);
        $("expdlg").close();
        toast("Settings exported.");
    });

    $("exp-copy").addEventListener("click", () =>
        navigator.clipboard.writeText(exported())
            .then(() => toast("Settings copied as JSON."))
            .catch(() => toast("Could not copy JSON to the clipboard.")));

    $("imp-file-btn").addEventListener("click", () => file.click());
    file.addEventListener("change", async () => {
        const f = file.files[0];
        file.value = "";
        if (f) text.value = await f.text();
        $("imp-result").hidden = true;
    });
    text.addEventListener("input", () => ($("imp-result").hidden = true));

    $("imp-load").addEventListener("click", () => {
        const src = text.value.trim();
        if (!src) return say("bad", "Paste the JSON or choose a file first.");
        let parsed;
        try { parsed = JSON.parse(src); } catch (e) { parsed = extractJson(src); }
        if (parsed == null) return say("bad", "That is not valid JSON.");
        if (typeof parsed !== "object" || Array.isArray(parsed)) return say("bad", "Expected an object with the group's settings and its <code>rules</code> list.");
        const { rules: given, ...settings } = parsed;
        if (!Array.isArray(given)) return say("bad", "The <code>rules</code> list is missing.");
        const read = readSettings(settings), got = readRules(given, { ownSecrets: true });
        const errors = [...(read.errors || []), ...(got.errors || [])];
        if (errors.length) return say("bad", problems("Not loaded — the settings have problems:", errors));
        edit(() => { st.rules = got.rules; st.settings = read.settings; });
        st.folded = new Set();
        text.value = "";
        $("impdlg").close();
        go("home", { replace: true });
        toast(`Loaded the settings and ${plural(got.rules.length, "rule")}. Press Apply changes to send.`);
    });
}

document.addEventListener("editor:ready", setupTransfer);
