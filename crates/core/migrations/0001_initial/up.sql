-- Initial schema (Milestone 1 + 2). Lifted verbatim from the former inline
-- `db.rs` SCHEMA constant when db.rs moved to Diesel (#20). Timestamps are RFC3339
-- text and booleans 0/1 integers so the on-disk format stays inspectable (§8).
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
