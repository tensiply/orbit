//! Cross-platform process helpers.

use anyhow::Result;
use std::process::{Child, Command};

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

/// Spawn `cmd` as a detached background process that outlives the launching
/// terminal. On Windows the child is given `CREATE_NO_WINDOW | DETACHED_PROCESS`
/// so it neither inherits nor allocates a console — without this the daemon
/// dies (or flashes a console window) when the parent shell closes. On unix a
/// plain spawn with redirected stdio already detaches, so no flags are needed.
pub fn spawn_detached(mut cmd: Command) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    cmd.spawn()
}
