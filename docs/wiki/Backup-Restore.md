# Backup & Restore

ClovaLink includes a comprehensive backup and restore system that lets you export all tenant data, import it on another instance, and apply declarative settings profiles.

## Overview

Backups cover your entire tenant configuration and optionally large data sets:

### Always Included (Core Sections)
| Section | Description |
|---------|-------------|
| `tenant_core` | Tenant settings (compliance, SMTP, auth, storage) |
| `users` | User accounts, roles, department assignments |
| `departments` | Organization structure |
| `roles` | Custom roles and permission grants |
| `settings_audit` | Audit log configuration |
| `settings_virus_scan` | Virus scanning settings |
| `settings_ai` | AI provider configuration |
| `settings_discord` | Discord webhook settings |
| `sso_oidc` | OIDC provider configurations |
| `sso_saml` | SAML provider configurations |
| `sso_mappings` | SSO attribute-to-role mappings |
| `sso_identities` | User SSO account links |
| `approval_policies` | Document approval rules |
| `email_templates` | Custom email template overrides |
| `notification_settings` | Notification preferences |

### Optional Sections (Large Data)
| Section | Description |
|---------|-------------|
| `file_metadata` | File records, shares, and requests (not actual files) |
| `audit_logs` | Activity history |
| `approval_history` | Approval request decisions |

### Global Backup (SuperAdmin Only)
| Section | Description |
|---------|-------------|
| `global_settings` | App name, branding, ToS, etc. |
| `global_email_templates` | Default email templates |

## Security

Security is the #1 priority for the backup system.

### Encryption
- **Mandatory**: All backups are encrypted before download — no plaintext option
- **ChaCha20-Poly1305** AEAD cipher (same as file-at-rest encryption)
- **Argon2id** key derivation from your passphrase (64MB memory, 4 iterations, 4 parallelism, 32-byte key)
- Random 16-byte salt and 12-byte nonce per export
- Wrong passphrase → AEAD tag verification fails → clear error message
- Max passphrase length: 1024 characters (prevents DoS via expensive key derivation)

### BACKUP_MASTER_KEY (Required for Auto-Backups)
The `BACKUP_MASTER_KEY` environment variable encrypts the system-generated passphrase used by scheduled auto-backups:
- **Required to enable scheduled auto-backups** — the toggle is disabled in the UI until configured
- **Not required for manual export/import** — users provide their own passphrase each time
- **Not required to start the server** — startup logs a warning if unset, but the server runs fine
- Minimum 32 characters. Generate with: `openssl rand -base64 48`
- Processed through Argon2id KDF with fixed salt (not used as raw bytes)
- Add to your `.env` file: `BACKUP_MASTER_KEY=<your-generated-key>`

### Authentication
- Password re-confirmation required for all backup operations
- **Rate limited**: 5 failed password confirmations per 15 minutes → `PasswordConfirmFailed` alert + lockout
- SuperAdmin required for: global exports, `include_secrets`, importing password hashes
- Admin+ required for: tenant exports/imports, settings profiles
- Every query scoped to `tenant_id` — cross-tenant access is impossible

### Sensitive Field Handling
- **Always redacted** (never exported): `password_hash`, `totp_secret`, `recovery_token`
- **Redacted by default**, included with `include_secrets=true` (SuperAdmin only): SMTP passwords, client secrets, API keys, webhook URLs, signing keys
- **Pattern-based detection**: Any global settings key containing `secret`, `password`, `key`, `token`, or `encrypted` is automatically stripped from exports
- On import: redacted fields (`***REDACTED***`) are skipped — existing values preserved

### Import Validation
Data is validated before being applied to the database:
- **Permissions**: Only known permission names accepted (unknown permissions logged and skipped)
- **Email addresses**: Basic format validation (must contain `@` and `.`, max 254 chars)
- **Department parent_id**: Verified to exist in the same tenant before creating child departments

### Rate Limiting
- Export: 1 per 5 minutes per tenant
- Import: 1 per 10 minutes per tenant
- Failed passphrase: 5 attempts per 15 minutes → lockout + critical security alert
- Password confirmation: 5 attempts per 15 minutes → lockout
- Max upload size: 50MB

### Security Alerts
Four alert types are created automatically:
- **BackupDecryptFailed** (High) — failed passphrase attempt, emails admins
- **BackupBruteForce** (Critical) — 5+ failures in 15 min, locks backup operations, emails admins
- **BackupExportSecrets** (High) — `include_secrets` used, logs who and from where
- **PasswordConfirmFailed** (Medium) — failed password confirmation for backup operation

### Audit Trail
Every backup operation is logged:
- `backup_export` — sections exported, include_secrets flag, IP
- `backup_import` — sections imported, dry_run flag, changes summary
- `backup_decrypt_failed` — failed decryption with IP
- `backup_apply_profile` — changed fields summary

## API Reference

### Tenant Backup

#### Export
```
GET /api/backup/export
Headers: Authorization, X-Confirm-Password, X-Backup-Passphrase
Query: ?sections=tenant_core,users,departments&include_optional=file_metadata&include_secrets=true
```

#### Import
```
POST /api/backup/import
Headers: Authorization, X-Confirm-Password, X-Backup-Passphrase
Body: encrypted backup JSON
```

#### Import Preview (Dry Run)
```
POST /api/backup/import/preview
Headers: Authorization, X-Confirm-Password, X-Backup-Passphrase
Body: encrypted backup JSON
```

