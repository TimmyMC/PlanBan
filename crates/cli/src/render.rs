//! Overview table rendering. Driver-side presentation only (§7): core hands us
//! data, the CLI decides how it looks. UX matters here (§10) — the overview is
//! the product's face, so keep it scannable.

use clabby_core::model::{GitState, Session, SessionKind};
use clabby_core::overview::OverviewRow;

fn pad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        // truncate with an ellipsis so columns stay aligned
        if w <= 1 {
            s.chars().take(w).collect()
        } else {
            let mut t: String = s.chars().take(w - 1).collect();
            t.push('~');
            t
        }
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

fn session_cell(sessions: &[Session]) -> String {
    let Some(latest) = sessions.last() else {
        return "-".to_string();
    };
    let kind = match latest.kind {
        SessionKind::Managed => "mgd",
        SessionKind::External => "ext",
    };
    let mut cell = format!("{} ({kind})", latest.status);
    let extra = sessions.len().saturating_sub(1);
    if extra > 0 {
        cell.push_str(&format!(" +{extra}"));
    }
    cell
}

fn git_cell(git: &Option<GitState>) -> String {
    let Some(g) = git else {
        return "-".to_string();
    };
    let mut parts = Vec::new();
    if g.ahead > 0 {
        parts.push(format!("+{} ahead", g.ahead));
    }
    if g.behind > 0 {
        parts.push(format!("-{} behind", g.behind));
    }
    if g.dirty {
        parts.push("dirty".to_string());
    }
    if parts.is_empty() {
        "clean".to_string()
    } else {
        parts.join(", ")
    }
}

fn worktree_cell(path: &Option<String>) -> String {
    match path {
        Some(p) => std::path::Path::new(p)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.clone()),
        None => "-".to_string(),
    }
}

fn flags_cell(row: &OverviewRow) -> String {
    if row.issue.diverged {
        format!("! diverged(->{})", row.issue.tracker_status)
    } else {
        String::new()
    }
}

/// Render the overview as an aligned text table.
pub fn overview_table(rows: &[OverviewRow]) -> String {
    const W_ISSUE: usize = 12;
    const W_STATUS: usize = 14;
    const W_SESSION: usize = 18;
    const W_WT: usize = 16;
    const W_GIT: usize = 20;

    let mut out = String::new();
    out.push_str(&format!(
        "{} {} {} {} {} {}\n",
        pad("ISSUE", W_ISSUE),
        pad("STATUS", W_STATUS),
        pad("SESSION", W_SESSION),
        pad("WORKTREE", W_WT),
        pad("GIT", W_GIT),
        "FLAGS",
    ));

    if rows.is_empty() {
        out.push_str("(no issues — run `clabby sync`)\n");
        return out;
    }

    for row in rows {
        out.push_str(&format!(
            "{} {} {} {} {} {}\n",
            pad(&row.issue.key, W_ISSUE),
            pad(&row.issue.local_status, W_STATUS),
            pad(&session_cell(&row.sessions), W_SESSION),
            pad(&worktree_cell(&row.worktree_path), W_WT),
            pad(&git_cell(&row.git), W_GIT),
            flags_cell(row),
        ));
    }

    let diverged = rows.iter().filter(|r| r.issue.diverged).count();
    out.push_str(&format!(
        "\n{} issue(s), {} diverged.\n",
        rows.len(),
        diverged
    ));
    out
}
