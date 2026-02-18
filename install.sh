#!/usr/bin/env bash
# FrankenPHP Panel – server installer
# Usage: sudo ./install.sh [OPTIONS]
# Options:
#   --prefix DIR     Install directory (default: /opt/frankenphp-panel)
#   --user USER      Run panel as this user (default: panel)
#   --no-systemd     Do not install systemd service
#   --no-build       Do not build; only install files (binary must exist in target/release/)
#   --skip-deps      Do not install system dependencies (Rust, PostgreSQL client libs)

set -e

PREFIX="${PREFIX:-/opt/frankenphp-panel}"
PANEL_USER="${PANEL_USER:-panel}"
INSTALL_SYSTEMD=true
SKIP_BUILD=false
SKIP_DEPS=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --prefix)    PREFIX="$2"; shift 2 ;;
    --user)      PANEL_USER="$2"; shift 2 ;;
    --no-systemd) INSTALL_SYSTEMD=false; shift ;;
    --no-build)  SKIP_BUILD=true; shift ;;
    --skip-deps) SKIP_DEPS=true; shift ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "==> FrankenPHP Panel installer"
echo "    Prefix: $PREFIX"
echo "    User:   $PANEL_USER"
echo ""

# --- Dependencies ---
if [[ "$SKIP_DEPS" != true ]]; then
  echo "==> Checking dependencies..."
  if command -v apt-get &>/dev/null; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq curl build-essential pkg-config libssl-dev libpq-dev postgresql-client || true
  elif command -v dnf &>/dev/null; then
    dnf install -y curl gcc gcc-c++ make pkg-config openssl-devel postgresql-devel postgresql || true
  elif command -v yum &>/dev/null; then
    yum install -y curl gcc gcc-c++ make pkg-config openssl-devel postgresql-devel postgresql || true
  else
    echo "Warning: Unsupported package manager. Install manually: curl, build-essential, libssl-dev, libpq-dev, postgresql-client"
  fi

  if ! command -v cargo &>/dev/null; then
    echo "==> Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --default-toolchain stable
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  export PATH="${HOME/.cargo/bin:-$HOME/.cargo/bin}:$PATH"
  if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Add ~/.cargo/bin to PATH and re-run."
    exit 1
  fi
fi

# --- Build ---
if [[ "$SKIP_BUILD" != true ]]; then
  echo "==> Building panel (release)..."
  cargo build --release -q
  BINARY="$SCRIPT_DIR/target/release/frankenphp-panel"
  if [[ ! -f "$BINARY" ]]; then
    echo "Error: Binary not found at $BINARY"
    exit 1
  fi
else
  BINARY="$SCRIPT_DIR/target/release/frankenphp-panel"
  if [[ ! -f "$BINARY" ]]; then
    echo "Error: --no-build set but binary not found at $BINARY"
    exit 1
  fi
  echo "==> Skipping build (using existing binary)"
fi

# --- Create user ---
if ! getent passwd "$PANEL_USER" &>/dev/null; then
  echo "==> Creating user: $PANEL_USER"
  useradd -r -s /usr/sbin/nologin -d "$PREFIX" "$PANEL_USER" 2>/dev/null || true
fi

# --- Install directory ---
echo "==> Installing to $PREFIX"
mkdir -p "$PREFIX"
mkdir -p "$PREFIX/static"
mkdir -p "$PREFIX/migrations"

install -m 755 "$BINARY" "$PREFIX/frankenphp-panel"
cp -r "$SCRIPT_DIR/static/"* "$PREFIX/static/"
cp "$SCRIPT_DIR/migrations/"*.sql "$PREFIX/migrations/"

# --- Generate secrets and .env ---
if [[ ! -f "$PREFIX/.env" ]]; then
  echo "==> Generating random passwords and session secret..."
  DB_PASS=$(openssl rand -base64 24 | tr -dc 'a-zA-Z0-9' | head -c 24)
  SESSION_SECRET=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 48)
  ADMIN_PASS=$(openssl rand -base64 16 | tr -dc 'a-zA-Z0-9' | head -c 16)

  # Create PostgreSQL user and database if postgres is available
  if command -v psql &>/dev/null && sudo -u postgres psql -c '\q' 2>/dev/null; then
    echo "==> Creating PostgreSQL user and database..."
    sudo -u postgres psql -c "DROP USER IF EXISTS panel;" 2>/dev/null || true
    sudo -u postgres psql -c "CREATE USER panel WITH PASSWORD '$DB_PASS';" 2>/dev/null || true
    sudo -u postgres psql -c "DROP DATABASE IF EXISTS panel;" 2>/dev/null || true
    sudo -u postgres psql -c "CREATE DATABASE panel OWNER panel;" 2>/dev/null || true
  else
    echo "==> PostgreSQL not detected or not running – skipping DB creation."
    echo "    Create user and database manually, then edit $PREFIX/.env"
    DB_PASS="CHANGE_ME"
    ADMIN_PASS=""
  fi

  DATABASE_URL="postgres://panel:${DB_PASS}@127.0.0.1/panel"
  cat > "$PREFIX/.env" << EOF
