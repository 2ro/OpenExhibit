# Configuration

OpenExhibit reads two kinds of configuration:

1. **Environment variables** — loaded once at startup, parsed by
   `config::Config::from_env` in `src/config.rs`. Some are required;
   missing-but-required values cause the binary to refuse to start.
2. **The `settings` row** — a singleton at `id = 1` in the `settings`
   table, managed through `/admin/settings`. Covers display name,
   theme colors, SMTP credentials, custom CSS, and the greentext
   toggle. See [`database-schema.md`](database-schema.md) for the
   full column inventory.

This page covers (1). The admin-UI surface for (2) is the live
source of truth.

> Generated from source. Re-run `scripts/gen-docs.sh` whenever
> `src/config.rs` or `.env.example` changes.

## Environment variables

| Variable | Type | Default | Required | Effect |
|----------|------|---------|----------|--------|
| `DATABASE_URL` | string (PostgreSQL URL) | — | **yes** | Connection string passed to `sqlx`. Pool is sized at up to 16 connections with a 5-second acquire timeout (`src/db.rs::init`). Missing → startup fails with `"DATABASE_URL env var required"`. |
| `SESSION_KEY` | string (≥64 bytes, ≥8 distinct bytes) | — | **yes** | Symmetric key for `actix-session` cookie encryption and the AEAD key used to encrypt `settings.smtp_pass` at rest. Validated by `config::validate_session_key`. Generate with `openssl rand -hex 64`. Rotating invalidates existing sessions and AEAD ciphertexts — re-enter the SMTP password in the admin after rotation. |
| `BIND_ADDR` | `host:port` | `127.0.0.1:8080` | no | What the HTTP server binds. Hostnames are passed through to the OS resolver. Loopback binds are always permitted; non-loopback binds require `COOKIE_SECURE=true` or `ALLOW_INSECURE_HTTP=true`. |
| `COOKIE_SECURE` | bool (`true`/`1`) | `false` | no | Sets the session cookie's `Secure` flag. Set this to `true` in production behind TLS. The app refuses to start if `BIND_ADDR` parses as a non-loopback socket address and neither `COOKIE_SECURE` nor `ALLOW_INSECURE_HTTP` is set. |
| `ALLOW_INSECURE_HTTP` | bool (`true`/`1`) | `false` | no | Bypasses the bind check for trusted local networks. The safer fix is to terminate TLS in front (Caddy / nginx) and set `COOKIE_SECURE=true`. |
| `FILES_DIR` | path | `./files` | no | Root for uploaded media. Originals live at `gimgs/{ref_id}/{file}`; lazy derivatives at `dimgs/{ref_id}/{shape}_{size}_{file}`. Created lazily by the upload handler. |
| `STATIC_DIR` | path | `./static` | no | Root for SCSS-compiled CSS, fonts, icons. Served as `GET /static/*`. |
| `TRUSTED_PROXIES` | comma-separated IPs | (loopback only) | no | Reverse-proxy IPs whose `X-Forwarded-For` header is honored when logging hits (`src/stats.rs`) and rate-limiting (`src/ratelimit.rs`). `127.0.0.1` and `::1` are always trusted. Invalid entries log a warning and are ignored. Untrusted XFF is dropped so a hostile client can't poison `stats.addr` with arbitrary IPs. |
| `BASE_URL` | URL | `http://localhost:8080` | no | Public origin used to build password-reset links in `routes::admin::auth::forgot_post`. Read directly from `env::var` at email-send time, not stored on `Config`. |
| `RUST_LOG` | tracing filter | `openexhibit=info,actix_web=info` | no | Filter string for `tracing_subscriber::EnvFilter`. Defaults applied by `src/main.rs::main` when unset. |

### The bind guard

`config::guard_insecure_bind` enforces:

- Loopback (`127.0.0.1`, `::1`) — always allowed.
- Other parsed `SocketAddr` — requires `COOKIE_SECURE=true` or
  `ALLOW_INSECURE_HTTP=true`.
- Hostname binds (e.g. `localhost:8080`) — not parsed as a socket
  address, so the guard does not run; the OS resolves the hostname
  at bind time.

