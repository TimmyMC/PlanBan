# Use-case registry

The canonical list of user-facing **use cases** and the test that proves each. It is the
*source of intent* for "100% of use cases are tested" (Constitution §9). It is **not** prose
you have to remember to update: `crates/cli/tests/use_case_coverage.rs` walks the clap command
tree and **fails the build** if any CLI command or flag is missing a row here or references a
test that doesn't exist. See [`docs/testing.md`](./testing.md) for the test taxonomy.

A **use case** = (CLI) each leaf command + each meaningful flag; (UI) each `api` seam method;
(UX) each named user flow.

## CLI commands & flags

Enforced by `use_case_coverage.rs` (command/flag completeness + the named test must exist).

| Use case | Test |
| --- | --- |
| `sync` | `cli_blackbox::sync_then_status_succeeds` |
| `status` | `cli_blackbox::sync_then_status_succeeds` |
| `status --watch` | `cli_blackbox::status_watch_starts_and_keeps_running` |
| `issue set-status` | `cli_blackbox::set_status_without_push_is_rejected` |
| `session spawn` | `cli_blackbox::managed_session_runs_and_is_recorded` |
| `session spawn --agent` | `cli_blackbox::managed_session_runs_and_is_recorded` |
| `session attach` | `cli_blackbox::external_session_attaches_and_lists` |
| `session attach --worktree` | `cli_blackbox::external_session_attaches_and_lists` |
| `session attach --branch` | `cli_blackbox::external_session_attaches_and_lists` |
| `session attach --log` | `cli_blackbox::external_session_attaches_and_lists` |
| `session list` | `cli_blackbox::managed_session_runs_and_is_recorded` |
| `worktree add` | `cli_blackbox::worktree_add_then_list` |
| `worktree add --branch` | `cli_blackbox::worktree_add_then_list` |
| `worktree add --base` | `cli_blackbox::worktree_add_then_list` |
| `worktree add --path` | `cli_blackbox::worktree_add_then_list` |
| `worktree list` | `cli_blackbox::worktree_add_then_list` |
| `logs tail` | `cli_blackbox::logs_tail_shows_a_managed_sessions_output` |
| `logs tail --lines` | `cli_blackbox::logs_tail_shows_a_managed_sessions_output` |
| `cron run` | `cli_blackbox::cron_run_once_executes_actions` |
| `cron run --once` | `cli_blackbox::cron_run_once_executes_actions` |
| `move` | `cli_blackbox::transition_with_passing_steps_completes` |
| `override` | `cli_blackbox::required_step_gates_then_override_resumes` |
| `override --reason` | `cli_blackbox::required_step_gates_then_override_resumes` |

## UI api methods

Enforced by `ui/src/use-case-coverage.test.ts` (every `Api` method needs a `@usecase:api/<m>`
tagged Playwright spec).

| Use case | Test |
| --- | --- |
| `api/getBoard` | `board.spec.ts` |
| `api/move` | `board.spec.ts` |
| `api/override` | `board.spec.ts` |
| `api/sync` | `board.spec.ts` |

## UX flows (human-judgment residual)

Flows aren't enumerable from code, so this list's *completeness* is the one part CI can't
prove — every listed flow must have a `@usecase:<id>` tagged Playwright spec, but adding a new
flow to the product without adding it here is a review responsibility.

| Flow (id) | Test |
| --- | --- |
| `flow/drag-to-move` | `board.spec.ts` |
| `flow/gated-move-banner` | `board.spec.ts` |
| `flow/override-resume` | `board.spec.ts` |
| `flow/a11y-board` | `board.spec.ts` |
