# Skill: refine-issue

Pick the next unrefined issue and grill the user with clarifying questions
until the issue is ready to hand to an agent. The final rewrite must contain zero
alternatives or open decisions — just the minimal, focused, final description.

## 1. Fetch the issue

Run `.claude/skills/refine-issue/pick-issue.sh` to get the issue.

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

## 3. Grill the user

Ask one focused question at a time. Keep drilling until every ambiguity is gone:

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

## 5. Update GitHub

```bash
gh issue edit <N> --body "<final body>"
gh issue edit <N> --remove-label "status:unrefined" --add-label "status:refined"
```
