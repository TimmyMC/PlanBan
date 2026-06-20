# Tiered agent pipeline

An opt-in, hook-driven pipeline that takes a tracker issue from raw to merged through
escalating model tiers, with every agent-authored PR reviewed by a strictly higher tier and
a human at the decisions that matter. It rides **on top of** the existing CI gates — it never
relaxes them (Constitution §1, §9).

## Flow

```
issue opened ─► issue-triage adds status:unrefined ─►(hook) agent-refine (Sonnet)
   ├─ clear      → proposed complexity:<tier> + acceptance-criteria + status:refined
   │                 └─► HUMAN reviews, applies status:ready (the go decision)
   └─ ambiguous  → questions + status:needs-decision ─► HUMAN answers ─► requeue
status:ready ─►(hook) agent-implement (model = tier)  → claims status:in-progress
   └─ draft PR (agent-authored, complexity:<tier>) → CI runs
PR ─►(hook) agent-review (reviewer = tier+1)
   ├─ pass (haiku/sonnet) → approve + mark ready → auto-merge
   ├─ issues              → request changes (stays draft) → implementer loop
   └─ opus PR             → reviewer comments; a HUMAN gives the final approval
gate-touching PR → gate-integrity stays red until an OWNER approves with a review
```

Reviewer tiers: haiku→Sonnet, sonnet→Opus, opus→Opus **+ human**.

## One hard invariant

**The identity that writes code never approves the gate for that code.** A gate change (edits to
CI/test machinery flagged by `scripts/gate-guard.sh`) is cleared only by an **owner approving
review** — an identity-bound signal, computed in `ci-complete.yml`'s `gate-integrity` job from
the PR's reviews, not from a label. The old `gate-change-approved` label grants nothing now, and
`label-guard.yml` strips it if a non-owner applies it. This closes the hole where an implementing
agent self-applied that label to clear its own gate change.

## Trust boundaries & threat model (zero-trust)

**Premise: assume every agent can be prompt-injected.** Refine reads issue bodies, implement and
review read diffs and comments — all attacker-controllable (anyone who can file an issue or open a
PR). So we do **not** rely on an agent doing the right thing; we design so a *hijacked* agent is
still bounded by what its credential and the deterministic gates allow. "The agent promises not
to" is never a control.

**Identities and what each may do** (least privilege):

| Principal | May do | May NOT do |
| --- | --- | --- |
| **Owner** (human, browser, 2FA) | clear gate changes; final-approve opus PRs; merge anything | — must never be an agent's credential |
| **Implementer bot** | branch, push, open draft PRs, comment, non-protected labels | approve, merge, clear a gate, push to trunk |
| **Reviewer bot** | submit reviews, approve+merge *ordinary* PRs | clear a gate change (not an `OWNERS` login); approve its own work |
| **CI `GITHUB_TOKEN`** | run checks, read | trigger further workflows; nothing privileged |

**The injection-resistant floor.** The deterministic gates do not trust agent intent: a hijacked
agent still cannot make a failing test pass, cannot strip `-D warnings`/a coverage floor without
`gate-guard` tripping, and cannot clear a gate change without an `OWNERS`-login *review*. Automation
rides on top of these gates and can never relax them.

**Hard rules (the zero-trust requirements):**

1. **No agent ever holds an owner-allowlisted credential.** Owner approvals are human and
   browser-only — never a stored token, never automated. The gate's identity allowlist is only as
   strong as this rule. (A personal-access token acts *as its creating user*, so a PAT minted by an
   owner still counts as the owner — bot identities must be **separate GitHub accounts**.)
