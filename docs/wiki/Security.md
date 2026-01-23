# Security

This document covers ClovaLink's security features, configuration options, and best practices.

<p align="center">
<img src="../assets/screenshots/Security.png" alt="Security Alerts Dashboard" width="80%">
<br><em>Security Alerts Dashboard — Real-time monitoring for suspicious activity</em>
</p>

## Overview

ClovaLink implements multiple layers of security:

```
┌─────────────────────────────────────────────────────────────┐
│                    Security Layers                           │
├─────────────────────────────────────────────────────────────┤
│  Network Layer                                               │
│    - HTTPS/TLS encryption                                   │
│    - Rate limiting                                          │
│    - IP restrictions                                        │
├─────────────────────────────────────────────────────────────┤
│  Authentication Layer                                        │
│    - JWT tokens                                             │
│    - Session fingerprinting                                 │
│    - Multi-factor authentication                            │
├─────────────────────────────────────────────────────────────┤
│  Authorization Layer                                         │
│    - Role-based access control                              │
│    - Tenant isolation                                       │
│    - Department boundaries                                  │
├─────────────────────────────────────────────────────────────┤
│  Data Layer                                                  │
│    - Password hashing (Argon2id)                            │
│    - Encrypted file storage                                 │
│    - Audit logging                                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Password Security

### Password Policies

Each tenant can configure their own password requirements:

| Setting | Description | Default |
|---------|-------------|---------|
| `min_length` | Minimum password length | 8 |
| `require_uppercase` | At least one uppercase letter | true |
| `require_lowercase` | At least one lowercase letter | true |
| `require_number` | At least one digit | true |
| `require_special` | At least one special character | false |
| `max_age_days` | Password expiration (null = never) | null |
| `prevent_reuse` | Number of previous passwords to block | 0 |

### Configuring Password Policy

#### Via UI
1. Go to **Settings** → **Security** → **Password Policy**
2. Configure requirements
3. Save changes

#### Via API
```bash
PUT /api/settings/password-policy
{
  "min_length": 12,
  "require_uppercase": true,
  "require_lowercase": true,
  "require_number": true,
  "require_special": true,
  "max_age_days": 90,
  "prevent_reuse": 5
}
```

### Password Hashing

ClovaLink uses Argon2id for password hashing with the following parameters:

```
Algorithm: Argon2id (v0x13)
Memory:    64 MB (65536 KB)
Iterations: 3
Parallelism: 4 lanes
```

Why Argon2id:
- **Memory-hard**: Resistant to GPU/ASIC attacks
- **Time-hard**: Configurable iteration count
- **Side-channel resistant**: Combines Argon2i benefits
- **OWASP recommended**: Industry best practice

---

## Session Management

### JWT Tokens

Authentication uses JSON Web Tokens:

```json
{
  "sub": "user-uuid",
  "tenant_id": "tenant-uuid",
  "role": "Admin",
  "fingerprint": "sha256-hash",
  "exp": 1703260800,
  "iss": "clovalink",
  "aud": "clovalink-api"
}
```

| Claim | Purpose |
|-------|---------|
| `sub` | User ID |
| `tenant_id` | Current tenant context |
| `role` | User's role |
| `fingerprint` | Session fingerprint hash |
| `exp` | Expiration timestamp |
| `iss` | Issuer ("clovalink") |
| `aud` | Audience ("clovalink-api") |

### Session Fingerprinting

Every session is fingerprinted to detect potential token theft:

```
Fingerprint = SHA256(
    User-Agent +
    Accept-Language +
    IP-Prefix (first 3 octets)
)
```

On each request:
1. Extract fingerprint from JWT claims
2. Generate fingerprint from current request
3. Compare fingerprints
4. Log warning if mismatch (allows for mobile/NAT scenarios)

### Session Timeout

Configurable per-tenant:

| Compliance Mode | Default Timeout |
|-----------------|-----------------|
| Standard | 30 minutes (inactivity) |
| HIPAA | 15 minutes |
| SOX | 30 minutes |
| GDPR | 30 minutes |

### Managing Sessions

Users can view and revoke their sessions:

1. Go to **Profile** → **Sessions**
2. View all active sessions with:
   - Device info
   - IP address
   - Last active time
3. Click **Revoke** to invalidate a session

Admins can revoke all sessions for a user:
1. Go to **Users** → Select user
2. Click **Revoke All Sessions**

---

## Multi-Factor Authentication

### TOTP Setup

ClovaLink supports TOTP-based 2FA (compatible with Google Authenticator, Authy, etc.):

1. User goes to **Profile** → **Security**
2. Click **Enable 2FA**
3. Scan QR code with authenticator app
4. Enter verification code to confirm

### Enforcing MFA

Admins can require MFA for all users:

1. Go to **Settings** → **Security**
2. Enable **Require MFA**
3. Users must set up 2FA on next login

Compliance modes HIPAA and SOX automatically enforce MFA.

### Recovery

If a user loses their 2FA device:
1. Admin goes to **Users** → Select user
2. Click **Reset 2FA**
3. User can set up new 2FA device

---

## Role-Based Access Control

### Standard Roles

| Role | Description |
|------|-------------|
| **SuperAdmin** | Platform-wide access across all tenants |
| **Admin** | Full access within their tenant |
| **Manager** | Department oversight and file management |
| **Employee** | Basic file access within their scope |

### Permission Matrix - Page Access

| Feature | SuperAdmin | Admin | Manager | Employee |
|---------|:----------:|:-----:|:-------:|:--------:|
| Dashboard | ✅ | ✅ | ❌ | ❌ |
| Companies | ✅ | ❌ | ❌ | ❌ |
| Users | ✅ | ✅ | ❌ | ❌ |
| Files | ✅ | ✅ | ✅ | ✅ |
| File Requests | ✅ | ✅ | ✅ | ✅ |
| Settings | ✅ | ✅ | ❌ | ❌ |
| Roles | ✅ | ✅ | ❌ | ❌ |
| Audit Logs | ✅ | ✅ | ❌ | ❌ |
| Security | ✅ | ✅ | ❌ | ❌ |

### Permission Matrix - File Operations

| Operation | SuperAdmin | Admin | Manager | Employee |
|-----------|:----------:|:-----:|:-------:|:--------:|
| View all files | ✅ | ✅ | Own dept | Own dept |
| View private files | ✅ | ✅ | Own only | Own only |
| View locked files | ✅ | ✅ | ✅ | Authorized only |
| Upload files | ✅ | ✅ | ✅ | ✅ |
| Download files | ✅ | ✅ | Accessible | Accessible |
| Delete files | ✅ | ✅ | Own files | Own files |
| Lock/Unlock files | ✅ | ✅ | ✅ | ❌ |
| Share files | ✅ | ✅ | ✅ | Own files |
| Create folders | ✅ | ✅ | ✅ | ✅ |

### Permission Matrix - Search Results

| Search Type | SuperAdmin | Admin | Manager | Employee |
|-------------|:----------:|:-----:|:-------:|:--------:|
| Companies | ✅ | ❌ | ❌ | ❌ |
| Users | ✅ | ✅ | ❌ | ❌ |
| Files | All | All tenant | Accessible | Accessible |

### Custom Roles

Custom roles inherit permissions from a **base role** (Manager, Employee) and can have additional permissions granted:

```
files.lock      - Can lock files
files.unlock    - Can unlock files
users.view      - Can view user list
users.manage    - Can create/edit users
audit.view      - Can view audit logs
settings.view   - Can view settings
```

The system looks up a custom role's `base_role` to determine baseline permissions, then applies any additional granted permissions.

---

## File Access Control

### Visibility Levels

| Visibility | Who Can Access |
|------------|----------------|
| **department** | Users in the same department or with department in `allowed_department_ids` |
| **private** | Only the file owner |

### File Locking

Locked files have restricted access. Only these users can access a locked file:

1. **Locker**: User who locked the file
2. **Owner**: File owner always has access
3. **Role requirement**: Optional role restriction (e.g., "Manager" required)
4. **Password protection**: Optional password for unlock

Non-authorized users cannot:
- Preview locked files
- Download locked files
- Share locked files
- View locked files in search results

### Department Access

Files inherit department from their parent folder. Users can access files if:

1. File has no department (root-level)
2. File is in user's primary department
3. File is in user's `allowed_department_ids`
4. User owns the file

### File Locking via UI

1. Right-click file → **Lock**
2. Optionally set:
   - Password requirement
   - Role requirement
3. Click **Lock File**

To unlock: Right-click → **Unlock** (requires authorization)

---

## IP Restrictions

### Configuration

Per-tenant IP restrictions with allowlist/blocklist:

#### Modes

| Mode | Behavior |
|------|----------|
| `disabled` | No IP restrictions |
| `allowlist_only` | Only allow listed IPs |
| `blocklist_only` | Block listed IPs |
| `both` | Allow allowlist, block blocklist |

#### CIDR Support

Both lists support CIDR notation:
- Single IP: `192.168.1.100`
- Subnet: `192.168.1.0/24`
- Range: `10.0.0.0/8`

### Configuring IP Restrictions

#### Via UI
1. Go to **Settings** → **Security** → **IP Restrictions**
2. Select mode
3. Add IPs/CIDR ranges to appropriate list
4. Save

#### Via API
```bash
PUT /api/settings/ip-restrictions
{
  "mode": "both",
  "allowlist": [
    "192.168.1.0/24",
    "10.0.0.100"
  ],
  "blocklist": [
    "1.2.3.4"
  ]
}
```

### Behavior

When a request is blocked:
1. Request returns 403 Forbidden
2. Security alert is created
3. Event is logged in audit log

---

## Security Alerts

### Alert Types

| Type | Severity | Trigger |
|------|----------|---------|
| `failed_login_spike` | High | >5 failed logins in 5 minutes |
| `new_ip_login` | Medium | Login from new IP address |
| `permission_escalation` | High | Role upgraded to Admin/SuperAdmin |
| `suspended_access_attempt` | Medium | Suspended user tried to access |
| `bulk_download` | Medium | >50 files downloaded in 1 hour |
| `blocked_extension_attempt` | Low | Upload of blocked file type |
| `excessive_sharing` | Medium | >10 shares created in 1 hour |
| `account_lockout` | High | Account locked after failed attempts |
| `ip_blocked` | Medium | Request from blocked IP |

### Viewing Alerts

1. Navigate to **Security** in sidebar
2. View alerts by severity and status
3. Filter by:
   - Severity (Critical, High, Medium, Low)
   - Status (Resolved, Unresolved)
   - Date range
   - User

### Managing Alerts

#### Resolve
Mark an alert as handled:
1. Click on alert
2. Click **Resolve**
3. Add resolution notes (optional)

#### Dismiss
Acknowledge without action:
1. Click on alert
2. Click **Dismiss**

#### Bulk Actions
Select multiple alerts:
1. Check boxes on alerts
2. Click **Bulk Actions**
3. Choose Resolve All or Dismiss All

### Alert Notifications

Critical and High severity alerts trigger:
1. In-app notification to admins
2. Email notification (if configured)

Configure in **Settings** → **Notifications**

---

## Audit Logging

### What's Logged

All security-relevant actions are recorded:

| Category | Events |
|----------|--------|
| **Authentication** | Login, logout, failed login, 2FA setup |
| **User Management** | Create, update, delete, suspend |
| **File Operations** | Upload, download, delete, share |
| **Settings Changes** | Any configuration modification |
| **Role Changes** | Permission grants/revokes |
| **Compliance** | Consent changes, deletion requests |

### Audit Log Entry

```json
{
  "id": "uuid",
  "tenant_id": "uuid",
  "user_id": "uuid",
  "action": "file.download",
  "resource_type": "file",
  "resource_id": "uuid",
  "ip_address": "192.168.1.100",
  "metadata": {
    "file_name": "document.pdf",
    "file_size": 1048576
  },
  "created_at": "2024-12-20T15:30:00Z"
}
```

### Retention

Configure retention in **Settings** → **Audit**:
- Default: 90 days
- HIPAA/SOX compliance: Recommend 7+ years
- Logs can be exported before deletion

### Exporting Logs

1. Go to **Audit Logs**
2. Apply filters (date range, user, action)
3. Click **Export**
4. Choose CSV or JSON format

For compliance, schedule regular exports to external storage.

---

## Data Protection

### File Encryption

#### Files at Rest

**S3 Storage:**
- Server-side encryption (SSE-S3 or SSE-KMS)
- Managed by your S3 provider

**Local Storage (v0.1.3+):**
- Optional ChaCha20-Poly1305 authenticated encryption
- Enable by setting `ENCRYPTION_KEY` environment variable
- Each file uses a unique random 12-byte nonce
- AEAD provides both confidentiality and integrity

```bash
# Generate a secure 32-byte encryption key
openssl rand -base64 32

