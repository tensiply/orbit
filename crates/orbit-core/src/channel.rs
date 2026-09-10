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
}
