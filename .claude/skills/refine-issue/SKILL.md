# Skill: refine-issue

Pick the next unrefined issue and grill the user with clarifying questions
until the issue is ready to hand to an agent. The final rewrite must contain zero
alternatives or open decisions — just the minimal, focused, final description.

## 1. Fetch the issue

Run `.claude/skills/refine-issue/pick-issue.sh` to get the issue. The caller cannot see
the script's output, so **immediately announce which issue was picked** — lead with
`#<N> — <title>` plus a one-clause note on what it is — before any evaluation or questions.

## 2. Critical evaluation — do this before asking the user anything

Read the issue and the relevant code. Then ask yourself:

- **Is this the right problem?** Does the issue address a real gap, or is it already
  handled elsewhere in the codebase?
- **Is there a better solution?** Would a more idiomatic, testable, or maintainable
  approach solve the same problem with less surface area?
- **Is the scope right?** Is the issue too broad (should be split) or too narrow
  (misses an obvious related case that will be filed next week anyway)?

If a clearly superior alternative exists, **propose it to the user first** — one
concrete counter-proposal with a brief rationale (2–3 sentences max). Ask whether
to proceed with the original or the alternative before continuing. Do not propose
alternatives just to have something to say; only raise one if it is genuinely better.

**If evaluation changes the issue's shape, confirm the move, then:**

- **Already handled / redundant →** close it instead of refining:
  `gh issue close <N> --reason "not planned" --comment "<why, + where the work now lives>"`.
- **Too broad → split.** Scope this issue down, and `gh issue create` the carved-out
  sibling(s) with a full label set (§5) and a `Depends on #<N>` / `Follow-ups: #<M>` link
  in both bodies. A sibling that still needs grilling gets `status:unrefined`.
- **Too narrow →** fold the missing case in, or note the related issue it pairs with.

## 3. Grill the user

Don't ask what you can answer yourself by reading the code — only escalate genuine
decisions. Batch related questions in one `AskUserQuestion`; keep drilling until every
ambiguity is gone:

- What exact behavior changes? (inputs → outputs, or acceptance criteria)
- Which files / modules are in scope?
- What is explicitly out of scope?
- Any constraints (performance, compat, API shape)?
- What does "done" look like — how would a reviewer verify it in 10 seconds?

Don't move on until you'd be confident writing an unambiguous PR yourself.

## 4. Rewrite the issue

Produce the final issue body with:
- One-paragraph **Summary** (what + why, no alternatives)
- **Acceptance criteria** — a tight checklist, each item verifiable
- **Out of scope** — explicit list of what this issue does NOT cover
- **Files in scope** — concrete paths or modules

No "option A / option B". No "we could also…". One path only.

## 5. Set labels (required)

An issue is **not refined** until it carries all four: exactly one `priority:*`, one
`complexity:*`, at least one `area:*`, and one `type:*` label.

**`priority:` and `complexity:` — suggest, then confirm.** Propose a value for each
(with a one-line rationale) and **confirm with the user** via `AskUserQuestion` before
applying — don't set them silently. If the issue already has one, still surface your
suggestion and confirm it's still right.

- `priority:` — `low` (nice-to-have), `medium` (normal queue), `high` (next cycle).
- `complexity:` — `haiku` (one-shot, self-contained), `sonnet` (multi-file / needs
  iteration), `opus` (large; multi-session, needs a plan first), `manual-only` (needs
  human action in an external system), `mythos` (foundational; needs a human
  architectural decision first).

**`area:` and `type:` — set without asking.** These follow from the code the issue
touches and its nature; pick them yourself. Run `gh label list` if unsure which
`area:*`/`type:*` values exist.

Apply the labels (replacing any stale one). **Never put the same label in both
`--remove-label` and `--add-label` in one call — they cancel and the label ends up
unset.**

```bash
gh issue edit <N> --add-label "priority:<x>" --add-label "complexity:<y>" \
  --add-label "area:<z>" --add-label "type:<t>"
```

## 6. Update GitHub

Only after the body is final **and** all four label classes are set. Pass the body via
stdin (`--body-file -`) with a heredoc — inline `--body "…"` forces fragile backtick
escaping:

```bash
gh issue edit <N> --body-file - <<'EOF'
<final body>
EOF
gh issue edit <N> --remove-label "status:unrefined" --add-label "status:refined"
```

Never apply `status:ready` — that's the human's go decision, not the refiner's.

**Verify before you're done** — fail loudly if any required class is missing (this catches
a cancelled label or a typo'd name):

```bash
gh issue view <N> --json labels --jq '[.labels[].name] as $l
  | {priority: ($l|map(select(startswith("priority:")))|length==1),
     complexity:($l|map(select(startswith("complexity:")))|length==1),
     area:     ($l|map(select(startswith("area:")))|length>=1),
     type:     ($l|map(select(startswith("type:")))|length==1),
     refined:  ($l|contains(["status:refined"]))}'
```