# Set in your .env or environment
ENCRYPTION_KEY=K7gNU3sdo+OL0wNhqoVWhr3g6s1xYv72ol/pe/Unols=
```

**Key Management:**
- Store encryption keys securely (never commit to git)
- Use secrets managers in production (AWS Secrets Manager, HashiCorp Vault, etc.)
- Key loss = data loss (no recovery without the key)
- Key rotation requires re-encrypting all files (not currently automated)

**Backwards Compatibility:**
- When encryption is enabled, existing unencrypted files remain readable
- New uploads are encrypted, old files are served as-is
- No migration required when enabling encryption

#### Files in Transit

- HTTPS/TLS for all API communication
- Presigned URLs include expiration

### Tenant Isolation

Each tenant's data is completely isolated:

| Layer | Isolation Method |
|-------|------------------|
| Database | Row-level filtering by `tenant_id` |
| Storage | Prefixed paths: `{tenant_id}/path/file` |
| Cache | Namespaced keys: `tenant:{id}:key` |
| Sessions | JWT contains `tenant_id` claim |

Cross-tenant access is impossible through normal API usage.

### Content Deduplication

Files are deduplicated using Blake3 hashing:
- **Per-department** deduplication only
- Same file in different departments = separate storage
- Maintains tenant/department isolation

### Content-Addressed Storage

Files are stored using Blake3 content hashes:

```
uploads/{tenant_id}/{department_id}/{content_hash}
```

Benefits:
- **Deduplication**: Identical files stored once per department
- **Integrity verification**: Hash mismatch indicates corruption
- **Immutable references**: Renaming/moving files doesn't touch storage

### Presigned URLs

For S3-compatible storage, downloads can use presigned URLs:

```bash
USE_PRESIGNED_URLS=true
PRESIGNED_URL_EXPIRY_SECS=3600  # 1 hour
```

Benefits:
- Bypasses application server for large files
- Reduces bandwidth costs
- Maintains security via time-limited signed URLs

### Streaming Downloads (Zero-Copy)

When presigned URLs are unavailable (local storage or fallback), downloads use zero-copy streaming:

| File Size | Memory Usage |
|-----------|--------------|
| 10 MB | ~64 KB buffer |
| 100 MB | ~64 KB buffer |
| 1 GB | ~64 KB buffer |

Benefits:
- Constant memory usage regardless of file size
- No OOM risk from large file downloads
- Multiple concurrent downloads without memory pressure
- Files are never fully loaded into RAM

---

## Rate Limiting

### Default Limits

| Limit | Value |
|-------|-------|
| Requests per second | 100 |
| Burst allowance | 200 |
| Concurrent requests | 1000 |

### Rate Limit Headers

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1703174400
```

