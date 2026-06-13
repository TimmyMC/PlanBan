//! The deterministic workflow engine (Milestone 2).
//!
//! Moving an issue from one status to another runs the configured transition's
//! steps in order. A required step that fails **gates** the transition: the issue
//! stays in its source state and nothing downstream runs (§2). A human can
//! override the blocked step with a recorded reason (logged for debugging, not
//! compliance), which resumes the remaining steps. The whole workflow is config
//! (§5, §11) — the
//! engine knows only "issue", "status", "step".

use serde_json::{json, Value};

use crate::config::{Config, StepConfig, TransitionConfig};
use crate::db::{Db, NewStepRun};
use crate::events::EventBus;
use crate::model::{SessionStatus, StepStatus, TransitionStatus};
use crate::{audit, runner, session, template, Error, Result};

/// What happened when we tried to move an issue.
#[derive(Debug, Clone)]
pub struct TransitionOutcome {
    pub transition_run_id: i64,
    pub issue_key: String,
    pub from: String,
    pub to: String,
    pub completed: bool,
    pub blocked: Option<BlockedStep>,
}

#[derive(Debug, Clone)]
pub struct BlockedStep {
    pub step_run_id: i64,
    pub step_id: String,
    pub error: String,
}

/// Move `issue_key` to `to_state`, running the transition's steps under the gate.
pub async fn transition(
    db: &Db,
    config: &Config,
    bus: &EventBus,
    issue_key: &str,
    to_state: &str,
) -> Result<TransitionOutcome> {
    Engine { db, config, bus }
        .transition(issue_key, to_state)
        .await
}

/// Override the blocked step of an issue's latest blocked transition, then resume
/// the remaining steps. The reason is mandatory and audited (§2).
pub async fn override_for_issue(
    db: &Db,
    config: &Config,
    bus: &EventBus,
    issue_key: &str,
    reason: &str,
) -> Result<TransitionOutcome> {
    Engine { db, config, bus }
        .override_for_issue(issue_key, reason)
        .await
}

/// Bundles the engine's collaborators so step plumbing isn't threaded through
/// every helper as positional arguments.
struct Engine<'a> {
    db: &'a Db,
    config: &'a Config,
    bus: &'a EventBus,
}

