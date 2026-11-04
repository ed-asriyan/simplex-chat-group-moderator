# User experience

The screens of the web editor: what is on each, how each element is reached — its role and its accessible name, as a screen reader and the end-to-end tests find it — when it is shown, and what it does; the states each screen can be in, and the way between them. What a person reads as the result of an action is in the [stories](user-stories.md), which this document names instead of quoting; the bot's chat has no screen.

This is a living document: it describes the editor as it is. Element names are exact; an element whose accessible name is only a symbol (`ⓘ`, `⋮`, `✕`, `↑`, `↓`, `⤢`) explains itself in a tooltip, given under *Does*.

## Screens

### The editor
**Purpose:** everything about one group's configuration on one page: its mode, its rules, and the way to send them back to the bot. US-B4, US-G1–US-G8, and the fields of every condition and action (US-E1–US-E30, US-F1–US-F3).

**Layout:** top to bottom:
1. The header: the eyebrow *Group settings*, the page's heading, the three steps of US-G1, the error of a link without a bot id when there is one, and the header's buttons, *Undo*, *Redo*, *Import JSON*, *Export JSON* and *Apply changes*.
2. *Edit with AI*, alone on its line.
3. Two panes, side by side on a computer: on the left the *Rules* pane — the *Group settings* panel, then the list of rules and *+ Add rule* — and on the right the *Rule* pane, the rule that is open (see *A rule*).

On a phone (narrower than 1000 px) the panes are one screen each: the rules list first, and a tap on a rule replaces it with that rule, which has *‹ Rules* to go back; the browser's Back does the same.

**Elements:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `heading` (level 1) | Moderation settings for Moderator Bot | always | its last words are a `link` *Moderator Bot*, to the bot's page in the SimpleX directory |
| `paragraph` | the three steps of US-G1 | always | |
| `paragraph` | the error of a link without a bot id (US-G1) | the link has no `bot_id` | |
| `button` | Undo | there is a change to undo | undoes the last change (US-G6); tooltip "Undo the last change (Ctrl+Z)" |
| `button` | Redo | there is a change to redo | redoes it; tooltip "Redo (Ctrl+Shift+Z)" |
| `button` | Import JSON | always | opens *Import settings from JSON* |
| `button` | Export JSON | always | opens *Export settings as JSON* |
| `button` | Apply changes | the link has a `bot_id` | opens *Changes before you send them* |
| `button` | Edit with AI | always | opens *Edit your settings with AI* |
| `region` | Rules | always | holds the *Group settings* panel and the list of rules |
| `heading` (level 2) | Group settings | always | |
| `button` | ⓘ | always, in the *Group settings* heading | shows or hides what the settings do, under the heading; pressed while shown; tooltip "What these do" |
| `combobox` | 🛡 Mode | always | the group's mode: *Notifications — act and tell me*, *Silent — act without telling me*, *Dry — only tell me what it would do* (US-G2) |
| `heading` (level 2) | Rules | always | followed by the number of rules, `3 total` |
| `button` | `1 IF contains one of 2 words THEN 🗑 moderate` | one per rule | the rule's number, its condition and its actions in plain words; opens the rule |
| `button` | + Add rule | always | adds a rule — *💬 Message Contains Any of These Words* with no words, and *🗑 Moderate the message* — and opens it (US-G3) |
| `region` | Rule | always | the open rule (see *A rule*) |
| `link` | View source on GitHub | always, in the top right corner | the project on GitHub |

**States:**
- *Usual:* the group's mode and rules, the first rule open beside the list.
- *No rules:* the list reads as in US-G1; the *Rule* pane reads *No rule selected* with `+ Add the first rule`, which does what *+ Add rule* does.
- *A link without a bot id* (or no hash at all): the error of US-G1 under the steps, and no *Apply changes*; everything else works, Import and Export included.
- *Settings that cannot be read:* the notice of US-G1, and the editor as with no rules, mode *Notifications*.
- *Loading:* `Loading editor…` until the page has read the link; if it cannot start, `Failed to load the editor:` and the reason.

**Keyboard and focus:** Ctrl/Cmd+Z undoes, Ctrl/Cmd+Shift+Z and Ctrl/Cmd+Y redo, except in a text field, where the field's own undo works (US-G6). Every control is a native button, field or select, reached by Tab in the order of the layout.

### A rule
**Purpose:** one rule's condition tree and actions, with every field of each (US-E1–US-E30, US-F1–US-F3), and its warnings (US-G4, US-G5).

**Layout:** the rule's heading, *Rule 2*, with its tools; the *If 🔎 Condition* block, with the condition tree, one row per condition, a composite's conditions indented under it; the *Then ⚡ Actions* block, one row per action, then the line "Numbers are the order the bot runs them in; it skips an action a stronger one already covers."

