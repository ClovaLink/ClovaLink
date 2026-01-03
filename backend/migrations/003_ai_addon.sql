-- AI Add-on Feature Migration
-- Adds tables for tenant AI settings, usage tracking, and file embeddings

-- Note: pgvector extension is optional - semantic search requires it
-- To enable: Install pgvector in PostgreSQL, then run: CREATE EXTENSION IF NOT EXISTS vector;
-- Without it, the file_embeddings table will use a placeholder column type

-- Tenant AI settings - controls per-tenant AI configuration
CREATE TABLE IF NOT EXISTS tenant_ai_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT false,
    provider VARCHAR(50) NOT NULL DEFAULT 'openai',
    api_key_encrypted TEXT,  -- encrypted at rest
    allowed_roles TEXT[] NOT NULL DEFAULT ARRAY['Admin', 'SuperAdmin'],
    hipaa_approved_only BOOLEAN NOT NULL DEFAULT false,
    sox_read_only BOOLEAN NOT NULL DEFAULT false,
    monthly_token_limit INTEGER NOT NULL DEFAULT 100000,
    daily_request_limit INTEGER NOT NULL DEFAULT 100,
    tokens_used_this_month INTEGER NOT NULL DEFAULT 0,
    requests_today INTEGER NOT NULL DEFAULT 0,
    last_usage_reset DATE DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Usage tracking (audit log without content for compliance)
CREATE TABLE IF NOT EXISTS ai_usage_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    file_id UUID,  -- No FK to allow logging even if file deleted
    action VARCHAR(50) NOT NULL,  -- summarize, answer, embed, search
    provider VARCHAR(50) NOT NULL,
    model VARCHAR(100),
    tokens_used INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL,  -- success, error, rate_limited, forbidden
    error_message TEXT,  -- Only technical error, never content
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_usage_tenant ON ai_usage_logs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_usage_user ON ai_usage_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_ai_usage_created ON ai_usage_logs(created_at);

-- File embeddings for semantic search
-- Note: Uses BYTEA for embeddings storage (pgvector not required)
-- For vector similarity search, install pgvector and ALTER COLUMN to vector type
CREATE TABLE IF NOT EXISTS file_embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    file_id UUID NOT NULL REFERENCES files_metadata(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL DEFAULT 0,
    chunk_text_hash VARCHAR(64) NOT NULL,  -- SHA256 hash to detect changes
    embedding BYTEA,  -- Stored as binary, convert to vector for similarity search
    embedding_dim INTEGER DEFAULT 1536,  -- Dimension hint for reconstruction
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(file_id, chunk_index)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_file ON file_embeddings(file_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_tenant ON file_embeddings(tenant_id);

-- Function to reset daily request counts
CREATE OR REPLACE FUNCTION reset_daily_ai_requests() RETURNS void AS $$
BEGIN
    UPDATE tenant_ai_settings 
    SET requests_today = 0, 
        last_usage_reset = CURRENT_DATE 
    WHERE last_usage_reset < CURRENT_DATE;
END;
$$ LANGUAGE plpgsql;

-- Function to reset monthly token counts (call on 1st of month)
CREATE OR REPLACE FUNCTION reset_monthly_ai_tokens() RETURNS void AS $$
BEGIN
    UPDATE tenant_ai_settings 
    SET tokens_used_this_month = 0;
END;
$$ LANGUAGE plpgsql;

