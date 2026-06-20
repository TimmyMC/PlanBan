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
status:ready ─►(local) /implement-next picks a TRUSTED-authored ready issue → claims status:in-progress
   └─ draft PR (agent-authored, complexity:<tier>) opened as Zlyzart → CI runs
PR ─►(hook) agent-review (installed App, reviewer = tier+1)
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

| Principal | May do | Its approvals/merges are bounded by |
| --- | --- | --- |
| **Owner** (human, browser, 2FA) | clear gate changes; final-approve opus PRs; merge anything | — must never be an agent's credential |
| **Implementer** (default: local Claude Code authed as the **Zlyzart** bot; or the CI implementer when `CI_IMPLEMENT_ENABLED=true`) | branch, push, open draft PRs, comment, non-protected labels | *can* submit reviews (PR-write includes that), but its approval is **inert**: not an `OWNERS` login (can't clear a gate), not in `REVIEWER_LOGINS` (can't trigger auto-merge), and GitHub blocks self-approval. Cannot push to trunk or merge. |
| **Reviewer** (the installed Claude **GitHub App**) | submit reviews, approve → auto-merge *ordinary* PRs | cannot clear a gate change (not an `OWNERS` login) or approve its own work |
| **CI `GITHUB_TOKEN`** | run checks, read | cannot approve PRs at all (GitHub blocks the Actions token), nor trigger further workflows |

> **There is no GitHub permission for "open a PR but never approve."** Reviews live under the same
> fine-grained `Pull requests: write` scope as creating PRs, so any bot that can open a PR can also
> *cast* an approval. Containment is therefore by **identity allowlist** (`OWNERS` for gates,
> `REVIEWER_LOGINS` for merge) plus GitHub's self-approval block — **never** by assuming the token
> lacks the verb. Two corollaries: (a) require branch-protection approvals from **CODEOWNERS / a
> specific reviewer**, not "any 1 review," so a stray bot approval can't satisfy the merge rule; and
> (b) the reviewer must be a real bot identity — a **GitHub App** (PATs are unsupported here) —
> because the Actions `GITHUB_TOKEN` is forbidden from approving at all.

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
   authority. A harness guard (the `deny-pr-write.sh` PreToolUse hook) denies `gh pr merge` /
   `gh pr review` — including wrapped/aliased/REST-API forms — as defense in depth.
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

The pipeline is **inert until enabled** — merging it changes nothing until you set the secrets
and variables. This repo runs the **local implementer mode** (below) by default; the all-CI
two-bot setup is the opt-in fallback.

### Local implementer mode (default)

Implementation runs **locally** — Claude Code on the maintainer's machine, authed as the
least-privilege **Zlyzart** bot — driven by the `/implement-next` routine
(`.claude/commands/implement-next.md`). Only **review** runs in CI, as the installed Claude
**GitHub App**. Two distinct identities (Zlyzart ≠ the App) keep the second-party-review invariant.

*Why this shape:* a CI implementer is an ephemeral, repo-scoped token in a throwaway runner;
a local implementer is a credential on a real machine, so a prompt-injected run could reach the
whole box. We accept that **only** because (a) the implementer identity is least-privilege
(Zlyzart: Write, not `OWNERS`/CODEOWNERS — its approvals are inert, it can't merge or clear a
gate), (b) the deterministic gates are unchanged, and (c) the **author allowlist** below means
the issue body feeding the local agent is never attacker-authored. For unattended runs, run the
local implementer in an **isolated environment** (container/VM/dedicated OS user), never your
daily login — Zlyzart bounds the *GitHub* authority but not the *machine*.

1. **Identities** — install the Claude **GitHub App** on the repo (e.g. via `/install-github-app`)
   with **Pull requests: Read & Write** so it can approve. Run interactive/local Claude Code authed
   as **Zlyzart**, never the owner. The two must differ (the App ≠ Zlyzart) so the App's approval is
   a genuine second-party review.
2. **Secrets** (Settings → Secrets and variables → Actions → Secrets):
   - `CLAUDE_CODE_OAUTH_TOKEN` — Claude subscription token for `claude-code-action` (used by review
     + refine). *No `IMPLEMENTER_TOKEN`/`REVIEWER_TOKEN` needed in this mode* — the reviewer auths as
     the installed App (omit `github_token`), and the implementer is your local `gh`/git (Zlyzart).
3. **Variables** (same screen → Variables):
   - `REVIEWER_LOGINS` — the App's `<app-name>[bot]` login; auto-merge only honors an agent PR
     approved by one of these (or an owner). Unset ⇒ agent PRs wait for a human.
   - `IMPLEMENT_AUTHORS` — space-separated logins whose issues may be auto-implemented
     (default `TimmyMC Zlyzart`). `ready-author-guard.yml` only lets an issue hold `status:ready`
     when it is **authored by** one of these **and** was **promoted to ready by an `OWNERS` login**
     (the human go-decision); otherwise it strips `status:ready` → `status:needs-decision`. The
     owner-promoter check stops a hijacked bot (which is itself on the author allowlist) from
     self-authoring + self-promoting an issue. The `/implement-next` routine also filters by author,
     and re-queues failures to `status:deferred` (never `status:ready`) so the reconciler re-promotes.
   - `AGENTS_ENABLED` — `true` turns review/refine/reconciler on. Leave unset/false to pause.
   - `CI_IMPLEMENT_ENABLED` — leave **unset**. Set `true` only to fall back to CI implementation.
4. **Owner allowlist** — the logins permitted to clear a gate change live in the `OWNERS` env of
   `ci-complete.yml` (gate-integrity) and `label-guard.yml`, default `TimmyMC`. Update if owners
   change.

### CI implementer fallback (opt-in)

To implement in CI instead, set `CI_IMPLEMENT_ENABLED=true` and create a **second** distinct
GitHub App for the implementer (implementer ≠ reviewer). Wire `agent-implement.yml`'s
`IMPLEMENTER_TOKEN` as an **App token minted at runtime** with `actions/create-github-app-token`
(secrets `IMPLEMENTER_APP_ID` + `IMPLEMENTER_PRIVATE_KEY`) — a static PAT is unsupported (a
non-collaborator PAT 403s `claude-code-action`'s actor precheck) and a static App token can't be
pasted as a secret (it expires hourly). The same `actions/create-github-app-token` swap is how
you'd give the reviewer its own keyed App instead of the installed one.

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
