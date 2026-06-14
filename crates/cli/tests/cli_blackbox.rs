//! Black-box functional tests (Constitution §7, §9).
//!
//! Each test spawns the compiled binary and inspects only the public contract —
//! exit code, stdout, stderr. No knowledge of the internals; the engine could be
//! rewritten and these would still hold. `assert_fs` gives every test an isolated
//! temp directory so they run in parallel without clobbering each other, and the
//! committed fixtures are never mutated.
//!
//! Requires `node` and `git` on PATH (the offline fake tracker is a node script).

use std::path::Path;

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

fn clabby() -> Command {
    Command::cargo_bin("clabby").unwrap()
}

/// A sandbox seeded with the offline tracker fixtures (clabby.toml, tracker.mjs,
/// issues.json). The originals under tests/fixtures/ are copied, never touched.
fn project() -> assert_fs::TempDir {
    let tmp = assert_fs::TempDir::new().unwrap();
    tmp.copy_from("tests/fixtures", &["*"]).unwrap();
    tmp
}

// ---- error / edge cases -----------------------------------------------------

#[test]
fn missing_config_is_a_helpful_error() {
    let tmp = assert_fs::TempDir::new().unwrap();
    clabby()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("clabby.toml"));
}

#[test]
fn unknown_subcommand_exits_with_usage_error() {
    // clap uses exit code 2 for argument/usage errors.
    clabby().arg("frobnicate").assert().failure().code(2);
}

#[test]
fn set_status_without_push_is_rejected() {
    let tmp = project();
    tmp.child("clabby.toml").write_str(CONFIG_NO_PUSH).unwrap();
    clabby()
        .current_dir(tmp.path())
        .args(["issue", "set-status", "PROJ-12", "Done"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tracker.push"));
}

#[test]
fn unknown_agent_is_rejected() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .args(["session", "spawn", "PROJ-12", "--agent", "does-not-exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does-not-exist"));
}

// ---- happy-path behaviors ---------------------------------------------------

#[test]
fn sync_then_status_succeeds() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Synced 3 issue(s); 0 diverged"));
    clabby()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("PROJ-12")
                .and(predicate::str::contains("PROJ-44"))
                .and(predicate::str::contains("0 diverged")),
        );
}

#[test]
fn our_push_reconciles_but_external_change_diverges() {
    let tmp = project();
    let issues = tmp.path().join("issues.json");

    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();

    // Our own transition, via Clabby (pushes back to the tracker).
    clabby()
        .current_dir(tmp.path())
        .args(["issue", "set-status", "PROJ-12", "In Review"])
        .assert()
        .success();

    // Someone else edits the tracker directly (external change to PROJ-44).
    let mut data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&issues).unwrap()).unwrap();
    data["issues"][2]["fields"]["status"]["name"] = serde_json::json!("Done");
    std::fs::write(&issues, serde_json::to_string_pretty(&data).unwrap()).unwrap();

    // Re-sync: only the external change is flagged (Constitution §4).
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 diverged").and(predicate::str::contains("PROJ-44")));

    clabby()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(
            // PROJ-44 flagged; exactly one issue diverged (PROJ-12 reconciled).
            predicate::str::contains("diverged(->Done)")
                .and(predicate::str::contains("1 diverged")),
        );
}

