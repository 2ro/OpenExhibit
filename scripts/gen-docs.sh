#!/usr/bin/env bash
# Regenerate every doc under docs/ from source.
#
# Idempotent. Re-runs the cargo rustdoc build into target/doc/ and runs the
# Markdown auditors that ship in this script. Used by hand and by the
# project Makefile target `docs`.
#
# Source-of-truth file paths the markdown docs derive from:
#
#   docs/architecture.md     ← src/main.rs, src/routes/, src/formats/mod.rs,
#                              src/models/, src/{auth,csrf,markup,images,
#                              crypto,db,error,ratelimit,stats,mail}.rs
#   docs/api-routes.md       ← every #[get]/#[post]/web::resource in src/routes/
#   docs/database-schema.md  ← migrations/0001..0008
#   docs/exhibit-formats.md  ← src/formats/mod.rs + src/formats/<key>.rs
#   docs/configuration.md    ← src/config.rs, .env.example
#
# This script does two things:
#   1. Audit: compare what the docs claim against what the source says,
#      and fail loudly when they drift (CI signal).
#   2. Rebuild cargo's API HTML into target/doc/.
#
# The four prose .md files are hand-written but their tables (route
# inventory, schema columns, format descriptions, env vars) are checked
# here against source so the docs can't silently rot.

set -euo pipefail

cd "$(dirname "$0")/.."

red()    { printf '\033[31m%s\033[0m\n' "$*" >&2; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }

errors=0
fail() { red "  ✗ $*"; errors=$((errors + 1)); }
ok()   { green "  ✓ $*"; }

# ─── Audit 1: api-routes.md mentions every #[get]/#[post] macro ────────────
#
# Macros under src/routes/admin/ are registered inside web::scope("/admin"),
# so their effective path is "/admin" + the macro argument. The lazy
# derivative endpoint in src/routes/admin/media.rs is the only exception —
# it's wired through `configure_public` outside the scope, so its macro
# argument (/files/dimgs/...) is the effective path.
audit_routes() {
  yellow "Auditing docs/api-routes.md against #[get]/#[post] macros…"
  local missing=0
  while IFS= read -r raw; do
    # grep -H prefixes every match with FILE:LINE:; strip both.
    local file path expected
    file="${raw%%:*}"
    path="${raw#*:#}"  # drop FILE:LINE: → leaves "[get(\"…\")]"
    # Extract the quoted argument; empty string is valid (dashboard "").
    path=$(printf '%s' "$path" | sed -nE 's/.*\("([^"]*)"\).*/\1/p')

    if [[ "$file" == src/routes/admin/* ]] && [[ "$path" != "/files/dimgs/"* ]]; then
      if [ -z "$path" ]; then
        expected="/admin"
      else
        expected="/admin${path}"
      fi
    else
      expected="$path"
    fi

    if ! grep -F -q "\`${expected}\`" docs/api-routes.md; then
      fail "route not documented: ${expected} (from ${file})"
      missing=$((missing + 1))
    fi
  done < <(
    grep -rHE '#\[(get|post)\("' src/routes/ | sort -u
  )
  # Also check the four web::resource() routes in admin/auth.rs.
  for r in /admin/login /admin/forgot /admin/reset/{token} /admin/logout; do
    if ! grep -F -q "\`${r}\`" docs/api-routes.md; then
      fail "route not documented: ${r} (web::resource in admin/auth.rs)"
      missing=$((missing + 1))
    fi
  done
  if [ "$missing" -eq 0 ]; then
    ok "every route in src/routes/ appears in docs/api-routes.md"
  fi
}

# ─── Audit 2: exhibit-formats.md lists every registered format key ─────────
audit_formats() {
  yellow "Auditing docs/exhibit-formats.md against src/formats/mod.rs…"
  local missing=0
  while IFS= read -r key; do
    if ! grep -F -q "\`${key}\`" docs/exhibit-formats.md; then
      fail "format key not documented: ${key}"
      missing=$((missing + 1))
    fi
  done < <(
    # Pull keys out of the FORMATS slice + their corresponding module's
    # `fn key()` returns. Both should agree; this audit just needs the names.
    grep -hE 'fn key' src/formats/*.rs -A1 \
      | grep -oE '"[a-z_]+"' | tr -d '"' | sort -u
  )
  if [ "$missing" -eq 0 ]; then
    ok "every registered format key appears in docs/exhibit-formats.md"
  fi
}

# ─── Audit 3: database-schema.md references every migration ────────────────
audit_migrations() {
  yellow "Auditing docs/database-schema.md against migrations/…"
  local missing=0
  for m in migrations/*.sql; do
    local stem
    stem="$(basename "$m")"
    if ! grep -F -q "$stem" docs/database-schema.md; then
      fail "migration not referenced: ${stem}"
      missing=$((missing + 1))
    fi
  done
  if [ "$missing" -eq 0 ]; then
    ok "every migration filename appears in docs/database-schema.md"
  fi
}

# ─── Audit 4: configuration.md mentions every env var in .env.example ──────
audit_env() {
  yellow "Auditing docs/configuration.md against .env.example…"
  local missing=0
  while IFS= read -r var; do
    if ! grep -F -q "\`${var}\`" docs/configuration.md; then
      fail "env var not documented: ${var}"
      missing=$((missing + 1))
    fi
  done < <(
    # Strip comments and blank lines, take the part before '='.
    grep -E '^[A-Z][A-Z0-9_]*=' .env.example \
      | sed -E 's/^([A-Z0-9_]+)=.*$/\1/' \
      | sort -u
  )
  # Also audit env vars read in src/config.rs that should be documented.
  while IFS= read -r var; do
    if ! grep -F -q "\`${var}\`" docs/configuration.md; then
      fail "config.rs env var not documented: ${var}"
      missing=$((missing + 1))
    fi
  done < <(
    grep -oE 'env::var\("[A-Z_]+"\)' src/config.rs \
      | sed -E 's/env::var\("([A-Z_]+)"\)/\1/' \
      | sort -u
  )
  if [ "$missing" -eq 0 ]; then
    ok "every .env.example and src/config.rs env var appears in docs/configuration.md"
  fi
}

# ─── Build: cargo rustdoc into target/doc/ ─────────────────────────────────
build_rustdoc() {
  yellow "Building rustdoc HTML (cargo doc --no-deps)…"
  if cargo doc --no-deps --quiet 2>&1 | tail -20; then
    ok "rustdoc HTML at target/doc/openexhibit/index.html"
  else
    fail "cargo doc failed"
  fi
}

audit_routes
audit_formats
audit_migrations
audit_env
build_rustdoc

echo
if [ "$errors" -gt 0 ]; then
  red "FAIL: ${errors} doc-vs-source mismatch(es). Re-edit the .md files above to match the source, or update the source."
  exit 1
fi
green "OK: docs are consistent with source."
