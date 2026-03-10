use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::mailer;
use crate::models::Tenant;

// ==================== Notification Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    FileUpload,
    RequestExpiring,
    UserCreated,
    RoleChanged,
    ComplianceAlert,
    StorageWarning,
    FileShared,
    MalwareDetected,
    ApprovalRequired,
    ApprovalDecision,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::FileUpload => "file_upload",
            NotificationType::RequestExpiring => "request_expiring",
            NotificationType::UserCreated => "user_created",
            NotificationType::RoleChanged => "role_changed",
            NotificationType::ComplianceAlert => "compliance_alert",
            NotificationType::StorageWarning => "storage_warning",
            NotificationType::FileShared => "file_shared",
            NotificationType::MalwareDetected => "malware_detected",
            NotificationType::ApprovalRequired => "approval_required",
            NotificationType::ApprovalDecision => "approval_decision",
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            NotificationType::FileUpload => "file_upload",
            NotificationType::RequestExpiring => "request_expiring",
            NotificationType::UserCreated | NotificationType::RoleChanged => "user_action",
            NotificationType::ComplianceAlert => "compliance_alert",
            NotificationType::StorageWarning => "storage_warning",
            NotificationType::FileShared => "file_shared",
            NotificationType::MalwareDetected => "security_alert",
            NotificationType::ApprovalRequired | NotificationType::ApprovalDecision => "approval",
        }
    }
}

