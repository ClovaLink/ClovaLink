-- AI Maintenance Mode and Audit Improvements
-- Adds maintenance mode toggle and improves usage tracking

-- Add maintenance mode columns to tenant_ai_settings
ALTER TABLE tenant_ai_settings 
ADD COLUMN IF NOT EXISTS maintenance_mode BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE tenant_ai_settings 
ADD COLUMN IF NOT EXISTS maintenance_message TEXT DEFAULT 'AI features are temporarily unavailable for maintenance. Please try again later.';

-- Add file_name to ai_usage_logs for better audit display
ALTER TABLE ai_usage_logs 
ADD COLUMN IF NOT EXISTS file_name VARCHAR(255);

-- Add index for pagination queries
CREATE INDEX IF NOT EXISTS idx_ai_usage_tenant_created ON ai_usage_logs(tenant_id, created_at DESC);