#[test]
fn managed_session_runs_and_is_recorded() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();

    clabby()
        .current_dir(tmp.path())
        .args(["session", "spawn", "PROJ-12", "--agent", "echo"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("ran agent for PROJ-12")
                .and(predicate::str::contains("exited (exit 0)")),
        );

    clabby()
        .current_dir(tmp.path())
        .args(["session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("managed").and(predicate::str::contains("exited")));
}

#[test]
fn cron_run_once_executes_actions() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .args(["cron", "run", "--once"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Running 'sync' once")
                .and(predicate::str::contains("cron run(s) recorded")),
        );
}

#[test]
fn worktree_add_then_list() {
    let tmp = project();
    init_git(tmp.path());
    clabby()
        .current_dir(tmp.path())
        .args([
            "worktree",
            "add",
            "PROJ-12", //
            "--branch",
            "feature/PROJ-12", //
            "--base",
            "HEAD", //
            "--path",
            "wt-custom",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feature/PROJ-12"));
    clabby()
        .current_dir(tmp.path())
        .args(["worktree", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PROJ-12"));
}

// ---- observe / inspect use cases (attach, logs tail, status --watch) --------

#[test]
fn external_session_attaches_and_lists() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();
    clabby()
        .current_dir(tmp.path())
        .args([
            "session",
            "attach",
            "PROJ-12", //
            "--worktree",
            "wt/PROJ-12", //
            "--branch",
            "feature/PROJ-12", //
            "--log",
            "session.log",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Attached external session")
                .and(predicate::str::contains("PROJ-12")),
        );
    clabby()
        .current_dir(tmp.path())
        .args(["session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("external").and(predicate::str::contains("PROJ-12")));
}

#[test]
fn logs_tail_shows_a_managed_sessions_output() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();
    clabby()
        .current_dir(tmp.path())
        .args(["session", "spawn", "PROJ-12", "--agent", "echo"])
        .assert()
        .success();
    // The first session in a fresh db has id 1; the echo agent logged a line.
    clabby()
        .current_dir(tmp.path())
        .args(["logs", "tail", "1", "--lines", "10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ran agent for PROJ-12"));
}

#[test]
fn status_watch_starts_and_keeps_running() {
    use std::process::{Command as PCommand, Stdio};
    use std::time::Duration;

    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();

    // `--watch` loops until interrupted, so drive it directly: it must still be
    // running after a moment (proves the watch loop started and didn't crash).
    let mut child = PCommand::new(assert_cmd::cargo::cargo_bin("clabby"))
        .current_dir(tmp.path())
        .args(["status", "--watch"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let still_running = child.try_wait().unwrap().is_none();
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        still_running,
        "status --watch should keep running until interrupted"
    );
}

// ---- M2: transitions (gate-with-override) -----------------------------------

#[test]
fn transition_with_passing_steps_completes() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();
    clabby()
        .current_dir(tmp.path())
        .args(["move", "PROJ-12", "In Review"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Moved PROJ-12"));
}

#[test]
fn illegal_transition_is_rejected() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();
    // No "In Progress" -> "Done" transition is configured.
    clabby()
        .current_dir(tmp.path())
        .args(["move", "PROJ-12", "Done"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no transition"));
}

#[test]
fn required_step_gates_then_override_resumes() {
    let tmp = project();
    clabby()
        .current_dir(tmp.path())
        .arg("sync")
        .assert()
        .success();

    // PROJ-31 starts "In Review"; the In Review -> Done transition has a failing
    // required step, so the move is gated (exit 1) and the issue stays put.
    clabby()
        .current_dir(tmp.path())
        .args(["move", "PROJ-31", "Done"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("BLOCKED").and(predicate::str::contains("gate")));

    clabby()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("PROJ-31").and(predicate::str::contains("In Review")));

    // Overriding with a reason records the audit entry and resumes to completion.
    clabby()
        .current_dir(tmp.path())
        .args([
            "override",
            "PROJ-31",
            "--reason",
            "tracker CLI offline; verified the move by hand",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Override recorded")
                .and(predicate::str::contains("Moved PROJ-31")),
        );

    clabby()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Done"));
}

// ---- helpers ----------------------------------------------------------------

fn init_git(dir: &Path) {
    let run = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.email", "t@example.com"]);
    run(&["config", "user.name", "clabby-test"]);
    std::fs::write(dir.join("seed.txt"), "seed\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "seed"]);
}

const CONFIG_NO_PUSH: &str = r#"
[project]
name = "t"
db_path = ".clabby/clabby.db"

[tracker]
fetch = "node tracker.mjs issues.json fetch"
jql = ""

[tracker.map]
items = "issues"
key = "key"
summary = "fields.summary"
status = "fields.status.name"
"#;
