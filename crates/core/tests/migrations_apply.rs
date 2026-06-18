//! Schema-drift guard (#20). Diesel makes a query that mismatches `schema.rs` a
//! *compile* error; the residual drift vector is the migrations themselves vs the
//! schema. This test connects to a fresh temp DB (which runs the embedded
//! migrations) and round-trips through **every** table via the public `Db` API —
//! so an incomplete or renamed migration fails here rather than in production.

use chrono::Utc;
use clabby_core::db::{Db, NewSession, NewStepRun};
use clabby_core::model::{
    Issue, SessionKind, SessionStatus, StepStatus, TransitionStatus, Worktree,
};

/// Concurrent writers must not fail with `SQLITE_BUSY`: SQLite has one writer, so
/// the pool's `busy_timeout` is what makes overlapping writes wait rather than
/// error (the regression a naive Diesel pool would reintroduce). Many tasks
/// stream logs at once here, like managed sessions do in production.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writes_wait_instead_of_erroring() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Db::connect(tmp.path().join("clabby.db")).await.unwrap();
    db.upsert_issue(&Issue::new("K-1", "s", "todo"))
        .await
        .unwrap();
    let session = db
        .insert_session(NewSession {
            issue_key: "K-1",
            kind: SessionKind::Managed,
            pid: None,
            worktree_path: None,
            branch: None,
            agent: None,
            status: SessionStatus::Running,
            log_path: None,
        })
        .await
        .unwrap();

    let mut handles = Vec::new();
    for i in 0..50 {
        let db = db.clone();
        let sid = session.id;
        handles.push(tokio::spawn(async move {
            db.insert_log(sid, "stdout", &format!("line {i}"), Utc::now())
                .await
        }));
    }
    for h in handles {
        h.await
            .unwrap()
            .expect("concurrent insert_log must not hit SQLITE_BUSY");
    }
    assert_eq!(db.tail_logs(session.id, 100).await.unwrap().len(), 50);
}

#[tokio::test]
async fn migrations_apply_and_every_table_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Db::connect(tmp.path().join("clabby.db")).await.unwrap();

    // issues
    db.upsert_issue(&Issue::new("K-1", "summary", "todo"))
        .await
        .unwrap();
    assert_eq!(db.list_issues().await.unwrap().len(), 1);
    db.mark_issue_pushed("K-1", "doing").await.unwrap();
    assert_eq!(
        db.get_issue("K-1").await.unwrap().unwrap().local_status,
        "doing"
    );
    // An optional timestamp must survive the round-trip (it's parsed back from
    // stored RFC3339 text): persist a `last_synced` and read it back as Some.
    let mut synced = Issue::new("K-2", "summary", "todo");
    synced.last_synced = Some(Utc::now());
    db.upsert_issue(&synced).await.unwrap();
    assert!(db
        .get_issue("K-2")
        .await
        .unwrap()
        .unwrap()
        .last_synced
        .is_some());

    // sessions + session_logs
    let session = db
        .insert_session(NewSession {
            issue_key: "K-1",
            kind: SessionKind::Managed,
            pid: Some(42),
            worktree_path: None,
            branch: None,
            agent: None,
            status: SessionStatus::Running,
            log_path: None,
        })
        .await
        .unwrap();
    db.insert_log(session.id, "stdout", "hello", Utc::now())
        .await
        .unwrap();
    assert_eq!(db.tail_logs(session.id, 10).await.unwrap().len(), 1);
    assert_eq!(db.list_sessions_for_issue("K-1").await.unwrap().len(), 1);
    // The global session list (CLI `sessions` command) must surface it too.
    assert_eq!(db.list_sessions().await.unwrap().len(), 1);
    db.update_session_status(session.id, SessionStatus::Exited, Some(0))
        .await
        .unwrap();

    // worktrees
    db.upsert_worktree(&Worktree {
        path: "/wt/K-1".into(),
        issue_key: Some("K-1".into()),
        branch: Some("feat/K-1".into()),
        created_at: Utc::now(),
    })
    .await
    .unwrap();
    assert!(db.worktree_for_issue("K-1").await.unwrap().is_some());
    assert_eq!(db.list_worktrees().await.unwrap().len(), 1);

    // sync_log + cron_runs + audit_log. Assert the counts move from 0 -> 1 so a
    // stubbed counter (always returning a constant) can't satisfy the assertion.
    db.insert_sync_log(3, 1, Some("ok")).await.unwrap();
    assert_eq!(db.count_cron_runs().await.unwrap(), 0);
    db.insert_cron_run("sync", true, None).await.unwrap();
    assert_eq!(db.count_cron_runs().await.unwrap(), 1);
    assert_eq!(db.count_audit("override").await.unwrap(), 0);
    db.insert_audit("override", Some("K-1"), Some("reason"), None)
        .await
        .unwrap();
    assert_eq!(db.count_audit("override").await.unwrap(), 1);

    // transition_runs + step_runs
    let tr = db
        .insert_transition_run("K-1", "todo", "doing", TransitionStatus::Running)
        .await
        .unwrap();
    assert!(db.get_transition_run(tr).await.unwrap().is_some());
    db.update_transition_status(tr, TransitionStatus::Blocked)
        .await
        .unwrap();
    assert!(db.latest_blocked_transition("K-1").await.unwrap().is_some());
    let step = db
        .insert_step_run(NewStepRun {
            transition_run_id: tr,
            step_id: "build",
            step_index: 0,
            required: true,
            status: StepStatus::Failed,
            exit_code: Some(1),
            stderr: Some("boom"),
        })
        .await
        .unwrap();
    db.update_step_status(step, StepStatus::Overridden)
        .await
        .unwrap();
    assert!(db.blocking_step(tr).await.unwrap().is_none()); // no longer 'failed'
}

/// `Db::connect` creates the database file's parent directory if it's missing —
/// a config can point `db_path` at a not-yet-existing `.clabby/` dir and connect
/// must still succeed (it can't open a SQLite file in a nonexistent directory).
#[tokio::test]
async fn connect_creates_missing_parent_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("does").join("not").join("exist");
    assert!(!nested.exists());
    let db = Db::connect(nested.join("clabby.db")).await.unwrap();
    // A working connection proves the directory was created and migrations ran.
    db.upsert_issue(&Issue::new("K-1", "s", "todo"))
        .await
        .unwrap();
    assert_eq!(db.list_issues().await.unwrap().len(), 1);
}
