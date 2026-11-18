//! Helpers for running external commands (systemctl, docker, apt, ...).
//!
//! Commands are executed directly (no shell) unless [`run_shell`] is used.

use std::fmt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

/// Captured output of a successful command.
#[derive(Debug, Default)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

/// A command that could not be started, exited non-zero or timed out.
#[derive(Debug)]
pub struct CmdError {
    pub message: String,
    pub stdout: String,
    pub stderr: String,
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CmdError {}

/// Options for [`run_with`].
#[derive(Default)]
pub struct RunOptions<'a> {
    pub cwd: Option<&'a Path>,
    pub timeout: Option<Duration>,
}

/// Runs `program` with `args` and fails if it exits non-zero.
pub async fn run<S: AsRef<str>>(program: &str, args: &[S]) -> Result<Output, CmdError> {
    run_with(program, args, RunOptions::default()).await
}

/// Runs a command line through `sh -c`. Only use with trusted, constant input.
pub async fn run_shell(command: &str) -> Result<Output, CmdError> {
    run("sh", &["-c", command]).await
}

pub async fn run_with<S: AsRef<str>>(
    program: &str,
    args: &[S],
    options: RunOptions<'_>,
) -> Result<Output, CmdError> {
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    let command_line = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");

    let mut command = Command::new(program);
    command.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if let Some(cwd) = options.cwd {
        command.current_dir(cwd);
    }

    let child = command.spawn().map_err(|e| CmdError {
        message: format!("Command failed: {command_line}\n{e}"),
        stdout: String::new(),
        stderr: String::new(),
    })?;

    let output = match options.timeout {
        Some(timeout) => match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(result) => result,
            Err(_) => {
                return Err(CmdError {
                    message: format!("Command timed out after {}s: {command_line}", timeout.as_secs()),
                    stdout: String::new(),
                    stderr: String::new(),
                })
            }
        },
        None => child.wait_with_output().await,
    }
    .map_err(|e| CmdError {
        message: format!("Command failed: {command_line}\n{e}"),
        stdout: String::new(),
        stderr: String::new(),
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(Output { stdout, stderr })
    } else {
        Err(CmdError { message: format!("Command failed: {command_line}\n{stderr}"), stdout, stderr })
    }
}

/// Whether this process runs as root.
pub fn is_root() -> bool {
    nix::unistd::getuid().is_root()
}