**Elements of the rule:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `button` | ‹ Rules | on a phone | back to the rules list |
| `button` | ↑ | always | moves the rule up the list; tooltip "Move rule up" |
| `button` | ↓ | always | moves it down; tooltip "Move rule down" |
| `button` | Delete rule | always | deletes the rule, with no question asked: *Undo* brings it back (US-G6) |
| `button` | ⓘ | in *If 🔎 Condition* and in *Then ⚡ Actions* | shows or hides what conditions, or actions, are; pressed while shown; tooltips "About conditions", "About actions" |
| `button` | Collapse all / Expand all | in *If 🔎 Condition* | folds or unfolds every row of the tree |
| `paragraph` | the warning of US-G5 | the tree passes 64 conditions or 8 levels | |
| `button` | + Add action | always | adds the first action the rule does not have yet; with every action there, says so |
| `button` | Rule 2 | a subtree is focused | back to the whole rule; the heading reads *Rule 2 › subtree* |

**Elements of a condition row:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `button` | Collapse / Expand | the condition has fields or conditions inside | folds or unfolds the row; expanded or not |
| `combobox` | Condition type | always | the condition's type, grouped by subject (US-G3); changing it keeps the fields both types have (US-G4). A type the editor does not know shows as `FutureType (unknown)`, with no fields (US-G1) |
| (text) | the condition's settings in short, `spam, blow job`, `< 60 min ago` | the condition has settings | |
| `button` | ⤢ | a composite two or more levels deep | shows only this subtree; tooltip "Focus this subtree" |
| `button` | ⓘ | always | shows or hides the condition's full description and its fields', under the row; pressed while shown; tooltip "What this detects" |
| `button` | ⋮ | always | opens the row's menu; tooltip "More" |
| `button` | Unwrap | in the menu, for a composite | moves its conditions up into its parent (US-G4) |
| `button` | Wrap in NOT / Wrap in AND | in the menu | puts the condition under a new *Not* or *All* |
| `button` | Move up / Move down | in the menu | swaps it with its neighbour inside its composite |
| `button` | Delete | in the menu | deletes the condition |
| `button` | + Add to AND / + Add to OR | under a composite's conditions | adds a condition to it |

**Elements of an action row:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| (text) | its number, `1` | always | the order the bot runs the actions in; tooltip "Execution order chosen by the bot" |
| `combobox` | Action | always | the action's type |
| (text) | `covered` | another action of the rule covers it (US-G5) | tooltip as in US-G5 |
| `button` | ⓘ | always | shows or hides what the action does; tooltip "What this does" |
| `button` | ✕ | always | removes the action; tooltip "Remove action" |

**Fields**, under a condition or an action, each labelled with its field's title (the titles are in US-E1–US-E30 and US-F1–US-F3):

| Role | Accessible name | For | Does |
|---|---|---|---|
| `spinbutton` | the field's title, *Time Window (Minutes)* | a number | takes a whole number within the field's limits |
| `checkbox` | the field's title, *Case sensitive* | yes or no | |
| `combobox` | the field's title, *Harassment* (and the number, once one is set) | a choice | its choices, *Off*, *OpenAI decides*, and *Min score, %*, which adds a `spinbutton` for the number |
| `textbox` | the field's title, *Instruction* | a text | several lines for *Instruction*, with its length out of its limit, `Instruction · 120 / 4000` |
| `textbox` | the field's title followed by `show` or `hide`, *OpenAI API Key show* | a key | masked; the `button` *show* shows it, *hide* masks it again |
| `textbox` | add + Enter | a list | Enter adds what is typed as an entry; an entry already there is not added twice |
| `button` | Remove spam | one per entry of a list | removes that entry |
| `textbox` | Filter Words / Phrases | a list of more than 20 entries | shows only the entries containing what is typed; placeholder `filter 150 entries` |
| `button` | bulk edit / chips | a list | switches the list to one entry per line in a text box, applied when the box loses focus, and back (US-G3) |

The list's title is followed by its number of entries, `Words / Phrases · 2`, and by `· 3 shown` while it is filtered. A list with no entries reads `nothing matches “”` under its field.

**States:** *usual*; *a part folded* (its rows hidden, the button reads *Expand*); *a subtree focused*; *over the limits* (the warning of US-G5 above the tree); *a list in bulk edit*; *a description shown* (the ⓘ pressed).

**Keyboard and focus:** Enter in a list's field adds the entry. The menu closes on the next click anywhere.

### Changes before you send them
**Purpose:** see every change before sending it, and copy the link (US-G7, US-G8).

**Layout:** a modal dialog: its title and the number of rules before and after; its body — the warning about the link's size, the changed settings, one line per changed rule, the unchanged rules; its buttons.

