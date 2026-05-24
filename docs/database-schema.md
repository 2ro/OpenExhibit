# Database schema

PostgreSQL schema as installed by the migrations under `./migrations/`,
applied in order at startup by `db::init` (`src/db.rs`). Each table
below is followed by its columns, types, defaults, and the migration
that introduced each column.

> Generated from source. Re-run `scripts/gen-docs.sh` whenever a new
> migration is added.

Migration index:

| File | Purpose |
|------|---------|
| `0001_initial_schema.sql` | Initial schema for every table. |
| `0002_seed_install.sql` | Minimal seed (sections, exhibits, exhibit_prefs, settings singleton). Admin user is *not* seeded — `auth::ensure_first_admin` creates it on first boot. |
| `0003_smtp_settings.sql` | Adds `smtp_*` columns to `settings`. |
| `0004_section_hide_title.sql` | Adds `sections.hide_title`. |
| `0005_custom_css.sql` | Adds `exhibits.custom_css` and `settings.custom_css`. |
| `0006_theme_colors.sql` | Adds `settings.theme_text_color` and `settings.theme_bg_color`. |
| `0007_greentext.sql` | Adds `settings.enable_greentext`. |
| `0008_media_sha256.sql` | Adds `media.sha256` + partial index for upload dedupe. |

## `sections`

Top-level nav buckets. One row per section visible (or hidden) in the
public nav. Originated in `0001`; `hide_title` added in `0004`.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SMALLSERIAL` | (auto) | `0001` | Primary key. |
| `name` | `VARCHAR(60)` | `''` | `0001` | Internal short name. |
| `kind` | `VARCHAR(50)` | `'exhibits'` | `0001` | `exhibits`/`xml`/`tag`. Only `exhibits` is used by public/admin code today. |
| `ord` | `SMALLINT` | `0` | `0001` | Display order. |
| `display` | `SMALLINT` | `1` | `0001` | Legacy visibility flag (1 = show); operational flag is `hidden`. |
| `hidden` | `BOOLEAN` | `FALSE` | `0001` | Suppresses the section in `Section::list_visible`. |
| `password` | `VARCHAR(255)` | `''` | `0001` | Reserved; not enforced by current routes. |
| `created_at` | `TIMESTAMPTZ` | `NULL` | `0001` | |
| `path` | `VARCHAR(250)` | `''` | `0001` | URL prefix (e.g. `/work`). |
| `description` | `VARCHAR(100)` | `''` | `0001` | Display label preferred over `name` in the nav when non-empty. |
| `proj` | `SMALLINT` | `0` | `0001` | Legacy. |
| `grp` | `SMALLINT` | `0` | `0001` | Legacy. |
| `report` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `hide_title` | `BOOLEAN` | `FALSE` | `0004` | When true the public nav renders this section's children as a bare list with no heading. |

Indexes (`0001`): `sections_path_idx ON (path)`, `sections_ord_idx ON (ord)`.

## `subsections`

Optional groupings inside a section. `exhibits.section_sub` matches by
the subsection's `title` (string-keyed in the legacy schema).

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SMALLSERIAL` | (auto) | `0001` | Primary key. |
| `section_id` | `SMALLINT` | (required) | `0001` | `REFERENCES sections(id) ON DELETE CASCADE`. |
| `title` | `VARCHAR(255)` | `''` | `0001` | Subsection heading; the join key for `exhibits.section_sub`. |
| `folder` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `ord` | `SMALLINT` | `0` | `0001` | Display order. |
| `hidden` | `BOOLEAN` | `FALSE` | `0001` | Suppresses the subsection from the nav. |

Indexes (`0001`): `subsections_section_idx ON (section_id)`.

## `exhibits`

