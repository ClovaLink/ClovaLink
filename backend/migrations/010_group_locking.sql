-- ============================================================================
-- GROUP LOCKING
-- Add locking capabilities to file groups with password and role requirements
-- ============================================================================

-- Add locking columns to file_groups table
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS is_locked BOOLEAN DEFAULT FALSE;
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS locked_by UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS locked_at TIMESTAMPTZ;
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS lock_password_hash VARCHAR(255);
ALTER TABLE file_groups ADD COLUMN IF NOT EXISTS lock_requires_role VARCHAR(50);

-- Index for querying locked groups
CREATE INDEX IF NOT EXISTS idx_file_groups_locked ON file_groups(is_locked) WHERE is_locked = true;

-- Comments
COMMENT ON COLUMN file_groups.is_locked IS 'Whether the group is locked (prevents access to files within)';
COMMENT ON COLUMN file_groups.locked_by IS 'User who locked the group';
COMMENT ON COLUMN file_groups.locked_at IS 'When the group was locked';
COMMENT ON COLUMN file_groups.lock_password_hash IS 'Optional argon2 password hash for unlocking';
COMMENT ON COLUMN file_groups.lock_requires_role IS 'Minimum role required to access locked group (e.g., Admin, Manager)';

