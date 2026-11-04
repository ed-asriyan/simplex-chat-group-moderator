---
name: ux
description: Stage 2 of a feature — describe the screens the reviewed stories are played on — their elements by role and accessible name, their states, the way between them — in the UX document and nothing else; nothing to do for a project without a user interface. Then stop for review.
argument-hint: what to focus on
disable-model-invocation: true
---

# Stage 2: user experience

You are doing **only** the user experience stage of the process in `AGENTS.md`. $ARGUMENTS

The UX document says how a user interface is laid out and behaves: which screens and dialogs there are, what is on each and in which order, how each element is reached (its role and its accessible name), when it is shown, hidden or unavailable, which states each screen can be in, and the way from one screen to another. The stories say what a person does and what they read; the UX document says where they do it. The end-to-end tests reach an element through it, and the implementation builds it as it says.

The project's own values — where the UX document is, where the user interface's code is and how to open it — are in `AGENTS.md` *Workflow*, under *UX*. Read it first. If it says the project has no user interface, say that this stage has nothing to do, and stop: the next stage is `/e2e-tests`. If *Workflow* does not say something this skill needs, ask the maintainer.

## Input
The stories changed by stage 1: the uncommitted diff of the stories document (if empty, the last commits that touched it, or ask the maintainer which stories this is about). Read those stories in full, the UX document in full, and the user interface as it is for the screens they touch: its code, and the page itself when it can be opened.

If a story cannot be placed on a screen without deciding what the story leaves open — which element starts it, what a person sees when it fails — **don't decide it here**: stop, say what is missing, and propose the change to the story.

## Scope
- Edit only the UX document.
- Never change the stories, the user interface's code, or any test.
- A screen that does not exist yet may be drawn as an HTML mockup for the maintainer's review: shown to them, never committed. The document stays what later stages follow.

## The document
One `### <screen or dialog>` section each, with:
- **Purpose:** what a person comes to it for, and the stories it serves, by id.
- **Layout:** its regions in order, top to bottom, as on a computer, and how it differs on a phone.
- **Elements:** a table, one row per element a person can see or use: *Role* (`button`, `dialog`, `checkbox`, `textbox`, `link`, `heading`, `list`, …) | *Accessible name* | *Shown* (always, or when) | *Does*. The name is the label the stories give it, word for word.
- **States:** each state the screen can be in — the usual one, empty, loading, error, success, partial, a part unavailable — what it shows, and how a person gets out of it.
- **Keyboard and focus:** its shortcuts, where focus goes when it opens and when it closes, and the order of the controls.

After the screens, `## Navigation`: from which screen and element a person reaches which other screen.

Rules:
- **Texts belong to the stories.** What a person reads as the result of an action — a message, a confirmation, an error, a label a story names — is written in the stories only; the document names the story instead of quoting it. Only text no story covers, because it explains rather than answers (a placeholder, a tooltip, a heading), is written here.
- Every element a story names has a row, with its role and the accessible name the story gives it. An element a person must use that has no accessible name is a defect: say so.
- Describe what a person perceives and does, never the markup: no classes, ids, pixels or colours, unless a colour carries meaning (*marked removed, in red*).
- Write only what is true now; when a screen or an element goes, delete every mention of it.

## Review
Hand the diff of the UX document and the stories it serves to a **subagent** (general-purpose), so it is judged by eyes that did not write it, with this task:

> Evaluate these screens against Jakob Nielsen's ten usability heuristics: visibility of system status; match between the system and the real world; user control and freedom; consistency and standards; error prevention; recognition rather than recall; flexibility and efficiency of use; aesthetic and minimalist design; help users recognise, diagnose and recover from errors; help and documentation. Also check that every element a person must use can be reached by role and accessible name and by keyboard. For each problem, report the heuristic, the screen and element, its severity from 0 (not a problem) to 4 (a usability catastrophe), and a proposed fix. Don't edit any file.

The review is advice for the maintainer, not a gate: show it, and apply none of it unasked.

## Exit check
1. Every story changed in stage 1 that is played through the user interface is cited by a screen's *Purpose*.
2. Every element those stories name has a row, with a role and the accessible name the story gives it.
3. Every screen the change touches lists its states.
4. Where the screen exists already, the document matches it as it is, for everything the stories do not change.

## Finish
Show the diff of the UX document, the screens and elements added or changed per story, the review's findings, and **stop**. The next stage is `/e2e-tests`, run by the maintainer after review.
