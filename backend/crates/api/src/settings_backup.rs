//! Settings Backup, Export/Import, and Settings Profiles
//!
//! Provides comprehensive backup functionality:
//! - Export all tenant settings, users, departments, roles as encrypted JSON
//! - Import/restore from backup with dry-run preview
//! - Apply partial settings profiles (NixOS-style declarative config)
//! - Global settings export/import (SuperAdmin only)
//!
//! Security:
//! - All backups encrypted with ChaCha20-Poly1305 (passphrase-derived key via Argon2id)
//! - Password re-confirmation required for all operations
//! - Sensitive fields (passwords, API keys) redacted by default
//! - Rate-limited exports/imports, brute-force detection on decrypt
//! - Full audit logging of all backup operations

use axum::{
    extract::{State, Query, Path},
    http::{StatusCode, HeaderMap, header},
    response::{Json, Response},
    Extension,
    body::Body,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use argon2::Argon2;
use rand::RngCore;

use crate::AppState;
use crate::health::CURRENT_VERSION;
use clovalink_auth::AuthUser;
use clovalink_core::security_service::{self, AlertType};
use clovalink_core::circuit_breaker::CircuitState;

// ============================================================================
// CONSTANTS
// ============================================================================

const REDACTED: &str = "***REDACTED***";
const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 16;
const KEY_SIZE: usize = 32;
const MAX_BACKUP_SIZE: usize = 50 * 1024 * 1024; // 50MB

/// Fields that are NEVER exported (even with include_secrets)
const ALWAYS_REDACTED: &[&str] = &["password_hash", "totp_secret", "recovery_token"];

/// Prefix for encrypted-at-rest values (to distinguish from plaintext)
const ENCRYPTED_PREFIX: &str = "enc:";

/// Valid permission names for import validation
const VALID_PERMISSIONS: &[&str] = &[
    "files.read", "files.write", "files.delete", "files.share",
    "requests.read", "requests.write", "requests.manage",
    "users.read", "users.write", "users.manage",
    "roles.read", "roles.write",
    "audit.read", "audit.manage",
    "settings.read", "settings.write",
    "tenants.read", "tenants.manage",
    "approvals.view", "approvals.manage",
];

/// Derive a proper key from BACKUP_MASTER_KEY using Argon2id (instead of direct byte-copy)
fn derive_master_key_bytes(master_key: &[u8]) -> [u8; KEY_SIZE] {
    let params = argon2::Params::new(65536, 4, 4, Some(KEY_SIZE))
        .expect("valid Argon2 params");
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0u8; KEY_SIZE];
    argon2.hash_password_into(master_key, b"CLOVALINK_MASTER_KEY_SALT", &mut key)
        .expect("Argon2 master key derivation failed");
    key
}

/// Check if BACKUP_MASTER_KEY is configured and valid (≥32 chars)
pub(crate) fn is_master_key_configured() -> bool {
    std::env::var("BACKUP_MASTER_KEY")
        .map(|k| k.len() >= 32)
        .unwrap_or(false)
}

/// Normalize a cron expression to 6-field format (with seconds) for the `cron` crate.
/// Standard 5-field expressions like "0 2 * * *" become "0 0 2 * * *".
fn normalize_cron(expr: &str) -> String {
    let fields: Vec<&str> = expr.trim().split_whitespace().collect();
    if fields.len() == 5 {
        format!("0 {}", expr.trim())
    } else {
        expr.to_string()
    }
}

/// Encrypt a passphrase for at-rest storage using BACKUP_MASTER_KEY env var.
/// Returns "enc:<base64(nonce + ciphertext)>" if master key is set, or plaintext if not.
fn encrypt_passphrase_at_rest(passphrase: &str) -> String {
    let master_key = match std::env::var("BACKUP_MASTER_KEY") {
        Ok(k) if k.len() >= 32 => k,
        _ => {
            tracing::warn!("BACKUP_MASTER_KEY not set or too short — storing passphrase without at-rest encryption");
            return passphrase.to_string();
        }
    };

    let key_bytes = derive_master_key_bytes(master_key.as_bytes());

    let cipher = ChaCha20Poly1305::new((&key_bytes).into());
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    match cipher.encrypt(nonce, passphrase.as_bytes()) {
        Ok(ciphertext) => {
            let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
            combined.extend_from_slice(&nonce_bytes);
            combined.extend_from_slice(&ciphertext);
            format!("{}{}", ENCRYPTED_PREFIX, base64::engine::general_purpose::STANDARD.encode(combined))
        }
        Err(e) => {
            tracing::error!("Failed to encrypt passphrase at rest: {:?}", e);
            passphrase.to_string()
        }
    }
}

/// Decrypt a passphrase from at-rest storage. Handles both encrypted ("enc:...") and legacy plaintext.
fn decrypt_passphrase_at_rest(stored: &str) -> Result<String, &'static str> {
    if !stored.starts_with(ENCRYPTED_PREFIX) {
        // Legacy plaintext — return as-is
        return Ok(stored.to_string());
    }

    let master_key = std::env::var("BACKUP_MASTER_KEY")
        .map_err(|_| "BACKUP_MASTER_KEY not set — cannot decrypt passphrase")?;
    if master_key.len() < 32 {
        return Err("BACKUP_MASTER_KEY too short");
    }

    let key_bytes = derive_master_key_bytes(master_key.as_bytes());

    let encoded = &stored[ENCRYPTED_PREFIX.len()..];
    let combined = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Invalid base64 in encrypted passphrase")?;

    if combined.len() < NONCE_SIZE {
        return Err("Encrypted passphrase too short");
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let cipher = ChaCha20Poly1305::new((&key_bytes).into());
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Failed to decrypt passphrase — wrong BACKUP_MASTER_KEY?")?;

    String::from_utf8(plaintext)
        .map_err(|_| "Decrypted passphrase is not valid UTF-8")
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Deserialize)]
pub struct ExportParams {
    pub sections: Option<String>,
    pub include_optional: Option<String>,
    pub include_secrets: Option<bool>,
    pub audit_days: Option<i64>,
    pub file_limit: Option<i64>,
    pub approval_days: Option<i64>,
}

#[derive(Deserialize)]
pub struct ImportRequest {
    pub data: String, // base64-encoded encrypted backup
    pub sections: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct SettingsProfileRequest {
    pub profile: Value,       // plaintext partial JSON config
    pub dry_run: Option<bool>,
}

#[derive(Deserialize)]
pub struct CurrentSettingsParams {
    pub mode: Option<String>, // "global" or "tenant" (default)
}

#[derive(Deserialize)]
pub struct GlobalToggleRequest {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct BackupListParams {
    pub mode: Option<String>, // "global" or "tenant" (default)
}

#[derive(Deserialize)]
pub struct GlobalScheduleRequest {
    pub enabled: Option<bool>,
    pub cron: Option<String>,
    pub retention_count: Option<i32>,
}

// ============================================================================
// ENCRYPTION / DECRYPTION
// ============================================================================

/// Encrypt JSON backup data with ChaCha20-Poly1305
/// Key derived from passphrase using Argon2id
fn encrypt_backup(plaintext: &[u8], passphrase: &str) -> Result<Value, StatusCode> {
    // Generate random salt and nonce
    let mut salt = [0u8; SALT_SIZE];
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    // Derive key from passphrase using Argon2id
    let key = derive_key(passphrase, &salt)?;

    // Encrypt with ChaCha20-Poly1305
    let cipher = ChaCha20Poly1305::new((&key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|e| {
            tracing::error!("Backup encryption failed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(json!({
        "encrypted": true,
        "kdf": "argon2id",
        "salt": base64::engine::general_purpose::STANDARD.encode(salt),
        "nonce": base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
        "data": base64::engine::general_purpose::STANDARD.encode(ciphertext)
    }))
}

/// Decrypt backup data
fn decrypt_backup(encrypted: &Value, passphrase: &str) -> Result<Vec<u8>, &'static str> {
    use base64::Engine;

    let salt_b64 = encrypted.get("salt").and_then(|v| v.as_str()).ok_or("Missing salt")?;
    let nonce_b64 = encrypted.get("nonce").and_then(|v| v.as_str()).ok_or("Missing nonce")?;
    let data_b64 = encrypted.get("data").and_then(|v| v.as_str()).ok_or("Missing data")?;

    let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64).map_err(|_| "Invalid salt")?;
    let nonce_bytes = base64::engine::general_purpose::STANDARD.decode(nonce_b64).map_err(|_| "Invalid nonce")?;
    let ciphertext = base64::engine::general_purpose::STANDARD.decode(data_b64).map_err(|_| "Invalid data")?;

    if salt.len() != SALT_SIZE {
        return Err("Invalid salt length");
    }
    if nonce_bytes.len() != NONCE_SIZE {
        return Err("Invalid nonce length");
    }

    // Derive key
    let key = derive_key(passphrase, &salt).map_err(|_| "Key derivation failed")?;