Every public page. A wide row carrying every column any format
might use. New formats reuse these columns where possible; unused
columns stay untouched on save.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | Primary key. |
| `kind` | `VARCHAR(100)` | `'exhibits'` | `0001` | `exhibits`/`xml`/`tag`. Public routes filter on `kind = 'exhibits'`. |
| `ref_id` | `INTEGER` | `0` | `0001` | Legacy cross-ref. |
| `title` | `VARCHAR(255)` | `''` | `0001` | |
| `content` | `TEXT` | `''` | `0001` | Long-form text. Rendered through the markup pipeline. |
| `is_home` | `BOOLEAN` | `FALSE` | `0001` | Exactly one row should have this set; enforced by the edit handler. |
| `link` | `VARCHAR(255)` | `''` | `0001` | External URL for `external_link` (and any format that opts in). Sanitized against an allowlist at write time. |
| `link_target` | `BOOLEAN` | `FALSE` | `0001` | When true `external_link` opens in a new tab. |
| `iframe` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `is_new` | `BOOLEAN` | `FALSE` | `0001` | Drives a "new" badge in the nav. |
| `tags` | `VARCHAR(250)` | `'0'` | `0001` | Legacy CSV; the live tag list comes from `tagged`. |
| `header` | `TEXT` | `''` | `0001` | Legacy. |
| `updated_at` | `TIMESTAMPTZ` | `NULL` | `0001` | Bumped on save. |
| `published_at` | `TIMESTAMPTZ` | `NULL` | `0001` | Reserved. |
| `creator` | `SMALLINT` | `0` | `0001` | Legacy. |
| `status` | `SMALLINT` | `0` | `0001` | `0 = draft`, `1 = published`. Public routes filter on `status = 1`. |
| `process` | `BOOLEAN` | `TRUE` | `0001` | Legacy. |
| `page_cache` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `section_id` | `SMALLINT` | `0` | `0001` | The section this exhibit belongs to. |
| `section_top` | `BOOLEAN` | `FALSE` | `0001` | When true the exhibit becomes the section's "top" entry in the nav. |
| `section_sub` | `VARCHAR(255)` | `''` | `0001` | Optional subsection title; joins to `subsections.title`. |
| `subdir` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `url` | `VARCHAR(250)` | `''` | `0001` | Public URL slug. Normalized to `/.../` on save. |
| `ord` | `SMALLINT` | `999` | `0001` | Display order within the section. |
| `color` | `VARCHAR(7)` | `'ffffff'` | `0001` | Legacy per-exhibit color. |
| `bgimg` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `hidden` | `BOOLEAN` | `FALSE` | `0001` | Hidden from the nav (still reachable by URL). |
| `current_flag` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `perm` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `media_source` | `SMALLINT` | `0` | `0001` | Legacy. |
| `media_source_detail` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `images` | `SMALLINT` | `9999` | `0001` | Legacy soft cap. |
| `thumbs_shape` | `SMALLINT` | `0` | `0001` | Legacy. |
| `thumbs` | `SMALLINT` | `200` | `0001` | Thumbnail size in pixels (drives the lazy derivative URL). |
| `format` | `VARCHAR(100)` | `'visual_index'` | `0001` | Registry key (`src/formats/mod.rs`). Unknown values fall back to `visual_index`. |
| `thumbs_format` | `SMALLINT` | `0` | `0001` | Legacy. |
| `operand` | `SMALLINT` | `0` | `0001` | Legacy. |
| `titling` | `SMALLINT` | `0` | `0001` | Legacy. |
| `break_count` | `SMALLINT` | `0` | `0001` | Legacy. |
| `tiling` | `BOOLEAN` | `TRUE` | `0001` | Legacy. |
| `year` | `VARCHAR(4)` | `'2010'` | `0001` | Legacy. |
| `report` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `password` | `VARCHAR(100)` | `''` | `0001` | Argon2 PHC. Non-empty triggers the public password gate. |
| `placement` | `SMALLINT` | `0` | `0001` | Legacy. |
| `template` | `VARCHAR(25)` | `'index.php'` | `0001` | Legacy. |
| `extra` | `JSONB` | `'{}'::jsonb` | `0001` | Per-format escape hatch. |
| `custom_css` | `TEXT` | `''` | `0005` | Per-exhibit CSS rendered inline in `<head>` after the site-wide block. |

Indexes (`0001`): `exhibits_url_idx ON (url)`, `exhibits_section_idx ON (section_id)`,
`exhibits_section_top_ix ON (section_top)`, `exhibits_status_idx ON (status)`,
`exhibits_home_idx ON (is_home)`, `exhibits_kind_idx ON (kind)`.

## `exhibit_prefs`

Per-kind preference rows seeded in `0002`. Not written to by current
code paths but loaded by legacy compatibility helpers.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `ref_type` | `VARCHAR(255)` | `''` | `0001` | `exhibits`/`xml`/`tag`. |
| `active` | `BOOLEAN` | `TRUE` | `0001` | |
| `title` | `VARCHAR(255)` | `''` | `0001` | |
| `section` | `SMALLINT` | `1` | `0001` | |
| `template` | `VARCHAR(50)` | `''` | `0001` | |
| `members` | `VARCHAR(255)` | `''` | `0001` | |
| `img` | `VARCHAR(255)` | `''` | `0001` | |
| `settings` | `JSONB` | `'{}'::jsonb` | `0001` | |
| `grp` | `VARCHAR(255)` | `''` | `0001` | |

