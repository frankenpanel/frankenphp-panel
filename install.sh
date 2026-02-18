#!/usr/bin/env bash
# FrankenPHP Panel – full-stack installer for a fresh server
#
# Installs everything required so you can log in to the panel and add PHP/WordPress
# sites; the panel manages site folders, Caddy config, FrankenPHP reload, and
# (for WordPress) MariaDB DB + WP files behind the scenes.
#
# Usage: sudo ./install.sh [OPTIONS]
# Options:
#   --prefix DIR     Install directory (default: /opt/frankenphp-panel)
#   --user USER      Run panel as this user (default: panel)
#   --skip-caddy     Do not install FrankenPHP (only if you already have it)
#   --no-systemd     Do not install systemd service
#   --no-build       Do not build; use existing binary in target/release/
#   --skip-deps      Do not install system packages or Rust (only if already present)

set -e

PREFIX="${PREFIX:-/opt/frankenphp-panel}"
PANEL_USER="${PANEL_USER:-panel}"
INSTALL_SYSTEMD=true
SKIP_BUILD=false
SKIP_DEPS=false
SKIP_CADDY=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --prefix)      PREFIX="$2"; shift 2 ;;
    --user)        PANEL_USER="$2"; shift 2 ;;
    --skip-caddy)  SKIP_CADDY=true; shift ;;
    --no-systemd)  INSTALL_SYSTEMD=false; shift ;;
    --no-build)    SKIP_BUILD=true; shift ;;
    --skip-deps)   SKIP_DEPS=true; shift ;;
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
    apt-get install -y -qq curl build-essential pkg-config libssl-dev libpq-dev postgresql postgresql-client mariadb-server php-cli php-mysql || true
  elif command -v dnf &>/dev/null; then
    dnf install -y curl gcc gcc-c++ make pkg-config openssl-devel postgresql-devel postgresql postgresql-server mariadb-server php-cli php-mysqlnd || true
    if command -v postgresql-setup &>/dev/null; then postgresql-setup --initdb 2>/dev/null || true; fi
  elif command -v yum &>/dev/null; then
    yum install -y curl gcc gcc-c++ make pkg-config openssl-devel postgresql-devel postgresql postgresql-server mariadb-server php-cli php-mysqlnd || true
    if command -v postgresql-setup &>/dev/null; then postgresql-setup --initdb 2>/dev/null || true; fi
  else
    echo "Warning: Unsupported package manager. Install manually: curl, build-essential, libssl-dev, libpq-dev, postgresql, postgresql-client"
  fi

  # Start and enable PostgreSQL so DB is ready for the panel
  if command -v systemctl &>/dev/null; then
    echo "==> Starting PostgreSQL..."
    systemctl start postgresql 2>/dev/null || systemctl start postgresql@*-main 2>/dev/null || true
    systemctl enable postgresql 2>/dev/null || systemctl enable postgresql@*-main 2>/dev/null || true
    echo "==> Waiting for PostgreSQL to accept connections..."
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
      if sudo -u postgres psql -c '\q' 2>/dev/null; then break; fi
      sleep 1
    done
    echo "==> Starting MariaDB (for WordPress sites)..."
    systemctl start mariadb 2>/dev/null || systemctl start mysql 2>/dev/null || true
    systemctl enable mariadb 2>/dev/null || systemctl enable mysql 2>/dev/null || true
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

  # WP-CLI for completing WordPress install (admin user, title, etc.) when adding a site
  if ! command -v wp &>/dev/null && command -v php &>/dev/null; then
    echo "==> Installing WP-CLI..."
    curl -sSLf "https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar" -o /tmp/wp-cli.phar
    chmod +x /tmp/wp-cli.phar
    mv /tmp/wp-cli.phar /usr/local/bin/wp 2>/dev/null || true
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
mkdir -p "$PREFIX/scripts"

install -m 755 "$BINARY" "$PREFIX/frankenphp-panel"
cp -r "$SCRIPT_DIR/static/"* "$PREFIX/static/"
cp "$SCRIPT_DIR/migrations/"*.sql "$PREFIX/migrations/"
if [[ -f "$SCRIPT_DIR/scripts/site-create.sh" ]]; then
  install -m 755 "$SCRIPT_DIR/scripts/site-create.sh" "$PREFIX/scripts/site-create.sh"
fi
if [[ -f "$SCRIPT_DIR/scripts/install-frankenphp.sh" ]]; then
  install -m 755 "$SCRIPT_DIR/scripts/install-frankenphp.sh" "$PREFIX/scripts/install-frankenphp.sh"
fi

# --- Generate secrets and .env ---
if [[ ! -f "$PREFIX/.env" ]]; then
  echo "==> Generating random PostgreSQL password, session secret, and admin password..."
  DB_PASS=$(openssl rand -base64 24 | tr -dc 'a-zA-Z0-9' | head -c 24)
  SESSION_SECRET=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 48)
  ADMIN_PASS=$(openssl rand -base64 16 | tr -dc 'a-zA-Z0-9' | head -c 16)

  # Create PostgreSQL user and database with the generated password; .env will be updated below
  if command -v psql &>/dev/null && sudo -u postgres psql -c '\q' 2>/dev/null; then
    echo "==> Creating PostgreSQL user and database (random password)..."
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
PANEL_BIND=0.0.0.0:2090
SITE_CREATE_SCRIPT=$PREFIX/scripts/site-create.sh
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
  sudo -u "$PANEL_USER" env DATABASE_URL="$DATABASE_URL" PANEL_SESSION_SECRET="$SESSION_SECRET" bash -c "cd $PREFIX && ./frankenphp-panel migrate"
  echo "==> Setting admin password..."
  sudo -u "$PANEL_USER" env DATABASE_URL="$DATABASE_URL" PANEL_SESSION_SECRET="$SESSION_SECRET" bash -c "cd $PREFIX && ./frankenphp-panel set-admin-password \"$ADMIN_PASS\""
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
# NoNewPrivileges=yes would block sudo (needed for site-create.sh)
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
  fi
