-- 006_global_backup_history.sql
-- Allow global backups in backup_history (tenant_id = NULL for global)
-- Add global auto-backup settings

-- Make tenant_id nullable for global backups
ALTER TABLE backup_history ALTER COLUMN tenant_id DROP NOT NULL;

-- Partial index for efficient global backup queries
CREATE INDEX IF NOT EXISTS idx_backup_history_global ON backup_history(created_at DESC) WHERE tenant_id IS NULL;

-- Global auto-backup settings (stored in global_settings key-value store)
INSERT INTO global_settings (key, value, updated_at)
VALUES
    ('global_auto_backup_enabled', 'false', NOW()),
    ('global_auto_backup_cron', '"0 3 * * 0"', NOW()),
    ('global_auto_backup_retention_count', '5', NOW())
ON CONFLICT (key) DO NOTHING;
