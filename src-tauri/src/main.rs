//! Tauri v2 shell — thin driver over `clabby-core` (Constitution §7).
//!
//! Exposes four `invoke()` commands that mirror the `Api` interface in
//! `ui/src/api/index.ts`. A background task subscribes to `clabby-core`'s
//! `EventBus` and re-emits each event as a Tauri event (`"clabby://event"`),
//! giving the React board real-time push updates (Constitution §3, §8).
//!
//! The DB path and all config are read from `clabby.toml` discovered from the
//! working directory at startup — same discovery as the CLI (Constitution §7).
//! Run from a Clabby project directory: `cargo tauri dev` or `cargo run` from
//! `src-tauri/`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// The shell is a driver entry point; unwrap/expect on startup errors is fine here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use clabby_core::config::Config;
use clabby_core::db::Db;
use clabby_core::events::EventBus;
use clabby_core::model::{GitState, Session};
use clabby_core::overview::OverviewRow;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

// ---- Application state (managed by Tauri, shared across commands) -----------

struct AppState {
    db: Db,
    config: Config,
    bus: EventBus,
}

// ---- Data-transfer objects (Rust → TypeScript) ------------------------------
// Field names use `rename_all = "camelCase"` to match `ui/src/types.ts`.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionLiteDto {
    id: i64,
    kind: String,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitStateDto {
    branch: Option<String>,
    ahead: i64,
    behind: i64,
    dirty: bool,
    last_commit_summary: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowDto {
    key: String,
    summary: String,
    /// The status Clabby displays / the column the card sits in.
    status: String,
    /// Status last seen in the tracker (for the divergence badge).
    tracker_status: String,
    diverged: bool,
    sessions: Vec<SessionLiteDto>,
    worktree_path: Option<String>,
    git: Option<GitStateDto>,
}

#[derive(Serialize)]
struct BoardDto {
    statuses: Vec<String>,
    rows: Vec<RowDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockedDto {
    step_id: String,
    error: String,
}

#[derive(Serialize)]
struct MoveResultDto {
    completed: bool,
    blocked: Option<BlockedDto>,
}

#[derive(Serialize)]
struct SyncResultDto {
    fetched: usize,
    diverged: usize,
}

// ---- Conversions ------------------------------------------------------------

fn to_row_dto(row: OverviewRow) -> RowDto {
    RowDto {
        key: row.issue.key,
        summary: row.issue.summary,
        status: row.issue.local_status,
        tracker_status: row.issue.tracker_status,
        diverged: row.issue.diverged,
        sessions: row.sessions.into_iter().map(to_session_dto).collect(),
        worktree_path: row.worktree_path,
        git: row.git.map(to_git_dto),
    }
}

fn to_session_dto(s: Session) -> SessionLiteDto {
    SessionLiteDto {
        id: s.id,
        kind: s.kind.to_string(),
        status: s.status.to_string(),
    }
}

fn to_git_dto(g: GitState) -> GitStateDto {
    GitStateDto {
        branch: g.branch,
        ahead: g.ahead,
        behind: g.behind,
        dirty: g.dirty,
        last_commit_summary: g.last_commit_summary,
    }
}

// ---- Commands ---------------------------------------------------------------

/// Return the full board: statuses from config + all issue rows with sessions
/// and live git state. Called by `api.getBoard()` in the UI.
#[tauri::command]
async fn get_board(state: State<'_, AppState>) -> Result<BoardDto, String> {
    let rows = clabby_core::overview::build(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let statuses = state.config.states.iter().map(|s| s.name.clone()).collect();
    Ok(BoardDto {
        statuses,
        rows: rows.into_iter().map(to_row_dto).collect(),
    })
}

/// Pull issues from the tracker and reconcile. Called by `api.sync()`.
#[tauri::command]
async fn sync(state: State<'_, AppState>) -> Result<SyncResultDto, String> {
    let out = clabby_core::sync::sync(&state.db, &state.config, &state.bus)
        .await
        .map_err(|e| e.to_string())?;
    Ok(SyncResultDto {
        fetched: out.fetched,
        diverged: out.diverged,
    })
}

/// Move an issue to a new status, running the transition's steps under the gate.
/// Called by `api.move(key, to)`.
#[tauri::command]
async fn move_issue(
    key: String,
    to: String,
    state: State<'_, AppState>,
) -> Result<MoveResultDto, String> {
    let out = clabby_core::engine::transition(&state.db, &state.config, &state.bus, &key, &to)
        .await
        .map_err(|e| e.to_string())?;
    Ok(MoveResultDto {
        completed: out.completed,
        blocked: out.blocked.map(|b| BlockedDto {
            step_id: b.step_id,
            error: b.error,
        }),
    })
}

/// Override the blocked step of an issue's latest blocked transition and resume.
/// Called by `api.override(key, reason)`.
#[tauri::command]
async fn override_issue(
    key: String,
    reason: String,
    state: State<'_, AppState>,
) -> Result<MoveResultDto, String> {
    let out =
        clabby_core::engine::override_for_issue(&state.db, &state.config, &state.bus, &key, &reason)
            .await
            .map_err(|e| e.to_string())?;
    Ok(MoveResultDto {
        completed: out.completed,
        blocked: out.blocked.map(|b| BlockedDto {
            step_id: b.step_id,
            error: b.error,
        }),
    })
}

// ---- Event forwarding -------------------------------------------------------

/// Subscribe to the `EventBus` and forward each event to the Tauri frontend as
/// `"clabby://event"`. The React board listens for this event and re-fetches
/// the board data, giving real-time push updates without polling.
fn start_event_forwarding(app: AppHandle, bus: &EventBus) {
    let mut rx = bus.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let _ = app.emit("clabby://event", &event);
                }
                Err(_) => break, // channel closed — app is shutting down
            }
        }
    });
}

// ---- Entry point ------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let cwd = std::env::current_dir().unwrap_or_default();
            let config = Config::discover(&cwd)
                .expect("clabby.toml not found — run Clabby from a project directory");

            let bus = EventBus::new();
            let db = tauri::async_runtime::block_on(Db::connect(config.db_file()))
                .expect("failed to open the Clabby database");

            start_event_forwarding(app.handle().clone(), &bus);

            app.manage(AppState { db, config, bus });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_board,
            sync,
            move_issue,
            override_issue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Clabby");
}
