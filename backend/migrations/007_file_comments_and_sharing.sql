-- File Comments & User-Specific Sharing Migration

-- ===========================================
-- Part 1: File Comments
-- ===========================================

-- Comments on files
CREATE TABLE IF NOT EXISTS file_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files_metadata(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    parent_id UUID REFERENCES file_comments(id) ON DELETE CASCADE,  -- For threaded replies
    is_edited BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_file_comments_file ON file_comments(file_id);
CREATE INDEX IF NOT EXISTS idx_file_comments_tenant ON file_comments(tenant_id);
CREATE INDEX IF NOT EXISTS idx_file_comments_user ON file_comments(user_id);
CREATE INDEX IF NOT EXISTS idx_file_comments_parent ON file_comments(parent_id);
CREATE INDEX IF NOT EXISTS idx_file_comments_created ON file_comments(created_at DESC);

-- ===========================================
-- Part 2: User-Specific File Sharing
-- ===========================================

-- Add column for user-specific sharing (nullable - if null, share is org-wide/public)
ALTER TABLE file_shares ADD COLUMN IF NOT EXISTS shared_with_user_id UUID REFERENCES users(id) ON DELETE CASCADE;

-- Index for finding shares for a specific user
CREATE INDEX IF NOT EXISTS idx_file_shares_shared_with ON file_shares(shared_with_user_id);

-- Composite index for finding all shares a user can access
CREATE INDEX IF NOT EXISTS idx_file_shares_tenant_user ON file_shares(tenant_id, shared_with_user_id);

