-- ============================================================================
-- ClovaLink Demo Data v1.0
-- Run this migration for development/demo environments
-- Skip for production (create your own admin user instead)
-- ============================================================================

-- ============================================================================
-- GLOBAL SETTINGS
-- ============================================================================

INSERT INTO global_settings (key, value) VALUES
    ('date_format', '"MM/DD/YYYY"'),
    ('time_format', '"12h"'),
    ('timezone', '"America/New_York"'),
    ('footer_attribution', '"An open source project by ClovaLink.org"'),
    ('footer_disclaimer', '"ClovaLink is provided \"as is\" without warranty of any kind. The authors and contributors are not liable for any damages arising from use of this software."'),
    ('app_name', '"ClovaLink"'),
    ('tos_content', '""'),
    ('privacy_content', '""'),
    ('help_content', '""'),
    ('maintenance_mode', '"false"'),
    ('maintenance_message', '"The system is currently undergoing maintenance. We will be back shortly!"')
ON CONFLICT (key) DO NOTHING;

-- ============================================================================
-- SYSTEM ROLES
-- ============================================================================

INSERT INTO roles (id, tenant_id, name, description, base_role, is_system) VALUES
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', NULL, 'SuperAdmin', 'Full administrative control over all companies and settings', 'SuperAdmin', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'Admin', 'Company administrator with user and settings management', 'Admin', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', NULL, 'Manager', 'Team manager with file request and sharing capabilities', 'Manager', true),
    ('04d5e6f7-a7b8-4c9d-0e1f-2a3b4c5d6e7f', NULL, 'Employee', 'Standard user with basic file access', 'Employee', true)
ON CONFLICT DO NOTHING;

