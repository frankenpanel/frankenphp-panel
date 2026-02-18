#!/usr/bin/env bash
# Install FrankenPHP (Caddy + PHP) so panel-created sites are served.
# Usage: sudo ./install-frankenphp.sh
# Creates: /usr/local/bin/frankenphp, /etc/caddy/Caddyfile, frankenphp.service

set -e

FRANKENPHP_VERSION="${FRANKENPHP_VERSION:-v1.11.2}"
CADDYFILE="${CADDYFILE:-/etc/caddy/Caddyfile}"
CADDY_SITES_DIR="${CADDY_SITES_DIR:-/etc/caddy/sites}"

echo "==> Installing FrankenPHP (${FRANKENPHP_VERSION})..."

# Detect arch
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)  BINARY="frankenphp-linux-x86_64" ;;
  aarch64|arm64) BINARY="frankenphp-linux-aarch64" ;;
  *) echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

URL="https://github.com/php/frankenphp/releases/download/${FRANKENPHP_VERSION}/${BINARY}"
echo "==> Downloading $URL"
curl -sSLf -o /tmp/frankenphp "$URL"
chmod 755 /tmp/frankenphp
mv /tmp/frankenphp /usr/local/bin/frankenphp
echo "    Installed /usr/local/bin/frankenphp"

# Caddyfile that imports panel-created site configs
mkdir -p "$CADDY_SITES_DIR"
if [[ ! -f "$CADDYFILE" ]]; then
  cat > "$CADDYFILE" << 'CADDY'
# FrankenPHP – panel-created sites are in import below
{
}
import /etc/caddy/sites/*
CADDY
  echo "==> Created $CADDYFILE (imports /etc/caddy/sites/*)"
else
  if ! grep -q 'import /etc/caddy/sites' "$CADDYFILE" 2>/dev/null; then
    echo "" >> "$CADDYFILE"
    echo "import /etc/caddy/sites/*" >> "$CADDYFILE"
    echo "==> Appended 'import /etc/caddy/sites/*' to $CADDYFILE"
  fi
fi

# systemd service (run on ports 80/443; needs CAP_NET_BIND_SERVICE or run as root)
if [[ ! -f /etc/systemd/system/frankenphp.service ]]; then
  cat > /etc/systemd/system/frankenphp.service << 'SVC'
[Unit]
Description=FrankenPHP (Caddy + PHP)
After=network.target network-online.target
Requires=network-online.target

[Service]
Type=simple
User=root
Group=root
ExecStartPre=/usr/local/bin/frankenphp validate --config /etc/caddy/Caddyfile
ExecStart=/usr/local/bin/frankenphp run --config /etc/caddy/Caddyfile
ExecReload=/usr/local/bin/frankenphp reload --config /etc/caddy/Caddyfile --force
Restart=on-failure
RestartSec=3
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
SVC
  systemctl daemon-reload
  echo "==> Installed systemd service frankenphp.service"
fi

# Firewall
if command -v ufw &>/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
  ufw allow 80/tcp 2>/dev/null || true
  ufw allow 443/tcp 2>/dev/null || true
  echo "==> Allowed ports 80 and 443 in ufw"
fi

systemctl enable frankenphp 2>/dev/null || true
systemctl start frankenphp 2>/dev/null || true
echo "==> Started frankenphp.service"
echo ""
echo "Sites in /etc/caddy/sites/ are now served. Reload after adding a site: sudo systemctl reload frankenphp"
echo "Test: curl -H 'Host: site.sohail.work' http://127.0.0.1/"
