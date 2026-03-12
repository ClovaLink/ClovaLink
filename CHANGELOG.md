# Changelog

All notable changes to ClovaLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.6] - 2026-03-12

### Added

- **Backup & Restore**: Full tenant and global backup/restore system
  - Export all tenant data: settings, users, departments, roles, SSO config, policies, notifications
  - Optional large sections: file metadata, audit logs, approval history
  - Global settings export/import (SuperAdmin only)
  - NixOS-style settings profiles — apply partial JSON config as a merge
  - Import preview with dry-run diff before committing changes
  - Transaction-safe imports — any error rolls back everything

- **Backup Security**: Defense-in-depth encryption and abuse prevention
  - Mandatory ChaCha20-Poly1305 AEAD encryption on all backup exports
  - Argon2id key derivation from user passphrase (64MB memory, 4 iterations, 4 parallelism)
  - Password re-confirmation required for all backup operations
  - Sensitive fields (passwords, secrets) always redacted; `include_secrets` requires SuperAdmin
  - Rate limiting: 1 export per 5 min, 1 import per 10 min per tenant
  - Brute-force protection: 5 failed passphrase attempts → lockout + critical security alert
  - Security alerts: `BackupDecryptFailed` (High), `BackupBruteForce` (Critical), `BackupExportSecrets` (High)
  - Full audit trail for every export, import, and failed attempt

- **Backup Frontend**: Integrated UI for backup operations
  - Backup & Restore dedicated tab in Global Settings (moved from System tab)
  - Backup & Restore tab in Company Details (tenant-level)
  - Categorized section checkboxes with human-readable names and count badges
  - Export destination toggle: download to browser or save to storage backend
  - Saved Backups list with download/delete, auto/manual badges
  - Pagination controls for large data sections (audit days, file limit)
  - Circuit breaker health indicator (green/yellow/red)
  - Storage migration info banner

- **Per-Tenant Backup Toggle**: Make backup optional per tenant
  - `backup_enabled` column on tenants table (default true)
  - Admin toggle in Backup tab header, SuperAdmin bypass
  - Tab hidden when backup disabled (non-SuperAdmin)

- **Save-to-Storage**: Persist backups on S3/local storage backend
  - `POST /api/backup/save` — export + save to `_backups/` directory
  - `GET /api/backup/saved` — list saved backups from history
  - `GET /api/backup/saved/:id/download` — download from storage
  - `DELETE /api/backup/saved/:id` — delete from storage + history
  - `backup_history` table tracks all backups with metadata

- **Scheduled Automatic Backups**: Cron-based auto-backup (SuperAdmin)
  - `auto_backup_enabled`, `auto_backup_cron`, `auto_backup_retention_count` per tenant
  - Background scheduler polls every 60 seconds with Redis distributed lock
  - Random jitter (0-30s) and batch limit (5/cycle) prevent thundering herd
  - System-generated passphrase stored in `global_settings`
  - Auto-retention cleanup deletes oldest backups beyond limit

- **Backup Circuit Breaker + Concurrency**: Production-safe backup operations
  - Circuit breaker (3 failures → open, 60s recovery) on all backup operations
  - `tokio::Semaphore` limits concurrent backups (env `BACKUP_MAX_CONCURRENT`, default 2)
  - 503 when circuit open, 429 when semaphore full
  - Only infrastructure failures trip the breaker, not user errors

- **Backup Pagination**: Memory-safe export for large tenants
  - Audit logs: date-range filter (`audit_days`, default 90)
  - File metadata: record limit (`file_limit`, default 50000, max 100k)
  - Approval history: date-range filter (`approval_days`, default 90)
  - Hard caps on all SQL queries with LIMIT

- **Backup Performance Dashboard**: Metrics in Performance tab
  - Circuit breaker state, total/auto/manual counts, failed in 24h, storage used
  - Concurrency status (max/active/available permits)
  - Per-tenant table: backup count, last backup, auto-backup status
  - Color-coded warnings for stale backups (>7 days)

