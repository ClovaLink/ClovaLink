use serde::{Deserialize, Deserializer, Serialize};
use sqlx::types::{
    chrono::{DateTime, Utc},
    Uuid,
};

/// Deserialize an optional UUID that might be an empty string
/// Empty strings are treated as None
fn deserialize_optional_uuid<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Uuid::parse_str(&s)
            .map(Some)
            .map_err(|e| D::Error::custom(format!("Invalid UUID: {}", e))),
    }
}

// ==================== Tenant/Company ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub domain: String,
    pub plan: String,
    pub status: String,
    pub compliance_mode: String,
    pub encryption_standard: String,
    pub retention_policy_days: i32,
    pub storage_quota_bytes: Option<i64>,
    pub storage_used_bytes: i64,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub smtp_secure: Option<bool>,
    pub enable_totp: Option<bool>,
    pub enable_passkeys: Option<bool>,
    // Compliance enforcement fields
    pub mfa_required: Option<bool>,
    pub session_timeout_minutes: Option<i32>,
    pub public_sharing_enabled: Option<bool>,
    pub data_export_enabled: Option<bool>,
    // Upload limits
    pub max_upload_size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantInput {
    pub name: String,
    pub domain: String,
    pub plan: String,
    pub storage_quota_bytes: Option<i64>,
    pub departments: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantInput {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub plan: Option<String>,
    pub status: Option<String>,
    pub compliance_mode: Option<String>,
    pub storage_quota_bytes: Option<i64>,
    pub retention_policy_days: Option<i32>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub smtp_secure: Option<bool>,
    pub enable_totp: Option<bool>,
    pub enable_passkeys: Option<bool>,
    // Compliance enforcement fields
    pub mfa_required: Option<bool>,
    pub session_timeout_minutes: Option<i32>,
    pub public_sharing_enabled: Option<bool>,
    pub data_export_enabled: Option<bool>,
    // Upload limits
    pub max_upload_size_bytes: Option<i64>,
}

// ==================== Department ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Department {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDepartmentInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDepartmentInput {
    pub name: Option<String>,
    pub description: Option<String>,
}