    // Decrypt
    let cipher = ChaCha20Poly1305::new((&key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher.decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| "Invalid passphrase")
}

/// Derive a 256-bit key from passphrase using Argon2id
fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE], StatusCode> {
    let params = argon2::Params::new(65536, 4, 4, Some(KEY_SIZE))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2.hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| {
            tracing::error!("Argon2 key derivation failed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(key)
}

// ============================================================================
// SECURITY HELPERS
// ============================================================================

/// Verify password re-confirmation from X-Confirm-Password header
/// Rate-limited: 5 failures per user per 15 minutes
pub(crate) async fn verify_password_confirmation(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    // Rate limit: check recent password confirmation failures
    let fifteen_min_ago = Utc::now() - chrono::Duration::minutes(15);
    let fail_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM security_alerts
           WHERE alert_type = 'password_confirm_failed'
           AND user_id = $1
           AND created_at > $2"#
    )
    .bind(user_id)
    .bind(fifteen_min_ago)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if fail_count.0 >= 5 {
        tracing::warn!("Password confirmation rate limit hit for user {}", user_id);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let password = headers.get("X-Confirm-Password")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let hash: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT password_hash FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let hash = hash
        .and_then(|(h,)| h)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let parsed = argon2::PasswordHash::new(&hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if argon2::PasswordVerifier::verify_password(
        &Argon2::default(),
        password.as_bytes(),
        &parsed,
    ).is_err() {
        // Record failed attempt as security alert for rate limiting
        let _ = security_service::create_alert(
            pool,
            None,
            Some(user_id),
            AlertType::PasswordConfirmFailed,
            "Failed password confirmation for backup operation",
            &format!("Failed password confirmation attempt ({} in 15 min window)", fail_count.0 + 1),
            json!({ "attempt_count": fail_count.0 + 1 }),
            None,
        ).await;
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}

/// Get backup passphrase from X-Backup-Passphrase header
fn get_passphrase(headers: &HeaderMap) -> Result<String, StatusCode> {
    let passphrase = headers.get("X-Backup-Passphrase")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Prevent DoS via extremely long passphrases hitting Argon2id
    if passphrase.len() > 1024 {
        tracing::warn!("Passphrase exceeds max length ({})", passphrase.len());
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(passphrase)
}

/// Check brute-force attempts for backup decrypt
async fn check_and_record_decrypt_failure(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    ip_address: &str,
) -> Result<bool, StatusCode> {
    // Check recent failures
    let fifteen_min_ago = Utc::now() - chrono::Duration::minutes(15);
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM security_alerts
        WHERE alert_type = 'backup_decrypt_failed'
        AND user_id = $1
        AND created_at > $2
        "#
    )
    .bind(user_id)
    .bind(fifteen_min_ago)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Record the failed attempt
    let _ = security_service::create_alert(
        pool,
        Some(tenant_id),
        Some(user_id),
        AlertType::BackupDecryptFailed,
        "Failed backup decrypt attempt",
        &format!("Failed to decrypt backup file (attempt {} in 15 min window)", count.0 + 1),
        json!({
            "attempt_count": count.0 + 1,
            "ip_address": ip_address
        }),
        Some(ip_address),
    ).await;

    // If 5+ failures, trigger brute-force alert
    if count.0 + 1 >= 5 {
        let _ = security_service::create_alert(
            pool,
            Some(tenant_id),
            Some(user_id),
            AlertType::BackupBruteForce,
            "Backup brute-force attempt detected",
            &format!("{} failed backup decrypt attempts in 15 minutes — user locked out of backup operations", count.0 + 1),
            json!({
                "attempt_count": count.0 + 1,
                "ip_address": ip_address,
                "lockout": true
            }),
            Some(ip_address),
        ).await;
        return Ok(true); // locked out
    }

    Ok(false)
}

/// Check if user is locked out from backup operations
async fn is_backup_locked_out(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<bool, StatusCode> {
    let fifteen_min_ago = Utc::now() - chrono::Duration::minutes(15);
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM security_alerts
        WHERE alert_type = 'backup_brute_force'
        AND user_id = $1
        AND created_at > $2
        "#
    )
    .bind(user_id)
    .bind(fifteen_min_ago)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(count.0 > 0)
}

/// Log a backup audit event
async fn log_backup_audit(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    action: &str,
    metadata: Value,
    ip_address: &str,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, metadata, ip_address)
        VALUES ($1, $2, $3, 'backup', $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(action)
    .bind(&metadata)
    .bind(ip_address)
    .execute(pool)
    .await;
}

// ============================================================================
// PER-TENANT + CIRCUIT BREAKER GUARDS
// ============================================================================

/// Check if backup is enabled for this tenant (SuperAdmin bypasses)
async fn check_backup_enabled(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    role: &str,
) -> Result<(), StatusCode> {
    if role == "SuperAdmin" {
        return Ok(());
    }
    let enabled: Option<(Option<bool>,)> = sqlx::query_as(
        "SELECT backup_enabled FROM tenants WHERE id = $1"
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match enabled {
        Some((Some(false),)) => Err(StatusCode::FORBIDDEN),
        _ => Ok(()), // default true
    }
}

/// Check circuit breaker and acquire semaphore permit.
/// Returns 503 if circuit is open, 429 if too many concurrent operations.
fn check_backup_infra(state: &AppState) -> Result<tokio::sync::OwnedSemaphorePermit, StatusCode> {
    if !state.backup_circuit_breaker.allow_request() {
        tracing::warn!("Backup circuit breaker is open — rejecting request");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    state.backup_semaphore.clone().try_acquire_owned()
        .map_err(|_| {
            tracing::warn!("Backup concurrency limit reached — rejecting request");
            StatusCode::TOO_MANY_REQUESTS
        })
}

// ============================================================================
// SECTION COLLECTORS (EXPORT)
// ============================================================================

fn redact_value(obj: &mut Value, field: &str, include_secrets: bool) {
    if let Some(map) = obj.as_object_mut() {
        if let Some(val) = map.get_mut(field) {
            if ALWAYS_REDACTED.contains(&field) || !include_secrets {
                *val = Value::String(REDACTED.to_string());
            }
        }
    }
}

async fn collect_tenant_core(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    include_secrets: bool,
) -> Result<Value, StatusCode> {
    let row: Value = sqlx::query_scalar(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT compliance_mode, encryption_standard, retention_policy_days,
                   mfa_required, session_timeout_minutes, public_sharing_enabled,
                   data_export_enabled, blocked_extensions, password_policy,
                   ip_restriction_mode, ip_allowlist, ip_blocklist,
                   storage_quota_bytes, max_upload_size_bytes,
                   enable_totp, enable_passkeys, auth_methods,
                   approval_workflow_enabled,
                   smtp_host, smtp_port, smtp_username, smtp_password,
                   smtp_from, smtp_secure
            FROM tenants WHERE id = $1
        ) t
        "#
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to collect tenant core: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut result = row;
    redact_value(&mut result, "smtp_password", include_secrets);
    Ok(result)
}

async fn collect_users(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    _include_secrets: bool,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(u) FROM (
            SELECT email, name, role, status, department_id, custom_role_id,
                   identity_provider, avatar_url, allowed_tenant_ids,
                   allowed_department_ids, password_changed_at,
                   suspended_at, suspended_until, suspension_reason,
                   dashboard_layout, widget_config, last_active_at,
                   created_at, updated_at
            FROM users WHERE tenant_id = $1
            ORDER BY created_at
        ) u
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to collect users: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Value::Array(rows))
}

async fn collect_departments(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(d) FROM (
            SELECT name, description, created_at
            FROM departments WHERE tenant_id = $1
            ORDER BY name
        ) d
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to collect departments: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Value::Array(rows))
}

async fn collect_roles(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT json_build_object(
            'name', r.name,
            'description', r.description,
            'base_role', r.base_role,
            'is_system', r.is_system,
            'permissions', COALESCE(
                (SELECT json_agg(json_build_object('permission', rp.permission, 'granted', rp.granted))
                 FROM role_permissions rp WHERE rp.role_id = r.id),
                '[]'::json
            )
        )
        FROM roles r
        WHERE r.tenant_id = $1
        ORDER BY r.name
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to collect roles: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Value::Array(rows))
}

async fn collect_audit_settings(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(a) FROM (
            SELECT log_logins, log_file_operations, log_user_changes,
                   log_settings_changes, log_role_changes, retention_days
            FROM audit_settings WHERE tenant_id = $1
        ) a
        "#
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(row.unwrap_or(json!({
        "log_logins": true,
        "log_file_operations": true,
        "log_user_changes": true,
        "log_settings_changes": true,
        "log_role_changes": true,
        "retention_days": 90
    })))
}

async fn collect_virus_scan(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(v) FROM (
            SELECT enabled, file_types, max_file_size_mb, action_on_detect,
                   notify_admin, notify_uploader, auto_suspend_uploader, suspend_threshold
            FROM virus_scan_settings WHERE tenant_id = $1
        ) v
        "#
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(row.unwrap_or(Value::Null))
}

async fn collect_ai_settings(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    include_secrets: bool,
) -> Result<Value, StatusCode> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(a) FROM (
            SELECT enabled, provider, api_key_encrypted, allowed_roles,
                   hipaa_approved_only, sox_read_only, monthly_token_limit, daily_request_limit,
                   custom_endpoint, custom_model, maintenance_mode, maintenance_message
            FROM tenant_ai_settings WHERE tenant_id = $1
        ) a
        "#
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match row {
        Some(mut v) => {
            redact_value(&mut v, "api_key_encrypted", include_secrets);
            Ok(v)
        }
        None => Ok(Value::Null),
    }
}

async fn collect_discord_settings(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    _include_secrets: bool,
) -> Result<Value, StatusCode> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(d) FROM (
            SELECT enabled
            FROM tenant_discord_settings WHERE tenant_id = $1
        ) d
        "#
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(row.unwrap_or(Value::Null))
}

async fn collect_sso_oidc(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    include_secrets: bool,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(p) FROM (
            SELECT name, slug, provider_type, issuer_url, client_id, client_secret_encrypted,
                   scopes, authorization_endpoint, token_endpoint, userinfo_endpoint, jwks_uri,
                   auto_provision, default_role, default_department_id, email_domains,
                   trust_idp_mfa, enabled
            FROM tenant_oidc_providers WHERE tenant_id = $1
            ORDER BY name
        ) p
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows: Vec<Value> = rows.into_iter().map(|mut v| {
        redact_value(&mut v, "client_secret_encrypted", include_secrets);
        v
    }).collect();

    Ok(Value::Array(rows))
}

async fn collect_sso_saml(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    include_secrets: bool,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(p) FROM (
            SELECT name, slug, provider_type, idp_entity_id, idp_sso_url, idp_slo_url,
                   idp_metadata_url, idp_signing_certificate, sp_entity_id, nameid_format,
                   request_signing, want_assertions_signed, want_response_signed,
                   sp_signing_key_encrypted, sp_signing_cert, sso_binding,
                   attribute_email, attribute_name,
                   auto_provision, default_role, default_custom_role_id, default_department_id,
                   email_domains, trust_idp_mfa, enabled
            FROM tenant_saml_providers WHERE tenant_id = $1
            ORDER BY name
        ) p
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows: Vec<Value> = rows.into_iter().map(|mut v| {
        redact_value(&mut v, "sp_signing_key_encrypted", include_secrets);
        v
    }).collect();

    Ok(Value::Array(rows))
}

async fn collect_sso_identities(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let oidc: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(i) FROM (
            SELECT u.email as user_email, oi.oidc_subject, oi.oidc_issuer,
                   oi.oidc_email, oi.oidc_name, oi.login_count
            FROM user_oidc_identities oi
            JOIN users u ON u.id = oi.user_id
            WHERE u.tenant_id = $1
        ) i
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let saml: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(i) FROM (
            SELECT u.email as user_email, si.saml_name_id, si.saml_name_id_format,
                   si.saml_email, si.saml_name, si.login_count
            FROM user_saml_identities si
            JOIN users u ON u.id = si.user_id
            WHERE u.tenant_id = $1
        ) i
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(json!({
        "oidc": oidc,
        "saml": saml
    }))
}

async fn collect_sso_mappings(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(m) FROM (
            SELECT protocol, attribute_name, attribute_value, match_type,
                   target_role, target_custom_role_id, target_department_id,
                   priority, enabled
            FROM sso_attribute_mappings WHERE tenant_id = $1
            ORDER BY priority
        ) m
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

async fn collect_approval_policies(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(p) FROM (
            SELECT name, scope, scope_value, required_approvals, is_active
            FROM approval_policies WHERE tenant_id = $1
            ORDER BY name
        ) p
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

async fn collect_email_templates(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT template_key, subject, body_html, body_text
            FROM tenant_email_templates WHERE tenant_id = $1
            ORDER BY template_key
        ) t
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

async fn collect_notification_settings(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Result<Value, StatusCode> {
    let tenant_settings: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(n) FROM (
            SELECT event_type, role, enabled, email_enforced, in_app_enforced,
                   default_email, default_in_app
            FROM tenant_notification_settings WHERE tenant_id = $1
            ORDER BY event_type, role
        ) n
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(tenant_settings))
}

// Optional large sections

async fn collect_file_metadata(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    limit: i64,
) -> Result<Value, StatusCode> {
    let limit = limit.min(100_000);
    let files: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(f) FROM (
            SELECT name, storage_path, size_bytes, content_type, is_directory,
                   parent_path, visibility, is_company_folder, version,
                   is_immutable, is_locked, content_hash, ulid,
                   is_deleted, deleted_at, approval_status, created_at, updated_at
            FROM files_metadata WHERE tenant_id = $1
            ORDER BY created_at
            LIMIT $2
        ) f
        "#
    )
    .bind(tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let shares: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(s) FROM (
            SELECT token, is_public, is_directory, share_policy,
                   expires_at, download_count, created_at
            FROM file_shares WHERE tenant_id = $1
            ORDER BY created_at
        ) s
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(json!({
        "files": files,
        "shares": shares
    }))
}

async fn collect_audit_logs(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    days: i64,
) -> Result<Value, StatusCode> {
    let days = days.max(1).min(3650);
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(a) FROM (
            SELECT action, resource_type, resource_id, metadata,
                   ip_address, created_at
            FROM audit_logs WHERE tenant_id = $1
            AND created_at > NOW() - make_interval(days => $2)
            ORDER BY created_at
            LIMIT 500000
        ) a
        "#
    )
    .bind(tenant_id)
    .bind(days as i32)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

async fn collect_approval_history(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    days: i64,
) -> Result<Value, StatusCode> {
    let days = days.max(1).min(3650);
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(a) FROM (
            SELECT status, step, rejection_reason, decided_at, created_at
            FROM approval_requests WHERE tenant_id = $1
            AND created_at > NOW() - make_interval(days => $2)
            ORDER BY created_at
            LIMIT 500000
        ) a
        "#
    )
    .bind(tenant_id)
    .bind(days as i32)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

/// Keys that must never appear in exported backup data
const SENSITIVE_GLOBAL_KEYS: &[&str] = &["auto_backup_passphrase"];

/// Patterns in key names that indicate sensitive data
const SENSITIVE_KEY_PATTERNS: &[&str] = &["secret", "password", "key", "token", "encrypted"];

/// Check if a global settings key is sensitive (exact match or pattern)
fn is_sensitive_key(key: &str) -> bool {
    if SENSITIVE_GLOBAL_KEYS.contains(&key) {
        return true;
    }
    let lower = key.to_lowercase();
    SENSITIVE_KEY_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Strip sensitive keys from a global_settings JSON object before export
fn strip_sensitive_keys(mut settings: Value) -> Value {
    if let Some(map) = settings.as_object_mut() {
        let sensitive_keys: Vec<String> = map.keys()
            .filter(|k| is_sensitive_key(k))
            .cloned()
            .collect();
        for key in sensitive_keys {
            map.remove(&key);
        }
    }
    settings
}

// Global settings collector
async fn collect_global_settings(
    pool: &sqlx::PgPool,
) -> Result<Value, StatusCode> {
    let settings: Vec<(String, Value)> = sqlx::query_as(
        "SELECT key, value FROM global_settings ORDER BY key"
    )
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut map = serde_json::Map::new();
    for (key, value) in settings {
        map.insert(key, value);
    }
    Ok(Value::Object(map))
}

async fn collect_global_email_templates(
    pool: &sqlx::PgPool,
) -> Result<Value, StatusCode> {
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT template_key, name, subject, body_html, body_text, variables
            FROM email_templates
            ORDER BY template_key
        ) t
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Value::Array(rows))
}

// ============================================================================
// EXPORT HANDLERS
// ============================================================================

/// GET /api/backup/export
/// Export tenant backup as encrypted download
pub async fn export_tenant_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Query(params): Query<ExportParams>,
) -> Result<Response<Body>, StatusCode> {
    // Auth: Admin+ only
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Per-tenant check + circuit breaker + concurrency
    check_backup_enabled(&state.pool, auth.tenant_id, &auth.role).await?;
    let _permit = check_backup_infra(&state)?;

    // Check lockout
    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Verify password
    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;

    // Get passphrase
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // include_secrets requires SuperAdmin
    let include_secrets = params.include_secrets.unwrap_or(false);
    if include_secrets && auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    // Pagination params for large sections
    let audit_days = params.audit_days.unwrap_or(90);
    let file_limit = params.file_limit.unwrap_or(50000);
    let approval_days = params.approval_days.unwrap_or(90);

    // Parse requested sections
    let valid_tenant_sections: &[&str] = &[
        "tenant_core", "users", "departments", "roles",
        "settings_audit", "settings_virus_scan", "settings_ai", "settings_discord",
        "sso_oidc", "sso_saml", "sso_mappings", "sso_identities",
        "approval_policies", "email_templates", "notification_settings",
    ];
    let valid_optional_sections: &[&str] = &[
        "file_metadata", "audit_logs", "approval_history",
    ];
    let sections: Vec<String> = params.sections
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| valid_tenant_sections.iter().map(|s| s.to_string()).collect());

    // Reject unknown sections
    for s in &sections {
        if !valid_tenant_sections.contains(&s.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let optional: Vec<String> = params.include_optional
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    for s in &optional {
        if !valid_optional_sections.contains(&s.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Get tenant name
    let tenant_name: (String,) = sqlx::query_as("SELECT name FROM tenants WHERE id = $1")
        .bind(auth.tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build backup JSON
    let mut backup = json!({
        "_meta": {
            "format": "clovalink-backup",
            "format_version": 1,
            "clovalink_version": CURRENT_VERSION,
            "export_type": "tenant",
            "tenant_id": auth.tenant_id.to_string(),
            "tenant_name": tenant_name.0,
            "exported_at": Utc::now().to_rfc3339(),
            "exported_by": auth.user_id.to_string(),
            "include_secrets": include_secrets,
            "sections": &sections
        }
    });

    let backup_map = backup.as_object_mut().unwrap();

    // Collect each section
    for section in &sections {
        let value = match section.as_str() {
            "tenant_core" => collect_tenant_core(&state.pool, auth.tenant_id, include_secrets).await?,
            "users" => collect_users(&state.pool, auth.tenant_id, include_secrets).await?,
            "departments" => collect_departments(&state.pool, auth.tenant_id).await?,
            "roles" => collect_roles(&state.pool, auth.tenant_id).await?,
            "settings_audit" => collect_audit_settings(&state.pool, auth.tenant_id).await?,
            "settings_virus_scan" => collect_virus_scan(&state.pool, auth.tenant_id).await?,
            "settings_ai" => collect_ai_settings(&state.pool, auth.tenant_id, include_secrets).await?,
            "settings_discord" => collect_discord_settings(&state.pool, auth.tenant_id, include_secrets).await?,
            "sso_oidc" => collect_sso_oidc(&state.pool, auth.tenant_id, include_secrets).await?,
            "sso_saml" => collect_sso_saml(&state.pool, auth.tenant_id, include_secrets).await?,
            "sso_mappings" => collect_sso_mappings(&state.pool, auth.tenant_id).await?,
            "sso_identities" => collect_sso_identities(&state.pool, auth.tenant_id).await?,
            "approval_policies" => collect_approval_policies(&state.pool, auth.tenant_id).await?,
            "email_templates" => collect_email_templates(&state.pool, auth.tenant_id).await?,
            "notification_settings" => collect_notification_settings(&state.pool, auth.tenant_id).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    // Collect optional sections (with pagination limits)
    for section in &optional {
        let value = match section.as_str() {
            "file_metadata" => collect_file_metadata(&state.pool, auth.tenant_id, file_limit).await?,
            "audit_logs" => collect_audit_logs(&state.pool, auth.tenant_id, audit_days).await?,
            "approval_history" => collect_approval_history(&state.pool, auth.tenant_id, approval_days).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    // Encrypt the backup
    let plaintext = serde_json::to_vec(&backup)
        .map_err(|e| {
            state.backup_circuit_breaker.record_failure();
            tracing::error!("Backup serialization failed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let encrypted = encrypt_backup(&plaintext, &passphrase).map_err(|e| {
        state.backup_circuit_breaker.record_failure();
        e
    })?;
    let encrypted_bytes = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.backup_circuit_breaker.record_success();

    // Audit log
    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_export",
        json!({
            "sections": &sections,
            "optional_sections": &optional,
            "include_secrets": include_secrets,
            "size_bytes": encrypted_bytes.len()
        }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    // Security alert if secrets were included
    if include_secrets {
        let _ = security_service::create_alert(
            &state.pool,
            Some(auth.tenant_id),
            Some(auth.user_id),
            AlertType::BackupExportSecrets,
            "Backup exported with secrets",
            &format!("User exported backup including encrypted secrets for tenant {}", tenant_name.0),
            json!({
                "sections": &sections,
                "tenant_name": tenant_name.0
            }),
            auth.ip_address.as_deref(),
        ).await;
    }

    // Build response with download headers
    let filename = format!(
        "clovalink-backup-{}-{}-{:06x}.clovalink.json",
        tenant_name.0.to_lowercase().replace(' ', "-"),
        Utc::now().format("%Y%m%d-%H%M%S"),
        rand::random::<u32>() & 0xFFFFFF
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(encrypted_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/backup/global/export
/// Export global settings as encrypted download (SuperAdmin only)
pub async fn export_global(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Query(params): Query<ExportParams>,
) -> Result<Response<Body>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_global_backup_enabled(&state.pool).await?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let valid_global = ["global_settings", "global_email_templates"];
    let selected: Vec<String> = params.sections
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())
        .unwrap_or_else(|| valid_global.iter().map(|s| s.to_string()).collect());

    // Reject unknown sections
    for s in &selected {
        if !valid_global.contains(&s.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let mut backup = json!({
        "_meta": {
            "format": "clovalink-backup",
            "format_version": 1,
            "clovalink_version": CURRENT_VERSION,
            "export_type": "global",
            "exported_at": Utc::now().to_rfc3339(),
            "exported_by": auth.user_id.to_string(),
            "sections": &selected
        }
    });

    let backup_map = backup.as_object_mut().unwrap();
    for section in &selected {
        let value = match section.as_str() {
            "global_settings" => strip_sensitive_keys(collect_global_settings(&state.pool).await?),
            "global_email_templates" => collect_global_email_templates(&state.pool).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    let plaintext = serde_json::to_vec(&backup)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = encrypt_backup(&plaintext, &passphrase)?;
    let encrypted_bytes = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_export_global",
        json!({ "size_bytes": encrypted_bytes.len() }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    let filename = format!(
        "clovalink-global-backup-{}-{:06x}.clovalink.json",
        Utc::now().format("%Y%m%d-%H%M%S"),
        rand::random::<u32>() & 0xFFFFFF
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(encrypted_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ============================================================================
// IMPORT / PREVIEW HANDLERS
// ============================================================================

/// POST /api/backup/import/preview
/// Dry-run import showing what would change
pub async fn preview_import(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Per-tenant check + circuit breaker
    check_backup_enabled(&state.pool, auth.tenant_id, &auth.role).await?;
    let _permit = check_backup_infra(&state)?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Size check
    if body.data.len() > MAX_BACKUP_SIZE {
        return Ok(Json(json!({ "valid": false, "errors": ["Backup file exceeds 50MB limit"] })));
    }

    // Parse and decrypt
    let encrypted: Value = serde_json::from_str(&body.data)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let plaintext = match decrypt_backup(&encrypted, &passphrase) {
        Ok(data) => data,
        Err(_) => {
            let locked = check_and_record_decrypt_failure(
                &state.pool, auth.tenant_id, auth.user_id, auth.ip_address.as_deref().unwrap_or("unknown")
            ).await?;
            if locked {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            return Ok(Json(json!({ "valid": false, "errors": ["Invalid passphrase"] })));
        }
    };

    let backup: Value = serde_json::from_slice(&plaintext)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Validate format
    let meta = backup.get("_meta").ok_or(StatusCode::BAD_REQUEST)?;
    let format = meta.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if format != "clovalink-backup" {
        return Ok(Json(json!({ "valid": false, "errors": ["Invalid backup format"] })));
    }

    let format_version = meta.get("format_version").and_then(|v| v.as_i64()).unwrap_or(0);
    if format_version != 1 {
        return Ok(Json(json!({ "valid": false, "errors": [format!("Unsupported backup version: {}", format_version)] })));
    }

    // Build preview of changes per section
    let mut sections_preview: HashMap<String, Value> = HashMap::new();
    let mut warnings = Vec::new();

    let selected_sections = body.sections.clone().unwrap_or_else(|| {
        meta.get("sections")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    });

    for section in &selected_sections {
        if let Some(section_data) = backup.get(section) {
            match section.as_str() {
                "users" => {
                    if let Some(users) = section_data.as_array() {
                        let mut added = 0;
                        let mut updated = 0;
                        for user in users {
                            if let Some(email) = user.get("email").and_then(|v| v.as_str()) {
                                let exists: (i64,) = sqlx::query_as(
                                    "SELECT COUNT(*) FROM users WHERE email = $1 AND tenant_id = $2"
                                )
                                .bind(email)
                                .bind(auth.tenant_id)
                                .fetch_one(&state.pool)
                                .await
                                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                                if exists.0 > 0 { updated += 1; } else { added += 1; }
                            }
                        }
                        sections_preview.insert(section.clone(), json!({
                            "total": users.len(),
                            "new_users": added,
                            "existing_users_updated": updated
                        }));
                    }
                }
                "departments" => {
                    if let Some(depts) = section_data.as_array() {
                        let mut added = 0;
                        let mut existing = 0;
                        for dept in depts {
                            if let Some(name) = dept.get("name").and_then(|v| v.as_str()) {
                                let exists: (i64,) = sqlx::query_as(
                                    "SELECT COUNT(*) FROM departments WHERE name = $1 AND tenant_id = $2"
                                )
                                .bind(name)
                                .bind(auth.tenant_id)
                                .fetch_one(&state.pool)
                                .await
                                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                                if exists.0 > 0 { existing += 1; } else { added += 1; }
                            }
                        }
                        sections_preview.insert(section.clone(), json!({
                            "total": depts.len(),
                            "new": added,
                            "existing": existing
                        }));
                    }
                }
                "roles" => {
                    if let Some(roles) = section_data.as_array() {
                        sections_preview.insert(section.clone(), json!({
                            "total": roles.len(),
                            "note": "Custom roles will be matched by name; permissions will be replaced"
                        }));
                    }
                }
                "tenant_core" => {
                    // Show which fields would change
                    let current = collect_tenant_core(&state.pool, auth.tenant_id, false).await?;
                    let mut changes = Vec::new();
                    if let (Some(cur_map), Some(new_map)) = (current.as_object(), section_data.as_object()) {
                        for (key, new_val) in new_map {
                            if ALWAYS_REDACTED.contains(&key.as_str()) { continue; }
                            if new_val.as_str() == Some(REDACTED) { continue; }
                            if let Some(cur_val) = cur_map.get(key) {
                                if cur_val != new_val {
                                    changes.push(json!({
                                        "field": key,
                                        "current": cur_val,
                                        "new": new_val
                                    }));
                                }
                            }
                        }
                    }
                    sections_preview.insert(section.clone(), json!({
                        "changes": changes,
                        "change_count": changes.len()
                    }));
                }
                _ => {
                    // Generic: just show count
                    if let Some(arr) = section_data.as_array() {
                        sections_preview.insert(section.clone(), json!({
                            "items": arr.len()
                        }));
                    } else {
                        sections_preview.insert(section.clone(), json!({
                            "has_data": !section_data.is_null()
                        }));
                    }
                }
            }
        } else {
            warnings.push(format!("Section '{}' not found in backup file", section));
        }
    }

    Ok(Json(json!({
        "valid": true,
        "meta": meta,
        "sections": sections_preview,
        "warnings": warnings,
        "errors": []
    })))
}

/// POST /api/backup/import
/// Import tenant backup (full restore)
pub async fn import_tenant_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Per-tenant check + circuit breaker
    check_backup_enabled(&state.pool, auth.tenant_id, &auth.role).await?;
    let _permit = check_backup_infra(&state)?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    if body.data.len() > MAX_BACKUP_SIZE {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Parse and decrypt
    let encrypted: Value = serde_json::from_str(&body.data)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let plaintext = match decrypt_backup(&encrypted, &passphrase) {
        Ok(data) => data,
        Err(_) => {
            let locked = check_and_record_decrypt_failure(
                &state.pool, auth.tenant_id, auth.user_id, auth.ip_address.as_deref().unwrap_or("unknown")
            ).await?;
            if locked {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            return Ok(Json(json!({ "success": false, "error": "Invalid passphrase" })));
        }
    };

    let backup: Value = serde_json::from_slice(&plaintext)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let meta = backup.get("_meta").ok_or(StatusCode::BAD_REQUEST)?;
    let format = meta.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if format != "clovalink-backup" {
        return Ok(Json(json!({ "success": false, "error": "Invalid backup format" })));
    }

    let selected_sections = body.sections.clone().unwrap_or_else(|| {
        meta.get("sections")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    });

    // Run import in a transaction
    let mut tx = state.pool.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut results: HashMap<String, Value> = HashMap::new();
    let tenant_id = auth.tenant_id;

    for section in &selected_sections {
        if let Some(section_data) = backup.get(section) {
            let result = match section.as_str() {
                "tenant_core" => apply_tenant_core(&mut tx, tenant_id, section_data).await,
                "departments" => apply_departments(&mut tx, tenant_id, section_data).await,
                "roles" => apply_roles(&mut tx, tenant_id, section_data).await,
                "users" => apply_users(&mut tx, tenant_id, section_data).await,
                "settings_audit" => apply_audit_settings(&mut tx, tenant_id, section_data).await,
                "settings_virus_scan" => apply_virus_scan(&mut tx, tenant_id, section_data).await,
                "settings_ai" => apply_ai_settings(&mut tx, tenant_id, section_data).await,
                "settings_discord" => apply_discord_settings(&mut tx, tenant_id, section_data).await,
                "approval_policies" => apply_approval_policies(&mut tx, tenant_id, section_data).await,
                "email_templates" => apply_email_templates(&mut tx, tenant_id, section_data).await,
                "notification_settings" => apply_notification_settings(&mut tx, tenant_id, section_data).await,
                _ => Ok(json!({ "skipped": true, "reason": "Section not supported for import" })),
            };

            match result {
                Ok(r) => { results.insert(section.clone(), r); }
                Err(e) => {
                    // Rollback on any error
                    let _ = tx.rollback().await;
                    return Ok(Json(json!({
                        "success": false,
                        "error": format!("Failed to import section '{}': {:?}", section, e),
                        "partial_results": results
                    })));
                }
            }
        }
    }

    // Commit transaction
    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit backup import: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log
    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_import",
        json!({
            "sections": &selected_sections,
            "results": &results
        }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    // Invalidate caches
    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::tenant_settings(tenant_id)).await;
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    Ok(Json(json!({
        "success": true,
        "sections_imported": selected_sections,
        "results": results
    })))
}

/// POST /api/backup/apply-profile
/// Apply a partial settings profile (merge semantics)
pub async fn apply_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Same as import but the backup data can be partial
    // Reuse import logic
    import_tenant_backup(State(state), Extension(auth), headers, Json(body)).await
}

/// POST /api/backup/apply-settings-profile
/// Apply a plaintext partial settings profile (NixOS-style declarative merge)
/// SuperAdmin only — no encryption needed, just password confirmation
pub async fn apply_settings_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<SettingsProfileRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_backup_enabled(&state.pool, auth.tenant_id, &auth.role).await?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;

    let profile = &body.profile;
    let dry_run = body.dry_run.unwrap_or(false);

    // Validate that profile is an object
    if !profile.is_object() {
        return Ok(Json(json!({ "success": false, "error": "Profile must be a JSON object" })));
    }

    let profile_obj = profile.as_object().unwrap();
    if profile_obj.is_empty() {
        return Ok(Json(json!({ "success": false, "error": "Profile is empty — nothing to apply" })));
    }

    // Validate section names
    let valid_sections = [
        "tenant_core", "departments", "roles", "users",
        "settings_audit", "settings_virus_scan", "settings_ai",
        "settings_discord", "approval_policies", "email_templates",
        "notification_settings",
    ];
    let unknown: Vec<&str> = profile_obj.keys()
        .filter(|k| !valid_sections.contains(&k.as_str()))
        .map(|k| k.as_str())
        .collect();
    if !unknown.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "error": format!("Unknown sections: {}. Valid sections: {}", unknown.join(", "), valid_sections.join(", "))
        })));
    }

    let mut tx = state.pool.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut results: HashMap<String, Value> = HashMap::new();
    let tenant_id = auth.tenant_id;

    for (section, section_data) in profile_obj {
        let result = match section.as_str() {
            "tenant_core" => apply_tenant_core(&mut tx, tenant_id, section_data).await,
            "departments" => apply_departments(&mut tx, tenant_id, section_data).await,
            "roles" => apply_roles(&mut tx, tenant_id, section_data).await,
            "users" => apply_users(&mut tx, tenant_id, section_data).await,
            "settings_audit" => apply_audit_settings(&mut tx, tenant_id, section_data).await,
            "settings_virus_scan" => apply_virus_scan(&mut tx, tenant_id, section_data).await,
            "settings_ai" => apply_ai_settings(&mut tx, tenant_id, section_data).await,
            "settings_discord" => apply_discord_settings(&mut tx, tenant_id, section_data).await,
            "approval_policies" => apply_approval_policies(&mut tx, tenant_id, section_data).await,
            "email_templates" => apply_email_templates(&mut tx, tenant_id, section_data).await,
            "notification_settings" => apply_notification_settings(&mut tx, tenant_id, section_data).await,
            _ => Ok(json!({ "skipped": true })),
        };

        match result {
            Ok(r) => { results.insert(section.clone(), r); }
            Err(e) => {
                let _ = tx.rollback().await;
                return Ok(Json(json!({
                    "success": false,
                    "error": format!("Failed to apply section '{}': {:?}", section, e),
                    "partial_results": results
                })));
            }
        }
    }

    if dry_run {
        let _ = tx.rollback().await;
        return Ok(Json(json!({
            "success": true,
            "dry_run": true,
            "sections": profile_obj.keys().collect::<Vec<_>>(),
            "results": results
        })));
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit settings profile: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_apply_profile",
        json!({
            "sections": profile_obj.keys().collect::<Vec<&String>>(),
            "results": &results
        }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::tenant_settings(tenant_id)).await;
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    Ok(Json(json!({
        "success": true,
        "sections_applied": profile_obj.keys().collect::<Vec<&String>>(),
        "results": results
    })))
}

/// GET /api/backup/current-settings
/// Returns current settings as plain JSON for the profile editor (SuperAdmin only)
pub async fn get_current_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<CurrentSettingsParams>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let mode = params.mode.as_deref().unwrap_or("tenant");

    match mode {
        "global" => {
            let global_settings = collect_global_settings(&state.pool).await?;
            let email_templates = collect_global_email_templates(&state.pool).await?;
            Ok(Json(json!({
                "global_settings": global_settings,
                "global_email_templates": email_templates
            })))
        }
        "tenant" | _ => {
            let tenant_id = auth.tenant_id;
            let tenant_core = collect_tenant_core(&state.pool, tenant_id, false).await?;
            let audit = collect_audit_settings(&state.pool, tenant_id).await?;
            let virus = collect_virus_scan(&state.pool, tenant_id).await?;
            let ai = collect_ai_settings(&state.pool, tenant_id, false).await?;
            let discord = collect_discord_settings(&state.pool, tenant_id, false).await?;
            let policies = collect_approval_policies(&state.pool, tenant_id).await?;
            let emails = collect_email_templates(&state.pool, tenant_id).await?;
            let notifs = collect_notification_settings(&state.pool, tenant_id).await?;

            Ok(Json(json!({
                "tenant_core": tenant_core,
                "settings_audit": audit,
                "settings_virus_scan": virus,
                "settings_ai": ai,
                "settings_discord": discord,
                "approval_policies": policies,
                "email_templates": emails,
                "notification_settings": notifs
            })))
        }
    }
}

/// POST /api/backup/global/apply-settings-profile
/// Apply a plaintext partial global settings profile (SuperAdmin only)
pub async fn apply_global_settings_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<SettingsProfileRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_global_backup_enabled(&state.pool).await?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;

    let profile = &body.profile;
    let dry_run = body.dry_run.unwrap_or(false);

    if !profile.is_object() {
        return Ok(Json(json!({ "success": false, "error": "Profile must be a JSON object" })));
    }

    let profile_obj = profile.as_object().unwrap();
    if profile_obj.is_empty() {
        return Ok(Json(json!({ "success": false, "error": "Profile is empty — nothing to apply" })));
    }

    let valid_sections = ["global_settings", "global_email_templates"];
    let unknown: Vec<&str> = profile_obj.keys()
        .filter(|k| !valid_sections.contains(&k.as_str()))
        .map(|k| k.as_str())
        .collect();
    if !unknown.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "error": format!("Unknown sections: {}. Valid sections: {}", unknown.join(", "), valid_sections.join(", "))
        })));
    }

    let mut tx = state.pool.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut updated = 0;

    // Apply global_settings
    if let Some(settings) = profile_obj.get("global_settings").and_then(|v| v.as_object()) {
        for (key, value) in settings {
            // Skip redacted values
            if value.as_str() == Some(REDACTED) { continue; }
            sqlx::query(
                r#"
                INSERT INTO global_settings (key, value, updated_by, updated_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE SET value = $2, updated_by = $3, updated_at = NOW()
                "#
            )
            .bind(key)
            .bind(value)
            .bind(auth.user_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated += 1;
        }
    }

    // Apply global_email_templates
    if let Some(templates) = profile_obj.get("global_email_templates").and_then(|v| v.as_array()) {
        for template in templates {
            let key = template.get("template_key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() { continue; }
            sqlx::query(
                r#"
                UPDATE email_templates
                SET subject = COALESCE($2, subject),
                    body_html = COALESCE($3, body_html),
                    body_text = COALESCE($4, body_text),
                    updated_at = NOW()
                WHERE template_key = $1
                "#
            )
            .bind(key)
            .bind(template.get("subject").and_then(|v| v.as_str()))
            .bind(template.get("body_html").and_then(|v| v.as_str()))
            .bind(template.get("body_text").and_then(|v| v.as_str()))
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated += 1;
        }
    }

    if dry_run {
        let _ = tx.rollback().await;
        return Ok(Json(json!({
            "success": true,
            "dry_run": true,
            "settings_updated": updated,
            "sections": profile_obj.keys().collect::<Vec<_>>()
        })));
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit global settings profile: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_apply_global_profile",
        json!({ "sections": profile_obj.keys().collect::<Vec<&String>>(), "settings_updated": updated }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({
        "success": true,
        "settings_updated": updated,
        "sections_applied": profile_obj.keys().collect::<Vec<&String>>()
    })))
}

/// PUT /api/backup/global/toggle
/// Enable or disable global backups (SuperAdmin only)
pub async fn toggle_global_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<GlobalToggleRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;

    sqlx::query(
        r#"
        INSERT INTO global_settings (key, value, updated_by, updated_at)
        VALUES ('global_backup_enabled', $1, $2, NOW())
        ON CONFLICT (key) DO UPDATE SET value = $1, updated_by = $2, updated_at = NOW()
        "#
    )
    .bind(json!(body.enabled))
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_global_toggle",
        json!({ "enabled": body.enabled }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({ "success": true, "enabled": body.enabled })))
}

/// Check if global backup is enabled (defaults to true)
async fn check_global_backup_enabled(pool: &sqlx::PgPool) -> Result<(), StatusCode> {
    let row: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM global_settings WHERE key = 'global_backup_enabled'"
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match row {
        Some((val,)) if val == json!(false) => Err(StatusCode::FORBIDDEN),
        _ => Ok(()),
    }
}

/// GET /api/backup/global/status
/// Returns whether global backup is enabled
pub async fn global_backup_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let row: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM global_settings WHERE key = 'global_backup_enabled'"
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let enabled = match row {
        Some((val,)) => val != json!(false),
        None => true, // default enabled
    };

    Ok(Json(json!({ "enabled": enabled })))
}

/// POST /api/backup/global/import/preview
pub async fn preview_global_import(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_global_backup_enabled(&state.pool).await?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let encrypted: Value = serde_json::from_str(&body.data)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let plaintext = match decrypt_backup(&encrypted, &passphrase) {
        Ok(data) => data,
        Err(msg) => {
            return Ok(Json(json!({ "valid": false, "errors": [msg] })));
        }
    };

    let backup: Value = serde_json::from_slice(&plaintext)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let meta = backup.get("_meta").ok_or(StatusCode::BAD_REQUEST)?;
    let export_type = meta.get("export_type").and_then(|v| v.as_str()).unwrap_or("");
    if export_type != "global" {
        return Ok(Json(json!({ "valid": false, "errors": ["This is not a global settings backup"] })));
    }

    let mut changes = Vec::new();
    if let Some(new_settings) = backup.get("global_settings").and_then(|v| v.as_object()) {
        let current = collect_global_settings(&state.pool).await?;
        if let Some(cur_map) = current.as_object() {
            for (key, new_val) in new_settings {
                // Skip sensitive keys entirely
                if is_sensitive_key(key.as_str()) {
                    continue;
                }
                if let Some(cur_val) = cur_map.get(key) {
                    if cur_val != new_val {
                        changes.push(json!({ "field": key, "current": cur_val, "new": new_val }));
                    }
                } else {
                    changes.push(json!({ "field": key, "current": null, "new": new_val }));
                }
            }
        }
    }

    Ok(Json(json!({
        "valid": true,
        "meta": meta,
        "changes": changes,
        "change_count": changes.len()
    })))
}

/// POST /api/backup/global/import
pub async fn import_global(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_global_backup_enabled(&state.pool).await?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let encrypted: Value = serde_json::from_str(&body.data)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let plaintext = match decrypt_backup(&encrypted, &passphrase) {
        Ok(data) => data,
        Err(msg) => {
            return Ok(Json(json!({ "success": false, "error": msg })));
        }
    };

    let backup: Value = serde_json::from_slice(&plaintext)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut updated = 0;

    // Apply global settings (skip sensitive keys that should never be imported)
    if let Some(settings) = backup.get("global_settings").and_then(|v| v.as_object()) {
        for (key, value) in settings {
            if is_sensitive_key(key.as_str()) {
                continue;
            }
            sqlx::query(
                r#"
                INSERT INTO global_settings (key, value, updated_by, updated_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE SET value = $2, updated_by = $3, updated_at = NOW()
                "#
            )
            .bind(key)
            .bind(value)
            .bind(auth.user_id)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated += 1;
        }
    }

    // Apply global email templates
    if let Some(templates) = backup.get("global_email_templates").and_then(|v| v.as_array()) {
        for template in templates {
            let key = template.get("template_key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() { continue; }

            sqlx::query(
                r#"
                UPDATE email_templates
                SET subject = COALESCE($2, subject),
                    body_html = COALESCE($3, body_html),
                    body_text = COALESCE($4, body_text),
                    updated_at = NOW()
                WHERE template_key = $1
                "#
            )
            .bind(key)
            .bind(template.get("subject").and_then(|v| v.as_str()))
            .bind(template.get("body_html").and_then(|v| v.as_str()))
            .bind(template.get("body_text").and_then(|v| v.as_str()))
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    // Invalidate cache
    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_import_global",
        json!({ "settings_updated": updated }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({
        "success": true,
        "settings_updated": updated
    })))
}

// ============================================================================
// APPLY FUNCTIONS (per section)
// ============================================================================

async fn apply_tenant_core(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let obj = data.as_object().ok_or(StatusCode::BAD_REQUEST)?;
    let mut updated = Vec::new();

    // Build dynamic UPDATE query for non-redacted fields
    let updatable_fields = [
        "compliance_mode", "retention_policy_days", "mfa_required",
        "session_timeout_minutes", "public_sharing_enabled", "data_export_enabled",
        "blocked_extensions", "password_policy", "ip_restriction_mode",
        "ip_allowlist", "ip_blocklist", "storage_quota_bytes", "max_upload_size_bytes",
        "enable_totp", "enable_passkeys", "auth_methods", "approval_workflow_enabled",
        "smtp_host", "smtp_port", "smtp_username", "smtp_from", "smtp_secure",
    ];

    for field in &updatable_fields {
        if let Some(value) = obj.get(*field) {
            if value.as_str() == Some(REDACTED) { continue; }

            let query = format!("UPDATE tenants SET {} = $1 WHERE id = $2", field);
            match value {
                Value::String(s) => {
                    sqlx::query(&query).bind(s).bind(tenant_id)
                        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        sqlx::query(&query).bind(i as i32).bind(tenant_id)
                            .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    }
                }
                Value::Bool(b) => {
                    sqlx::query(&query).bind(b).bind(tenant_id)
                        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                _ => {
                    // JSONB or array fields
                    sqlx::query(&query).bind(value).bind(tenant_id)
                        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
            updated.push(*field);
        }
    }

    // Handle smtp_password separately (only if not redacted)
    if let Some(pwd) = obj.get("smtp_password").and_then(|v| v.as_str()) {
        if pwd != REDACTED {
            sqlx::query("UPDATE tenants SET smtp_password = $1 WHERE id = $2")
                .bind(pwd).bind(tenant_id)
                .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated.push("smtp_password");
        }
    }

    Ok(json!({ "fields_updated": updated, "count": updated.len() }))
}

async fn apply_departments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let depts = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;
    let mut created = 0;
    let mut updated = 0;

    for dept in depts {
        let name = dept.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
        let description = dept.get("description").and_then(|v| v.as_str());

        // Validate parent_id exists if provided
        if let Some(parent_id) = dept.get("parent_id").and_then(|v| v.as_str()) {
            if let Ok(pid) = parent_id.parse::<Uuid>() {
                let exists: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM departments WHERE id = $1 AND tenant_id = $2"
                )
                .bind(pid).bind(tenant_id)
                .fetch_optional(&mut **tx).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if exists.is_none() {
                    tracing::warn!("Skipping department '{}' — parent_id {} not found", name, parent_id);
                    continue;
                }
            }
        }

        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM departments WHERE name = $1 AND tenant_id = $2"
        )
        .bind(name).bind(tenant_id)
        .fetch_optional(&mut **tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some((id,)) = existing {
            sqlx::query("UPDATE departments SET description = COALESCE($1, description) WHERE id = $2")
                .bind(description).bind(id)
                .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated += 1;
        } else {
            sqlx::query("INSERT INTO departments (tenant_id, name, description) VALUES ($1, $2, $3)")
                .bind(tenant_id).bind(name).bind(description)
                .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            created += 1;
        }
    }

    Ok(json!({ "created": created, "updated": updated }))
}

async fn apply_roles(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let roles = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;
    let mut created = 0;
    let mut updated = 0;

    for role in roles {
        let name = role.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
        let description = role.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let base_role = role.get("base_role").and_then(|v| v.as_str()).unwrap_or("Employee");

        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM roles WHERE name = $1 AND tenant_id = $2"
        )
        .bind(name).bind(tenant_id)
        .fetch_optional(&mut **tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let role_id = if let Some((id,)) = existing {
            sqlx::query("UPDATE roles SET description = $1, base_role = $2, updated_at = NOW() WHERE id = $3")
                .bind(description).bind(base_role).bind(id)
                .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Clear existing permissions
            sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
                .bind(id)
                .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            updated += 1;
            id
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO roles (tenant_id, name, description, base_role) VALUES ($1, $2, $3, $4) RETURNING id"
            )
            .bind(tenant_id).bind(name).bind(description).bind(base_role)
            .fetch_one(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            created += 1;
            id
        };

        // Add permissions (validated against known permission names)
        if let Some(perms) = role.get("permissions").and_then(|v| v.as_array()) {
            for perm in perms {
                let permission = perm.get("permission").and_then(|v| v.as_str()).unwrap_or("");
                let granted = perm.get("granted").and_then(|v| v.as_bool()).unwrap_or(true);
                if !permission.is_empty() {
                    if !VALID_PERMISSIONS.contains(&permission) {
                        tracing::warn!("Skipping unknown permission in import: {}", permission);
                        continue;
                    }
                    sqlx::query("INSERT INTO role_permissions (role_id, permission, granted) VALUES ($1, $2, $3)")
                        .bind(role_id).bind(permission).bind(granted)
                        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
        }
    }

    Ok(json!({ "created": created, "updated": updated }))
}

async fn apply_users(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let users = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;
    let mut created = 0;
    let mut updated = 0;

    for user in users {
        let email = user.get("email").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;

        // Basic email format validation
        if !email.contains('@') || !email.contains('.') || email.len() > 254 {
            tracing::warn!("Skipping user with invalid email in import: {}", email);
            continue;
        }

        let name = user.get("name").and_then(|v| v.as_str()).unwrap_or(email);
        let role = user.get("role").and_then(|v| v.as_str()).unwrap_or("Employee");
        let status = user.get("status").and_then(|v| v.as_str()).unwrap_or("active");
        let identity_provider = user.get("identity_provider").and_then(|v| v.as_str()).unwrap_or("local");

        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM users WHERE email = $1 AND tenant_id = $2"
        )
        .bind(email).bind(tenant_id)
        .fetch_optional(&mut **tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some((id,)) = existing {
            // Update existing user (don't touch password)
            sqlx::query(
                r#"UPDATE users SET name = $1, role = $2, status = $3,
                   identity_provider = $4, updated_at = NOW() WHERE id = $5"#
            )
            .bind(name).bind(role).bind(status).bind(identity_provider).bind(id)
            .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Resolve and set department if provided
            if let Some(dept_id) = user.get("department_id") {
                if !dept_id.is_null() {
                    sqlx::query("UPDATE users SET department_id = $1::uuid WHERE id = $2")
                        .bind(dept_id.as_str()).bind(id)
                        .execute(&mut **tx).await.ok(); // Best effort
                }
            }
            updated += 1;
        } else {
            // Create new user with random password (forced reset)
            let random_hash = format!("$argon2id$v=19$m=65536,t=3,p=1${}${}",
                nanoid::nanoid!(22), nanoid::nanoid!(43));

            sqlx::query(
                r#"INSERT INTO users (tenant_id, email, name, password_hash, role, status, identity_provider)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#
            )
            .bind(tenant_id).bind(email).bind(name).bind(&random_hash)
            .bind(role).bind(status).bind(identity_provider)
            .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            created += 1;
        }
    }

    Ok(json!({ "created": created, "updated": updated }))
}

async fn apply_audit_settings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let obj = data.as_object().ok_or(StatusCode::BAD_REQUEST)?;

    sqlx::query(
        r#"
        INSERT INTO audit_settings (tenant_id, log_logins, log_file_operations, log_user_changes,
            log_settings_changes, log_role_changes, retention_days)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (tenant_id) DO UPDATE SET
            log_logins = $2, log_file_operations = $3, log_user_changes = $4,
            log_settings_changes = $5, log_role_changes = $6, retention_days = $7,
            updated_at = NOW()
        "#
    )
    .bind(tenant_id)
    .bind(obj.get("log_logins").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("log_file_operations").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("log_user_changes").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("log_settings_changes").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("log_role_changes").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("retention_days").and_then(|v| v.as_i64()).unwrap_or(90) as i32)
    .execute(&mut **tx).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(json!({ "applied": true }))
}

async fn apply_virus_scan(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    if data.is_null() {
        return Ok(json!({ "skipped": true, "reason": "No virus scan settings in backup" }));
    }
    let obj = data.as_object().ok_or(StatusCode::BAD_REQUEST)?;

    sqlx::query(
        r#"
        INSERT INTO virus_scan_settings (tenant_id, enabled, file_types, max_file_size_mb,
            action_on_detect, notify_admin, notify_uploader, auto_suspend_uploader, suspend_threshold)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (tenant_id) DO UPDATE SET
            enabled = $2, file_types = $3, max_file_size_mb = $4,
            action_on_detect = $5, notify_admin = $6, notify_uploader = $7,
            auto_suspend_uploader = $8, suspend_threshold = $9
        "#
    )
    .bind(tenant_id)
    .bind(obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false))
    .bind(obj.get("file_types").unwrap_or(&Value::Null))
    .bind(obj.get("max_file_size_mb").and_then(|v| v.as_i64()).unwrap_or(100) as i32)
    .bind(obj.get("action_on_detect").and_then(|v| v.as_str()).unwrap_or("quarantine"))
    .bind(obj.get("notify_admin").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("notify_uploader").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("auto_suspend_uploader").and_then(|v| v.as_bool()).unwrap_or(false))
    .bind(obj.get("suspend_threshold").and_then(|v| v.as_i64()).unwrap_or(3) as i32)
    .execute(&mut **tx).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(json!({ "applied": true }))
}

async fn apply_ai_settings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    if data.is_null() {
        return Ok(json!({ "skipped": true }));
    }
    let obj = data.as_object().ok_or(StatusCode::BAD_REQUEST)?;

    // Don't import api_key if redacted
    let api_key = obj.get("api_key_encrypted")
        .and_then(|v| v.as_str())
        .filter(|s| *s != REDACTED);

    sqlx::query(
        r#"
        INSERT INTO tenant_ai_settings (tenant_id, enabled, provider, allowed_roles,
            monthly_token_limit, daily_request_limit)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (tenant_id) DO UPDATE SET
            enabled = $2, provider = $3, allowed_roles = $4,
            monthly_token_limit = $5, daily_request_limit = $6
        "#
    )
    .bind(tenant_id)
    .bind(obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false))
    .bind(obj.get("provider").and_then(|v| v.as_str()).unwrap_or("openai"))
    .bind(obj.get("allowed_roles").unwrap_or(&Value::Null))
    .bind(obj.get("monthly_token_limit").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(obj.get("daily_request_limit").and_then(|v| v.as_i64()).map(|v| v as i32))
    .execute(&mut **tx).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update API key separately if provided
    if let Some(key) = api_key {
        sqlx::query("UPDATE tenant_ai_settings SET api_key_encrypted = $1 WHERE tenant_id = $2")
            .bind(key).bind(tenant_id)
            .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(json!({ "applied": true }))
}

async fn apply_discord_settings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    if data.is_null() {
        return Ok(json!({ "skipped": true }));
    }
    let obj = data.as_object().ok_or(StatusCode::BAD_REQUEST)?;

    let webhook = obj.get("webhook_url_encrypted")
        .and_then(|v| v.as_str())
        .filter(|s| *s != REDACTED);

    sqlx::query(
        r#"
        INSERT INTO tenant_discord_settings (tenant_id, enabled, notify_on_upload,
            notify_on_share, notify_on_comment, notify_on_request, channel_id, thread_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (tenant_id) DO UPDATE SET
            enabled = $2, notify_on_upload = $3, notify_on_share = $4,
            notify_on_comment = $5, notify_on_request = $6, channel_id = $7, thread_id = $8
        "#
    )
    .bind(tenant_id)
    .bind(obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false))
    .bind(obj.get("notify_on_upload").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("notify_on_share").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("notify_on_comment").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("notify_on_request").and_then(|v| v.as_bool()).unwrap_or(true))
    .bind(obj.get("channel_id").and_then(|v| v.as_str()))
    .bind(obj.get("thread_id").and_then(|v| v.as_str()))
    .execute(&mut **tx).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(url) = webhook {
        sqlx::query("UPDATE tenant_discord_settings SET webhook_url_encrypted = $1 WHERE tenant_id = $2")
            .bind(url).bind(tenant_id)
            .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(json!({ "applied": true }))
}

async fn apply_approval_policies(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let policies = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;

    // Replace all policies for this tenant
    sqlx::query("DELETE FROM approval_policies WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for policy in policies {
        let name = policy.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let scope = policy.get("scope").and_then(|v| v.as_str()).unwrap_or("all");
        let scope_value = policy.get("scope_value").and_then(|v| v.as_str());
        let required = policy.get("required_approvals").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let active = policy.get("is_active").and_then(|v| v.as_bool()).unwrap_or(true);

        sqlx::query(
            "INSERT INTO approval_policies (tenant_id, name, scope, scope_value, required_approvals, is_active) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(tenant_id).bind(name).bind(scope).bind(scope_value).bind(required).bind(active)
        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(json!({ "replaced": policies.len() }))
}

async fn apply_email_templates(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let templates = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;
    let mut applied = 0;

    for template in templates {
        let key = template.get("template_key").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() { continue; }

        sqlx::query(
            r#"
            INSERT INTO tenant_email_templates (tenant_id, template_key, subject, body_html, body_text)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (tenant_id, template_key) DO UPDATE SET
                subject = COALESCE($3, tenant_email_templates.subject),
                body_html = COALESCE($4, tenant_email_templates.body_html),
                body_text = COALESCE($5, tenant_email_templates.body_text),
                updated_at = NOW()
            "#
        )
        .bind(tenant_id).bind(key)
        .bind(template.get("subject").and_then(|v| v.as_str()))
        .bind(template.get("body_html").and_then(|v| v.as_str()))
        .bind(template.get("body_text").and_then(|v| v.as_str()))
        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        applied += 1;
    }

    Ok(json!({ "applied": applied }))
}

async fn apply_notification_settings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    data: &Value,
) -> Result<Value, StatusCode> {
    let settings = data.as_array().ok_or(StatusCode::BAD_REQUEST)?;

    // Replace all tenant notification settings
    sqlx::query("DELETE FROM tenant_notification_settings WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for setting in settings {
        let event_type = setting.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
        if event_type.is_empty() { continue; }

        sqlx::query(
            r#"
            INSERT INTO tenant_notification_settings (tenant_id, event_type, role, enabled,
                email_enforced, in_app_enforced, default_email, default_in_app)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(tenant_id)
        .bind(event_type)
        .bind(setting.get("role").and_then(|v| v.as_str()))
        .bind(setting.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true))
        .bind(setting.get("email_enforced").and_then(|v| v.as_bool()).unwrap_or(false))
        .bind(setting.get("in_app_enforced").and_then(|v| v.as_bool()).unwrap_or(false))
        .bind(setting.get("default_email").and_then(|v| v.as_bool()).unwrap_or(true))
        .bind(setting.get("default_in_app").and_then(|v| v.as_bool()).unwrap_or(true))
        .execute(&mut **tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(json!({ "replaced": settings.len() }))
}

// ============================================================================
// SECTION COUNTS ENDPOINT
// ============================================================================

/// GET /api/backup/section-counts
/// Returns record counts per section for count badges in UI
pub async fn section_counts(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let tid = auth.tenant_id;

    let users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let departments: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM departments WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let roles: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM roles WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let files: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files_metadata WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let audit_logs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let approval_policies: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approval_policies WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let approval_requests: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approval_requests WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let oidc: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tenant_oidc_providers WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let saml: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tenant_saml_providers WHERE tenant_id = $1")
        .bind(tid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "users": users.0,
        "departments": departments.0,
        "roles": roles.0,
        "file_metadata": files.0,
        "audit_logs": audit_logs.0,
        "approval_policies": approval_policies.0,
        "approval_history": approval_requests.0,
        "sso_oidc": oidc.0,
        "sso_saml": saml.0,
    })))
}

// ============================================================================
// SAVE-TO-STORAGE ENDPOINTS
// ============================================================================

/// POST /api/backup/save
/// Export backup and save to storage backend (not browser download)
pub async fn save_backup_to_storage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Query(params): Query<ExportParams>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    check_backup_enabled(&state.pool, auth.tenant_id, &auth.role).await?;
    let _permit = check_backup_infra(&state)?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let include_secrets = params.include_secrets.unwrap_or(false);
    if include_secrets && auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let start = std::time::Instant::now();

    // Build backup payload (reuse existing collect logic)
    let (encrypted_bytes, filename, sections) = build_backup_payload(
        &state, auth.tenant_id, auth.user_id, include_secrets, &params, &passphrase,
    ).await.map_err(|e| {
        state.backup_circuit_breaker.record_failure();
        e
    })?;

    // Save to storage
    let storage_path = format!("_backups/{}", filename);
    let size_bytes = encrypted_bytes.len() as i64;

    state.storage.upload(&storage_path, encrypted_bytes.clone()).await.map_err(|e| {
        tracing::error!("Failed to save backup to storage: {:?}", e);
        state.backup_circuit_breaker.record_failure();
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let duration_ms = start.elapsed().as_millis() as i32;

    // Record in backup_history
    let record: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO backup_history (tenant_id, filename, storage_path, size_bytes, sections,
            is_auto_backup, status, duration_ms, created_by)
        VALUES ($1, $2, $3, $4, $5, false, 'completed', $6, $7)
        RETURNING id
        "#
    )
    .bind(auth.tenant_id)
    .bind(&filename)
    .bind(&storage_path)
    .bind(size_bytes)
    .bind(json!(sections))
    .bind(duration_ms)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to record backup history: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    state.backup_circuit_breaker.record_success();

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_save",
        json!({
            "filename": filename,
            "size_bytes": size_bytes,
            "duration_ms": duration_ms,
            "sections": sections,
        }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({
        "success": true,
        "id": record.0,
        "filename": filename,
        "size_bytes": size_bytes,
        "duration_ms": duration_ms,
    })))
}

/// Build backup payload — shared by export-to-browser, save-to-storage, and auto-backup.
/// Returns (encrypted_bytes, filename, sections_list).
async fn build_backup_payload(
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
    include_secrets: bool,
    params: &ExportParams,
    passphrase: &str,
) -> Result<(Vec<u8>, String, Vec<String>), StatusCode> {
    let valid_sections: &[&str] = &[
        "tenant_core", "users", "departments", "roles",
        "settings_audit", "settings_virus_scan", "settings_ai", "settings_discord",
        "sso_oidc", "sso_saml", "sso_mappings", "sso_identities",
        "approval_policies", "email_templates", "notification_settings",
    ];
    let valid_optional: &[&str] = &[
        "file_metadata", "audit_logs", "approval_history",
    ];
    let sections: Vec<String> = params.sections
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| valid_sections.iter().map(|s| s.to_string()).collect());

    for s in &sections {
        if !valid_sections.contains(&s.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let optional: Vec<String> = params.include_optional
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    for s in &optional {
        if !valid_optional.contains(&s.as_str()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let audit_days = params.audit_days.unwrap_or(90);
    let file_limit = params.file_limit.unwrap_or(50000);
    let approval_days = params.approval_days.unwrap_or(90);

    let tenant_name: (String,) = sqlx::query_as("SELECT name FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut all_sections = sections.clone();
    all_sections.extend(optional.iter().cloned());

    let mut backup = json!({
        "_meta": {
            "format": "clovalink-backup",
            "format_version": 1,
            "clovalink_version": CURRENT_VERSION,
            "export_type": "tenant",
            "tenant_id": tenant_id.to_string(),
            "tenant_name": tenant_name.0,
            "exported_at": Utc::now().to_rfc3339(),
            "exported_by": user_id.to_string(),
            "include_secrets": include_secrets,
            "sections": &all_sections
        }
    });

    let backup_map = backup.as_object_mut().unwrap();

    for section in &sections {
        let value = match section.as_str() {
            "tenant_core" => collect_tenant_core(&state.pool, tenant_id, include_secrets).await?,
            "users" => collect_users(&state.pool, tenant_id, include_secrets).await?,
            "departments" => collect_departments(&state.pool, tenant_id).await?,
            "roles" => collect_roles(&state.pool, tenant_id).await?,
            "settings_audit" => collect_audit_settings(&state.pool, tenant_id).await?,
            "settings_virus_scan" => collect_virus_scan(&state.pool, tenant_id).await?,
            "settings_ai" => collect_ai_settings(&state.pool, tenant_id, include_secrets).await?,
            "settings_discord" => collect_discord_settings(&state.pool, tenant_id, include_secrets).await?,
            "sso_oidc" => collect_sso_oidc(&state.pool, tenant_id, include_secrets).await?,
            "sso_saml" => collect_sso_saml(&state.pool, tenant_id, include_secrets).await?,
            "sso_mappings" => collect_sso_mappings(&state.pool, tenant_id).await?,
            "sso_identities" => collect_sso_identities(&state.pool, tenant_id).await?,
            "approval_policies" => collect_approval_policies(&state.pool, tenant_id).await?,
            "email_templates" => collect_email_templates(&state.pool, tenant_id).await?,
            "notification_settings" => collect_notification_settings(&state.pool, tenant_id).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    for section in &optional {
        let value = match section.as_str() {
            "file_metadata" => collect_file_metadata(&state.pool, tenant_id, file_limit).await?,
            "audit_logs" => collect_audit_logs(&state.pool, tenant_id, audit_days).await?,
            "approval_history" => collect_approval_history(&state.pool, tenant_id, approval_days).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    let plaintext = serde_json::to_vec(&backup)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = encrypt_backup(&plaintext, passphrase)?;
    let encrypted_bytes = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = format!(
        "clovalink-backup-{}-{}-{:06x}.clovalink.json",
        tenant_name.0.to_lowercase().replace(' ', "-"),
        Utc::now().format("%Y%m%d-%H%M%S"),
        rand::random::<u32>() & 0xFFFFFF
    );

    Ok((encrypted_bytes, filename, all_sections))
}

/// GET /api/backup/saved
/// List saved backups for this tenant
pub async fn list_saved_backups(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<BackupListParams>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let is_global = params.mode.as_deref() == Some("global");
    if is_global && auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let rows: Vec<(Uuid, String, String, i64, Value, bool, String, Option<String>, Option<i32>, Option<Uuid>, chrono::DateTime<Utc>)> = if is_global {
        sqlx::query_as(
            r#"
            SELECT id, filename, storage_path, size_bytes, sections, is_auto_backup,
                   status, error_message, duration_ms, created_by, created_at
            FROM backup_history
            WHERE tenant_id IS NULL
            ORDER BY created_at DESC
            LIMIT 50
            "#
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        sqlx::query_as(
            r#"
            SELECT id, filename, storage_path, size_bytes, sections, is_auto_backup,
                   status, error_message, duration_ms, created_by, created_at
            FROM backup_history
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT 50
            "#
        )
        .bind(auth.tenant_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let backups: Vec<Value> = rows.into_iter().map(|r| json!({
        "id": r.0,
        "filename": r.1,
        "storage_path": r.2,
        "size_bytes": r.3,
        "sections": r.4,
        "is_auto_backup": r.5,
        "status": r.6,
        "error_message": r.7,
        "duration_ms": r.8,
        "created_by": r.9,
        "created_at": r.10,
    })).collect();

    Ok(Json(json!(backups)))
}

/// GET /api/backup/saved/:id/download
/// Download a saved backup from storage
pub async fn download_saved_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Response<Body>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let row: Option<(String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT filename, storage_path, tenant_id FROM backup_history WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (filename, storage_path, backup_tenant_id) = row.ok_or(StatusCode::NOT_FOUND)?;

    // Authorization: global backups require SuperAdmin, tenant backups require matching tenant
    match backup_tenant_id {
        None => {
            if auth.role != "SuperAdmin" { return Err(StatusCode::FORBIDDEN); }
        }
        Some(tid) => {
            if tid != auth.tenant_id { return Err(StatusCode::NOT_FOUND); }
        }
    }

    let data = state.storage.download(&storage_path).await.map_err(|e| {
        tracing::error!("Failed to download backup from storage: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(data))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// DELETE /api/backup/saved/:id
/// Delete a saved backup from storage and history
pub async fn delete_saved_backup(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT storage_path, tenant_id FROM backup_history WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (storage_path, backup_tenant_id) = row.ok_or(StatusCode::NOT_FOUND)?;

    // Authorization: global backups require SuperAdmin, tenant backups require matching tenant
    match backup_tenant_id {
        None => {
            if auth.role != "SuperAdmin" { return Err(StatusCode::FORBIDDEN); }
        }
        Some(tid) => {
            if tid != auth.tenant_id { return Err(StatusCode::NOT_FOUND); }
        }
    }

    // Delete from storage (best effort)
    let _ = state.storage.delete(&storage_path).await;

    // Delete from history
    sqlx::query("DELETE FROM backup_history WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_delete",
        json!({ "backup_id": id, "storage_path": storage_path }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({ "success": true })))
}

// ============================================================================
// HEALTH + METRICS ENDPOINTS
// ============================================================================

/// GET /api/backup/health
/// Circuit breaker state
pub async fn backup_health(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let cb = &state.backup_circuit_breaker;
    let state_str = match cb.state() {
        CircuitState::Closed => "closed",
        CircuitState::Open => "open",
        CircuitState::HalfOpen => "half_open",
    };

    Ok(Json(json!({
        "state": state_str,
        "failure_count": cb.metrics().failure_count,
        "master_key_configured": is_master_key_configured(),
    })))
}

/// GET /api/backup/metrics
/// Backup performance metrics (SuperAdmin only)
pub async fn backup_metrics(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let cb = &state.backup_circuit_breaker;
    let cb_state = match cb.state() {
        CircuitState::Closed => "closed",
        CircuitState::Open => "open",
        CircuitState::HalfOpen => "half_open",
    };

    let max_concurrent: usize = std::env::var("BACKUP_MAX_CONCURRENT").ok()
        .and_then(|v| v.parse().ok()).unwrap_or(2);
    let available = state.backup_semaphore.available_permits();

    // Aggregate stats from backup_history
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backup_history")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let auto_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backup_history WHERE is_auto_backup = true")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let failed_24h: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM backup_history WHERE status = 'failed' AND created_at > NOW() - interval '24 hours'"
    ).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_storage: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM backup_history WHERE status = 'completed'"
    ).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let last_backup: Option<(Option<i32>, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT duration_ms, created_at FROM backup_history ORDER BY created_at DESC LIMIT 1"
    ).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Per-tenant breakdown
    let by_tenant: Vec<(String, i64, Option<chrono::DateTime<Utc>>, bool)> = sqlx::query_as(
        r#"
        SELECT t.name, COUNT(bh.id), MAX(bh.created_at),
               COALESCE(t.auto_backup_enabled, false)
        FROM tenants t
        LEFT JOIN backup_history bh ON bh.tenant_id = t.id
        WHERE t.status = 'active'
        GROUP BY t.id, t.name, t.auto_backup_enabled
        ORDER BY t.name
        "#
    ).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tenant_list: Vec<Value> = by_tenant.into_iter().map(|(name, count, last, auto)| json!({
        "tenant_name": name,
        "backup_count": count,
        "last_backup": last,
        "auto_enabled": auto,
    })).collect();

    Ok(Json(json!({
        "circuit_breaker": { "state": cb_state, "failure_count": cb.metrics().failure_count },
        "concurrency": { "max": max_concurrent, "available": available, "active": max_concurrent - available },
        "total_backups": total.0,
        "total_auto_backups": auto_count.0,
        "total_manual_backups": total.0 - auto_count.0,
        "last_backup_at": last_backup.as_ref().map(|r| r.1),
        "last_backup_duration_ms": last_backup.as_ref().and_then(|r| r.0),
        "failed_backups_24h": failed_24h.0,
        "total_storage_bytes": total_storage.and_then(|r| r.0).unwrap_or(0),
        "by_tenant": tenant_list,
    })))
}

// ============================================================================
// BACKUP SCHEDULER
// ============================================================================

/// Background task that polls for tenants with auto-backup enabled and runs
/// scheduled backups. Uses Redis distributed lock to prevent duplicate runs
/// across instances.
pub async fn start_backup_scheduler(
    pool: sqlx::PgPool,
    storage: Arc<dyn clovalink_storage::Storage>,
    circuit_breaker: Arc<clovalink_core::circuit_breaker::CircuitBreaker>,
    semaphore: Arc<tokio::sync::Semaphore>,
    redis_url: String,
) {
    tracing::info!("Backup scheduler started");

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

    loop {
        interval.tick().await;

        // Skip if circuit breaker is open
        if !circuit_breaker.allow_request() {
            tracing::debug!("Backup scheduler: circuit breaker open, skipping cycle");
            continue;
        }

        // Acquire distributed lock via Redis
        let lock_acquired = match acquire_scheduler_lock(&redis_url).await {
            Ok(acquired) => acquired,
            Err(e) => {
                tracing::debug!("Backup scheduler: failed to acquire lock: {:?}", e);
                continue;
            }
        };
        if !lock_acquired {
            continue;
        }

        // Find tenants with auto-backup due
        let due_tenants = match find_due_tenants(&pool).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Backup scheduler: failed to find due tenants: {:?}", e);
                continue;
            }
        };

        // Check for global auto-backup
        if let Err(e) = check_and_run_global_auto_backup(
            &pool, &storage, &circuit_breaker, &semaphore,
        ).await {
            tracing::debug!("Global auto-backup check: {:?}", e);
        }

        if due_tenants.is_empty() {
            continue;
        }

        tracing::info!("Backup scheduler: {} tenants due for backup", due_tenants.len());

        // Process at most 5 per cycle
        let batch: Vec<_> = due_tenants.into_iter().take(5).collect();

        for (tenant_id, tenant_name, _cron_expr, retention_count) in batch {
            // Acquire semaphore (shared with manual backups)
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    tracing::info!("Backup scheduler: semaphore full, deferring remaining tenants");
                    break;
                }
            };

            // Random jitter 0-30 seconds
            let jitter_ms = rand::random::<u64>() % 30_000;
            tokio::time::sleep(std::time::Duration::from_millis(jitter_ms)).await;

            let pool_clone = pool.clone();
            let storage_clone = storage.clone();
            let cb_clone = circuit_breaker.clone();

            let result = run_auto_backup(
                &pool_clone, &storage_clone, &cb_clone,
                tenant_id, &tenant_name,
            ).await;

            match result {
                Ok((size, duration_ms)) => {
                    tracing::info!(
                        "Auto-backup completed for '{}': {}KB in {}ms",
                        tenant_name, size / 1024, duration_ms
                    );
                    // Enforce retention: delete oldest beyond limit
                    let _ = enforce_retention(&pool_clone, &storage_clone, tenant_id, retention_count).await;
                }
                Err(e) => {
                    tracing::error!("Auto-backup failed for '{}': {:?}", tenant_name, e);
                }
            }

            drop(permit);
        }
    }
}

/// Acquire a distributed lock via Redis SET NX EX
async fn acquire_scheduler_lock(redis_url: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result: Option<String> = redis::cmd("SET")
        .arg("clovalink:backup:scheduler")
        .arg("locked")
        .arg("NX")
        .arg("EX")
        .arg(55) // 55 second TTL (less than 60s poll interval)
        .query_async(&mut conn)
        .await?;
    Ok(result.is_some())
}

/// Find tenants with auto-backup enabled that are due for a backup
async fn find_due_tenants(
    pool: &sqlx::PgPool,
) -> Result<Vec<(Uuid, String, String, i32)>, sqlx::Error> {
    // Get all tenants with auto-backup enabled
    let tenants: Vec<(Uuid, String, String, i32)> = sqlx::query_as(
        r#"
        SELECT id, name,
               COALESCE(auto_backup_cron, '0 2 * * 0'),
               COALESCE(auto_backup_retention_count, 5)
        FROM tenants
        WHERE COALESCE(auto_backup_enabled, false) = true
          AND COALESCE(backup_enabled, true) = true
          AND status = 'active'
        ORDER BY id
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut due = Vec::new();
    let now = Utc::now();

    for (tenant_id, name, cron_expr, retention) in tenants {
        // Parse cron expression
        let schedule = match normalize_cron(&cron_expr).parse::<cron::Schedule>() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Invalid cron '{}' for tenant '{}': {:?}", cron_expr, name, e);
                continue;
            }
        };

        // Get last backup time for this tenant
        let last: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as(
            "SELECT created_at FROM backup_history WHERE tenant_id = $1 AND is_auto_backup = true ORDER BY created_at DESC LIMIT 1"
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let last_time = last.map(|r| r.0);

        // Check if a backup is due: find the most recent scheduled time before now
        // and check if it's after the last backup
        let should_run = if let Some(prev_time) = schedule.after(&(now - chrono::Duration::hours(25))).take(1).next() {
            if prev_time <= now {
                match last_time {
                    Some(lt) => prev_time > lt,
                    None => true, // Never backed up
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_run {
            due.push((tenant_id, name, cron_expr, retention));
        }
    }

    Ok(due)
}

/// Run an automatic backup for a tenant
async fn run_auto_backup(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn clovalink_storage::Storage>,
    circuit_breaker: &Arc<clovalink_core::circuit_breaker::CircuitBreaker>,
    tenant_id: Uuid,
    tenant_name: &str,
) -> Result<(i64, i32), StatusCode> {
    let start = std::time::Instant::now();

    // Auto-backups use system passphrase from global_settings
    let passphrase = get_or_create_auto_passphrase(pool).await?;

    // Core sections only for auto-backup (no large optional data)
    let default_sections = vec![
        "tenant_core", "users", "departments", "roles",
        "settings_audit", "settings_virus_scan", "settings_ai", "settings_discord",
        "sso_oidc", "sso_saml", "sso_mappings", "sso_identities",
        "approval_policies", "email_templates", "notification_settings",
    ];
    let sections: Vec<String> = default_sections.iter().map(|s| s.to_string()).collect();

    let mut backup = json!({
        "_meta": {
            "format": "clovalink-backup",
            "format_version": 1,
            "clovalink_version": CURRENT_VERSION,
            "export_type": "tenant",
            "tenant_id": tenant_id.to_string(),
            "tenant_name": tenant_name,
            "exported_at": Utc::now().to_rfc3339(),
            "exported_by": "system-auto-backup",
            "include_secrets": false,
            "sections": &sections
        }
    });

    let backup_map = backup.as_object_mut().unwrap();

    for section in &sections {
        let value = match section.as_str() {
            "tenant_core" => collect_tenant_core(pool, tenant_id, false).await?,
            "users" => collect_users(pool, tenant_id, false).await?,
            "departments" => collect_departments(pool, tenant_id).await?,
            "roles" => collect_roles(pool, tenant_id).await?,
            "settings_audit" => collect_audit_settings(pool, tenant_id).await?,
            "settings_virus_scan" => collect_virus_scan(pool, tenant_id).await?,
            "settings_ai" => collect_ai_settings(pool, tenant_id, false).await?,
            "settings_discord" => collect_discord_settings(pool, tenant_id, false).await?,
            "sso_oidc" => collect_sso_oidc(pool, tenant_id, false).await?,
            "sso_saml" => collect_sso_saml(pool, tenant_id, false).await?,
            "sso_mappings" => collect_sso_mappings(pool, tenant_id).await?,
            "sso_identities" => collect_sso_identities(pool, tenant_id).await?,
            "approval_policies" => collect_approval_policies(pool, tenant_id).await?,
            "email_templates" => collect_email_templates(pool, tenant_id).await?,
            "notification_settings" => collect_notification_settings(pool, tenant_id).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    let plaintext = serde_json::to_vec(&backup)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = encrypt_backup(&plaintext, &passphrase)?;
    let encrypted_bytes = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = format!(
        "clovalink-auto-backup-{}-{}-{:06x}.clovalink.json",
        tenant_name.to_lowercase().replace(' ', "-"),
        Utc::now().format("%Y%m%d-%H%M%S"),
        rand::random::<u32>() & 0xFFFFFF
    );
    let storage_path = format!("_backups/{}", filename);
    let size_bytes = encrypted_bytes.len() as i64;

    storage.upload(&storage_path, encrypted_bytes).await.map_err(|e| {
        tracing::error!("Auto-backup storage upload failed: {:?}", e);
        circuit_breaker.record_failure();
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let duration_ms = start.elapsed().as_millis() as i32;

    circuit_breaker.record_success();

    // Record in backup_history
    sqlx::query(
        r#"
        INSERT INTO backup_history (tenant_id, filename, storage_path, size_bytes, sections,
            is_auto_backup, status, duration_ms)
        VALUES ($1, $2, $3, $4, $5, true, 'completed', $6)
        "#
    )
    .bind(tenant_id)
    .bind(&filename)
    .bind(&storage_path)
    .bind(size_bytes)
    .bind(json!(sections))
    .bind(duration_ms)
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    log_backup_audit(
        pool, tenant_id, Uuid::nil(),
        "backup_auto",
        json!({
            "filename": filename,
            "size_bytes": size_bytes,
            "duration_ms": duration_ms,
        }),
        "system",
    ).await;

    Ok((size_bytes, duration_ms))
}

/// Get or create the system auto-backup passphrase
async fn get_or_create_auto_passphrase(pool: &sqlx::PgPool) -> Result<String, StatusCode> {
    let existing: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM global_settings WHERE key = 'auto_backup_passphrase'"
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((val,)) = existing {
        if let Some(stored) = val.as_str() {
            // Decrypt from at-rest encryption (handles both encrypted and legacy plaintext)
            let passphrase = decrypt_passphrase_at_rest(stored).map_err(|e| {
                tracing::error!("Failed to decrypt auto-backup passphrase: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // If stored as plaintext and master key is available, re-encrypt in place
            if !stored.starts_with(ENCRYPTED_PREFIX) && std::env::var("BACKUP_MASTER_KEY").is_ok() {
                let encrypted = encrypt_passphrase_at_rest(&passphrase);
                let _ = sqlx::query(
                    "UPDATE global_settings SET value = $1, updated_at = NOW() WHERE key = 'auto_backup_passphrase'"
                )
                .bind(json!(encrypted))
                .execute(pool)
                .await;
            }

            return Ok(passphrase);
        }
    }

    // Generate a random 32-char passphrase
    let passphrase = nanoid::nanoid!(32);

    // Encrypt before storing
    let stored_value = encrypt_passphrase_at_rest(&passphrase);

    sqlx::query(
        r#"
        INSERT INTO global_settings (key, value, updated_at)
        VALUES ('auto_backup_passphrase', $1, NOW())
        ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()
        "#
    )
    .bind(json!(stored_value))
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(passphrase)
}

/// Enforce retention: delete oldest auto-backups beyond the limit
async fn enforce_retention(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn clovalink_storage::Storage>,
    tenant_id: Uuid,
    retention_count: i32,
) -> Result<(), StatusCode> {
    let old_backups: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, storage_path FROM backup_history
        WHERE tenant_id = $1 AND is_auto_backup = true AND status = 'completed'
        ORDER BY created_at DESC
        OFFSET $2
        "#
    )
    .bind(tenant_id)
    .bind(retention_count)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for (id, path) in old_backups {
        let _ = storage.delete(&path).await;
        let _ = sqlx::query("DELETE FROM backup_history WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await;
        tracing::info!("Retention cleanup: deleted backup {} for tenant {}", id, tenant_id);
    }

    Ok(())
}

// ============================================================================
// GLOBAL BACKUP: SAVE-TO-STORAGE, SCHEDULE, AUTO-BACKUP
// ============================================================================

/// Check if global auto-backup is due and run it
async fn check_and_run_global_auto_backup(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn clovalink_storage::Storage>,
    circuit_breaker: &Arc<clovalink_core::circuit_breaker::CircuitBreaker>,
    semaphore: &Arc<tokio::sync::Semaphore>,
) -> Result<(), StatusCode> {
    // Check if enabled
    let enabled: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM global_settings WHERE key = 'global_auto_backup_enabled'"
    ).fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_enabled = match enabled {
        Some((val,)) => val.as_bool().unwrap_or(false) || val.as_str() == Some("true"),
        None => false,
    };
    if !is_enabled { return Ok(()); }

    // Check if global backup is not disabled
    check_global_backup_enabled(pool).await?;

    // Get cron expression
    let cron_row: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM global_settings WHERE key = 'global_auto_backup_cron'"
    ).fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cron_expr = cron_row
        .and_then(|(v,)| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "0 3 * * 0".to_string());

    let schedule = match normalize_cron(&cron_expr).parse::<cron::Schedule>() {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    // Get last global auto-backup time
    let last: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as(
        "SELECT created_at FROM backup_history WHERE tenant_id IS NULL AND is_auto_backup = true ORDER BY created_at DESC LIMIT 1"
    ).fetch_optional(pool).await.unwrap_or(None);

    let now = Utc::now();
    let should_run = if let Some(prev_time) = schedule.after(&(now - chrono::Duration::hours(25))).take(1).next() {
        if prev_time <= now {
            match last {
                Some((lt,)) => prev_time > lt,
                None => true,
            }
        } else {
            false
        }
    } else {
        false
    };

    if !should_run { return Ok(()); }

    let permit = semaphore.clone().try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;

    tracing::info!("Running global auto-backup");

    let result = run_auto_backup_global(pool, storage, circuit_breaker).await;

    match result {
        Ok((size, duration_ms)) => {
            tracing::info!("Global auto-backup completed: {}KB in {}ms", size / 1024, duration_ms);
            // Enforce retention
            let retention_row: Option<(Value,)> = sqlx::query_as(
                "SELECT value FROM global_settings WHERE key = 'global_auto_backup_retention_count'"
            ).fetch_optional(pool).await.unwrap_or(None);
            let retention = retention_row
                .and_then(|(v,)| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .unwrap_or(5) as i32;
            let _ = enforce_global_retention(pool, storage, retention).await;
        }
        Err(e) => {
            tracing::error!("Global auto-backup failed: {:?}", e);
        }
    }

    drop(permit);
    Ok(())
}

/// Build global backup payload — returns (encrypted_bytes, filename, sections_list)
async fn build_global_backup_payload(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    sections: &[String],
    passphrase: &str,
) -> Result<(Vec<u8>, String, Vec<String>), StatusCode> {
    let valid = ["global_settings", "global_email_templates"];
    let selected: Vec<String> = if sections.is_empty() {
        valid.iter().map(|s| s.to_string()).collect()
    } else {
        sections.iter().filter(|s| valid.contains(&s.as_str())).cloned().collect()
    };

    let mut backup = json!({
        "_meta": {
            "format": "clovalink-backup",
            "format_version": 1,
            "clovalink_version": CURRENT_VERSION,
            "export_type": "global",
            "exported_at": Utc::now().to_rfc3339(),
            "exported_by": user_id.to_string(),
            "sections": &selected
        }
    });

    let backup_map = backup.as_object_mut().unwrap();

    for section in &selected {
        let value = match section.as_str() {
            "global_settings" => strip_sensitive_keys(collect_global_settings(pool).await?),
            "global_email_templates" => collect_global_email_templates(pool).await?,
            _ => continue,
        };
        backup_map.insert(section.clone(), value);
    }

    let plaintext = serde_json::to_vec(&backup)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = encrypt_backup(&plaintext, passphrase)?;
    let encrypted_bytes = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = format!(
        "clovalink-global-backup-{}-{:06x}.clovalink.json",
        Utc::now().format("%Y%m%d-%H%M%S"),
        rand::random::<u32>() & 0xFFFFFF
    );

    Ok((encrypted_bytes, filename, selected))
}

/// POST /api/backup/global/save
/// Save a global backup to storage
pub async fn save_global_backup_to_storage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Query(params): Query<ExportParams>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    check_global_backup_enabled(&state.pool).await?;
    let _permit = check_backup_infra(&state)?;

    if is_backup_locked_out(&state.pool, auth.user_id).await? {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    let passphrase = get_passphrase(&headers)?;
    if passphrase.len() < 12 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let start = std::time::Instant::now();

    let sections: Vec<String> = params.sections
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let (encrypted_bytes, filename, selected_sections) = build_global_backup_payload(
        &state.pool, auth.user_id, &sections, &passphrase,
    ).await.map_err(|e| {
        state.backup_circuit_breaker.record_failure();
        e
    })?;

    let storage_path = format!("_backups/{}", filename);
    let size_bytes = encrypted_bytes.len() as i64;

    state.storage.upload(&storage_path, encrypted_bytes).await.map_err(|e| {
        tracing::error!("Failed to save global backup to storage: {:?}", e);
        state.backup_circuit_breaker.record_failure();
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let duration_ms = start.elapsed().as_millis() as i32;

    let record: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO backup_history (tenant_id, filename, storage_path, size_bytes, sections,
            is_auto_backup, status, duration_ms, created_by)
        VALUES (NULL, $1, $2, $3, $4, false, 'completed', $5, $6)
        RETURNING id
        "#
    )
    .bind(&filename)
    .bind(&storage_path)
    .bind(size_bytes)
    .bind(json!(selected_sections))
    .bind(duration_ms)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to record global backup history: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    state.backup_circuit_breaker.record_success();

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_global_save",
        json!({
            "filename": filename,
            "size_bytes": size_bytes,
            "duration_ms": duration_ms,
            "sections": selected_sections,
        }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({
        "success": true,
        "id": record.0,
        "filename": filename,
        "size_bytes": size_bytes,
        "duration_ms": duration_ms,
    })))
}

/// GET /api/backup/global/schedule
pub async fn get_global_backup_schedule(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let rows: Vec<(String, Value)> = sqlx::query_as(
        "SELECT key, value FROM global_settings WHERE key LIKE 'global_auto_backup_%'"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut enabled = false;
    let mut cron = "0 3 * * 0".to_string();
    let mut retention = 5;

    for (key, val) in rows {
        match key.as_str() {
            "global_auto_backup_enabled" => {
                enabled = val.as_bool().unwrap_or(false)
                    || val.as_str().map(|s| s == "true").unwrap_or(false);
            }
            "global_auto_backup_cron" => {
                cron = val.as_str().unwrap_or("0 3 * * 0").to_string();
            }
            "global_auto_backup_retention_count" => {
                retention = val.as_i64().unwrap_or(
                    val.as_str().and_then(|s| s.parse().ok()).unwrap_or(5)
                ) as i32;
            }
            _ => {}
        }
    }

    Ok(Json(json!({
        "enabled": enabled,
        "cron": cron,
        "retention_count": retention
    })))
}

/// PUT /api/backup/global/schedule
pub async fn set_global_backup_schedule(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    headers: HeaderMap,
    Json(body): Json<GlobalScheduleRequest>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;

    if let Some(enabled) = body.enabled {
        if enabled && !is_master_key_configured() {
            return Ok(Json(json!({
                "success": false,
                "error": "BACKUP_MASTER_KEY must be configured to enable auto-backups. See deployment docs."
            })));
        }
        sqlx::query(
            "INSERT INTO global_settings (key, value, updated_by, updated_at) VALUES ('global_auto_backup_enabled', $1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = $1, updated_by = $2, updated_at = NOW()"
        )
        .bind(json!(enabled))
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    if let Some(ref cron_expr) = body.cron {
        // Validate cron expression
        if normalize_cron(&cron_expr).parse::<cron::Schedule>().is_err() {
            return Ok(Json(json!({ "success": false, "error": "Invalid cron expression" })));
        }
        sqlx::query(
            "INSERT INTO global_settings (key, value, updated_by, updated_at) VALUES ('global_auto_backup_cron', $1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = $1, updated_by = $2, updated_at = NOW()"
        )
        .bind(json!(cron_expr))
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    if let Some(retention) = body.retention_count {
        sqlx::query(
            "INSERT INTO global_settings (key, value, updated_by, updated_at) VALUES ('global_auto_backup_retention_count', $1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = $1, updated_by = $2, updated_at = NOW()"
        )
        .bind(json!(retention))
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    if let Some(ref cache) = state.cache {
        let _ = cache.delete(&clovalink_core::cache::keys::global_settings()).await;
    }

    log_backup_audit(
        &state.pool, auth.tenant_id, auth.user_id,
        "backup_global_schedule_update",
        json!({ "enabled": body.enabled, "cron": body.cron, "retention_count": body.retention_count }),
        auth.ip_address.as_deref().unwrap_or("unknown"),
    ).await;

    Ok(Json(json!({ "success": true })))
}

/// Run an automatic global backup
async fn run_auto_backup_global(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn clovalink_storage::Storage>,
    circuit_breaker: &Arc<clovalink_core::circuit_breaker::CircuitBreaker>,
) -> Result<(i64, i32), StatusCode> {
    let start = std::time::Instant::now();
    let passphrase = get_or_create_auto_passphrase(pool).await?;

    let sections = vec!["global_settings".to_string(), "global_email_templates".to_string()];

    let (encrypted_bytes, filename, selected) = build_global_backup_payload(
        pool, Uuid::nil(), &sections, &passphrase,
    ).await?;

    let storage_path = format!("_backups/{}", filename);
    let size_bytes = encrypted_bytes.len() as i64;

    storage.upload(&storage_path, encrypted_bytes).await.map_err(|e| {
        tracing::error!("Global auto-backup storage upload failed: {:?}", e);
        circuit_breaker.record_failure();
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let duration_ms = start.elapsed().as_millis() as i32;
    circuit_breaker.record_success();

    sqlx::query(
        r#"
        INSERT INTO backup_history (tenant_id, filename, storage_path, size_bytes, sections,
            is_auto_backup, status, duration_ms)
        VALUES (NULL, $1, $2, $3, $4, true, 'completed', $5)
        "#
    )
    .bind(&filename)
    .bind(&storage_path)
    .bind(size_bytes)
    .bind(json!(selected))
    .bind(duration_ms)
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    log_backup_audit(
        pool, Uuid::nil(), Uuid::nil(),
        "backup_global_auto",
        json!({ "filename": filename, "size_bytes": size_bytes, "duration_ms": duration_ms }),
        "system",
    ).await;

    Ok((size_bytes, duration_ms))
}

/// Enforce retention for global auto-backups
async fn enforce_global_retention(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn clovalink_storage::Storage>,
    retention_count: i32,
) -> Result<(), StatusCode> {
    let old_backups: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, storage_path FROM backup_history
        WHERE tenant_id IS NULL AND is_auto_backup = true AND status = 'completed'
        ORDER BY created_at DESC
        OFFSET $1
        "#
    )
    .bind(retention_count)
    .fetch_all(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for (id, path) in old_backups {
        let _ = storage.delete(&path).await;
        let _ = sqlx::query("DELETE FROM backup_history WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await;
        tracing::info!("Global retention cleanup: deleted backup {}", id);
    }

    Ok(())
}