- **Backup Section Counts API**: `GET /api/backup/section-counts`
  - Returns record counts for users, departments, roles, files, etc.
  - Powers count badges in the export section selector UI

- **Document Approvals Redesign**: Card-based UI matching Security Alerts style
  - Card-style approval items with color-coded status icons
  - Bulk select/approve/reject with select-all and indeterminate state
  - Detail modal with full approval request info
  - Pagination (15 per page) with smart ellipsis
  - Filter panel: status, date range, keyword search

- **File Browser: Items-per-Page Selector**: Power-user control over pagination
  - Options: Auto (viewport-based), 10, 25, 50, 100
  - Persisted per-user in localStorage

- **File Browser: Display Density Toggle**: Adjustable row sizing in list view
  - Compact, Normal, and Comfortable modes
  - Persisted per-user in localStorage

- **File Browser: Resizable Columns**: Drag column borders in list view
  - Resize Size, Modified, and Owner columns by dragging
  - Double-click column border to reset to default width
  - Persisted per-user in localStorage

- **File Browser: View-Mode-Aware Pagination**: Smarter auto-pagination
  - List view calculates items from viewport height and row density
  - Grid view calculates from viewport width breakpoints

### Changed

- **File browser date formatting** — Modified column uses user-configured date/time format via `formatDateTime`
- **File browser view mode persistence** — grid/list choice saved per-user in localStorage
- **Per-user localStorage scoping** — all preferences keyed by user ID (sidebar, view mode, density, columns, per-page)
- **Upload progress modal** — replaced blocking full-screen modal with non-blocking bottom-right toast (auto-dismiss, collapsible)
- **Global Settings width** — all pages widened from `max-w-3xl` to `max-w-6xl mx-auto`
- **Company Settings width** — all tabs (Settings, Notifications, Document Workflow, Email Templates, AI, Audit) widened to `max-w-6xl mx-auto`
- **File action menu positioning** — measures actual menu height instead of fixed 550px estimate; "Add to Group" submenu flies left to avoid viewport cutoff

### Security

- **BACKUP_MASTER_KEY required for auto-backups** — enabling scheduled backups now requires BACKUP_MASTER_KEY to be configured; manual export/import unaffected; startup logs warning if not set
- **KDF applied to master key** — BACKUP_MASTER_KEY now processed through Argon2id instead of direct byte-copy
- **Argon2id parameters increased** — 4 iterations, 4 parallelism (was 3 iterations, 1 parallelism)
- **Max passphrase length enforced** — 1024 character limit prevents KDF denial-of-service
- **Password confirmation rate limited** — 5 failed attempts per 15 minutes triggers lockout with `PasswordConfirmFailed` security alert
- **Import data validation** — permission names validated against known list, email format checked, department parent_id verified
- **Sensitive key detection expanded** — pattern-based matching for keys containing `secret`, `password`, `key`, `token`, `encrypted`
- **Frontend passphrase cleanup** — passphrases cleared from state on error, success, and component unmount

### Notes

- **New env var**: `BACKUP_MASTER_KEY` (minimum 32 chars) — required to enable scheduled auto-backups (encrypts the system-generated passphrase at rest). Manual export/import works without it. Generate with: `openssl rand -base64 48`.
- **Migration compatibility**: v0.1.6 migrations (005, 006) are safe to run on v0.1.4 and v0.1.5 databases. All use `IF NOT EXISTS`, `ON CONFLICT DO NOTHING`, and sensible defaults.
- **No breaking changes**: All frontend changes are additive. localStorage keys are new per-user scoped keys; old keys are ignored.

## [0.1.5] - 2026-03-10

### Added

- **Document Approval Workflow**: Configurable approval policies for document uploads
  - Per-tenant toggle to enable/disable the workflow (`approval_workflow_enabled`)
  - 7 policy scope types: `all`, `department`, `company_folder`, `file_type`, `file_size`, `role`, `private_files`
  - Priority-based policy matching — specific scopes evaluated before catch-all
  - Automatic approval check on file upload when enabled
  - Manual "Send for Approval" action for files not caught by policies
  - Approve/reject flow with comments and resubmission support
  - Pending and History tabs for approvers, My Pending section for file owners
  - Approval statistics and metrics endpoint
  - Email notifications for ApprovalRequired and ApprovalDecision events