impl Engine<'_> {
    async fn transition(&self, issue_key: &str, to_state: &str) -> Result<TransitionOutcome> {
        let issue = self.db.get_issue(issue_key).await?.ok_or_else(|| {
            Error::not_found(format!(
                "issue '{issue_key}' not found (run `clabby sync`?)"
            ))
        })?;
        let from = issue.local_status.clone();

        let tc = self
            .config
            .find_transition(&from, to_state)
            .ok_or_else(|| {
                Error::other(format!(
                    "no transition configured from '{from}' to '{to_state}'"
                ))
            })?;

        let run_id = self
            .db
            .insert_transition_run(issue_key, &from, to_state, TransitionStatus::Running)
            .await?;

        self.run_hooks(
            issue_key,
            &from,
            to_state,
            &self.config.hooks.pre_transition,
        )
        .await;

        self.execute_from(run_id, issue_key, tc, to_state, 0).await
    }

    async fn override_for_issue(&self, issue_key: &str, reason: &str) -> Result<TransitionOutcome> {
        let tr = self
            .db
            .latest_blocked_transition(issue_key)
            .await?
            .ok_or_else(|| {
                Error::other(format!(
                    "issue '{issue_key}' has no blocked transition to override"
                ))
            })?;
        let step = self.db.blocking_step(tr.id).await?.ok_or_else(|| {
            Error::other("blocked transition has no failed step (inconsistent state)")
        })?;

        audit::record_override(self.db, issue_key, step.id, &step.step_id, reason).await?;
        self.db
            .update_step_status(step.id, StepStatus::Overridden)
            .await?;
        self.db
            .update_transition_status(tr.id, TransitionStatus::Running)
            .await?;

        let tc = self
            .config
            .find_transition(&tr.from_state, &tr.to_state)
            .ok_or_else(|| Error::other("the transition's config no longer exists"))?;

        let resume_at = usize::try_from(step.step_index + 1).unwrap_or(0);
        self.execute_from(tr.id, issue_key, tc, &tr.to_state, resume_at)
            .await
    }

    /// Run a transition's steps from `start_index`, applying gate-with-override.
    async fn execute_from(
        &self,
        run_id: i64,
        issue_key: &str,
        tc: &TransitionConfig,
        to_state: &str,
        start_index: usize,
    ) -> Result<TransitionOutcome> {
        let ctx = self.build_context(issue_key, &tc.from, to_state).await?;

        for (i, step) in tc.steps.iter().enumerate().skip(start_index) {
            let idx = i as i64;

            // `when` guard: skip the step (recorded) when the expression is false.
            if let Some(expr) = &step.when {
                if !template::eval_bool(expr, &ctx)? {
                    self.db
                        .insert_step_run(NewStepRun {
                            transition_run_id: run_id,
                            step_id: &step.id,
                            step_index: idx,
                            required: step.required,
                            status: StepStatus::Skipped,
                            exit_code: None,
                            stderr: None,
                        })
                        .await?;
                    continue;
                }
            }

            match self.run_step(issue_key, &ctx, step).await {
                Ok(()) => {
                    self.db
                        .insert_step_run(NewStepRun {
                            transition_run_id: run_id,
                            step_id: &step.id,
                            step_index: idx,
                            required: step.required,
                            status: StepStatus::Ok,
                            exit_code: Some(0),
                            stderr: None,
                        })
                        .await?;
                }
                Err(e) => {
                    let msg = e.to_string();
                    self.run_hooks(
                        issue_key,
                        &tc.from,
                        to_state,
                        &self.config.hooks.on_step_fail,
                    )
                    .await;
                    let step_run_id = self
                        .db
                        .insert_step_run(NewStepRun {
                            transition_run_id: run_id,
                            step_id: &step.id,
                            step_index: idx,
                            required: step.required,
                            status: StepStatus::Failed,
                            exit_code: None,
                            stderr: Some(&msg),
                        })
                        .await?;
                    if step.required {
                        // Gate: stop here, leave the issue in its source state (§2).
                        self.db
                            .update_transition_status(run_id, TransitionStatus::Blocked)
                            .await?;
                        return Ok(TransitionOutcome {
                            transition_run_id: run_id,
                            issue_key: issue_key.to_string(),
                            from: tc.from.clone(),
                            to: to_state.to_string(),
                            completed: false,
                            blocked: Some(BlockedStep {
                                step_run_id,
                                step_id: step.id.clone(),
                                error: msg,
                            }),
                        });
                    }
                    // Optional step: recorded as failed, but the workflow continues.
                }
            }
        }

        // All steps passed / were skipped / were overridden: the move is complete.
        self.db
            .update_transition_status(run_id, TransitionStatus::Completed)
            .await?;
        // Record the move as ours so the next sync doesn't flag it as divergence (§4).
        self.db.mark_issue_pushed(issue_key, to_state).await?;
        self.run_hooks(
            issue_key,
            &tc.from,
            to_state,
            &self.config.hooks.post_transition,
        )
        .await;

        Ok(TransitionOutcome {
            transition_run_id: run_id,
            issue_key: issue_key.to_string(),
            from: tc.from.clone(),
            to: to_state.to_string(),
            completed: true,
            blocked: None,
        })
    }

    async fn run_step(&self, issue_key: &str, ctx: &Value, step: &StepConfig) -> Result<()> {
        if let Some(agent_name) = &step.agent {
            let s =
                session::run_managed(self.db, self.bus, self.config, issue_key, agent_name).await?;
            if s.status == SessionStatus::Exited {
                Ok(())
            } else {
                Err(Error::command(format!(
                    "agent step '{}' failed (exit {})",
                    step.id,
                    s.exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".to_string())
                )))
            }
        } else if let Some(cmd_tmpl) = &step.cmd {
            let cmd = template::render(cmd_tmpl, ctx)?;
            let out = runner::run_capture(&cmd, Some(&self.config.repo_dir())).await?;
            if out.success() {
                Ok(())
            } else {
                Err(Error::command(format!(
                    "step '{}' exited {}: {}",
                    step.id,
                    out.status,
                    out.stderr.trim()
                )))
            }
        } else {
            Err(Error::config(format!(
                "step '{}' has neither `cmd` nor `agent`",
                step.id
            )))
        }
    }

    /// Build the variable context steps render against. Tracker-neutral (§11):
    /// the issue's fields plus the transition's from/to and any bound worktree.
    async fn build_context(&self, issue_key: &str, from: &str, to: &str) -> Result<Value> {
        let issue = self.db.get_issue(issue_key).await?;
        let wt = self.db.worktree_for_issue(issue_key).await?;
        let summary = issue.map(|i| i.summary).unwrap_or_default();
        Ok(json!({
            "issue_key": issue_key,
            "key": issue_key,
            "ticket": issue_key,
            "summary": summary,
            "from": from,
            "to": to,
            "status": to,
            "worktree_path": wt.as_ref().map(|w| w.path.clone()),
            "branch": wt.as_ref().and_then(|w| w.branch.clone()),
        }))
    }

    /// Run best-effort hook command templates; failures are logged, never gating.
    /// Anything that must block belongs in a required step, not a hook.
    async fn run_hooks(&self, issue_key: &str, from: &str, to: &str, hooks: &[String]) {
        if hooks.is_empty() {
            return;
        }
        let ctx = json!({ "issue_key": issue_key, "key": issue_key, "from": from, "to": to });
        for tmpl in hooks {
            match template::render(tmpl, &ctx) {
                Ok(cmd) => match runner::run_capture(&cmd, Some(&self.config.repo_dir())).await {
                    Ok(out) if !out.success() => {
                        tracing::warn!(hook = %cmd, "hook exited non-zero: {}", out.stderr.trim());
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(hook = %cmd, "hook failed to spawn: {e}"),
                },
                Err(e) => tracing::warn!("hook template error: {e}"),
            }
        }
    }
}
