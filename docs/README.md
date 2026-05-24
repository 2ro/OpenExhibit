# OpenExhibit docs

Contributor and operator documentation. The user-facing intro lives
in the [project README](../README.md); this directory is for people
extending the code or running it in production.

## Prose docs

| File | What it covers |
|------|----------------|
| [architecture.md](architecture.md) | Process model, middleware stack, request lifecycle, format registry, markup pipeline, session / CSRF / auth, lazy derivative images. |
| [api-routes.md](api-routes.md) | Every HTTP endpoint with method, path, auth, CSRF, rate-limit, and a one-line note. |
| [database-schema.md](database-schema.md) | Every table, column, type, default, and the migration that introduced it. |
| [exhibit-formats.md](exhibit-formats.md) | What ships, capabilities matrix, and a step-by-step walkthrough for adding a new format. |
| [configuration.md](configuration.md) | Every environment variable and the in-app `settings` row split. |

## Generated API docs

The cargo rustdoc HTML lives at
[`../target/doc/openexhibit/index.html`](../target/doc/openexhibit/index.html)
after running:

```
make -C docs            # audit + cargo doc
# or
cargo doc --no-deps     # rustdoc only
```

The HTML isn't committed to the repo — it's regenerated on demand.

## Regenerating and auditing

```
./scripts/gen-docs.sh   # full pass: audit + cargo doc
make -C docs            # same, via Make
make -C docs check      # audit only, no rustdoc rebuild
```

`scripts/gen-docs.sh` cross-checks the prose `.md` files against
source. It fails when:

- An `#[get]` / `#[post]` macro in `src/routes/` references a path
  not mentioned in `api-routes.md`.
- A format registered in `src/formats/` has no entry in
  `exhibit-formats.md`.
- A file under `migrations/` isn't referenced by `database-schema.md`.
- An env var from `.env.example` or `src/config.rs` isn't documented
  in `configuration.md`.

When something changes, update the relevant `.md` and re-run the
script until it passes. The prose isn't auto-rewritten — only the
presence checks are automated.

## Conventions

- Markdown only, GitHub-flavored.
- Cross-reference between docs with relative links.
- Keep examples copy-pasteable.
- When a doc cites a specific file or function, include the path
  (e.g. `src/markup.rs::render_with`) so readers can jump.
- Don't restate what `cargo doc` already says about a public API —
  link to the rustdoc HTML instead.
