//! Integration tests for the git/worktree helpers (§3 overview state).
//!
//! These run real `git` against a temp repo (no network), asserting on the
//! *parsed output* of `list_worktrees` and the derived `git_state` — the values
//! that feed the overview. Without asserting on the returned data, any stubbed
//! return survives mutation.

use std::path::Path;

use clabby_core::git;
use clabby_core::runner::run_capture;

async fn init_repo(root: &Path) {
    run_capture("git init -b main", Some(root)).await.unwrap();
    run_capture("git config user.email test@clabby.local", Some(root))
        .await
        .unwrap();
    run_capture("git config user.name clabby-test", Some(root))
        .await
        .unwrap();
    std::fs::write(root.join("README.md"), "seed\n").unwrap();
    run_capture("git add -A", Some(root)).await.unwrap();
    run_capture("git commit -m seed-commit", Some(root))
        .await
        .unwrap();
}

#[tokio::test]
async fn list_worktrees_reports_added_worktree_and_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init_repo(root).await;

    let wt_path = root.join("wt-PROJ-1");
    git::add_worktree(root, &wt_path, Some("feature/PROJ-1"), true, None)
        .await
        .unwrap();

    let worktrees = git::list_worktrees(root).await.unwrap();

    // The main checkout plus the one we added.
    assert_eq!(worktrees.len(), 2, "got: {worktrees:?}");

    // The added worktree is present with its branch parsed (refs/heads/ stripped).
    let added = worktrees
        .iter()
        .find(|(p, _)| p.contains("wt-PROJ-1"))
        .expect("added worktree should be listed");
    assert_eq!(added.1.as_deref(), Some("feature/PROJ-1"));

    // The main worktree is on `main`.
    let main = worktrees
        .iter()
        .find(|(_, b)| b.as_deref() == Some("main"))
        .expect("main worktree should be on `main`");
    assert!(!main.0.is_empty());
}

#[tokio::test]
async fn git_state_reports_branch_clean_and_last_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init_repo(root).await;

    let st = git::git_state(root).await.unwrap();
    assert_eq!(st.branch.as_deref(), Some("main"));
    assert!(!st.dirty, "a freshly committed repo is clean");
    assert_eq!(st.last_commit_summary.as_deref(), Some("seed-commit"));
}

#[tokio::test]
async fn git_state_counts_commits_ahead_of_upstream() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init_repo(root).await;

    // A bare repo to act as `origin`, with an upstream-tracking `main`.
    let origin = tempfile::tempdir().unwrap();
    let origin_url = origin.path().display().to_string().replace('\\', "/");
    run_capture("git init --bare -b main", Some(origin.path()))
        .await
        .unwrap();
    run_capture(
        &format!("git remote add origin \"{origin_url}\""),
        Some(root),
    )
    .await
    .unwrap();
    run_capture("git push -u origin main", Some(root))
        .await
        .unwrap();

    // At the upstream tip: neither ahead nor behind.
    let st = git::git_state(root).await.unwrap();
    assert_eq!(st.ahead, 0);
    assert_eq!(st.behind, 0);

    // One unpushed local commit -> ahead by exactly one.
    std::fs::write(root.join("next.txt"), "more\n").unwrap();
    run_capture("git add -A", Some(root)).await.unwrap();
    run_capture("git commit -m second", Some(root))
        .await
        .unwrap();

    let st = git::git_state(root).await.unwrap();
    assert_eq!(st.ahead, 1, "one unpushed commit means ahead = 1");
    assert_eq!(st.behind, 0);
}

#[tokio::test]
async fn git_state_flags_a_dirty_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init_repo(root).await;

    // An untracked file makes `status --porcelain` non-empty.
    std::fs::write(root.join("scratch.txt"), "uncommitted\n").unwrap();
    let st = git::git_state(root).await.unwrap();
    assert!(st.dirty, "an untracked file should mark the worktree dirty");
}
