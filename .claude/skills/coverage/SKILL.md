---
name: coverage
description: Check which user stories are not covered by tests — mechanically by story-id citations, then by an independent read of whether the citing tests really check the story. Read-only.
argument-hint: story ids or an epic letter to check; all stories by default
---

# Story coverage

Find the stories that the tests do not really check. Scope: $ARGUMENTS (all stories if empty). This skill changes no file.

The project's own values — where the stories are, the story id pattern, which files are tests, and which stories are not unit-testable by nature — are in `AGENTS.md` *Workflow*. Read it first.

## 1. Citations
Run `.claude/skills/coverage/scripts/story_coverage.sh` with the arguments *Workflow* gives. It lists:
- stories no test file cites;
- citations of story ids that do not exist (stale after a renumbering);
- for each story, the test files citing it.

## 2. Does the test check the story?
A citation is only a claim. Hand the check to a **subagent** (general-purpose), so it is done by eyes that did not write the tests. Give it the stories document's path, the list of stories in scope with the files citing each, and this task:

> For each story: read its full text in the stories document and the tests citing it (search the files for the id). Split the story into the separate behaviours it states — each condition, each role, each variant where it differs, each exact text, each time boundary. For each behaviour, name the test that asserts it, or say that none does. A test that only mentions the id, or asserts something weaker (that a call happened, not its result), does not count. Don't edit any file. Report per story: covered / partly covered (which behaviours are missing) / not covered.

For stories with no citation, the subagent also searches the tests by behaviour (command names, text fields, port methods) — the test may exist without the id.

Stories that *Workflow* says are not unit-testable by nature are reported as such, not as gaps.

## Report
A table: story, verdict, missing behaviours, suggested test location. Then the stale citations. Then stop: writing the missing tests is `/tests`.
