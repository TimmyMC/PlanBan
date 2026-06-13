//! Session lifecycle — the hybrid model (managed + external).
//!
//! `run_managed` spawns a simple headless agent run that Clabby owns: it streams
//! output to the log store and the event bus, then records the exit status.
//! `attach` registers an interactive session the user runs themselves, so it
//! still appears in the overview (§3) via its worktree/git state.
//!
//! Core stays UI-free (§7): logs are published to the bus; drivers decide whether
//! to print them.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use serde_json::json;

use crate::config::Config;
use crate::db::{Db, NewSession};
use crate::events::{Event, EventBus};
use crate::model::{Session, SessionKind, SessionStatus};
use crate::{runner, template, Error, Result};

/// Spawn a managed agent run for an issue and drive it to completion, streaming
/// output to the DB and event bus. Returns the final session record.
pub async fn run_managed(
    db: &Db,
    bus: &EventBus,
    config: &Config,
    issue_key: &str,
    agent_name: &str,
) -> Result<Session> {
    let agent = config.agents.get(agent_name).ok_or_else(|| {
        Error::not_found(format!("agent '{agent_name}' is not defined in [agents]"))
    })?;

    let wt = db.worktree_for_issue(issue_key).await?;
    let wt_path = wt.as_ref().map(|w| w.path.clone());
    let branch = wt.as_ref().and_then(|w| w.branch.clone());

    let ctx = json!({
        "issue_key": issue_key,
        "ticket": issue_key,
        "worktree_path": wt_path,
        "branch": branch,
        "agent": agent_name,
    });

    let cmd_line = template::render(&agent.cmd, &ctx)?;
    let cwd: Option<PathBuf> = match &agent.cwd {
        Some(c) => {
            let rendered = template::render(c, &ctx)?;
            if rendered.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(rendered))
            }
        }
        None => wt_path.as_ref().map(PathBuf::from),
    };

    let mut env = HashMap::new();
    for (k, v) in &agent.env {
        env.insert(k.clone(), template::render(v, &ctx)?);
    }

    let mut streaming = runner::spawn_streaming(&cmd_line, cwd.as_deref(), &env)?;

    let session = db
        .insert_session(NewSession {
            issue_key,
            kind: SessionKind::Managed,
            pid: streaming.pid.map(i64::from),
            worktree_path: wt_path.as_deref(),
            branch: branch.as_deref(),
            agent: Some(agent_name),
            status: SessionStatus::Running,
            log_path: None,
        })
        .await?;

    bus.publish(Event::SessionStatus {
        session_id: session.id,
        issue_key: issue_key.to_string(),
        status: SessionStatus::Running.as_str().to_string(),
    });

    // Stream every line into the log store and out to subscribers.
    while let Some(line) = streaming.lines.recv().await {
        let ts = Utc::now();
        db.insert_log(session.id, line.stream.as_str(), &line.text, ts)
            .await?;
        bus.publish(Event::SessionLog {
            session_id: session.id,
            stream: line.stream.as_str().to_string(),
            line: line.text,
            ts,
        });
    }

    let (status, code) = match streaming.child.wait().await {
        Ok(es) if es.success() => (SessionStatus::Exited, es.code().map(|c| c as i64)),
        Ok(es) => (SessionStatus::Failed, es.code().map(|c| c as i64)),
        Err(_) => (SessionStatus::Failed, None),
    };

    db.update_session_status(session.id, status, code).await?;
    bus.publish(Event::SessionStatus {
        session_id: session.id,
        issue_key: issue_key.to_string(),
        status: status.as_str().to_string(),
    });

    db.get_session(session.id)
        .await?
        .ok_or_else(|| Error::other("session record disappeared after completion"))
}

/// Register an externally-run interactive session so it shows up in the overview.
/// Clabby does not own the process; status is observational (defaults to Running).
pub async fn attach(
    db: &Db,
    issue_key: &str,
    worktree_path: &str,
    branch: Option<&str>,
    log_path: Option<&str>,
) -> Result<Session> {
    db.insert_session(NewSession {
        issue_key,
        kind: SessionKind::External,
        pid: None,
        worktree_path: Some(worktree_path),
        branch,
        agent: None,
        status: SessionStatus::Running,
        log_path,
    })
    .await
}
