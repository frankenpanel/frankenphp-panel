#!/usr/bin/env bash
# Remove site directory, Caddy config, and optionally drop MariaDB databases.
# Usage: sudo ./site-delete.sh <domain> <site_path> [db_name1 db_user1] [db_name2 db_user2] ...
# Optional: set MYSQL_ROOT_PASSWORD if MariaDB root has a password.

set -e

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <domain> <site_path> [db_name1 db_user1] [db_name2 db_user2] ..." >&2
  exit 1
fi

DOMAIN="$1"
SITE_PATH="$2"
shift 2
CADDY_SITES_DIR="${CADDY_SITES_DIR:-/etc/caddy/sites}"
CADDYFILE="${CADDYFILE:-/etc/caddy/Caddyfile}"

# Remove Caddy snippet so the site is no longer served
SAFE_DOMAIN="${DOMAIN//\*/_}"
CONF_FILE="$CADDY_SITES_DIR/${SAFE_DOMAIN}.conf"
if [[ -f "$CONF_FILE" ]]; then
  rm -f "$CONF_FILE"
  echo "Removed Caddy config: $CONF_FILE"
fi

# Remove site directory and all files
if [[ -d "$SITE_PATH" ]]; then
  rm -rf "$SITE_PATH"
  echo "Removed site directory: $SITE_PATH"
fi

# Drop MariaDB databases and users (pairs of db_name db_user)
if [[ $# -ge 2 ]] && command -v mysql &>/dev/null; then
  if [[ -n "${MYSQL_ROOT_PASSWORD:-}" ]]; then
    export MYSQL_PWD="$MYSQL_ROOT_PASSWORD"
  fi
  while [[ $# -ge 2 ]]; do
    DB_NAME="$1"
    DB_USER="$2"
    shift 2
    if mysql -u root -e "DROP DATABASE IF EXISTS \`$DB_NAME\`; DROP USER IF EXISTS '$DB_USER'@'localhost'; DROP USER IF EXISTS '$DB_USER'@'127.0.0.1'; FLUSH PRIVILEGES;" 2>/dev/null; then
      echo "Dropped database and user: $DB_NAME / $DB_USER"
    fi
  done
  [[ -n "${MYSQL_PWD:-}" ]] && unset MYSQL_PWD
fi

# Reload Caddy/FrankenPHP so the site is no longer active
if [[ -n "$CADDY_RELOAD_CMD" ]]; then
  eval "$CADDY_RELOAD_CMD"
elif systemctl is-active --quiet frankenphp 2>/dev/null; then
  systemctl reload frankenphp 2>/dev/null || true
elif systemctl is-active --quiet caddy 2>/dev/null; then
  systemctl reload caddy 2>/dev/null || true
elif command -v frankenphp &>/dev/null && [[ -f "$CADDYFILE" ]]; then
  frankenphp reload --config "$CADDYFILE" --force 2>/dev/null || true
elif command -v caddy &>/dev/null && [[ -f "$CADDYFILE" ]]; then
  caddy reload --config "$CADDYFILE" 2>/dev/null || true
else
  echo "Warning: Caddy/FrankenPHP reload skipped." >&2
fi

echo "Site removed: $DOMAIN"
