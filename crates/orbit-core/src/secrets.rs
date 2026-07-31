use anyhow::{Result, bail};

const KEYRING_SERVICE: &str = "orbit";

// ── public API ────────────────────────────────────────────────────────────────

/// Resolve a potentially-secret value from an `orbit.json` `env` entry.
///
/// Supported forms:
/// - `$VAR`              → value of environment variable `VAR`
/// - `env://VAR`         → value of environment variable `VAR`
/// - `file:///path`      → trimmed contents of the file at `/path`
/// - `file://path`       → trimmed contents of the file at `path`
/// - `keychain://KEY`    → secret stored under `KEY` in the OS keychain
/// - `"Bearer ${VAR}"`   → inline interpolation: `${VAR}` replaced with env var
/// - anything else       → returned as-is (literal value)
///
/// On resolution failure, logs a warning and returns an empty string (full
/// substitution) or leaves the placeholder unchanged (inline interpolation)
/// so that a missing secret does not abort the launch.
pub fn resolve(value: &str) -> String {
    if let Some(var) = value.strip_prefix('$') {
        // Full substitution: $VAR (no braces) — but not ${VAR} handled below
        if !var.starts_with('{') {
            return resolve_env(var, value);
        }
    }
    if let Some(var) = value.strip_prefix("env://") {
        return resolve_env(var, value);
    }
    if let Some(path) = value.strip_prefix("file://") {
        return resolve_file(path, value);
    }
    if let Some(key) = value.strip_prefix("keychain://") {
        return resolve_keychain(key, value);
    }
    // Inline interpolation: replace all ${VAR} occurrences within the string
    if value.contains("${") {
        return interpolate(value);
    }
    value.to_string()
}

/// Replace every `${VAR}` occurrence in `template` with the value of env var `VAR`.
/// Unresolved placeholders are left unchanged.
fn interpolate(template: &str) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find('}') {
            let expr = &rest[..end];
            let value = if let Some(key) = expr.strip_prefix("keychain://") {
                match keychain_get(key) {
                    Ok(v) => v,
                    Err(_) => {
                        tracing::warn!("keychain lookup failed for '{key}' (referenced as '${{{expr}}}')");
                        format!("${{{expr}}}")
                    }
                }
            } else {
                match std::env::var(expr) {
                    Ok(v) => v,
                    Err(_) => {
                        tracing::warn!("env var '{expr}' not set (referenced as '${{{expr}}}')");
                        format!("${{{expr}}}")
                    }
                }
            };
            result.push_str(&value);
            rest = &rest[end + 1..];
        } else {
            // Unclosed ${ — emit as-is
            result.push_str("${");
        }
    }
    result.push_str(rest);
    result
}

/// Store a secret in the OS keychain under `KEY`.
pub fn keychain_set(key: &str, secret: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)?;
    entry.set_password(secret)?;
    Ok(())
}

/// Retrieve a secret from the OS keychain.
pub fn keychain_get(key: &str) -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)?;
    let secret = entry.get_password()?;
    Ok(secret)
}

/// Delete a secret from the OS keychain.
pub fn keychain_delete(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => bail!("no secret found for key '{key}'"),
        Err(e) => Err(e.into()),
    }
}

// ── resolvers ─────────────────────────────────────────────────────────────────

fn resolve_env(var: &str, original: &str) -> String {
    match std::env::var(var) {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!("env var '{var}' not set (referenced as '{original}')");
            String::new()
        }
    }
}

fn resolve_file(path: &str, original: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(contents) => contents.trim().to_string(),
        Err(e) => {
            tracing::warn!("could not read secret file '{path}': {e} (referenced as '{original}')");
            String::new()
        }
    }
}

fn resolve_keychain(key: &str, original: &str) -> String {
    match keychain_get(key) {
        Ok(secret) => secret,
        Err(e) => {
            tracing::warn!(
                "keychain lookup failed for '{key}': {e} (referenced as '{original}')\n  \
                 hint: run `orbit secret set {key} <value>` to store it"
            );
            String::new()
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn literal_passthrough() {
        assert_eq!(resolve("hello"), "hello");
        assert_eq!(resolve("https://example.com"), "https://example.com");
    }

    #[test]
    fn dollar_var_resolves() {
        unsafe { env::set_var("ORBIT_TEST_SECRET_VAR", "resolved") };
        assert_eq!(resolve("$ORBIT_TEST_SECRET_VAR"), "resolved");
        unsafe { env::remove_var("ORBIT_TEST_SECRET_VAR") };
    }

    #[test]
    fn env_prefix_resolves() {
        unsafe { env::set_var("ORBIT_TEST_ENV_PREFIX", "via-prefix") };
        assert_eq!(resolve("env://ORBIT_TEST_ENV_PREFIX"), "via-prefix");
        unsafe { env::remove_var("ORBIT_TEST_ENV_PREFIX") };
    }

    #[test]
    fn missing_env_var_returns_empty() {
        unsafe { env::remove_var("ORBIT_DEFINITELY_NOT_SET_XYZ") };
        assert_eq!(resolve("$ORBIT_DEFINITELY_NOT_SET_XYZ"), "");
    }

    #[test]
    fn file_resolves_to_trimmed_contents() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "  my-secret\n").unwrap();
        let uri = format!("file://{}", tmp.path().display());
        assert_eq!(resolve(&uri), "my-secret");
    }

    #[test]
    fn missing_file_returns_empty() {
        assert_eq!(resolve("file:///nonexistent/secret.txt"), "");
    }

    #[test]
    fn inline_interpolation_single() {
        unsafe { env::set_var("ORBIT_TEST_INLINE", "tok123") };
        assert_eq!(resolve("Bearer ${ORBIT_TEST_INLINE}"), "Bearer tok123");
        unsafe { env::remove_var("ORBIT_TEST_INLINE") };
    }

    #[test]
    fn inline_interpolation_multiple() {
        unsafe { env::set_var("ORBIT_TEST_A", "hello") };
        unsafe { env::set_var("ORBIT_TEST_B", "world") };
        assert_eq!(resolve("${ORBIT_TEST_A} ${ORBIT_TEST_B}"), "hello world");
        unsafe { env::remove_var("ORBIT_TEST_A") };
        unsafe { env::remove_var("ORBIT_TEST_B") };
    }

    #[test]
    fn inline_interpolation_missing_var_leaves_placeholder() {
        unsafe { env::remove_var("ORBIT_DEFINITELY_NOT_SET_INLINE") };
        let result = resolve("Bearer ${ORBIT_DEFINITELY_NOT_SET_INLINE}");
        assert_eq!(result, "Bearer ${ORBIT_DEFINITELY_NOT_SET_INLINE}");
    }

    #[test]
    fn dollar_without_braces_still_full_substitution() {
        unsafe { env::set_var("ORBIT_TEST_FULL", "resolved") };
        assert_eq!(resolve("$ORBIT_TEST_FULL"), "resolved");
        unsafe { env::remove_var("ORBIT_TEST_FULL") };
    }

    #[test]
    fn inline_interpolation_missing_keychain_leaves_placeholder() {
        // Key that definitely does not exist in keychain
        let result = resolve("Bearer ${keychain://ORBIT_DEFINITELY_NOT_SET_KC_XYZ}");
        assert_eq!(result, "Bearer ${keychain://ORBIT_DEFINITELY_NOT_SET_KC_XYZ}");
    }
}
