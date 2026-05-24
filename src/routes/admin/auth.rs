use std::env;
use std::time::Duration;

use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use askama::Template;
use base64::Engine;
use rand::Rng;

use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::config::Config;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::mail;
use crate::models::user::User;
use crate::ratelimit::{self, RateLimiter};

// Per-IP limits. Generous enough not to lock out legitimate users, tight
// enough to take the edge off online brute-force. Operators are still
// encouraged to add a reverse-proxy limit on top.
const LOGIN_MAX_PER_WINDOW: u32 = 10;
const LOGIN_WINDOW: Duration = Duration::from_secs(60);
const FORGOT_MAX_PER_WINDOW: u32 = 5;
const FORGOT_WINDOW: Duration = Duration::from_secs(60 * 5);
const RESET_MAX_PER_WINDOW: u32 = 10;
const RESET_WINDOW: Duration = Duration::from_secs(60);

#[derive(Template)]
#[template(path = "admin/login.html")]
struct LoginPage {
    csrf_token: String,
    error: Option<String>,
}

#[derive(Deserialize)]
struct LoginForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    userid: String,
    password: String,
}

async fn login_get(session: Session) -> AppResult<HttpResponse> {
    let csrf_token = csrf::get_or_create(&session)?;
    let html = LoginPage {
        csrf_token,
        error: None,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn login_post(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
    rl: web::Data<RateLimiter>,
    form: web::Form<LoginForm>,
) -> AppResult<HttpResponse> {
    if !csrf::verify(&session, &form.csrf) {
        return Ok(HttpResponse::Forbidden().body("Invalid CSRF token"));
    }
    if !allow_attempt(&req, &rl, "login", LOGIN_MAX_PER_WINDOW, LOGIN_WINDOW) {
        return Ok(too_many_requests());
    }

    let user_opt = User::find_by_userid(pool.get_ref(), &form.userid).await?;
    let render_error = |session: &Session| -> AppResult<HttpResponse> {
        let csrf_token = csrf::get_or_create(session)?;
        let html = LoginPage {
            csrf_token,
            error: Some("Invalid username or password".into()),
        }
        .render()?;
        Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html))
    };

    let user = match user_opt {
        Some(u) if u.is_active && u.is_admin => u,
        _ => {
            // Hash against a real PHC string to keep verify-time constant
            // whether the userid existed or not (mitigates username enumeration).
            let _ = auth::verify_password(&form.password, auth::dummy_hash());
            return render_error(&session);
        }
    };

    if !auth::verify_password(&form.password, &user.password_hash) {
        return render_error(&session);
    }

    auth::login(&session, user.id).map_err(crate::error::AppError::Internal)?;
    Ok(HttpResponse::Found()
        .append_header(("Location", "/admin"))
        .finish())
}

#[derive(Deserialize)]
struct LogoutForm {
    #[serde(rename = "_csrf")]
    csrf: String,
}

async fn logout_post(session: Session, form: web::Form<LogoutForm>) -> impl Responder {
    if csrf::verify(&session, &form.csrf) {
        auth::logout(&session);
    }
    HttpResponse::Found()
        .append_header(("Location", "/admin/login"))
        .finish()
}

// ─── Password recovery ───────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin/forgot.html")]
struct ForgotPage {
    csrf_token: String,
    message: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/reset.html")]
struct ResetPage {
    csrf_token: String,
    token: String,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ForgotForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    email: String,
}

#[derive(Deserialize)]
struct ResetForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    password: String,
    confirm: String,
}

