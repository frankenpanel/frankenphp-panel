-- Add privileges for site databases (full = ALL PRIVILEGES, readonly = SELECT only)
ALTER TABLE site_databases ADD COLUMN IF NOT EXISTS privileges VARCHAR(32) NOT NULL DEFAULT 'full';
