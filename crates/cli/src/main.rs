//! clabby — headless command-center CLI.
//!
//! A thin driver over `clabby-core` (§7). Everything it does is also doable by the
//! future GUI through the same engine functions. UX-first (§10): the default
//! command (`status`) is the overview.

#![warn(clippy::all)]
#![forbid(unsafe_code)]
// Enforce the no-panic-in-production rule (§ error handling); tests may unwrap freely.
#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]
#![warn(clippy::dbg_macro)]
// Pedantic/style noise stays opt-out so velocity isn't taxed if pedantic is enabled later.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]

mod render;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::broadcast::error::RecvError;

use clabby::{Cli, Command, CronCmd, IssueCmd, LogsCmd, SessionCmd, WorktreeCmd};
use clabby_core::config::Config;
use clabby_core::db::Db;
use clabby_core::engine::{self, TransitionOutcome};
use clabby_core::events::{Event, EventBus};
use clabby_core::{git, overview, session, sync};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let cli = Cli::parse();

    match cli.command {
        Command::Sync => {
            let (config, db) = load(&cli.config).await?;
            cmd_sync(&config, &db).await
        }
        Command::Status { watch } => {
            let (config, db) = load(&cli.config).await?;
            cmd_status(&config, &db, watch).await
        }
        Command::Issue(IssueCmd::SetStatus { key, status }) => {
            let (config, db) = load(&cli.config).await?;
            sync::push_status(&db, &config, &key, &status).await?;
            println!("Pushed {key} -> {status}");
            Ok(())
        }
        Command::Session(cmd) => {
            let (config, db) = load(&cli.config).await?;
            cmd_session(&config, &db, cmd).await
        }
        Command::Worktree(cmd) => {
            let (config, db) = load(&cli.config).await?;
            cmd_worktree(&config, &db, cmd).await
        }
        Command::Logs(LogsCmd::Tail { session_id, lines }) => {
            let (_config, db) = load(&cli.config).await?;
            let rows = db.tail_logs(session_id, lines).await?;
            if rows.is_empty() {
                println!("(no log lines for session {session_id})");
            }
            for (stream, line, ts) in rows {
                let marker = if stream == "stderr" { "!" } else { " " };
                println!("{} {marker} {line}", ts.format("%H:%M:%S"));
            }
            Ok(())
        }
        Command::Cron(CronCmd::Run { once }) => {
            let (config, db) = load(&cli.config).await?;
            cmd_cron_run(&config, &db, once).await
        }
        Command::Move { key, to } => {
            let (config, db) = load(&cli.config).await?;
            cmd_move(&config, &db, &key, &to).await
        }
        Command::Override { key, reason } => {
            let (config, db) = load(&cli.config).await?;
            cmd_override(&config, &db, &key, &reason).await
        }
    }
}

/// Subscribe to the event bus and print streamed agent log lines live (§7: core
/// only publishes). Returns a handle to await after dropping the bus.
fn spawn_log_printer(bus: &EventBus) -> tokio::task::JoinHandle<()> {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Event::SessionLog { stream, line, .. }) => {
                    if stream == "stderr" {
                        eprintln!("{line}");
                    } else {
                        println!("{line}");
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    })
}

fn report_outcome(o: &TransitionOutcome) {
    if o.completed {
        println!(
            "Moved {} : {} -> {}  (transition #{})",
            o.issue_key, o.from, o.to, o.transition_run_id
        );
    } else if let Some(b) = &o.blocked {
        println!("BLOCKED moving {} : {} -> {}", o.issue_key, o.from, o.to);
        println!("  required step '{}' failed: {}", b.step_id, b.error);
        println!(
            "  override with: clabby override {} --reason \"...\"",
            o.issue_key
        );
    }
}

