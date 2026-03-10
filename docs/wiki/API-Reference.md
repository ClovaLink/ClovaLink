# API Reference

ClovaLink provides a REST API for all operations. All endpoints return JSON unless otherwise specified.

## Base URL

```
http://localhost:3000/api
```

## Authentication

Most endpoints require a JWT token in the Authorization header:

```
Authorization: Bearer <token>
```

### Getting a Token

```http
POST /api/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "password123"
}
```

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "name": "User Name",
    "role": "Admin",
    "tenant_id": "uuid"
  }
}
```

---

## Authentication Endpoints

### Login
```http
POST /api/auth/login
```
Authenticate a user and receive a JWT token.

**Request Body:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| email | string | Yes | User's email address |
| password | string | Yes | User's password |
| totp_code | string | No | 2FA code if enabled |

### Register (First User Only)
```http
POST /api/auth/register
```
Register the first SuperAdmin user. Only works when no users exist.

### Get Current User
```http
GET /api/auth/me
```
Returns the authenticated user's profile.

### Forgot Password
```http
POST /api/auth/forgot-password
```
Request a password reset email.

### Reset Password
```http
POST /api/auth/reset-password
```
Reset password using a reset token.

### Get Password Policy
```http
GET /api/auth/password-policy
```
Get the tenant's password requirements.

### Setup 2FA
```http
POST /api/auth/2fa/setup
```
Generate TOTP secret for two-factor authentication.

### Verify 2FA
```http
POST /api/auth/2fa/verify
```
Verify TOTP code and enable 2FA.

---

## User Endpoints

### List Users
```http
GET /api/users
```
List all users in the current tenant.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| page | number | Page number (default: 1) |
| per_page | number | Items per page (default: 20) |
| search | string | Search by name or email |
| role | string | Filter by role |
| status | string | Filter by status |

### Get User
```http
GET /api/users/{id}
```
Get a specific user by ID.

### Create User
```http
POST /api/users
```
Create a new user (Admin only).

**Request Body:**
```json
{
  "email": "new@example.com",
  "name": "New User",
  "password": "securePassword123!",
  "role": "Employee",
  "department_id": "uuid"
}
```

### Update User
```http
PUT /api/users/{id}
```
Update user information.

### Delete User
```http
DELETE /api/users/{id}
```
Soft delete a user.

### Suspend User
```http
POST /api/users/{id}/suspend
```
Temporarily suspend a user.

**Request Body:**
```json
{
  "reason": "Policy violation",
  "until": "2024-12-31T23:59:59Z"
}
```

### Unsuspend User
```http
POST /api/users/{id}/unsuspend
```
Remove suspension from a user.

### Reset User Password (Admin)
```http
POST /api/users/{id}/reset-password
```
Admin can reset a user's password.

### Send Password Reset Email
```http
POST /api/users/{id}/send-reset-email
```
Send password reset email to user.

### Permanently Delete User
```http
DELETE /api/users/{id}/permanent
```
Permanently delete a user and their data (GDPR).

---

## Profile Endpoints (Current User)

### Update My Profile
```http
PUT /api/users/me/profile
```
Update the authenticated user's profile.

### Change Password
```http
PUT /api/users/me/password
```
Change the authenticated user's password.

**Request Body:**
```json
{
  "current_password": "oldPassword",
  "new_password": "newPassword123!"
}
```

### Upload Avatar
```http
POST /api/users/me/avatar
Content-Type: multipart/form-data
```
Upload a profile picture.

### Export My Data
```http
GET /api/users/me/export
```
Export all personal data (GDPR).

### List My Sessions
```http
GET /api/users/me/sessions
```
List all active sessions for the current user.

### Revoke Session
```http
DELETE /api/users/me/sessions/{id}
```
Revoke a specific session.

---

## File Management Endpoints

### List Files
```http
GET /api/files/{company_id}
```
List files and folders.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| path | string | Directory path (default: root) |
| department_id | string | Filter by department |

### Upload File
```http
POST /api/upload/{company_id}
Content-Type: multipart/form-data
```
Upload one or more files.

**Form Fields:**
| Field | Type | Description |
|-------|------|-------------|
| file | file | File to upload (multiple allowed) |
| path | string | Destination path |
| department_id | string | Department UUID |

### Download File
```http
GET /api/download/{company_id}/{file_id}
```
Download a file.

### Create Folder
```http
POST /api/folders/{company_id}
```
Create a new folder.

**Request Body:**
```json
{
  "name": "New Folder",
  "path": "parent/path",
  "department_id": "uuid"
}
```

### Rename File/Folder
```http
POST /api/files/{company_id}/rename
```
Rename a file or folder.

### Delete File/Folder
```http
POST /api/files/{company_id}/delete
```
Move file/folder to trash.

### Move File
```http
PUT /api/files/{company_id}/{file_id}/move
```
Move a file to a different location.

### Lock File
```http
POST /api/files/{company_id}/{file_id}/lock
```
Lock a file to prevent modifications.

### Unlock File
```http
POST /api/files/{company_id}/{file_id}/unlock
```
Unlock a previously locked file.

### Create File Share
```http
POST /api/files/{company_id}/{file_id}/share
```
Create a shareable link for a file.

**Request Body:**
```json
{
  "is_public": false,
  "expires_at": "2024-12-31T23:59:59Z"
}
```

**Response:**
```json
{
  "token": "abc123xyz",
  "url": "http://localhost:8080/share/abc123xyz"
}
```

### Get File Activity
```http
GET /api/files/{company_id}/{file_id}/activity
```
Get audit log for a specific file.

### Export Files
```http
GET /api/files/{company_id}/export
```
Export file metadata as CSV/JSON.

---

## Trash Endpoints

### List Trash
```http
GET /api/trash/{company_id}
```
List deleted files awaiting permanent deletion.

### Restore File
```http
POST /api/trash/{company_id}/restore/{filename}
```
Restore a file from trash.

### Permanent Delete
```http
POST /api/trash/{company_id}/delete/{filename}
```
Permanently delete a file (cannot be undone).

---

## File Sharing (Public)

### Download Shared File
```http
GET /api/share/{token}
```
Download a file via share link.

### Get Share Info
```http
GET /api/share/{token}/info
```
Get metadata about a shared file.

---

## File Requests

### List File Requests
```http
GET /api/file-requests
```
List all file requests.

### Create File Request
```http
POST /api/file-requests
```
Create a new file request portal.

**Request Body:**
```json
{
  "name": "Q4 Documents",
  "destination_path": "/Finance/2024",
  "expires_at": "2024-12-31T23:59:59Z",
  "department_id": "uuid",
  "max_uploads": 10
}
```

### Get File Request
```http
GET /api/file-requests/{id}
```
Get a specific file request.

### Update File Request
```http
PUT /api/file-requests/{id}
```
Update file request settings.

### Delete File Request
```http
DELETE /api/file-requests/{id}
```
Delete a file request.

### List Uploads
```http
GET /api/file-requests/{id}/uploads
```
List files uploaded to a request.

### Public Upload
```http
POST /api/public-upload/{token}
Content-Type: multipart/form-data
```
Upload files to a request (no authentication required).

---

## Tenant/Company Endpoints

### List Tenants
```http
GET /api/tenants
```
List all tenants (SuperAdmin only).

### Create Tenant
```http
POST /api/tenants
```
Create a new tenant (SuperAdmin only).

### Get Tenant
```http
GET /api/tenants/{id}
```
Get tenant details.

### Update Tenant
```http
PUT /api/tenants/{id}
```
Update tenant settings.

### Delete Tenant
```http
DELETE /api/tenants/{id}
```
Delete a tenant and all its data.

### Suspend Tenant
```http
POST /api/tenants/{id}/suspend
```
Suspend a tenant (SuperAdmin only).

### Unsuspend Tenant
```http
POST /api/tenants/{id}/unsuspend
```
Reactivate a suspended tenant.

### Test SMTP
```http
POST /api/tenants/{id}/smtp/test
```
Test SMTP configuration.

### Switch Tenant
```http
POST /api/tenants/switch/{tenant_id}
```
Switch to a different tenant (for multi-tenant users).

### Accessible Tenants
```http
GET /api/tenants/accessible
```
List tenants the current user can access.

---

## Department Endpoints

### List Departments
```http
GET /api/departments
```
List all departments in the current tenant.

### Create Department
```http
POST /api/departments
```
Create a new department.

### Get Department
```http
GET /api/departments/{id}
```
Get department details.

### Update Department
```http
PUT /api/departments/{id}
```
Update department information.

### Delete Department
```http
DELETE /api/departments/{id}
```
Delete a department.

---

## Role Endpoints

### List Roles
```http
GET /api/roles
```
List all roles (system + custom).

### Create Role
```http
POST /api/roles
```
Create a custom role.

### Get Role
```http
GET /api/roles/{id}
```
Get role details with permissions.

### Update Role
```http
PUT /api/roles/{id}
```
Update a custom role.

### Delete Role
```http
DELETE /api/roles/{id}
```
Delete a custom role.

### Update Permissions
```http
PUT /api/roles/{id}/permissions
```
Update role permissions.

---

## Settings Endpoints

### Compliance Settings
```http
GET /api/settings/compliance
PUT /api/settings/compliance
```
Get/update compliance mode settings.

### Blocked Extensions
```http
GET /api/settings/blocked-extensions
PUT /api/settings/blocked-extensions
```
Get/update blocked file extensions.

### Password Policy
```http
GET /api/settings/password-policy
PUT /api/settings/password-policy
```
Get/update password policy.

### IP Restrictions
```http
GET /api/settings/ip-restrictions
PUT /api/settings/ip-restrictions
```
Get/update IP allowlist/blocklist.

---

## Global Settings (SuperAdmin)

### Get Global Settings
```http
GET /api/global-settings
```
Get all global settings.

### Update Global Settings
```http
PUT /api/global-settings
```
Update global settings.

### Upload Logo
```http
POST /api/global-settings/logo
Content-Type: multipart/form-data
```
Upload application logo.

### Upload Favicon
```http
POST /api/global-settings/favicon
Content-Type: multipart/form-data
```
Upload application favicon.

---

## Notification Endpoints

### List Notifications
```http
GET /api/notifications
```
List user's notifications.

### Get Unread Count
```http
GET /api/notifications/unread-count
```
Get count of unread notifications.

### Mark as Read
```http
PUT /api/notifications/{id}/read
```
Mark a notification as read.

### Mark All as Read
```http
PUT /api/notifications/read-all
```
Mark all notifications as read.

### Delete Notification
```http
DELETE /api/notifications/{id}
```
Delete a notification.

### Get Preferences
```http
GET /api/notifications/preferences
```
Get notification preferences.

### Update Preferences
```http
PUT /api/notifications/preferences
```
Update notification preferences.

---

## Security Endpoints

### List Security Alerts
```http
GET /api/security/alerts
```
List security alerts.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| page | number | Page number |
| per_page | number | Items per page |
| severity | string | Filter by severity |
| resolved | boolean | Filter resolved/unresolved |

### Get Alert Stats
```http
GET /api/security/alerts/stats
```
Get security alert statistics.

### Get Alert Badge
```http
GET /api/security/alerts/badge
```
Get count of unresolved critical/high alerts.

### Resolve Alert
```http
POST /api/security/alerts/{id}/resolve
```
Mark an alert as resolved.

### Dismiss Alert
```http
POST /api/security/alerts/{id}/dismiss
```
Dismiss an alert (acknowledge without action).

### Bulk Alert Action
```http
POST /api/security/alerts/bulk
```
Perform action on multiple alerts.

**Request Body:**
```json
{
  "action": "resolve",
  "alert_ids": ["uuid1", "uuid2"]
}
```

---

## Audit Log Endpoints

### List Activity Logs
```http
GET /api/activity-logs
```
List audit logs.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| page | number | Page number |
| per_page | number | Items per page |
| action | string | Filter by action type |
| user_id | string | Filter by user |
| resource_type | string | Filter by resource type |
| start_date | string | Start date filter |
| end_date | string | End date filter |

### Export Logs
```http
GET /api/activity-logs/export
```
Export audit logs as CSV.

### Get Action Types
```http
GET /api/activity-logs/actions
```
List available action types for filtering.

### Get Resource Types
```http
GET /api/activity-logs/resource-types
```
List available resource types for filtering.

### Audit Settings
```http
GET /api/audit-settings
PUT /api/audit-settings
```
Get/update audit configuration.

---

## GDPR/Compliance Endpoints

### Record Consent
```http
POST /api/compliance/consent
```
Record user consent for data processing.

### Get Consent Status
```http
GET /api/compliance/consent/user/{user_id}
```
Get consent status for a user.

### Revoke Consent
```http
DELETE /api/compliance/consent/revoke/{consent_type}
```
Revoke a specific consent.

### Create Deletion Request
```http
POST /api/gdpr/deletion-request
```
Request data deletion (GDPR right to be forgotten).

### List Deletion Requests
```http
GET /api/gdpr/deletion-requests
```
List pending deletion requests.

### Process Deletion Request
```http
POST /api/gdpr/deletion-requests/{id}/process
```
Process (approve/reject) a deletion request.

### Get Compliance Restrictions
```http
GET /api/compliance/restrictions
```
Get compliance-specific restrictions for current tenant.

---

## Dashboard Endpoints

### Get Dashboard Stats
```http
GET /api/dashboard/stats
```
Get dashboard statistics.

**Response:**
```json
{
  "total_files": 1234,
  "total_storage_bytes": 1073741824,
  "active_requests": 5,
  "total_users": 42,
  "recent_uploads": [...],
  "storage_by_department": [...]
}
```

### Get File Types
```http
GET /api/dashboard/file-types
```
Get file type distribution.

---

## Search Endpoint

### Global Search
```http
GET /api/search
```
Search files, users, and file requests.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| q | string | Search query |
| type | string | Filter by type (files, users, requests) |

---

## Health Endpoints

### Liveness Check
```http
GET /health
```
Basic health check (returns empty 200 OK).

### Readiness Check
```http
GET /health/ready
```
Check if database and Redis are connected.

### Detailed Health (Admin)
```http
GET /api/admin/health
```
Detailed health information including:
- Database connection and pool stats
- Redis connection and latency
- Storage backend status
- Memory usage
- Uptime

---

## Extension Endpoints

### Register Extension
```http
POST /api/extensions/register
```
Register a new extension.

### Install Extension
```http
POST /api/extensions/install/{extension_id}
```
Install an extension for the current tenant.

### List Extensions
```http
GET /api/extensions/list
```
List available extensions.

### List Installed Extensions
```http
GET /api/extensions/installed
```
List extensions installed for current tenant.

### Get UI Extensions
```http
GET /api/extensions/ui
```
Get UI extension components.

### Update Extension Settings
```http
PUT /api/extensions/{id}/settings
```
Update extension configuration.

### Trigger Automation
```http
POST /api/extensions/trigger/automation/{job_id}
```
Manually trigger an automation job.

---

## SSO Endpoints

### OIDC

#### Discover OIDC Configuration
```http
GET /api/auth/sso/oidc/discover
```
Returns the tenant's OIDC provider configuration (client ID, issuer, scopes). Used by the frontend to render SSO login buttons.

#### OIDC Authorize
```http
GET /api/auth/sso/oidc/authorize
```
Initiates the OIDC authorization code flow. Generates state and nonce parameters, stores them in the session, and redirects the user to the IdP's authorization endpoint.

#### OIDC Callback
```http
GET /api/auth/sso/oidc/callback
```
Handles the IdP callback after user authentication. Validates the authorization code, exchanges it for tokens, verifies the ID token (including nonce), and creates or resolves the local user via auto-provisioning. Returns a JWT session.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| code | string | Authorization code from IdP |
| state | string | Anti-CSRF state parameter |

### SAML 2.0

#### SAML Metadata
```http
GET /api/auth/sso/saml/metadata
```
Returns the SP (Service Provider) metadata XML document. Provide this URL to your IdP during SAML configuration.

#### SAML Authorize
```http
GET /api/auth/sso/saml/authorize
```
Initiates a SAML authentication request. Generates an AuthnRequest, stores the request ID for InResponseTo validation, and redirects to the IdP's SSO URL.

#### SAML ACS (Assertion Consumer Service)
```http
POST /api/auth/sso/saml/acs
Content-Type: application/x-www-form-urlencoded
```
Receives and processes the SAML Response from the IdP. Validates the XML signature, checks InResponseTo correlation, enforces audience restriction and time conditions, and creates or resolves the local user. Returns a JWT session.

**Form Fields:**
| Field | Type | Description |
|-------|------|-------------|
| SAMLResponse | string | Base64-encoded SAML Response |
| RelayState | string | Optional relay state for redirect |

### SSO Provider Management (SuperAdmin)

#### List SSO Providers
```http
GET /api/settings/sso/providers
```
List all configured SSO providers for the tenant.

#### Create SSO Provider
```http
POST /api/settings/sso/providers
```
Create a new OIDC or SAML provider configuration.

#### Update SSO Provider
```http
PUT /api/settings/sso/providers/{id}
```
Update an existing SSO provider.

#### Delete SSO Provider
```http
DELETE /api/settings/sso/providers/{id}
```
Delete an SSO provider. Removes the corresponding auth method from the tenant.

#### Toggle SSO Provider
```http
PUT /api/settings/sso/providers/{id}/toggle
```
Enable or disable an SSO provider (Admin or SuperAdmin).

### SSO Attribute Mappings

#### List Attribute Mappings
```http
GET /api/settings/sso/attribute-mappings
```
List attribute mappings for the tenant.

#### Create Attribute Mapping
```http
POST /api/settings/sso/attribute-mappings
```
Create a new attribute mapping (e.g., map IdP `groups` claim to ClovaLink role).

#### Update Attribute Mapping
```http
PUT /api/settings/sso/attribute-mappings/{id}
```
Update an attribute mapping.

#### Delete Attribute Mapping
```http
DELETE /api/settings/sso/attribute-mappings/{id}
```
Delete an attribute mapping.

---

## Document Approval Endpoints

### List Pending Approvals
```http
GET /api/approvals/pending
```
List documents awaiting the current user's approval.

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| page | number | Page number (default: 1) |
| per_page | number | Items per page (default: 20) |
| policy_id | string | Filter by approval policy |

### List Approval History
```http
GET /api/approvals/history
```
List past approval decisions (approved, rejected) visible to the current user.

### Approve Document
```http
POST /api/approvals/{id}/approve
```
Approve a pending document.

**Request Body:**
```json
{
  "comment": "Looks good, approved for distribution."
}
```

### Reject Document
```http
POST /api/approvals/{id}/reject
```
Reject a pending document with a reason.

**Request Body:**
```json
{
  "reason": "Missing required signatures on page 3."
}
```

### Resubmit Document
```http
POST /api/approvals/{id}/resubmit
```
Resubmit a previously rejected document for approval. Only the document owner can resubmit.

### Send for Approval
```http
POST /api/approvals/send
```
Manually submit a document for approval under a specific policy.

**Request Body:**
```json
{
  "file_id": "uuid",
  "policy_id": "uuid",
  "message": "Please review the updated Q4 report."
}
```

### List Approval Policies
```http
GET /api/approvals/policies
```
List all approval policies for the tenant.

### Create Approval Policy
```http
POST /api/approvals/policies
```
Create a new approval policy (Admin only).

**Request Body:**
```json
{
  "name": "Finance Review",
  "approvers": ["user-uuid-1", "user-uuid-2"],
  "conditions": {
    "department_ids": ["uuid"],
    "file_types": ["pdf", "xlsx"]
  },
  "notify_on_submit": true
}
```

### Update Approval Policy
```http
PUT /api/approvals/policies/{id}
```
Update an existing approval policy.

### Delete Approval Policy
```http
DELETE /api/approvals/policies/{id}
```
Delete an approval policy.

---

## Error Responses

All endpoints return errors in a consistent format:

```json
{
  "error": "Error message description",
  "code": "ERROR_CODE"
}
```

### Common HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 400 | Bad Request - Invalid input |
| 401 | Unauthorized - Invalid or missing token |
| 403 | Forbidden - Insufficient permissions |
| 404 | Not Found |
| 409 | Conflict - Resource already exists |
| 413 | Payload Too Large - File too big |
| 422 | Unprocessable Entity - Validation error |
| 429 | Too Many Requests - Rate limited |
| 500 | Internal Server Error |

---

## Rate Limiting

API requests are rate limited per IP address:
- **Default**: 100 requests/second with burst of 200
- **File uploads**: Subject to additional size limits

Rate limit headers are included in responses:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1703174400
```

