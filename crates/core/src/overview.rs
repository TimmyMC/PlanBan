//! The overview — Clabby's reason to exist (§3).
//!
//! Aggregates each issue with its sessions and live git state into rows. Returns
//! data only; formatting belongs to the driver (§7), so the CLI table and the
//! future GUI render the same source.

use std::path::Path;

use crate::db::Db;
use crate::model::{GitState, Issue, Session};
use crate::Result;

#[derive(Debug, Clone)]
pub struct OverviewRow {
    pub issue: Issue,
    pub sessions: Vec<Session>,
    pub worktree_path: Option<String>,
    pub git: Option<GitState>,
}

/// Build the full overview: every issue, its sessions, and the git state of its
/// worktree (if any). Git derivation failures degrade to `None` rather than
/// failing the whole overview.
pub async fn build(db: &Db) -> Result<Vec<OverviewRow>> {
    let issues = db.list_issues().await?;
    let mut rows = Vec::with_capacity(issues.len());
    for issue in issues {
        let sessions = db.list_sessions_for_issue(&issue.key).await?;
        let wt = db.worktree_for_issue(&issue.key).await?;
        let git = match &wt {
            Some(w) => crate::git::git_state(Path::new(&w.path)).await.ok(),
            None => None,
        };
        rows.push(OverviewRow {
            issue,
            sessions,
            worktree_path: wt.map(|w| w.path),
            git,
        });
    }
    Ok(rows)
}
