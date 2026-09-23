//! Release channel identity for the running binary.
//!
//! A channel is resolved once per process from the `ORBIT_CHANNEL` environment
//! variable, which each binary's `main()` sets before any orbit code runs (the
//! same pattern the dev binary already used for `ORBIT_HOME`). Everything that
//! must differ per channel — the data home suffix, the keychain service, the
//! banner label — derives from this single axiom.

/// The release channel a binary was built to serve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Stable,
    Canary,
    Dev,
}

impl Channel {
    /// Channel of the running process, read from `ORBIT_CHANNEL`.
    /// Unset or unrecognized resolves to `Stable`.
    pub fn current() -> Channel {
        match std::env::var("ORBIT_CHANNEL").as_deref() {
            Ok("canary") => Channel::Canary,
            Ok("dev") => Channel::Dev,
            _ => Channel::Stable,
        }
    }

    /// Channel implied by an `ORBIT_HOME` override that names a known
    /// channel-suffixed directory (`.orbit`, `.orbit-canary`, `.orbit-dev`).
    ///
    /// A custom home (tests / CI) has no recognized suffix and returns `None`.
    fn from_home_env() -> Option<Channel> {
        let home = std::env::var_os("ORBIT_HOME")?;
        match std::path::Path::new(&home).file_name()?.to_str()? {
            ".orbit" => Some(Channel::Stable),
            ".orbit-canary" => Some(Channel::Canary),
            ".orbit-dev" => Some(Channel::Dev),
            _ => None,
        }
    }

    /// The channel whose data home is actually in effect for this process.
    ///
    /// `ORBIT_HOME` decides where data physically lives (see `data_paths::orbit_home`),
    /// so when it pins a channel-suffixed directory it is the ground truth for the
    /// channel — independent of a possibly-contaminated `ORBIT_CHANNEL`. Otherwise
    /// falls back to [`Channel::current`].
    ///
    /// This closes the `ORBIT_HOME`/`ORBIT_CHANNEL` split: a daemon whose home is
    /// `~/.orbit-canary` but whose `ORBIT_CHANNEL` was contaminated to `dev` (e.g. an
    /// inherited value from the shell that launched it) still resolves — and stamps the
    /// sessions it spawns — as canary, so generated files land in the home the launching
    /// desktop actually watches.
    pub fn for_home() -> Channel {
        Self::from_home_env().unwrap_or_else(Channel::current)
    }

