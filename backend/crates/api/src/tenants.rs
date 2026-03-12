use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    Extension,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
use crate::AppState;
use crate::compliance::{ComplianceRestrictions, get_tenant_compliance_mode, can_modify_setting};
use clovalink_auth::{AuthUser, require_super_admin, generate_token};
use clovalink_core::models::{Tenant, CreateTenantInput, UpdateTenantInput, User};

#[derive(Deserialize)]
pub struct TenantFilters {
    pub status: Option<String>,
    pub plan: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// List all tenants/companies
/// GET /api/tenants
/// SuperAdmin only - for managing companies
pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(filters): Query<TenantFilters>,
) -> Result<Json<Value>, StatusCode> {
    // SECURITY: Only SuperAdmin can list/manage all companies
    require_super_admin(&auth)?;
    
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let mut query = String::from("SELECT * FROM tenants WHERE 1=1");
    let mut param_count = 1;
    
    if filters.status.is_some() {
        query.push_str(&format!(" AND status = ${}", param_count));
        param_count += 1;
    }
    if filters.plan.is_some() {
        query.push_str(&format!(" AND plan = ${}", param_count));
        param_count += 1;
    }
    if filters.search.is_some() {
        query.push_str(&format!(" AND (name ILIKE ${} OR domain ILIKE ${})", param_count, param_count));
        param_count += 1;
    }
    
    query.push_str(" ORDER BY created_at DESC");
    query.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));

    let mut db_query = sqlx::query_as::<_, Tenant>(&query);

    if let Some(status) = filters.status {
        db_query = db_query.bind(status);
    }
    if let Some(plan) = filters.plan {
        db_query = db_query.bind(plan);
    }
    if let Some(search) = filters.search {
        let search_pattern = format!("%{}%", search);
        db_query = db_query.bind(search_pattern.clone());
    }

    let tenants = db_query
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list tenants: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Count users per tenant and calculate actual storage
    // Include users whose primary tenant matches OR who have this tenant in allowed_tenant_ids
    let mut results = Vec::new();
    for tenant in tenants {
        let user_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM users WHERE tenant_id = $1 OR $1 = ANY(allowed_tenant_ids)"
        )
        .bind(tenant.id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        // Calculate actual storage from files_metadata (not stale tenant.storage_used_bytes)
        let actual_storage: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM files_metadata WHERE tenant_id = $1 AND is_deleted = false AND is_directory = false"
        )
        .bind(tenant.id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

        let restrictions = ComplianceRestrictions::for_mode(&tenant.compliance_mode);

        results.push(json!({
            "id": tenant.id,
            "name": tenant.name,
            "domain": tenant.domain,
            "plan": tenant.plan,
            "status": tenant.status,
            "compliance_mode": tenant.compliance_mode,
            "retention_policy_days": tenant.retention_policy_days,
            "storage_used_bytes": actual_storage,
            "storage_quota_bytes": tenant.storage_quota_bytes,
            "max_upload_size_bytes": tenant.max_upload_size_bytes,
            "mfa_required": tenant.mfa_required,
            "session_timeout_minutes": tenant.session_timeout_minutes,
            "public_sharing_enabled": tenant.public_sharing_enabled,
            "data_export_enabled": tenant.data_export_enabled.unwrap_or(true),
            "approval_workflow_enabled": tenant.approval_workflow_enabled.unwrap_or(false),
            "backup_enabled": tenant.backup_enabled.unwrap_or(true),
            "auto_backup_enabled": tenant.auto_backup_enabled.unwrap_or(false),
            "auto_backup_cron": tenant.auto_backup_cron.as_deref().unwrap_or("0 2 * * 0"),
            "auto_backup_retention_count": tenant.auto_backup_retention_count.unwrap_or(5),
            "enable_totp": tenant.enable_totp,
            "auth_methods": tenant.auth_methods,
            "user_count": user_count,
            "created_at": tenant.created_at,
            "restrictions": restrictions,
            "smtp_host": tenant.smtp_host,
            "smtp_port": tenant.smtp_port,
            "smtp_username": tenant.smtp_username,
            "smtp_password": tenant.smtp_password,
            "smtp_from": tenant.smtp_from,
            "smtp_secure": tenant.smtp_secure,
        }));
    }

    Ok(Json(json!(results)))
}

