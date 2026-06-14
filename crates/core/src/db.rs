//! SQLite persistence: schema migration + repositories.
//!
//! Timestamps are stored as RFC3339 text and booleans as 0/1 integers so the
//! on-disk format is predictable and inspectable (§8: easy to verify). Mapping to
//! domain types happens here; callers see only `model` types.
//!
//! **Compile-time-checked SQL (#20).** Queries go through Diesel's query builder,
//! checked at build time against [`crate::schema`] (the `table!` definitions) — a
//! column rename/retype that a query misses is a *compile* error, not a runtime
//! one. The schema is applied via versioned migrations in `migrations/`
//! (embedded at build time). Async is provided by `diesel-async`'s
//! `SyncConnectionWrapper`, which runs the synchronous SQLite driver on a blocking
//! pool — so these stay `async fn` with no live database needed at build time
//! (no offline cache to maintain; §8/§12 velocity preserved). After changing a
//! migration, keep `schema.rs` in sync (canonically `diesel print-schema`).

use std::path::Path;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::upsert::excluded;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::sync_connection_wrapper::SyncConnectionWrapper;
use diesel_async::RunQueryDsl;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use crate::model::{
    Issue, Session, SessionKind, SessionStatus, StepRun, StepStatus, TransitionRun,
    TransitionStatus, Worktree,
};
use crate::schema::{
    audit_log, cron_runs, issues, session_logs, sessions, step_runs, sync_log, transition_runs,
    worktrees,
};
use crate::{Error, Result};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// The async connection type: the sync SQLite driver wrapped so queries `.await`
/// (each runs on `spawn_blocking`).
type AsyncConn = SyncConnectionWrapper<SqliteConnection>;

#[derive(Clone)]
pub struct Db {
    pool: Pool<AsyncConn>,
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

// ---- Diesel row structs (DB-facing; primitive types, explicit nullability) ----
//
// Read structs derive `Queryable, Selectable` (with `check_for_backend` so a
// type/column mismatch fails the build). Tables with a non-autoincrement primary
// key (issues, worktrees) reuse one struct for read *and* insert; tables with an
// AUTOINCREMENT id use a separate `New*` insert struct without the id.

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = issues, check_for_backend(diesel::sqlite::Sqlite))]
struct IssueRow {
    key: String,
    summary: String,
    tracker_status: String,
    assignee: Option<String>,
    local_status: String,
    last_pushed_status: Option<String>,
    diverged: bool,
    url: Option<String>,
    labels: String,
    last_synced: Option<String>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = sessions, check_for_backend(diesel::sqlite::Sqlite))]
struct SessionRow {
    id: i64,
    issue_key: String,
    kind: String,
    pid: Option<i64>,
    worktree_path: Option<String>,
    branch: Option<String>,
    agent: Option<String>,
    status: String,
    log_path: Option<String>,
    exit_code: Option<i64>,
    started_at: String,
    updated_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = sessions)]
struct NewSessionRow {
    issue_key: String,
    kind: String,
    pid: Option<i64>,
    worktree_path: Option<String>,
    branch: Option<String>,
    agent: Option<String>,
    status: String,
    log_path: Option<String>,
    exit_code: Option<i64>,
    started_at: String,
    updated_at: String,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = worktrees, check_for_backend(diesel::sqlite::Sqlite))]
struct WorktreeRow {
    path: String,
    issue_key: Option<String>,
    branch: Option<String>,
    created_at: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = session_logs, check_for_backend(diesel::sqlite::Sqlite))]
struct LogRow {
    stream: String,
    line: String,
    ts: String,
}

#[derive(Insertable)]
#[diesel(table_name = session_logs)]
struct NewLogRow {
    session_id: i64,
    stream: String,
    line: String,
    ts: String,
}

#[derive(Insertable)]
#[diesel(table_name = sync_log)]
struct NewSyncLogRow {
    ts: String,
    fetched: i64,
    diverged: i64,
    detail: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = cron_runs)]
