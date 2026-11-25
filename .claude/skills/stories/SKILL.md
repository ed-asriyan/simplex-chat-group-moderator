---
name: stories
description: Stage 1 of a feature — change the user stories and nothing else, then stop for the maintainer's review.
argument-hint: the behaviour to add or change
disable-model-invocation: true
---

# Stage 1: user stories

You are doing **only** the user-stories stage of the process in `AGENTS.md` for: $ARGUMENTS

The project's own values — where the stories are, how a story id looks, how stories are written there — are in `AGENTS.md` *Workflow*. Read it first, and `docs/design.md` *Overview* and *Components*, so the stories use the system's terms. If they do not say something this skill needs, ask the maintainer.

## Scope
- Edit the stories document.
- If you renumber stories, also update the references to the old ids wherever they are (code comments, tests, other docs). No other change.
- Touch nothing else: no design, no ports, no schema, no code, no tests.

## Steps
1. Read the stories document in full, so the new stories stay consistent with the rest.
2. If the behaviour is ambiguous (a case the request does not decide, a conflict with an existing story), ask the maintainer before writing.
3. Decide which stories the request touches. A story is one user flow: **one** thing a person does and what they get back — opening a page or a chat and what it shows, pressing one control and what follows, sending one message or entering one value and the answer to it, one thing the system does on its own and what a person then sees. Not a feature described whole: a page with ten controls is the story of opening it, then a story per control. The cases of one action that differ only by the value given and the answer to it (the refusals of a form or of a command) may be one story, as a list.
   - **The request changes what happens in a flow that already has a story** (another reply, a different text, a new case of the same action): **edit that story in place**. Do not add a story, a sub-story or a note elsewhere that patches it. Each existing story whose flow is affected gets the change; if the same outcome is described in several stories, update each one.
   - **Add a new story only for a new flow**: a person can do something they could not do before, or the system does something on its own in a situation no story covers.
4. Write every story in the project's form (*Workflow*). Whatever the form, a story is **"As a \<role\>, when I \<concrete action\>, \<what I get\>"**:
   - The action is concrete and physical, with real values, never an abstraction such as "when I change a setting".
   - What follows is what each person involved observes, with the exact wording of any text that matters, written out in the story itself; don't send the reader to another story for "the same reply".
   - A general rule (who owns what, what persists) is a sentence inside the story it belongs to, never a story of its own without an actor and an action.
   - Where an action leads is written out exactly: the address of the page it opens, the chat it opens, the link it follows, not just "the settings page".
   - A link the person sees is written as a Markdown link, its text and where it leads, `[text](https://example.com/path)`, inside the quoted text it belongs to.
5. Write the stories as the current intended behaviour: rewrite or remove stories that no longer apply, don't keep history. A removed or renamed behaviour is simply not mentioned: never write that it "no longer exists", "was replaced" or "is gone".
6. A new flow gets a new id; never renumber to make room. If you do renumber, grep the repository for every old id and list the test files a later stage will have to update.
7. If the request does not change behaviour (a refactoring, a rename in code), say so and change nothing.

## Finish
Show the diff of the stories document, list the story ids you added, changed or removed, and **stop**. The next stage is `/ux`, run by the maintainer after review.
