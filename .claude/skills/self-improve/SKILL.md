---
name: self-improve
description: Capture a reusable, repo-specific learning as a new or updated skill so future agents don't re-derive it. Use right after you solve a non-obvious problem, discover a gotcha, or work out a multi-step procedure that will recur in this codebase.
---

# Self-improve: turn a hard-won learning into a skill

The point of this loop is that the *next* agent (or you, next session) starts where the
last one finished. When you've just spent effort figuring something out that isn't
obvious from the code or `CLAUDE.md`, record it as a skill.

## When to capture (and when not to)

Capture when **all** of these hold:
- It's **reusable** — it'll come up again, not a one-off for this exact change.
- It's **non-obvious** — not derivable by reading the code, `CLAUDE.md`, or git history.
- It's **procedural or a gotcha** — a sequence of steps, a sharp edge, a "do X not Y".

Do **not** capture: one-off facts, anything already in `CLAUDE.md`/`CONSTITUTION.md`/the
code, secrets, or transient state. If it belongs in `CLAUDE.md` (always-loaded project
rules), put it there instead — skills are for on-demand, task-specific know-how.

## How to add a skill

1. Pick a focused, kebab-case slug (one capability per skill).
2. Create `.claude/skills/<slug>/SKILL.md` with frontmatter:
   ```
   ---
   name: <slug>
   description: <one line — written so a future agent matches it by "when to use">
   ---
   ```
3. Body: lead with the trigger ("Use when…"), then concrete steps and **exact commands**
   (copy-pasteable). Keep it short; link related skills inline by name.
4. If a skill already covers the area, **update it** rather than adding a near-duplicate.

## After writing

- Skills are committed (they ship with the repo, so every clone/agent gets them).
- The `SessionStart` hook (`.claude/hooks/list-skills.sh`) lists every skill's
  `name`/`description` at the start of each session, so a new skill is immediately visible.
- Land it through the normal trunk-based PR flow like any other change.

## Quality bar

A good skill makes the next agent measurably faster or stops them repeating a mistake.
If you can't state the recurring task it accelerates, it's probably a note, not a skill.
