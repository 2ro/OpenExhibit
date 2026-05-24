// Double-submit CSRF using the (server-encrypted) session as the secret store.
// Templates render the token into a hidden form input; the verify helper
// constant-time-compares the submitted value against the session value.

use actix_session::Session;
use base64::Engine;
use rand::{thread_rng, RngCore};

use crate::error::{AppError, AppResult};

pub(crate) const SESSION_KEY: &str = "csrf_token";

pub fn get_or_create(session: &Session) -> AppResult<String> {
    if let Some(existing) = session
        .get::<String>(SESSION_KEY)
        .map_err(|e| AppError::Internal(e.into()))?
    {
        return Ok(existing);
    }
    let mut bytes = [0u8; 32];
    thread_rng().fill_bytes(&mut bytes);
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    session
        .insert(SESSION_KEY, &token)
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(token)
}

pub fn verify(session: &Session, submitted: &str) -> bool {
    let Ok(Some(expected)) = session.get::<String>(SESSION_KEY) else {
        return false;
    };
    constant_time_eq(expected.as_bytes(), submitted.as_bytes())
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn ct_eq_basic() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