- **Approval Permissions**: New granular permissions for the approval system
  - `approvals.view` — view approval requests (Manager+ by default)
  - `approvals.manage` — approve/reject requests (Manager+ by default)
  - Configurable per custom role

- **Wiki Documentation**: Added dedicated feature guides
  - SSO Authentication guide (OIDC & SAML setup, attribute mapping, troubleshooting)
  - Document Approval guide (policies, workflow, API reference)
  - Updated Home, Admin Guide, API Reference, and Security pages

### Changed

- Files table has new `approval_status` column (`pending`, `approved`, `rejected`, or null)
- Tenants table has new `approval_workflow_enabled` boolean column (default false)
- New database tables: `approval_policies` and `approval_requests`
- Document Workflow tab added to Company Details in admin panel
- Sidebar navigation conditionally shows Approvals link based on tenant flag

### Security

- Tenant isolation enforced on all approval endpoints
- Atomic approve/reject operations prevent race conditions
- Role-based access control for policy management and approval actions
- All approval actions recorded in audit log
- Input validation on policy scopes and approval request state transitions

### Notes

- **Backwards compatible**: Approval workflow is disabled by default. Existing files have null `approval_status`. No action needed for existing installations.
- **Migration 004**: `004_document_approvals.sql` adds the approval tables and columns.

## [0.1.4] - 2026-03-04

### Added

- **OIDC Single Sign-On (SSO)**: Enterprise SSO support via OpenID Connect
  - Connect Google Workspace, Microsoft Entra ID, Okta, or any OIDC provider
  - Per-tenant provider configuration with admin UI (Settings → SSO)
  - Email domain discovery — SSO buttons appear automatically on login page
  - Account linking — existing users can link SSO from their profile
  - Auto-provisioning (opt-in) — new users created on first SSO login
  - Configurable MFA trust — choose whether to require ClovaLink 2FA on top of IdP auth
  - Passwordless users — create SSO-only accounts with no password
  - Hybrid auth — users can have both password and SSO enabled

- **SAML 2.0 Single Sign-On**: Enterprise SAML SSO for organizations using ADFS, Okta, Azure AD, and other SAML IdPs
  - Pure Rust implementation — no C dependencies, no unsafe code, no Dockerfile changes
  - SP metadata endpoint for easy IdP configuration
  - HTTP-POST and HTTP-Redirect bindings
  - RSA-SHA256 and RSA-SHA1 XML signature verification
  - Assertion replay protection, time window validation, audience restriction
  - Configurable NameID format, attribute mapping for email/name
  - Same auto-provision, MFA trust, and account linking features as OIDC

- **IdP Attribute/Claim Mapping**: Map IdP-provided attributes to ClovaLink roles and departments
  - Works for both OIDC and SAML providers
  - Map IdP groups, roles, or department attributes to ClovaLink base roles, custom roles, and departments
  - Priority-based evaluation — first match wins
  - Exact, contains, and regex match types
  - Admin UI in Settings → SSO → Attribute Mappings tab

- **Install Script Update Command**: `install.sh --update` for easy in-place upgrades
  - Backs up .env and compose.yml, pulls latest images, restarts services
  - One-liner: `curl -fsSL .../install.sh | bash -s -- --update`

### Changed

- `password_hash` column is now nullable (supports SSO-only users)
- Users table has new `identity_provider` column (`local`, `oidc`, `saml`, `hybrid`)
- Tenants table has new `auth_methods` column (array of enabled auth methods)
- Login endpoint returns `sso_required` error for SSO-only users attempting password login
- Password reset is blocked for SSO-only users (directed to IdP instead)
- SSO provider management elevated to SuperAdmin (security-critical configuration)
- SSO Settings page now has tabs: OIDC Providers, SAML Providers, Attribute Mappings
- Linked Accounts section on Profile shows both OIDC and SAML identities with protocol badge