struct NewCronRow {
    action: String,
    ts: String,
    ok: bool,
    detail: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = audit_log)]
struct NewAuditRow {
    ts: String,
    kind: String,
    issue_key: Option<String>,
    reason: Option<String>,
    detail: Option<String>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = transition_runs, check_for_backend(diesel::sqlite::Sqlite))]
struct TransitionRow {
    id: i64,
    issue_key: String,
    from_state: String,
    to_state: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = transition_runs)]
struct NewTransitionRow {
    issue_key: String,
    from_state: String,
    to_state: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = step_runs, check_for_backend(diesel::sqlite::Sqlite))]
struct StepRunRow {
    id: i64,
    transition_run_id: i64,
    step_id: String,
    step_index: i64,
    required: bool,
    status: String,
    exit_code: Option<i64>,
    stderr: Option<String>,
    created_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = step_runs)]
struct NewStepRunRow {
    transition_run_id: i64,
    step_id: String,
    step_index: i64,
    required: bool,
    status: String,
    exit_code: Option<i64>,
    stderr: Option<String>,
    created_at: String,
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
        let url = path.to_string_lossy().into_owned();

        // Migrations run on a one-off *sync* connection (the harness is sync),
        // before the async pool is built — every caller gets the schema applied.
        run_migrations(&url)?;

