#!/usr/bin/env bash
# Create a MariaDB/MySQL database and user, with optional privilege level.
# Usage: sudo ./db-create.sh <db_name> <db_user> <db_password> <privileges>
#   privileges: full (ALL PRIVILEGES) or readonly (SELECT only)
# Optional: set MYSQL_ROOT_PASSWORD if MariaDB root has a password.

set -e

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 <db_name> <db_user> <db_password> <privileges>" >&2
  echo "  privileges: full | readonly" >&2
  exit 1
fi

DB_NAME="$1"
DB_USER="$2"
DB_PASS="$3"
PRIV="$4"

if [[ "$PRIV" != "full" && "$PRIV" != "readonly" ]]; then
  echo "Error: privileges must be 'full' or 'readonly'." >&2
  exit 1
fi

if ! command -v mysql &>/dev/null; then
  echo "Error: mysql client not found. Install MariaDB/MySQL." >&2
  exit 1
fi

if ! systemctl is-active --quiet mariadb 2>/dev/null && ! systemctl is-active --quiet mysql 2>/dev/null; then
  echo "Error: MariaDB/MySQL is not running." >&2
  exit 1
fi

if [[ -n "${MYSQL_ROOT_PASSWORD:-}" ]]; then
  export MYSQL_PWD="$MYSQL_ROOT_PASSWORD"
fi

MYSQL_ERR=$(mktemp)
if [[ "$PRIV" == "readonly" ]]; then
  GRANT_SQL="GRANT SELECT ON \`$DB_NAME\`.* TO '$DB_USER'@'localhost'; GRANT SELECT ON \`$DB_NAME\`.* TO '$DB_USER'@'127.0.0.1';"
else
  GRANT_SQL="GRANT ALL PRIVILEGES ON \`$DB_NAME\`.* TO '$DB_USER'@'localhost'; GRANT ALL PRIVILEGES ON \`$DB_NAME\`.* TO '$DB_USER'@'127.0.0.1';"
fi

CMD="CREATE DATABASE IF NOT EXISTS \`$DB_NAME\`; CREATE USER IF NOT EXISTS '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASS'; CREATE USER IF NOT EXISTS '$DB_USER'@'127.0.0.1' IDENTIFIED BY '$DB_PASS'; $GRANT_SQL FLUSH PRIVILEGES;"

if ! mysql -u root -e "$CMD" 2>"$MYSQL_ERR"; then
  echo "Error: Could not create database or user." >&2
  cat "$MYSQL_ERR" >&2
  rm -f "$MYSQL_ERR"
  [[ -n "${MYSQL_PWD:-}" ]] && unset MYSQL_PWD
  exit 1
fi

rm -f "$MYSQL_ERR"
[[ -n "${MYSQL_PWD:-}" ]] && unset MYSQL_PWD
echo "Database created: $DB_NAME (user: $DB_USER, privileges: $PRIV)"
