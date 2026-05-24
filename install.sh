#!/usr/bin/env bash
#
# OpenExhibit one-shot installer for Debian / Ubuntu.
#
#   curl -fsSL https://raw.githubusercontent.com/2ro/OpenExhibit/main/install.sh | sudo bash
#   curl -fsSL https://raw.githubusercontent.com/2ro/OpenExhibit/main/install.sh | sudo bash -s -- --domain example.com
#
# Flags:
#   --domain <fqdn>   Install Caddy and serve via auto-HTTPS on <fqdn>.
#                     Without this flag the app binds to 127.0.0.1:8080 (loopback only).
#                     Edit .env to expose publicly (and ideally put TLS in front).
#   --dir <path>      Install directory (default: /opt/openexhibit).
#   --branch <name>   Git branch / tag to deploy (default: main).
#   --repo <url>      Git repo URL (default: https://github.com/2ro/OpenExhibit.git).
#   --port <n>        Local port the app binds to (default: 8080).
#   --yes             Skip the confirmation prompt.

set -euo pipefail

DOMAIN=""
INSTALL_DIR="/opt/openexhibit"
BRANCH="main"
REPO_URL="https://github.com/2ro/OpenExhibit.git"
PORT="8080"
ASSUME_YES="0"

SERVICE_USER="openexhibit"
DB_USER="openexhibit"
DB_NAME="openexhibit"

while [ $# -gt 0 ]; do
  case "$1" in
    --domain) DOMAIN="${2:?--domain requires a value}"; shift 2 ;;
    --dir)    INSTALL_DIR="${2:?--dir requires a value}"; shift 2 ;;
    --branch) BRANCH="${2:?--branch requires a value}"; shift 2 ;;
    --repo)   REPO_URL="${2:?--repo requires a value}"; shift 2 ;;
    --port)   PORT="${2:?--port requires a value}"; shift 2 ;;
    --yes|-y) ASSUME_YES="1"; shift ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -t 1 ]; then
  BOLD=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31m'; GRN=$'\033[32m'; YLW=$'\033[33m'; RST=$'\033[0m'
else
  BOLD=""; DIM=""; RED=""; GRN=""; YLW=""; RST=""
fi
log()  { printf '%s==>%s %s\n' "$GRN" "$RST" "$*"; }
warn() { printf '%s!!%s  %s\n' "$YLW" "$RST" "$*" >&2; }
die()  { printf '%sxx%s  %s\n' "$RED" "$RST" "$*" >&2; exit 1; }

[ "$(id -u)" = "0" ] || die "Run as root (try: curl ... | sudo bash)."

if [ -r /etc/os-release ]; then . /etc/os-release; else die "Cannot detect OS."; fi
case "${ID:-}:${ID_LIKE:-}" in
  *debian*|*ubuntu*) : ;;
  *) warn "Untested OS '$ID'. Continuing — apt-get is required." ;;
esac
command -v apt-get >/dev/null || die "apt-get not found. Only Debian-family distros are supported by this script."

cat <<EOF
${BOLD}OpenExhibit installer${RST}
  install dir   : $INSTALL_DIR
  service user  : $SERVICE_USER
  database      : $DB_NAME (role $DB_USER, local socket)
  bind          : 127.0.0.1:$PORT $( [ -n "$DOMAIN" ] && echo "(behind Caddy)" || echo "(loopback only — edit .env to expose)" )
  domain        : ${DOMAIN:-<none — plain HTTP>}
  repo / branch : $REPO_URL @ $BRANCH
EOF

if [ "$ASSUME_YES" != "1" ] && [ -t 0 ]; then
  read -r -p "Proceed? [y/N] " ans
  case "$ans" in y|Y|yes|YES) : ;; *) die "Aborted." ;; esac
fi

log "Installing system packages..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends \
  ca-certificates curl git pkg-config build-essential \
  libssl-dev libpq-dev \
  postgresql postgresql-contrib \
  openssl sudo