async fn cmd_move(config: &Config, db: &Db, key: &str, to: &str) -> Result<()> {
    let bus = EventBus::new();
    let printer = spawn_log_printer(&bus);
    let outcome = engine::transition(db, config, &bus, key, to).await;
    drop(bus);
    let _ = printer.await;

    let outcome = outcome?;
    report_outcome(&outcome);
    if !outcome.completed {
        // Gate worked as designed, but signal "did not complete" to scripts.
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_override(config: &Config, db: &Db, key: &str, reason: &str) -> Result<()> {
    let bus = EventBus::new();
    let printer = spawn_log_printer(&bus);
    let outcome = engine::override_for_issue(db, config, &bus, key, reason).await;
    drop(bus);
    let _ = printer.await;

    let outcome = outcome?;
    println!("Override recorded for {key} (reason logged to audit).");
    report_outcome(&outcome);
    if !outcome.completed {
        std::process::exit(1);
    }
    Ok(())
}

/// Load config (explicit path or discovered) and open the database.
async fn load(cli_config: &Option<PathBuf>) -> Result<(Config, Db)> {
    let config = match cli_config {
        Some(p) => Config::load(p).with_context(|| format!("loading config {}", p.display()))?,
        None => Config::discover(std::env::current_dir()?)
            .context("discovering clabby.toml (use --config to point at one)")?,
    };
    let db = Db::connect(config.db_file())
        .await
        .context("opening database")?;
    Ok((config, db))
}

async fn cmd_sync(config: &Config, db: &Db) -> Result<()> {
    let bus = EventBus::new();
    let outcome = sync::sync(db, config, &bus).await?;
    println!(
        "Synced {} issue(s); {} diverged.",
        outcome.fetched, outcome.diverged
    );
    for k in &outcome.diverged_keys {
        println!("  ! {k} changed in the tracker since our last push");
    }
    Ok(())
}

async fn cmd_status(config: &Config, db: &Db, watch: bool) -> Result<()> {
    if !watch {
        let rows = overview::build(db).await?;
        print!("{}", render::overview_table(&rows));
        return Ok(());
    }

    use std::io::Write;
    loop {
        let rows = overview::build(db).await?;
        print!("\x1b[2J\x1b[H");
        println!(
            "clabby — {}   (watching, Ctrl-C to exit)\n",
            config.project.name
        );
        print!("{}", render::overview_table(&rows));
        std::io::stdout().flush().ok();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

async fn cmd_session(config: &Config, db: &Db, cmd: SessionCmd) -> Result<()> {
    match cmd {
        SessionCmd::Spawn { key, agent } => {
            let bus = EventBus::new();
            let printer = spawn_log_printer(&bus);
            let result = session::run_managed(db, &bus, config, &key, &agent).await;
            drop(bus); // closes the channel so the printer task ends
            let _ = printer.await;

            let s = result?;
            println!(
                "\nSession {} finished: {} (exit {})",
                s.id,
                s.status,
                s.exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
            Ok(())
        }
        SessionCmd::Attach {
            key,
            worktree,
            branch,
            log,
        } => {
            let s = session::attach(db, &key, &worktree, branch.as_deref(), log.as_deref()).await?;
            println!(
                "Attached external session {} to {key} (worktree {worktree})",
                s.id
            );
            Ok(())
        }
        SessionCmd::List => {
            let sessions = db.list_sessions().await?;
            if sessions.is_empty() {
                println!("(no sessions)");
            }
            for s in sessions {
                let line = format!(
                    "{:>4}  {:8}  {:8}  {:10}  {}",
                    s.id,
                    s.kind.as_str(),
                    s.status.as_str(),
                    s.issue_key,
                    s.worktree_path.unwrap_or_default()
                );
                println!("{}", line.trim_end());
            }
            Ok(())
        }
    }
}

async fn cmd_worktree(config: &Config, db: &Db, cmd: WorktreeCmd) -> Result<()> {
    match cmd {
        WorktreeCmd::Add {
            key,
            branch,
            base,
            path,
        } => {
            let path = match path {
                Some(p) => p,
                None => {
                    let base_dir = config
                        .project
                        .worktrees_dir
                        .clone()
                        .unwrap_or_else(|| "../clabby-worktrees".to_string());
                    config.resolve_path(&base_dir).join(&key)
                }
            };
            let branch_name = branch.unwrap_or_else(|| key.clone());
            let repo = config.repo_dir();
            let mut wt =
                git::add_worktree(&repo, &path, Some(&branch_name), true, base.as_deref()).await?;
            wt.issue_key = Some(key.clone());
            db.upsert_worktree(&wt).await?;
            println!(
                "Created worktree {} on branch {branch_name} for {key}",
                path.display()
            );
            Ok(())
        }
        WorktreeCmd::List => {
            let wts = db.list_worktrees().await?;
            if wts.is_empty() {
                println!("(no worktrees)");
            }
            for w in wts {
                println!(
                    "{:30}  {:20}  {}",
                    w.path,
                    w.branch.unwrap_or_default(),
                    w.issue_key.unwrap_or_default()
                );
            }
            Ok(())
        }
    }
}

async fn cmd_cron_run(config: &Config, db: &Db, once: bool) -> Result<()> {
    use tokio_cron_scheduler::{Job, JobScheduler};

    if config.cron.is_empty() {
        println!("No [[cron]] jobs configured in clabby.toml.");
        return Ok(());
    }

    let bus = EventBus::new();

    if once {
        for c in &config.cron {
            println!("Running '{}' once...", c.action);
            run_cron_action(db, config, &bus, &c.action).await;
        }
        let total = db.count_cron_runs().await?;
        println!("Done. {total} cron run(s) recorded in total.");
        return Ok(());
    }

    let sched = JobScheduler::new().await?;

    for c in &config.cron {
        let action = c.action.clone();
        let schedule = c.schedule.clone();
        let db = db.clone();
        let config = config.clone();
        let bus = bus.clone();

        let job = Job::new_async(schedule.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            let config = config.clone();
            let bus = bus.clone();
            let action = action.clone();
            Box::pin(async move {
                run_cron_action(&db, &config, &bus, &action).await;
            })
        })?;
        sched.add(job).await?;
        println!("Scheduled '{}' on '{}'", c.action, c.schedule);
    }

    sched.start().await?;
    println!("Scheduler running. Press Ctrl-C to stop.");
    tokio::signal::ctrl_c().await?;
    println!("Stopping scheduler.");
    Ok(())
}

/// Dispatch a cron action by name and record the run. Adding new actions here is
/// the only code change needed to extend the scheduler (§11).
async fn run_cron_action(db: &Db, config: &Config, bus: &EventBus, action: &str) {
    match action {
        "sync" => match sync::sync(db, config, bus).await {
            Ok(o) => {
                let detail = format!("fetched {} diverged {}", o.fetched, o.diverged);
                let _ = db.insert_cron_run("sync", true, Some(&detail)).await;
            }
            Err(e) => {
                let _ = db
                    .insert_cron_run("sync", false, Some(&e.to_string()))
                    .await;
            }
        },
        other => {
            let _ = db
                .insert_cron_run(other, false, Some("unknown cron action"))
                .await;
        }
    }
}