### The session-key validator

`config::validate_session_key` rejects keys shorter than 64 bytes or
with fewer than 8 distinct byte values. Unit tests live alongside the
function in `src/config.rs::tests`.

## Sample `.env`

Pulled verbatim from `.env.example`:

```env
DATABASE_URL=postgres://openexhibit:openexhibit@localhost:5432/openexhibit
BIND_ADDR=127.0.0.1:8080
SESSION_KEY=change-me-to-a-64-byte-hex-string-generate-with-openssl-rand-hex-32
RUST_LOG=openexhibit=debug,actix_web=info,sqlx=warn
FILES_DIR=./files
STATIC_DIR=./static
# Set to true when behind TLS (HTTPS) in production. The app refuses to start
# if BIND_ADDR is a public address and COOKIE_SECURE is false (set
# ALLOW_INSECURE_HTTP=true to override — only for trusted networks).
COOKIE_SECURE=false
# Public base URL used in password-reset emails.
BASE_URL=http://localhost:8080

# Comma-separated list of IPs (your reverse proxy) whose `X-Forwarded-For` is
# trusted. Loopback (127.0.0.1, ::1) is always trusted. Untrusted XFF is ignored
# so clients can't poison the hit-log with arbitrary IPs.
# TRUSTED_PROXIES=10.0.0.1

# SMTP (set via the admin UI: /admin/settings → "SMTP for password reset").
# Stored encrypted at rest using a key derived from SESSION_KEY.
```

## Things you used to set in env but now set in the admin

| Setting | Where | Notes |
|---------|-------|-------|
| SMTP host/port/user/password/from | `/admin/settings` → "SMTP for password reset" | Password is AEAD-encrypted at rest (`src/crypto.rs`), keyed off `SESSION_KEY`. |
| Site name / display name (`obj_name`, `site_name`) | `/admin/settings` | Substituted into `obj_itop`/`obj_ibot`. |
| Sidebar top/bottom HTML (`obj_itop`, `obj_ibot`) | `/admin/settings` | Goes through the markup pipeline; supports Markdown + BBCode + sanitized HTML. |
| Site-wide custom CSS (`custom_css`) | `/admin/settings` | Rendered inline in `<head>` before any per-exhibit `custom_css`. |
| Theme text/background colors (`theme_text_color`, `theme_bg_color`) | `/admin/settings` | HTML5 color pickers; values normalized to `#rrggbb` or empty. |
| Greentext toggle (`enable_greentext`) | `/admin/settings` | When on, `>` lines render as `<p class="greentext">` instead of Markdown blockquotes. |
| Site language (`site_lang`) | `/admin/settings` | Used by `<html lang>` and the (currently single-locale) i18n table. |

## Runtime constants worth knowing

These aren't env-tunable, but they affect behavior:

| Constant | Source | Value |
|----------|--------|-------|
| Session cookie name | `src/main.rs` | `ndxz_session` |
| Session lifetime | `src/main.rs` | 7 days (persistent). |
| Login rate limit | `src/routes/admin/auth.rs` | 10 attempts / 60 s per IP. |
| Forgot rate limit | `src/routes/admin/auth.rs` | 5 attempts / 300 s per IP. |
| Reset rate limit | `src/routes/admin/auth.rs` | 10 attempts / 60 s per IP. |
| Public unlock rate limit | `src/routes/public.rs` | 8 attempts / 60 s per IP. |
| Per-file upload cap | `src/routes/admin/media.rs` | 50 MiB. |
| Total upload cap per request | `src/routes/admin/media.rs` | 250 MiB. |
| Derivative size range | `src/routes/admin/media.rs` | `16..=4096` pixels. |
| DB pool size | `src/db.rs` | 16 connections. |
| DB acquire timeout | `src/db.rs` | 5 s. |
| Minimum password length | `src/routes/admin/{auth,users}.rs` | 10 characters. |
| First-admin password length | `src/auth.rs` | 24 chars, alphabet excludes `0/O/I/l/1`. |
| First-admin password entropy | `src/auth.rs` | ~141 bits. |