        let manager = AsyncDieselConnectionManager::<AsyncConn>::new(url);
        let pool = Pool::builder(manager)
            .build()
            .map_err(|e| Error::other(format!("db pool build: {e}")))?;
        Ok(Db { pool })
    }

    async fn conn(&self) -> Result<Object<AsyncConn>> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::other(format!("db connection: {e}")))
    }

    // ---- issues -------------------------------------------------------------

    pub async fn get_issue(&self, key: &str) -> Result<Option<Issue>> {
        let mut conn = self.conn().await?;
        let row = issues::table
            .filter(issues::key.eq(key.to_owned()))
            .select(IssueRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        row.map(row_to_issue).transpose()
    }

    pub async fn list_issues(&self) -> Result<Vec<Issue>> {
        let mut conn = self.conn().await?;
        let rows = issues::table
            .order(issues::key.asc())
            .select(IssueRow::as_select())
            .load(&mut conn)
            .await?;
        rows.into_iter().map(row_to_issue).collect()
    }

    /// Insert or replace an issue wholesale. Divergence/status reconciliation is
    /// computed by `sync` (§4); this layer just persists the decided row.
    pub async fn upsert_issue(&self, issue: &Issue) -> Result<()> {
        let mut conn = self.conn().await?;
        let row = IssueRow {
            key: issue.key.clone(),
            summary: issue.summary.clone(),
            tracker_status: issue.tracker_status.clone(),
            assignee: issue.assignee.clone(),
            local_status: issue.local_status.clone(),
            last_pushed_status: issue.last_pushed_status.clone(),
            diverged: issue.diverged,
            url: issue.url.clone(),
            labels: serde_json::to_string(&issue.labels)?,
            last_synced: issue.last_synced.map(|d| d.to_rfc3339()),
        };
        diesel::insert_into(issues::table)
            .values(row)
            .on_conflict(issues::key)
            .do_update()
            .set((
                issues::summary.eq(excluded(issues::summary)),
                issues::tracker_status.eq(excluded(issues::tracker_status)),
                issues::assignee.eq(excluded(issues::assignee)),
                issues::local_status.eq(excluded(issues::local_status)),
                issues::last_pushed_status.eq(excluded(issues::last_pushed_status)),
                issues::diverged.eq(excluded(issues::diverged)),
                issues::url.eq(excluded(issues::url)),
                issues::labels.eq(excluded(issues::labels)),
                issues::last_synced.eq(excluded(issues::last_synced)),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    /// Record that Clabby pushed `status` to the tracker for `key`: local and
    /// last-pushed move to the new status and the divergence flag clears.
    pub async fn mark_issue_pushed(&self, key: &str, status: &str) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::update(issues::table.filter(issues::key.eq(key.to_owned())))
            .set((
                issues::local_status.eq(status.to_owned()),
                issues::last_pushed_status.eq(status.to_owned()),
                issues::diverged.eq(false),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    // ---- sessions -----------------------------------------------------------

    pub async fn insert_session(&self, new: NewSession<'_>) -> Result<Session> {
        let mut conn = self.conn().await?;
        let now = Utc::now().to_rfc3339();
        let row = NewSessionRow {
            issue_key: new.issue_key.to_owned(),
            kind: new.kind.as_str().to_owned(),
            pid: new.pid,
            worktree_path: new.worktree_path.map(str::to_owned),
            branch: new.branch.map(str::to_owned),
            agent: new.agent.map(str::to_owned),
            status: new.status.as_str().to_owned(),
            log_path: new.log_path.map(str::to_owned),
            exit_code: None,
            started_at: now.clone(),
            updated_at: now,
        };
        let out = diesel::insert_into(sessions::table)
            .values(row)
            .returning(SessionRow::as_returning())
            .get_result(&mut conn)
            .await?;
        row_to_session(out)
    }

    pub async fn update_session_status(
        &self,
        id: i64,
        status: SessionStatus,
        exit_code: Option<i64>,
    ) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::update(sessions::table.filter(sessions::id.eq(id)))
            .set((
                sessions::status.eq(status.as_str().to_owned()),
                sessions::exit_code.eq(exit_code),
                sessions::updated_at.eq(Utc::now().to_rfc3339()),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_session(&self, id: i64) -> Result<Option<Session>> {
        let mut conn = self.conn().await?;
        let row = sessions::table
            .filter(sessions::id.eq(id))
            .select(SessionRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        row.map(row_to_session).transpose()
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut conn = self.conn().await?;
        let rows = sessions::table
            .order(sessions::id.asc())
            .select(SessionRow::as_select())
            .load(&mut conn)
            .await?;
        rows.into_iter().map(row_to_session).collect()
    }

    pub async fn list_sessions_for_issue(&self, issue_key: &str) -> Result<Vec<Session>> {
        let mut conn = self.conn().await?;
        let rows = sessions::table
            .filter(sessions::issue_key.eq(issue_key.to_owned()))
            .order(sessions::id.asc())
            .select(SessionRow::as_select())
            .load(&mut conn)
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
        let mut conn = self.conn().await?;
        diesel::insert_into(session_logs::table)
            .values(NewLogRow {
                session_id,
                stream: stream.to_owned(),
                line: line.to_owned(),
                ts: ts.to_rfc3339(),
            })
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    /// Most recent `limit` log lines for a session, in chronological order.
    pub async fn tail_logs(
        &self,
        session_id: i64,
        limit: i64,
    ) -> Result<Vec<(String, String, DateTime<Utc>)>> {
        let mut conn = self.conn().await?;
        let rows = session_logs::table
            .filter(session_logs::session_id.eq(session_id))
            .order(session_logs::id.desc())
            .limit(limit)
            .select(LogRow::as_select())
            .load(&mut conn)
            .await?;
        let mut out: Vec<(String, String, DateTime<Utc>)> = rows
            .into_iter()
            .map(|r| (r.stream, r.line, parse_ts(&r.ts)))
            .collect();
        out.reverse();
        Ok(out)
    }

    // ---- worktrees ----------------------------------------------------------

    pub async fn upsert_worktree(&self, wt: &Worktree) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::insert_into(worktrees::table)
            .values(WorktreeRow {
                path: wt.path.clone(),
                issue_key: wt.issue_key.clone(),
                branch: wt.branch.clone(),
                created_at: wt.created_at.to_rfc3339(),
            })
            .on_conflict(worktrees::path)
            .do_update()
            .set((
                worktrees::issue_key.eq(excluded(worktrees::issue_key)),
                worktrees::branch.eq(excluded(worktrees::branch)),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        let mut conn = self.conn().await?;
        let rows = worktrees::table
            .order(worktrees::path.asc())
            .select(WorktreeRow::as_select())
            .load(&mut conn)
            .await?;
        Ok(rows.into_iter().map(row_to_worktree).collect())
    }

    pub async fn worktree_for_issue(&self, issue_key: &str) -> Result<Option<Worktree>> {
        let mut conn = self.conn().await?;
        let row = worktrees::table
            .filter(worktrees::issue_key.eq(issue_key.to_owned()))
            .order(worktrees::created_at.desc())
            .limit(1)
            .select(WorktreeRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        Ok(row.map(row_to_worktree))
    }

    // ---- logs of cross-cutting activity ------------------------------------

    pub async fn insert_sync_log(
        &self,
        fetched: i64,
        diverged: i64,
        detail: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::insert_into(sync_log::table)
            .values(NewSyncLogRow {
                ts: Utc::now().to_rfc3339(),
                fetched,
                diverged,
                detail: detail.map(str::to_owned),
            })
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn insert_cron_run(
        &self,
        action: &str,
        ok: bool,
        detail: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::insert_into(cron_runs::table)
            .values(NewCronRow {
                action: action.to_owned(),
                ts: Utc::now().to_rfc3339(),
                ok,
                detail: detail.map(str::to_owned),
            })
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn count_cron_runs(&self) -> Result<i64> {
        let mut conn = self.conn().await?;
        let n = cron_runs::table.count().get_result(&mut conn).await?;
        Ok(n)
    }

    // ---- transitions / steps / audit (Milestone 2) -------------------------

    pub async fn insert_transition_run(
        &self,
        issue_key: &str,
        from_state: &str,
        to_state: &str,
        status: TransitionStatus,
    ) -> Result<i64> {
        let mut conn = self.conn().await?;
        let now = Utc::now().to_rfc3339();
        let id = diesel::insert_into(transition_runs::table)
            .values(NewTransitionRow {
                issue_key: issue_key.to_owned(),
                from_state: from_state.to_owned(),
                to_state: to_state.to_owned(),
                status: status.as_str().to_owned(),
                created_at: now.clone(),
                updated_at: now,
            })
            .returning(transition_runs::id)
            .get_result(&mut conn)
            .await?;
        Ok(id)
    }

    pub async fn update_transition_status(&self, id: i64, status: TransitionStatus) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::update(transition_runs::table.filter(transition_runs::id.eq(id)))
            .set((
                transition_runs::status.eq(status.as_str().to_owned()),
                transition_runs::updated_at.eq(Utc::now().to_rfc3339()),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_transition_run(&self, id: i64) -> Result<Option<TransitionRun>> {
        let mut conn = self.conn().await?;
        let row = transition_runs::table
            .filter(transition_runs::id.eq(id))
            .select(TransitionRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        row.map(row_to_transition_run).transpose()
    }

    /// The most recent blocked transition for an issue (the override target).
    pub async fn latest_blocked_transition(
        &self,
        issue_key: &str,
    ) -> Result<Option<TransitionRun>> {
        let mut conn = self.conn().await?;
        let row = transition_runs::table
            .filter(transition_runs::issue_key.eq(issue_key.to_owned()))
            .filter(transition_runs::status.eq("blocked".to_owned()))
            .order(transition_runs::id.desc())
            .limit(1)
            .select(TransitionRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        row.map(row_to_transition_run).transpose()
    }

    pub async fn insert_step_run(&self, s: NewStepRun<'_>) -> Result<i64> {
        let mut conn = self.conn().await?;
        let id = diesel::insert_into(step_runs::table)
            .values(NewStepRunRow {
                transition_run_id: s.transition_run_id,
                step_id: s.step_id.to_owned(),
                step_index: s.step_index,
                required: s.required,
                status: s.status.as_str().to_owned(),
                exit_code: s.exit_code,
                stderr: s.stderr.map(str::to_owned),
                created_at: Utc::now().to_rfc3339(),
            })
            .returning(step_runs::id)
            .get_result(&mut conn)
            .await?;
        Ok(id)
    }

    pub async fn update_step_status(&self, id: i64, status: StepStatus) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::update(step_runs::table.filter(step_runs::id.eq(id)))
            .set(step_runs::status.eq(status.as_str().to_owned()))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    /// The failed step that is blocking a transition (the one an override resumes).
    pub async fn blocking_step(&self, transition_run_id: i64) -> Result<Option<StepRun>> {
        let mut conn = self.conn().await?;
        let row = step_runs::table
            .filter(step_runs::transition_run_id.eq(transition_run_id))
            .filter(step_runs::status.eq("failed".to_owned()))
            .order(step_runs::step_index.desc())
            .limit(1)
            .select(StepRunRow::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        row.map(row_to_step_run).transpose()
    }

    pub async fn insert_audit(
        &self,
        kind: &str,
        issue_key: Option<&str>,
        reason: Option<&str>,
        detail: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn().await?;
        diesel::insert_into(audit_log::table)
            .values(NewAuditRow {
                ts: Utc::now().to_rfc3339(),
                kind: kind.to_owned(),
                issue_key: issue_key.map(str::to_owned),
                reason: reason.map(str::to_owned),
                detail: detail.map(str::to_owned),
            })
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn count_audit(&self, kind: &str) -> Result<i64> {
        let mut conn = self.conn().await?;
        let n = audit_log::table
            .filter(audit_log::kind.eq(kind.to_owned()))
            .count()
            .get_result(&mut conn)
            .await?;
        Ok(n)
    }
}

/// Run pending migrations on a one-off synchronous connection.
fn run_migrations(url: &str) -> Result<()> {
    let mut conn = SqliteConnection::establish(url).map_err(|e| Error::Migration(e.to_string()))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| Error::Migration(e.to_string()))?;
    Ok(())
}

// ---- row -> model mappers ---------------------------------------------------

fn row_to_issue(r: IssueRow) -> Result<Issue> {
    let labels: Vec<String> = serde_json::from_str(&r.labels).unwrap_or_default();
    Ok(Issue {
        key: r.key,
        summary: r.summary,
        tracker_status: r.tracker_status,
        assignee: r.assignee,
        local_status: r.local_status,
        last_pushed_status: r.last_pushed_status,
        diverged: r.diverged,
        url: r.url,
        labels,
        last_synced: parse_ts_opt(r.last_synced),
    })
}

fn row_to_session(r: SessionRow) -> Result<Session> {
    Ok(Session {
        id: r.id,
        issue_key: r.issue_key,
        kind: r.kind.parse::<SessionKind>()?,
        pid: r.pid,
        worktree_path: r.worktree_path,
        branch: r.branch,
        agent: r.agent,
        status: r.status.parse::<SessionStatus>()?,
        log_path: r.log_path,
        exit_code: r.exit_code,
        started_at: parse_ts(&r.started_at),
        updated_at: parse_ts(&r.updated_at),
    })
}

fn row_to_worktree(r: WorktreeRow) -> Worktree {
    Worktree {
        path: r.path,
        issue_key: r.issue_key,
        branch: r.branch,
        created_at: parse_ts(&r.created_at),
    }
}

fn row_to_transition_run(r: TransitionRow) -> Result<TransitionRun> {
    Ok(TransitionRun {
        id: r.id,
        issue_key: r.issue_key,
        from_state: r.from_state,
        to_state: r.to_state,
        status: r.status.parse::<TransitionStatus>()?,
        created_at: parse_ts(&r.created_at),
        updated_at: parse_ts(&r.updated_at),
    })
}

fn row_to_step_run(r: StepRunRow) -> Result<StepRun> {
    Ok(StepRun {
        id: r.id,
        transition_run_id: r.transition_run_id,
        step_id: r.step_id,
        step_index: r.step_index,
        required: r.required,
        status: r.status.parse::<StepStatus>()?,
        exit_code: r.exit_code,
        stderr: r.stderr,
        created_at: parse_ts(&r.created_at),
    })
}
