#!/usr/bin/env bash
# Drop a MariaDB/MySQL database and user.
# Usage: sudo ./db-delete.sh <db_name> <db_user>
# Optional: set MYSQL_ROOT_PASSWORD if MariaDB root has a password.

set -e

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <db_name> <db_user>" >&2
  exit 1
fi

DB_NAME="$1"
DB_USER="$2"

if ! command -v mysql &>/dev/null; then
  echo "Error: mysql client not found." >&2
  exit 1
fi

if [[ -n "${MYSQL_ROOT_PASSWORD:-}" ]]; then
  export MYSQL_PWD="$MYSQL_ROOT_PASSWORD"
fi

MYSQL_ERR=$(mktemp)
if ! mysql -u root -e "DROP DATABASE IF EXISTS \`$DB_NAME\`; DROP USER IF EXISTS '$DB_USER'@'localhost'; DROP USER IF EXISTS '$DB_USER'@'127.0.0.1'; FLUSH PRIVILEGES;" 2>"$MYSQL_ERR"; then
  echo "Error: Could not drop database or user." >&2
  cat "$MYSQL_ERR" >&2
  rm -f "$MYSQL_ERR"
  [[ -n "${MYSQL_PWD:-}" ]] && unset MYSQL_PWD
  exit 1
fi

rm -f "$MYSQL_ERR"
[[ -n "${MYSQL_PWD:-}" ]] && unset MYSQL_PWD
echo "Database removed: $DB_NAME"