## `media`

One row per uploaded file under `$FILES_DIR/gimgs/{ref_id}/{file}`.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `ref_id` | `INTEGER` | `0` | `0001` | The owning exhibit's id (when `obj_type = 'exhibits'`). |
| `obj_type` | `VARCHAR(15)` | `''` | `0001` | Currently always `'exhibits'`. |
| `mime` | `VARCHAR(15)` | `''` | `0001` | Short MIME tag (`jpg`/`png`/`gif`/`webp`/`mp4`/`mov`/`webm`/`mp3`/`ogg`). |
| `tags` | `VARCHAR(255)` | `'0'` | `0001` | Legacy. |
| `file` | `VARCHAR(255)` | `''` | `0001` | Path-safe filename (post-sanitize). |
| `thumb` | `VARCHAR(255)` | `''` | `0001` | Optional precomputed thumbnail filename — if empty the lazy derivative path is used. |
| `file_replace` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `title` | `VARCHAR(255)` | `''` | `0001` | Admin-editable. |
| `caption` | `TEXT` | `''` | `0001` | Rendered through the markup pipeline. |
| `width` | `INTEGER` | `0` | `0001` | Pixels (0 for non-images). |
| `height` | `INTEGER` | `0` | `0001` | Pixels (0 for non-images). |
| `width_resp` | `INTEGER` | `0` | `0001` | Legacy responsive size. |
| `height_resp` | `INTEGER` | `0` | `0001` | Legacy responsive size. |
| `bytes` | `INTEGER` | `0` | `0001` | File size in bytes. |
| `updated_at` | `TIMESTAMPTZ` | `NULL` | `0001` | |
| `uploaded_at` | `TIMESTAMPTZ` | `NULL` | `0001` | |
| `ord` | `SMALLINT` | `999` | `0001` | Display order within the exhibit. |
| `hidden` | `BOOLEAN` | `FALSE` | `0001` | |
| `dir` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `src` | `VARCHAR(25)` | `''` | `0001` | Legacy. |
| `sha256` | `VARCHAR(64)` | `''` | `0008` | Hex SHA-256 of the raw uploaded bytes. Populated at upload time; used to skip exact-duplicate re-uploads inside the same exhibit. |

Indexes (`0001`): `media_ref_idx ON (ref_id)`, `media_type_idx ON (obj_type)`,
`media_order_idx ON (ord)`. Partial index in `0008`:
`media_sha256_idx ON (sha256) WHERE sha256 <> ''`.

## `users`

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | Primary key. |
| `userid` | `VARCHAR(100)` | (required, UNIQUE) | `0001` | Login name. |
| `password_hash` | `TEXT` | (required) | `0001` | Argon2id PHC string. Replaces the legacy MD5 column. |
| `email` | `VARCHAR(100)` | `''` | `0001` | |
| `threads` | `SMALLINT` | `10` | `0001` | Legacy. |
| `writing` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `offset_` | `SMALLINT` | `0` | `0001` | Legacy TZ offset (column name escaped because `offset` is reserved). |
| `date_format` | `VARCHAR(30)` | `'%d %B %Y'` | `0001` | Per-user date format. |
| `lang` | `VARCHAR(8)` | `'en-us'` | `0001` | |
| `user_hash` | `VARCHAR(32)` | `''` | `0001` | Legacy. |
| `help` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `mode` | `SMALLINT` | `0` | `0001` | Legacy. |
| `first_name` | `VARCHAR(35)` | `''` | `0001` | |
| `last_name` | `VARCHAR(35)` | `''` | `0001` | |
| `is_admin` | `BOOLEAN` | `FALSE` | `0001` | Gates every `/admin/*` route. |
| `is_active` | `BOOLEAN` | `TRUE` | `0001` | Disabled users can't log in. |
| `is_client` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `img` | `VARCHAR(255)` | `''` | `0001` | Legacy avatar. |
| `reset_token` | `VARCHAR(64)` | `NULL` | `0001` | Password-reset bearer token. |
| `reset_expires` | `TIMESTAMPTZ` | `NULL` | `0001` | Token expiry. |