// ==================== User ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub department_id: Option<Uuid>,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub status: String,
    pub avatar_url: Option<String>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub dashboard_layout: Option<serde_json::Value>,
    pub widget_config: Option<serde_json::Value>,
    pub allowed_tenant_ids: Option<Vec<Uuid>>,
    pub allowed_department_ids: Option<Vec<Uuid>>,
    pub custom_role_id: Option<Uuid>, // Reference to custom role if assigned
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    #[serde(skip_serializing)]
    pub recovery_token: Option<String>,
    #[serde(skip_serializing)]
    pub recovery_token_expires_at: Option<DateTime<Utc>>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_until: Option<DateTime<Utc>>,
    pub suspension_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserInput {
    pub email: String,
    pub name: String,
    pub password: String,
    pub role: String,
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub department_id: Option<Uuid>,
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub tenant_id: Option<Uuid>,
    pub allowed_tenant_ids: Option<Vec<Uuid>>,
    pub allowed_department_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserInput {
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub department_id: Option<Uuid>,
    pub dashboard_layout: Option<serde_json::Value>,
    pub widget_config: Option<serde_json::Value>,
    pub allowed_tenant_ids: Option<Vec<Uuid>>,
    pub allowed_department_ids: Option<Vec<Uuid>>,
    pub custom_role_id: Option<Uuid>,
    /// Password confirmation required when changing user role (SuperAdmin only)
    pub confirm_password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SuspendUserInput {
    pub until: Option<DateTime<Utc>>, // None = indefinite suspension
    pub reason: Option<String>,
}

// ==================== File Request ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileRequest {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub department_id: Option<Uuid>,
    pub name: String,
    pub destination_path: String,
    pub token: String,
    pub created_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub status: String,
    pub upload_count: i32,
    pub max_uploads: Option<i32>,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFileRequestInput {
    pub name: String,
    pub destination_path: String,
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub department_id: Option<Uuid>,
    pub expires_in_days: i64,
    #[serde(default)]
    pub max_uploads: Option<i32>,
    #[serde(default)]
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileRequestUpload {
    pub id: Uuid,
    pub file_request_id: Uuid,
    pub file_metadata_id: Option<Uuid>,
    pub filename: String,
    pub original_filename: String,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    pub storage_path: String,
    pub uploaded_by_email: Option<String>,
    pub uploaded_at: DateTime<Utc>,
}

// ==================== File Metadata ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileMetadata {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub department_id: Option<Uuid>,
    pub name: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    pub is_directory: bool,
    pub owner_id: Option<Uuid>,
    pub parent_path: Option<String>,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    // SOX compliance versioning fields
    pub version: Option<i32>,
    pub version_parent_id: Option<Uuid>,
    pub is_immutable: Option<bool>,
    // File locking fields
    pub is_locked: bool,
    pub locked_by: Option<Uuid>,
    pub locked_at: Option<DateTime<Utc>>,
    pub lock_password_hash: Option<String>,
    pub lock_requires_role: Option<String>,
    // Visibility: 'department' (shared) or 'private' (owner-only)
    pub visibility: String,
    // Company folder: hide owner avatar and show building icon
    pub is_company_folder: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==================== User Preferences ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserPreferences {
    pub id: Uuid,
    pub user_id: Uuid,
    pub starred_files: Vec<String>,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==================== Audit Log ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub ip_address: Option<std::net::IpAddr>,
    pub created_at: DateTime<Utc>,
}

// ==================== Roles ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Role {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>, // NULL = global role
    pub name: String,
    pub description: Option<String>,
    pub base_role: String, // 'Employee', 'Manager', 'Admin', 'SuperAdmin'
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleInput {
    pub name: String,
    pub description: Option<String>,
    pub base_role: String,
    pub permissions: Option<Vec<String>>, // Additional permissions to grant
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub base_role: Option<String>,
}

// ==================== Role Permissions ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RolePermission {
    pub id: Uuid,
    pub role_id: Uuid,
    pub permission: String,
    pub granted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRolePermissionsInput {
    pub permissions: Vec<PermissionUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionUpdate {
    pub permission: String,
    pub granted: bool,
}

// ==================== Audit Settings ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditSettings {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub log_logins: bool,
    pub log_file_operations: bool,
    pub log_user_changes: bool,
    pub log_settings_changes: bool,
    pub log_role_changes: bool,
    pub retention_days: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAuditSettingsInput {
    pub log_logins: Option<bool>,
    pub log_file_operations: Option<bool>,
    pub log_user_changes: Option<bool>,
    pub log_settings_changes: Option<bool>,
    pub log_role_changes: Option<bool>,
    pub retention_days: Option<i32>,
}

// ==================== Permission Constants ====================

/// All available permissions in the system
pub const ALL_PERMISSIONS: &[&str] = &[
    // File permissions
    "files.view",
    "files.upload",
    "files.download",
    "files.delete",
    "files.share",
    "files.lock",
    "files.export",
    // Request permissions
    "requests.create",
    "requests.view",
    // User permissions
    "users.view",
    "users.invite",
    "users.edit",
    "users.delete",
    "users.suspend",
    // Role permissions
    "roles.view",
    "roles.manage",
    // Audit permissions
    "audit.view",
    "audit.export",
    // Settings permissions
    "settings.view",
    "settings.edit",
    // Tenant permissions
    "tenants.manage",
];

/// Get base permissions for a role level
pub fn get_base_permissions(base_role: &str) -> Vec<&'static str> {
    match base_role {
        "Employee" => vec!["files.view", "files.upload", "files.download"],
        "Manager" => vec![
            "files.view",
            "files.upload",
            "files.download",
            "files.delete",
            "files.share",
            "files.lock",
            "requests.create",
            "requests.view",
        ],
        "Admin" => vec![
            "files.view",
            "files.upload",
            "files.download",
            "files.delete",
            "files.share",
            "files.lock",
            "files.export",
            "requests.create",
            "requests.view",
            "users.view",
            "users.invite",
            "users.edit",
            "users.suspend",
            "roles.view",
            "audit.view",
            "settings.view",
        ],
        "SuperAdmin" => vec![
            "files.view",
            "files.upload",
            "files.download",
            "files.delete",
            "files.share",
            "files.lock",
            "files.export",
            "requests.create",
            "requests.view",
            "users.view",
            "users.invite",
            "users.edit",
            "users.delete",
            "users.suspend",
            "roles.view",
            "roles.manage",
            "audit.view",
            "audit.export",
            "settings.view",
            "settings.edit",
            "tenants.manage",
        ],
        _ => vec![],
    }
}
