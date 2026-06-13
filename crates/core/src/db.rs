//! SQLite persistence: schema migration + repositories.
//!
//! Timestamps are stored as RFC3339 text and booleans as 0/1 integers so the
//! on-disk format is predictable and inspectable (§8: easy to verify). Mapping to
//! domain types happens here; callers see only `model` types.
//!
//! **Deliberate choice:** queries use the runtime `sqlx::query` API, not the
//! compile-time-checked `query!` macros. The macros require a `DATABASE_URL` or a
//! committed offline cache at build time and a regeneration step on every SQL
//! change — friction that fights velocity (Constitution §8, §12). Correctness of
//! these queries is instead guaranteed by the integration tests
//! (`crates/core/tests/e2e.rs` and the CLI black-box suite), which exercise every
//! statement end-to-end against a real database.

use std::path::Path;

use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

use crate::model::{
    Issue, Session, SessionKind, SessionStatus, StepRun, StepStatus, TransitionRun,
    TransitionStatus, Worktree,
};
use crate::{Error, Result};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS issues (
    key                TEXT PRIMARY KEY,
    summary            TEXT NOT NULL DEFAULT '',
    tracker_status     TEXT NOT NULL DEFAULT '',
    assignee           TEXT,
    local_status       TEXT NOT NULL DEFAULT '',
    last_pushed_status TEXT,
    diverged           INTEGER NOT NULL DEFAULT 0,
    url                TEXT,
    labels             TEXT NOT NULL DEFAULT '[]',
    last_synced        TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_key     TEXT NOT NULL,
    kind          TEXT NOT NULL,
    pid           INTEGER,
    worktree_path TEXT,
    branch        TEXT,
    agent         TEXT,
    status        TEXT NOT NULL,
    log_path      TEXT,
    exit_code     INTEGER,
    started_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS worktrees (
    path       TEXT PRIMARY KEY,
    issue_key  TEXT,
    branch     TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS session_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    stream     TEXT NOT NULL,
    line       TEXT NOT NULL,
    ts         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_logs_session ON session_logs(session_id);

CREATE TABLE IF NOT EXISTS sync_log (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    ts       TEXT NOT NULL,
    fetched  INTEGER NOT NULL,
    diverged INTEGER NOT NULL,
    detail   TEXT
);

CREATE TABLE IF NOT EXISTS cron_runs (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    ts     TEXT NOT NULL,
    ok     INTEGER NOT NULL,
    detail TEXT
);

-- Operational log: override reasons and other manual actions, for debugging.
CREATE TABLE IF NOT EXISTS audit_log (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    ts        TEXT NOT NULL,
    kind      TEXT NOT NULL,
    issue_key TEXT,
    reason    TEXT,
    detail    TEXT
);

-- Milestone 2: one row per attempt to move an issue between statuses.
CREATE TABLE IF NOT EXISTS transition_runs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_key  TEXT NOT NULL,
    from_state TEXT NOT NULL,
    to_state   TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- One row per step executed within a transition.
CREATE TABLE IF NOT EXISTS step_runs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    transition_run_id INTEGER NOT NULL,
    step_id           TEXT NOT NULL,
    step_index        INTEGER NOT NULL,
    required          INTEGER NOT NULL,
    status            TEXT NOT NULL,
    exit_code         INTEGER,
    stderr            TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_step_runs_transition ON step_runs(transition_run_id);
"#;

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

/// Parameters for [`Db::insert_session`], grouped so the call site reads as named
/// fields instead of a row of positional arguments.
pub struct NewSession<'a> {
    pub issue_key: &'a str,
    pub kind: SessionKind,
    pub pid: Option<i64>,
    pub worktree_path: Option<&'a str>,
    pub branch: Option<&'a str>,
    pub agent: Option<&'a str>,
    pub status: SessionStatus,
    pub log_path: Option<&'a str>,
}

/// Parameters for [`Db::insert_step_run`] (same rationale as [`NewSession`]).
pub struct NewStepRun<'a> {
    pub transition_run_id: i64,
    pub step_id: &'a str,
    pub step_index: i64,
    pub required: bool,
    pub status: StepStatus,
    pub exit_code: Option<i64>,
    pub stderr: Option<&'a str>,
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map_or_else(|_| Utc::now(), |d| d.with_timezone(&Utc))
}

