-- 005_backup_settings.sql
-- Per-tenant backup toggle, scheduled backup config, and backup history

-- Per-tenant backup toggle
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS backup_enabled BOOLEAN DEFAULT true;

-- Scheduled backup config (per-tenant, SuperAdmin only)
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS auto_backup_enabled BOOLEAN DEFAULT false;
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS auto_backup_cron VARCHAR(100) DEFAULT '0 2 * * 0';
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS auto_backup_retention_count INTEGER DEFAULT 5;

-- Backup history (manual + auto)
CREATE TABLE IF NOT EXISTS backup_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    filename VARCHAR(512) NOT NULL,
    storage_path VARCHAR(1024) NOT NULL,
    size_bytes BIGINT NOT NULL,
    sections JSONB NOT NULL DEFAULT '[]',
    is_auto_backup BOOLEAN NOT NULL DEFAULT false,
    status VARCHAR(20) NOT NULL DEFAULT 'completed',
    error_message TEXT,
    duration_ms INTEGER,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backup_history_tenant ON backup_history(tenant_id);
CREATE INDEX IF NOT EXISTS idx_backup_history_created ON backup_history(created_at DESC);