#### Apply Settings Profile
```
POST /api/backup/apply-profile
Headers: Authorization, X-Confirm-Password
Body: { "tenant_core": { "smtp_host": "mail.example.com" }, "settings_ai": { ... } }
```

### Save-to-Storage
```
POST   /api/backup/save                 # Export + save to storage backend
GET    /api/backup/saved                # List saved backups
GET    /api/backup/saved/:id/download   # Download saved backup
DELETE /api/backup/saved/:id            # Delete saved backup
```

### Health & Metrics
```
GET /api/backup/health                  # Circuit breaker state (Admin+)
GET /api/backup/metrics                 # Backup performance metrics (SuperAdmin)
GET /api/backup/section-counts          # Record counts per section (Admin+)
```

### Global Backup (SuperAdmin)

```
GET  /api/backup/global/export         # Export global settings
POST /api/backup/global/import         # Import global settings
POST /api/backup/global/import/preview # Preview global import
```

## Per-Tenant Toggle

Backups can be enabled/disabled per tenant via the `backup_enabled` column (default: `true`). When disabled:
- All backup endpoints return 403 for non-SuperAdmin users
- The Backup tab is hidden in Company Details
- SuperAdmin always has access regardless of the toggle

## Circuit Breaker

The backup system uses a circuit breaker pattern to prevent cascading failures:
- **Closed** (normal): requests pass through
- **Open** (after 3 infrastructure failures): returns 503, recovers after 60 seconds
- **Half-open** (recovery test): allows limited requests, closes after 2 successes

Only infrastructure failures (DB/storage errors) trip the breaker. User errors (bad passphrase, 403) do not.

A concurrency semaphore (default 2, configurable via `BACKUP_MAX_CONCURRENT`) limits concurrent backup operations across manual + automatic backups. Returns 429 when full.

## Scheduled Automatic Backups

SuperAdmin can enable automatic backups per tenant:
- **Schedule**: Configurable cron expression (default: `0 2 * * 0` = weekly Sunday 2am)
- **Retention**: Keep last N backups (default: 5), oldest auto-deleted
- **Passphrase**: System-generated, stored in `global_settings`
- **Safety**: Redis distributed lock prevents duplicate runs, max 5 tenants per poll cycle, random jitter (0-30s) between tenants

## Performance Monitoring

Navigate to **Performance → Backups** tab (SuperAdmin only) to see:
- Circuit breaker state
- Total/auto/manual backup counts
- Failed backups in last 24 hours
- Storage usage
- Per-tenant backup status table

## Pagination for Large Sections

Export query parameters for controlling large data:
- `audit_days` (default 90): Only include audit logs from the last N days
- `file_limit` (default 50000, max 100000): Maximum file metadata records
- `approval_days` (default 90): Only include approval history from the last N days

## Frontend Usage

### Tenant Backup
Navigate to **Company Details → Backup & Restore** tab. From here you can:
1. **Export**: Select sections by category, choose pagination limits for large data, pick download or save-to-storage
2. **Import**: Upload a backup file, enter the passphrase to decrypt, preview changes, and apply
3. **Saved Backups**: View, download, or delete previously saved backups

### Global Backup
Navigate to **Settings → System → Global Backup & Restore**. Same workflow as tenant backup but for global settings.

### Settings Profiles
Use the **Apply Profile** feature to merge partial JSON configuration into your tenant settings. This is similar to NixOS declarative configuration — you define only the fields you want to change.

Example profile:
```json
{
  "tenant_core": {
    "smtp_host": "mail.example.com",
    "smtp_port": 587,
    "session_timeout_minutes": 30
  },
  "settings_ai": {
    "provider": "anthropic",
    "model": "claude-sonnet-4-20250514"
  }
}
```

## Backup File Format

### Encrypted (default)
```json
{
  "encrypted": true,
  "kdf": "argon2id",
  "salt": "<base64>",
  "nonce": "<base64>",
  "data": "<base64 ciphertext>"
}
```

### Decrypted Structure
```json
{
  "_meta": {
    "format": "clovalink-backup",
    "format_version": 1,
    "clovalink_version": "0.1.6",
    "export_type": "tenant",
    "tenant_id": "uuid",
    "tenant_name": "Acme Corp",
    "exported_at": "2026-03-10T12:00:00Z",
    "exported_by": "user-uuid",
    "include_secrets": false,
    "sections": ["tenant_core", "users", "departments", "roles"]
  },
  "tenant_core": { ... },
  "users": [ ... ],
  "departments": [ ... ],
  "roles": [ ... ]
}
```

## Import Behavior

| Data Type | Match Strategy | Exists → | Missing → |
|-----------|---------------|----------|-----------|
| Users | Match by email | Update fields | Create with random password + forced reset |
| Departments | Match by name | Update | Create |
| Roles | Match by name | Replace permissions | Create |
| Settings | Direct merge | Overwrite | Set |
| SSO Providers | Match by name | Update config | Create |
| File Metadata | Match by storage_path | Update | Create (warns if file missing on disk) |

All imports run in a single database transaction. Any error rolls back everything.

## Disaster Recovery

If your ClovaLink instance crashes or the database is corrupted:

1. Files on pluggable storage (S3/local) survive but become orphaned
2. Restore from your latest backup to recover all metadata, users, and settings
3. File metadata import re-links files to their storage paths
4. Files whose `storage_path` no longer exists are flagged as warnings

**Recommendation**: Schedule regular exports and store encrypted backups off-site.