### Exceeded Rate Limit

Response:
```
HTTP 429 Too Many Requests
```

The client should:
1. Wait until reset time
2. Implement exponential backoff
3. Cache responses where possible

---

## API Security

### CORS Configuration

Configure allowed origins for cross-origin requests:

```bash
# Production: Explicit allowed origins
CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com

# Development: Enable localhost origins
CORS_DEV_MODE=true
```

Security measures:
- **Strict origin validation**: Only configured origins allowed
- **Credentials support**: Cookies and auth headers permitted from allowed origins
- **Limited methods**: Only GET, POST, PUT, DELETE, PATCH, OPTIONS
- **Limited headers**: Only Content-Type, Authorization, X-Requested-With

### Redis Rate Limiting

Atomic Redis-based rate limiting prevents abuse:

```bash
# Configuration
PER_IP_REQUESTS_PER_SEC=100    # Max requests per second
PER_IP_BURST_SIZE=200          # Burst allowance
```

- **Atomic operations**: Uses Redis INCR + EXPIRE to prevent race conditions
- **Per-IP tracking**: Limits applied per client IP
- **Trusted proxy support**: Configure `TRUSTED_PROXY_IPS` for load balancer scenarios

### Request Security

Built-in protections against common attacks:

| Attack | Protection |
|--------|------------|
| **Header injection** | Content-Disposition sanitization |
| **Path traversal** | Input validation on file paths |
| **Zip-slip** | Zip downloads sanitize paths |
| **DoS via large files** | Configurable max upload size |
| **Memory exhaustion** | Streaming downloads (constant memory) |

