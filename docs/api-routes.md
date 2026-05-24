# API routes

Every HTTP endpoint OpenExhibit serves, derived from the `#[get]` /
`#[post]` macros and `web::resource` registrations in `src/routes/`.
Admin routes are mounted under the `/admin` scope and require an
active admin session via `auth::require_admin` unless noted otherwise.

> Generated from source. Re-run `scripts/gen-docs.sh` after adding,
> removing, or renaming a route.

Legend:

- **Auth: admin** — calls `auth::require_admin`; redirects to
  `/admin/login` if the session isn't an active admin.
- **Auth: public** — anyone can reach it.
- **CSRF** — POST handlers verify a `_csrf` form field via
  `csrf::verify`.
- **Rate-limited** — see `src/ratelimit.rs` for bucket details.

## Public

Defined in `src/routes/public.rs`, plus the lazy derivative endpoint
in `src/routes/admin/media.rs` (wired ahead of the catch-all in
`main.rs`).

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/` | public | Renders the exhibit with `is_home = TRUE`. 404 if none is marked home. Logs a hit. |
| GET | `/files/dimgs/{ref_id}/{filename}` | public | Lazy thumbnail/derivative. Parses `{shape}_{size}_{name}`, clamps size to `16..=4096`, returns the cached or newly-generated derivative with `Cache-Control: public, max-age=31536000, immutable`. |
| GET | `/tag/{name}` | public | Synthetic tag page, joins `exhibits`/`tagged`/`tags`. Logs a hit. |
| GET | `/{path:.*}` | public | Looks up an exhibit by URL (tries `/{path}/` then `/{path}`). Honors the format's `intercept` hook. Returns the password gate when applicable. |
| POST | `/{path:.*}` | public | Password gate submission. CSRF-checked, rate-limited (bucket `unlock`, 8/60 s). |

## Admin — auth

Defined in `src/routes/admin/auth.rs`. Login / forgot / reset are
unauthenticated.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/login` | public | Login form. |
| POST | `/admin/login` | public | CSRF-checked. Rate-limited (bucket `login`, 10/60 s). On success stores `user_id` in the session and 302s to `/admin`. |
| POST | `/admin/logout` | session | CSRF-checked. Purges the session, 302s to `/admin/login`. |
| GET | `/admin/forgot` | public | Forgot-password form. |
| POST | `/admin/forgot` | public | CSRF-checked. Rate-limited (bucket `forgot`, 5/300 s). Renders the same generic confirmation in every case. On a real match, writes a 32-byte token + 1-hour expiry and sends a reset email via `mail::send`. |
| GET | `/admin/reset/{token}` | public | New-password form. |
| POST | `/admin/reset/{token}` | public | CSRF-checked. Rate-limited (bucket `reset`, 10/60 s). Requires `password == confirm` and `len >= 10`. |

## Admin — dashboard

Defined in `src/routes/admin/dashboard.rs`. Both routes call the
same inner handler.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin` | admin | Counts of exhibits/sections/media/tags. |
| GET | `/admin/` | admin | Same as above (trailing slash). |

## Admin — exhibits

Defined in `src/routes/admin/exhibits.rs`. All mutating routes are
CSRF-checked.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/exhibits` | admin | Grouped list of every exhibit with section + first-media thumbnail. Orphans bucket at the bottom. |
| GET | `/admin/exhibits/new` | admin | Step 1: format picker, populated from `formats::registry()`. |
| POST | `/admin/exhibits/new` | admin | Step 2: inserts a stub exhibit and redirects to its edit form. |
| GET | `/admin/exhibits/{id}/edit` | admin | Edit form. Fieldsets are gated by the format's `FormatCapabilities`. |
| POST | `/admin/exhibits/{id}/edit` | admin | Save. Normalizes URL, sanitizes external `link`, hashes password if supplied, syncs tags from a CSV input, enforces a single home. Redirects to the list or to the media uploader based on which button was used. |
| GET | `/admin/exhibits/{id}/confirm-delete` | admin | Confirmation page. |
| POST | `/admin/exhibits/{id}/delete` | admin | Deletes the exhibit and cascades its media rows. |
| POST | `/admin/exhibits/reorder` | admin | Bulk reorder from `ord_{id}` form inputs, in a transaction. |
| POST | `/admin/exhibits/{id}/move-up` | admin | Swap `ord` with the previous exhibit in the same section. |
| POST | `/admin/exhibits/{id}/move-down` | admin | Swap `ord` with the next exhibit in the same section. |

## Admin — media

