use actix_files::Files;
use actix_session::storage::CookieSessionStore;
use actix_session::{config::PersistentSession, SessionMiddleware};
use actix_web::cookie::{time::Duration, Key, SameSite};
use actix_web::http::header;
use actix_web::middleware::DefaultHeaders;
use actix_web::{web, App, HttpServer};
use tracing_actix_web::TracingLogger;

mod auth;
mod config;
mod crypto;
mod csrf;
mod db;
mod error;
mod flash;
mod formats;
mod i18n;
mod images;
mod mail;
mod markup;
mod models;
mod ratelimit;
mod routes;
mod stats;
mod templates;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openexhibit=info,actix_web=info".into()),
        )
        .init();

    let cfg = config::Config::from_env()?;
    let pool = db::init(&cfg.database_url).await?;
    let session_key = Key::from(cfg.session_key.as_bytes());

    // First-boot: provision the initial admin user and print one-time
    // credentials to stdout. Replaces the previous anonymous /admin/setup form,
    // which was a remote-exploitable land-grab when the app was exposed on a
    // non-loopback bind before any operator had visited the page.
    auth::ensure_first_admin(&pool).await?;

    tracing::info!(addr = %cfg.bind_addr, "openexhibit starting");

    let bind_addr = cfg.bind_addr.clone();
    let files_dir = cfg.files_dir.clone();
    let static_dir = cfg.static_dir.clone();
    let cookie_secure = cfg.cookie_secure;
    let cfg_data = web::Data::new(cfg);
    let pool_data = web::Data::new(pool);
    let ratelimit_data = web::Data::new(ratelimit::RateLimiter::new());

    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .wrap(
                DefaultHeaders::new()
                    .add((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
                    .add((
                        "Content-Security-Policy",
                        "default-src 'self'; \
                         script-src 'self'; script-src-attr 'none'; \
                         img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
                         media-src 'self'; object-src 'none'; \
                         frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
                    ))
                    .add(("Permissions-Policy", "interest-cohort=()")),
            )
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .cookie_name("ndxz_session".to_string())
                    .cookie_http_only(true)
                    .cookie_same_site(SameSite::Lax)
                    .cookie_secure(cookie_secure)
                    .session_lifecycle(PersistentSession::default().session_ttl(Duration::days(7)))
                    .build(),
            )
            .app_data(cfg_data.clone())
            .app_data(pool_data.clone())
            .app_data(ratelimit_data.clone())
            // Lazy image derivative endpoint must beat the static Files handler.
            .configure(routes::admin::media::configure_public)
            .configure(routes::admin::configure)
            .service(Files::new("/static", &static_dir))
            .service(Files::new("/files", &files_dir))
            // Public catch-all goes last (it routes any /{path}).
            .configure(routes::public::configure)
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