-- Employee permissions
INSERT INTO role_permissions (role_id, permission, granted) VALUES
    ('04d5e6f7-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'files.view', true),
    ('04d5e6f7-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'files.upload', true),
    ('04d5e6f7-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'files.download', true)
ON CONFLICT DO NOTHING;

-- Manager permissions
INSERT INTO role_permissions (role_id, permission, granted) VALUES
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'files.view', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'files.upload', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'files.download', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'files.delete', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'files.share', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'requests.create', true),
    ('03c4d5e6-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'requests.view', true)
ON CONFLICT DO NOTHING;

-- Admin permissions
INSERT INTO role_permissions (role_id, permission, granted) VALUES
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'files.view', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'files.upload', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'files.download', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'files.delete', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'files.share', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'requests.create', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'requests.view', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'users.view', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'users.invite', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'users.edit', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'roles.view', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'audit.view', true),
    ('02b3c4d5-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'settings.view', true)
ON CONFLICT DO NOTHING;

-- SuperAdmin permissions
INSERT INTO role_permissions (role_id, permission, granted) VALUES
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'files.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'files.upload', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'files.download', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'files.delete', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'files.share', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'requests.create', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'requests.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'users.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'users.invite', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'users.edit', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'users.delete', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'roles.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'roles.manage', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'audit.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'audit.export', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'settings.view', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'settings.edit', true),
    ('01a2b3c4-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'tenants.manage', true)
ON CONFLICT DO NOTHING;

-- ============================================================================
-- DEMO TENANTS
-- ============================================================================

INSERT INTO tenants (id, name, domain, plan, status, compliance_mode) VALUES
    ('a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'ClovaLink', 'clovalink.com', 'Enterprise', 'active', 'HIPAA'),
    ('b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'Globex Inc', 'globex.com', 'Business', 'active', 'SOX'),
    ('c3d4e5f6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'Soylent Corp', 'soylent.com', 'Starter', 'suspended', 'Standard')
ON CONFLICT DO NOTHING;

-- Apply compliance defaults
UPDATE tenants SET mfa_required = true, public_sharing_enabled = false, session_timeout_minutes = 15
WHERE id = 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d';

UPDATE tenants SET mfa_required = true, public_sharing_enabled = false
WHERE id = 'b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e';

-- ============================================================================
-- DEMO DEPARTMENTS
-- ============================================================================

-- ClovaLink departments
INSERT INTO departments (id, tenant_id, name, description) VALUES
    ('d1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Finance', 'Financial operations and accounting'),
    ('d2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Legal', 'Legal affairs and compliance'),
    ('d3c4d5e6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Human Resources', 'Employee management and HR operations'),
    ('d4d5e6f7-b8c9-4d0e-1f2a-3b4c5d6e7f8a', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Engineering', 'Software development and technical operations')
ON CONFLICT (tenant_id, name) DO NOTHING;

-- Globex Inc departments
INSERT INTO departments (id, tenant_id, name, description) VALUES
    ('d5e6f7a8-c9d0-4e1f-2a3b-4c5d6e7f8a9b', 'b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'Operations', 'Business operations'),
    ('d6f7a8b9-d0e1-4f2a-3b4c-5d6e7f8a9b0c', 'b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'Sales', 'Sales and marketing')
ON CONFLICT (tenant_id, name) DO NOTHING;

-- ============================================================================
-- DEMO USERS
-- Password for all users: "password123"
-- Hash: $argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU
-- ============================================================================

-- SuperAdmin (password: password123)
INSERT INTO users (id, tenant_id, email, name, password_hash, role, status) VALUES
    ('00a1b2c3-d4e5-4f6a-7b8c-9d0e1f2a3b4c', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'superadmin@clovalink.com', 'Super Admin', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'SuperAdmin', 'active')
ON CONFLICT DO NOTHING;

-- ClovaLink users
INSERT INTO users (id, tenant_id, email, name, password_hash, role, status) VALUES
    ('0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'admin@clovalink.com', 'Admin User', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'Admin', 'active'),
    ('0b2c3d4e-5f6a-4b8c-9d0e-1f2a3b4c5d6e', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'manager@clovalink.com', 'Manager User', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'Manager', 'active'),
    ('0c3d4e5f-6a7b-4c9d-0e1f-2a3b4c5d6e7f', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'employee@clovalink.com', 'Employee User', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'Employee', 'active')
ON CONFLICT DO NOTHING;

-- Globex Inc users
INSERT INTO users (id, tenant_id, email, name, password_hash, role, status) VALUES
    ('0d4e5f6a-7b8c-4d9e-0f1a-2b3c4d5e6f7a', 'b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'admin@globex.com', 'Globex Admin', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'Admin', 'active')
ON CONFLICT DO NOTHING;

-- Soylent Corp users
INSERT INTO users (id, tenant_id, email, name, password_hash, role, status) VALUES
    ('0e5f6a7b-8c9d-4e0f-1a2b-3c4d5e6f7a8b', 'c3d4e5f6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 'admin@soylent.com', 'Soylent Admin', '$argon2id$v=19$m=19456,t=2,p=1$ZZQeowa8qOIGQziIPCF9kg$yGQS+h+6nGq+E8Aol7Uq0mAeeWYCHlKk4yexS97wiHU', 'Admin', 'inactive')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- DEMO FILE REQUESTS
-- ============================================================================

INSERT INTO file_requests (tenant_id, name, destination_path, token, created_by, expires_at, status, upload_count) VALUES
    ('a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Q4 Financials', '/Finance/2024', 'demo-token-001', '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NOW() + INTERVAL '30 days', 'active', 3),
    ('a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'Vendor Contracts', '/Legal/Contracts', 'demo-token-002', '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NOW() + INTERVAL '15 days', 'active', 12),
    ('b2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 'Marketing Assets', '/Marketing/2024', 'demo-token-003', '0d4e5f6a-7b8c-4d9e-0f1a-2b3c4d5e6f7a', NOW() - INTERVAL '5 days', 'expired', 25)
ON CONFLICT DO NOTHING;

-- ============================================================================
-- DEMO FOLDERS
-- ============================================================================

-- Root folders for ClovaLink (visible as company folders)
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, is_directory, owner_id, parent_path, visibility, is_company_folder)
VALUES
    -- Projects folder (shared company folder)
    ('f0a1b2c3-d4e5-4f6a-7b8c-9d0e1f2a3b01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', NULL, 
     'Projects', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Projects/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'department', true)
ON CONFLICT (id) DO NOTHING;

-- Finance folders
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Finance', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'department'),
    ('f1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     '2024', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/2024/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance', 'department'),
    ('f1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c03', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Invoices', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/Invoices/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance', 'department')
ON CONFLICT (id) DO NOTHING;

-- Legal folders
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'Legal', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'department'),
    ('f2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'Contracts', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/Contracts/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Legal', 'department'),
    ('f2c3d4e5-f6a7-4b8c-9d0e-1f2a3b4c5d03', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'Policies', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/Policies/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Legal', 'department')
ON CONFLICT (id) DO NOTHING;

-- Human Resources folders
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f3d4e5f6-a7b8-4c9d-0e1f-2a3b4c5d6e01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd3c4d5e6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 
     'Human Resources', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Human Resources/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'department'),
    ('f3d4e5f6-a7b8-4c9d-0e1f-2a3b4c5d6e02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd3c4d5e6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 
     'Onboarding', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Human Resources/Onboarding/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Human Resources', 'department')
ON CONFLICT (id) DO NOTHING;

-- Engineering folders
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f4e5f6a7-b8c9-4d0e-1f2a-3b4c5d6e7f01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd4d5e6f7-b8c9-4d0e-1f2a-3b4c5d6e7f8a', 
     'Engineering', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Engineering/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', NULL, 'department'),
    ('f4e5f6a7-b8c9-4d0e-1f2a-3b4c5d6e7f02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd4d5e6f7-b8c9-4d0e-1f2a-3b4c5d6e7f8a', 
     'Documentation', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Engineering/Documentation/', 0, true, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Engineering', 'department')
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- DEMO FILES
-- ============================================================================

-- Finance files
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Q1_Budget.xlsx', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/2024/Q1_Budget.xlsx', 24576, 
     'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance/2024', 'department'),
    ('f5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Q2_Report.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/2024/Q2_Report.pdf', 156789, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance/2024', 'department'),
    ('f5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a03', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Annual_Forecast.xlsx', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/2024/Annual_Forecast.xlsx', 35840, 
     'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance/2024', 'department'),
    ('f5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a04', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd1a2b3c4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 
     'Invoice_Template.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Finance/Invoices/Invoice_Template.pdf', 45678, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Finance/Invoices', 'department')
ON CONFLICT (id) DO NOTHING;

-- Legal files
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f6a7b8c9-d0e1-4f2a-3b4c-5d6e7f8a9b01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'Vendor_Agreement.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/Contracts/Vendor_Agreement.pdf', 234567, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Legal/Contracts', 'department'),
    ('f6a7b8c9-d0e1-4f2a-3b4c-5d6e7f8a9b02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'NDA_Template.docx', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/Contracts/NDA_Template.docx', 28672, 
     'application/vnd.openxmlformats-officedocument.wordprocessingml.document', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Legal/Contracts', 'department'),
    ('f6a7b8c9-d0e1-4f2a-3b4c-5d6e7f8a9b03', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd2b3c4d5-f6a7-4b8c-9d0e-1f2a3b4c5d6e', 
     'Employee_Handbook.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Legal/Policies/Employee_Handbook.pdf', 512000, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Legal/Policies', 'department')
ON CONFLICT (id) DO NOTHING;

-- Human Resources files
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f7b8c9d0-e1f2-4a3b-4c5d-6e7f8a9b0c01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd3c4d5e6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 
     'Welcome_Guide.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Human Resources/Onboarding/Welcome_Guide.pdf', 89012, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Human Resources/Onboarding', 'department'),
    ('f7b8c9d0-e1f2-4a3b-4c5d-6e7f8a9b0c02', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd3c4d5e6-a7b8-4c9d-0e1f-2a3b4c5d6e7f', 
     'Benefits_Overview.xlsx', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Human Resources/Onboarding/Benefits_Overview.xlsx', 18432, 
     'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Human Resources/Onboarding', 'department')
ON CONFLICT (id) DO NOTHING;

-- Engineering files
INSERT INTO files_metadata (id, tenant_id, department_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, parent_path, visibility)
VALUES
    ('f8c9d0e1-f2a3-4b4c-5d6e-7f8a9b0c1d01', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d', 'd4d5e6f7-b8c9-4d0e-1f2a-3b4c5d6e7f8a', 
     'API_Spec.pdf', 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d/Engineering/Documentation/API_Spec.pdf', 145678, 
     'application/pdf', false, 
     '0a1b2c3d-4e5f-4a7b-8c9d-0e1f2a3b4c5d', 'Engineering/Documentation', 'department')
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- UPDATE TENANT STORAGE USED
-- ============================================================================

UPDATE tenants 
SET storage_used_bytes = (
    SELECT COALESCE(SUM(size_bytes), 0) 
    FROM files_metadata 
    WHERE tenant_id = 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d' 
    AND is_directory = false 
    AND is_deleted = false
)
WHERE id = 'a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d';

-- ============================================================================
-- DEFAULT NOTIFICATION PREFERENCES
-- ============================================================================

-- Insert default notification preferences for demo users
INSERT INTO notification_preferences (user_id, event_type, email_enabled, in_app_enabled)
SELECT u.id, event_type.type, true, true
FROM users u
CROSS JOIN (
    VALUES 
        ('file_upload'),
        ('request_expiring'),
        ('user_action'),
        ('compliance_alert'),
        ('storage_warning'),
        ('file_shared')
) AS event_type(type)
ON CONFLICT (user_id, event_type) DO NOTHING;

-- Insert tenant notification settings
INSERT INTO tenant_notification_settings (tenant_id, event_type, enabled, email_enforced, in_app_enforced, default_email, default_in_app)
SELECT t.id, event_type.type, true, false, false, true, true
FROM tenants t
CROSS JOIN (
    VALUES 
        ('file_upload'),
        ('request_expiring'),
        ('user_action'),
        ('compliance_alert'),
        ('storage_warning'),
        ('file_shared')
) AS event_type(type)
ON CONFLICT (tenant_id, event_type, role) DO NOTHING;

-- ============================================================================
-- DEFAULT EMAIL TEMPLATES
-- ============================================================================

INSERT INTO email_templates (template_key, name, subject, body_html, body_text, variables) VALUES
(
    'file_upload',
    'File Upload Notification',
    'New upload to "{{request_name}}"',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .highlight { background: #f3f4f6; padding: 15px; border-radius: 6px; margin: 20px 0; }
        .button { display: inline-block; padding: 12px 24px; background: #667eea; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📁 New File Upload</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>A new file has been uploaded to your file request.</p>
            <div class="highlight">
                <strong>Request:</strong> {{request_name}}<br>
                <strong>File:</strong> {{file_name}}<br>
                <strong>Uploaded by:</strong> {{uploader_name}}
            </div>
            <p>You can view and manage this file in your dashboard.</p>
            <a href="{{app_url}}" class="button">View in Dashboard</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

A new file has been uploaded to your file request "{{request_name}}".

File: {{file_name}}
Uploaded by: {{uploader_name}}

View in dashboard: {{app_url}}

This is an automated notification from {{company_name}}.',
    '["user_name", "request_name", "file_name", "uploader_name", "company_name", "app_url"]'::jsonb
),
(
    'request_expiring',
    'Request Expiring Soon',
    'File request "{{request_name}}" expiring soon',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .warning { background: #fef3c7; border-left: 4px solid #f59e0b; padding: 15px; margin: 20px 0; }
        .button { display: inline-block; padding: 12px 24px; background: #f59e0b; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>⚠️ Request Expiring</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <div class="warning">
                <strong>Your file request "{{request_name}}" will expire in {{days_until_expiry}} day(s).</strong>
            </div>
            <p>After expiration, no new files can be uploaded to this request. If you need to extend the deadline, please update the request settings.</p>
            <a href="{{app_url}}" class="button">Manage Request</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

Your file request "{{request_name}}" will expire in {{days_until_expiry}} day(s).

After expiration, no new files can be uploaded to this request. If you need to extend the deadline, please update the request settings.

Manage request: {{app_url}}

This is an automated notification from {{company_name}}.',
    '["user_name", "request_name", "days_until_expiry", "company_name", "app_url"]'::jsonb
),
(
    'user_created',
    'New User Added',
    'New user added to {{company_name}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #10b981 0%, #059669 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .user-card { background: #f3f4f6; padding: 15px; border-radius: 6px; margin: 20px 0; }
        .button { display: inline-block; padding: 12px 24px; background: #10b981; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>👤 New User Added</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>A new user has been added to your organization.</p>
            <div class="user-card">
                <strong>Name:</strong> {{new_user_name}}<br>
                <strong>Email:</strong> {{new_user_email}}<br>
                <strong>Role:</strong> {{new_user_role}}
            </div>
            <a href="{{app_url}}/users" class="button">View Users</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

A new user has been added to your organization.

Name: {{new_user_name}}
Email: {{new_user_email}}
Role: {{new_user_role}}

View users: {{app_url}}/users

This is an automated notification from {{company_name}}.',
    '["user_name", "new_user_name", "new_user_email", "new_user_role", "company_name", "app_url"]'::jsonb
),
(
    'role_changed',
    'Role Updated',
    'Your role has been updated',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #8b5cf6 0%, #7c3aed 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .role-change { background: #f3f4f6; padding: 15px; border-radius: 6px; margin: 20px 0; text-align: center; }
        .role { display: inline-block; padding: 8px 16px; background: #8b5cf6; color: white; border-radius: 20px; margin: 5px; }
        .arrow { color: #9ca3af; margin: 0 10px; }
        .button { display: inline-block; padding: 12px 24px; background: #8b5cf6; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔄 Role Updated</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>Your role in {{company_name}} has been updated.</p>
            <div class="role-change">
                <span class="role">{{old_role}}</span>
                <span class="arrow">→</span>
                <span class="role">{{new_role}}</span>
            </div>
            <p>Your permissions have been adjusted accordingly. If you have any questions, please contact your administrator.</p>
            <a href="{{app_url}}" class="button">Go to Dashboard</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

Your role in {{company_name}} has been updated.

Previous role: {{old_role}}
New role: {{new_role}}

Your permissions have been adjusted accordingly. If you have any questions, please contact your administrator.

Go to dashboard: {{app_url}}

This is an automated notification from {{company_name}}.',
    '["user_name", "old_role", "new_role", "company_name", "app_url"]'::jsonb
),
(
    'file_shared',
    'File Shared With You',
    '{{sharer_name}} shared a file with you',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .file-card { background: #eff6ff; border: 1px solid #bfdbfe; padding: 15px; border-radius: 6px; margin: 20px 0; display: flex; align-items: center; }
        .file-icon { font-size: 32px; margin-right: 15px; }
        .button { display: inline-block; padding: 12px 24px; background: #3b82f6; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📤 File Shared</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p><strong>{{sharer_name}}</strong> has shared a file with you.</p>
            <div class="file-card">
                <span class="file-icon">📄</span>
                <div>
                    <strong>{{file_name}}</strong>
                </div>
            </div>
            <a href="{{app_url}}" class="button">View File</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

{{sharer_name}} has shared a file with you.

File: {{file_name}}

View file: {{app_url}}

This is an automated notification from {{company_name}}.',
    '["user_name", "sharer_name", "file_name", "company_name", "app_url"]'::jsonb
),
(
    'compliance_alert',
    'Compliance Alert',
    'Compliance Alert: {{alert_type}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .alert { background: #fef2f2; border-left: 4px solid #ef4444; padding: 15px; margin: 20px 0; }
        .button { display: inline-block; padding: 12px 24px; background: #ef4444; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚨 Compliance Alert</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <div class="alert">
                <strong>{{alert_type}}</strong><br><br>
                {{message}}
            </div>
            <p>Please review this alert and take appropriate action to maintain compliance.</p>
            <a href="{{app_url}}/settings" class="button">View Settings</a>
        </div>
        <div class="footer">
            <p>This is an automated compliance notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

COMPLIANCE ALERT: {{alert_type}}

{{message}}

Please review this alert and take appropriate action to maintain compliance.

View settings: {{app_url}}/settings

This is an automated compliance notification from {{company_name}}.',
    '["user_name", "alert_type", "message", "company_name", "app_url"]'::jsonb
),
(
    'storage_warning',
    'Storage Warning',
    'Storage quota warning for {{company_name}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #f97316 0%, #ea580c 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .progress-bar { background: #e5e7eb; border-radius: 10px; height: 20px; margin: 20px 0; overflow: hidden; }
        .progress { background: linear-gradient(90deg, #f97316, #ea580c); height: 100%; border-radius: 10px; }
        .stats { background: #fff7ed; padding: 15px; border-radius: 6px; margin: 20px 0; text-align: center; }
        .button { display: inline-block; padding: 12px 24px; background: #f97316; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>💾 Storage Warning</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>Your organization''s storage is running low.</p>
            <div class="progress-bar">
                <div class="progress" style="width: {{percentage_used}}%;"></div>
            </div>
            <div class="stats">
                <strong style="font-size: 24px;">{{percentage_used}}%</strong><br>
                of storage used
            </div>
            <p>Consider freeing up space by removing old files or upgrading your storage plan.</p>
            <a href="{{app_url}}/settings" class="button">Manage Storage</a>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

Your organization''s storage is running low.

Storage used: {{percentage_used}}%

Consider freeing up space by removing old files or upgrading your storage plan.

Manage storage: {{app_url}}/settings

This is an automated notification from {{company_name}}.',
    '["user_name", "percentage_used", "company_name", "app_url"]'::jsonb
),
(
    'password_reset',
    'Password Reset Request',
    'Password reset request for {{company_name}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .button { display: inline-block; padding: 14px 28px; background: #6366f1; color: white; text-decoration: none; border-radius: 6px; margin: 20px 0; font-weight: bold; }
        .warning { background: #fef3c7; border-left: 4px solid #f59e0b; padding: 15px; margin: 20px 0; font-size: 14px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔐 Password Reset</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>We received a request to reset your password. Click the button below to create a new password:</p>
            <p style="text-align: center;">
                <a href="{{reset_link}}" class="button">Reset Password</a>
            </p>
            <div class="warning">
                <strong>⚠️ This link will expire in 1 hour.</strong><br>
                If you didn''t request this password reset, you can safely ignore this email.
            </div>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

We received a request to reset your password.

Click the link below to create a new password:
{{reset_link}}

This link will expire in 1 hour.

If you didn''t request this password reset, you can safely ignore this email.

This is an automated notification from {{company_name}}.',
    '["user_name", "reset_link", "company_name"]'::jsonb
),
(
    'welcome',
    'Welcome Email',
    'Welcome to {{company_name}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 40px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 28px; }
        .header p { margin: 10px 0 0 0; opacity: 0.9; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .credentials { background: #f3f4f6; padding: 20px; border-radius: 6px; margin: 20px 0; }
        .credentials p { margin: 5px 0; }
        .button { display: inline-block; padding: 14px 28px; background: #667eea; color: white; text-decoration: none; border-radius: 6px; margin: 20px 0; font-weight: bold; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>👋 Welcome!</h1>
            <p>You''ve been added to {{company_name}}</p>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>An account has been created for you at {{company_name}}. Here are your login credentials:</p>
            <div class="credentials">
                <p><strong>Email:</strong> {{user_email}}</p>
                <p><strong>Temporary Password:</strong> {{temp_password}}</p>
                <p><strong>Role:</strong> {{role}}</p>
            </div>
            <p><strong>⚠️ Important:</strong> Please change your password after your first login.</p>
            <p style="text-align: center;">
                <a href="{{app_url}}/login" class="button">Log In Now</a>
            </p>
        </div>
        <div class="footer">
            <p>This is an automated notification from {{company_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

Welcome to {{company_name}}! An account has been created for you.

Your login credentials:
Email: {{user_email}}
Temporary Password: {{temp_password}}
Role: {{role}}

IMPORTANT: Please change your password after your first login.

Log in at: {{app_url}}/login

This is an automated notification from {{company_name}}.',
    '["user_name", "user_email", "temp_password", "role", "company_name", "app_url"]'::jsonb
),
(
    'security_alert',
    'Security Alert',
    '🚨 Security Alert: {{alert_title}}',
    '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, ''Segoe UI'', Roboto, sans-serif; line-height: 1.6; color: #333; margin: 0; padding: 0; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #dc2626 0%, #991b1b 100%); color: white; padding: 30px 20px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .content p { margin: 0 0 15px 0; }
        .severity-critical { background: #fef2f2; border-left: 4px solid #dc2626; padding: 15px; margin: 20px 0; }
        .severity-high { background: #fff7ed; border-left: 4px solid #ea580c; padding: 15px; margin: 20px 0; }
        .severity-badge { display: inline-block; padding: 4px 12px; border-radius: 20px; font-size: 12px; font-weight: bold; text-transform: uppercase; }
        .severity-critical .severity-badge { background: #dc2626; color: white; }
        .severity-high .severity-badge { background: #ea580c; color: white; }
        .details { background: #f9fafb; padding: 15px; border-radius: 6px; margin: 20px 0; }
        .details p { margin: 5px 0; font-size: 14px; }
        .details strong { color: #374151; }
        .button { display: inline-block; padding: 12px 24px; background: #dc2626; color: white; text-decoration: none; border-radius: 6px; margin-top: 15px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; color: #6b7280; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🛡️ Security Alert</h1>
        </div>
        <div class="content">
            <p>Hi {{user_name}},</p>
            <p>A security alert has been triggered that requires your attention.</p>
            <div class="severity-{{severity_lower}}">
                <span class="severity-badge">{{severity}}</span>
                <h3 style="margin: 10px 0 5px 0;">{{alert_title}}</h3>
                <p style="margin: 0; color: #6b7280;">{{description}}</p>
            </div>
            <div class="details">
                <p><strong>Alert Type:</strong> {{alert_type_display}}</p>
                <p><strong>Time:</strong> {{timestamp}}</p>
                {{#if affected_user}}<p><strong>Affected User:</strong> {{affected_user}}</p>{{/if}}
                {{#if ip_address}}<p><strong>IP Address:</strong> {{ip_address}}</p>{{/if}}
                <p><strong>Company:</strong> {{tenant_name}}</p>
            </div>
            <p>Please review this alert and take appropriate action.</p>
            <a href="{{app_url}}/security" class="button">View Security Dashboard</a>
        </div>
        <div class="footer">
            <p>This is an automated security notification from {{tenant_name}}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>',
    'Hi {{user_name}},

A security alert has been triggered that requires your attention.

SEVERITY: {{severity}}
ALERT: {{alert_title}}

{{description}}

Details:
- Alert Type: {{alert_type_display}}
- Time: {{timestamp}}
- Affected User: {{affected_user}}
- IP Address: {{ip_address}}
- Company: {{tenant_name}}

Please review this alert and take appropriate action.

View Security Dashboard: {{app_url}}/security

This is an automated security notification from {{tenant_name}}.',
    '["user_name", "severity", "severity_lower", "alert_title", "description", "alert_type", "alert_type_display", "timestamp", "affected_user", "ip_address", "tenant_name", "app_url"]'::jsonb
),
(
    'malware_detected',
    'Malware Detection Alert',
    '🛡️ Security Alert: Malware Detected in {{file_name}}',
    '<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; line-height: 1.6; color: #333; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #dc2626 0%, #991b1b 100%); color: white; padding: 30px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .alert-box { background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px; padding: 20px; margin: 20px 0; }
        .alert-box h3 { color: #991b1b; margin-top: 0; }
        .detail-row { display: flex; padding: 10px 0; border-bottom: 1px solid #e5e7eb; }
        .detail-label { font-weight: 600; width: 140px; color: #6b7280; }
        .detail-value { color: #111827; }
        .action-badge { display: inline-block; padding: 4px 12px; border-radius: 9999px; font-size: 14px; font-weight: 600; }
        .action-quarantine { background: #fef3c7; color: #92400e; }
        .action-delete { background: #fee2e2; color: #991b1b; }
        .action-flag { background: #dbeafe; color: #1e40af; }
        .button { display: inline-block; background: #dc2626; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 600; margin-top: 20px; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; font-size: 14px; color: #6b7280; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🛡️ Malware Detected</h1>
        </div>
        <div class="content">
            <p>A file uploaded to <strong>{{company_name}}</strong> has been detected as malicious.</p>
            
            <div class="alert-box">
                <h3>Threat Details</h3>
                <div class="detail-row">
                    <span class="detail-label">File Name:</span>
                    <span class="detail-value">{{file_name}}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Threat:</span>
                    <span class="detail-value"><strong>{{threat_name}}</strong></span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Uploaded By:</span>
                    <span class="detail-value">{{uploader_email}}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Scanned At:</span>
                    <span class="detail-value">{{scanned_at}}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Action Taken:</span>
                    <span class="detail-value">
                        <span class="action-badge action-{{action_class}}">{{action_taken}}</span>
                    </span>
                </div>
            </div>
            
            <p>Please review this incident in your security dashboard.</p>
            
            <a href="{{app_url}}/security" class="button">View Security Dashboard</a>
        </div>
        <div class="footer">
            <p>This is an automated security alert from {{company_name}}.</p>
            <p>If you have questions, please contact your system administrator.</p>
        </div>
    </div>
</body>
</html>',
    'SECURITY ALERT: Malware Detected

A file uploaded to {{company_name}} has been detected as malicious.

THREAT DETAILS:
- File Name: {{file_name}}
- Threat: {{threat_name}}
- Uploaded By: {{uploader_email}}
- Scanned At: {{scanned_at}}
- Action Taken: {{action_taken}}

Please review this incident in your security dashboard:
{{app_url}}/security

This is an automated security alert from {{company_name}}.
If you have questions, please contact your system administrator.',
    '{"file_name": "Name of the infected file", "threat_name": "Name of the detected threat", "uploader_email": "Email of the user who uploaded the file", "scanned_at": "Timestamp of the scan", "action_taken": "Action taken (Quarantined, Deleted, Flagged)", "action_class": "CSS class for action badge", "company_name": "Organization name", "app_url": "Application URL"}'::jsonb
),
(
    'malware_detected_uploader',
    'File Security Alert (Uploader)',
    'Security Notice: Your uploaded file was flagged',
    '<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; line-height: 1.6; color: #333; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%); color: white; padding: 30px; border-radius: 8px 8px 0 0; text-align: center; }
        .header h1 { margin: 0; font-size: 24px; }
        .content { background: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; }
        .notice-box { background: #fffbeb; border: 1px solid #fde68a; border-radius: 8px; padding: 20px; margin: 20px 0; }
        .detail-row { display: flex; padding: 8px 0; }
        .detail-label { font-weight: 600; width: 120px; color: #6b7280; }
        .detail-value { color: #111827; }
        .footer { background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; text-align: center; font-size: 14px; color: #6b7280; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>⚠️ Security Notice</h1>
        </div>
        <div class="content">
            <p>Hi,</p>
            
            <p>A file you recently uploaded has been flagged by our security scanner and has been <strong>{{action_taken}}</strong> as a precaution.</p>
            
            <div class="notice-box">
                <div class="detail-row">
                    <span class="detail-label">File:</span>
                    <span class="detail-value">{{file_name}}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Reason:</span>
                    <span class="detail-value">{{threat_name}}</span>
                </div>
            </div>
            
            <p><strong>What does this mean?</strong></p>
            <p>Our automated security scanner detected something potentially harmful in your file. This can sometimes happen with:</p>
            <ul>
                <li>Files containing macros or scripts</li>
                <li>Password-protected archives</li>
                <li>Legitimate software that triggers false positives</li>
            </ul>
            
            <p><strong>What should I do?</strong></p>
            <p>If you believe this was a mistake, please contact your administrator. They can review the detection and restore the file if appropriate.</p>
        </div>
        <div class="footer">
            <p>This is an automated message from {{company_name}}.</p>
        </div>
    </div>
</body>
</html>',
    'SECURITY NOTICE

Hi,

A file you recently uploaded has been flagged by our security scanner and has been {{action_taken}} as a precaution.

FILE DETAILS:
- File: {{file_name}}
- Reason: {{threat_name}}

WHAT DOES THIS MEAN?
Our automated security scanner detected something potentially harmful in your file. This can sometimes happen with files containing macros, password-protected archives, or legitimate software that triggers false positives.

WHAT SHOULD I DO?
If you believe this was a mistake, please contact your administrator. They can review the detection and restore the file if appropriate.

This is an automated message from {{company_name}}.',
    '{"file_name": "Name of the flagged file", "threat_name": "Reason for flagging", "action_taken": "Action taken (quarantined, removed, flagged)", "company_name": "Organization name"}'::jsonb
)
ON CONFLICT (template_key) DO NOTHING;