Defined in `src/routes/admin/media.rs`. CSRF on every mutating
route. Uploads use `MultipartForm` with per-file and total caps.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/media` | admin | Cross-exhibit media browser. Filter by `kind`, `hidden`, free-text `q`. Paginated (50/page). |
| GET | `/admin/exhibits/{id}/media` | admin | Per-exhibit media list. |
| POST | `/admin/exhibits/{id}/media` | admin | Multipart upload. Magic-byte sniffs MIME, dedupes via `media.sha256`, EXIF-rotates images. |
| GET | `/admin/exhibits/{eid}/media/{mid}/edit` | admin | Edit title/caption/ord/hidden. |
| POST | `/admin/exhibits/{eid}/media/{mid}/edit` | admin | Save. |
| GET | `/admin/exhibits/{eid}/media/{mid}/confirm-delete` | admin | Confirmation page. |
| POST | `/admin/exhibits/{eid}/media/{mid}/delete` | admin | Deletes the row, the original in `gimgs/`, and every matching derivative in `dimgs/`. |
| POST | `/admin/exhibits/{eid}/media/reorder` | admin | Bulk reorder from `ord_{mid}` inputs. Scope-bound to `eid`. |
| POST | `/admin/exhibits/{eid}/media/{mid}/move-up` | admin | Swap `ord` with the previous media in the exhibit. |
| POST | `/admin/exhibits/{eid}/media/{mid}/move-down` | admin | Swap `ord` with the next media in the exhibit. |

## Admin — sections

Defined in `src/routes/admin/sections.rs`. CSRF on every mutating
route.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/sections` | admin | Section list. |
| GET | `/admin/sections/new` | admin | Blank section form. |
| POST | `/admin/sections/new` | admin | Insert. Path is right-stripped of trailing `/`. |
| GET | `/admin/sections/{id}/edit` | admin | Edit form. |
| POST | `/admin/sections/{id}/edit` | admin | Save. |
| GET | `/admin/sections/{id}/confirm-delete` | admin | Confirmation page. |
| POST | `/admin/sections/{id}/delete` | admin | Delete (cascades subsections via FK). |
| POST | `/admin/sections/{id}/move-up` | admin | Swap `ord` with the previous section. |
| POST | `/admin/sections/{id}/move-down` | admin | Swap `ord` with the next section. |

## Admin — settings

Defined in `src/routes/admin/settings.rs`. CSRF on POST.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/settings` | admin | Edit form for the singleton settings row. |
| POST | `/admin/settings` | admin | Save. SMTP password is AEAD-encrypted before write (skipped when blank — keeps the stored password). Theme colors are normalized to `#rrggbb` or empty. |

## Admin — tags

Defined in `src/routes/admin/tags.rs`. CSRF on every mutating route.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/tags` | admin | Tag list. |
| GET | `/admin/tags/new` | admin | Blank tag form. |
| POST | `/admin/tags/new` | admin | Insert. |
| GET | `/admin/tags/{id}/edit` | admin | Edit form. |
| POST | `/admin/tags/{id}/edit` | admin | Save. |
| GET | `/admin/tags/{id}/confirm-delete` | admin | Confirmation page. |
| POST | `/admin/tags/{id}/delete` | admin | Delete (cascades the `tagged` join via FK). |

## Admin — users

Defined in `src/routes/admin/users.rs`. CSRF on every mutating route.

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| GET | `/admin/users` | admin | User list. |
| GET | `/admin/users/new` | admin | Blank user form. |
| POST | `/admin/users/new` | admin | Insert. Requires `password.len() >= 10`; hashes with argon2. |
| GET | `/admin/users/{id}/edit` | admin | Edit form (identity + flags). |
| POST | `/admin/users/{id}/edit` | admin | Save. |
| GET | `/admin/users/{id}/password` | admin | Change-password form. |
| POST | `/admin/users/{id}/password` | admin | Validates `password == confirm` and `len >= 10`. |
| GET | `/admin/users/{id}/confirm-delete` | admin | Confirmation page. Refuses self-deletion (400). |
| POST | `/admin/users/{id}/delete` | admin | Delete. Refuses self-deletion (400). |

## Static files

Mounted directly by `actix_files::Files` in `main.rs`:

| Path | Source | Notes |
|------|--------|-------|
| `/static/*` | `Config.static_dir` (default `./static`) | SCSS-compiled CSS, fonts, icons. |
| `/files/*` | `Config.files_dir` (default `./files`) | Media originals at `gimgs/...`, derivatives at `dimgs/...`. The `derivative_get` handler is mounted ahead of this for the `dimgs/` path. |
