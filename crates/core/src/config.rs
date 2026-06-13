//! Configuration model — the entire workflow lives here, not in code (§5, §11).
//!
//! A different team adopts Clabby by writing a different `clabby.toml` + hook
//! scripts. The engine must never grow a type that only makes sense for one
//! tracker or one agent.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub tracker: TrackerConfig,
    #[serde(default)]
    pub states: Vec<StateConfig>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub cron: Vec<CronConfig>,

    /// Absolute path of the config file's directory. Filled in by [`Config::load`];
    /// not part of the TOML. Relative paths in config resolve against this.
    #[serde(skip)]
    pub root_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    /// Base directory under which issue worktrees are created.
    #[serde(default)]
    pub worktrees_dir: Option<String>,
    /// Repository the worktrees are derived from (defaults to the config dir).
    #[serde(default)]
    pub repo_dir: Option<String>,
}

fn default_db_path() -> String {
    ".clabby/clabby.db".to_string()
}

/// Command templates that talk to the external issue tracker. All vendor
/// specifics (Jira, GitHub, Azure Boards) live in these strings (§5).
#[derive(Debug, Clone, Deserialize)]
pub struct TrackerConfig {
    /// Command that prints the issue list as JSON to stdout. `{{ jql }}` available.
    pub fetch: String,
    /// Command that pushes a status change. `{{ key }}` and `{{ status }}` available.
    #[serde(default)]
    pub push: Option<String>,
    /// The query string substituted into `fetch` as `{{ jql }}`.
    #[serde(default)]
    pub jql: String,
    /// How to read issues out of the fetch command's JSON. Keeps the engine
    /// agnostic to any tracker's response shape.
    pub map: TrackerMap,
}

/// Dotted paths into the tracker's JSON. `items` points at the array; the rest
/// are relative to each element. Supports `a.b.c` and array indexing `a.0.b`.
#[derive(Debug, Clone, Deserialize)]
pub struct TrackerMap {
    pub items: String,
    pub key: String,
    pub summary: String,
    pub status: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub labels: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateConfig {
    pub name: String,
    /// Optional grouping (e.g. "todo"/"in-progress"/"done") for display.
    #[serde(default)]
    pub category: Option<String>,
}

/// A spawnable agent harness. Just a command template + environment (§5).
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub cmd: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CronConfig {
    /// 6-field cron expression (sec min hour day month weekday).
    pub schedule: String,
    /// A built-in action name the driver knows how to run (e.g. "sync").
    pub action: String,
}

impl Config {
    /// Parse a config file and record its directory for relative-path resolution.
    pub fn load(path: impl AsRef<Path>) -> Result<Config> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::config(format!("reading {}: {e}", path.display())))?;
        let mut cfg: Config = toml::from_str(&text)?;
        cfg.root_dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        cfg.validate()?;
        Ok(cfg)
    }

    /// Search the current directory and its ancestors for `clabby.toml`.
    pub fn discover(start: impl AsRef<Path>) -> Result<Config> {
        let start = start.as_ref();
        for dir in start.ancestors() {
            let candidate = dir.join("clabby.toml");
            if candidate.is_file() {
                return Config::load(candidate);
            }
        }
        Err(Error::config(format!(
            "no clabby.toml found in {} or any parent directory",
            start.display()
        )))
    }

    fn validate(&self) -> Result<()> {
        if self.tracker.fetch.trim().is_empty() {
            return Err(Error::config("tracker.fetch must not be empty"));
        }
        // States are optional in M1 (sync discovers them), but if present, names
        // must be unique so column mapping is unambiguous.
        let mut seen = std::collections::HashSet::new();
        for s in &self.states {
            if !seen.insert(&s.name) {
                return Err(Error::config(format!("duplicate state name: {}", s.name)));
            }
        }
        Ok(())
    }

    /// Resolve a possibly-relative path against the config directory.
    pub fn resolve_path(&self, p: &str) -> PathBuf {
        let path = Path::new(p);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root_dir.join(path)
        }
    }

    /// Absolute path to the SQLite database file.
    pub fn db_file(&self) -> PathBuf {
        self.resolve_path(&self.project.db_path)
    }

    /// Repo the worktrees come from (defaults to the config directory).
    pub fn repo_dir(&self) -> PathBuf {
        match &self.project.repo_dir {
            Some(p) => self.resolve_path(p),
            None => self.root_dir.clone(),
        }
    }
}
