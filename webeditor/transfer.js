/* Export and import of the group's whole config — its settings and its rules —
   as plain JSON, for owners who keep it in a file or write it by hand. The rules
   go through the same strict validation as a pasted AI reply, the settings
   through the schema's, and the result lands on the editor's undo stack; the
   write path to the bot is still `Apply changes`. */

function setupTransfer() {
    const dlg = document.getElementById("impdlg");
    const exportDlg = document.getElementById("expdlg");
    const text = document.getElementById("imp-text");
    const result = document.getElementById("imp-result");
    const file = document.getElementById("imp-file");

    const show = (kind, html) => {
        result.className = `airesult ${kind}`;
        result.innerHTML = html;
        result.hidden = false;
    };

    const exportJson = () => {
        const clearSecrets = document.getElementById("exp-clear-secrets").checked;
        const walk = (value) => {
            if (Array.isArray(value)) return value.map(walk);
            if (!value || typeof value !== "object") return value;
            const out = {};
            for (const [key, child] of Object.entries(value))
                out[key] = clearSecrets && secretField(value, key) ? "" : walk(child);
            return out;
        };
        return JSON.stringify(walk(config()), null, 2);
    };

    document.getElementById("exp-btn").addEventListener("click", () => exportDlg.showModal());
    for (const id of ["expdlg-x", "expdlg-close"])
        document.getElementById(id).addEventListener("click", () => exportDlg.close());

    document.getElementById("exp-download").addEventListener("click", () => {
        const blob = new Blob([exportJson()], { type: "application/json" });
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = "moderation-settings.json";
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 1000);
        exportDlg.close();
        toast("Settings exported.");
    });

    document.getElementById("exp-copy").addEventListener("click", () => {
        navigator.clipboard.writeText(exportJson()).then(() => {
            exportDlg.close();
            toast("Settings copied as JSON.");
        }).catch(() => toast("Could not copy JSON to the clipboard."));
    });

    document.getElementById("imp-open").addEventListener("click", () => {
        result.hidden = true;
        dlg.showModal();
    });
    for (const id of ["impdlg-x", "impdlg-close"])
        document.getElementById(id).addEventListener("click", () => dlg.close());

    document.getElementById("imp-file-btn").addEventListener("click", () => file.click());
    file.addEventListener("change", async () => {
        const f = file.files[0];
        file.value = "";
        if (f) text.value = await f.text();
        result.hidden = true;
    });

    document.getElementById("imp-load").addEventListener("click", () => {
        const src = text.value.trim();
        if (!src) return show("bad", "Paste the JSON or choose a file first.");

        let parsed;
        try {
            parsed = JSON.parse(src);
        } catch (e) {
            parsed = extractJson(src);
        }
        if (parsed === null || parsed === undefined)
            return show("bad", "That is not valid JSON.");

        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed))
            return show("bad", "Expected an object with the group's settings and its <code>rules</code> list.");
        const { rules: given, ...givenSettings } = parsed;
        if (!Array.isArray(given)) return show("bad", "The <code>rules</code> list is missing.");
        const checked = validateSettings(givenSettings);
        const { rules, errors: ruleErrors } = validateRules(given, { allowSecretValues: true });
        const errors = [...(checked.errors || []), ...(ruleErrors || [])];
        if (errors.length)
            return show(
                "bad",
                `<b>Not loaded — the settings have problems:</b><ul>${errors
                    .slice(0, 6)
                    .map((e) => `<li>${esc(e)}</li>`)
                    .join("")}</ul>${errors.length > 6 ? `<p>…and ${errors.length - 6} more.</p>` : ""}`
            );

        const before = snapshot();
        state.rules = rules;
        state.settings = checked.settings;
        state.sel = Math.min(state.sel, Math.max(0, rules.length - 1));
        state.focus = null;
        state.collapsed = new Set();
        recordUndo(before);
        text.value = "";
        render();
        dlg.close();
        toast(`Loaded the settings and ${rules.length} rule${rules.length === 1 ? "" : "s"}. Press Apply changes to send.`);
    });
}

document.addEventListener("editor:ready", setupTransfer);
