-- Add parent_path to file_groups so groups can be nested inside folders
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS parent_path VARCHAR(1024);

-- Index for efficient filtering by parent_path
CREATE INDEX IF NOT EXISTS idx_file_groups_parent_path ON file_groups(tenant_id, parent_path);

