# Tiered agent pipeline

An opt-in, hook-driven pipeline that takes a tracker issue from raw to merged through
escalating model tiers, with every agent PR reviewed by a strictly higher tier and
a human at the decisions that matter. It rides **on top of** the existing CI gates — it never
relaxes them (Constitution §1, §9).

## Flow

```
issue opened ─► issue-triage adds status:unrefined ─►(hook) agent-refine (Sonnet)
   ├─ clear      → proposed complexity:<tier> + acceptance-criteria + status:refined
   └─ ambiguous  → questions + status:needs-decision ─► HUMAN answers ─► requeue
status:refined ─►(local) /implement-next picks a TRUSTED-authored refined issue → claims status:in-progress
   └─ draft PR (agent-review, complexity:<tier>) opened as Zlyzart → CI runs
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
| **Implementer** (local Claude Code authed as the **Zlyzart** bot) | branch, push, open draft PRs, comment, non-protected labels | *can* submit reviews (PR-write includes that), but its approval is **inert**: not an `OWNERS` login (can't clear a gate), not in `REVIEWER_LOGINS` (can't trigger auto-merge), and GitHub blocks self-approval. Cannot push to trunk or merge. |
| **Reviewer** (the installed Claude **GitHub App**) | submit reviews, approve → auto-merge *ordinary* PRs | cannot clear a gate change (not an `OWNERS` login) or approve its own work |
| **Refiner** (CI Claude via `agent-refine`, runs on **untrusted** issue text) | comment on issues, set non-protected labels, read the repo | runs as the Actions `GITHUB_TOKEN` (can't approve PRs or trigger downstream workflows) with `contents: read` (no code write) and tools `Bash(gh:*),Read,Grep,Glob` (no merge). It processes **any** public author's issue body, so it is the pipeline's prompt-injection **front door** — bounded to issue/label bookkeeping on an ephemeral runner. |
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
- **Untrusted input reaches the refiner — non-collaborators are NOT walled off from the agents.**
  This is a public repo: anyone can open an issue, and `issue-triage.yml` auto-labels every new one
  `status:unrefined`, which feeds `agent-refine` — an LLM run over the **raw, attacker-controllable
  body**. Labels gate the *implementer* (only collaborators can apply `status:*`), but they do not
  gate the *refiner*; its protection is **containment, not exclusion**. *Contained*: refine holds
  only the Actions `GITHUB_TOKEN` (`contents: read` + `issues: write`; tools `Bash(gh:*),Read,Grep,
  Glob`) — ephemeral runner, no code-write, no merge, can't approve or trigger downstream workflows.
  An injected refiner *could* shuffle labels, including self-applying `status:refined` to the
  attacker's own issue — but the downstream author check (the implementer's consume-time selector +
  the reconciler's author re-check, both requiring write access) reject the non-collaborator author,
  with the least-privilege/gate floor bounding whatever slips. Worst case is issue/label noise, not code.
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
whole box. We accept that **only** because of **containment**, the two boundaries a hijacked run can't
cross: (a) the implementer identity is least-privilege (Zlyzart: Write, not `OWNERS`/CODEOWNERS
— its approvals are inert, it can't merge or clear a gate), and (b) the deterministic gates are
unchanged. The author re-check (the implementer's consume-time selector + the reconciler's
author re-check, both requiring write access) is **defense-in-depth, not part of that
acceptance**: the `/implement-next` selector that checks at consume-time lives in a prompt, so it
can't contain a hijacked run. Its value is keeping an *honest* run from being handed attacker-authored
input. For unattended runs, run the local implementer in an **isolated environment**
(container/VM/dedicated OS user), never your daily login — Zlyzart bounds the *GitHub* authority
but not the *machine*.

1. **Identities** — install the Claude **GitHub App** on the repo (e.g. via `/install-github-app`)
   with **Pull requests: Read & Write** so it can approve. Run interactive/local Claude Code authed
   as **Zlyzart**, never the owner. The two must differ (the App ≠ Zlyzart) so the App's approval is
   a genuine second-party review.
2. **Secrets** (Settings → Secrets and variables → Actions → Secrets):
   - `CLAUDE_CODE_OAUTH_TOKEN` — Claude subscription token for `claude-code-action` (used by review
     + refine). *No `IMPLEMENTER_TOKEN`/`REVIEWER_TOKEN` needed in this mode* — the reviewer auths as
     the installed App (omit `github_token`), and the implementer is your local `gh`/git (Zlyzart).
3. **Variables** (same screen → Variables):
   - `REVIEWER_LOGINS` — the App's `<app-name>[bot]` login (e.g. `claude[bot]`); auto-merge only
     honors an agent PR approved by one of these (or an owner). Unset ⇒ agent PRs wait for a human.
   - `AGENTS_ENABLED` — `true` turns review/refine/reconciler on. Leave unset/false to pause.
4. **Owner allowlist** — the logins permitted to clear a gate change live in the `OWNERS` env of
   `ci-complete.yml` (gate-integrity) and `label-guard.yml`, default `TimmyMC`. Update if owners
   change.

The **author allowlist needs no config**: `/implement-next` selects only write-collaborator-authored
issues (checked live from the collaborator-permission API — no hardcoded list; manage trust via
Settings → Collaborators) and re-queues failures to `status:deferred` (never `status:refined`)
so the reconciler re-promotes.

## Labels

Lifecycle: `status:unrefined → status:refined → status:in-progress → (PR) → merged`, with
`status:needs-decision` (blocked on a human) and `status:deferred` (retry) as off-ramps.
The refiner produces `status:refined`; `/implement-next` picks it up automatically. Routing:
`complexity:{haiku,sonnet,opus}`, `agent-review`, `review:{passed,changes-requested}`. Holds:
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
are set; soak on `complexity:haiku` issues first (let a couple refine to `status:refined` and
watch `/implement-next` pick them up automatically), then let sonnet and opus issues through. Each
agent workflow also has a `workflow_dispatch` for manual, targeted runs while you build confidence.
