use actix_web::middleware::DefaultHeaders;
use actix_web::web;

pub mod auth;
pub mod dashboard;
pub mod exhibits;
pub mod media;
pub mod sections;
pub mod settings;
pub mod tags;
pub mod users;

// Login/forgot/reset are rate-limited in-process by `crate::ratelimit`;
// reverse proxies are still encouraged to add their own limits.

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin")
            // Admin pages contain CSRF tokens and per-user content — never
            // store them in shared caches or browser back-button history.
            .wrap(
                DefaultHeaders::new()
                    .add(("Cache-Control", "private, no-store"))
                    .add(("Pragma", "no-cache")),
            )
            .configure(auth::configure_unauthed)
            .configure(auth::configure_authed)
            .configure(dashboard::configure)
            .configure(exhibits::configure)
            .configure(media::configure)
            .configure(sections::configure)
            .configure(settings::configure)
            .configure(tags::configure)
            .configure(users::configure),
    );
}
