// One-shot flash messages stored in the encrypted session cookie.
// Typical pattern: `flash::redirect(&session, "Saved", "/admin/exhibits")` from a
// mutating handler, `flash::take(&session)` in the next GET to consume and clear.

use actix_session::Session;
use actix_web::HttpResponse;

const KEY: &str = "flash";

pub fn set(session: &Session, msg: impl Into<String>) {
    let _ = session.insert(KEY, msg.into());
}

pub fn take(session: &Session) -> Option<String> {
    let msg = session.get::<String>(KEY).ok().flatten();
    if msg.is_some() {
        session.remove(KEY);
    }
    msg
}

/// Convenience: set a flash, return a 302 to `location`.
pub fn redirect(
    session: &Session,
    msg: impl Into<String>,
    location: impl AsRef<str>,
) -> HttpResponse {
    set(session, msg);
    HttpResponse::Found()
        .append_header(("Location", location.as_ref()))
        .finish()
}
