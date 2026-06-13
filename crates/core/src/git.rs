//! Git/worktree operations and live state derivation.
//!
//! All git access goes through `git -C <dir>` so behavior doesn't depend on the
//! process working directory. Used to create per-issue worktrees and to derive
//! the at-a-glance git state shown in the overview (§3).

use std::path::Path;

use chrono::Utc;

use crate::model::{GitState, Worktree};
use crate::runner::{run_capture, CommandOutput};
use crate::{Error, Result};

async fn git(dir: &Path, args: &str) -> Result<CommandOutput> {
    let cmd = format!("git -C \"{}\" {}", dir.display(), args);
    run_capture(&cmd, None).await
}

/// Create a worktree for an issue. With `create_branch`, passes `-b <branch>`.
pub async fn add_worktree(
    repo_dir: &Path,
    worktree_path: &Path,
    branch: Option<&str>,
    create_branch: bool,
    base: Option<&str>,
) -> Result<Worktree> {
    let mut args = String::from("worktree add");
    if create_branch {
        if let Some(b) = branch {
            args.push_str(&format!(" -b {b}"));
        }
    }
    args.push_str(&format!(" \"{}\"", worktree_path.display()));
    if let Some(base) = base {
        args.push_str(&format!(" {base}"));
    }
    let out = git(repo_dir, &args).await?;
    if !out.success() {
        return Err(Error::command(format!(
            "git worktree add failed: {}",
            out.stderr.trim()
        )));
    }
    Ok(Worktree {
        path: worktree_path.display().to_string(),
        issue_key: None,
        branch: branch.map(|s| s.to_string()),
        created_at: Utc::now(),
    })
}

/// Parse `git worktree list --porcelain` into (path, branch) pairs.
pub async fn list_worktrees(repo_dir: &Path) -> Result<Vec<(String, Option<String>)>> {
    let out = git(repo_dir, "worktree list --porcelain").await?;
    if !out.success() {
        return Err(Error::command(format!(
            "git worktree list failed: {}",
            out.stderr.trim()
        )));
    }
    let mut result = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_branch: Option<String> = None;
    for line in out.stdout.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            // flush previous
            if let Some(path) = cur_path.take() {
                result.push((path, cur_branch.take()));
            }
            cur_path = Some(p.trim().to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            cur_branch = Some(b.trim().trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = cur_path.take() {
        result.push((path, cur_branch.take()));
    }
    Ok(result)
}

/// Derive live git state for a worktree. Missing pieces (e.g. no upstream) are
/// reported as zero/none rather than as errors — this feeds a dashboard.
pub async fn git_state(path: &Path) -> Result<GitState> {
    let mut st = GitState::default();

    if let Ok(o) = git(path, "rev-parse --abbrev-ref HEAD").await {
        if o.success() {
            let b = o.stdout.trim();
            if !b.is_empty() {
                st.branch = Some(b.to_string());
            }
        }
    }

    if let Ok(o) = git(path, "status --porcelain").await {
        st.dirty = o.success() && !o.stdout.trim().is_empty();
    }

    // left-right count vs upstream: "<behind>\t<ahead>". Fails with no upstream.
    if let Ok(o) = git(path, "rev-list --left-right --count @{upstream}...HEAD").await {
        if o.success() {
            let parts: Vec<&str> = o.stdout.split_whitespace().collect();
            if parts.len() == 2 {
                st.behind = parts[0].parse().unwrap_or(0);
                st.ahead = parts[1].parse().unwrap_or(0);
            }
        }
    }

    if let Ok(o) = git(path, "log -1 --pretty=%s").await {
        if o.success() {
            let s = o.stdout.trim();
            if !s.is_empty() {
                st.last_commit_summary = Some(s.to_string());
            }
        }
    }

    Ok(st)
}
