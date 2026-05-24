// SMTP relay is configured via the admin settings row (table `settings`).
// If smtp_host is empty, the message is written to stdout instead — useful
// for development and self-hosted installs without an SMTP relay yet.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::{AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;

use crate::crypto;
use crate::models::settings::Settings;

pub async fn send(
    pool: &PgPool,
    session_key: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let cfg = Settings::load(pool).await?;

    if cfg.smtp_host.is_empty() {
        println!("=== MAIL (no SMTP configured) ===");
        println!("  To:      {to}");
        println!("  Subject: {subject}");
        println!("---");
        println!("{body}");
        println!("=== end ===");
        return Ok(());
    }

    let from = if cfg.smtp_from.is_empty() {
        "noreply@localhost".to_string()
    } else {
        cfg.smtp_from
    };
    let port = u16::try_from(cfg.smtp_port).unwrap_or(587);

    let message = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())?;

    let mut builder =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)?.port(port);
    if !cfg.smtp_user.is_empty() {
        let plain = crypto::decrypt(&cfg.smtp_pass, session_key)?;
        builder = builder.credentials(Credentials::new(cfg.smtp_user, plain));
    }
    let mailer = builder.build();
    mailer.send(message).await?;
    Ok(())
}
