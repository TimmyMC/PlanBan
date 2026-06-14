//! The `clabby` CLI command surface, defined once with `clap` so both the binary
//! (`main.rs`) and the use-case-coverage gate (`tests/use_case_coverage.rs`) share
//! a single source of truth. The gate walks `command()` to assert every leaf
//! command and flag is exercised by a test — see `docs/testing.md`.

#![warn(clippy::all)]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "clabby",
    version,
    about = "Local command center for agent work, synced to your issue tracker"
)]
pub struct Cli {
    /// Path to clabby.toml (default: discovered from the current directory upward).
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Pull issues from the tracker and reconcile them locally.
    Sync,
    /// Show the overview dashboard.
    Status {
        /// Re-render on a 2s interval until interrupted.
        #[arg(long)]
        watch: bool,
    },
    /// Issue operations.
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Session operations.
    #[command(subcommand)]
    Session(SessionCmd),
    /// Worktree operations.
    #[command(subcommand)]
    Worktree(WorktreeCmd),
    /// Session log inspection.
    #[command(subcommand)]
    Logs(LogsCmd),
    /// Cron scheduler for configured [[cron]] jobs.
    #[command(subcommand)]
    Cron(CronCmd),
    /// Move an issue to a new status, running the transition's steps (gated).
    Move { key: String, to: String },
    /// Override the blocked step of an issue's transition (records a reason).
    Override {
        key: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Subcommand)]
pub enum IssueCmd {
    /// Push a status change to the tracker (records it as ours, §4).
    SetStatus { key: String, status: String },
}

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Spawn a managed agent run for an issue and stream its output.
    Spawn {
        key: String,
        #[arg(long)]
        agent: String,
    },
    /// Register an externally-run interactive session for the overview.
    Attach {
        key: String,
        #[arg(long)]
        worktree: String,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        log: Option<String>,
    },
    /// List all known sessions.
    List,
}

#[derive(Subcommand)]
pub enum WorktreeCmd {
    /// Create a git worktree for an issue and bind it.
    Add {
        key: String,
        #[arg(long)]
        branch: Option<String>,
        /// Base commit/ref to branch from.
        #[arg(long)]
        base: Option<String>,
        /// Explicit worktree path (default: <worktrees_dir>/<key>).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// List recorded worktrees.
    List,
}

#[derive(Subcommand)]
pub enum LogsCmd {
    /// Print the most recent log lines for a session.
    Tail {
        session_id: i64,
        #[arg(long, default_value_t = 50)]
        lines: i64,
    },
}

#[derive(Subcommand)]
pub enum CronCmd {
    /// Run configured jobs. By default starts a scheduler until interrupted;
    /// `--once` runs every configured action a single time and exits.
    Run {
        #[arg(long)]
        once: bool,
    },
}

/// The fully-built clap command tree — the canonical CLI surface the coverage
/// gate introspects.
#[must_use]
pub fn command() -> clap::Command {
    Cli::command()
}

/// Every CLI "use-case key": each leaf command path (`"session attach"`) and each
/// of its flags (`"session attach --worktree"`). The coverage gate
/// (`tests/use_case_coverage.rs`) asserts each has a `docs/use-cases.md` row + a
/// real test, so a new command/flag can't ship untested. The global `--config`
/// and clap's `--help`/`--version` are excluded.
#[must_use]
pub fn use_case_keys() -> BTreeSet<String> {
    fn walk(cmd: &clap::Command, prefix: &str, out: &mut BTreeSet<String>) {
        let mut has_sub = false;
        for sub in cmd.get_subcommands() {
            has_sub = true;
            walk(sub, &format!("{prefix} {}", sub.get_name()), out);
        }
        if !has_sub {
            out.insert(prefix.trim().to_string());
            for arg in cmd.get_arguments() {
                if let Some(long) = arg.get_long() {
                    if !matches!(long, "help" | "version" | "config") {
                        out.insert(format!("{} --{long}", prefix.trim()));
                    }
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&command(), "", &mut out);
    out
}