async fn forgot_get(session: Session) -> AppResult<HttpResponse> {
    let html = ForgotPage {
        csrf_token: csrf::get_or_create(&session)?,
        message: None,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn forgot_post(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
    cfg: web::Data<Config>,
    rl: web::Data<RateLimiter>,
    form: web::Form<ForgotForm>,
) -> AppResult<HttpResponse> {
    if !csrf::verify(&session, &form.csrf) {
        return Ok(HttpResponse::Forbidden().body("Invalid CSRF token"));
    }
    if !allow_attempt(&req, &rl, "forgot", FORGOT_MAX_PER_WINDOW, FORGOT_WINDOW) {
        return Ok(too_many_requests());
    }

    let user_opt: Option<User> = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND is_active = TRUE LIMIT 1",
    )
    .bind(&form.email)
    .fetch_optional(pool.get_ref())
    .await?;

    // Always render the same generic response to avoid email enumeration.
    let message = "If that email is registered, a reset link has been sent.".to_string();

    if let Some(user) = user_opt {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let expires = chrono::Utc::now() + chrono::Duration::hours(1);

        sqlx::query("UPDATE users SET reset_token = $1, reset_expires = $2 WHERE id = $3")
            .bind(&token)
            .bind(expires)
            .bind(user.id)
            .execute(pool.get_ref())
            .await?;

        let base = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
        let link = format!("{base}/admin/reset/{token}");
        let body = format!(
            "Hello,\n\nA password reset was requested for your OpenExhibit admin account.\n\nUse this link within one hour:\n\n  {link}\n\nIf you did not request this, ignore this message.\n"
        );
        let to = user.email.clone();
        let pool_for_mail = pool.get_ref().clone();
        let key = cfg.session_key.clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = mail::send(
                &pool_for_mail,
                &key,
                &to,
                "OpenExhibit password reset",
                &body,
            )
            .await
            {
                tracing::warn!(error = %e, "reset email failed");
            }
        });
    }

    let html = ForgotPage {
        csrf_token: csrf::get_or_create(&session)?,
        message: Some(message),
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn reset_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let token = path.into_inner();
    let valid = lookup_reset_user(pool.get_ref(), &token).await?.is_some();
    let error = if valid {
        None
    } else {
        Some("Reset link expired or invalid.".to_string())
    };
    let html = ResetPage {
        csrf_token: csrf::get_or_create(&session)?,
        token,
        error,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn reset_post(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
    rl: web::Data<RateLimiter>,
    path: web::Path<String>,
    form: web::Form<ResetForm>,
) -> AppResult<HttpResponse> {
    if !csrf::verify(&session, &form.csrf) {
        return Ok(HttpResponse::Forbidden().body("Invalid CSRF token"));
    }
    if !allow_attempt(&req, &rl, "reset", RESET_MAX_PER_WINDOW, RESET_WINDOW) {
        return Ok(too_many_requests());
    }
    let token = path.into_inner();

    if form.password != form.confirm {
        return render_reset_err(&session, &token, "Passwords do not match.");
    }
    if form.password.len() < 10 {
        return render_reset_err(&session, &token, "Password must be at least 10 characters.");
    }

    let Some(user) = lookup_reset_user(pool.get_ref(), &token).await? else {
        return render_reset_err(&session, &token, "Reset link expired or invalid.");
    };

    let hash = auth::hash_password(&form.password).map_err(AppError::Internal)?;
    sqlx::query(
        "UPDATE users SET password_hash = $1, reset_token = NULL, reset_expires = NULL WHERE id = $2",
    )
    .bind(&hash)
    .bind(user.id)
    .execute(pool.get_ref())
    .await?;

    Ok(HttpResponse::Found()
        .append_header(("Location", "/admin/login"))
        .finish())
}

async fn lookup_reset_user(pool: &PgPool, token: &str) -> AppResult<Option<User>> {
    if token.is_empty() {
        return Ok(None);
    }
    // Fetch all unexpired candidates, then compare the token in constant time.
    // In practice there will almost never be more than one outstanding token,
    // and even at scale this is a few rows.
    let candidates: Vec<User> = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE reset_token IS NOT NULL \
         AND reset_expires > now() AND is_active = TRUE",
    )
    .fetch_all(pool)
    .await?;
    for u in candidates {
        if let Some(t) = u.reset_token.as_deref() {
            if crate::csrf::constant_time_eq(t.as_bytes(), token.as_bytes()) {
                return Ok(Some(u));
            }
        }
    }
    Ok(None)
}

fn render_reset_err(session: &Session, token: &str, msg: &str) -> AppResult<HttpResponse> {
    let html = ResetPage {
        csrf_token: csrf::get_or_create(session)?,
        token: token.to_string(),
        error: Some(msg.to_string()),
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

fn allow_attempt(
    req: &HttpRequest,
    rl: &RateLimiter,
    bucket: &str,
    max: u32,
    window: Duration,
) -> bool {
    let Some(ip) = ratelimit::peer_ip(req) else {
        return true; // No peer? Don't break the world; trust the proxy guard.
    };
    rl.check(bucket, ip, max, window)
}

fn too_many_requests() -> HttpResponse {
    HttpResponse::TooManyRequests()
        .append_header(("Retry-After", "60"))
        .content_type("text/plain; charset=utf-8")
        .body("Too many attempts. Try again in a minute.")
}

// ─── Wiring ──────────────────────────────────────────────────────────────────

/// Public, unauthed entry points. Rate-limiting for login/forgot/reset is
/// handled in-process via `crate::ratelimit`; operators are still encouraged
/// to add a reverse-proxy limit on top.
pub fn configure_unauthed(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/login")
            .route(web::get().to(login_get))
            .route(web::post().to(login_post)),
    )
    .service(
        web::resource("/forgot")
            .route(web::get().to(forgot_get))
            .route(web::post().to(forgot_post)),
    )
    .service(
        web::resource("/reset/{token}")
            .route(web::get().to(reset_get))
            .route(web::post().to(reset_post)),
    );
}

pub fn configure_authed(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/logout").route(web::post().to(logout_post)));
}
