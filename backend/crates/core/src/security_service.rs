//! Security Service - Detects and creates security alerts for unusual activity
//!
//! This module provides functions to:
//! - Track login attempts and detect brute force attacks
//! - Detect logins from new IP addresses
//! - Monitor for permission escalation
//! - Track bulk downloads (potential data exfiltration)
//! - Monitor blocked extension upload attempts
//! - Detect excessive sharing patterns

use sqlx::{PgPool, Row};
use uuid::Uuid;
use serde_json::json;
use chrono::{Utc, Duration};
use crate::models::Tenant;
use crate::notification_service;

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlertSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertSeverity::Critical => "critical",
            AlertSeverity::High => "high",
            AlertSeverity::Medium => "medium",
            AlertSeverity::Low => "low",
        }
    }
}

/// Alert types that can be triggered
#[derive(Debug, Clone)]
pub enum AlertType {
    FailedLoginSpike,
    NewIpLogin,
    PermissionEscalation,
    SuspendedAccessAttempt,
    BulkDownload,
    BlockedExtensionAttempt,
    ExcessiveSharing,
    AccountLockout,
    PotentialTokenTheft,
    MalwareDetected,
    UserSuspendedMalware,
    BackupDecryptFailed,
    BackupBruteForce,
    BackupExportSecrets,
    PasswordConfirmFailed,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::FailedLoginSpike => "failed_login_spike",
            AlertType::NewIpLogin => "new_ip_login",
            AlertType::PermissionEscalation => "permission_escalation",
            AlertType::SuspendedAccessAttempt => "suspended_access_attempt",
            AlertType::BulkDownload => "bulk_download",
            AlertType::BlockedExtensionAttempt => "blocked_extension_attempt",
            AlertType::ExcessiveSharing => "excessive_sharing",
            AlertType::AccountLockout => "account_lockout",
            AlertType::PotentialTokenTheft => "potential_token_theft",
            AlertType::MalwareDetected => "malware_detected",
            AlertType::UserSuspendedMalware => "user_suspended_malware",
            AlertType::BackupDecryptFailed => "backup_decrypt_failed",
            AlertType::BackupBruteForce => "backup_brute_force",
            AlertType::BackupExportSecrets => "backup_export_secrets",
            AlertType::PasswordConfirmFailed => "password_confirm_failed",
        }
    }

    pub fn default_severity(&self) -> AlertSeverity {
        match self {
            AlertType::FailedLoginSpike => AlertSeverity::High,
            AlertType::NewIpLogin => AlertSeverity::Medium,
            AlertType::PermissionEscalation => AlertSeverity::High,
            AlertType::SuspendedAccessAttempt => AlertSeverity::Medium,
            AlertType::BulkDownload => AlertSeverity::High,
            AlertType::BlockedExtensionAttempt => AlertSeverity::Low,
            AlertType::ExcessiveSharing => AlertSeverity::Medium,
            AlertType::AccountLockout => AlertSeverity::Critical,
            AlertType::PotentialTokenTheft => AlertSeverity::High,
            AlertType::MalwareDetected => AlertSeverity::High,
            AlertType::UserSuspendedMalware => AlertSeverity::Critical,
            AlertType::BackupDecryptFailed => AlertSeverity::High,
            AlertType::BackupBruteForce => AlertSeverity::Critical,
            AlertType::BackupExportSecrets => AlertSeverity::High,
            AlertType::PasswordConfirmFailed => AlertSeverity::Medium,
        }
    }
}