    /// Canonical lowercase name: `"stable"`, `"canary"`, `"dev"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Canary => "canary",
            Channel::Dev => "dev",
        }
    }

    /// Suffix appended to the `~/.orbit` home so channels stay isolated:
    /// `""`, `"-canary"`, `"-dev"`.
    pub fn home_suffix(self) -> &'static str {
        match self {
            Channel::Stable => "",
            Channel::Canary => "-canary",
            Channel::Dev => "-dev",
        }
    }

    /// Process name (`/proc/self/comm`) for the CLI of this channel:
    /// `orbit`, `orbit-canary`, `orbit-dev`. Derived from the home suffix so it
    /// stays in step with the data home, independent of the installed binary or
    /// symlink filename. Fits the 15-char `comm` limit on Linux.
    pub fn process_name(self) -> String {
        format!("orbit{}", self.home_suffix())
    }

    /// Process name (`/proc/self/comm`) for the daemon of this channel:
    /// `orbitd`, `orbitd-canary`, `orbitd-dev`. The short `orbitd` prefix keeps
    /// every channel within the 15-char `comm` limit on Linux.
    pub fn daemon_process_name(self) -> String {
        format!("orbitd{}", self.home_suffix())
    }

    /// Base TCP port for this channel's local debug/MCP server:
    /// stable 17777, canary 17787, dev 17797. Each channel owns a 10-port block
    /// (`base..base+10`) so its server has a fixed, predictable address and can
    /// fall back within its own block without ever colliding with another
    /// channel. The single source of truth for the port contract.
    pub fn debug_port_base(self) -> u16 {
        match self {
            Channel::Stable => 17777,
            Channel::Canary => 17787,
            Channel::Dev => 17797,
        }
    }

    /// Uppercase branding tag shown in the banner for non-stable builds.
    /// `None` for stable (no tag).
    pub fn label(self) -> Option<&'static str> {
        match self {
            Channel::Stable => None,
            Channel::Canary => Some("CANARY"),
            Channel::Dev => Some("DEV"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_and_label_pair_up() {
        assert_eq!(Channel::Stable.home_suffix(), "");
        assert_eq!(Channel::Stable.label(), None);
        assert_eq!(Channel::Canary.home_suffix(), "-canary");
        assert_eq!(Channel::Canary.label(), Some("CANARY"));
        assert_eq!(Channel::Dev.home_suffix(), "-dev");
        assert_eq!(Channel::Dev.label(), Some("DEV"));
    }

    #[test]
    fn process_names_fit_comm_limit() {
        for ch in [Channel::Stable, Channel::Canary, Channel::Dev] {
            assert!(ch.process_name().len() <= 15, "{}", ch.process_name());
            assert!(
                ch.daemon_process_name().len() <= 15,
                "{}",
                ch.daemon_process_name()
            );
        }
        assert_eq!(Channel::Stable.process_name(), "orbit");
        assert_eq!(Channel::Canary.process_name(), "orbit-canary");
        assert_eq!(Channel::Dev.process_name(), "orbit-dev");
        assert_eq!(Channel::Stable.daemon_process_name(), "orbitd");
        assert_eq!(Channel::Canary.daemon_process_name(), "orbitd-canary");
        assert_eq!(Channel::Dev.daemon_process_name(), "orbitd-dev");
    }

    #[test]
    fn debug_ports_are_fixed_and_blocked_per_channel() {
        assert_eq!(Channel::Stable.debug_port_base(), 17777);
        assert_eq!(Channel::Canary.debug_port_base(), 17787);
        assert_eq!(Channel::Dev.debug_port_base(), 17797);
        // 10-port blocks never overlap.
        for (a, b) in [
            (Channel::Stable, Channel::Canary),
            (Channel::Canary, Channel::Dev),
        ] {
            assert!(b.debug_port_base() - a.debug_port_base() >= 10);
        }
    }

    #[test]
    fn current_parses_env() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("ORBIT_CHANNEL", "canary") };
        assert_eq!(Channel::current(), Channel::Canary);
        unsafe { std::env::set_var("ORBIT_CHANNEL", "dev") };
        assert_eq!(Channel::current(), Channel::Dev);
        unsafe { std::env::set_var("ORBIT_CHANNEL", "nonsense") };
        assert_eq!(Channel::current(), Channel::Stable);
        unsafe { std::env::remove_var("ORBIT_CHANNEL") };
        assert_eq!(Channel::current(), Channel::Stable);
    }

    #[test]
    fn for_home_prefers_pinned_home_over_contaminated_channel() {
        let _lock = crate::TEST_ENV_LOCK.lock().unwrap();
        let prev_channel = std::env::var("ORBIT_CHANNEL").ok();
        let prev_home = std::env::var("ORBIT_HOME").ok();

        // A channel-suffixed ORBIT_HOME is ground truth even when ORBIT_CHANNEL disagrees.
        unsafe {
            std::env::set_var("ORBIT_CHANNEL", "dev");
            std::env::set_var("ORBIT_HOME", "/tmp/.orbit-canary");
        }
        assert_eq!(Channel::for_home(), Channel::Canary);

        // A custom (non-suffixed) home falls back to ORBIT_CHANNEL.
        unsafe { std::env::set_var("ORBIT_HOME", "/tmp/orbit-test-home") };
        assert_eq!(Channel::for_home(), Channel::Dev);

        // No home override → identical to current().
        unsafe {
            std::env::remove_var("ORBIT_HOME");
            std::env::set_var("ORBIT_CHANNEL", "canary");
        }
        assert_eq!(Channel::for_home(), Channel::Canary);

        unsafe {
            match prev_channel {
                Some(v) => std::env::set_var("ORBIT_CHANNEL", v),
                None => std::env::remove_var("ORBIT_CHANNEL"),
            }
            match prev_home {
                Some(v) => std::env::set_var("ORBIT_HOME", v),
                None => std::env::remove_var("ORBIT_HOME"),
            }
        }
    }
}
