//! Integration tests for the deterministic workflow engine (Milestone 2).
//!
//! These drive `engine::transition` / `engine::override_for_issue` end-to-end
//! against a real temp SQLite DB and the real command-template runner, using
//! offline shell commands (no network, no agents). They pin the engine's load-
//! bearing behavior: a required step gates, a `when`-guard skips, an optional
//! step logs-and-continues, and an override resumes from the next step (§2).

use std::collections::HashMap;
use std::path::Path;

use clabby_core::config::{
    AgentConfig, Config, HooksConfig, ProjectConfig, StepConfig, TrackerConfig, TrackerMap,
    TransitionConfig,
};
use clabby_core::db::Db;
use clabby_core::events::EventBus;
use clabby_core::model::{Issue, StepStatus, TransitionStatus};
use clabby_core::{engine, Error};

/// A shell command that exits 0 on either platform.
fn ok_cmd() -> String {
    if cfg!(windows) {
        "cmd /c exit 0"
    } else {
        "true"
    }
    .to_string()
}

/// A shell command that exits non-zero on either platform.
fn fail_cmd() -> String {
    if cfg!(windows) {
        "cmd /c exit 1"
    } else {
        "false"
    }
    .to_string()
}

fn base_config(root: &Path, transitions: Vec<TransitionConfig>) -> Config {
    Config {
        project: ProjectConfig {
            name: "test".to_string(),
            db_path: "clabby.db".to_string(),
            worktrees_dir: None,
            repo_dir: None,
        },
        tracker: TrackerConfig {
            fetch: ok_cmd(),
            push: Some(ok_cmd()),
            jql: String::new(),
            map: TrackerMap {
                items: "issues".to_string(),
                key: "key".to_string(),
                summary: "summary".to_string(),
                status: "status".to_string(),
                assignee: None,
                url: None,
                labels: None,
            },
        },
        states: vec![],
        agents: HashMap::new(),
        cron: vec![],
        transitions,
        hooks: HooksConfig::default(),
        root_dir: root.to_path_buf(),
    }
}

fn step(id: &str, cmd: String, required: bool, when: Option<&str>) -> StepConfig {
    StepConfig {
        id: id.to_string(),
        cmd: Some(cmd),
        agent: None,
        required,
        when: when.map(str::to_string),
    }
}

/// A required step that runs the named `[agents.*]` entry as a managed session.
fn agent_step(id: &str, agent: &str) -> StepConfig {
    StepConfig {
        id: id.to_string(),
        cmd: None,
        agent: Some(agent.to_string()),
        required: true,
        when: None,
    }
}

fn agent(cmd: String) -> AgentConfig {
    AgentConfig {
        cmd,
        cwd: None,
        env: HashMap::new(),
    }
}

/// Seed an issue in `from` state and return a connected DB + bus.
async fn setup(config: &Config, from: &str) -> (Db, EventBus) {
    let db = Db::connect(config.db_file()).await.unwrap();
    db.upsert_issue(&Issue::new("PROJ-1", "Demo issue", from))
        .await
        .unwrap();
    (db, EventBus::new())
}

#[tokio::test]
async fn all_steps_pass_completes_the_transition() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![
                step("first", ok_cmd(), true, None),
                step("second", ok_cmd(), true, None),
            ],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();

    assert!(outcome.completed);
    assert!(outcome.blocked.is_none());
    // The completed move is recorded as ours so the next sync doesn't flag it (§4).
    let issue = db.get_issue("PROJ-1").await.unwrap().unwrap();
    assert_eq!(issue.local_status, "In Progress");
    assert_eq!(issue.last_pushed_status.as_deref(), Some("In Progress"));
    let tr = db
        .get_transition_run(outcome.transition_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tr.status, TransitionStatus::Completed);
}

#[tokio::test]
async fn failing_required_step_gates_the_transition() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![
                step("ok-before", ok_cmd(), true, None),
                step("gate", fail_cmd(), true, None),
                step("never", ok_cmd(), true, None),
            ],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();

    // Gate: the move did not complete, the blocking step is reported, and the
    // issue stays in its source state (it was never pushed).
    assert!(!outcome.completed);
    let blocked = outcome.blocked.expect("a failed required step must block");
    assert_eq!(blocked.step_id, "gate");
    assert!(
        blocked.step_run_id > 0,
        "blocked step must reference a real row"
    );

    let issue = db.get_issue("PROJ-1").await.unwrap().unwrap();
    assert_eq!(issue.local_status, "To Do");
    assert_eq!(issue.last_pushed_status, None);

    let tr = db
        .get_transition_run(outcome.transition_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tr.status, TransitionStatus::Blocked);

    // The blocking_step lookup returns the step we stopped on.
    let bs = db.blocking_step(tr.id).await.unwrap().unwrap();
    assert_eq!(bs.step_id, "gate");
    assert_eq!(bs.id, blocked.step_run_id);
    assert_eq!(bs.status, StepStatus::Failed);
}

#[tokio::test]
async fn failing_optional_step_logs_and_continues() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![
                step("optional-fail", fail_cmd(), false, None),
                step("after", ok_cmd(), true, None),
            ],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();

    // An optional step's failure is recorded but does not gate.
    assert!(outcome.completed);
    assert!(outcome.blocked.is_none());
    assert_eq!(
        db.get_issue("PROJ-1").await.unwrap().unwrap().local_status,
        "In Progress"
    );
}

