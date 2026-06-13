//! clabby-core — the engine.
//!
//! Constitution §7: this crate has **no** UI/Tauri dependency. The CLI, cron, and
//! (later) the Tauri app are thin drivers over these modules. §11: nothing here
//! encodes a specific workflow — that lives in `Config` (clabby.toml) and hooks.

pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod git;
pub mod jsonpath;
pub mod model;
pub mod overview;
pub mod runner;
pub mod session;
pub mod sync;
pub mod template;

pub use config::Config;
pub use db::Db;
pub use error::{Error, Result};
pub use events::{Event, EventBus};
pub use model::{
    GitState, Issue, Session, SessionKind, SessionStatus, Worktree,
};
