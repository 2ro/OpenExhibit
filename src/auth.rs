use std::sync::OnceLock;

use actix_session::Session;
use actix_web::HttpResponse;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sqlx::PgPool;

use crate::models::user::User;

const SESSION_USER_KEY: &str = "user_id";

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash: {e}"))?
        .to_string())
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// PHC string of an argon2 hash computed once at startup. Used by login flows
/// to keep the verify-time constant whether the userid existed or not.
/// The previous hard-coded `…$dummy` string was not valid base64 and caused
/// `PasswordHash::new` to fail in microseconds, leaking username existence.
pub fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("openexhibit-dummy-password-not-used")
            .expect("argon2 hash of constant should succeed")
    })
}

pub fn login(session: &Session, user_id: i32) -> anyhow::Result<()> {
    // Rotate CSRF on auth-state transition so a pre-login token can't be replayed.
    session.remove(crate::csrf::SESSION_KEY);
    session.insert(SESSION_USER_KEY, user_id)?;
    Ok(())
}

pub fn logout(session: &Session) {
    session.purge();
}

/// On first boot, when no admin exists, provision one with a randomly
/// generated password and print credentials once to stdout. This replaces
/// the previous open `/admin/setup` form, which let any unauthenticated
/// caller claim the admin slot if they reached the binding ahead of the
/// operator.
///
/// The credentials are emitted to stdout (not the structured logger) so
/// `journalctl -u openexhibit` and `install.sh` can scrape them on
/// first-boot, and so they survive even if tracing is filtered.
pub async fn ensure_first_admin(pool: &PgPool) -> anyhow::Result<()> {
    if User::admin_exists(pool).await? {
        return Ok(());
    }
    let password = generate_temporary_password();
    let hash = hash_password(&password)?;
    User::insert(pool, "admin", &hash, "", "", "", true).await?;

    // Single block, single newline-terminated lines. install.sh's journal
    // grep matches on "admin" + ("password" or "created").
    eprintln!();
    eprintln!("================================================================");
    eprintln!("OpenExhibit: created initial admin account");
    eprintln!("  username : admin");
    eprintln!("  password : {password}");
    eprintln!("Change it immediately at /admin/users after logging in.");
    eprintln!("================================================================");
    eprintln!();
    Ok(())
}

fn generate_temporary_password() -> String {
    // 24 chars from an unambiguous alphabet (no 0/O/I/l/1) = ~141 bits.
    use rand::Rng;
    const ALPHA: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| ALPHA[rng.gen_range(0..ALPHA.len())] as char)
        .collect()
}

/// Returns the active admin user, or an HTTP redirect to /admin/login.
pub async fn require_admin(session: &Session, pool: &PgPool) -> Result<User, HttpResponse> {
    let Ok(Some(id)) = session.get::<i32>(SESSION_USER_KEY) else {
        return Err(redirect_to_login());
    };
    match User::find_by_id(pool, id).await {
        Ok(Some(u)) if u.is_active && u.is_admin => Ok(u),
        _ => {
            session.purge();
            Err(redirect_to_login())
        }
    }
}

fn redirect_to_login() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/admin/login"))
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_hash_is_valid_phc() {
        // Regression for the prior "$argon2id$…$dummy" bug: the dummy must
        // parse and verify-time must dominate, not fail-fast.
        assert!(argon2::PasswordHash::new(dummy_hash()).is_ok());
    }

    #[test]
    fn dummy_hash_verify_returns_false() {
        assert!(!verify_password("wrong-password", dummy_hash()));
    }

    #[test]
    fn generated_password_has_expected_shape() {
        let p = generate_temporary_password();
        assert_eq!(p.len(), 24);
        // No ambiguous chars.
        for c in p.chars() {
            assert!(!"0OIl1".contains(c), "ambiguous char {c} in {p}");
        }
    }
}
