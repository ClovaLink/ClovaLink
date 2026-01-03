-- ============================================================================
-- FILE GROUPS
-- Allows users to manually group related files together into named collections
-- ============================================================================

CREATE TABLE IF NOT EXISTS file_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    department_id UUID REFERENCES departments(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    color VARCHAR(7), -- hex color like #FF5733
    icon VARCHAR(50) DEFAULT 'folder-kanban', -- icon name
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Unique name per tenant+department combination
    UNIQUE(tenant_id, department_id, name)
);

-- Add group_id to files_metadata for file-to-group association
ALTER TABLE files_metadata ADD COLUMN IF NOT EXISTS group_id UUID REFERENCES file_groups(id) ON DELETE SET NULL;

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_files_metadata_group_id ON files_metadata(group_id) WHERE group_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_file_groups_tenant_dept ON file_groups(tenant_id, department_id);
CREATE INDEX IF NOT EXISTS idx_file_groups_created_by ON file_groups(created_by);

-- Comments
COMMENT ON TABLE file_groups IS 'User-created collections to manually group related files together';
COMMENT ON COLUMN file_groups.department_id IS 'If NULL, group is tenant-wide; otherwise visible to department members';
COMMENT ON COLUMN file_groups.color IS 'Optional hex color for visual distinction (e.g., #FF5733)';
COMMENT ON COLUMN file_groups.icon IS 'Icon name from lucide-react icon set';
COMMENT ON COLUMN files_metadata.group_id IS 'Optional reference to a file group this file belongs to';

