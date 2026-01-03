use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Extension,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppState;
use crate::compliance::{ComplianceRestrictions, get_tenant_compliance_mode, can_modify_setting};
use clovalink_auth::AuthUser;
use clovalink_core::models::{AuditSettings, UpdateAuditSettingsInput};
use sqlx::FromRow;
use chrono::{DateTime, Utc, NaiveDate};
use uuid::Uuid;

// ==================== Query Parameters ====================

#[derive(Debug, Deserialize)]
pub struct ListAuditLogsParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub resource_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExportAuditLogsParams {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub resource_type: Option<String>,
}

// ==================== Response Types ====================

#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
    pub id: String,
    pub user: String,
    pub user_id: Option<String>,
    pub action: String,
    pub action_display: String,
    pub resource: String,
    pub resource_type: String,
    pub description: String,
    pub timestamp: String,
    pub status: String,
    pub ip_address: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(FromRow)]
struct AuditLogRow {
    id: Uuid,
    action: String,
    resource_type: String,
    #[allow(dead_code)]
    resource_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    user_id: Option<Uuid>,
    user_name: Option<String>,
    metadata: Option<Value>,
    ip_address: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogsListResponse {
    pub logs: Vec<AuditLogResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

// ==================== Handlers ====================

/// List activity logs for a tenant with filtering
/// GET /api/activity-logs
pub async fn list_activity_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<ListAuditLogsParams>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);

    // Build WHERE clause dynamically
    let mut conditions = vec!["a.tenant_id = $1".to_string()];
    let mut param_idx = 4; // $1 = tenant_id, $2 = limit, $3 = offset

    if params.start_date.is_some() {
        conditions.push(format!("a.created_at >= ${}", param_idx));
        param_idx += 1;
    }
    if params.end_date.is_some() {
        conditions.push(format!("a.created_at < ${} + INTERVAL '1 day'", param_idx));
        param_idx += 1;
    }
    if params.action.is_some() {
        conditions.push(format!("a.action = ${}", param_idx));
        param_idx += 1;
    }
    if params.user_id.is_some() {
        conditions.push(format!("a.user_id = ${}", param_idx));
        param_idx += 1;
    }
    if params.resource_type.is_some() {
        conditions.push(format!("a.resource_type = ${}", param_idx));
    }

    let where_clause = conditions.join(" AND ");

    // Count total for pagination
    let count_query = format!(
        r#"
        SELECT COUNT(*) as count
        FROM audit_logs a
        WHERE {}
        "#,
        where_clause
    );

    let mut count_builder = sqlx::query_scalar::<_, i64>(&count_query)
        .bind(auth.tenant_id);
    
    if let Some(start) = params.start_date {
        count_builder = count_builder.bind(start);
    }
    if let Some(end) = params.end_date {
        count_builder = count_builder.bind(end);
    }
    if let Some(ref action) = params.action {
        count_builder = count_builder.bind(action);
    }
    if let Some(user_id) = params.user_id {
        count_builder = count_builder.bind(user_id);
    }
    if let Some(ref resource_type) = params.resource_type {
        count_builder = count_builder.bind(resource_type);
    }

    let total = count_builder
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    // Fetch logs with filters
    let query = format!(
        r#"
        SELECT 
            a.id, 
            a.action, 
            a.resource_type, 
            a.resource_id,
            a.created_at, 
            a.user_id,
            u.name as user_name,
            a.metadata,
            a.ip_address::text as ip_address
        FROM audit_logs a
        LEFT JOIN users u ON a.user_id = u.id
        WHERE {}
        ORDER BY a.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        where_clause
    );

    let mut query_builder = sqlx::query_as::<_, AuditLogRow>(&query)
        .bind(auth.tenant_id)
        .bind(limit)
        .bind(offset);
    
    if let Some(start) = params.start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = params.end_date {
        query_builder = query_builder.bind(end);
    }
    if let Some(ref action) = params.action {
        query_builder = query_builder.bind(action);
    }
    if let Some(user_id) = params.user_id {
        query_builder = query_builder.bind(user_id);
    }
    if let Some(ref resource_type) = params.resource_type {
        query_builder = query_builder.bind(resource_type);
    }

