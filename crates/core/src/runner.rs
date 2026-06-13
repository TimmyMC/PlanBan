//! Process runner — the one place the engine shells out.
//!
//! Every external action is a rendered command template run here (§5). Two modes:
//! `run_capture` for short synchronous commands (tracker fetch/push, git
//! queries), and `spawn_streaming` for long-lived managed agent sessions whose
//! stdout/stderr we stream line-by-line.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    pub fn as_str(self) -> &'static str {
        match self {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: Stream,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

/// Build a `Command` that runs `cmd_line` through the platform shell, so arbitrary
/// command-template strings (with their own args/quoting) work unchanged (§5).
///
/// On Windows we pass the command line to `cmd /C` **verbatim** via `raw_arg`:
/// std's normal argument escaping mangles quoted paths in a way `cmd` rejects, so
/// the template's own quoting must reach `cmd` untouched.
pub fn shell(cmd_line: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Wrap the whole command line in one outer quote pair. `cmd /C "<line>"`
        // strips exactly that outer pair and runs the rest verbatim — the only
        // reliable way to preserve inner quotes (e.g. an arg like "In Review")
        // without cmd's quote-stripping heuristics mangling them.
        let mut std_cmd = std::process::Command::new("cmd");
        std_cmd.raw_arg("/C");
        std_cmd.raw_arg(format!("\"{cmd_line}\""));
        Command::from(std_cmd)
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd_line);
        c
    }
}

/// Run a command to completion, capturing all output.
pub async fn run_capture(cmd_line: &str, cwd: Option<&Path>) -> Result<CommandOutput> {
    let mut cmd = shell(cmd_line);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let out = cmd.output().await?;
    Ok(CommandOutput {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// A spawned, streaming child process. Caller owns the `child` handle (to await
/// exit / kill) and drains `lines` until the channel closes.
pub struct StreamingChild {
    pub child: Child,
    pub pid: Option<u32>,
    pub lines: mpsc::UnboundedReceiver<LogLine>,
}

/// Spawn a long-lived command, streaming stdout/stderr lines over a channel.
pub fn spawn_streaming(
    cmd_line: &str,
    cwd: Option<&Path>,
    env: &HashMap<String, String>,
) -> Result<StreamingChild> {
    let mut cmd = shell(cmd_line);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let (tx, rx) = mpsc::unbounded_channel();

    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx
                    .send(LogLine {
                        stream: Stream::Stdout,
                        text: line,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        // Move the original sender here so the channel closes once both readers end.
        tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx
                    .send(LogLine {
                        stream: Stream::Stderr,
                        text: line,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    Ok(StreamingChild {
        child,
        pid,
        lines: rx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capture_runs_a_command() {
        let out = run_capture("echo hello-clabby", None).await.unwrap();
        assert!(out.success());
        assert!(out.stdout.contains("hello-clabby"));
    }

    #[tokio::test]
    async fn nonzero_exit_is_reported() {
        // `exit 3` works in both cmd and sh.
        let out = run_capture("exit 3", None).await.unwrap();
        assert_eq!(out.status, 3);
        assert!(!out.success());
    }

    // Regression: a quoted argument containing a space (e.g. a status like
    // "In Review") must reach the program as ONE argument. cmd.exe's quote
    // stripping previously split it. Uses node to echo argv unambiguously.
    #[cfg(windows)]
    #[tokio::test]
    async fn quoted_argument_with_space_is_one_arg() {
        let out = run_capture(
            "node -e \"process.stdout.write(process.argv.slice(1).join('|'))\" \"In Review\" second",
            None,
        )
        .await
        .unwrap();
        assert!(out.success(), "command failed; stderr: {}", out.stderr);
        assert_eq!(out.stdout.trim(), "In Review|second");
    }
}