// ==================== Email Template Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailTemplate {
    pub id: Uuid,
    pub template_key: String,
    pub name: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: Option<String>,
    pub variables: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TenantEmailTemplate {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub template_key: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rendered email template with variables replaced
#[derive(Debug, Clone)]
pub struct RenderedTemplate {
    pub subject: String,
    pub body_html: String,
    pub body_text: Option<String>,
}

/// Fetch email template (tenant override first, then global default)
pub async fn get_email_template(
    pool: &PgPool,
    tenant_id: Uuid,
    template_key: &str,
) -> Option<(String, String, Option<String>)> {
    // First, try to get tenant-specific template
    let tenant_template: Option<TenantEmailTemplate> = sqlx::query_as(
        "SELECT id, tenant_id, template_key, subject, body_html, body_text, created_at, updated_at 
         FROM tenant_email_templates WHERE tenant_id = $1 AND template_key = $2"
    )
    .bind(tenant_id)
    .bind(template_key)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(t) = tenant_template {
        return Some((t.subject, t.body_html, t.body_text));
    }

    // Fall back to global template
    let global_template: Option<EmailTemplate> = sqlx::query_as(
        "SELECT id, template_key, name, subject, body_html, body_text, variables, created_at, updated_at 
         FROM email_templates WHERE template_key = $1"
    )
    .bind(template_key)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    global_template.map(|t| (t.subject, t.body_html, t.body_text))
}

/// Replace template variables with actual values
pub fn render_template(template: &str, variables: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

/// Render a complete email with template and variables
pub async fn render_email_template(
    pool: &PgPool,
    tenant: &Tenant,
    template_key: &str,
    variables: HashMap<String, String>,
) -> Option<RenderedTemplate> {
    let (subject, body_html, body_text) = get_email_template(pool, tenant.id, template_key).await?;

    // Add default variables
    let mut all_vars = variables;
    all_vars.entry("company_name".to_string()).or_insert_with(|| tenant.name.clone());
    all_vars.entry("app_url".to_string()).or_insert_with(|| {
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://app.clovalink.com".to_string())
    });

    Some(RenderedTemplate {
        subject: render_template(&subject, &all_vars),
        body_html: render_template(&body_html, &all_vars),
        body_text: body_text.map(|t| render_template(&t, &all_vars)),
    })
}

/// Send a templated email directly (useful for transactional emails like password reset)
pub async fn send_templated_email(
    pool: &PgPool,
    tenant: &Tenant,
    to_email: &str,
    template_key: &str,
    variables: HashMap<String, String>,
) -> Result<(), String> {
    let rendered = render_email_template(pool, tenant, template_key, variables).await
        .ok_or_else(|| format!("Email template '{}' not found", template_key))?;
    
    mailer::send_email(tenant, to_email, &rendered.subject, &rendered.body_html)
        .await
        .map_err(|e| format!("Failed to send email: {:?}", e))
}

// ==================== Notification Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub metadata: Value,
    pub is_read: bool,
    pub email_sent: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationPreference {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub email_enabled: bool,
    pub in_app_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferenceInput {
    pub event_type: String,
    pub email_enabled: Option<bool>,
    pub in_app_enabled: Option<bool>,
}

// ==================== Role-Based Access ====================

/// Check if a user role can receive a specific notification type
/// SuperAdmin and Admin can receive all notifications
/// Manager and Employee have limited access based on ownership
pub fn can_receive_notification(
    role: &str,
    notification_type: &NotificationType,
    user_id: Uuid,
    resource_owner_id: Option<Uuid>,
) -> bool {
    match role {
        "SuperAdmin" | "Admin" => true,
        "Manager" => match notification_type {
            NotificationType::FileUpload
            | NotificationType::RequestExpiring
            | NotificationType::FileShared
            | NotificationType::ApprovalRequired
            | NotificationType::ApprovalDecision => true,
            NotificationType::UserCreated
            | NotificationType::RoleChanged
            | NotificationType::ComplianceAlert
            | NotificationType::StorageWarning
            | NotificationType::MalwareDetected => false,
        },
        "Employee" => match notification_type {
            NotificationType::FileShared
            | NotificationType::ApprovalDecision => true,
            NotificationType::FileUpload | NotificationType::RequestExpiring => {
                // Only if they own the resource
                resource_owner_id.map_or(false, |owner| owner == user_id)
            }
            // Employees can receive malware notifications for their own uploads
            NotificationType::MalwareDetected => {
                resource_owner_id.map_or(false, |owner| owner == user_id)
            }
            _ => false,
        },
        _ => false,
    }
}

// ==================== Core Notification Functions ====================

/// Create a notification for a user
/// Respects both tenant-level settings and user preferences
/// SuperAdmins are exempt from tenant-level controls
pub async fn create_notification(
    pool: &PgPool,
    tenant: &Tenant,
    user_id: Uuid,
    user_role: &str,
    notification_type: NotificationType,
    title: &str,
    message: &str,
    metadata: Option<Value>,
    user_email: Option<&str>,
) -> Result<Notification, sqlx::Error> {
    let event_type = notification_type.event_type();
    
    // Get effective preferences (merges tenant settings + user preferences, exempts SuperAdmins)
    let effective_prefs = get_effective_preferences(pool, tenant.id, user_id, user_role, event_type).await;
    
    // If notification type is disabled at company level, skip entirely
    if !effective_prefs.notification_enabled {
        // Return a placeholder notification (not stored)
        return Ok(Notification {
            id: Uuid::new_v4(),
            user_id,
            tenant_id: tenant.id,
            notification_type: notification_type.as_str().to_string(),
            title: title.to_string(),
            message: message.to_string(),
            metadata: metadata.unwrap_or(json!({})),
            is_read: false,
            email_sent: false,
            created_at: Utc::now(),
        });
    }
    
    let mut email_sent = false;
    let mut notification_id = None;
    
    // Create in-app notification if enabled
    if effective_prefs.in_app_enabled {
        let notif: Notification = sqlx::query_as(
            r#"
            INSERT INTO notifications (user_id, tenant_id, notification_type, title, message, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#
        )
        .bind(user_id)
        .bind(tenant.id)
        .bind(notification_type.as_str())
        .bind(title)
        .bind(message)
        .bind(metadata.clone().unwrap_or(json!({})))
        .fetch_one(pool)
        .await?;
        
        notification_id = Some(notif.id);
    }
    
    // Send email if enabled and user email is provided
    if effective_prefs.email_enabled {
        if let Some(email) = user_email {
            // Use database templates with fallback
            let (email_subject, email_body) = format_email_body_with_template(
                pool, tenant, &notification_type, title, message, &metadata
            ).await;
            
            match mailer::send_email(tenant, email, &email_subject, &email_body).await {
                Ok(_) => {
                    email_sent = true;
                    // Update notification to mark email as sent
                    if let Some(nid) = notification_id {
                        let _ = sqlx::query("UPDATE notifications SET email_sent = true WHERE id = $1")
                            .bind(nid)
                            .execute(pool)
                            .await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to send notification email: {:?}", e);
                }
            }
        }
    }
    
    // Return the notification (or a placeholder if not created)
    if let Some(nid) = notification_id {
        sqlx::query_as("SELECT * FROM notifications WHERE id = $1")
            .bind(nid)
            .fetch_one(pool)
            .await
    } else {
        // Return a placeholder notification (not stored)
        Ok(Notification {
            id: Uuid::new_v4(),
            user_id,
            tenant_id: tenant.id,
            notification_type: notification_type.as_str().to_string(),
            title: title.to_string(),
            message: message.to_string(),
            metadata: metadata.unwrap_or(json!({})),
            is_read: false,
            email_sent,
            created_at: Utc::now(),
        })
    }
}

// ==================== Tenant Settings ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TenantNotificationSetting {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub event_type: String,
    pub enabled: bool,
    pub email_enforced: bool,
    pub in_app_enforced: bool,
    pub default_email: bool,
    pub default_in_app: bool,
    pub role: Option<String>,  // NULL = applies to all roles, specific value = role-specific
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Effective preferences after merging tenant and user settings
#[derive(Debug, Clone)]
pub struct EffectivePreferences {
    pub email_enabled: bool,
    pub in_app_enabled: bool,
    pub notification_enabled: bool, // If false, skip notification entirely
}

/// Get tenant notification settings for an event type, checking role-specific first then global
async fn get_tenant_settings_for_role(
    pool: &PgPool,
    tenant_id: Uuid,
    event_type: &str,
    role: &str,
) -> Option<TenantNotificationSetting> {
    // First try to get role-specific settings
    let role_specific: Option<TenantNotificationSetting> = sqlx::query_as(
        "SELECT * FROM tenant_notification_settings WHERE tenant_id = $1 AND event_type = $2 AND role = $3"
    )
    .bind(tenant_id)
    .bind(event_type)
    .bind(role)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    
    if role_specific.is_some() {
        return role_specific;
    }
    
    // Fall back to global settings (role = NULL)
    sqlx::query_as(
        "SELECT * FROM tenant_notification_settings WHERE tenant_id = $1 AND event_type = $2 AND role IS NULL"
    )
    .bind(tenant_id)
    .bind(event_type)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Get user preferences for a specific event type
async fn get_user_preferences(
    pool: &PgPool,
    user_id: Uuid,
    event_type: &str,
) -> Result<NotificationPreference, sqlx::Error> {
    // Try to get existing preference
    let preference: Option<NotificationPreference> = sqlx::query_as(
        "SELECT * FROM notification_preferences WHERE user_id = $1 AND event_type = $2"
    )
    .bind(user_id)
    .bind(event_type)
    .fetch_optional(pool)
    .await?;
    
    // Return existing or create default
    match preference {
        Some(p) => Ok(p),
        None => {
            // Insert default preference
            sqlx::query_as(
                r#"
                INSERT INTO notification_preferences (user_id, event_type, email_enabled, in_app_enabled)
                VALUES ($1, $2, true, true)
                RETURNING *
                "#
            )
            .bind(user_id)
            .bind(event_type)
            .fetch_one(pool)
            .await
        }
    }
}

/// Get effective preferences by merging tenant settings with user preferences
/// Rules:
/// - SuperAdmins are EXEMPT from tenant settings (only user prefs apply)
/// - Role-specific tenant settings take priority over global tenant settings
/// - If tenant disables a notification type, it's disabled for that role
/// - If tenant enforces email/in-app, users can't disable it
/// - Otherwise, user preferences apply
pub async fn get_effective_preferences(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    user_role: &str,
    event_type: &str,
) -> EffectivePreferences {
    // SuperAdmins are exempt from tenant-level notification controls
    if user_role == "SuperAdmin" {
        let user_prefs = get_user_preferences(pool, user_id, event_type).await.ok();
        return match user_prefs {
            Some(up) => EffectivePreferences {
                email_enabled: up.email_enabled,
                in_app_enabled: up.in_app_enabled,
                notification_enabled: true,
            },
            None => EffectivePreferences {
                email_enabled: true,
                in_app_enabled: true,
                notification_enabled: true,
            },
        };
    }
    
    // Get tenant settings (role-specific first, then global)
    let tenant_settings = get_tenant_settings_for_role(pool, tenant_id, event_type, user_role).await;
    
    // Get user preferences
    let user_prefs = get_user_preferences(pool, user_id, event_type).await.ok();
    
    match tenant_settings {
        Some(ts) => {
            // Tenant has explicit settings
            if !ts.enabled {
                // Notification type is disabled at company level
                return EffectivePreferences {
                    email_enabled: false,
                    in_app_enabled: false,
                    notification_enabled: false,
                };
            }
            
            // Merge with user preferences
            let user_email = user_prefs.as_ref().map(|p| p.email_enabled).unwrap_or(ts.default_email);
            let user_in_app = user_prefs.as_ref().map(|p| p.in_app_enabled).unwrap_or(ts.default_in_app);
            
            EffectivePreferences {
                // If enforced, it's always on; otherwise use user preference
                email_enabled: ts.email_enforced || user_email,
                in_app_enabled: ts.in_app_enforced || user_in_app,
                notification_enabled: true,
            }
        }
        None => {
            // No tenant settings, use user preferences with defaults
            match user_prefs {
                Some(up) => EffectivePreferences {
                    email_enabled: up.email_enabled,
                    in_app_enabled: up.in_app_enabled,
                    notification_enabled: true,
                },
                None => EffectivePreferences {
                    email_enabled: true,
                    in_app_enabled: true,
                    notification_enabled: true,
                },
            }
        }
    }
}

/// Format email body using database templates with fallback to hardcoded template
async fn format_email_body_with_template(
    pool: &PgPool,
    tenant: &Tenant,
    notification_type: &NotificationType,
    title: &str,
    message: &str,
    metadata: &Option<Value>,
) -> (String, String) {
    // Build variables from metadata
    let mut variables = HashMap::new();
    variables.insert("user_name".to_string(), "User".to_string()); // Default, will be overridden
    variables.insert("company_name".to_string(), tenant.name.clone());
    variables.insert("app_url".to_string(), 
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://app.clovalink.com".to_string()));
    variables.insert("message".to_string(), message.to_string());
    
    // Extract variables from metadata
    if let Some(meta) = metadata {
        if let Some(obj) = meta.as_object() {
            for (key, value) in obj {
                if let Some(s) = value.as_str() {
                    variables.insert(key.clone(), s.to_string());
                } else if let Some(n) = value.as_i64() {
                    variables.insert(key.clone(), n.to_string());
                } else if let Some(id) = value.as_str() {
                    variables.insert(key.clone(), id.to_string());
                }
            }
        }
    }
    
    let template_key = notification_type.as_str();
    
    // Try to get template from database
    if let Some(rendered) = render_email_template(pool, tenant, template_key, variables.clone()).await {
        return (rendered.subject, rendered.body_html);
    }
    
    // Fallback to hardcoded template
    let fallback_html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 20px; border-radius: 8px 8px 0 0; }}
        .content {{ background: #f9fafb; padding: 20px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px; }}
        .footer {{ text-align: center; color: #6b7280; font-size: 12px; margin-top: 20px; }}
        .badge {{ display: inline-block; padding: 4px 12px; background: #e0e7ff; color: #3730a3; border-radius: 9999px; font-size: 12px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h2 style="margin: 0;">{title}</h2>
            <span class="badge">{notification_type}</span>
        </div>
        <div class="content">
            <p>{message}</p>
        </div>
        <div class="footer">
            <p>This is an automated notification from {company_name}.</p>
            <p>Powered by ClovaLink</p>
        </div>
    </div>
</body>
</html>"#,
        title = title,
        notification_type = notification_type.as_str().replace('_', " ").to_uppercase(),
        message = message,
        company_name = tenant.name,
    );
    
    (title.to_string(), fallback_html)
}

// ==================== Event-Specific Notification Helpers ====================

/// Notify about a file upload to a file request
pub async fn notify_file_upload(
    pool: &PgPool,
    tenant: &Tenant,
    request_owner_id: Uuid,
    request_owner_email: &str,
    request_owner_role: &str,
    request_name: &str,
    uploader_name: &str,
    file_name: &str,
    file_id: Uuid,
    request_id: Uuid,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        request_owner_role,
        &NotificationType::FileUpload,
        request_owner_id,
        Some(request_owner_id),
    ) {
        return Ok(());
    }
    
    let title = format!("New upload to \"{}\"", request_name);
    let message = format!(
        "{} uploaded \"{}\" to your file request.",
        uploader_name, file_name
    );
    let metadata = json!({
        "file_id": file_id,
        "file_name": file_name,
        "request_id": request_id,
        "request_name": request_name,
        "uploader_name": uploader_name
    });
    
    create_notification(
        pool,
        tenant,
        request_owner_id,
        request_owner_role,
        NotificationType::FileUpload,
        &title,
        &message,
        Some(metadata),
        Some(request_owner_email),
    ).await?;
    
    Ok(())
}

/// Notify about expiring file requests
pub async fn notify_expiring_request(
    pool: &PgPool,
    tenant: &Tenant,
    user_id: Uuid,
    user_email: &str,
    user_role: &str,
    request_name: &str,
    request_id: Uuid,
    days_until_expiry: i32,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        user_role,
        &NotificationType::RequestExpiring,
        user_id,
        Some(user_id),
    ) {
        return Ok(());
    }
    
    let title = format!("File request \"{}\" expiring soon", request_name);
    let message = if days_until_expiry == 0 {
        format!("Your file request \"{}\" expires today!", request_name)
    } else if days_until_expiry == 1 {
        format!("Your file request \"{}\" expires tomorrow.", request_name)
    } else {
        format!("Your file request \"{}\" expires in {} days.", request_name, days_until_expiry)
    };
    let metadata = json!({
        "request_id": request_id,
        "request_name": request_name,
        "days_until_expiry": days_until_expiry
    });
    
    create_notification(
        pool,
        tenant,
        user_id,
        user_role,
        NotificationType::RequestExpiring,
        &title,
        &message,
        Some(metadata),
        Some(user_email),
    ).await?;
    
    Ok(())
}

/// Notify admins about new user creation
pub async fn notify_user_created(
    pool: &PgPool,
    tenant: &Tenant,
    admin_id: Uuid,
    admin_email: &str,
    admin_role: &str,
    new_user_name: &str,
    new_user_email: &str,
    new_user_role: &str,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        admin_role,
        &NotificationType::UserCreated,
        admin_id,
        None,
    ) {
        return Ok(());
    }
    
    let title = "New user added".to_string();
    let message = format!(
        "{} ({}) was added as {} to the organization.",
        new_user_name, new_user_email, new_user_role
    );
    let metadata = json!({
        "new_user_email": new_user_email,
        "new_user_name": new_user_name,
        "new_user_role": new_user_role
    });
    
    create_notification(
        pool,
        tenant,
        admin_id,
        admin_role,
        NotificationType::UserCreated,
        &title,
        &message,
        Some(metadata),
        Some(admin_email),
    ).await?;
    
    Ok(())
}

/// Notify user about role change
pub async fn notify_role_changed(
    pool: &PgPool,
    tenant: &Tenant,
    user_id: Uuid,
    user_email: &str,
    old_role: &str,
    new_role: &str,
) -> Result<(), sqlx::Error> {
    let title = "Your role has been updated".to_string();
    let message = format!(
        "Your role has been changed from {} to {}.",
        old_role, new_role
    );
    let metadata = json!({
        "old_role": old_role,
        "new_role": new_role
    });
    
    // User always receives their own role change notification
    // Use new_role since that's their current role
    create_notification(
        pool,
        tenant,
        user_id,
        new_role,
        NotificationType::RoleChanged,
        &title,
        &message,
        Some(metadata),
        Some(user_email),
    ).await?;
    
    Ok(())
}

/// Notify admins about compliance alerts
pub async fn notify_compliance_alert(
    pool: &PgPool,
    tenant: &Tenant,
    admin_id: Uuid,
    admin_email: &str,
    admin_role: &str,
    alert_type: &str,
    alert_message: &str,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        admin_role,
        &NotificationType::ComplianceAlert,
        admin_id,
        None,
    ) {
        return Ok(());
    }
    
    let title = format!("Compliance Alert: {}", alert_type);
    let metadata = json!({
        "alert_type": alert_type,
        "compliance_mode": tenant.compliance_mode
    });
    
    create_notification(
        pool,
        tenant,
        admin_id,
        admin_role,
        NotificationType::ComplianceAlert,
        &title,
        alert_message,
        Some(metadata),
        Some(admin_email),
    ).await?;
    
    Ok(())
}

/// Notify admins about storage warnings
pub async fn notify_storage_warning(
    pool: &PgPool,
    tenant: &Tenant,
    admin_id: Uuid,
    admin_email: &str,
    admin_role: &str,
    percentage_used: i32,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        admin_role,
        &NotificationType::StorageWarning,
        admin_id,
        None,
    ) {
        return Ok(());
    }
    
    let (title, message) = if percentage_used >= 100 {
        (
            "Storage quota exceeded".to_string(),
            "Your storage quota has been exceeded. Please free up space or upgrade your plan.".to_string()
        )
    } else if percentage_used >= 90 {
        (
            "Storage quota critical".to_string(),
            format!("You have used {}% of your storage quota. Consider freeing up space.", percentage_used)
        )
    } else {
        (
            "Storage quota warning".to_string(),
            format!("You have used {}% of your storage quota.", percentage_used)
        )
    };
    
    let metadata = json!({
        "percentage_used": percentage_used,
        "storage_used_bytes": tenant.storage_used_bytes,
        "storage_quota_bytes": tenant.storage_quota_bytes
    });
    
    create_notification(
        pool,
        tenant,
        admin_id,
        admin_role,
        NotificationType::StorageWarning,
        &title,
        &message,
        Some(metadata),
        Some(admin_email),
    ).await?;
    
    Ok(())
}

/// Notify user about file being shared with them
pub async fn notify_file_shared(
    pool: &PgPool,
    tenant: &Tenant,
    user_id: Uuid,
    user_email: &str,
    user_role: &str,
    sharer_name: &str,
    file_name: &str,
    file_id: Uuid,
) -> Result<(), sqlx::Error> {
    let title = format!("{} shared a file with you", sharer_name);
    let message = format!(
        "{} shared \"{}\" with you.",
        sharer_name, file_name
    );
    let metadata = json!({
        "file_id": file_id,
        "file_name": file_name,
        "sharer_name": sharer_name
    });
    
    create_notification(
        pool,
        tenant,
        user_id,
        user_role,
        NotificationType::FileShared,
        &title,
        &message,
        Some(metadata),
        Some(user_email),
    ).await?;
    
    Ok(())
}

// ==================== Bulk Notification Functions ====================

/// Get all admins for a tenant to send them notifications
pub async fn get_tenant_admins(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<(Uuid, String, String)>, sqlx::Error> {
    let admins: Vec<(Uuid, String, String)> = sqlx::query_as(
        r#"
        SELECT id, email, role 
        FROM users 
        WHERE tenant_id = $1 
        AND role IN ('SuperAdmin', 'Admin') 
        AND status = 'active'
        "#
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    
    Ok(admins)
}

/// Notify all admins of a tenant about an event
pub async fn notify_all_admins(
    pool: &PgPool,
    tenant: &Tenant,
    notification_type: NotificationType,
    title: &str,
    message: &str,
    metadata: Option<Value>,
) -> Result<(), sqlx::Error> {
    let admins = get_tenant_admins(pool, tenant.id).await?;
    
    for (admin_id, admin_email, admin_role) in admins {
        if can_receive_notification(&admin_role, &notification_type, admin_id, None) {
            let _ = create_notification(
                pool,
                tenant,
                admin_id,
                &admin_role,
                notification_type.clone(),
                title,
                message,
                metadata.clone(),
                Some(&admin_email),
            ).await;
        }
    }
    
    Ok(())
}

/// Send security alert emails to all admins in a tenant
/// Only triggers for Critical and High severity alerts
pub async fn notify_security_alert(
    pool: &PgPool,
    tenant: &Tenant,
    alert_type: &str,
    severity: &str,
    title: &str,
    description: &str,
    affected_user_email: Option<&str>,
    ip_address: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get all admins in this tenant
    let admins = get_tenant_admins(pool, tenant.id).await?;
    
    if admins.is_empty() {
        tracing::warn!("No admins found for tenant {} to notify about security alert", tenant.id);
        return Ok(());
    }

    // Get APP_URL for links
    let app_url = std::env::var("APP_URL")
        .or_else(|_| std::env::var("BASE_URL"))
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Format alert type for display
    let alert_type_display = alert_type
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ");

    // Format timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    // Send email to each admin
    for (admin_id, admin_email, _admin_role) in admins {
        // Get admin's name
        let admin_name: Option<String> = sqlx::query_scalar(
            "SELECT name FROM users WHERE id = $1"
        )
        .bind(admin_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        let admin_name = admin_name.unwrap_or_else(|| "Admin".to_string());

        // Build template variables
        let mut variables = HashMap::new();
        variables.insert("user_name".to_string(), admin_name);
        variables.insert("severity".to_string(), severity.to_uppercase());
        variables.insert("severity_lower".to_string(), severity.to_lowercase());
        variables.insert("alert_title".to_string(), title.to_string());
        variables.insert("description".to_string(), description.to_string());
        variables.insert("alert_type".to_string(), alert_type.to_string());
        variables.insert("alert_type_display".to_string(), alert_type_display.clone());
        variables.insert("timestamp".to_string(), timestamp.clone());
        variables.insert("affected_user".to_string(), affected_user_email.unwrap_or("N/A").to_string());
        variables.insert("ip_address".to_string(), ip_address.unwrap_or("N/A").to_string());
        variables.insert("tenant_name".to_string(), tenant.name.clone());
        variables.insert("app_url".to_string(), app_url.clone());

        // Render and send email
        if let Some(rendered) = render_email_template(pool, tenant, "security_alert", variables).await {
            if let Err(e) = mailer::send_email(
                tenant,
                &admin_email,
                &rendered.subject,
                &rendered.body_html,
            ).await {
                tracing::error!(
                    "Failed to send security alert email to {}: {:?}",
                    admin_email, e
                );
            } else {
                tracing::info!(
                    "Sent security alert email to {} for {} alert",
                    admin_email, severity
                );
            }
        } else {
            // Fallback if template not found - use basic email
            let subject = format!("🚨 Security Alert: {}", title);
            let body = format!(
                "<h2>Security Alert</h2>\
                <p><strong>Severity:</strong> {}</p>\
                <p><strong>Alert:</strong> {}</p>\
                <p>{}</p>\
                <p><strong>Time:</strong> {}</p>\
                <p><strong>Company:</strong> {}</p>\
                <p><a href=\"{}/security\">View Security Dashboard</a></p>",
                severity.to_uppercase(),
                title,
                description,
                timestamp,
                tenant.name,
                app_url
            );

            if let Err(e) = mailer::send_email(
                tenant,
                &admin_email,
                &subject,
                &body,
            ).await {
                tracing::error!(
                    "Failed to send fallback security alert email to {}: {:?}",
                    admin_email, e
                );
            }
        }
    }

    Ok(())
}

// ==================== Malware Detection Notifications ====================

/// Notify admins about malware detection
pub async fn notify_malware_detected_admin(
    pool: &PgPool,
    tenant: &Tenant,
    admin_id: Uuid,
    admin_email: &str,
    admin_role: &str,
    file_id: Uuid,
    file_name: &str,
    threat_name: &str,
    action_taken: &str,
    uploader_email: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !can_receive_notification(
        admin_role,
        &NotificationType::MalwareDetected,
        admin_id,
        None,
    ) {
        return Ok(());
    }
    
    let title = format!("Malware Detected: {}", threat_name);
    let message = format!(
        "A file uploaded to your organization was detected as malicious.\n\n\
        File: {}\n\
        Threat: {}\n\
        Action: {}\n\
        Uploader: {}",
        file_name,
        threat_name,
        action_taken,
        uploader_email.unwrap_or("Unknown")
    );
    let metadata = json!({
        "file_id": file_id.to_string(),
        "file_name": file_name,
        "threat_name": threat_name,
        "action_taken": action_taken,
        "uploader_email": uploader_email
    });
    
    create_notification(
        pool,
        tenant,
        admin_id,
        admin_role,
        NotificationType::MalwareDetected,
        &title,
        &message,
        Some(metadata),
        Some(admin_email),
    ).await?;
    
    Ok(())
}

/// Notify the file uploader about malware detection in their file
pub async fn notify_malware_detected_uploader(
    pool: &PgPool,
    tenant: &Tenant,
    user_id: Uuid,
    user_email: &str,
    user_role: &str,
    file_id: Uuid,
    file_name: &str,
    threat_name: &str,
    action_taken: &str,
) -> Result<(), sqlx::Error> {
    // For MalwareDetected, uploader can receive if they own the file
    if !can_receive_notification(
        user_role,
        &NotificationType::MalwareDetected,
        user_id,
        Some(user_id), // They are the resource owner
    ) {
        return Ok(());
    }
    
    let title = "Security Alert: File removed".to_string();
    let message = format!(
        "A file you uploaded was flagged as potentially harmful and has been {}.\n\n\
        File: {}\n\
        Detected threat: {}\n\n\
        If you believe this was a mistake, please contact your administrator.",
        action_taken.to_lowercase(),
        file_name,
        threat_name
    );
    let metadata = json!({
        "file_id": file_id.to_string(),
        "file_name": file_name,
        "threat_name": threat_name,
        "action_taken": action_taken
    });
    
    create_notification(
        pool,
        tenant,
        user_id,
        user_role,
        NotificationType::MalwareDetected,
        &title,
        &message,
        Some(metadata),
        Some(user_email),
    ).await?;
    
    Ok(())
}

/// Notify all relevant parties about malware detection (convenience function)
pub async fn notify_malware_detection(
    pool: &PgPool,
    tenant: &Tenant,
    file_id: Uuid,
    file_name: &str,
    threat_name: &str,
    action_taken: &str,
    uploader_id: Option<Uuid>,
    uploader_email: Option<&str>,
    uploader_role: Option<&str>,
    notify_admin: bool,
    notify_uploader: bool,
) -> Result<(), sqlx::Error> {
    // Notify admins if enabled
    if notify_admin {
        let admins = get_tenant_admins(pool, tenant.id).await?;
        for (admin_id, admin_email, admin_role) in admins {
            if let Err(e) = notify_malware_detected_admin(
                pool,
                tenant,
                admin_id,
                &admin_email,
                &admin_role,
                file_id,
                file_name,
                threat_name,
                action_taken,
                uploader_email,
            ).await {
                tracing::error!(
                    "Failed to notify admin {} about malware detection: {:?}",
                    admin_email, e
                );
            }
        }
    }
    
    // Notify uploader if enabled and we know who they are
    if notify_uploader {
        if let (Some(uid), Some(email), Some(role)) = (uploader_id, uploader_email, uploader_role) {
            if let Err(e) = notify_malware_detected_uploader(
                pool,
                tenant,
                uid,
                email,
                role,
                file_id,
                file_name,
                threat_name,
                action_taken,
            ).await {
                tracing::error!(
                    "Failed to notify uploader {} about malware detection: {:?}",
                    email, e
                );
            }
        }
    }
    
    Ok(())
}
