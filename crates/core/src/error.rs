//! Crate-wide error type.
//!
//! Constitution §11: the vocabulary here is deliberately domain-neutral — no
//! tracker- or vendor-specific variants leak into the engine.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("template error: {0}")]
    Template(#[from] minijinja::Error),

    /// An external command (issue tracker, git, agent, hook) exited non-zero or
    /// could not be spawned. Carries human-facing detail for the audit trail.
    #[error("command failed: {0}")]
    Command(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }
    pub fn command(msg: impl Into<String>) -> Self {
        Error::Command(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }
}
