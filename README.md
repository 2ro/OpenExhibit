# OpenExhibit

A self-hosted portfolio CMS in Rust. Spiritual successor to
[Indexhibit](https://www.indexhibit.org/), rewritten from scratch with no
JavaScript anywhere — public site, admin, lightbox, slideshow autoplay,
mobile menu, everything. HTML and CSS primitives only.

- **Actix-Web 4**, PostgreSQL via **sqlx**, **Askama** templates,
  SCSS compiled by **grass** at `cargo build` time.
- **Eleven pluggable exhibit formats** out of the box. Add your own as a
  one-file Rust module + one Askama template.
- **Server-side rich text** — Markdown + a small BBCode subset + sanitized
  inline HTML.
- **Argon2id** passwords. CSRF on every mutating endpoint. CSP, frame
  options, referrer policy, in-process per-IP rate limits on auth.
- SMTP password encrypted at rest. Trusted-proxy XFF allowlist.

## Install (Debian / Ubuntu VPS)

Point your domain at the box, then run:

```sh
curl -fsSL https://raw.githubusercontent.com/2ro/OpenExhibit/main/install.sh \
  | sudo bash -s -- --domain example.com
```

The script installs PostgreSQL, Rust, and Caddy, builds the release binary,
writes `/opt/openexhibit/.env`, configures Caddy to terminate TLS and
reverse-proxy to `127.0.0.1:8080`, and starts the `openexhibit` systemd unit.
The first-boot admin password is printed to `journalctl -u openexhibit`.

For a loopback-only install (bring your own reverse proxy) drop the
`--domain` flag. Installer flags: `--port`, `--dir`, `--branch`, `--repo`,
`--yes`.

## Run locally

```sh
# any PostgreSQL 14+
createdb openexhibit
cp .env.example .env  # set DATABASE_URL + a 64-byte SESSION_KEY
cargo run
```

First boot creates an `admin` user and prints a random password once. Visit
`http://localhost:8080/admin`.

## Docs

| File | What |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Process model, request lifecycle, format registry, markup pipeline, lazy derivative images. |
| [docs/api-routes.md](docs/api-routes.md) | Every HTTP endpoint with method, path, auth, CSRF, rate-limit. |
| [docs/database-schema.md](docs/database-schema.md) | Every table column with type, default, migration provenance. |
| [docs/exhibit-formats.md](docs/exhibit-formats.md) | What ships + walkthrough for adding a new format. |
| [docs/configuration.md](docs/configuration.md) | Every env var with type, default, effect. |

`scripts/gen-docs.sh` audits the docs against source and fails when they drift.

## License

MIT — see [LICENSE](LICENSE).

---

🤖 Built with AI pair-programming assistance (Claude)
