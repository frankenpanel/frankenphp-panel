# FrankenPHP Panel

Open-source panel to run **PHP and WordPress sites** on a server. One installer on a **fresh server** installs everything; you log in and add sites from the UI. The panel manages the rest: site folders, Caddy/FrankenPHP config, databases, and WordPress setup.

## What you get

- **Fresh server → one command** – The installer adds all required packages (PostgreSQL, MariaDB, FrankenPHP, Rust if needed), builds and installs the panel, and starts services. No manual DB or Caddy setup.
- **Log in and add sites** – In the panel you enter a domain and optionally check “Install WordPress”. PHP sites go live immediately; WordPress sites get a database and WP files, then you complete the 5-minute setup in the browser.
- **Panel manages everything** – Site roots (`/var/www/<domain>`), Caddy config, FrankenPHP reload, and (for WordPress) MariaDB database + user, WordPress download, and `wp-config.php`.

## Features

- **Login** – Username/password; credentials printed and saved at install
- **Dashboard** – List sites, Add Site, Add Database, site details
- **Add Site** – Domain only. **PHP:** panel creates folder + Caddy; site is live. **WordPress:** panel creates folder, Caddy, MariaDB DB, WP files, and wp-config; open the site to finish the wizard
- **Website details** – Domain, path, DB list, Restart / Delete

## Install on a fresh server

Supported: **Ubuntu, Debian** (apt); **RHEL, Fedora** (dnf/yum). No need to install PostgreSQL, MariaDB, or FrankenPHP yourself—the installer does it.

```bash
# Clone or upload the project, then run as root:
sudo ./install.sh
```

The installer will:

1. **Install system packages** – curl, build-essential, libssl, libpq, **PostgreSQL**, **MariaDB**, and (if needed) **Rust**
2. **Build and install the panel** – binary, static files, migrations under `/opt/frankenphp-panel`
3. **Create panel database** – PostgreSQL user and database `panel`, random passwords, `.env`, migrations, admin password
4. **Install FrankenPHP** – Caddy+PHP binary, `/etc/caddy/Caddyfile`, systemd service on ports 80/443
5. **Configure sudo** – panel user can run the site-create script (creates dirs, Caddy snippets, WordPress)
6. **Start services** – panel (port 2090) and FrankenPHP (80/443)
7. **Print credentials** – panel URL, username `admin`, and generated password (also saved to `/opt/frankenphp-panel/.panel-credentials`)

After install: **open the printed URL in your browser, log in with `admin` and the printed password, then add sites.** Save the password and remove the credentials file: `sudo rm /opt/frankenphp-panel/.panel-credentials`.

### Installer options (advanced)

- `--prefix DIR` – Install directory (default: `/opt/frankenphp-panel`)
- `--user USER` – System user for the panel (default: `panel`)
- `--skip-caddy` – Do not install FrankenPHP (only if you already run Caddy/FrankenPHP yourself)
- `--no-systemd` – Do not install systemd units
- `--no-build` – Use existing binary in `target/release/`
- `--skip-deps` – Do not install system packages or Rust (only if they are already installed)

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

## Add Site – managed by the panel

- **PHP site:** Panel creates `/var/www/<domain>`, Caddy snippet, and reloads FrankenPHP. Site is live; add your PHP files via SFTP or deploy.
- **WordPress site:** Panel does the same, then creates a MariaDB database and user, downloads WordPress, and writes `wp-config.php`. You open the site in the browser and complete the 5-minute setup (title, admin user, password). No manual DB or wp-config steps.

## WordPress: database not created

If you add a WordPress site but the database is not created:

1. **MariaDB must be running** – The script checks this and will fail with a clear error if not. Start it: `sudo systemctl start mariadb` (or `mysql` on some systems).
2. **Root password** – By default the script connects as `mysql -u root` (no password). If you have set a root password (e.g. via `mysql_secure_installation`), set `MYSQL_ROOT_PASSWORD` in the environment that runs the panel (e.g. in the panel’s systemd unit) so the site-create script can create the WordPress database.

If the script fails, the panel will show the MySQL/MariaDB error message so you can fix the cause.

**“Error establishing a database connection”** on an existing WordPress site: the panel now generates `wp-config.php` with `DB_HOST` set to `127.0.0.1` (instead of `localhost`) so PHP connects over TCP and avoids socket/IPv6 issues. If you already created the site before this change, edit the site’s `wp-config.php` and set `define('DB_HOST', '127.0.0.1');`. If it still fails, ensure the MariaDB user can connect from 127.0.0.1 (e.g. run as root: `mysql -e "CREATE USER 'wp_YourDbUser'@'127.0.0.1' IDENTIFIED BY 'password'; GRANT ALL ON wp_YourDbName.* TO 'wp_YourDbUser'@'127.0.0.1'; FLUSH PRIVILEGES;"` using the DB_NAME/DB_USER/DB_PASSWORD from `wp-config.php`).

## WordPress debug and fixing issues

WordPress sites installed by the panel have **debug logging enabled by default** so you can fix errors without showing them to visitors:

- **Log file:** Errors and notices are written to `wp-content/debug.log` (e.g. `/var/www/example.com/wp-content/debug.log`).
- **Display off:** `WP_DEBUG_DISPLAY` is `false`, so visitors do not see PHP errors on the page.

**To fix issues smoothly:**

1. **Check the log** – On the server: `tail -f /var/www/<your-domain>/wp-content/debug.log` (or open the file). Reproduce the problem and read the messages.
2. **Show errors in the browser (temporarily)** – Edit the site’s `wp-config.php` and set `define('WP_DEBUG_DISPLAY', true);`. Reload the page, fix the issue, then set it back to `false` and clear or rotate the log.
3. **Turn off debug in production** – In `wp-config.php` set `define('WP_DEBUG', false);` (and optionally remove or comment out `WP_DEBUG_LOG` and `WP_DEBUG_DISPLAY`).

## TODO (backend integration)

- SSL status from Caddy; real site status (e.g. health check).
- “Add Database” in panel to create MariaDB DB/user for existing sites.

## License

MIT.