fi

# --- Sudoers: allow panel user to run site-create.sh (creates /var/www, Caddy config, reload) ---
if [[ -f "$PREFIX/scripts/site-create.sh" ]]; then
  SUDOERS_FILE="/etc/sudoers.d/frankenphp-panel-site-create"
  echo "$PANEL_USER ALL=(root) NOPASSWD: $PREFIX/scripts/site-create.sh" > "$SUDOERS_FILE"
  chmod 440 "$SUDOERS_FILE"
  echo "==> Configured sudoers: $PANEL_USER may run site-create.sh"
  mkdir -p /etc/caddy/sites
  echo "==> Created /etc/caddy/sites (Caddy include dir for new sites)"
fi

# --- Install FrankenPHP (Caddy+PHP) so panel-created sites are served on 80/443 ---
if [[ "$SKIP_CADDY" != true ]] && [[ -f "$SCRIPT_DIR/scripts/install-frankenphp.sh" ]]; then
  echo "==> Installing FrankenPHP (sites will be served on ports 80/443)..."
  bash "$SCRIPT_DIR/scripts/install-frankenphp.sh"
fi

# --- Firewall (allow port 2090 if ufw is active) ---
if command -v ufw &>/dev/null && sudo ufw status 2>/dev/null | grep -q "Status: active"; then
  echo "==> Allowing port 2090 in firewall (ufw)..."
  sudo ufw allow 2090/tcp 2>/dev/null || true
  sudo ufw status 2>/dev/null | grep 2090 || true
fi

# --- Start panel service so it is running when install finishes ---
if [[ "$INSTALL_SYSTEMD" == true ]]; then
  echo "==> Starting FrankenPHP Panel service..."
  systemctl start frankenphp-panel 2>/dev/null || true
  systemctl enable frankenphp-panel 2>/dev/null || true
fi

# --- Detect server IP for display ---
SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}')
[[ -z "$SERVER_IP" ]] && SERVER_IP=$(ip -4 route get 8.8.8.8 2>/dev/null | grep -oE 'src [0-9.]+' | awk '{print $2}')
[[ -z "$SERVER_IP" ]] && SERVER_IP="127.0.0.1"
PANEL_URL="http://${SERVER_IP}:2090"

echo ""
echo "=============================================="
echo "  FrankenPHP Panel – install complete"
echo "=============================================="
echo ""

# Save and print all credentials when we generated them
if [[ -n "$ADMIN_PASS" ]] && [[ -n "$DATABASE_URL" ]]; then
  CREDS_FILE="$PREFIX/.panel-credentials"
  cat > "$CREDS_FILE" << EOF
# FrankenPHP Panel – SAVE THESE THEN DELETE: sudo rm $CREDS_FILE
# --- Panel (web UI) ---
Panel URL:     $PANEL_URL
Admin user:    admin
Admin pass:    $ADMIN_PASS

# --- PostgreSQL (panel database) ---
DB host:       127.0.0.1
DB port:       5432
DB name:       panel
DB user:       panel
DB password:   $DB_PASS
Connection:    postgres://panel:$DB_PASS@127.0.0.1/panel
EOF
  chown "$PANEL_USER:$PANEL_USER" "$CREDS_FILE"
  chmod 600 "$CREDS_FILE"

  echo "  PANEL (web UI)"
  echo "  -------------"
  echo "  URL:      $PANEL_URL"
  echo "  Username: admin"
  echo "  Password: $ADMIN_PASS"
  echo ""
  echo "  POSTGRESQL (panel database)"
  echo "  ---------------------------"
  echo "  Host:     127.0.0.1:5432"
  echo "  Database: panel"
  echo "  User:     panel"
  echo "  Password: $DB_PASS"
  echo ""
  echo "  Full credentials saved to: $CREDS_FILE"
  echo "  (Save them, then remove file: sudo rm $CREDS_FILE)"
  echo ""
else
  echo "  (Existing .env kept – no new credentials generated)"
  echo "  Panel URL: $PANEL_URL"
  echo ""
fi

echo "  NEXT STEP"
echo "  ---------"
if [[ "$INSTALL_SYSTEMD" == true ]]; then
  echo "  Panel is running. Open in browser: $PANEL_URL"
else
  echo "  1. Start the panel: cd $PREFIX && sudo -u $PANEL_USER ./frankenphp-panel"
  echo "  2. Open in browser: $PANEL_URL"
fi
if [[ -f "$PREFIX/scripts/site-create.sh" ]]; then
  echo ""
  if [[ "$SKIP_CADDY" != true ]]; then
    echo "  Add Site creates /var/www/<domain>, Caddy config, and reloads FrankenPHP – sites go live on ports 80/443."
  else
    echo "  Add Site creates /var/www/<domain> and Caddy config. To serve sites, run: sudo $PREFIX/scripts/install-frankenphp.sh"
  fi
fi
if [[ -z "$ADMIN_PASS" ]]; then
  echo ""
  echo "  Set admin password: sudo -u $PANEL_USER $PREFIX/frankenphp-panel set-admin-password YOUR_PASSWORD"
fi
echo ""
echo "=============================================="
echo ""