DATABASE_URL=$DATABASE_URL
PANEL_SESSION_SECRET=$SESSION_SECRET
EOF
  chmod 600 "$PREFIX/.env"
  echo "==> Wrote $PREFIX/.env with generated values"
else
  echo "==> Keeping existing $PREFIX/.env"
  ADMIN_PASS=""
  DB_PASS=""
  SESSION_SECRET=""
  DATABASE_URL=""
fi

chown -R "$PANEL_USER:$PANEL_USER" "$PREFIX"

# --- Run migrations and set admin password (only if we generated .env and have ADMIN_PASS) ---
if [[ -n "$ADMIN_PASS" ]] && [[ -n "$DATABASE_URL" ]]; then
  echo "==> Running database migrations..."
  sudo -u "$PANEL_USER" env DATABASE_URL="$DATABASE_URL" PANEL_SESSION_SECRET="$SESSION_SECRET" bash -c "cd $PREFIX && ./frankenphp-panel migrate" || true
  echo "==> Setting admin password..."
  sudo -u "$PANEL_USER" env DATABASE_URL="$DATABASE_URL" PANEL_SESSION_SECRET="$SESSION_SECRET" bash -c "cd $PREFIX && ./frankenphp-panel set-admin-password \"$ADMIN_PASS\"" || true
fi

# --- Systemd ---
if [[ "$INSTALL_SYSTEMD" == true ]]; then
  echo "==> Installing systemd service"
  SVC_FILE="$SCRIPT_DIR/scripts/frankenphp-panel.service"
  if [[ -f "$SVC_FILE" ]]; then
    sed -e "s|/opt/frankenphp-panel|$PREFIX|g" \
        -e "s|User=panel|User=$PANEL_USER|g" \
        -e "s|Group=panel|Group=$PANEL_USER|g" \
        "$SVC_FILE" > /etc/systemd/system/frankenphp-panel.service
    systemctl daemon-reload
    echo "    Service file: /etc/systemd/system/frankenphp-panel.service"
  else
    cat > /etc/systemd/system/frankenphp-panel.service << EOF
[Unit]
Description=FrankenPHP Panel
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=$PANEL_USER
Group=$PANEL_USER
WorkingDirectory=$PREFIX
EnvironmentFile=$PREFIX/.env
ExecStart=$PREFIX/frankenphp-panel
Restart=on-failure
RestartSec=5
NoNewPrivileges=yes
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
  fi
fi

echo ""
echo "==> Install complete"
echo ""

# Save and print credentials when we generated them
if [[ -n "$ADMIN_PASS" ]] && [[ -n "$DATABASE_URL" ]]; then
  CREDS_FILE="$PREFIX/.panel-credentials"
  cat > "$CREDS_FILE" << EOF
# FrankenPHP Panel – save these and then delete this file: rm $CREDS_FILE
Panel URL:  http://127.0.0.1:2090
Username:   admin
Password:   $ADMIN_PASS
EOF
  chown "$PANEL_USER:$PANEL_USER" "$CREDS_FILE"
  chmod 600 "$CREDS_FILE"
  echo "-------------------------------------------"
  echo "  Panel URL:   http://127.0.0.1:2090"
  echo "  Username:    admin"
  echo "  Password:    $ADMIN_PASS"
  echo "-------------------------------------------"
  echo ""
  echo "Credentials saved to $CREDS_FILE – save them and remove the file: sudo rm $CREDS_FILE"
  echo ""
fi

echo "Next steps:"
echo "  1. Start the panel:"
if [[ "$INSTALL_SYSTEMD" == true ]]; then
  echo "     sudo systemctl start frankenphp-panel"
  echo "     sudo systemctl enable frankenphp-panel   # start on boot"
  echo "     sudo systemctl status frankenphp-panel"
else
  echo "     cd $PREFIX && sudo -u $PANEL_USER ./frankenphp-panel"
fi
echo ""
echo "  2. Open http://127.0.0.1:2090 (put Caddy or nginx in front for HTTPS and a public URL)."
if [[ -z "$ADMIN_PASS" ]]; then
  echo "  3. If you did not get credentials above, create DB and run migrations, then: $PREFIX/frankenphp-panel set-admin-password YOUR_PASSWORD"
fi
echo ""