### Security

- OIDC state/nonce parameters for CSRF and replay protection (10-minute expiry, one-time use)
- SAML RelayState CSRF protection (10-minute expiry, one-time use)
- SAML assertion replay protection via consumed assertions table
- SAML XML signature verification (pure Rust: Exclusive C14N, RSA-SHA256/SHA1)
- SAML InResponseTo validation against stored AuthnRequest IDs
- Client secrets encrypted at rest in database
- Account unlinking blocked if it would lock out the user (no password + last identity)
- Provider deletion warns about SSO-only users who would lose access

### Notes

- **Backwards compatible**: Existing users default to `identity_provider = 'local'`, tenants default to `auth_methods = ['local']`. No action needed for existing installations.
- **New env var** (only needed if configuring SSO): `SECRETS_ENCRYPTION_KEY` for encrypting IdP client secrets at rest. OIDC callback and frontend URLs are derived automatically from `BASE_URL`.
- **Pure Rust SAML**: No system packages or C libraries required. SAML verification uses `rsa`, `x509-cert`, `quick-xml`, and `der` crates.

## [0.1.3.1] - 2026-02-09

### Fixed

- **SMTP TLS Configuration**: Fixed conflicting TLS modes that caused email delivery failures on most SMTP providers (Gmail, SendGrid, Office365, etc.). Port 465 now correctly uses implicit TLS, port 587 uses STARTTLS, and non-secure connections are handled properly.
- **HTML Email Rendering**: Emails were being sent with `text/plain` content type despite containing HTML, causing recipients to see raw HTML tags. Emails now use `text/html` content type for proper rendering.
- **User Invite 422 Error**: Fixed "Failed to invite user" error caused by empty string department/tenant IDs failing UUID deserialization. Empty strings are now correctly treated as null.

## [0.1.3] - 2026-01-23

### Added

- **Local Storage Encryption**: Files stored on local disk are now encrypted at rest using ChaCha20-Poly1305 authenticated encryption
  - Enable by setting `ENCRYPTION_KEY` environment variable (base64-encoded 32-byte key)
  - Generate a key with: `openssl rand -base64 32`
  - Each file uses a unique random nonce (no nonce reuse)
  - AEAD provides both confidentiality and integrity verification
  - Backwards compatible: existing unencrypted files remain readable when encryption is enabled

- **Install Script Encryption Prompt**: The installer now asks if you want to enable local file encryption and automatically generates a secure key

### Changed

- Docker Compose now includes commented `ENCRYPTION_KEY` configuration with documentation

### Security

- Local storage encryption uses ChaCha20-Poly1305 (RFC 8439), a modern AEAD cipher
- S3 storage continues to use provider-side encryption (AWS SSE, etc.)
- Encryption keys must be exactly 32 bytes (256 bits)

### Notes

- **Key Management**: Store your encryption key securely. Losing the key means losing access to all encrypted files.
- **S3 Users**: The `ENCRYPTION_KEY` setting only applies to local storage. S3 storage uses provider-side encryption.
- **Performance**: ChaCha20 is optimized for software implementations and adds minimal overhead.

## [0.1.2] - 2026-01-15

### Added

- File Groups feature for organizing files into collections
- File comments and threaded replies
- Discord OAuth integration for notifications
- AI-powered file summaries and semantic search

### Fixed

- Various bug fixes and performance improvements

## [0.1.1] - 2026-01-01

### Added

- Virus scanning with ClamAV integration
- S3 replication for enterprise durability
- Transfer scheduler for prioritized uploads/downloads
- Circuit breaker pattern for external service resilience

## [0.1.0] - 2025-12-15

### Added

- Initial release
- Multi-tenant file management
- Role-based access control
- File sharing with expiring links
- File requests for external uploads
- Audit logging and compliance modes (HIPAA, SOX, GDPR)
- Content-addressed storage with deduplication
- S3 and local storage backends
