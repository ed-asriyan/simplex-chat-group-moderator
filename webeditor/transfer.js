/* Export and import of the rules as plain JSON, for owners who keep rules in a
   file or write them by hand. Import goes through the same strict validation as
   a pasted AI reply and lands on the editor's undo stack; the write path to the
   bot is still `Apply changes`. */

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
        return JSON.stringify(walk(state.rules), null, 2);
    };

    document.getElementById("exp-btn").addEventListener("click", () => exportDlg.showModal());
    for (const id of ["expdlg-x", "expdlg-close"])
        document.getElementById(id).addEventListener("click", () => exportDlg.close());

    document.getElementById("exp-download").addEventListener("click", () => {
        const blob = new Blob([exportJson()], { type: "application/json" });
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = "moderation-rules.json";
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 1000);
        exportDlg.close();
        toast("Rules exported.");
    });

    document.getElementById("exp-copy").addEventListener("click", () => {
        navigator.clipboard.writeText(exportJson()).then(() => {
            exportDlg.close();
            toast("Rules copied as JSON.");
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

        const { rules, errors } = validateRules(parsed, { allowSecretValues: true });
        if (errors)
            return show(
                "bad",
                `<b>Not loaded — the rules have problems:</b><ul>${errors
                    .slice(0, 6)
                    .map((e) => `<li>${esc(e)}</li>`)
                    .join("")}</ul>${errors.length > 6 ? `<p>…and ${errors.length - 6} more.</p>` : ""}`
            );

        const before = clone(state.rules);
        state.rules = rules;
        state.sel = Math.min(state.sel, Math.max(0, rules.length - 1));
        state.focus = null;
        state.collapsed = new Set();
        recordUndo(before);
        text.value = "";
        render();
        dlg.close();
        toast(`Loaded ${rules.length} rule${rules.length === 1 ? "" : "s"}. Press Apply changes to send.`);
    });
}

document.addEventListener("editor:ready", setupTransfer);