**Elements:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `dialog` | Changes before you send them | after *Apply changes* | |
| `button` | Cancel | before copying | closes it, nothing copied |
| `button` | Copy the link | before copying | copies the link and shows the result of US-G8 |
| `button` | Close | after copying | closes it |

**States:** *changes* (the lines of US-G7: a setting's line, then each rule as `changed`, `added`, `removed` or `moved` with *was rule 2*, a list that lost entries in red, then the unchanged rules); *no change* (the text of US-G7); *the link near or past the size limit* (the warnings of US-G7, above the changes); *copied* (US-G8); *the clipboard refused* (US-G8).

**Keyboard and focus:** focus goes to *Cancel* when it opens, or to the first change that unfolds (a changed list) when there is one; Escape closes it; focus goes back to *Apply changes* when it closes.

### Edit your settings with AI
**Purpose:** have an AI chat explain or change the configuration (US-G9, US-G10).

**Layout:** a modal dialog with two numbered steps, *Copy the question* and *Paste what it answers*, then its buttons.

**Elements:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `dialog` | Edit your settings with AI | after *Edit with AI* | |
| `button` | Close | always, the `×` in the title | closes it |
| `heading` (level 3) | Copy the question | always | |
| `button` | Copy the prompt | always | copies the prompt (US-G9) |
| `group` | `1 long list sent as a placeholder` | a list has more than 100 entries | folded; unfolded, one `checkbox` per long list, ticked to send it in full (US-G10) |
| `heading` (level 3) | Paste what it answers | always | |
| `textbox` | Paste the AI's reply here | always | |
| `button` | Load into the editor | always | loads the reply (US-G9) |
| `button` | Clear | something is pasted | empties the box |
| `button` | Undo the load | after a load | undoes it (US-G9) |
| `button` | Close | always, at the bottom | closes it |

The first step explains, under its heading: "Paste it into ChatGPT, Gemini, Claude or any other AI chat. It will explain your settings and rules in plain language and can change them for you." The second: "Only needed if you asked for a change. Paste the whole reply — the editor finds the settings in it."

**States:** *usual*; *loaded* (the result of US-G9 under the box, and *Undo the load*); *refused* (the problems of US-G9 or US-G10 under the box); *long lists* (the group above).

**Keyboard and focus:** focus goes to the title's *Close* when it opens; Escape closes it; focus goes back to *Edit with AI* when it closes.

### Import settings from JSON
**Purpose:** load a configuration from JSON (US-G11).

**Elements:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `dialog` | Import settings from JSON | after *Import JSON* | |
| `button` | Close | always, the `×` in the title | closes it |
| `textbox` | Paste the group's settings: its mode and its rules | always | placeholder: a one-rule configuration, `{"mode":"notifications","rules":[…]}` |
| `button` | Choose JSON file | always | picks a `.json` file, whose content goes into the box |
| `button` | Cancel | always | closes it |
| `button` | Load settings | always | loads it (US-G11) |

**States:** *usual*; *refused* (the reason of US-G11 under the box); *loaded* closes the dialog, with the notice of US-G11.

**Keyboard and focus:** Escape closes it; focus goes back to *Import JSON*.

### Export settings as JSON
**Purpose:** take the configuration out as JSON (US-G11).

**Elements:**

| Role | Accessible name | Shown | Does |
|---|---|---|---|
| `dialog` | Export settings as JSON | after *Export JSON* | |
| `button` | Close | always, the `×` in the title | closes it |
| `checkbox` | Clear secret values (API keys) | always, ticked | empties the API keys in what is exported |
| `button` | Cancel | always | closes it |
| `button` | Copy JSON | always | copies the configuration and closes, with the notice of US-G11 |
| `button` | Download JSON | always | downloads `moderation-settings.json` and closes, with the notice of US-G11 |

**Keyboard and focus:** Escape closes it; focus goes back to *Export JSON*.

### Notices
**Purpose:** a short message about what just happened: the notices of US-G1, US-G9 and US-G11, and `That entry is already in the list.` when a list's entry is added twice.

**Layout:** at the bottom of the page, over everything, for about three seconds; then it goes by itself. It has no role, and is not announced.

## Navigation
- The bot's link (US-B4) opens **the editor**, with the group's configuration.
- **The editor:** a rule in the list opens it in **a rule** (on a phone, as a screen of its own, left with *‹ Rules* or Back); *Apply changes* opens **Changes before you send them**; *Edit with AI* opens **Edit your settings with AI**; *Import JSON* and *Export JSON* open **Import settings from JSON** and **Export settings as JSON**.
- **A rule:** *⤢* focuses a subtree, and *Rule 2* in the heading goes back to the whole rule.
- Each dialog goes back to **the editor** when it closes; after *Copy the link*, the person goes to the chat with the bot and sends what was copied (US-C1).
