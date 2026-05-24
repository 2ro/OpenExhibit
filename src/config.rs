use std::collections::HashSet;
use std::env;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub session_key: String,
    pub files_dir: String,
    pub static_dir: String,
    pub cookie_secure: bool,
    /// IPs whose `X-Forwarded-For` header is trusted. Loopback is always trusted.
    pub trusted_proxies: HashSet<IpAddr>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let session_key = env::var("SESSION_KEY")
            .map_err(|_| anyhow::anyhow!("SESSION_KEY env var required (>=64 bytes)"))?;
        validate_session_key(&session_key)?;

        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
        let cookie_secure = env::var("COOKIE_SECURE")
            .ok()
            .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");

        let allow_insecure = env::var("ALLOW_INSECURE_HTTP")
            .ok()
            .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");
        guard_insecure_bind(&bind_addr, cookie_secure, allow_insecure)?;

        let trusted_proxies = parse_trusted_proxies(env::var("TRUSTED_PROXIES").ok().as_deref());

        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL env var required"))?,
            bind_addr,
            session_key,
            files_dir: env::var("FILES_DIR").unwrap_or_else(|_| "./files".into()),
            static_dir: env::var("STATIC_DIR").unwrap_or_else(|_| "./static".into()),
            cookie_secure,
            trusted_proxies,
        })
    }
}

fn validate_session_key(key: &str) -> anyhow::Result<()> {
    if key.len() < 64 {
        anyhow::bail!("SESSION_KEY must be at least 64 bytes");
    }
    // Reject obviously low-entropy keys (all-same byte, alternating two bytes).
    let distinct: HashSet<u8> = key.bytes().collect();
    if distinct.len() < 8 {
        anyhow::bail!(
            "SESSION_KEY appears low-entropy ({} distinct bytes). \
             Generate with: openssl rand -hex 64",
            distinct.len()
        );
    }
    Ok(())
}

fn guard_insecure_bind(bind_addr: &str, cookie_secure: bool, allow: bool) -> anyhow::Result<()> {
    if cookie_secure || allow {
        return Ok(());
    }
    let Ok(socket): Result<SocketAddr, _> = bind_addr.parse() else {
        return Ok(()); // Hostname binds (e.g. "localhost:8080") — let the OS resolve.
    };
    if socket.ip().is_loopback() {
        return Ok(());
    }
    anyhow::bail!(
        "refusing to bind {bind_addr} with COOKIE_SECURE=false: session cookie \
         would travel in cleartext. Either put OpenExhibit behind TLS (Caddy/nginx) \
         and set COOKIE_SECURE=true, bind to 127.0.0.1, or — only for trusted local \
         networks — set ALLOW_INSECURE_HTTP=true."
    )
}

fn parse_trusted_proxies(raw: Option<&str>) -> HashSet<IpAddr> {
    let mut set: HashSet<IpAddr> = ["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()]
        .into_iter()
        .collect();
    let Some(raw) = raw else {
        return set;
    };
    for item in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match item.parse::<IpAddr>() {
            Ok(ip) => {
                set.insert(ip);
            }
            Err(e) => {
                tracing::warn!(value = %item, error = %e, "ignoring invalid TRUSTED_PROXIES entry");
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_length() {
        assert!(validate_session_key("short").is_err());
        assert!(validate_session_key(&"a".repeat(63)).is_err());
    }

    #[test]
    fn session_key_low_entropy_rejected() {
        // 64 bytes of 'a' has length but only 1 distinct byte.
        assert!(validate_session_key(&"a".repeat(64)).is_err());
        // 64 bytes alternating between 2 values is also rejected.
        let s = "ab".repeat(32);
        assert_eq!(s.len(), 64);
        assert!(validate_session_key(&s).is_err());
    }

    #[test]
    fn session_key_real_hex_accepted() {
        // 64 random-ish hex chars (16 distinct bytes from the hex alphabet).
        let s = "0123456789abcdef".repeat(4);
        assert_eq!(s.len(), 64);
        assert!(validate_session_key(&s).is_ok());
    }

    #[test]
    fn loopback_bind_ok_without_tls() {
        assert!(guard_insecure_bind("127.0.0.1:8080", false, false).is_ok());
        assert!(guard_insecure_bind("[::1]:8080", false, false).is_ok());
    }

    #[test]
    fn public_bind_requires_tls_or_override() {
        assert!(guard_insecure_bind("0.0.0.0:8080", false, false).is_err());
        assert!(guard_insecure_bind("192.168.1.5:8080", false, false).is_err());
        // Either flag clears it.
        assert!(guard_insecure_bind("0.0.0.0:8080", true, false).is_ok());
        assert!(guard_insecure_bind("0.0.0.0:8080", false, true).is_ok());
    }

    #[test]
    fn trusted_proxies_default_loopback() {
        let set = parse_trusted_proxies(None);
        assert!(set.contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(set.contains(&"::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn trusted_proxies_parse() {
        let set = parse_trusted_proxies(Some("10.0.0.1, 10.0.0.2, garbage"));
        assert!(set.contains(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(set.contains(&"10.0.0.2".parse::<IpAddr>().unwrap()));
    }
}
