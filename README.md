# FrankenPHP Panel

Lightweight hosting panel for managing PHP and WordPress websites. Rust backend (port **2090**), PostgreSQL for panel metadata, HTML templates + CSS/JS served from the backend.

## Features

- **Login** – Username/password, validation, loading state
- **Dashboard** – List sites (status, path, WordPress), Add Site, Add Database, search/filter
- **Add Site** – Domain, folder path, optional WordPress install; validation and success/error messages
- **Add Database** – Site, DB name, user, password; validation (forbidden chars, length, strength)
- **Website details** – Domain, folder, DB list, SSL status, Restart / Delete

## Requirements

- Rust 1.70+
- PostgreSQL (panel metadata)
- (Optional) MariaDB/MySQL for site databases

## Install on server

Use the included installer to build and install the panel, create a system user, and optionally install a systemd service:

```bash
# Clone or upload the project to the server, then:
sudo ./install.sh
```

Options:

- `--prefix DIR` – Install directory (default: `/opt/frankenphp-panel`)
- `--user USER` – User to run the panel (default: `panel`)
- `--no-systemd` – Do not install systemd service
- `--no-build` – Skip build; only copy files (binary must exist in `target/release/`)
- `--skip-deps` – Do not install system packages (Rust, libpq, etc.)

The installer will:

1. Install dependencies (Debian/Ubuntu: `apt-get`; RHEL/Fedora: `dnf`/`yum`) and Rust if needed
2. Build the panel with `cargo build --release`
3. Create user `panel` (or `--user`), install binary + `static/` + `migrations/` under `--prefix`
4. **If `.env` does not exist:** generate random database password, session secret, and admin password; create PostgreSQL user and database `panel` (if `psql`/postgres is available); write `.env`; run migrations; set admin password
5. Install `frankenphp-panel.service` and run `systemctl daemon-reload`
6. **Print and save** panel URL, username (`admin`), and generated password to the console and to `$PREFIX/.panel-credentials` (chmod 600)

After install:

1. If the installer created the DB and credentials, **save the printed URL, username, and password** (and remove the credentials file when done: `sudo rm /opt/frankenphp-panel/.panel-credentials`).
2. If PostgreSQL was not available during install, create the database and user, edit `/opt/frankenphp-panel/.env`, then run migrations and set admin password:
   ```bash
   sudo -u postgres createuser -P panel
   sudo -u postgres createdb -O panel panel
   # Edit .env with DATABASE_URL and PANEL_SESSION_SECRET, then:
   cd /opt/frankenphp-panel && sudo -u panel ./frankenphp-panel migrate
   cd /opt/frankenphp-panel && sudo -u panel ./frankenphp-panel set-admin-password YOUR_PASSWORD
   ```
3. Start: `sudo systemctl start frankenphp-panel` and enable: `sudo systemctl enable frankenphp-panel`
4. Open the panel at the URL shown (or put Caddy/nginx in front of `http://127.0.0.1:2090` for HTTPS)

## Setup (development)

1. **PostgreSQL**

   Create DB and user, e.g.:

   ```bash
   createdb panel
   createuser -P panel
   # Grant all on database panel to panel
   ```

2. **Environment**

   ```bash
   export DATABASE_URL="postgres://panel:YOUR_PASSWORD@127.0.0.1/panel"
   export PANEL_SESSION_SECRET="your-min-32-char-secret"
   # Optional: bind address (default 127.0.0.1:2090)
   export PANEL_BIND="127.0.0.1:2090"
   ```

3. **Run**

   ```bash
   cargo run
   ```

   Panel: **http://127.0.0.1:2090** (expose via Caddy reverse proxy in production).

4. **Default login**

   Migration seeds user `admin`. The default password may be `admin` (development only). For production, set `password_hash` in the `users` table to a bcrypt hash (cost 12) of your chosen password.

## Project layout

- `src/` – Rust backend (axum, askama, sqlx)
- `templates/` – Askama HTML with **Tailwind CSS** (base, login, dashboard, add_site, add_database, site_detail)
- `static/` – `style.css` (spinner, toast animation), `app.js` (toasts, form loading, search, delete confirm)
- `migrations/` – PostgreSQL schema (users, sessions, sites, site_databases)

The UI uses Tailwind via CDN (no build step). For production you may replace with a built Tailwind stylesheet.

## Security

- Panel binds to **127.0.0.1** by default; put Caddy (or another reverse proxy) in front for TLS and public access.
- Sessions in PostgreSQL; cookie `panel_session`, HttpOnly.
- Inputs validated (domain format, path uniqueness, DB identifiers, password length).
- User content escaped in templates (XSS). Safe, predefined commands only for site/DB operations (to be wired to your FrankenPHP/Caddy/MariaDB tooling).

## TODO (backend integration)

- Create site folder and Caddy block; reload FrankenPHP on add/delete site.
- Optional WordPress install on “Add Site”.
- Create MariaDB/MySQL DB and user when “Add Database” is used; store only metadata in PostgreSQL.
- Optional: SSL status from Caddy; real site status (e.g. health check).

## License

MIT.