if [ -n "$DOMAIN" ]; then
  if ! command -v caddy >/dev/null 2>&1; then
    log "Installing Caddy..."
    apt-get install -y --no-install-recommends debian-keyring debian-archive-keyring apt-transport-https gnupg
    curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt \
      | sed 's|^deb |deb [signed-by=/usr/share/keyrings/caddy-stable-archive-keyring.gpg] |' \
      > /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq
    apt-get install -y --no-install-recommends caddy
  fi
fi

if ! id "$SERVICE_USER" >/dev/null 2>&1; then
  log "Creating system user '$SERVICE_USER'..."
  useradd --system --home-dir "$INSTALL_DIR" --shell /usr/sbin/nologin "$SERVICE_USER"
fi

log "Fetching source into $INSTALL_DIR..."
if [ -d "$INSTALL_DIR/.git" ]; then
  git -C "$INSTALL_DIR" fetch --depth=1 origin "$BRANCH"
  git -C "$INSTALL_DIR" checkout -q "$BRANCH"
  git -C "$INSTALL_DIR" reset --hard "origin/$BRANCH"
else
  mkdir -p "$INSTALL_DIR"
  git clone --depth=1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR"
fi
chown -R "$SERVICE_USER:$SERVICE_USER" "$INSTALL_DIR"

if ! command -v cargo >/dev/null 2>&1 || ! sudo -u "$SERVICE_USER" -H bash -lc 'command -v cargo' >/dev/null 2>&1; then
  log "Installing Rust toolchain (for $SERVICE_USER, via rustup)..."
  sudo -u "$SERVICE_USER" -H bash -lc "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable"
fi

log "Building release binary (this can take several minutes the first time)..."
sudo -u "$SERVICE_USER" -H bash -lc "cd '$INSTALL_DIR' && source \$HOME/.cargo/env 2>/dev/null || true; cargo build --release --locked"

log "Configuring PostgreSQL role and database..."
DB_PASS_FILE="$INSTALL_DIR/.db_password"
if [ -s "$DB_PASS_FILE" ]; then
  DB_PASS="$(cat "$DB_PASS_FILE")"
else
  DB_PASS="$(openssl rand -hex 24)"
  umask 077; printf '%s\n' "$DB_PASS" > "$DB_PASS_FILE"
  chown "$SERVICE_USER:$SERVICE_USER" "$DB_PASS_FILE"; chmod 600 "$DB_PASS_FILE"
fi

