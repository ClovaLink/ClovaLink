# Changelog

All notable changes to ClovaLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
