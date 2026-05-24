// Server-side hit logging. Fire-and-forget INSERT into `stats` per public page request.
// Replaces the tracking pixel from the legacy site.

use std::collections::HashSet;
use std::net::IpAddr;
use std::str::FromStr;

use actix_web::HttpRequest;
use sqlx::PgPool;

pub fn log_hit(req: &HttpRequest, pool: &PgPool, page: &str) {
    let trusted = req
        .app_data::<actix_web::web::Data<crate::config::Config>>()
        .map_or_else(HashSet::new, |c| c.trusted_proxies.clone());
    let addr = client_ip(req, &trusted);
    let pool = pool.clone();
    let path = page.to_string();
    let referrer = req
        .headers()
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .map(|s| truncate(s, 250))
        .unwrap_or_default();
    let agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| truncate(s, 250))
        .unwrap_or_default();
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .map(|s| truncate(s, 100))
        .unwrap_or_default();

    let addr_str = addr.map(|a| a.to_string());
    actix_web::rt::spawn(async move {
        let now = chrono::Utc::now();
        let month = now.format("%Y-%m").to_string();
        let day = now.date_naive();
        // addr is bound as TEXT; Postgres parses it into INET on insert.
        if let Err(e) = sqlx::query(
            "INSERT INTO stats (addr, domain, referrer, page, agent, hit_at, hit_month, hit_day)
             VALUES ($1::inet, $2, $3, $4, $5, now(), $6, $7)",
        )
        .bind(addr_str)
        .bind(host)
        .bind(referrer)
        .bind(truncate(&path, 250))
        .bind(agent)
        .bind(month)
        .bind(day)
        .execute(&pool)
        .await
        {
            tracing::debug!(error = %e, "stats insert failed");
        }
    });
}

/// Returns the best guess of the originating client IP. `X-Forwarded-For` is
/// only honored when the immediate peer is in the trusted-proxies set —
/// otherwise any client could log arbitrary IPs into the stats table.
pub fn client_ip(req: &HttpRequest, trusted: &HashSet<IpAddr>) -> Option<IpAddr> {
    let peer = req.peer_addr().map(|s| s.ip());
    if let Some(peer_ip) = peer {
        if trusted.contains(&peer_ip) {
            if let Some(forwarded) = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| IpAddr::from_str(s.trim()).ok())
            {
                return Some(forwarded);
            }
        }
    }
    peer
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}
