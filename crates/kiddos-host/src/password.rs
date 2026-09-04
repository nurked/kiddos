//! Parent password: argon2id hash in `parent.hash`, never in the VFS.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use std::path::Path;

/// `None` if no password is set yet.
pub fn verify(path: &Path, password: &str) -> Option<bool> {
    let stored = std::fs::read_to_string(path).ok()?;
    let stored = stored.trim();
    if stored.is_empty() {
        return None;
    }
    let parsed = PasswordHash::new(stored).ok()?;
    Some(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn set(path: &Path, password: &str) -> Result<(), String> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    std::fs::write(path, hash).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let p = std::env::temp_dir().join(format!("kiddos-pw-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        assert_eq!(verify(&p, "x"), None);
        set(&p, "hunter2").unwrap();
        assert_eq!(verify(&p, "hunter2"), Some(true));
        assert_eq!(verify(&p, "hunter3"), Some(false));
        let _ = std::fs::remove_file(&p);
    }
}