2. **Bot identities are distinct, least-privilege accounts** not present in `OWNERS` or CODEOWNERS;
   implementer ≠ reviewer (so a reviewer can't self-approve, which GitHub also blocks).
3. **Interactive coding agents** (Claude Code/desktop, etc.) run authed as a least-privilege bot in
   an isolated environment — never the owner's `gh`/git login — so injection can't borrow owner
   authority. Add a harness deny-list for `gh pr merge` / `gh pr review` as defense in depth.
4. **Branch protection** backs the above at GitHub level: require CODEOWNERS review, require
   approval from someone other than the last pusher, and dismiss stale approvals.

**Residual risks (named, not hand-waved):**
- An injected **reviewer** could approve a PR crafted to inject it → that PR auto-merges. *Contained*:
  the code still passed every deterministic gate, and gate changes / opus PRs still require a human.
  Worst case is "fully-gate-passing malicious code merges," a bounded blast radius — not arbitrary.
- The `OWNERS` allowlist trusts that an owner-login review is genuinely a human (hence rule 1).
- Dependency supply chain is covered by Dependabot + the same review/gate path, not by this doc.

## Resilience to Claude usage limits

Triggers are **hooks** (issue/PR events), serialized through one repo-wide `agents-global`
concurrency group so **at most one model session runs at a time** — the main lever against
bursting the usage limit. If a run can't finish (usage limit or transient), the implement
workflow **releases the claim** (`status:in-progress` → `status:deferred`), comments, and exits
green. The **reconciler** (`reconciler.yml`, every 6h) re-queues `status:deferred`, reclaims stale
`status:in-progress`, nudges the oldest unrefined issue and un-reviewed PR, and posts a weekly
quality digest. After `ATTEMPT_CAP` (3) failures an issue goes to `status:needs-decision`.

## Setup (owner, one-time)

The pipeline is **inert until enabled** — merging it changes nothing until you:

1. **Identities** — create two GitHub identities distinct from your own (GitHub App
   installations or bot accounts), one implementer and one reviewer. They must be different so
   the reviewer's approval is a real second-party review (GitHub blocks self-approval) and so an
   agent approval can be told apart from the owner's.
2. **Secrets** (Settings → Secrets and variables → Actions → Secrets):
   - `CLAUDE_CODE_OAUTH_TOKEN` — Claude subscription token for `claude-code-action`.
   - `IMPLEMENTER_TOKEN` — PAT/App token for the implementer identity. **Must not be the default
     `GITHUB_TOKEN`** — a PR opened with `GITHUB_TOKEN` does not trigger CI.
   - `REVIEWER_TOKEN` — PAT/App token for the reviewer identity.
3. **Variables** (same screen → Variables):
   - `REVIEWER_LOGINS` — space-separated login(s) of the reviewer identity; auto-merge only
     accepts an agent PR approved by one of these (or an owner). Unset ⇒ agent PRs wait for a human.
   - `AGENTS_ENABLED` — set to `true` to turn the pipeline on. Leave unset/false to pause it.
4. **Owner allowlist** — the logins permitted to clear a gate change live in the `OWNERS` env of
   `ci-complete.yml` (gate-integrity) and `label-guard.yml`, default `TimmyMC`. Update if owners
   change.

## Labels

Lifecycle: `status:unrefined → status:refined → (human) status:ready → status:in-progress →
(PR) → merged`, with `status:needs-decision` (blocked on a human) and `status:deferred` (retry)
as off-ramps. The refiner only ever reaches `status:refined`; a human applies `status:ready`,
which is the gate between refinement and implementation. Routing:
`complexity:{haiku,sonnet,opus}`, `agent-authored`, `review:{passed,changes-requested}`. Holds:
`do-not-merge` (human stop). The taxonomy is version-controlled in `.github/labels.yml` and synced
by `label-sync.yml`.

## Quality invariants (why volume doesn't erode quality)

1. Automation never relaxes a gate — the floor stays machine-enforced (clippy `-D warnings`,
   ratcheting coverage floors, architecture fitness, gate-guard). Agent review is additive.
2. No agent can weaken a gate (gate-guard rule 6 + CODEOWNERS on `.github/**` + owner-review-only
   approval). The pipeline's own files live under `.github/`, so an agent can't edit them either.
3. Coverage can't be gamed silently — weekly `cargo-mutants` catches non-asserting tests.
4. One issue per PR; reviewer checks the PR against the issue's acceptance criteria.
5. `ATTEMPT_CAP` stops infinite failure loops; the weekly digest keeps the human in Overview (§3).

## Rolling out

Enable in stages, watching the `agents-global` runs: flip `AGENTS_ENABLED` after the secrets/vars
are set; soak on `complexity:haiku` issues first (let a couple refine to `status:refined`, then
*you* apply `status:ready` to send them through), then let sonnet and opus issues through. Each
agent workflow also has a `workflow_dispatch` for manual, targeted
runs while you build confidence.
