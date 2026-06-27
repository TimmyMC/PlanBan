# Skill: refine-issue

Pick the next unrefined issue and grill the user with clarifying questions
until the issue is ready to hand to an agent. The final rewrite must contain zero
alternatives or open decisions — just the minimal, focused, final description.

## 1. Fetch the issue

Run `.claude/skills/refine-issue/pick-issue.sh` to get the issue.

## 2. Grill the user

Ask one focused question at a time. Keep drilling until every ambiguity is gone:

- What exact behavior changes? (inputs → outputs, or acceptance criteria)
- Which files / modules are in scope?
- What is explicitly out of scope?
- Any constraints (performance, compat, API shape)?
- What does "done" look like — how would a reviewer verify it in 10 seconds?

Don't move on until you'd be confident writing an unambiguous PR yourself.

## 3. Rewrite the issue

Produce the final issue body with:
- One-paragraph **Summary** (what + why, no alternatives)
- **Acceptance criteria** — a tight checklist, each item verifiable
- **Out of scope** — explicit list of what this issue does NOT cover
- **Files in scope** — concrete paths or modules

No "option A / option B". No "we could also…". One path only.

## 4. Update GitHub

```bash
gh issue edit <N> --body "<final body>"
gh issue edit <N> --remove-label "status:unrefined" --add-label "status:refined"
```