/// List tenants accessible to the current user
/// GET /api/tenants/accessible
/// Returns the user's primary tenant plus any tenants from allowed_tenant_ids
/// SuperAdmins get ALL active tenants (god-mode access)
/// Available to all authenticated users
pub async fn accessible_tenants(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    // Get the user's primary tenant
    let user_row = sqlx::query(
        "SELECT tenant_id, allowed_tenant_ids FROM users WHERE id = $1"
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user for accessible tenants: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let primary_tenant_id: Uuid = user_row.get("tenant_id");

    // SuperAdmins get ALL active tenants
    let tenants = if auth.role == "SuperAdmin" {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE status = 'active' ORDER BY name"
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch all tenants for SuperAdmin: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    } else {
        // Non-SuperAdmins get their primary tenant + allowed_tenant_ids
        let allowed_ids: Option<Vec<Uuid>> = user_row.get("allowed_tenant_ids");

        // Build list of all tenant IDs to fetch
        let mut tenant_ids: Vec<Uuid> = vec![primary_tenant_id];
        if let Some(ref ids) = allowed_ids {
            for id in ids {
                if !tenant_ids.contains(id) {
                    tenant_ids.push(*id);
                }
            }
        }

        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE id = ANY($1) AND status = 'active' ORDER BY name"
        )
        .bind(&tenant_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch accessible tenants: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let mut results = Vec::new();
    for tenant in tenants {
        let restrictions = ComplianceRestrictions::for_mode(&tenant.compliance_mode);
        results.push(json!({
            "id": tenant.id,
            "name": tenant.name,
            "domain": tenant.domain,
            "plan": tenant.plan,
            "status": tenant.status,
            "compliance_mode": tenant.compliance_mode,
            "retention_policy_days": tenant.retention_policy_days,
            "data_export_enabled": tenant.data_export_enabled.unwrap_or(true),
            "approval_workflow_enabled": tenant.approval_workflow_enabled.unwrap_or(false),
            "backup_enabled": tenant.backup_enabled.unwrap_or(true),
            "auto_backup_enabled": tenant.auto_backup_enabled.unwrap_or(false),
            "auto_backup_cron": tenant.auto_backup_cron.as_deref().unwrap_or("0 2 * * 0"),
            "auto_backup_retention_count": tenant.auto_backup_retention_count.unwrap_or(5),
            "enable_totp": tenant.enable_totp,
            "auth_methods": tenant.auth_methods,
            "is_primary": tenant.id == primary_tenant_id,
            "restrictions": restrictions,
            "storage_used_bytes": 0,
            "storage_quota_bytes": tenant.storage_quota_bytes,
            "max_upload_size_bytes": tenant.max_upload_size_bytes,
            "smtp_host": tenant.smtp_host,
            "smtp_port": tenant.smtp_port,
            "smtp_username": tenant.smtp_username,
            "smtp_password": tenant.smtp_password,
            "smtp_from": tenant.smtp_from,
            "smtp_secure": tenant.smtp_secure,
        }));
    }

    Ok(Json(json!(results)))
}

/// Create new tenant
/// POST /api/tenants
/// Only SuperAdmin can create tenants
pub async fn create_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<CreateTenantInput>,
) -> Result<Json<Value>, StatusCode> {
    // Only SuperAdmin can create tenants
    require_super_admin(&auth)?;

    // Start transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        tracing::error!("Failed to start transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let tenant = sqlx::query_as::<_, Tenant>(
        r#"
        INSERT INTO tenants (name, domain, plan, storage_quota_bytes)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#
    )
    .bind(&input.name)
    .bind(&input.domain)
    .bind(&input.plan)
    .bind(input.storage_quota_bytes)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create tenant: {:?}", e);
        if e.to_string().contains("unique") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    // Create default departments if provided
    if let Some(departments) = &input.departments {
        for dept_name in departments {
            sqlx::query!(
                "INSERT INTO departments (tenant_id, name) VALUES ($1, $2)",
                tenant.id,
                dept_name
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create department: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "id": tenant.id,
        "name": tenant.name,
        "domain": tenant.domain,
        "plan": tenant.plan,
        "status": tenant.status,
        "storage_quota_bytes": tenant.storage_quota_bytes,
        "created_at": tenant.created_at,
    })))
}

