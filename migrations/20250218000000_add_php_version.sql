-- Add PHP version per site (e.g. 8.1, 8.2, 8.3)
ALTER TABLE sites ADD COLUMN IF NOT EXISTS php_version VARCHAR(10) NOT NULL DEFAULT '8.2';