---

## Blocked File Extensions

### Default Blocked Types

The following extensions are blocked by default in compliance modes:

```
exe, bat, cmd, sh, ps1, msi, dll, vbs, 
js, jar, py, rb, php, asp, jsp
```

### Configuring Blocked Extensions

1. Go to **Settings** → **Security**
2. Add/remove extensions (without the dot)
3. Save changes

```bash
PUT /api/settings/blocked-extensions
{
  "extensions": ["exe", "bat", "sh", "dll"]
}
```

### Bypass for Admins

Admins cannot bypass blocked extensions. The blocks apply universally to prevent accidental policy violations.

---

## Virus Scanning

ClovaLink integrates with **ClamAV** to automatically scan all uploaded files for viruses, malware, trojans, and other threats.

### How It Works

1. User uploads a file
2. File is stored with `scan_status: pending`
3. Background worker sends file to ClamAV daemon
4. Based on result:
   - **Clean**: File is available for download
   - **Infected**: File is quarantined, deleted, or flagged

### Actions on Detection

| Action | Behavior |
|--------|----------|
| `quarantine` | Move to quarantine, admin can restore (default) |
| `delete` | Permanently delete the file |
| `flag` | Mark as infected but keep accessible |

### Configuration

Enable/disable and configure per-tenant:

```bash
PUT /api/settings/virus-scan
{
  "enabled": true,
  "action_on_detect": "quarantine",
  "notify_admin": true,
  "notify_uploader": true,
  "auto_suspend_uploader": true,
  "suspend_threshold": 3
}
```

### Security Alerts

Malware detection triggers alerts:

| Type | Severity | Trigger |
|------|----------|---------|
| `malware_detected` | High | File contains malware |
| `user_auto_suspended` | High | User suspended for repeat offenses |

### Auto-Suspend

Automatically suspend users who repeatedly upload malware:
- Enable `auto_suspend_uploader`
- Set `suspend_threshold` (default: 1)
- User is suspended after reaching threshold
- Admin notified and must manually unsuspend

### Quarantine Management

Admins can manage quarantined files:
1. Go to **Security** → **Quarantine**
2. Review flagged files
3. **Release** if false positive, or **Delete Permanently**

> **📖 Full Documentation**: See [Virus-Scanning](Virus-Scanning.md) for complete configuration, troubleshooting, and API reference.

---

## Compliance Modes

### Standard Mode

No restrictions enforced. Suitable for:
- Internal team usage
- Non-regulated industries
- Development environments

### HIPAA Mode

Healthcare industry compliance:
- ✅ MFA required
- ✅ Public sharing disabled
- ✅ 15-minute session timeout
- ✅ Strong password policy
- ✅ Full audit logging
- ✅ IP restrictions available

### SOX Mode

Financial/accounting compliance:
- ✅ MFA required
- ✅ Public sharing disabled
- ✅ File versioning enabled
- ✅ No permanent deletion
- ✅ Complete audit trail
- ✅ Change tracking on all records

### GDPR Mode

European data protection:
- ✅ Data export enabled
- ✅ Consent tracking
- ✅ Deletion request workflow
- ✅ Data portability
- ✅ Privacy-first defaults

