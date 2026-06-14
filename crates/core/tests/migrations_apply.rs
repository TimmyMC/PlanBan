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

    // sync_log + cron_runs + audit_log
    db.insert_sync_log(3, 1, Some("ok")).await.unwrap();
    db.insert_cron_run("sync", true, None).await.unwrap();
    assert_eq!(db.count_cron_runs().await.unwrap(), 1);
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