/// Create a new security alert
/// Sends email notifications to admins for Critical and High severity alerts
pub async fn create_alert(
    pool: &PgPool,
    tenant_id: Option<Uuid>,
    user_id: Option<Uuid>,
    alert_type: AlertType,
    title: &str,
    description: &str,
    metadata: serde_json::Value,
    ip_address: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let severity = alert_type.default_severity();
    
    let result: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO security_alerts (tenant_id, user_id, alert_type, severity, title, description, metadata, ip_address)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8::inet)
        RETURNING id
        "#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(alert_type.as_str())
    .bind(severity.as_str())
    .bind(title)
    .bind(description)
    .bind(&metadata)
    .bind(ip_address)
    .fetch_one(pool)
    .await?;

    tracing::info!(
        "Security alert created: type={}, severity={}, tenant={:?}, user={:?}",
        alert_type.as_str(),
        severity.as_str(),
        tenant_id,
        user_id
    );

    // Send email notifications for Critical and High severity alerts
    if matches!(severity, AlertSeverity::Critical | AlertSeverity::High) {
        if let Some(tid) = tenant_id {
            // Get tenant info for email notification
            let tenant: Option<Tenant> = sqlx::query_as(
                "SELECT * FROM tenants WHERE id = $1"
            )
            .bind(tid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            if let Some(tenant) = tenant {
                // Get affected user email from metadata if available
                let affected_user_email = metadata.get("email")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Spawn email notification in background to not block the alert creation
                let pool_clone = pool.clone();
                let tenant_clone = tenant.clone();
                let alert_type_str = alert_type.as_str().to_string();
                let severity_str = severity.as_str().to_string();
                let title_owned = title.to_string();
                let description_owned = description.to_string();
                let ip_owned = ip_address.map(|s| s.to_string());

                tokio::spawn(async move {
                    if let Err(e) = notification_service::notify_security_alert(
                        &pool_clone,
                        &tenant_clone,
                        &alert_type_str,
                        &severity_str,
                        &title_owned,
                        &description_owned,
                        affected_user_email.as_deref(),
                        ip_owned.as_deref(),
                    ).await {
                        tracing::error!("Failed to send security alert notification: {:?}", e);
                    }
                });
            }
        }
    }

    Ok(result.0)
}

/// Record a failed login attempt and check for spike
/// Returns true if a spike was detected and alert was created
pub async fn record_failed_login(
    pool: &PgPool,
    email: &str,
    ip_address: Option<&str>,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    // Record the failed attempt
    sqlx::query(
        r#"
        INSERT INTO failed_login_attempts (email, ip_address, reason)
        VALUES ($1, $2::inet, $3)
        "#
    )
    .bind(email)
    .bind(ip_address)
    .bind(reason)
    .execute(pool)
    .await?;

    // Check for spike (5+ failures in last 5 minutes)
    let five_minutes_ago = Utc::now() - Duration::minutes(5);
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM failed_login_attempts
        WHERE email = $1 AND attempted_at > $2
        "#
    )
    .bind(email)
    .bind(five_minutes_ago)
    .fetch_one(pool)
    .await?;

    if count.0 >= 5 {
        // Check if we already created an alert for this recently (within 30 minutes)
        let thirty_minutes_ago = Utc::now() - Duration::minutes(30);
        let existing: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM security_alerts
            WHERE alert_type = 'failed_login_spike'
            AND metadata->>'email' = $1
            AND created_at > $2
            "#
        )
        .bind(email)
        .bind(thirty_minutes_ago)
        .fetch_one(pool)
        .await?;

        if existing.0 == 0 {
            // Try to find tenant_id from user's email
            let user_info: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT id, tenant_id FROM users WHERE email = $1"
            )
            .bind(email)
            .fetch_optional(pool)
            .await?;

            let (user_id, tenant_id) = match user_info {
                Some((uid, tid)) => (Some(uid), Some(tid)),
                None => (None, None),
            };

            create_alert(
                pool,
                tenant_id,
                user_id,
                AlertType::FailedLoginSpike,
                &format!("Multiple failed login attempts for {}", email),
                &format!("{} failed login attempts detected in the last 5 minutes", count.0),
                json!({
                    "email": email,
                    "attempt_count": count.0,
                    "last_ip": ip_address,
                    "reason": reason
                }),
                ip_address,
            ).await?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// Check if this is a new IP for the user and record the login
