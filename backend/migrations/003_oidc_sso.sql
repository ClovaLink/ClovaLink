-- SSO Support (OIDC + SAML 2.0)
-- Adds per-tenant SSO provider configuration, OAuth/SAML state tracking,
-- user identity linking, and IdP attribute-to-role mapping.

-- ==================== Tenant OIDC Provider Configuration ====================

CREATE TABLE IF NOT EXISTS tenant_oidc_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- Provider identity
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(50) NOT NULL,
    provider_type VARCHAR(50) NOT NULL DEFAULT 'generic',
    -- OIDC Configuration
    issuer_url TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    scopes TEXT NOT NULL DEFAULT 'openid email profile',
    -- Optional endpoint overrides (if discovery is unavailable)
    authorization_endpoint TEXT,
    token_endpoint TEXT,
    userinfo_endpoint TEXT,
    jwks_uri TEXT,
    -- Behavior
    auto_provision BOOLEAN NOT NULL DEFAULT false,
    default_role VARCHAR(50) NOT NULL DEFAULT 'Employee',
    default_department_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    -- Domain mapping for login page discovery
    email_domains TEXT[] NOT NULL DEFAULT '{}',
    -- MFA policy
    trust_idp_mfa BOOLEAN NOT NULL DEFAULT true,
    -- State: providers must be explicitly enabled after configuration
    enabled BOOLEAN NOT NULL DEFAULT false,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Constraints
    UNIQUE(tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_oidc_providers_tenant ON tenant_oidc_providers(tenant_id);
CREATE INDEX IF NOT EXISTS idx_oidc_providers_email_domains ON tenant_oidc_providers USING GIN(email_domains);

-- ==================== OIDC OAuth State (CSRF Protection) ====================

CREATE TABLE IF NOT EXISTS oidc_oauth_states (
    state VARCHAR(64) PRIMARY KEY,
    nonce VARCHAR(64) NOT NULL,
    provider_id UUID NOT NULL REFERENCES tenant_oidc_providers(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- NULL for login flow, set for account linking
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
);

CREATE INDEX IF NOT EXISTS idx_oidc_oauth_states_expires ON oidc_oauth_states(expires_at);

-- ==================== User OIDC Identities ====================

CREATE TABLE IF NOT EXISTS user_oidc_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES tenant_oidc_providers(id) ON DELETE CASCADE,
    -- OIDC identity
    oidc_subject TEXT NOT NULL,
    oidc_issuer TEXT NOT NULL,
    oidc_email TEXT,
    oidc_name TEXT,
    -- Tracking
    last_login_at TIMESTAMPTZ,
    login_count INTEGER NOT NULL DEFAULT 0,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- One identity per subject per provider
    UNIQUE(provider_id, oidc_subject)
);

CREATE INDEX IF NOT EXISTS idx_oidc_identities_user ON user_oidc_identities(user_id);
CREATE INDEX IF NOT EXISTS idx_oidc_identities_subject ON user_oidc_identities(oidc_subject);

-- ==================== Alter existing tables ====================

-- Allow OIDC-only users (no password)
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ALTER COLUMN password_hash SET DEFAULT NULL;

-- Track primary auth method: 'local' (password), 'oidc' (SSO-only), 'hybrid' (both)
ALTER TABLE users ADD COLUMN IF NOT EXISTS identity_provider VARCHAR(20) NOT NULL DEFAULT 'local';

-- Auth methods enabled per tenant: array of 'local', 'oidc'
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS auth_methods TEXT[] NOT NULL DEFAULT ARRAY['local']::TEXT[];

-- ==================== Tenant SAML Provider Configuration ====================

CREATE TABLE IF NOT EXISTS tenant_saml_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- Provider identity
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(50) NOT NULL,
    provider_type VARCHAR(50) NOT NULL DEFAULT 'generic',
    -- IdP Configuration
    idp_entity_id TEXT NOT NULL,
    idp_sso_url TEXT NOT NULL,
    idp_slo_url TEXT,
    idp_metadata_url TEXT,
    idp_metadata_xml TEXT,
    idp_signing_certificate TEXT NOT NULL,
    -- SP Configuration
    sp_entity_id TEXT NOT NULL,
    nameid_format VARCHAR(100) NOT NULL DEFAULT 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress',
    request_signing BOOLEAN NOT NULL DEFAULT false,
    want_assertions_signed BOOLEAN NOT NULL DEFAULT true,
    want_response_signed BOOLEAN NOT NULL DEFAULT true,
    sp_signing_key_encrypted TEXT,
    sp_signing_cert TEXT,
    sso_binding VARCHAR(50) NOT NULL DEFAULT 'HTTP-POST',
    -- Attribute name hints (for extracting identity from assertions)
    attribute_email VARCHAR(100) NOT NULL DEFAULT 'email',
    attribute_name VARCHAR(100) NOT NULL DEFAULT 'displayName',
    -- Behavior
    auto_provision BOOLEAN NOT NULL DEFAULT false,
    default_role VARCHAR(50) NOT NULL DEFAULT 'Employee',
    default_custom_role_id UUID REFERENCES roles(id) ON DELETE SET NULL,
    default_department_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    -- Domain mapping for login page discovery
    email_domains TEXT[] NOT NULL DEFAULT '{}',
    -- MFA policy
    trust_idp_mfa BOOLEAN NOT NULL DEFAULT true,
    -- State: providers must be explicitly enabled after configuration
    enabled BOOLEAN NOT NULL DEFAULT false,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Constraints
    UNIQUE(tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_saml_providers_tenant ON tenant_saml_providers(tenant_id);
CREATE INDEX IF NOT EXISTS idx_saml_providers_email_domains ON tenant_saml_providers USING GIN(email_domains);

-- ==================== SAML Auth State (CSRF / Replay Protection) ====================

CREATE TABLE IF NOT EXISTS saml_auth_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    relay_state VARCHAR(64) NOT NULL UNIQUE,
    authn_request_id VARCHAR(128) NOT NULL,
    provider_id UUID NOT NULL REFERENCES tenant_saml_providers(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- NULL for login flow, set for account linking
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
);

CREATE INDEX IF NOT EXISTS idx_saml_auth_states_expires ON saml_auth_states(expires_at);
CREATE INDEX IF NOT EXISTS idx_saml_auth_states_relay ON saml_auth_states(relay_state);

-- ==================== User SAML Identities ====================

CREATE TABLE IF NOT EXISTS user_saml_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES tenant_saml_providers(id) ON DELETE CASCADE,
    -- SAML identity
    saml_name_id TEXT NOT NULL,
    saml_name_id_format TEXT,
    saml_session_index TEXT,
    -- Cached attributes from last assertion
    saml_email TEXT,
    saml_name TEXT,
    -- Tracking
    last_login_at TIMESTAMPTZ,
    login_count INTEGER NOT NULL DEFAULT 0,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- One identity per NameID per provider
    UNIQUE(provider_id, saml_name_id)
);

CREATE INDEX IF NOT EXISTS idx_saml_identities_user ON user_saml_identities(user_id);
CREATE INDEX IF NOT EXISTS idx_saml_identities_nameid ON user_saml_identities(saml_name_id);

-- ==================== SSO Attribute-to-Role Mappings (Protocol-Agnostic) ====================
-- Works for both OIDC claims and SAML AttributeStatements

CREATE TABLE IF NOT EXISTS sso_attribute_mappings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    -- Provider reference (polymorphic — enforced at app level)
    protocol VARCHAR(10) NOT NULL CHECK (protocol IN ('oidc', 'saml')),
    provider_id UUID NOT NULL,
    -- Matching criteria
    attribute_name VARCHAR(200) NOT NULL,
    attribute_value VARCHAR(500) NOT NULL,
    match_type VARCHAR(20) NOT NULL DEFAULT 'exact' CHECK (match_type IN ('exact', 'contains', 'regex')),
    -- ClovaLink target role/department
    target_role VARCHAR(50) NOT NULL DEFAULT 'Employee',
    target_custom_role_id UUID REFERENCES roles(id) ON DELETE SET NULL,
    target_department_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    -- Priority (higher = evaluated first, first match wins)
    priority INTEGER NOT NULL DEFAULT 0,
    -- State
    enabled BOOLEAN NOT NULL DEFAULT true,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- One mapping per attribute name+value per provider
    UNIQUE(provider_id, attribute_name, attribute_value)
);

CREATE INDEX IF NOT EXISTS idx_sso_attr_mappings_provider ON sso_attribute_mappings(protocol, provider_id);
CREATE INDEX IF NOT EXISTS idx_sso_attr_mappings_tenant ON sso_attribute_mappings(tenant_id);
CREATE INDEX IF NOT EXISTS idx_sso_attr_mappings_query ON sso_attribute_mappings(protocol, provider_id, enabled, priority DESC);

-- ==================== SAML Assertion Replay Protection ====================

CREATE TABLE IF NOT EXISTS saml_consumed_assertions (
    assertion_id VARCHAR(256) PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES tenant_saml_providers(id) ON DELETE CASCADE,
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    not_on_or_after TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_saml_consumed_assertions_expiry ON saml_consumed_assertions(not_on_or_after);

-- ==================== Cleanup cron for expired states ====================

-- The backend cron system handles periodic cleanup:
-- DELETE FROM oidc_oauth_states WHERE expires_at < NOW()
-- DELETE FROM saml_auth_states WHERE expires_at < NOW()
-- DELETE FROM saml_consumed_assertions WHERE not_on_or_after < NOW() - INTERVAL '1 hour'
