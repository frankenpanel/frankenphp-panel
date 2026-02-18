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

# --- Environment file ---
if [[ ! -f "$PREFIX/.env" ]]; then
  if [[ -f "$SCRIPT_DIR/scripts/env.example" ]]; then
    cp "$SCRIPT_DIR/scripts/env.example" "$PREFIX/.env"
    echo "==> Created $PREFIX/.env – please edit and set DATABASE_URL and PANEL_SESSION_SECRET"
  else
    cat > "$PREFIX/.env" << 'EOF'
DATABASE_URL=postgres://panel:CHANGE_ME@127.0.0.1/panel
PANEL_SESSION_SECRET=change-me-min-32-characters-long
EOF
    echo "==> Created $PREFIX/.env – please edit and set DATABASE_URL and PANEL_SESSION_SECRET"
  fi
  chmod 600 "$PREFIX/.env"
else
  echo "==> Keeping existing $PREFIX/.env"
fi

chown -R "$PANEL_USER:$PANEL_USER" "$PREFIX"

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
echo "Next steps:"
echo "  1. Create PostgreSQL database and user (if not already):"
echo "     sudo -u postgres createuser -P panel"
echo "     sudo -u postgres createdb -O panel panel"
echo ""
echo "  2. Edit $PREFIX/.env and set:"
echo "     - DATABASE_URL (e.g. postgres://panel:YOUR_PASSWORD@127.0.0.1/panel)"
echo "     - PANEL_SESSION_SECRET (e.g. \$(openssl rand -base64 32))"
echo ""
echo "  3. Start the panel:"
if [[ "$INSTALL_SYSTEMD" == true ]]; then
  echo "     sudo systemctl start frankenphp-panel"
  echo "     sudo systemctl enable frankenphp-panel   # start on boot"
  echo "     sudo systemctl status frankenphp-panel"
else
  echo "     cd $PREFIX && sudo -u $PANEL_USER ./frankenphp-panel   # or use your own process manager"
fi
echo ""
echo "  4. Panel listens on http://127.0.0.1:2090 – put Caddy (or nginx) in front for TLS and public access."
echo "  5. Default login: admin (change password after first login or via SQL)."
echo ""
