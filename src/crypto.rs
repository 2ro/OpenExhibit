// Authenticated-encryption helpers for at-rest values stored in the DB
// (currently the SMTP relay password).
//
// Key is derived from SESSION_KEY via SHA-256 over a fixed domain string,
// so rotating SESSION_KEY invalidates ciphertexts. We treat that as an
// acceptable tradeoff: the same Config is required to start the app, and
// an operator rotating SESSION_KEY can re-enter the SMTP password.
//
// Encrypted values carry an "enc:" prefix to disambiguate from legacy
// plaintext rows. The on-disk format inside the prefix is base64(URL no pad)
// over (nonce || ciphertext || auth-tag).

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};

const PREFIX: &str = "enc:";
const DOMAIN: &[u8] = b"openexhibit/v1/smtp-password";

fn derive_key(session_key: &str) -> Key {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update(session_key.as_bytes());
    let out = h.finalize();
    *Key::from_slice(&out)
}

pub fn encrypt(plain: &str, session_key: &str) -> anyhow::Result<String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    let cipher = ChaCha20Poly1305::new(&derive_key(session_key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|e| anyhow::anyhow!("aead encrypt: {e}"))?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(format!(
        "{PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(out)
    ))
}

/// Decrypt a previously-encrypted value. Values without the `"enc:"` prefix
/// are returned as-is (legacy plaintext, so rolling forward is non-destructive).
pub fn decrypt(stored: &str, session_key: &str) -> anyhow::Result<String> {
    let Some(b64) = stored.strip_prefix(PREFIX) else {
        return Ok(stored.to_string());
    };
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|e| anyhow::anyhow!("aead b64: {e}"))?;
    if raw.len() < 12 + 16 {
        anyhow::bail!("ciphertext too short");
    }
    let (nonce_bytes, ct) = raw.split_at(12);
    let cipher = ChaCha20Poly1305::new(&derive_key(session_key));
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ct)
        .map_err(|e| anyhow::anyhow!("aead decrypt: {e}"))?;
    String::from_utf8(pt).map_err(|e| anyhow::anyhow!("utf-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn empty_string_is_passthrough() {
        assert_eq!(encrypt("", TEST_KEY).unwrap(), "");
        assert_eq!(decrypt("", TEST_KEY).unwrap(), "");
    }

    #[test]
    fn roundtrip_distinct_each_call() {
        let a = encrypt("hunter2", TEST_KEY).unwrap();
        let b = encrypt("hunter2", TEST_KEY).unwrap();
        assert_ne!(a, b, "nonces must differ across calls");
        assert_eq!(decrypt(&a, TEST_KEY).unwrap(), "hunter2");
        assert_eq!(decrypt(&b, TEST_KEY).unwrap(), "hunter2");
    }

    #[test]
    fn legacy_plaintext_passes_through_on_decrypt() {
        // No prefix → returned as-is. Lets pre-encryption rows keep working
        // until the operator next saves the settings page.
        assert_eq!(decrypt("legacy-plain", TEST_KEY).unwrap(), "legacy-plain");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let e = encrypt("hunter2", TEST_KEY).unwrap();
        // Swap the last b64 char for a different one — flipping bits inside
        // the AEAD-protected blob should make verification fail.
        let mut chars: Vec<char> = e.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert!(decrypt(&tampered, TEST_KEY).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let c = encrypt("hunter2", TEST_KEY).unwrap();
        let other = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        assert!(decrypt(&c, other).is_err());
    }
}
