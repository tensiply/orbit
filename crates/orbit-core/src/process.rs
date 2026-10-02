//! Cross-platform process helpers.

use anyhow::Result;
use std::process::{Child, Command};

/// Decide what to change to scrub AppImage-injected variables, given the mount
/// prefix (`$APPDIR`, e.g. `/tmp/.mount_orbit-XXXX`) and the current environment.
///
/// Each returned `(key, action)` describes one variable: `None` means remove it,
/// `Some(value)` means replace its value. List variables (colon-separated, like
/// `PATH` or `XDG_DATA_DIRS`) keep their non-AppImage entries; single-value
/// variables that point into the mount collapse to `None` and are removed.
///
/// Pure so it can be tested without touching process-global state.
pub fn plan_appimage_scrub(
    appdir: &str,
    vars: impl IntoIterator<Item = (String, String)>,
) -> Vec<(String, Option<String>)> {
    vars.into_iter()
        .filter(|(_, value)| value.contains(appdir))
        .map(|(key, value)| {
            let kept: Vec<&str> = value.split(':').filter(|e| !e.contains(appdir)).collect();
            let action = if kept.is_empty() {
                None
            } else {
                Some(kept.join(":"))
            };
            (key, action)
        })
        .collect()
}

/// Remove AppImage-injected environment variables from the current process so
/// they are not propagated to child processes (engine sessions, the Bash tool,
/// MCP servers).
///
/// When orbit runs from an AppImage (the desktop app's bundled binary),
/// `AppRun` exports `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `LD_LIBRARY_PATH`,
/// and a family of GTK/GI/GST/Qt loader paths — all pointing into the AppImage's
/// temporary mount (`$APPDIR`). Those are meant for the bundled GUI runtime
/// only; leaking them to arbitrary child processes breaks any external binary
/// that depends on the system's Python/Perl/shared libraries. The canonical
/// failure is a venv Python dying with `ModuleNotFoundError: No module named
/// 'encodings'` because `PYTHONHOME` redirects it to the AppImage's stdlib.
///
/// The scrub is value-based, not name-based: any variable whose value references
/// the mount is cleaned, so it stays correct even if the AppImage recipe adds
/// new variables. No-op when not launched from an AppImage (`APPDIR` unset),
/// which is every non-Linux platform and every non-bundled launch.
///
/// Call once at process startup, before any worker thread reads the environment.
/// The GUI process (Tauri) must NOT call this: it needs the bundled loader paths
/// to render. Only the CLI/daemon entrypoint scrubs, which keeps every spawned
/// session clean while leaving the GUI's own runtime intact.
///
/// # Safety
/// Mutates process-global env via `set_var`/`remove_var`, which is not
/// thread-safe in Rust 1.80+. Safe at the single-threaded startup call site.
pub fn scrub_appimage_env() {
    for (key, action) in appimage_scrub_plan() {
        // Safety: startup is single-threaded; no other thread touches env yet.
        unsafe {
            match action {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }
}

/// The AppImage scrub plan for the *current* process environment, without
/// mutating it. Returns an empty vec when not running from an AppImage.
///
/// Use this when the current process must keep its own environment intact (the
/// Tauri GUI needs the bundled loader paths to render) but a child process it
/// spawns must not inherit them: apply each `(key, action)` to the child's
/// command builder — `None` removes the variable, `Some(value)` replaces it.
pub fn appimage_scrub_plan() -> Vec<(String, Option<String>)> {
    let Some(appdir) = std::env::var_os("APPDIR") else {
        return Vec::new();
    };
    let appdir = appdir.to_string_lossy().into_owned();
    if appdir.is_empty() {
        return Vec::new();
    }
    plan_appimage_scrub(&appdir, std::env::vars())
}

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

#[cfg(test)]
mod tests {
    use super::plan_appimage_scrub;

    const APPDIR: &str = "/tmp/.mount_orbit-iHbaAn";

    fn plan(vars: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        let owned = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>();
        plan_appimage_scrub(APPDIR, owned)
    }

    #[test]
    fn single_value_var_in_mount_is_removed() {
        // PYTHONHOME points entirely into the mount → remove it outright.
        let out = plan(&[("PYTHONHOME", "/tmp/.mount_orbit-iHbaAn/usr/")]);
        assert_eq!(out, vec![("PYTHONHOME".to_string(), None)]);
    }

    #[test]
    fn list_var_keeps_non_mount_entries() {
        // PATH keeps system entries and drops only the AppImage bin dir.
        let out = plan(&[(
            "PATH",
            "/usr/local/bin:/tmp/.mount_orbit-iHbaAn/usr/bin/:/usr/bin",
        )]);
        assert_eq!(
            out,
            vec![(
                "PATH".to_string(),
                Some("/usr/local/bin:/usr/bin".to_string())
            )]
        );
    }

    #[test]
    fn list_var_entirely_in_mount_is_removed() {
        // A trailing empty entry (from the AppImage's `dir:` form) must not keep
        // the var alive — only mount references were present.
        let out = plan(&[("PYTHONPATH", "/tmp/.mount_orbit-iHbaAn/usr/share/pyshared/")]);
        assert_eq!(out, vec![("PYTHONPATH".to_string(), None)]);
    }

    #[test]
    fn appdir_itself_is_removed() {
        let out = plan(&[("APPDIR", APPDIR)]);
        assert_eq!(out, vec![("APPDIR".to_string(), None)]);
    }

    #[test]
    fn clean_vars_are_untouched() {
        // APPIMAGE points at the .AppImage file, not the mount → left alone.
        // HOME and a clean PATH are never in the plan.
        let out = plan(&[
            ("HOME", "/home/eloir"),
            ("APPIMAGE", "/home/eloir/Descargas/orbit-desktop.AppImage"),
            ("PATH", "/usr/local/bin:/usr/bin"),
        ]);
        assert!(out.is_empty());
    }
}