/// Update tenant
/// PUT /api/tenants/:id
/// Only SuperAdmin can update tenants
pub async fn update_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTenantInput>,
) -> Result<Json<Value>, StatusCode> {
    // Only SuperAdmin can update tenants
    require_super_admin(&auth)?;

    let mut updates = Vec::new();
    let mut param_count = 2;

    if let Some(_name) = &input.name {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if let Some(_domain) = &input.domain {
        updates.push(format!("domain = ${}", param_count));
        param_count += 1;
    }
    if let Some(_plan) = &input.plan {
        updates.push(format!("plan = ${}", param_count));
        param_count += 1;
    }
    if let Some(_status) = &input.status {
        updates.push(format!("status = ${}", param_count));
        param_count += 1;
    }
    if let Some(_compliance_mode) = &input.compliance_mode {
        updates.push(format!("compliance_mode = ${}", param_count));
        param_count += 1;
    }
    if let Some(_storage_quota_bytes) = &input.storage_quota_bytes {
        updates.push(format!("storage_quota_bytes = ${}", param_count));
        param_count += 1;
    }
    if let Some(_retention_policy_days) = &input.retention_policy_days {
        updates.push(format!("retention_policy_days = ${}", param_count));
        param_count += 1;
    }
    if let Some(_max_upload_size_bytes) = &input.max_upload_size_bytes {
        updates.push(format!("max_upload_size_bytes = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_host) = &input.smtp_host {
        updates.push(format!("smtp_host = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_port) = &input.smtp_port {
        updates.push(format!("smtp_port = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_username) = &input.smtp_username {
        updates.push(format!("smtp_username = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_password) = &input.smtp_password {
        updates.push(format!("smtp_password = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_from) = &input.smtp_from {
        updates.push(format!("smtp_from = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_secure) = &input.smtp_secure {
        updates.push(format!("smtp_secure = ${}", param_count));
        param_count += 1;
    }
    if let Some(_enable_totp) = &input.enable_totp {
        updates.push(format!("enable_totp = ${}", param_count));
        param_count += 1;
    }
    if let Some(_enable_passkeys) = &input.enable_passkeys {
        updates.push(format!("enable_passkeys = ${}", param_count));
        param_count += 1;
    }
    if let Some(_data_export_enabled) = &input.data_export_enabled {
        updates.push(format!("data_export_enabled = ${}", param_count));
        param_count += 1;
    }
    if let Some(_approval_workflow_enabled) = &input.approval_workflow_enabled {
        updates.push(format!("approval_workflow_enabled = ${}", param_count));
        param_count += 1;
    }
    if let Some(_backup_enabled) = &input.backup_enabled {
        updates.push(format!("backup_enabled = ${}", param_count));
        param_count += 1;
    }
    if let Some(auto_backup_enabled) = &input.auto_backup_enabled {
        if *auto_backup_enabled && !crate::settings_backup::is_master_key_configured() {
            return Err(StatusCode::BAD_REQUEST);
        }
        updates.push(format!("auto_backup_enabled = ${}", param_count));
        param_count += 1;
    }
    if let Some(_auto_backup_cron) = &input.auto_backup_cron {
        updates.push(format!("auto_backup_cron = ${}", param_count));
        param_count += 1;
    }
    if let Some(_auto_backup_retention_count) = &input.auto_backup_retention_count {
        updates.push(format!("auto_backup_retention_count = ${}", param_count));
    }

    if updates.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    updates.push("updated_at = NOW()".to_string());
    let query = format!(
        "UPDATE tenants SET {} WHERE id = $1 RETURNING *",
        updates.join(", ")
    );

    let mut db_query = sqlx::query_as::<_, Tenant>(&query).bind(id);

    if let Some(name) = input.name {
        db_query = db_query.bind(name);
    }
    if let Some(domain) = input.domain {
        db_query = db_query.bind(domain);
    }
    if let Some(plan) = input.plan {
        db_query = db_query.bind(plan);
    }
    if let Some(status) = input.status {
        db_query = db_query.bind(status);
    }
    if let Some(compliance_mode) = input.compliance_mode {
        db_query = db_query.bind(compliance_mode);
    }
    if let Some(storage_quota_bytes) = input.storage_quota_bytes {
        db_query = db_query.bind(storage_quota_bytes);
    }
    if let Some(retention_policy_days) = input.retention_policy_days {
        db_query = db_query.bind(retention_policy_days);
    }
    if let Some(max_upload_size_bytes) = input.max_upload_size_bytes {
        db_query = db_query.bind(max_upload_size_bytes);
    }
    if let Some(smtp_host) = input.smtp_host {
        db_query = db_query.bind(smtp_host);
    }
    if let Some(smtp_port) = input.smtp_port {
        db_query = db_query.bind(smtp_port);
    }
    if let Some(smtp_username) = input.smtp_username {
        db_query = db_query.bind(smtp_username);
    }
    if let Some(smtp_password) = input.smtp_password {
        db_query = db_query.bind(smtp_password);
    }
    if let Some(smtp_from) = input.smtp_from {
        db_query = db_query.bind(smtp_from);
    }
    if let Some(smtp_secure) = input.smtp_secure {
        db_query = db_query.bind(smtp_secure);
    }
    if let Some(enable_totp) = input.enable_totp {
        db_query = db_query.bind(enable_totp);
    }
    if let Some(enable_passkeys) = input.enable_passkeys {
        db_query = db_query.bind(enable_passkeys);
    }
    if let Some(data_export_enabled) = input.data_export_enabled {
        db_query = db_query.bind(data_export_enabled);
    }
    if let Some(approval_workflow_enabled) = input.approval_workflow_enabled {
        db_query = db_query.bind(approval_workflow_enabled);
    }
    if let Some(backup_enabled) = input.backup_enabled {
        db_query = db_query.bind(backup_enabled);
    }
    if let Some(auto_backup_enabled) = input.auto_backup_enabled {
        db_query = db_query.bind(auto_backup_enabled);
    }
    if let Some(auto_backup_cron) = input.auto_backup_cron {
        db_query = db_query.bind(auto_backup_cron);
    }
    if let Some(auto_backup_retention_count) = input.auto_backup_retention_count {
        db_query = db_query.bind(auto_backup_retention_count);
    }

    let tenant = db_query
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Invalidate ALL caches after tenant update (compliance, tenant, and user caches)
    // This ensures the /api/auth/me endpoint returns fresh data
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let compliance_key = keys::compliance(id);
        let tenant_key = keys::tenant(id);
        let _ = cache.delete(&compliance_key).await;
        let _ = cache.delete(&tenant_key).await;
        // Clear all user caches to ensure fresh tenant data on next /api/auth/me call
        let _ = cache.delete_pattern("clovalink:user:*").await;
        tracing::info!("Invalidated all caches for tenant {} after update", id);
    }

    Ok(Json(json!({
        "id": tenant.id,
        "name": tenant.name,
        "domain": tenant.domain,
        "plan": tenant.plan,
        "status": tenant.status,
        "compliance_mode": tenant.compliance_mode,
        "storage_quota_bytes": tenant.storage_quota_bytes,
        "retention_policy_days": tenant.retention_policy_days,
        "data_export_enabled": tenant.data_export_enabled.unwrap_or(true),
        "updated_at": tenant.updated_at,
    })))
}

/// Edit my company (for company owners/admins)
/// PUT /api/tenants/:id/edit
/// Owners and Admins can edit their own company
pub async fn edit_my_company(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateTenantInput>,
) -> Result<Json<Value>, StatusCode> {
    // Debug logging
    tracing::info!("edit_my_company called: user_id={}, user_tenant_id={}, user_role={}, requested_tenant_id={}", 
        auth.user_id, auth.tenant_id, auth.role, id);
    
    // Check if user is Owner or Admin of this tenant
    if auth.tenant_id != id {
        tracing::warn!("Forbidden: tenant_id mismatch. user_tenant={}, requested={}", auth.tenant_id, id);
        return Err(StatusCode::FORBIDDEN);
    }
    if auth.role != "Owner" && auth.role != "Admin" && auth.role != "SuperAdmin" {
        tracing::warn!("Forbidden: insufficient role. user_role={}", auth.role);
        return Err(StatusCode::FORBIDDEN);
    }
    
    tracing::info!("Authorization passed for user {} (role={}) to edit tenant {}", auth.user_id, auth.role, id);

    // Get compliance mode to check restrictions
    let compliance_mode = get_tenant_compliance_mode(&state.pool, id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // Check if trying to disable MFA when compliance requires it
    if let Some(enable_totp) = input.enable_totp {
        if !enable_totp && restrictions.mfa_locked {
            tracing::warn!("Cannot disable MFA in {} mode", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Check retention policy against minimum
    if let Some(retention_days) = input.retention_policy_days {
        if let Some(min_days) = restrictions.min_retention_days {
            if retention_days < min_days {
                tracing::warn!("Retention days {} below minimum {} for {} mode", 
                    retention_days, min_days, compliance_mode);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    let mut updates = Vec::new();
    let mut param_count = 2;

    if let Some(_retention_policy_days) = &input.retention_policy_days {
        if can_modify_setting(&compliance_mode, "retention_policy_days") || auth.role == "SuperAdmin" {
            updates.push(format!("retention_policy_days = ${}", param_count));
            param_count += 1;
        }
    }
    if let Some(_smtp_host) = &input.smtp_host {
        updates.push(format!("smtp_host = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_port) = &input.smtp_port {
        updates.push(format!("smtp_port = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_username) = &input.smtp_username {
        updates.push(format!("smtp_username = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_password) = &input.smtp_password {
        updates.push(format!("smtp_password = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_from) = &input.smtp_from {
        updates.push(format!("smtp_from = ${}", param_count));
        param_count += 1;
    }
    if let Some(_smtp_secure) = &input.smtp_secure {
        updates.push(format!("smtp_secure = ${}", param_count));
        param_count += 1;
    }
    if let Some(enable_totp) = &input.enable_totp {
        // Only allow modifying TOTP if not locked by compliance
        if can_modify_setting(&compliance_mode, "enable_totp") || *enable_totp {
            // Can always enable, but can only disable if not locked
            updates.push(format!("enable_totp = ${}", param_count));
            param_count += 1;
        }
    }
    // Handle mfa_required field
    if let Some(_mfa_required) = &input.mfa_required {
        if can_modify_setting(&compliance_mode, "mfa_required") || auth.role == "SuperAdmin" {
            updates.push(format!("mfa_required = ${}", param_count));
            param_count += 1;
        }
    }
    // Handle session_timeout_minutes field
    if let Some(_session_timeout) = &input.session_timeout_minutes {
        if can_modify_setting(&compliance_mode, "session_timeout_minutes") || auth.role == "SuperAdmin" {
            updates.push(format!("session_timeout_minutes = ${}", param_count));
            param_count += 1;
        }
    }
    // Handle public_sharing_enabled field
    if let Some(_public_sharing) = &input.public_sharing_enabled {
        if can_modify_setting(&compliance_mode, "public_sharing_enabled") || auth.role == "SuperAdmin" {
            updates.push(format!("public_sharing_enabled = ${}", param_count));
            param_count += 1;
        }
    }
    // Handle storage_quota_bytes field (Admins can set storage limits)
    if let Some(_storage_quota_bytes) = &input.storage_quota_bytes {
        updates.push(format!("storage_quota_bytes = ${}", param_count));
        param_count += 1;
    }
    // Handle max_upload_size_bytes field (Admins can set upload limits)
    if let Some(_max_upload_size_bytes) = &input.max_upload_size_bytes {
        updates.push(format!("max_upload_size_bytes = ${}", param_count));
        param_count += 1;
    }
    // Handle approval_workflow_enabled field (Admins can toggle document approval)
    if let Some(_approval_workflow_enabled) = &input.approval_workflow_enabled {
        updates.push(format!("approval_workflow_enabled = ${}", param_count));
        param_count += 1;
    }
    // Handle backup_enabled field (Admins can toggle backup)
    if let Some(_backup_enabled) = &input.backup_enabled {
        updates.push(format!("backup_enabled = ${}", param_count));
        param_count += 1;
    }
    // Handle auto_backup fields (SuperAdmin only, requires password confirmation)
    let has_backup_changes = input.auto_backup_enabled.is_some()
        || input.auto_backup_cron.is_some()
        || input.auto_backup_retention_count.is_some();
    if auth.role == "SuperAdmin" && has_backup_changes {
        crate::settings_backup::verify_password_confirmation(&state.pool, auth.user_id, &headers).await?;
    }
    if auth.role == "SuperAdmin" {
        if let Some(auto_backup_enabled) = &input.auto_backup_enabled {
            if *auto_backup_enabled && !crate::settings_backup::is_master_key_configured() {
                return Err(StatusCode::BAD_REQUEST);
            }
            updates.push(format!("auto_backup_enabled = ${}", param_count));
            param_count += 1;
        }
        if let Some(_auto_backup_cron) = &input.auto_backup_cron {
            updates.push(format!("auto_backup_cron = ${}", param_count));
            param_count += 1;
        }
        if let Some(_auto_backup_retention_count) = &input.auto_backup_retention_count {
            updates.push(format!("auto_backup_retention_count = ${}", param_count));
        }
    }

    if updates.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    updates.push("updated_at = NOW()".to_string());
    let query = format!(
        "UPDATE tenants SET {} WHERE id = $1 RETURNING *",
        updates.join(", ")
    );

    let mut db_query = sqlx::query_as::<_, Tenant>(&query).bind(id);

    if let Some(retention_policy_days) = input.retention_policy_days {
        if can_modify_setting(&compliance_mode, "retention_policy_days") || auth.role == "SuperAdmin" {
            db_query = db_query.bind(retention_policy_days);
        }
    }
    if let Some(smtp_host) = input.smtp_host {
        db_query = db_query.bind(smtp_host);
    }
    if let Some(smtp_port) = input.smtp_port {
        db_query = db_query.bind(smtp_port);
    }
    if let Some(smtp_username) = input.smtp_username {
        db_query = db_query.bind(smtp_username);
    }
    if let Some(smtp_password) = input.smtp_password {
        db_query = db_query.bind(smtp_password);
    }
    if let Some(smtp_from) = input.smtp_from {
        db_query = db_query.bind(smtp_from);
    }
    if let Some(smtp_secure) = input.smtp_secure {
        db_query = db_query.bind(smtp_secure);
    }
    if let Some(enable_totp) = input.enable_totp {
        if can_modify_setting(&compliance_mode, "enable_totp") || enable_totp {
            db_query = db_query.bind(enable_totp);
        }
    }
    if let Some(mfa_required) = input.mfa_required {
        if can_modify_setting(&compliance_mode, "mfa_required") || auth.role == "SuperAdmin" {
            db_query = db_query.bind(mfa_required);
        }
    }
    if let Some(session_timeout) = input.session_timeout_minutes {
        if can_modify_setting(&compliance_mode, "session_timeout_minutes") || auth.role == "SuperAdmin" {
            db_query = db_query.bind(session_timeout);
        }
    }
    if let Some(public_sharing) = input.public_sharing_enabled {
        if can_modify_setting(&compliance_mode, "public_sharing_enabled") || auth.role == "SuperAdmin" {
            db_query = db_query.bind(public_sharing);
        }
    }
    if let Some(storage_quota_bytes) = input.storage_quota_bytes {
        db_query = db_query.bind(storage_quota_bytes);
    }
    if let Some(max_upload_size_bytes) = input.max_upload_size_bytes {
        db_query = db_query.bind(max_upload_size_bytes);
    }
    if let Some(approval_workflow_enabled) = input.approval_workflow_enabled {
        db_query = db_query.bind(approval_workflow_enabled);
    }
    if let Some(backup_enabled) = input.backup_enabled {
        db_query = db_query.bind(backup_enabled);
    }
    if auth.role == "SuperAdmin" {
        if let Some(auto_backup_enabled) = input.auto_backup_enabled {
            db_query = db_query.bind(auto_backup_enabled);
        }
        if let Some(auto_backup_cron) = input.auto_backup_cron {
            db_query = db_query.bind(auto_backup_cron);
        }
        if let Some(auto_backup_retention_count) = input.auto_backup_retention_count {
            db_query = db_query.bind(auto_backup_retention_count);
        }
    }

    let tenant = db_query
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Log the settings change
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, metadata, ip_address)
        VALUES ($1, $2, 'tenant_settings_updated', 'tenant', $3, $4::inet)
        "#
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(json!({
        "compliance_mode": compliance_mode,
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Invalidate ALL caches after tenant update (compliance, tenant, and user caches)
    // This ensures the /api/auth/me endpoint returns fresh data
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let compliance_key = keys::compliance(id);
        let tenant_key = keys::tenant(id);
        let _ = cache.delete(&compliance_key).await;
        let _ = cache.delete(&tenant_key).await;
        // Clear all user caches to ensure fresh tenant data on next /api/auth/me call
        let _ = cache.delete_pattern("clovalink:user:*").await;
        tracing::info!("Invalidated all caches for tenant {} after edit", id);
    }

    Ok(Json(json!({
        "id": tenant.id,
        "name": tenant.name,
        "domain": tenant.domain,
        "plan": tenant.plan,
        "status": tenant.status,
        "compliance_mode": tenant.compliance_mode,
        "storage_quota_bytes": tenant.storage_quota_bytes,
        "max_upload_size_bytes": tenant.max_upload_size_bytes,
        "retention_policy_days": tenant.retention_policy_days,
        "smtp_host": tenant.smtp_host,
        "smtp_port": tenant.smtp_port,
        "smtp_username": tenant.smtp_username,
        "smtp_from": tenant.smtp_from,
        "smtp_secure": tenant.smtp_secure,
        "enable_totp": tenant.enable_totp,
        "mfa_required": tenant.mfa_required,
        "session_timeout_minutes": tenant.session_timeout_minutes,
        "public_sharing_enabled": tenant.public_sharing_enabled,
        "data_export_enabled": tenant.data_export_enabled.unwrap_or(true),
        "restrictions": restrictions,
    })))
}

/// Switch active tenant (generates new token for different tenant)
/// POST /api/tenants/switch/:tenant_id
/// User must have access to the target tenant
pub async fn switch_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    tracing::info!("Switch tenant request: user={}, target_tenant={}, role={}", auth.user_id, tenant_id, auth.role);
    
    // Verify user has access to this tenant
    // Only SuperAdmin can switch to any tenant
    if auth.role.as_str() != "SuperAdmin" {
        // Check if user belongs to the target tenant OR has it in allowed_tenant_ids
        let user_row = sqlx::query(
            "SELECT id, tenant_id, allowed_tenant_ids FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user for tenant switch: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let user_tenant_id: Uuid = user_row.get("tenant_id");
        let allowed_ids: Option<Vec<Uuid>> = user_row.get("allowed_tenant_ids");

        let has_access = user_tenant_id == tenant_id || 
            allowed_ids.map(|ids| ids.contains(&tenant_id)).unwrap_or(false);

        if !has_access {
            tracing::warn!("User {} denied access to tenant {}", auth.user_id, tenant_id);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Get tenant info (must be active)
    let tenant = sqlx::query_as::<_, Tenant>(
        "SELECT * FROM tenants WHERE id = $1"
    )
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch tenant {}: {:?}", tenant_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let tenant = match tenant {
        Some(t) if t.status == "active" => t,
        Some(t) => {
            tracing::warn!("Attempted to switch to non-active tenant {} (status: {})", tenant_id, t.status);
            return Err(StatusCode::FORBIDDEN);
        }
        None => {
            tracing::warn!("Tenant {} not found", tenant_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Get user info in the new tenant context    
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let token = generate_token(auth.user_id, tenant_id, user.role.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Invalidate user cache so next /api/auth/me returns fresh tenant data
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let user_key = keys::user(auth.user_id);
        let _ = cache.delete(&user_key).await;
        tracing::info!("Invalidated user cache for {} after tenant switch to {}", auth.user_id, tenant_id);
    }

    let restrictions = ComplianceRestrictions::for_mode(&tenant.compliance_mode);

    Ok(Json(json!({
        "token": token,
        "tenant": {
            "id": tenant.id,
            "name": tenant.name,
            "domain": tenant.domain,
            "plan": tenant.plan,
            "compliance_mode": tenant.compliance_mode,
            "retention_policy_days": tenant.retention_policy_days,
            "mfa_required": tenant.mfa_required,
            "session_timeout_minutes": tenant.session_timeout_minutes,
            "public_sharing_enabled": tenant.public_sharing_enabled,
            "data_export_enabled": tenant.data_export_enabled.unwrap_or(true),
            "restrictions": restrictions,
        }
    })))
}

#[derive(Deserialize)]
pub struct TestSmtpInput {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub secure: bool,
}

/// Test SMTP connection
/// POST /api/tenants/:id/smtp/test
pub async fn test_smtp(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(_id): Path<Uuid>,
    Json(input): Json<TestSmtpInput>,
) -> Result<Json<Value>, StatusCode> {
    // Only Admin/Owner can test SMTP
    if auth.role != "SuperAdmin" && auth.role != "Owner" && auth.role != "Admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    clovalink_core::mailer::test_smtp_connection(
        &input.host,
        input.port,
        &input.username,
        &input.password,
        input.secure
    ).await.map_err(|e| {
        tracing::error!("SMTP Test Failed: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;

    Ok(Json(json!({"success": true})))
}

#[derive(Deserialize)]
pub struct SuspendTenantInput {
    pub reason: Option<String>,
}

/// Suspend a tenant/company
/// POST /api/tenants/:id/suspend
/// SuperAdmin only
pub async fn suspend_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<SuspendTenantInput>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    // Cannot suspend your own tenant
    if id == auth.tenant_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check tenant exists
    let tenant: Option<Tenant> = sqlx::query_as("SELECT * FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tenant = tenant.ok_or(StatusCode::NOT_FOUND)?;

    if tenant.status == "suspended" {
        return Ok(Json(json!({
            "success": true,
            "message": "Tenant is already suspended"
        })));
    }

    // Update tenant status to suspended
    sqlx::query(
        "UPDATE tenants SET status = 'suspended', updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'suspend_tenant', 'tenant', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "tenant_name": tenant.name,
        "reason": input.reason
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    // Invalidate caches
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let tenant_key = keys::tenant(id);
        let _ = cache.delete(&tenant_key).await;
        let _ = cache.delete_pattern("clovalink:user:*").await;
    }

    tracing::info!("SuperAdmin {} suspended tenant {} ({})", auth.user_id, id, tenant.name);

    Ok(Json(json!({
        "success": true,
        "message": "Tenant suspended successfully"
    })))
}

/// Unsuspend a tenant/company
/// POST /api/tenants/:id/unsuspend
/// SuperAdmin only
pub async fn unsuspend_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    // Check tenant exists
    let tenant: Option<Tenant> = sqlx::query_as("SELECT * FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tenant = tenant.ok_or(StatusCode::NOT_FOUND)?;

    if tenant.status != "suspended" {
        return Ok(Json(json!({
            "success": true,
            "message": "Tenant is not suspended"
        })));
    }

    // Update tenant status to active
    sqlx::query(
        "UPDATE tenants SET status = 'active', updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'unsuspend_tenant', 'tenant', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "tenant_name": tenant.name
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    // Invalidate caches
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let tenant_key = keys::tenant(id);
        let _ = cache.delete(&tenant_key).await;
        let _ = cache.delete_pattern("clovalink:user:*").await;
    }

    tracing::info!("SuperAdmin {} unsuspended tenant {} ({})", auth.user_id, id, tenant.name);

    Ok(Json(json!({
        "success": true,
        "message": "Tenant unsuspended successfully"
    })))
}

/// Permanently delete a tenant/company
/// DELETE /api/tenants/:id
/// SuperAdmin only - DANGER: This permanently deletes ALL data
pub async fn delete_tenant(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    // Cannot delete your own tenant
    if id == auth.tenant_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check tenant exists
    let tenant: Option<Tenant> = sqlx::query_as("SELECT * FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tenant = tenant.ok_or(StatusCode::NOT_FOUND)?;

    // Get counts for audit
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files_metadata WHERE tenant_id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete in order (foreign key constraints)
    // 1. Delete file shares
    sqlx::query("DELETE FROM file_shares WHERE file_id IN (SELECT id FROM files_metadata WHERE tenant_id = $1)")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 2. Delete file requests
    sqlx::query("DELETE FROM file_requests WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 3. Delete files metadata
    sqlx::query("DELETE FROM files_metadata WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 4. Delete notifications
    sqlx::query("DELETE FROM notifications WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 5. Delete audit logs for this tenant
    sqlx::query("DELETE FROM audit_logs WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 6. Delete password reset tokens for tenant users
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id IN (SELECT id FROM users WHERE tenant_id = $1)")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 7. Delete users
    sqlx::query("DELETE FROM users WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 8. Delete departments
    sqlx::query("DELETE FROM departments WHERE tenant_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // 9. Finally delete the tenant
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log (in SuperAdmin's tenant)
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'delete_tenant', 'tenant', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "tenant_name": tenant.name,
        "tenant_domain": tenant.domain,
        "users_deleted": user_count.0,
        "files_deleted": file_count.0
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    // Invalidate caches
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let tenant_key = keys::tenant(id);
        let _ = cache.delete(&tenant_key).await;
        let _ = cache.delete_pattern("clovalink:user:*").await;
    }

    tracing::warn!(
        "SuperAdmin {} PERMANENTLY DELETED tenant {} ({}) - {} users, {} files removed",
        auth.user_id, id, tenant.name, user_count.0, file_count.0
    );

    Ok(Json(json!({
        "success": true,
        "message": "Tenant permanently deleted",
        "deleted": {
            "tenant_name": tenant.name,
            "users": user_count.0,
            "files": file_count.0
        }
    })))
}
