# Architecture

OpenExhibit is a single-binary, server-rendered portfolio CMS. It binds
one Actix-Web HTTP server, talks to one PostgreSQL database, and renders
every page on the server with Askama templates. There is no JavaScript
in the served UI — features that elsewhere depend on JS (autoplaying
slideshows, lazy thumbnails, lightboxes) use SCSS-only behavior or
on-demand server-side generation.

> Generated from source. Re-run `scripts/gen-docs.sh` after substantive
> changes to `src/main.rs`, `src/routes/`, `src/formats/mod.rs`, or
> `src/models/`.

## Process model

`src/main.rs` is the entry point. On startup it:

1. Loads `.env` via `dotenvy` and initializes `tracing_subscriber`.
2. Builds a `config::Config` from the environment (`config::Config::from_env`).
3. Opens a PostgreSQL pool and runs `sqlx::migrate!("./migrations")`
   (`src/db.rs`).
4. Calls `auth::ensure_first_admin(&pool)` — provisions an `admin` user
   on a fresh database and prints a randomly generated password to
   stdout once.
5. Launches `HttpServer` with the middleware stack and route
   configurators described below.

## Middleware stack (outermost → innermost)

Constructed in `src/main.rs::main`:

| Layer | Purpose |
|-------|---------|
| `TracingLogger::default()` | Per-request structured logs. |
| `DefaultHeaders` | Sets `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, a strict `Content-Security-Policy`, and `Permissions-Policy: interest-cohort=()`. |
| `SessionMiddleware` | `actix-session` with `CookieSessionStore`. Cookie name `ndxz_session`, HTTP-only, `SameSite=Lax`, `Secure` driven by `COOKIE_SECURE`, 7-day persistent lifetime. |
| `app_data` | Shares `Config`, `PgPool`, and `RateLimiter` with handlers. |
| Route configurators | The lazy derivative endpoint (`/files/dimgs/...`) is registered ahead of the static `Files` handler so it wins the match. Public `/{path:.*}` is registered last so admin and static routes resolve first. |

Admin routes get an inner `DefaultHeaders` layer
(`src/routes/admin/mod.rs`) that sets `Cache-Control: private, no-store`
and `Pragma: no-cache` so CSRF-token pages don't land in shared caches.

## Request lifecycle

### Public exhibit (`GET /some/exhibit/`)

1. `routes::public::catch_all` matches via `#[get("/{path:.*}")]`.
2. Loads `Settings`, then looks up the exhibit by URL
   (`Exhibit::find_by_url`). The lookup tries `/{path}/` then `/{path}`.
3. `stats::log_hit` spawns a fire-and-forget INSERT into `stats`.
4. The format's `intercept` hook runs. `external_link` returns a 302;
   other formats fall through.
5. `gated_render` checks `exhibit.password`. If non-empty and the
   session doesn't carry `unlocked_exhibit_{id} = true`, the password
   gate template is returned.
6. `render_exhibit` loads media, builds the nav via `build_nav`,
   assembles `BaseFields`, and calls
   `formats::render(&exhibit, &media, base, settings.enable_greentext)`.
7. `formats::render`:
   - Maps each `Media` row to a `MediaView` with file/thumb URLs.
   - Calls `wire_lightbox_links` so lightbox prev/next is computed
     server-side.
   - Pushes `exhibit.content` through the markup pipeline.
   - Dispatches to the format's `render`, which renders an Askama
     template under `templates/public/formats/<key>.html`.

### Public password gate (`POST /some/exhibit/`)

`routes::public::unlock_post`:

1. CSRF token verified via `csrf::verify`.
2. Rate-limited per peer-IP (bucket `unlock`, 8 attempts / 60 s).
3. Password verified against `exhibits.password` (argon2 PHC via
   `auth::verify_password`).
4. On success, `unlocked_exhibit_{id} = true` is stored in the session.

### Admin request

Every admin handler calls `auth::require_admin(&session, &pool)` first.
It reads `user_id` from the session and confirms the user is active
and an admin. Failure returns a 302 to `/admin/login`. Mutating
handlers then call `csrf::verify` before touching the database.

### Synthetic tag page (`GET /tag/{name}`)

`routes::public::tag_get` runs one query joining `exhibits`, `tagged`,
and `tags`, fetches a thumbnail per result, and renders
`templates/public/tag.html`. Reuses `BaseFields` so the nav and theme
bar match the rest of the public site.

## Session, CSRF, auth