    let logs = query_builder
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list audit logs: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response: Vec<AuditLogResponse> = logs.into_iter().map(|row| {
        let status = match row.action.as_str() {
            "login_failed" | "security_alert" => "warning",
            _ => "success",
        };

        // Extract human-readable resource name from metadata
        let resource = if let Some(meta) = &row.metadata {
            // Try various human-readable fields in order of preference
            meta.get("file_name")
                .or_else(|| meta.get("folder_name"))
                .or_else(|| meta.get("new_name"))
                .or_else(|| meta.get("old_name"))
                .or_else(|| meta.get("target_user_name"))
                .or_else(|| meta.get("target_user_email"))
                .or_else(|| meta.get("deleted_user_name"))
                .or_else(|| meta.get("deleted_user_email"))
                .or_else(|| meta.get("request_name"))
                .or_else(|| meta.get("resource_name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&row.resource_type)
                .to_string()
        } else {
            row.resource_type.clone()
        };

        // Generate human-readable action display name
        let action_display = format_action_display(&row.action);
        
        // Generate full human-readable description
        let user_name = row.user_name.clone().unwrap_or_else(|| "System".to_string());
        let description = format_audit_description(&row.action, &user_name, &resource, &row.metadata);

        AuditLogResponse {
            id: row.id.to_string(),
            user: user_name,
            user_id: row.user_id.map(|id| id.to_string()),
            action: row.action,
            action_display,
            resource,
            resource_type: row.resource_type,
            description,
            timestamp: row.created_at.to_rfc3339(),
            status: status.to_string(),
            ip_address: row.ip_address,
            metadata: row.metadata,
        }
    }).collect();

    Ok(Json(json!(AuditLogsListResponse {
        logs: response,
        total,
        limit,
        offset,
    })))
}

/// Export audit logs as CSV
/// GET /api/activity-logs/export
pub async fn export_activity_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<ExportAuditLogsParams>,
) -> Result<Response, StatusCode> {
    // Check permission
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Build WHERE clause dynamically
    let mut conditions = vec!["a.tenant_id = $1".to_string()];
    let mut param_idx = 2;

    if params.start_date.is_some() {
        conditions.push(format!("a.created_at >= ${}", param_idx));
        param_idx += 1;
    }
    if params.end_date.is_some() {
        conditions.push(format!("a.created_at < ${} + INTERVAL '1 day'", param_idx));
        param_idx += 1;
    }
    if params.action.is_some() {
        conditions.push(format!("a.action = ${}", param_idx));
        param_idx += 1;
    }
    if params.user_id.is_some() {
        conditions.push(format!("a.user_id = ${}", param_idx));
        param_idx += 1;
    }
    if params.resource_type.is_some() {
        conditions.push(format!("a.resource_type = ${}", param_idx));
    }

    let where_clause = conditions.join(" AND ");

    // Fetch all logs matching filters (limit to 10000 for safety)
    let query = format!(
        r#"
        SELECT 
            a.id, 
            a.action, 
            a.resource_type, 
            a.resource_id,
            a.created_at, 
            a.user_id,
            u.name as user_name,
            a.metadata,
            a.ip_address::text as ip_address
        FROM audit_logs a
        LEFT JOIN users u ON a.user_id = u.id
        WHERE {}
        ORDER BY a.created_at DESC
        LIMIT 10000
        "#,
        where_clause
    );

    let mut query_builder = sqlx::query_as::<_, AuditLogRow>(&query)
        .bind(auth.tenant_id);
    
    if let Some(start) = params.start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = params.end_date {
        query_builder = query_builder.bind(end);
    }
    if let Some(ref action) = params.action {
        query_builder = query_builder.bind(action);
    }
    if let Some(user_id) = params.user_id {
        query_builder = query_builder.bind(user_id);
    }
    if let Some(ref resource_type) = params.resource_type {
        query_builder = query_builder.bind(resource_type);
    }

    let logs = query_builder
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to export audit logs: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Build CSV content with human-readable columns
    let mut csv_content = String::from("ID,Timestamp,User,Action,Action Display,Resource Type,Resource,Description,IP Address\n");
    
    for row in logs {
        // Extract human-readable resource name from metadata
        let resource = if let Some(meta) = &row.metadata {
            meta.get("file_name")
                .or_else(|| meta.get("folder_name"))
                .or_else(|| meta.get("new_name"))
                .or_else(|| meta.get("old_name"))
                .or_else(|| meta.get("target_user_name"))
                .or_else(|| meta.get("target_user_email"))
                .or_else(|| meta.get("deleted_user_name"))
                .or_else(|| meta.get("deleted_user_email"))
                .or_else(|| meta.get("request_name"))
                .or_else(|| meta.get("resource_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        let user_name = row.user_name.clone().unwrap_or_else(|| "System".to_string());
        let ip = row.ip_address.unwrap_or_default();
        let action_display = format_action_display(&row.action);
        let description = format_audit_description(&row.action, &user_name, &resource, &row.metadata);

        // Escape CSV fields
        let escape_csv = |s: &str| {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };

        csv_content.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            row.id,
            row.created_at.to_rfc3339(),
            escape_csv(&user_name),
            escape_csv(&row.action),
            escape_csv(&action_display),
            escape_csv(&row.resource_type),
            escape_csv(&resource),
            escape_csv(&description),
            escape_csv(&ip),
        ));
    }

    // Log the export action
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (id, tenant_id, user_id, action, resource_type, metadata, ip_address)
        VALUES ($1, $2, $3, 'audit_logs_exported', 'audit', $4, $5::inet)
        "#
    )
    .bind(Uuid::new_v4())
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(json!({
        "filters": {
            "start_date": params.start_date,
            "end_date": params.end_date,
            "action": params.action,
            "user_id": params.user_id,
            "resource_type": params.resource_type,
        }
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Return CSV response
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=\"audit_logs.csv\"")
        .body(csv_content.into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}

/// Get audit settings for a tenant
/// GET /api/audit-settings
pub async fn get_audit_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    // Check permission
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get compliance mode to check restrictions
    let compliance_mode = get_tenant_compliance_mode(&state.pool, auth.tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // Try to get existing settings, or return defaults
    let settings = sqlx::query_as::<_, AuditSettings>(
        r#"
        SELECT id, tenant_id, log_logins, log_file_operations, log_user_changes, 
               log_settings_changes, log_role_changes, retention_days, created_at, updated_at
        FROM audit_settings
        WHERE tenant_id = $1
        "#
    )
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch audit settings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match settings {
        Some(s) => Ok(Json(json!({
            "id": s.id,
            "tenant_id": s.tenant_id,
            "log_logins": s.log_logins,
            "log_file_operations": s.log_file_operations,
            "log_user_changes": s.log_user_changes,
            "log_settings_changes": s.log_settings_changes,
            "log_role_changes": s.log_role_changes,
            "retention_days": s.retention_days,
            "created_at": s.created_at,
            "updated_at": s.updated_at,
            "compliance_mode": compliance_mode,
            "compliance_locked": restrictions.audit_settings_locked,
            "settings_locked": {
                "log_logins": restrictions.audit_settings_locked,
                "log_file_operations": restrictions.audit_settings_locked,
                "log_user_changes": restrictions.audit_settings_locked,
                "log_settings_changes": restrictions.audit_settings_locked,
                "log_role_changes": restrictions.audit_settings_locked,
            }
        }))),
        None => {
            // Return defaults with compliance info
            Ok(Json(json!({
                "tenant_id": auth.tenant_id,
                "log_logins": true,
                "log_file_operations": true,
                "log_user_changes": true,
                "log_settings_changes": true,
                "log_role_changes": true,
                "retention_days": 90,
                "compliance_mode": compliance_mode,
                "compliance_locked": restrictions.audit_settings_locked,
                "settings_locked": {
                    "log_logins": restrictions.audit_settings_locked,
                    "log_file_operations": restrictions.audit_settings_locked,
                    "log_user_changes": restrictions.audit_settings_locked,
                    "log_settings_changes": restrictions.audit_settings_locked,
                    "log_role_changes": restrictions.audit_settings_locked,
                }
            })))
        }
    }
}

/// Update audit settings for a tenant
/// PUT /api/audit-settings
pub async fn update_audit_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<UpdateAuditSettingsInput>,
) -> Result<Json<Value>, StatusCode> {
    // Check permission
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get compliance mode to check restrictions
    let compliance_mode = get_tenant_compliance_mode(&state.pool, auth.tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // If compliance mode requires audit logging, prevent disabling any audit settings
    if restrictions.audit_settings_locked {
        // Check if any setting is being disabled
        let is_disabling = |opt: Option<bool>| opt == Some(false);
        
        if is_disabling(input.log_logins) && !can_modify_setting(&compliance_mode, "log_logins") {
            tracing::warn!("Cannot disable log_logins in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
        if is_disabling(input.log_file_operations) && !can_modify_setting(&compliance_mode, "log_file_operations") {
            tracing::warn!("Cannot disable log_file_operations in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
        if is_disabling(input.log_user_changes) && !can_modify_setting(&compliance_mode, "log_user_changes") {
            tracing::warn!("Cannot disable log_user_changes in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
        if is_disabling(input.log_settings_changes) && !can_modify_setting(&compliance_mode, "log_settings_changes") {
            tracing::warn!("Cannot disable log_settings_changes in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
        if is_disabling(input.log_role_changes) && !can_modify_setting(&compliance_mode, "log_role_changes") {
            tracing::warn!("Cannot disable log_role_changes in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // If compliance mode is active, force all logging to be enabled
    let (log_logins, log_file_ops, log_user_changes, log_settings_changes, log_role_changes) = 
        if restrictions.audit_logging_mandatory {
            (Some(true), Some(true), Some(true), Some(true), Some(true))
        } else {
            (input.log_logins, input.log_file_operations, input.log_user_changes, 
             input.log_settings_changes, input.log_role_changes)
        };

    // Upsert settings
    let settings = sqlx::query_as::<_, AuditSettings>(
        r#"
        INSERT INTO audit_settings (id, tenant_id, log_logins, log_file_operations, log_user_changes, 
                                    log_settings_changes, log_role_changes, retention_days)
        VALUES ($1, $2, 
                COALESCE($3, true), COALESCE($4, true), COALESCE($5, true), 
                COALESCE($6, true), COALESCE($7, true), COALESCE($8, 90))
        ON CONFLICT (tenant_id) DO UPDATE SET
            log_logins = COALESCE($3, audit_settings.log_logins),
            log_file_operations = COALESCE($4, audit_settings.log_file_operations),
            log_user_changes = COALESCE($5, audit_settings.log_user_changes),
            log_settings_changes = COALESCE($6, audit_settings.log_settings_changes),
            log_role_changes = COALESCE($7, audit_settings.log_role_changes),
            retention_days = COALESCE($8, audit_settings.retention_days),
            updated_at = NOW()
        RETURNING id, tenant_id, log_logins, log_file_operations, log_user_changes, 
                  log_settings_changes, log_role_changes, retention_days, created_at, updated_at
        "#
    )
    .bind(Uuid::new_v4())
    .bind(auth.tenant_id)
    .bind(log_logins)
    .bind(log_file_ops)
    .bind(log_user_changes)
    .bind(log_settings_changes)
    .bind(log_role_changes)
    .bind(input.retention_days)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update audit settings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Log the settings update
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (id, tenant_id, user_id, action, resource_type, metadata, ip_address)
        VALUES ($1, $2, $3, 'audit_settings_updated', 'settings', $4, $5::inet)
        "#
    )
    .bind(Uuid::new_v4())
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(json!({
        "log_logins": settings.log_logins,
        "log_file_operations": settings.log_file_operations,
        "log_user_changes": settings.log_user_changes,
        "log_settings_changes": settings.log_settings_changes,
        "log_role_changes": settings.log_role_changes,
        "retention_days": settings.retention_days,
        "compliance_mode": compliance_mode,
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({
        "id": settings.id,
        "tenant_id": settings.tenant_id,
        "log_logins": settings.log_logins,
        "log_file_operations": settings.log_file_operations,
        "log_user_changes": settings.log_user_changes,
        "log_settings_changes": settings.log_settings_changes,
        "log_role_changes": settings.log_role_changes,
        "retention_days": settings.retention_days,
        "created_at": settings.created_at,
        "updated_at": settings.updated_at,
        "compliance_locked": restrictions.audit_settings_locked,
    })))
}

/// Get available action types for filtering
/// GET /api/activity-logs/actions
pub async fn get_action_types(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let actions: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT action FROM audit_logs 
        WHERE tenant_id = $1 
        ORDER BY action
        "#
    )
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch action types: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "actions": actions })))
}

/// Get available resource types for filtering
/// GET /api/activity-logs/resource-types
pub async fn get_resource_types(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let resource_types: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT resource_type FROM audit_logs 
        WHERE tenant_id = $1 
        ORDER BY resource_type
        "#
    )
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch resource types: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "resource_types": resource_types })))
}

/// Get activity logs for a specific user
/// GET /api/users/:id/activity-logs
pub async fn get_user_activity_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<ListAuditLogsParams>,
) -> Result<Json<Value>, StatusCode> {
    // Check permission - must be Admin/SuperAdmin or viewing own logs
    if !["Admin", "SuperAdmin"].contains(&auth.role.as_str()) && auth.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify the user exists and belongs to an accessible tenant
    let user_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM users 
            WHERE id = $1 AND (tenant_id = $2 OR $3 = 'SuperAdmin')
        )
        "#
    )
    .bind(user_id)
    .bind(auth.tenant_id)
    .bind(&auth.role)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to verify user: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !user_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);

    // Build WHERE clause - always filter by user_id
    let mut conditions = vec!["a.user_id = $1".to_string()];
    let mut param_idx = 4; // $1 = user_id, $2 = limit, $3 = offset

    if params.start_date.is_some() {
        conditions.push(format!("a.created_at >= ${}", param_idx));
        param_idx += 1;
    }
    if params.end_date.is_some() {
        conditions.push(format!("a.created_at < ${} + INTERVAL '1 day'", param_idx));
        param_idx += 1;
    }
    if params.action.is_some() {
        conditions.push(format!("a.action = ${}", param_idx));
        param_idx += 1;
    }
    if params.resource_type.is_some() {
        conditions.push(format!("a.resource_type = ${}", param_idx));
    }

    let where_clause = conditions.join(" AND ");

    // Count total for pagination
    let count_query = format!(
        r#"
        SELECT COUNT(*) as count
        FROM audit_logs a
        WHERE {}
        "#,
        where_clause
    );

    let mut count_builder = sqlx::query_scalar::<_, i64>(&count_query)
        .bind(user_id);
    
    if let Some(start) = params.start_date {
        count_builder = count_builder.bind(start);
    }
    if let Some(end) = params.end_date {
        count_builder = count_builder.bind(end);
    }
    if let Some(ref action) = params.action {
        count_builder = count_builder.bind(action);
    }
    if let Some(ref resource_type) = params.resource_type {
        count_builder = count_builder.bind(resource_type);
    }

    let total = count_builder
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    // Fetch logs
    let query = format!(
        r#"
        SELECT 
            a.id, 
            a.action, 
            a.resource_type, 
            a.resource_id,
            a.created_at, 
            a.user_id,
            u.name as user_name,
            a.metadata,
            a.ip_address::text as ip_address
        FROM audit_logs a
        LEFT JOIN users u ON a.user_id = u.id
        WHERE {}
        ORDER BY a.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        where_clause
    );

    let mut query_builder = sqlx::query_as::<_, AuditLogRow>(&query)
        .bind(user_id)
        .bind(limit)
        .bind(offset);
    
    if let Some(start) = params.start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = params.end_date {
        query_builder = query_builder.bind(end);
    }
    if let Some(ref action) = params.action {
        query_builder = query_builder.bind(action);
    }
    if let Some(ref resource_type) = params.resource_type {
        query_builder = query_builder.bind(resource_type);
    }

    let logs = query_builder
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list user audit logs: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response: Vec<AuditLogResponse> = logs.into_iter().map(|row| {
        let status = match row.action.as_str() {
            "login_failed" | "security_alert" => "warning",
            _ => "success",
        };

        // Extract human-readable resource name from metadata
        let resource = if let Some(meta) = &row.metadata {
            meta.get("file_name")
                .or_else(|| meta.get("folder_name"))
                .or_else(|| meta.get("new_name"))
                .or_else(|| meta.get("old_name"))
                .or_else(|| meta.get("target_user_name"))
                .or_else(|| meta.get("target_user_email"))
                .or_else(|| meta.get("deleted_user_name"))
                .or_else(|| meta.get("deleted_user_email"))
                .or_else(|| meta.get("request_name"))
                .or_else(|| meta.get("resource_name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&row.resource_type)
                .to_string()
        } else {
            row.resource_type.clone()
        };

        // Generate human-readable action display name
        let action_display = format_action_display(&row.action);
        
        // Generate full human-readable description
        let user_name = row.user_name.clone().unwrap_or_else(|| "System".to_string());
        let description = format_audit_description(&row.action, &user_name, &resource, &row.metadata);

        AuditLogResponse {
            id: row.id.to_string(),
            user: user_name,
            user_id: row.user_id.map(|id| id.to_string()),
            action: row.action,
            action_display,
            resource,
            resource_type: row.resource_type,
            description,
            timestamp: row.created_at.to_rfc3339(),
            status: status.to_string(),
            ip_address: row.ip_address,
            metadata: row.metadata,
        }
    }).collect();

    Ok(Json(json!(AuditLogsListResponse {
        logs: response,
        total,
        limit,
        offset,
    })))
}

// ==================== Helper Functions ====================

/// Format action into human-readable display text
fn format_action_display(action: &str) -> String {
    match action {
        // File operations
        "file_upload" => "Uploaded file".to_string(),
        "file_download" => "Downloaded file".to_string(),
        "file_preview" => "Previewed file".to_string(),
        "file_rename" => "Renamed file".to_string(),
        "file_delete" => "Deleted file".to_string(),
        "file_move" => "Moved file".to_string(),
        "file_lock" => "Locked file".to_string(),
        "file_unlock" => "Unlocked file".to_string(),
        "file_shared" => "Shared file".to_string(),
        "file_restore" => "Restored file".to_string(),
        "file_permanent_delete" => "Permanently deleted file".to_string(),
        "folder_download" => "Downloaded folder".to_string(),
        "folder_create" => "Created folder".to_string(),
        "private_files_view" => "Viewed private files".to_string(),
        
        // User operations
        "user_created" => "Created user".to_string(),
        "user_updated" => "Updated user".to_string(),
        "user_deleted" => "Deleted user".to_string(),
        "user_permanently_deleted" => "Permanently deleted user".to_string(),
        "user_suspended" => "Suspended user".to_string(),
        "user_activated" => "Activated user".to_string(),
        "admin_reset_password" => "Reset password".to_string(),
        "send_password_reset_email" => "Sent password reset email".to_string(),
        "admin_change_email" => "Changed email".to_string(),
        "role_change" => "Changed role".to_string(),
        
        // Authentication
        "login" => "Logged in".to_string(),
        "login_success" => "Logged in".to_string(),
        "login_failed" => "Failed login attempt".to_string(),
        "logout" => "Logged out".to_string(),
        "session_revoked" => "Revoked session".to_string(),
        "password_changed" => "Changed password".to_string(),
        "mfa_enabled" => "Enabled two-factor auth".to_string(),
        "mfa_disabled" => "Disabled two-factor auth".to_string(),
        
        // Settings
        "settings_updated" => "Updated settings".to_string(),
        "compliance_settings_updated" => "Updated compliance settings".to_string(),
        "audit_settings_updated" => "Updated audit settings".to_string(),
        
        // Share operations
        "share_created" => "Created share link".to_string(),
        "share_accessed" => "Accessed shared file".to_string(),
        "share_deleted" => "Deleted share link".to_string(),
        
        // File requests
        "file_request_created" => "Created file request".to_string(),
        "file_request_upload" => "Uploaded to file request".to_string(),
        
        // Tenant operations
        "tenant_created" => "Created company".to_string(),
        "tenant_updated" => "Updated company".to_string(),
        "tenant_suspended" => "Suspended company".to_string(),
        "tenant_deleted" => "Deleted company".to_string(),
        
        // Security
        "security_alert" => "Security alert".to_string(),
        
        // AI operations
        "ai_summarize" => "Generated AI summary".to_string(),
        "ai_summary_viewed" => "Viewed AI summary".to_string(),
        "ai_answer" => "Asked AI a question".to_string(),
        "ai_settings_updated" => "Updated AI settings".to_string(),
        
        // Default: convert snake_case to Title Case
        _ => action
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Generate a full human-readable description of an audit event
fn format_audit_description(action: &str, user: &str, resource: &str, metadata: &Option<Value>) -> String {
    match action {
        // File operations with specific details
        "file_upload" => format!("{} uploaded \"{}\"", user, resource),
        "file_download" => format!("{} downloaded \"{}\"", user, resource),
        "file_preview" => format!("{} previewed \"{}\"", user, resource),
        "file_rename" => {
            if let Some(meta) = metadata {
                let old_name = meta.get("old_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                let new_name = meta.get("new_name").and_then(|v| v.as_str()).unwrap_or(resource);
                format!("{} renamed \"{}\" to \"{}\"", user, old_name, new_name)
            } else {
                format!("{} renamed a file to \"{}\"", user, resource)
            }
        },
        "file_delete" => format!("{} deleted \"{}\"", user, resource),
        "file_move" => {
            if let Some(meta) = metadata {
                let from = meta.get("from_path").and_then(|v| v.as_str()).unwrap_or("unknown");
                let to = meta.get("to_path").and_then(|v| v.as_str()).unwrap_or("unknown");
                format!("{} moved file from \"{}\" to \"{}\"", user, from, to)
            } else {
                format!("{} moved \"{}\"", user, resource)
            }
        },
        "file_lock" => format!("{} locked \"{}\"", user, resource),
        "file_unlock" => format!("{} unlocked \"{}\"", user, resource),
        "file_shared" => {
            if let Some(meta) = metadata {
                let is_public = meta.get("is_public").and_then(|v| v.as_bool()).unwrap_or(false);
                let share_type = if is_public { "public" } else { "organization" };
                format!("{} shared \"{}\" ({} link)", user, resource, share_type)
            } else {
                format!("{} shared \"{}\"", user, resource)
            }
        },
        "file_restore" => format!("{} restored \"{}\" from trash", user, resource),
        "file_permanent_delete" => format!("{} permanently deleted \"{}\"", user, resource),
        "folder_download" => {
            if let Some(meta) = metadata {
                let count = meta.get("file_count").and_then(|v| v.as_i64()).unwrap_or(0);
                format!("{} downloaded folder \"{}\" ({} files)", user, resource, count)
            } else {
                format!("{} downloaded folder \"{}\"", user, resource)
            }
        },
        "folder_create" => format!("{} created folder \"{}\"", user, resource),
        "private_files_view" => format!("{} viewed private files", user),
        
        // User operations
        "user_created" => format!("{} created user account for {}", user, resource),
        "user_updated" => format!("{} updated user {}", user, resource),
        "user_deleted" | "user_permanently_deleted" => format!("{} deleted user {}", user, resource),
        "user_suspended" => format!("{} suspended user {}", user, resource),
        "user_activated" => format!("{} activated user {}", user, resource),
        "admin_reset_password" => format!("{} reset password for {}", user, resource),
        "send_password_reset_email" => format!("{} sent password reset email to {}", user, resource),
        "admin_change_email" => {
            if let Some(meta) = metadata {
                let old_email = meta.get("old_email").and_then(|v| v.as_str()).unwrap_or("unknown");
                let new_email = meta.get("new_email").and_then(|v| v.as_str()).unwrap_or("unknown");
                format!("{} changed email from {} to {}", user, old_email, new_email)
            } else {
                format!("{} changed email for {}", user, resource)
            }
        },
        "role_change" => {
            if let Some(meta) = metadata {
                let old_role = meta.get("old_role").and_then(|v| v.as_str()).unwrap_or("unknown");
                let new_role = meta.get("new_role").and_then(|v| v.as_str()).unwrap_or("unknown");
                format!("{} changed role from {} to {} for {}", user, old_role, new_role, resource)
            } else {
                format!("{} changed role for {}", user, resource)
            }
        },
        
        // Authentication
        "login" | "login_success" => format!("{} logged in", user),
        "login_failed" => format!("Failed login attempt for {}", resource),
        "logout" => format!("{} logged out", user),
        "session_revoked" => format!("{} revoked a session", user),
        "password_changed" => format!("{} changed their password", user),
        "mfa_enabled" => format!("{} enabled two-factor authentication", user),
        "mfa_disabled" => format!("{} disabled two-factor authentication", user),
        
        // Settings
        "settings_updated" => format!("{} updated settings", user),
        "compliance_settings_updated" => format!("{} updated compliance settings", user),
        "audit_settings_updated" => format!("{} updated audit settings", user),
        
        // Share operations
        "share_created" => format!("{} created a share link for \"{}\"", user, resource),
        "share_accessed" => format!("Share link for \"{}\" was accessed", resource),
        "share_deleted" => format!("{} deleted share link for \"{}\"", user, resource),
        
        // File requests
        "file_request_created" => format!("{} created file request \"{}\"", user, resource),
        "file_request_upload" => {
            if let Some(meta) = metadata {
                let uploader = meta.get("uploader_name").and_then(|v| v.as_str()).unwrap_or("Someone");
                format!("{} uploaded to file request \"{}\"", uploader, resource)
            } else {
                format!("File uploaded to request \"{}\"", resource)
            }
        },
        
        // Tenant operations
        "tenant_created" => format!("{} created company \"{}\"", user, resource),
        "tenant_updated" => format!("{} updated company settings", user),
        "tenant_suspended" => format!("{} suspended company \"{}\"", user, resource),
        "tenant_deleted" => format!("{} deleted company \"{}\"", user, resource),
        
        // Security
        "security_alert" => format!("Security alert: {}", resource),
        
        // AI operations
        "ai_summarize" => {
            if let Some(meta) = metadata {
                let file_name = meta.get("file_name").and_then(|v| v.as_str()).unwrap_or(resource);
                format!("{} generated AI summary for \"{}\"", user, file_name)
            } else {
                format!("{} generated AI summary for \"{}\"", user, resource)
            }
        },
        "ai_summary_viewed" => {
            if let Some(meta) = metadata {
                let file_name = meta.get("file_name").and_then(|v| v.as_str()).unwrap_or(resource);
                format!("{} viewed AI summary for \"{}\"", user, file_name)
            } else {
                format!("{} viewed AI summary for \"{}\"", user, resource)
            }
        },
        "ai_answer" => {
            if let Some(meta) = metadata {
                let file_name = meta.get("file_name").and_then(|v| v.as_str()).unwrap_or(resource);
                format!("{} asked AI about \"{}\"", user, file_name)
            } else {
                format!("{} asked AI about \"{}\"", user, resource)
            }
        },
        "ai_settings_updated" => format!("{} updated AI settings", user),
        
        // Default fallback
        _ => {
            let action_text = format_action_display(action);
            if resource != "file" && resource != "user" && resource != "settings" {
                format!("{} - {} ({})", user, action_text, resource)
            } else {
                format!("{} - {}", user, action_text)
            }
        },
    }
}

/// Check if an action should be logged based on tenant's audit settings
#[allow(dead_code)]
pub async fn should_log_action(pool: &sqlx::PgPool, tenant_id: Uuid, action_category: &str) -> bool {
    let settings = sqlx::query_as::<_, AuditSettings>(
        r#"
        SELECT id, tenant_id, log_logins, log_file_operations, log_user_changes, 
               log_settings_changes, log_role_changes, retention_days, created_at, updated_at
        FROM audit_settings
        WHERE tenant_id = $1
        "#
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match settings {
        Some(s) => match action_category {
            "login" => s.log_logins,
            "file" => s.log_file_operations,
            "user" => s.log_user_changes,
            "settings" => s.log_settings_changes,
            "role" => s.log_role_changes,
            _ => true, // Log unknown categories by default
        },
        None => true, // Log everything by default if no settings
    }
}