#[tokio::test]
async fn when_false_skips_the_step() {
    let tmp = tempfile::tempdir().unwrap();
    // The guarded step would FAIL if it ran. Its `when` is false, so a correct
    // engine skips it and the transition completes. If the `!` guard were dropped
    // (inverting skip), the step would run, fail, and block — so completion here
    // is what kills that mutant.
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![step(
                "guarded",
                fail_cmd(),
                true,
                Some("from == 'never-matches'"),
            )],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(
        outcome.completed,
        "a false `when` must skip the failing step"
    );
    assert!(outcome.blocked.is_none());
}

#[tokio::test]
async fn when_true_runs_the_step() {
    let tmp = tempfile::tempdir().unwrap();
    // Mirror image: the guard is true, so the failing step runs and gates. This
    // pins the guard's truthy branch (a `when` forced to always-skip would let
    // this complete).
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![step("guarded", fail_cmd(), true, Some("from == 'To Do'"))],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(
        !outcome.completed,
        "a true `when` must run the failing step"
    );
    assert_eq!(outcome.blocked.unwrap().step_id, "guarded");
}

#[tokio::test]
async fn override_resumes_after_the_blocked_step() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![
                step("gate", fail_cmd(), true, None),
                step("downstream", ok_cmd(), true, None),
            ],
        }],
    );
    let (db, bus) = setup(&config, "To Do").await;

    // First the gate blocks.
    let blocked = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(!blocked.completed);

    // Override with a reason: the blocked step is marked overridden, the audit
    // log records it, and the engine resumes at the NEXT step (resume index =
    // blocked index + 1) — so it does not re-run the failing gate.
    let resumed = engine::override_for_issue(&db, &config, &bus, "PROJ-1", "manual sign-off")
        .await
        .unwrap();

    assert!(resumed.completed, "override should resume and complete");
    assert!(resumed.blocked.is_none());
    assert_eq!(resumed.transition_run_id, blocked.transition_run_id);

    let bs = db.blocking_step(resumed.transition_run_id).await.unwrap();
    assert!(
        bs.is_none(),
        "the overridden step should no longer count as a blocking failure"
    );
    // The override is audited (record_override -> audit_log).
    assert_eq!(db.count_audit("override").await.unwrap(), 1);

    let issue = db.get_issue("PROJ-1").await.unwrap().unwrap();
    assert_eq!(issue.local_status, "In Progress");
}

#[tokio::test]
async fn agent_step_that_exits_zero_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![agent_step("run-agent", "worker")],
        }],
    );
    // A managed agent that exits 0 -> session Exited -> the step passes.
    config.agents.insert("worker".to_string(), agent(ok_cmd()));
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(outcome.completed, "a zero-exit agent step should pass");
    assert!(outcome.blocked.is_none());
}

#[tokio::test]
async fn agent_step_that_exits_nonzero_gates() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![agent_step("run-agent", "worker")],
        }],
    );
    // A managed agent that exits non-zero -> session Failed -> the step gates.
    config
        .agents
        .insert("worker".to_string(), agent(fail_cmd()));
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(!outcome.completed, "a failing agent step must gate");
    assert_eq!(outcome.blocked.unwrap().step_id, "run-agent");
    assert_eq!(
        db.get_issue("PROJ-1").await.unwrap().unwrap().local_status,
        "To Do"
    );
}

#[tokio::test]
async fn post_transition_hook_runs_on_completion() {
    let tmp = tempfile::tempdir().unwrap();
    // A hook with a real side effect (writing a marker file) so we can assert it
    // actually ran — a no-op `run_hooks` would leave the marker absent.
    let marker = tmp.path().join("hook-marker.txt");
    let hook = if cfg!(windows) {
        format!("cmd /c echo ran> \"{}\"", marker.display())
    } else {
        format!("printf ran > \"{}\"", marker.display())
    };

    let mut config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![step("noop", ok_cmd(), true, None)],
        }],
    );
    config.hooks.post_transition = vec![hook];
    let (db, bus) = setup(&config, "To Do").await;

    let outcome = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap();
    assert!(outcome.completed);
    assert!(
        marker.exists(),
        "the post_transition hook should have created the marker file"
    );
}

#[tokio::test]
async fn override_without_a_blocked_transition_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(tmp.path(), vec![]);
    let (db, bus) = setup(&config, "To Do").await;

    let err = engine::override_for_issue(&db, &config, &bus, "PROJ-1", "nope")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Other(_)), "got: {err:?}");
}

#[tokio::test]
async fn transition_without_config_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(tmp.path(), vec![]); // no transitions defined
    let (db, bus) = setup(&config, "To Do").await;

    let err = engine::transition(&db, &config, &bus, "PROJ-1", "In Progress")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Other(_)), "got: {err:?}");
}

#[tokio::test]
async fn transition_for_unknown_issue_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let config = base_config(
        tmp.path(),
        vec![TransitionConfig {
            from: "To Do".to_string(),
            to: "In Progress".to_string(),
            steps: vec![],
        }],
    );
    let db = Db::connect(config.db_file()).await.unwrap();
    let bus = EventBus::new();
    // No issue seeded.
    let err = engine::transition(&db, &config, &bus, "GHOST-1", "In Progress")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "got: {err:?}");
}