- **Session storage** — encrypted cookie via `actix-session`'s
  `CookieSessionStore`. Key is `Config.session_key`
  (env `SESSION_KEY`, validated by `config::validate_session_key`).
- **Cookie attributes** — name `ndxz_session`, `HttpOnly`,
  `SameSite=Lax`, `Secure` iff `COOKIE_SECURE=true`. The app refuses
  to start when a non-loopback bind is paired with
  `COOKIE_SECURE=false` unless `ALLOW_INSECURE_HTTP=true` is set.
- **CSRF** — double-submit using the session as the secret store.
  `csrf::get_or_create` lazily creates a 32-byte URL-safe token;
  templates render it as a hidden `_csrf` input; `csrf::verify`
  constant-time-compares the submitted value against the session.
- **Password hashing** — argon2id PHC strings via `auth::hash_password`.
  `auth::dummy_hash()` returns a process-lifetime cached real PHC,
  used during login to keep verify-time uniform.
- **First admin** — `auth::ensure_first_admin` provisions on first
  boot with a 24-char password from an unambiguous alphabet
  (no `0/O/I/l/1`). Printed to stderr in a single banner.
- **Password reset** — `/admin/forgot` writes a 32-byte token and
  1-hour expiry into `users.reset_token` / `reset_expires`, then
  sends an email via `mail::send`. Lookups are constant-time across
  all unexpired candidates.
- **Rate limiting** — `src/ratelimit.rs::RateLimiter`, in-process,
  per-IP, sliding window. Buckets: `login` (10/60 s), `forgot`
  (5/300 s), `reset` (10/60 s), `unlock` (8/60 s). Peer IP comes
  from `ratelimit::peer_ip` which only honors `X-Forwarded-For`
  when the immediate peer is in `Config.trusted_proxies`.

## Models and persistence

`src/models/` mirrors the database with `sqlx::FromRow` structs.
Column provenance and types are documented in
[`database-schema.md`](database-schema.md).

| Model | Source | Notes |
|-------|--------|-------|
| `Exhibit` | `models/exhibit.rs` | One row per public page. Loaders: `find_home`, `find_by_url`, `list_for_section`, `list_top_of_each_section`. |
| `Media` | `models/media.rs` | One row per uploaded file. `MediaListRow` is a thinner join used by the cross-exhibit admin browser. |
| `Section` | `models/section.rs` | Top-level nav buckets. `hide_title` lets a section render as a bare child list. |
| `Settings` | `models/settings.rs` | Singleton at `id = 1`. Carries SMTP, theme colors, custom CSS, greentext toggle. |
| `Tag` | `models/tag.rs` | One row per tag. The `tagged` join table has no struct — handlers pivot it directly. |
| `User` | `models/user.rs` | argon2 PHC in `password_hash`. Reset token + expiry are nullable. |

Pool initialization (`db::init`) opens up to 16 connections with a
5-second acquire timeout and runs all migrations at startup.

## Format registry

The exhibit-format abstraction lives in `src/formats/mod.rs`. Each
format is a unit struct implementing `ExhibitFormat`, with a stable
`key()`, a `display_name()` and `description()` shown in the admin
picker, an optional `capabilities()` that hides irrelevant admin
fields, and three behavior hooks:

| Hook | Default | When overridden |
|------|---------|-----------------|
| `intercept(&Exhibit) -> Option<HttpResponse>` | `None` | `external_link` returns a 302 to its `link` column. |
| `nav_href(&Exhibit) -> NavHref` | Internal URL, same tab | `external_link` returns the external URL with `open_in_new_tab = exhibit.link_target`. |
| `render(...)` | Required | Renders a per-format Askama template under `templates/public/formats/<key>.html`. |

The registry is a `&'static [&'static dyn ExhibitFormat]` slice named
`FORMATS`. `formats::find(key)` looks up by stored key and falls back
to `visual_index` for unknown values, so a hand-edited or stale row
still renders. `formats::registry()` returns the whole slice (the admin
picker uses this so adding a format requires no UI changes).

The shipped formats and the contributor walkthrough live in
[`exhibit-formats.md`](exhibit-formats.md); registry invariants are
checked by `src/formats/mod.rs::tests`.

## Markup pipeline (admin → public)

`src/markup.rs::render_with` is the only path admin-authored text
takes on its way to a public page. It runs on `exhibits.content`,
`media.caption`, `settings.obj_itop`, and `settings.obj_ibot`. The
pipeline:

1. **Greentext pre-pass** (optional, gated by
   `settings.enable_greentext`). Lines starting with `>` become
   `<p class="greentext">…</p>`. Off → `>` becomes a Markdown
   blockquote.
2. **BBCode pre-pass** (`bbcode_to_html`). Converts `[b]`, `[i]`,
   `[u]`, `[s]`, `[url=…]`, `[url]`, `[img]`, `[quote]`, `[code]`,
   `[hr]`, and `[list]…[*]…[/list]` (ordered when
   `[list=1|a|A|i|I]`) to inline HTML.
3. **CommonMark** via `pulldown-cmark` with tables, strikethrough,
   tasklists, and smart punctuation enabled. Embedded HTML passes
   through.
4. **Sanitization** via `ammonia` with a cached allowlist that
   expands the default set with `s`, `u`, `mark`, `kbd`, `sub`,
   `sup`, `del`, `ins`. `class` is allowed on
   `span`/`div`/`p`/`code`/`pre` so pulldown-cmark's fenced-code
   `class="language-rust"` survives. Scripts, event handlers,
   `javascript:` URLs, `<iframe>`, `<object>` are stripped.

Empty input returns empty output. Unit tests in
`src/markup.rs::tests` cover BBCode variants, greentext, raw-HTML
passthrough, and sanitizer behavior.

## Lazy derivative image system

Originals live at `$FILES_DIR/gimgs/{ref_id}/{file}`. Derivatives
(thumbnails, cropped shapes) live at
`$FILES_DIR/dimgs/{ref_id}/{shape}_{size}_{file}` and are generated
on first request by `routes/admin/media.rs::derivative_get`, mounted
at `GET /files/dimgs/{ref_id}/{filename}` ahead of the static `Files`
handler.

Pipeline (`src/images.rs`):

- **Shapes** (`Shape::parse`): `proportional`, `square`, `four_three`,
  `three_two`, `cinematic`. `target_dims` computes the output box;
  `proportional` preserves aspect ratio, others use `resize_to_fill`
  with Lanczos3 filtering.
- **EXIF orientation** — `load_oriented` re-reads the file's magic
  bytes (so tempfiles without an extension still decode) and applies
  the EXIF Orientation tag (1–8).
- **Upload** (`routes/admin/media.rs::save_one`) — magic-byte sniffs
  the MIME, SHA-256-hashes the raw bytes, skips writes if the same
  hash already exists in this exhibit (`media.sha256` dedupe),
  re-encodes images through the `image` crate, and stores
  width/height + size in `media`. Per-file cap 50 MiB, total per
  request 250 MiB.
- **Derivative request** — `derivative_get` parses
  `{shape}_{size}_{name}`, rejects path traversal, clamps `size`
  to `16..=4096`, then delegates to `images::ensure_derivative`.
  Responses get `Cache-Control: public, max-age=31536000, immutable`.
- **Delete** — when a media row is deleted, the original in `gimgs/`
  and every matching derivative in `dimgs/` are removed.
  `is_derivative_of` matches the precise `{shape}_{size}_{file}`
  pattern.

## Encryption at rest

`src/crypto.rs` provides `encrypt` / `decrypt` over
ChaCha20-Poly1305, keyed by
`SHA-256("openexhibit/v1/smtp-password" || SESSION_KEY)`. Today the
only consumer is the SMTP relay password in `settings.smtp_pass`
(encrypted in `routes/admin/settings.rs`, decrypted at send-time by
`mail::send`). Ciphertexts carry an `enc:` prefix; legacy plaintext
rows pass through unmodified. Rotating `SESSION_KEY` invalidates
these ciphertexts.

## Error model

`src/error.rs::AppError` carries the typical 404/403/400 cases plus
`Db`, `Template`, `Io`, `Image`, and `Internal(anyhow)`.
`ResponseError` is implemented so handlers can `?`-propagate.
Internal errors log via `tracing::error!`. Every error renders
through `templates/public/error.html`.

## Static assets and uploads

| Mount | Source | Notes |
|-------|--------|-------|
| `/static/*` | `Config.static_dir` (default `./static`) | SCSS-compiled CSS, fonts, icons. |
| `/files/gimgs/{ref_id}/{file}` | `Config.files_dir/gimgs/...` | Originals. |
| `/files/dimgs/{ref_id}/{shape}_{size}_{file}` | `Config.files_dir/dimgs/...` | Lazy derivatives (see above). Wired ahead of `Files` in `main.rs`. |

SCSS is compiled at build time by `build.rs` — there is no Sass
runtime in the binary.