fn parse_ts_opt(s: Option<String>) -> Option<DateTime<Utc>> {
    s.as_deref().map(parse_ts)
}

impl Db {
    /// Open (creating if necessary) a database at `path` and run migrations.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Db> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        let db = Db { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
        sqlx::raw_sql(SCHEMA).execute(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ---- issues -------------------------------------------------------------

    pub async fn get_issue(&self, key: &str) -> Result<Option<Issue>> {
        let row = sqlx::query("SELECT * FROM issues WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        row.map(row_to_issue).transpose()
    }

    pub async fn list_issues(&self) -> Result<Vec<Issue>> {
        let rows = sqlx::query("SELECT * FROM issues ORDER BY key")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(row_to_issue).collect()
    }

    /// Insert or replace an issue wholesale. Divergence/status reconciliation is
    /// computed by `sync` (§4); this layer just persists the decided row.
    pub async fn upsert_issue(&self, issue: &Issue) -> Result<()> {
        let labels = serde_json::to_string(&issue.labels)?;
        sqlx::query(
            r#"INSERT INTO issues
                (key, summary, tracker_status, assignee, local_status,
                 last_pushed_status, diverged, url, labels, last_synced)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(key) DO UPDATE SET
                 summary = excluded.summary,
                 tracker_status = excluded.tracker_status,
                 assignee = excluded.assignee,
                 local_status = excluded.local_status,
                 last_pushed_status = excluded.last_pushed_status,
                 diverged = excluded.diverged,
                 url = excluded.url,
                 labels = excluded.labels,
                 last_synced = excluded.last_synced"#,
        )
        .bind(&issue.key)
        .bind(&issue.summary)
        .bind(&issue.tracker_status)
        .bind(&issue.assignee)
        .bind(&issue.local_status)
        .bind(&issue.last_pushed_status)
        .bind(i64::from(issue.diverged))
        .bind(&issue.url)
        .bind(labels)
        .bind(issue.last_synced.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record that Clabby pushed `status` to the tracker for `key`: local and
    /// last-pushed move to the new status and the divergence flag clears.
    pub async fn mark_issue_pushed(&self, key: &str, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE issues SET local_status = ?, last_pushed_status = ?, diverged = 0 WHERE key = ?",
        )
        .bind(status)
        .bind(status)
        .bind(key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---- sessions -----------------------------------------------------------

    pub async fn insert_session(&self, new: NewSession<'_>) -> Result<Session> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            r#"INSERT INTO sessions
                (issue_key, kind, pid, worktree_path, branch, agent, status,
                 log_path, exit_code, started_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
               RETURNING *"#,
        )
        .bind(new.issue_key)
        .bind(new.kind.as_str())
        .bind(new.pid)
        .bind(new.worktree_path)
        .bind(new.branch)
        .bind(new.agent)
        .bind(new.status.as_str())
        .bind(new.log_path)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        row_to_session(row)
    }

    pub async fn update_session_status(
        &self,
        id: i64,
        status: SessionStatus,
        exit_code: Option<i64>,
    ) -> Result<()> {
        sqlx::query("UPDATE sessions SET status = ?, exit_code = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(exit_code)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_session(&self, id: i64) -> Result<Option<Session>> {
        let row = sqlx::query("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(row_to_session).transpose()
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let rows = sqlx::query("SELECT * FROM sessions ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(row_to_session).collect()
    }

    pub async fn list_sessions_for_issue(&self, issue_key: &str) -> Result<Vec<Session>> {
        let rows = sqlx::query("SELECT * FROM sessions WHERE issue_key = ? ORDER BY id")
            .bind(issue_key)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(row_to_session).collect()
    }

    // ---- session logs -------------------------------------------------------

    pub async fn insert_log(
        &self,
        session_id: i64,
        stream: &str,
        line: &str,
        ts: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO session_logs (session_id, stream, line, ts) VALUES (?, ?, ?, ?)")
            .bind(session_id)
            .bind(stream)
            .bind(line)
            .bind(ts.to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Most recent `limit` log lines for a session, in chronological order.
    pub async fn tail_logs(
        &self,
        session_id: i64,
        limit: i64,
    ) -> Result<Vec<(String, String, DateTime<Utc>)>> {
        let rows = sqlx::query(
            "SELECT stream, line, ts FROM session_logs WHERE session_id = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut out: Vec<(String, String, DateTime<Utc>)> = rows
            .into_iter()
            .map(|r| {
                let stream: String = r.try_get("stream")?;
                let line: String = r.try_get("line")?;
                let ts: String = r.try_get("ts")?;
                Ok::<_, Error>((stream, line, parse_ts(&ts)))
            })
            .collect::<Result<_>>()?;
        out.reverse();
        Ok(out)
    }

    // ---- worktrees ----------------------------------------------------------

    pub async fn upsert_worktree(&self, wt: &Worktree) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO worktrees (path, issue_key, branch, created_at)
               VALUES (?, ?, ?, ?)
               ON CONFLICT(path) DO UPDATE SET
                 issue_key = excluded.issue_key,
                 branch = excluded.branch"#,
        )
        .bind(&wt.path)
        .bind(&wt.issue_key)
        .bind(&wt.branch)
        .bind(wt.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        let rows = sqlx::query("SELECT * FROM worktrees ORDER BY path")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(row_to_worktree).collect()
    }

    pub async fn worktree_for_issue(&self, issue_key: &str) -> Result<Option<Worktree>> {
        let row = sqlx::query(
            "SELECT * FROM worktrees WHERE issue_key = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(issue_key)
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_worktree).transpose()
    }

    // ---- logs of cross-cutting activity ------------------------------------

    pub async fn insert_sync_log(
        &self,
        fetched: i64,
        diverged: i64,
        detail: Option<&str>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO sync_log (ts, fetched, diverged, detail) VALUES (?, ?, ?, ?)")
            .bind(Utc::now().to_rfc3339())
            .bind(fetched)
            .bind(diverged)
            .bind(detail)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_cron_run(
        &self,
        action: &str,
        ok: bool,
        detail: Option<&str>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO cron_runs (action, ts, ok, detail) VALUES (?, ?, ?, ?)")
            .bind(action)
            .bind(Utc::now().to_rfc3339())
            .bind(i64::from(ok))
            .bind(detail)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_cron_runs(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as n FROM cron_runs")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("n")?)
    }

    // ---- transitions / steps / audit (Milestone 2) -------------------------

    pub async fn insert_transition_run(
        &self,
        issue_key: &str,
        from_state: &str,
        to_state: &str,
        status: TransitionStatus,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            r#"INSERT INTO transition_runs
                (issue_key, from_state, to_state, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?) RETURNING id"#,
        )
        .bind(issue_key)
        .bind(from_state)
        .bind(to_state)
        .bind(status.as_str())
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get::<i64, _>("id")?)
    }

    pub async fn update_transition_status(&self, id: i64, status: TransitionStatus) -> Result<()> {
        sqlx::query("UPDATE transition_runs SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_transition_run(&self, id: i64) -> Result<Option<TransitionRun>> {
        let row = sqlx::query("SELECT * FROM transition_runs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(row_to_transition_run).transpose()
    }

    /// The most recent blocked transition for an issue (the override target).
    pub async fn latest_blocked_transition(
        &self,
        issue_key: &str,
    ) -> Result<Option<TransitionRun>> {
        let row = sqlx::query(
            "SELECT * FROM transition_runs WHERE issue_key = ? AND status = 'blocked' ORDER BY id DESC LIMIT 1",
        )
        .bind(issue_key)
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_transition_run).transpose()
    }

    pub async fn insert_step_run(&self, s: NewStepRun<'_>) -> Result<i64> {
        let row = sqlx::query(
            r#"INSERT INTO step_runs
                (transition_run_id, step_id, step_index, required, status, exit_code, stderr, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"#,
        )
        .bind(s.transition_run_id)
        .bind(s.step_id)
        .bind(s.step_index)
        .bind(i64::from(s.required))
        .bind(s.status.as_str())
        .bind(s.exit_code)
        .bind(s.stderr)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get::<i64, _>("id")?)
    }

    pub async fn update_step_status(&self, id: i64, status: StepStatus) -> Result<()> {
        sqlx::query("UPDATE step_runs SET status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The failed step that is blocking a transition (the one an override resumes).
    pub async fn blocking_step(&self, transition_run_id: i64) -> Result<Option<StepRun>> {
        let row = sqlx::query(
            "SELECT * FROM step_runs WHERE transition_run_id = ? AND status = 'failed' ORDER BY step_index DESC LIMIT 1",
        )
        .bind(transition_run_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_step_run).transpose()
    }

    pub async fn insert_audit(
        &self,
        kind: &str,
        issue_key: Option<&str>,
        reason: Option<&str>,
        detail: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO audit_log (ts, kind, issue_key, reason, detail) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(kind)
        .bind(issue_key)
        .bind(reason)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_audit(&self, kind: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as n FROM audit_log WHERE kind = ?")
            .bind(kind)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("n")?)
    }
}

// ---- row -> model mappers ---------------------------------------------------

fn row_to_issue(row: SqliteRow) -> Result<Issue> {
    let labels_json: String = row.try_get("labels")?;
    let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();
    let last_synced: Option<String> = row.try_get("last_synced")?;
    let diverged: i64 = row.try_get("diverged")?;
    Ok(Issue {
        key: row.try_get("key")?,
        summary: row.try_get("summary")?,
        tracker_status: row.try_get("tracker_status")?,
        assignee: row.try_get("assignee")?,
        local_status: row.try_get("local_status")?,
        last_pushed_status: row.try_get("last_pushed_status")?,
        diverged: diverged != 0,
        url: row.try_get("url")?,
        labels,
        last_synced: parse_ts_opt(last_synced),
    })
}

fn row_to_session(row: SqliteRow) -> Result<Session> {
    let kind: String = row.try_get("kind")?;
    let status: String = row.try_get("status")?;
    let started_at: String = row.try_get("started_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    Ok(Session {
        id: row.try_get("id")?,
        issue_key: row.try_get("issue_key")?,
        kind: kind.parse::<SessionKind>()?,
        pid: row.try_get("pid")?,
        worktree_path: row.try_get("worktree_path")?,
        branch: row.try_get("branch")?,
        agent: row.try_get("agent")?,
        status: status.parse::<SessionStatus>()?,
        log_path: row.try_get("log_path")?,
        exit_code: row.try_get("exit_code")?,
        started_at: parse_ts(&started_at),
        updated_at: parse_ts(&updated_at),
    })
}

fn row_to_worktree(row: SqliteRow) -> Result<Worktree> {
    let created_at: String = row.try_get("created_at")?;
    Ok(Worktree {
        path: row.try_get("path")?,
        issue_key: row.try_get("issue_key")?,
        branch: row.try_get("branch")?,
        created_at: parse_ts(&created_at),
    })
}

fn row_to_transition_run(row: SqliteRow) -> Result<TransitionRun> {
    let status: String = row.try_get("status")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    Ok(TransitionRun {
        id: row.try_get("id")?,
        issue_key: row.try_get("issue_key")?,
        from_state: row.try_get("from_state")?,
        to_state: row.try_get("to_state")?,
        status: status.parse::<TransitionStatus>()?,
        created_at: parse_ts(&created_at),
        updated_at: parse_ts(&updated_at),
    })
}

fn row_to_step_run(row: SqliteRow) -> Result<StepRun> {
    let status: String = row.try_get("status")?;
    let required: i64 = row.try_get("required")?;
    let created_at: String = row.try_get("created_at")?;
    Ok(StepRun {
        id: row.try_get("id")?,
        transition_run_id: row.try_get("transition_run_id")?,
        step_id: row.try_get("step_id")?,
        step_index: row.try_get("step_index")?,
        required: required != 0,
        status: status.parse::<StepStatus>()?,
        exit_code: row.try_get("exit_code")?,
        stderr: row.try_get("stderr")?,
        created_at: parse_ts(&created_at),
    })
}
