use anyhow::Result;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use orbit_core::net::{NetworkRole, OrbitClaims};
use std::{fs, path::PathBuf};

fn key_path() -> PathBuf {
    orbit_core::data_paths::orbit_data_root().join("serve.key")
}

pub fn load_or_create_signing_key() -> Result<[u8; 32]> {
    let path = key_path();
    if path.exists() {
        let hex_str = fs::read_to_string(&path)?;
        let bytes = hex::decode(hex_str.trim())?;
        if bytes.len() != 32 {
            anyhow::bail!("serve.key has invalid length {}", bytes.len());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, hex::encode(key))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

pub fn mint_token(
    role: NetworkRole,
    key: &[u8; 32],
    ttl_secs: u64,
    instance: &str,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let claims = OrbitClaims {
        role,
        exp: now + ttl_secs,
        iat: now,
        scope: "orbit:lan".into(),
        instance: instance.to_string(),
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(key),
    )?;
    Ok(token)
}

pub fn verify_token(token: &str, key: &[u8; 32]) -> Result<OrbitClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "iat"]);
    let data = decode::<OrbitClaims>(
        token,
        &DecodingKey::from_secret(key),
        &validation,
    )?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_key() -> [u8; 32] {
        [42u8; 32]
    }

    #[test]
    fn mint_and_verify_observer() {
        let key = test_key();
        let token = mint_token(NetworkRole::Observer, &key, 3600, "test-host").unwrap();
        let claims = verify_token(&token, &key).unwrap();
        assert_eq!(claims.role, NetworkRole::Observer);
        assert_eq!(claims.scope, "orbit:lan");
        assert_eq!(claims.instance, "test-host");
    }

    #[test]
    fn mint_and_verify_contributor() {
        let key = test_key();
        let token = mint_token(NetworkRole::Contributor, &key, 3600, "test").unwrap();
        let claims = verify_token(&token, &key).unwrap();
        assert_eq!(claims.role, NetworkRole::Contributor);
    }

    #[test]
    fn wrong_key_rejected() {
        let key1 = test_key();
        let key2 = [99u8; 32];
        let token = mint_token(NetworkRole::Observer, &key1, 3600, "test").unwrap();
        assert!(verify_token(&token, &key2).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let key = test_key();
        use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
        let claims = OrbitClaims {
            role: NetworkRole::Observer,
            exp: 1,
            iat: 0,
            scope: "orbit:lan".into(),
            instance: "test".into(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&key),
        )
        .unwrap();
        assert!(verify_token(&token, &key).is_err());
    }

    #[test]
    fn signing_key_persists() {
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var(
                "XDG_DATA_HOME",
                tmp.path().join("data").to_str().unwrap(),
            );
        }
        let key1 = load_or_create_signing_key().unwrap();
        let key2 = load_or_create_signing_key().unwrap();
        assert_eq!(key1, key2);
    }
}