## `tags`

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `name` | `VARCHAR(255)` | (required, UNIQUE) | `0001` | |
| `grp` | `SMALLINT` | `1` | `0001` | Optional group/category. |
| `created_at` | `TIMESTAMPTZ` | `NULL` | `0001` | |
| `icon` | `VARCHAR(255)` | `''` | `0001` | |

## `tagged`

Join table between `tags` and exhibits (and, eventually, other object types).

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `tag_id` | `INTEGER` | (required) | `0001` | `REFERENCES tags(id) ON DELETE CASCADE`. |
| `obj_type` | `VARCHAR(3)` | `''` | `0001` | `'exh'` for exhibits. |
| `obj_id` | `INTEGER` | (required) | `0001` | The tagged row's id. |

Constraints: `UNIQUE (tag_id, obj_type, obj_id)`.
Indexes (`0001`): `tagged_obj_idx ON (obj_type, obj_id)`.

## `settings`

Singleton (`id = 1`) loaded by `Settings::load` and saved through
`/admin/settings`. Columns added over time as new features landed.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SMALLSERIAL` | (auto) | `0001` | Always `1`. |
| `site_name` | `VARCHAR(255)` | `''` | `0001` | |
| `install_date` | `TIMESTAMPTZ` | `NULL` | `0001` | |
| `version` | `VARCHAR(25)` | `''` | `0001` | |
| `site_lang` | `VARCHAR(8)` | `'en-us'` | `0001` | |
| `time_format` | `VARCHAR(25)` | `'%d %B %Y'` | `0001` | |
| `tagging` | `BOOLEAN` | `TRUE` | `0001` | Toggles the tag UI. |
| `help` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `caching` | `BOOLEAN` | `FALSE` | `0001` | Legacy. |
| `hibernate` | `VARCHAR(255)` | `''` | `0001` | Legacy. |
| `obj_name` | `VARCHAR(255)` | `''` | `0001` | Displayed site name (substituted into `obj_itop`/`obj_ibot`). |
| `obj_theme` | `VARCHAR(50)` | `'default'` | `0001` | Theme name. |
| `obj_itop` | `TEXT` | `''` | `0001` | Sidebar HTML rendered above the nav. Goes through the markup pipeline; `{{ obj_name }}` / `{{ site_name }}` are substituted before rendering. |
| `obj_ibot` | `TEXT` | `''` | `0001` | Sidebar HTML rendered below the nav. |
| `obj_org` | `BOOLEAN` | `TRUE` | `0001` | Legacy. |
| `obj_apikey` | `VARCHAR(64)` | `''` | `0001` | Legacy. |
| `site_format` | `VARCHAR(30)` | `'%d %B %Y'` | `0001` | Site-wide date format. |
| `site_offset` | `SMALLINT` | `0` | `0001` | Legacy TZ offset. |
| `site_vars` | `JSONB` | `'{}'::jsonb` | `0001` | Legacy. |
| `smtp_host` | `VARCHAR(255)` | `''` | `0003` | Empty = no SMTP, mails go to stdout. |
| `smtp_port` | `INTEGER` | `587` | `0003` | |
| `smtp_user` | `VARCHAR(255)` | `''` | `0003` | |
| `smtp_pass` | `VARCHAR(255)` | `''` | `0003` | AEAD-encrypted via `crypto::encrypt`; carries an `enc:` prefix. Legacy plaintext rows pass through unchanged on read. |
| `smtp_from` | `VARCHAR(255)` | `''` | `0003` | |
| `custom_css` | `TEXT` | `''` | `0005` | Site-wide CSS rendered inline in `<head>` before any per-exhibit block. |
| `theme_text_color` | `VARCHAR(20)` | `''` | `0006` | `#rrggbb` or empty. Drives `:root { --color: …; --color5: …; }`. |
| `theme_bg_color` | `VARCHAR(20)` | `''` | `0006` | `#rrggbb` or empty. Drives `:root { --bg: …; }`. |
| `enable_greentext` | `BOOLEAN` | `FALSE` | `0007` | When true `>` lines render as `<p class="greentext">` instead of Markdown blockquotes. |

## `abstracts`

Generic key-value store for per-object extras. Not written to by
current code paths.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `obj` | `VARCHAR(32)` | `''` | `0001` | Owning object type. |
| `obj_id` | `INTEGER` | `0` | `0001` | Owning object id. |
| `var` | `VARCHAR(255)` | `''` | `0001` | Key. |
| `val` | `TEXT` | `''` | `0001` | Value. |