/// Returns true if this is a new IP and alert was created
pub async fn check_and_record_login_ip(
    pool: &PgPool,
    user_id: Uuid,
    tenant_id: Uuid,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
    user_email: &str,
) -> Result<bool, sqlx::Error> {
    let ip = match ip_address {
        Some(ip) if !ip.is_empty() => ip,
        _ => return Ok(false), // No IP to track
    };

    // Try to insert or update login history
    let result = sqlx::query(
        r#"
        INSERT INTO user_login_history (user_id, ip_address, user_agent, login_count)
        VALUES ($1, $2::inet, $3, 1)
        ON CONFLICT (user_id, ip_address) DO UPDATE SET
            last_seen_at = NOW(),
            login_count = user_login_history.login_count + 1,
            user_agent = COALESCE($3, user_login_history.user_agent)
        RETURNING (xmax = 0) as is_new
        "#
    )
    .bind(user_id)
    .bind(ip)
    .bind(user_agent)
    .fetch_one(pool)
    .await?;

    let is_new: bool = result.get("is_new");

    if is_new {
        // Check if user has logged in from at least one other IP before
        // (don't alert on very first login)
        let history_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_login_history WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        if history_count.0 > 1 {
            // This is a new IP and not the first login
            create_alert(
                pool,
                Some(tenant_id),
                Some(user_id),
                AlertType::NewIpLogin,
                &format!("Login from new location for {}", user_email),
                &format!("User logged in from a new IP address: {}", ip),
                json!({
                    "email": user_email,
                    "new_ip": ip,
                    "user_agent": user_agent
                }),
                Some(ip),
            ).await?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// Create alert for permission escalation (role change to Admin or higher)
pub async fn alert_permission_escalation(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    changed_by_id: Uuid,
    user_email: &str,
    old_role: &str,
    new_role: &str,
    ip_address: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    create_alert(
        pool,
        Some(tenant_id),
        Some(user_id),
        AlertType::PermissionEscalation,
        &format!("Role escalation: {} → {}", old_role, new_role),
        &format!("User {} was promoted from {} to {}", user_email, old_role, new_role),
        json!({
            "email": user_email,
            "old_role": old_role,
            "new_role": new_role,
            "changed_by": changed_by_id.to_string()
        }),
        ip_address,
    ).await
}

/// Create alert for suspended user attempting access
pub async fn alert_suspended_access_attempt(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    user_email: &str,
    attempted_action: &str,
    ip_address: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    // Check if we already alerted for this user recently (within 1 hour)
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let existing: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM security_alerts
        WHERE alert_type = 'suspended_access_attempt'
        AND user_id = $1
        AND created_at > $2
        "#
    )
    .bind(user_id)
    .bind(one_hour_ago)
    .fetch_one(pool)
    .await?;

    if existing.0 > 0 {
        // Already alerted recently, just return a dummy UUID
        return Ok(Uuid::nil());
    }

    create_alert(
        pool,
        Some(tenant_id),
        Some(user_id),
        AlertType::SuspendedAccessAttempt,
        &format!("Suspended user {} attempted access", user_email),
        &format!("Suspended user attempted to {}", attempted_action),
        json!({
            "email": user_email,
            "attempted_action": attempted_action
        }),
        ip_address,
    ).await
}

/// Check for bulk download pattern and create alert if detected
/// Returns true if alert was created
pub async fn check_bulk_download(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    user_email: &str,
    ip_address: Option<&str>,
) -> Result<bool, sqlx::Error> {
    // Count downloads in last 10 minutes
    let ten_minutes_ago = Utc::now() - Duration::minutes(10);
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM audit_logs
        WHERE tenant_id = $1 
        AND user_id = $2 
        AND action IN ('file_download', 'folder_download')
        AND created_at > $3
        "#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(ten_minutes_ago)
    .fetch_one(pool)
    .await?;

    if count.0 >= 20 {
        // Check if we already alerted recently (within 1 hour)
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let existing: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM security_alerts
            WHERE alert_type = 'bulk_download'
            AND user_id = $1
            AND created_at > $2
            "#
        )
        .bind(user_id)
        .bind(one_hour_ago)
        .fetch_one(pool)
        .await?;

        if existing.0 == 0 {
            create_alert(
                pool,
                Some(tenant_id),
                Some(user_id),
                AlertType::BulkDownload,
                &format!("Bulk download detected for {}", user_email),
                &format!("{} files downloaded in 10 minutes - potential data exfiltration", count.0),
                json!({
                    "email": user_email,
                    "download_count": count.0,
                    "time_window_minutes": 10
                }),
                ip_address,
            ).await?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// Create alert for blocked file extension upload attempt
pub async fn alert_blocked_extension(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    user_email: Option<&str>,
    filename: &str,
    extension: &str,
    ip_address: Option<&str>,
    is_public_upload: bool,
) -> Result<Uuid, sqlx::Error> {
    let title = if is_public_upload {
        format!("Blocked extension upload via file request: .{}", extension)
    } else {
        format!("Blocked extension upload attempt: .{}", extension)
    };

    let description = format!(
        "Attempted to upload file '{}' with blocked extension .{}",
        filename, extension
    );

    create_alert(
        pool,
        Some(tenant_id),
        user_id,
        AlertType::BlockedExtensionAttempt,
        &title,
        &description,
        json!({
            "filename": filename,
            "extension": extension,
            "email": user_email,
            "is_public_upload": is_public_upload
        }),
        ip_address,
    ).await
}

/// Check for excessive sharing pattern and create alert if detected
/// Returns true if alert was created
pub async fn check_excessive_sharing(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    user_email: &str,
    ip_address: Option<&str>,
) -> Result<bool, sqlx::Error> {
    // Count shares created in last hour
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM shares
        WHERE tenant_id = $1 
        AND created_by = $2 
        AND created_at > $3
        "#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(one_hour_ago)
    .fetch_one(pool)
    .await?;

    if count.0 >= 10 {
        // Check if we already alerted recently (within 2 hours)
        let two_hours_ago = Utc::now() - Duration::hours(2);
        let existing: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM security_alerts
            WHERE alert_type = 'excessive_sharing'
            AND user_id = $1
            AND created_at > $2
            "#
        )
        .bind(user_id)
        .bind(two_hours_ago)
        .fetch_one(pool)
        .await?;

        if existing.0 == 0 {
            create_alert(
                pool,
                Some(tenant_id),
                Some(user_id),
                AlertType::ExcessiveSharing,
                &format!("Excessive sharing by {}", user_email),
                &format!("{} share links created in 1 hour", count.0),
                json!({
                    "email": user_email,
                    "share_count": count.0,
                    "time_window_hours": 1
                }),
                ip_address,
            ).await?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// Create alert for account lockout
pub async fn alert_account_lockout(
    pool: &PgPool,
    tenant_id: Option<Uuid>,
    user_id: Option<Uuid>,
    email: &str,
    failed_attempts: i32,
    ip_address: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    create_alert(
        pool,
        tenant_id,
        user_id,
        AlertType::AccountLockout,
        &format!("Account locked: {}", email),
        &format!("Account locked after {} failed login attempts", failed_attempts),
        json!({
            "email": email,
            "failed_attempts": failed_attempts
        }),
        ip_address,
    ).await
}

/// Clean up old failed login attempts (older than 24 hours)
pub async fn cleanup_old_failed_attempts(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let one_day_ago = Utc::now() - Duration::hours(24);
    let result = sqlx::query("DELETE FROM failed_login_attempts WHERE attempted_at < $1")
        .bind(one_day_ago)
        .execute(pool)
        .await?;
    
    Ok(result.rows_affected())
}

/// Create alert for malware detection in uploaded file
pub async fn alert_malware_detected(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    file_id: Uuid,
    file_name: &str,
    threat_name: &str,
    action_taken: &str,
    user_email: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    create_alert(
        pool,
        Some(tenant_id),
        user_id,
        AlertType::MalwareDetected,
        &format!("Malware detected: {}", threat_name),
        &format!(
            "File '{}' was detected as malicious ({}). Action: {}",
            file_name, threat_name, action_taken
        ),
        json!({
            "file_id": file_id.to_string(),
            "file_name": file_name,
            "threat_name": threat_name,
            "action_taken": action_taken,
            "email": user_email
        }),
        None,
    ).await
}

/// Create alert for user auto-suspended due to malware uploads
pub async fn alert_user_suspended_malware(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    offense_count: i32,
    file_id: Uuid,
    file_name: &str,
    threat_name: &str,
) -> Result<Uuid, sqlx::Error> {
    // Get user email for the alert
    let user_email: Option<(String,)> = sqlx::query_as(
        "SELECT email FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let email = user_email.map(|(e,)| e).unwrap_or_else(|| "Unknown".to_string());

    create_alert(
        pool,
        Some(tenant_id),
        Some(user_id),
        AlertType::UserSuspendedMalware,
        &format!("User auto-suspended: {}", email),
        &format!(
            "User {} has been automatically suspended after uploading {} infected file(s). Last infection: '{}' with {}",
            email, offense_count, file_name, threat_name
        ),
        json!({
            "user_id": user_id.to_string(),
            "email": email,
            "offense_count": offense_count,
            "file_id": file_id.to_string(),
            "file_name": file_name,
            "threat_name": threat_name,
            "action": "auto_suspended"
        }),
        None,
    ).await
}