---

## Production Security Checklist

Before deploying to production, ensure:

### Required Configuration

```bash
# Strong JWT secret (64+ random characters)
JWT_SECRET=$(openssl rand -base64 64)

# Database with SSL
DATABASE_URL=postgres://user:pass@host/db?sslmode=require

# Redis with password
REDIS_URL=redis://:password@host:6379

# Explicit allowed origins (no wildcards)
CORS_ALLOWED_ORIGINS=https://your-domain.com

# Disable dev mode
CORS_DEV_MODE=false
ENVIRONMENT=production
```

### Deployment Checklist

- [ ] Set strong `JWT_SECRET` (64+ random characters)
- [ ] Configure explicit `CORS_ALLOWED_ORIGINS`
- [ ] Enable TLS termination (Nginx/Caddy)
- [ ] Use managed PostgreSQL with encryption
- [ ] Enable S3 server-side encryption
- [ ] Configure rate limiting
- [ ] Set up log aggregation for audit logs
- [ ] Regular security updates for containers
- [ ] Configure backup schedule
- [ ] Test disaster recovery procedure
- [ ] Enable virus scanning (ClamAV)
- [ ] Review and set compliance mode

### Optional Security Enhancements

```bash
# Trusted proxy for load balancer
TRUSTED_PROXY_IPS=10.0.0.0/8

# Rate limiting
PER_IP_REQUESTS_PER_SEC=100
PER_IP_BURST_SIZE=200

# Key rotation (zero-downtime)
JWT_SECRET_SECONDARY=<old-secret-during-rotation>
```

---

## Security Best Practices

### For Administrators

1. **Use strong JWT secret**: 32+ random characters
2. **Enable HTTPS**: Always use TLS in production
3. **Configure IP restrictions**: Limit access to known networks
4. **Enforce MFA**: Especially for admin accounts
5. **Review audit logs**: Regularly check for anomalies
6. **Update regularly**: Keep ClovaLink updated for security patches
7. **Backup data**: Regular backups with encryption

### For Users

1. **Use unique passwords**: Different password for each service
2. **Enable 2FA**: Protect your account with TOTP
3. **Verify share links**: Check expiration and permissions
4. **Report suspicious activity**: Contact admin if anything seems wrong
5. **Log out on shared devices**: Don't stay logged in on public computers

### For Developers

1. **Never commit secrets**: Use environment variables
2. **Validate all input**: Server-side validation is mandatory
3. **Use prepared statements**: Prevent SQL injection
4. **Implement CSRF protection**: Token-based protection
5. **Rate limit webhooks**: Prevent abuse
6. **Verify signatures**: Always verify webhook signatures

---

## Incident Response

### When a Security Alert is Triggered

1. **Assess**: Review the alert details and severity
2. **Investigate**: Check related audit logs and user activity
3. **Contain**: Suspend user/revoke sessions if necessary
4. **Remediate**: Reset passwords, rotate keys, update policies
5. **Document**: Record incident details and response
6. **Review**: Adjust policies to prevent recurrence

### Suspected Breach Checklist

- [ ] Revoke all suspicious sessions
- [ ] Reset affected user passwords
- [ ] Review audit logs for scope
- [ ] Check for unauthorized data access
- [ ] Rotate API keys and JWT secret
- [ ] Notify affected users (if required)
- [ ] Document timeline and actions
- [ ] Report to authorities (if required by regulation)

---

## Security Headers

The frontend Nginx configuration includes security headers:

```nginx
# Prevent clickjacking
add_header X-Frame-Options "SAMEORIGIN" always;

# Prevent MIME type sniffing
add_header X-Content-Type-Options "nosniff" always;

# Enable XSS protection
add_header X-XSS-Protection "1; mode=block" always;

# Referrer policy
add_header Referrer-Policy "strict-origin-when-cross-origin" always;

# Content Security Policy
add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';" always;
```

---

## Vulnerability Reporting

If you discover a security vulnerability:

1. **Do not** create a public GitHub issue
2. **Email** security@clovalink.org with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)
3. We will respond within 48 hours
4. Coordinated disclosure after patch is released

