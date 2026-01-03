-- AI Summary Caching
-- Stores generated summaries to avoid repeated API calls

CREATE TABLE IF NOT EXISTS file_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files_metadata(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    content_hash VARCHAR(64) NOT NULL,  -- SHA256 hash of file content to detect changes
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(file_id)  -- One summary per file
);

CREATE INDEX IF NOT EXISTS idx_file_summaries_file ON file_summaries(file_id);
CREATE INDEX IF NOT EXISTS idx_file_summaries_tenant ON file_summaries(tenant_id);

