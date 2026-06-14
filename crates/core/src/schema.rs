//! Diesel table definitions — the compile-time-checked source of truth for the
//! SQL layer (#20). Every query in `db.rs` is checked against these `table!`
//! blocks at build time, so a column rename/retype that a query misses is a
//! **compile error**, not a runtime one.
//!
//! Hand-maintained to mirror `migrations/`. After changing a migration, keep this
//! in sync (canonically `diesel print-schema` against a migrated DB). The
//! `migrations_apply` integration test guards that the migrations themselves
//! apply cleanly. Mapping conventions (matching the storage format, §8):
//! integer ids/counts are `BigInt` (→ `i64`); the 0/1 flag columns `diverged`,
//! `ok`, `required` are `Bool`; RFC3339 timestamps and JSON `labels` are `Text`
//! and converted to domain types in `db.rs`.

diesel::table! {
    issues (key) {
        key -> Text,
        summary -> Text,
        tracker_status -> Text,
        assignee -> Nullable<Text>,
        local_status -> Text,
        last_pushed_status -> Nullable<Text>,
        diverged -> Bool,
        url -> Nullable<Text>,
        labels -> Text,
        last_synced -> Nullable<Text>,
    }
}

diesel::table! {
    sessions (id) {
        id -> BigInt,
        issue_key -> Text,
        kind -> Text,
        pid -> Nullable<BigInt>,
        worktree_path -> Nullable<Text>,
        branch -> Nullable<Text>,
        agent -> Nullable<Text>,
        status -> Text,
        log_path -> Nullable<Text>,
        exit_code -> Nullable<BigInt>,
        started_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    worktrees (path) {
        path -> Text,
        issue_key -> Nullable<Text>,
        branch -> Nullable<Text>,
        created_at -> Text,
    }
}

diesel::table! {
    session_logs (id) {
        id -> BigInt,
        session_id -> BigInt,
        stream -> Text,
        line -> Text,
        ts -> Text,
    }
}

diesel::table! {
    sync_log (id) {
        id -> BigInt,
        ts -> Text,
        fetched -> BigInt,
        diverged -> BigInt,
        detail -> Nullable<Text>,
    }
}

diesel::table! {
    cron_runs (id) {
        id -> BigInt,
        action -> Text,
        ts -> Text,
        ok -> Bool,
        detail -> Nullable<Text>,
    }
}

diesel::table! {
    audit_log (id) {
        id -> BigInt,
        ts -> Text,
        kind -> Text,
        issue_key -> Nullable<Text>,
        reason -> Nullable<Text>,
        detail -> Nullable<Text>,
    }
}

diesel::table! {
    transition_runs (id) {
        id -> BigInt,
        issue_key -> Text,
        from_state -> Text,
        to_state -> Text,
        status -> Text,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    step_runs (id) {
        id -> BigInt,
        transition_run_id -> BigInt,
        step_id -> Text,
        step_index -> BigInt,
        required -> Bool,
        status -> Text,
        exit_code -> Nullable<BigInt>,
        stderr -> Nullable<Text>,
        created_at -> Text,
    }
}
