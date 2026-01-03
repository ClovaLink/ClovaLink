-- Discord OAuth Integration
-- Allows users to connect their Discord accounts to receive DM notifications

-- Tenant Discord settings (enable/disable feature per tenant)
CREATE TABLE IF NOT EXISTS tenant_discord_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- User Discord connections (OAuth tokens - encrypted)
CREATE TABLE IF NOT EXISTS user_discord_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    discord_user_id VARCHAR(50) NOT NULL,
    discord_username VARCHAR(100),
    discord_discriminator VARCHAR(10),
    discord_avatar VARCHAR(255),
    access_token_encrypted TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    token_expires_at TIMESTAMPTZ,
    -- Notification preferences
    dm_notifications_enabled BOOLEAN NOT NULL DEFAULT true,
    notify_file_shared BOOLEAN NOT NULL DEFAULT true,
    notify_file_uploaded BOOLEAN NOT NULL DEFAULT true,
    notify_comments BOOLEAN NOT NULL DEFAULT true,
    notify_file_requests BOOLEAN NOT NULL DEFAULT true,
    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id),
    UNIQUE(discord_user_id, tenant_id)
);

-- Index for looking up by discord user
CREATE INDEX IF NOT EXISTS idx_discord_connections_discord_user ON user_discord_connections(discord_user_id);
CREATE INDEX IF NOT EXISTS idx_discord_connections_tenant ON user_discord_connections(tenant_id);

-- Notification log (for debugging and rate limiting)
CREATE TABLE IF NOT EXISTS discord_notification_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,  -- file_shared, file_uploaded, comment, file_request
    status VARCHAR(20) NOT NULL,  -- sent, failed, rate_limited
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_discord_logs_user ON discord_notification_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_discord_logs_created ON discord_notification_logs(created_at);

-- OAuth state tokens (for CSRF protection during OAuth flow)
CREATE TABLE IF NOT EXISTS discord_oauth_states (
    state VARCHAR(64) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
);

-- Clean up expired states periodically
CREATE INDEX IF NOT EXISTS idx_discord_oauth_states_expires ON discord_oauth_states(expires_at);

