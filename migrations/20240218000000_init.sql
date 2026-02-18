-- Panel users (admin login)
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sessions for auth
CREATE TABLE IF NOT EXISTS sessions (
    token VARCHAR(64) PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

-- Sites (websites managed by panel)
CREATE TABLE IF NOT EXISTS sites (
    id SERIAL PRIMARY KEY,
    domain VARCHAR(255) UNIQUE NOT NULL,
    folder_path VARCHAR(1024) UNIQUE NOT NULL,
    wordpress_installed BOOLEAN NOT NULL DEFAULT FALSE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sites_user ON sites(user_id);

-- Site databases (MariaDB/MySQL metadata stored in PostgreSQL)
CREATE TABLE IF NOT EXISTS site_databases (
    id SERIAL PRIMARY KEY,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    db_name VARCHAR(64) NOT NULL,
    db_user VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(site_id, db_name)
);

CREATE INDEX IF NOT EXISTS idx_site_databases_site ON site_databases(site_id);

-- Default admin user (password: admin) - change in production
INSERT INTO users (username, password_hash) VALUES (
    'admin',
    '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4.G2ic1pRlHy.O7e'
) ON CONFLICT (username) DO NOTHING;
