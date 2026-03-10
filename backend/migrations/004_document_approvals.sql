-- Document Approval Workflow
-- Per-tenant opt-in feature for requiring file uploads to be approved before they become accessible.

-- 1. Tenant toggle
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS approval_workflow_enabled BOOLEAN DEFAULT false;

-- 2. File approval status (default 'approved' grandfathers existing files)
ALTER TABLE files_metadata ADD COLUMN IF NOT EXISTS approval_status VARCHAR(20) NOT NULL DEFAULT 'approved';
CREATE INDEX IF NOT EXISTS idx_files_approval_pending ON files_metadata(tenant_id, approval_status) WHERE approval_status = 'pending';

-- 3. Approval policies — define when approval is required
CREATE TABLE IF NOT EXISTS approval_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    scope VARCHAR(50) NOT NULL DEFAULT 'all',
    -- 'all' = every upload
    -- 'department' = uploads to a specific department (scope_value = department UUID)
    -- 'company_folder' = uploads to company folders only
    scope_value TEXT,
    required_approvals INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);
CREATE INDEX IF NOT EXISTS idx_approval_policies_tenant ON approval_policies(tenant_id);

-- 4. Approval requests — track each file's approval lifecycle
CREATE TABLE IF NOT EXISTS approval_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    file_id UUID NOT NULL REFERENCES files_metadata(id) ON DELETE CASCADE,
    policy_id UUID REFERENCES approval_policies(id) ON DELETE SET NULL,
    requested_by UUID NOT NULL REFERENCES users(id),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    step INTEGER NOT NULL DEFAULT 1,
    decided_by UUID REFERENCES users(id),
    decided_at TIMESTAMPTZ,
    rejection_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_approval_req_tenant_status ON approval_requests(tenant_id, status) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_approval_req_file ON approval_requests(file_id);
CREATE INDEX IF NOT EXISTS idx_approval_req_requested_by ON approval_requests(requested_by);

-- 5. Approval email templates (ON CONFLICT ensures safe for existing installs)
INSERT INTO email_templates (template_key, name, subject, body_html, body_text, variables) VALUES
(
    'approval_required',
    'File Approval Required',
    'Action Required: "{{file_name}}" needs your approval',
    '<p>Hi {{user_name}}, a file "{{file_name}}" uploaded by {{uploader_name}} requires your approval. <a href="{{app_url}}/approvals">Review now</a>.</p>',
    'Hi {{user_name}}, a file "{{file_name}}" uploaded by {{uploader_name}} requires your approval. Review at: {{app_url}}/approvals',
    '["user_name", "file_name", "uploader_name", "company_name", "app_url"]'::jsonb
),
(
    'approval_decision',
    'File Approval Decision',
    'Your file "{{file_name}}" has been {{decision}}',
    '<p>Your file "{{file_name}}" has been {{decision}}. {{reason}}</p>',
    'Your file "{{file_name}}" has been {{decision}}. {{reason}}',
    '["file_name", "decision", "reason", "company_name"]'::jsonb
)
ON CONFLICT (template_key) DO NOTHING;