Indexes (`0001`): `abstracts_obj_idx ON (obj, obj_id)`.

## `plugins`

Legacy plugin registry from the original Indexhibit schema. Not
written to by current code paths.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `SERIAL` | (auto) | `0001` | |
| `is_primary` | `BOOLEAN` | `FALSE` | `0001` | |
| `plugin_type` | `VARCHAR(15)` | `''` | `0001` | |
| `name` | `VARCHAR(255)` | `''` | `0001` | |
| `uri` | `VARCHAR(255)` | `''` | `0001` | |
| `version` | `VARCHAR(20)` | `''` | `0001` | |
| `file` | `VARCHAR(255)` | `''` | `0001` | |
| `function_name` | `VARCHAR(255)` | `''` | `0001` | |
| `hook` | `VARCHAR(255)` | `''` | `0001` | |
| `space` | `VARCHAR(100)` | `''` | `0001` | |
| `creator` | `VARCHAR(50)` | `''` | `0001` | |
| `www` | `VARCHAR(255)` | `''` | `0001` | |
| `description` | `TEXT` | `''` | `0001` | |
| `options` | `JSONB` | `'{}'::jsonb` | `0001` | |
| `options_build` | `TEXT` | `''` | `0001` | |
| `usage_text` | `VARCHAR(255)` | `''` | `0001` | |
| `usage_desc` | `VARCHAR(255)` | `''` | `0001` | |
| `ord` | `SMALLINT` | `100` | `0001` | |

## `stats`

Hit log. `routes::public::*` fire-and-forget inserts here via
`stats::log_hit`.

| Column | Type | Default | Migration | Notes |
|--------|------|---------|-----------|-------|
| `id` | `BIGSERIAL` | (auto) | `0001` | |
| `addr` | `INET` | `NULL` | `0001` | Client IP. `X-Forwarded-For` is honored only when the peer is in `Config.trusted_proxies`. |
| `country` | `VARCHAR(30)` | `''` | `0001` | Reserved (filled by an offline geo-IP pass when present). |
| `lang` | `VARCHAR(10)` | `''` | `0001` | Reserved. |
| `domain` | `VARCHAR(100)` | `''` | `0001` | `Host` header. |
| `referrer` | `VARCHAR(255)` | `''` | `0001` | `Referer` header. |
| `page` | `VARCHAR(255)` | `''` | `0001` | Path. |
| `agent` | `VARCHAR(255)` | `''` | `0001` | `User-Agent` header. |
| `keyword` | `VARCHAR(255)` | `''` | `0001` | Reserved. |
| `os` | `VARCHAR(20)` | `''` | `0001` | Reserved. |
| `browser` | `VARCHAR(20)` | `''` | `0001` | Reserved. |
| `hit_at` | `TIMESTAMPTZ` | `now()` | `0001` | |
| `hit_month` | `VARCHAR(7)` | `''` | `0001` | `YYYY-MM`. |
| `hit_day` | `DATE` | `NULL` | `0001` | |

Indexes (`0001`): `stats_hit_day_idx ON (hit_day)`.

## `stats_exhibits`, `stats_storage`, `iptocountry`

Empty companion tables shipped for schema completeness; no current
writers.

`stats_exhibits` — `url VARCHAR(255) PRIMARY KEY`, `count INTEGER NOT NULL DEFAULT 0`.

`stats_storage` — `month VARCHAR(7) PRIMARY KEY` (`YYYY-MM`),
`hits/uniques/referrers INTEGER NOT NULL DEFAULT 0`.

`iptocountry` — `ip_from BIGINT`, `ip_to BIGINT`, `country_code2 CHAR(2)`,
`country_code3 CHAR(3)`, `country_name VARCHAR(50)`, primary key
`(ip_from, ip_to)`.

## Seeded rows (from `0002`)

- `sections` — `main` (`/`), `work` (`/work`), `tag` (`/tag`, hidden).
- `exhibits` — Welcome (`is_home`, `/`), Work (`/work/`), Tags (`/tag/`, draft).
- `exhibit_prefs` — one per kind (`exhibits`/`xml`/`tag`).
- `settings` — singleton at `id = 1` (site name "My Portfolio",
  default theme, default obj_itop pointing at `/`).
