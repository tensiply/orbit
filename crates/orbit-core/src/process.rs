//! Cross-platform process helpers.

use anyhow::Result;
use std::process::Command;

/// Replace the current process with `cmd`. On unix this is a real `exec` and
/// never returns on success. Windows has no `exec`, so it spawns the command,
/// waits for it, and exits the current process with the child's status code —
/// giving callers the same "hand off and terminate" behavior.
#[cfg(unix)]
pub fn exec_replacing(mut cmd: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let err = cmd.exec();
    anyhow::bail!("failed to exec process: {err}");
}

/// Replace the current process with `cmd` (Windows: spawn, wait, exit).
#[cfg(windows)]
pub fn exec_replacing(mut cmd: Command) -> Result<()> {
    let status = cmd.status()?;
    std::process::exit(status.code().unwrap_or(1));
}
