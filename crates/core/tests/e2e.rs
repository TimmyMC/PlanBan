//! End-to-end integration test exercising the real command-template pipeline
//! against offline fakes (§8, §9). The "tracker" is a JSON file read via a
//! platform read command; git is a real temp repo. No network, no credentials.

use std::collections::HashMap;
use std::path::Path;

use clabby_core::config::{Config, ProjectConfig, TrackerConfig, TrackerMap, AgentConfig};
use clabby_core::db::Db;
use clabby_core::events::EventBus;
use clabby_core::model::SessionStatus;
use clabby_core::runner::run_capture;
use clabby_core::{git, overview, session, sync};

/// A command that prints a file's contents, cross-platform.
fn read_file_cmd(path: &Path) -> String {
    if cfg!(windows) {
        format!("type \"{}\"", path.display())
    } else {
        format!("cat \"{}\"", path.display())
    }
}

fn jira_json(p1_status: &str, p2_status: &str) -> String {
    format!(
        r#"{{ "issues": [
            {{ "key": "PROJ-1", "self": "http://tracker/PROJ-1",
               "fields": {{ "summary": "First issue",
                           "status": {{ "name": "{p1}" }},
                           "assignee": {{ "displayName": "Me" }},
                           "labels": ["backend"] }} }},
            {{ "key": "PROJ-2", "self": "http://tracker/PROJ-2",
               "fields": {{ "summary": "Second issue",
                           "status": {{ "name": "{p2}" }},
                           "assignee": null,
                           "labels": [] }} }}
        ] }}"#,
        p1 = p1_status,
        p2 = p2_status
    )
}

fn make_config(root: &Path, issues_file: &Path) -> Config {
    let mut agents = HashMap::new();
    agents.insert(
        "echo".to_string(),
        AgentConfig {
            cmd: "echo agent-ran-for {{ issue_key }}".to_string(),
            cwd: None,
            env: HashMap::new(),
        },
    );

    Config {
        project: ProjectConfig {
            name: "test".to_string(),
            db_path: "clabby.db".to_string(),
            worktrees_dir: Some("wt".to_string()),
            repo_dir: None,
        },
        tracker: TrackerConfig {
            fetch: read_file_cmd(issues_file),
            // A no-op push that always succeeds (the fake tracker file is updated
            // by the test itself to simulate the tracker's new state).
            push: Some("cmd /c exit 0".to_string()),
            jql: String::new(),
            map: TrackerMap {
                items: "issues".to_string(),
                key: "key".to_string(),
                summary: "fields.summary".to_string(),
                status: "fields.status.name".to_string(),
                assignee: Some("fields.assignee.displayName".to_string()),
                url: Some("self".to_string()),
                labels: Some("fields.labels".to_string()),
            },
        },
        states: vec![],
        agents,
        cron: vec![],
        root_dir: root.to_path_buf(),
    }
}

#[tokio::test]
async fn full_pipeline() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let issues_file = root.join("issues.json");

    // --- sync: pull two issues from the fake tracker -----------------------
    std::fs::write(&issues_file, jira_json("To Do", "In Progress")).unwrap();
    let mut config = make_config(root, &issues_file);
    // push is a no-op; make it cross-platform.
    config.tracker.push = Some(if cfg!(windows) {
        "cmd /c exit 0".to_string()
    } else {
        "true".to_string()
    });

    let db = Db::connect(config.db_file()).await.unwrap();
    let bus = EventBus::new();

    let out = sync::sync(&db, &config, &bus).await.unwrap();
    assert_eq!(out.fetched, 2);
    assert_eq!(out.diverged, 0);

    let p1 = db.get_issue("PROJ-1").await.unwrap().unwrap();
    assert_eq!(p1.tracker_status, "To Do");
    assert_eq!(p1.local_status, "To Do");
    assert_eq!(p1.assignee.as_deref(), Some("Me"));
    assert_eq!(p1.labels, vec!["backend".to_string()]);
    assert!(!p1.diverged);

    // --- push: Clabby transitions PROJ-1, then the tracker reflects it ------
    sync::push_status(&db, &config, "PROJ-1", "In Review").await.unwrap();
    let p1 = db.get_issue("PROJ-1").await.unwrap().unwrap();
    assert_eq!(p1.local_status, "In Review");
    assert_eq!(p1.last_pushed_status.as_deref(), Some("In Review"));
    assert!(!p1.diverged);

    // Tracker now shows our pushed value: a re-sync must NOT flag divergence.
    std::fs::write(&issues_file, jira_json("In Review", "In Progress")).unwrap();
    let out = sync::sync(&db, &config, &bus).await.unwrap();
    assert_eq!(out.diverged, 0);
    assert!(!db.get_issue("PROJ-1").await.unwrap().unwrap().diverged);

    // --- divergence: someone else moves PROJ-2 to Done ----------------------
    std::fs::write(&issues_file, jira_json("In Review", "Done")).unwrap();
    let out = sync::sync(&db, &config, &bus).await.unwrap();
    assert_eq!(out.diverged, 1);
    let p2 = db.get_issue("PROJ-2").await.unwrap().unwrap();
    assert!(p2.diverged);
    assert_eq!(p2.local_status, "Done"); // tracker wins (§4)

    // --- git worktree + managed session ------------------------------------
    init_git_repo(root).await;

    let wt_path = root.join("wt").join("PROJ-1");
    let mut wt = git::add_worktree(root, &wt_path, Some("feature/PROJ-1"), true, None)
        .await
        .unwrap();
    wt.issue_key = Some("PROJ-1".to_string());
    db.upsert_worktree(&wt).await.unwrap();

    let s = session::run_managed(&db, &bus, &config, "PROJ-1", "echo")
        .await
        .unwrap();
    assert_eq!(s.status, SessionStatus::Exited);
    assert_eq!(s.exit_code, Some(0));

    let logs = db.tail_logs(s.id, 50).await.unwrap();
    assert!(
        logs.iter().any(|(_, line, _)| line.contains("agent-ran-for PROJ-1")),
        "expected agent output in logs, got: {logs:?}"
    );

    // --- overview aggregates everything ------------------------------------
    let rows = overview::build(&db).await.unwrap();
    assert_eq!(rows.len(), 2);
    let p1row = rows.iter().find(|r| r.issue.key == "PROJ-1").unwrap();
    assert!(p1row.git.is_some(), "PROJ-1 has a worktree, expected git state");
    assert_eq!(p1row.git.as_ref().unwrap().branch.as_deref(), Some("feature/PROJ-1"));
    assert!(!p1row.sessions.is_empty());
}

async fn init_git_repo(root: &Path) {
    // -b main needs git >= 2.28; fall back is not needed on modern installs.
    run_capture("git init -b main", Some(root)).await.unwrap();
    run_capture("git config user.email test@clabby.local", Some(root)).await.unwrap();
    run_capture("git config user.name clabby-test", Some(root)).await.unwrap();
    std::fs::write(root.join("README.md"), "seed\n").unwrap();
    run_capture("git add -A", Some(root)).await.unwrap();
    run_capture("git commit -m seed", Some(root)).await.unwrap();
}