ROLE_EXISTS=$(sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='$DB_USER'")
if [ "$ROLE_EXISTS" = "1" ]; then
  sudo -u postgres psql -c "ALTER ROLE \"$DB_USER\" WITH LOGIN PASSWORD '$DB_PASS';" >/dev/null
else
  sudo -u postgres psql -c "CREATE ROLE \"$DB_USER\" WITH LOGIN PASSWORD '$DB_PASS';" >/dev/null
fi
DB_EXISTS=$(sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='$DB_NAME'")
if [ "$DB_EXISTS" != "1" ]; then
  sudo -u postgres createdb -O "$DB_USER" "$DB_NAME"
fi

ENV_FILE="$INSTALL_DIR/.env"
if [ ! -s "$ENV_FILE" ]; then
  log "Writing .env..."
  SESSION_KEY="$(openssl rand -hex 64)"
  EXTRA_LINES=""
  if [ -n "$DOMAIN" ]; then
    COOKIE_SECURE="true"
    BASE_URL="https://$DOMAIN"
    BIND_ADDR="127.0.0.1:$PORT"
    # Caddy / nginx on the same host proxies via loopback, which the app
    # already trusts by default — nothing extra needed.
  else
    # No --domain: bind to loopback by default. The app refuses non-loopback
    # binds without COOKIE_SECURE=true unless ALLOW_INSECURE_HTTP=true is set,
    # so we keep it loopback and let the operator decide.
    COOKIE_SECURE="false"
    BIND_ADDR="127.0.0.1:$PORT"
    BASE_URL="http://localhost:$PORT"
    EXTRA_LINES=$'\n# Default bind is loopback. To expose on a public interface,\n# either put a TLS reverse proxy in front (recommended) or set\n#   BIND_ADDR=0.0.0.0:'"$PORT"$'\n#   ALLOW_INSECURE_HTTP=true\n# Note: session cookies travel in cleartext over plain HTTP.'
  fi
  cat > "$ENV_FILE" <<EOF
# Generated by install.sh on $(date -u +%FT%TZ).
# Edit and then: sudo systemctl restart openexhibit

DATABASE_URL=postgres://$DB_USER:$DB_PASS@localhost:5432/$DB_NAME
BIND_ADDR=$BIND_ADDR
SESSION_KEY=$SESSION_KEY
RUST_LOG=openexhibit=info,actix_web=info,sqlx=warn
FILES_DIR=$INSTALL_DIR/files
STATIC_DIR=$INSTALL_DIR/static
COOKIE_SECURE=$COOKIE_SECURE
BASE_URL=$BASE_URL
$EXTRA_LINES

# SMTP for password-reset email is configured in the admin UI:
#   $BASE_URL/admin/settings  →  "SMTP (password-reset email)"
# The password is stored encrypted at rest using a key derived from SESSION_KEY.

# Comma-separated list of IPs whose X-Forwarded-For is trusted (your reverse
# proxy). Loopback is always trusted. Leave unset on simple installs.
# TRUSTED_PROXIES=
EOF
  chown "$SERVICE_USER:$SERVICE_USER" "$ENV_FILE"; chmod 600 "$ENV_FILE"
else
  log ".env already exists — leaving it alone."
  sed -i -E "s|^DATABASE_URL=.*|DATABASE_URL=postgres://$DB_USER:$DB_PASS@localhost:5432/$DB_NAME|" "$ENV_FILE"
fi

log "Installing systemd unit..."
cat > /etc/systemd/system/openexhibit.service <<EOF
[Unit]
Description=OpenExhibit (self-hosted portfolio CMS)
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$INSTALL_DIR
EnvironmentFile=$INSTALL_DIR/.env
ExecStart=$INSTALL_DIR/target/release/openexhibit
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=$INSTALL_DIR/files $INSTALL_DIR/static

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable openexhibit.service >/dev/null

if [ -n "$DOMAIN" ]; then
  log "Writing Caddyfile for $DOMAIN..."
  cat > /etc/caddy/Caddyfile <<EOF
$DOMAIN {
    encode zstd gzip
    reverse_proxy 127.0.0.1:$PORT
}
EOF
  systemctl reload caddy || systemctl restart caddy
fi

log "Starting openexhibit..."
systemctl restart openexhibit.service

# Wait briefly for first-boot output (random admin password is logged once).
sleep 2
ADMIN_LINE=$(journalctl -u openexhibit.service --since "2 min ago" --no-pager 2>/dev/null | grep -iE "admin.*(password|created)" | tail -5 || true)

cat <<EOF

${BOLD}${GRN}Done.${RST}

  Service     : systemctl status openexhibit
  Logs        : journalctl -u openexhibit -f
  URL         : $( [ -n "$DOMAIN" ] && echo "https://$DOMAIN" || echo "http://127.0.0.1:$PORT  (loopback only)" )
  Admin path  : /admin
  Env file    : $INSTALL_DIR/.env

To change anything (SMTP for password-reset mail, bind address, log level):
  sudo -e $INSTALL_DIR/.env  &&  sudo systemctl restart openexhibit

EOF

if [ -n "$ADMIN_LINE" ]; then
  echo "${BOLD}First-boot admin credentials (from journal):${RST}"
  echo "$ADMIN_LINE"
  echo
  echo "${DIM}If you missed them, the only fix is to wipe and reinit:${RST}"
  echo "  sudo systemctl stop openexhibit && sudo -u postgres dropdb $DB_NAME && sudo -u postgres createdb -O $DB_USER $DB_NAME && sudo systemctl start openexhibit"
else
  echo "${YLW}First-boot admin credentials should be in: journalctl -u openexhibit --since '5 min ago'${RST}"
fi
