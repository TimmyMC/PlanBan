//! Domain-neutral data model.
//!
//! Constitution §11: nothing here names Jira, Azure, or Claude. An "issue" has a
//! "status" that is just a string; a "session" runs an "agent"; a "worktree" has
//! git state. A different team's workflow maps onto these same types via config.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A unit of work, mirrored from the external tracker (the source of truth, §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Stable identifier in the tracker (e.g. `PROJ-12`).
    pub key: String,
    pub summary: String,
    /// Status as last observed in the tracker. Authoritative on conflict (§4).
    pub tracker_status: String,
    pub assignee: Option<String>,
    /// Status Clabby believes/displays. Normally equal to `tracker_status`.
    pub local_status: String,
    /// The last status Clabby itself pushed to the tracker. Used to distinguish
    /// "we changed it" from "someone else changed it" during divergence detection.
    pub last_pushed_status: Option<String>,
    /// True when the tracker moved out from under us since our last push.
    pub diverged: bool,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub last_synced: Option<DateTime<Utc>>,
}

impl Issue {
    /// A freshly-seen issue from the tracker with no local history yet.
    pub fn new(
        key: impl Into<String>,
        summary: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        let status = status.into();
        Issue {
            key: key.into(),
            summary: summary.into(),
            tracker_status: status.clone(),
            assignee: None,
            local_status: status,
            last_pushed_status: None,
            diverged: false,
            url: None,
            labels: Vec::new(),
            last_synced: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    /// Clabby spawned and owns the process; output is streamed and captured.
    Managed,
    /// A session the user runs themselves (interactive harness); Clabby observes
    /// it via its worktree/git state and an optional log file.
    External,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Managed => "managed",
            SessionKind::External => "external",
        }
    }
}

impl fmt::Display for SessionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SessionKind {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "managed" => Ok(SessionKind::Managed),
            "external" => Ok(SessionKind::External),
            other => Err(crate::Error::other(format!(
                "unknown session kind: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Waiting,
    Idle,
    Failed,
    Exited,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Running => "running",
            SessionStatus::Waiting => "waiting",
            SessionStatus::Idle => "idle",
            SessionStatus::Failed => "failed",
            SessionStatus::Exited => "exited",
        }
    }
    /// True once the session has stopped (cannot transition further on its own).
    pub fn is_terminal(self) -> bool {
        matches!(self, SessionStatus::Failed | SessionStatus::Exited)
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "running" => SessionStatus::Running,
            "waiting" => SessionStatus::Waiting,
            "idle" => SessionStatus::Idle,
            "failed" => SessionStatus::Failed,
            "exited" => SessionStatus::Exited,
            other => {
                return Err(crate::Error::other(format!(
                    "unknown session status: {other}"
                )))
            }
        })
    }
}

/// A tracked agent session bound to an issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub issue_key: String,
    pub kind: SessionKind,
    pub pid: Option<i64>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub agent: Option<String>,
    pub status: SessionStatus,
    pub log_path: Option<String>,
    pub exit_code: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A git worktree, optionally bound to an issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    pub path: String,
    pub issue_key: Option<String>,
    pub branch: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Live git state for a worktree, derived on demand (not persisted as truth).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitState {
    pub branch: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    pub dirty: bool,
    pub last_commit_summary: Option<String>,
}

// ---- Milestone 2: workflow transitions (gate-with-override) -----------------

/// State of a single attempt to move an issue from one status to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionStatus {
    /// Steps are executing.
    Running,
    /// A required step failed; the issue stays in the source state until override.
    Blocked,
    /// All steps passed (or were overridden); the issue reached the target state.
    Completed,
}

impl TransitionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TransitionStatus::Running => "running",
            TransitionStatus::Blocked => "blocked",
            TransitionStatus::Completed => "completed",
        }
    }
}

impl std::str::FromStr for TransitionStatus {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "running" => TransitionStatus::Running,
            "blocked" => TransitionStatus::Blocked,
            "completed" => TransitionStatus::Completed,
            other => {
                return Err(crate::Error::other(format!(
                    "unknown transition status: {other}"
                )))
            }
        })
    }
}

/// Outcome of running one step within a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    /// Ran successfully.
    Ok,
    /// Ran and failed (non-zero exit / spawn error).
    Failed,
    /// Skipped because its `when` guard was false.
    Skipped,
    /// A failed required step that a human explicitly overrode (audited).
    Overridden,
}

impl StepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StepStatus::Ok => "ok",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
            StepStatus::Overridden => "overridden",
        }
    }
}

impl std::str::FromStr for StepStatus {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ok" => StepStatus::Ok,
            "failed" => StepStatus::Failed,
            "skipped" => StepStatus::Skipped,
            "overridden" => StepStatus::Overridden,
            other => return Err(crate::Error::other(format!("unknown step status: {other}"))),
        })
    }
}

/// A persisted transition attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRun {
    pub id: i64,
    pub issue_key: String,
    pub from_state: String,
    pub to_state: String,
    pub status: TransitionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A persisted step execution within a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRun {
    pub id: i64,
    pub transition_run_id: i64,
    pub step_id: String,
    pub step_index: i64,
    pub required: bool,
    pub status: StepStatus,
    pub exit_code: Option<i64>,
    pub stderr: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_kind_as_str_display_and_from_str_roundtrip() {
        for k in [SessionKind::Managed, SessionKind::External] {
            assert_eq!(k.as_str().parse::<SessionKind>().unwrap(), k);
            assert_eq!(k.to_string(), k.as_str());
        }
        assert!("nope".parse::<SessionKind>().is_err());
    }

    #[test]
    fn session_status_roundtrips_and_reports_terminal() {
        use SessionStatus::{Exited, Failed, Idle, Running, Waiting};
        for s in [Running, Waiting, Idle, Failed, Exited] {
            assert_eq!(s.as_str().parse::<SessionStatus>().unwrap(), s);
            assert_eq!(s.to_string(), s.as_str());
        }
        assert!(Failed.is_terminal());
        assert!(Exited.is_terminal());
        for s in [Running, Waiting, Idle] {
            assert!(!s.is_terminal());
        }
        assert!("bogus".parse::<SessionStatus>().is_err());
    }

    #[test]
    fn transition_status_roundtrips() {
        use TransitionStatus::{Blocked, Completed, Running};
        for s in [Running, Blocked, Completed] {
            assert_eq!(s.as_str().parse::<TransitionStatus>().unwrap(), s);
        }
        assert!("x".parse::<TransitionStatus>().is_err());
    }

    #[test]
    fn step_status_roundtrips() {
        use StepStatus::{Failed, Ok, Overridden, Skipped};
        for s in [Ok, Failed, Skipped, Overridden] {
            assert_eq!(s.as_str().parse::<StepStatus>().unwrap(), s);
        }
        assert!("x".parse::<StepStatus>().is_err());
    }
}
