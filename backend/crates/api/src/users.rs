use axum::{
    extract::{Path, Query, State, Multipart},
    http::StatusCode,
    response::Json,
    Extension,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, PasswordVerifier, SaltString},
    PasswordHash
};
use crate::password::get_argon2;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use totp_rs::{Algorithm, TOTP, Secret};
use crate::AppState;
use crate::middleware::rate_limit::{RateLimitConfig, check_rate_limit_atomic};
use clovalink_auth::{AuthUser, require_admin, require_manager};
use clovalink_core::models::{User, Tenant, CreateUserInput, UpdateUserInput, SuspendUserInput};
use clovalink_core::notification_service;
use clovalink_core::security_service;

fn validate_role_assignment(auth_role: &str, target_role: &str) -> Result<(), StatusCode> {
    match auth_role {
        "SuperAdmin" => Ok(()),
        "Admin" => {
            if target_role == "SuperAdmin" || target_role == "Admin" {
                Err(StatusCode::FORBIDDEN)
            } else {
                Ok(())
            }
        },
        "Manager" => {
            if target_role == "SuperAdmin" || target_role == "Admin" || target_role == "Manager" {
                Err(StatusCode::FORBIDDEN)
            } else {
                Ok(())
            }
        },
        _ => Err(StatusCode::FORBIDDEN),
    }
}

#[derive(Deserialize)]
pub struct UserFilters {
    pub role: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub tenant_id: Option<String>,
    pub department_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// List users with filters
/// GET /api/users
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(filters): Query<UserFilters>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions - Managers and above can list users
    require_manager(&auth)?;

    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);
    
    // For Managers, get their accessible departments
    let manager_departments: Option<Vec<Uuid>> = if auth.role == "Manager" {
        let dept_info: Option<(Option<Uuid>, Option<Vec<Uuid>>)> = sqlx::query_as(
            "SELECT department_id, allowed_department_ids FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get manager departments: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        let (primary_dept, allowed_depts) = dept_info.unwrap_or((None, None));
        let mut all_depts = Vec::new();
        if let Some(pd) = primary_dept {
            all_depts.push(pd);
        }
        if let Some(ads) = allowed_depts {
            for d in ads {
                if !all_depts.contains(&d) {
                    all_depts.push(d);
                }
            }
        }
        if all_depts.is_empty() {
            None
        } else {
            Some(all_depts)
        }
    } else {
        None
    };

    // Build query
    let mut query = String::from(
        "SELECT id, tenant_id, department_id, email, name, role, status, avatar_url, last_active_at, dashboard_layout, widget_config, allowed_tenant_ids, allowed_department_ids, suspended_at, suspended_until, suspension_reason, created_at, updated_at 
         FROM users WHERE 1=1"
    );
    
    let mut param_count = 1;
    
    // Tenant filter (SuperAdmin can see all or filter, others see own)
    // Also include users who have access to this tenant via allowed_tenant_ids
    let tenant_id_filter = if auth.role == "SuperAdmin" {
        filters.tenant_id.as_ref().and_then(|t| Uuid::parse_str(t).ok())
    } else {
        Some(auth.tenant_id)
    };

    if let Some(_) = tenant_id_filter {
        // Include users whose primary tenant matches OR who have this tenant in allowed_tenant_ids
        query.push_str(&format!(" AND (tenant_id = ${0} OR ${0} = ANY(allowed_tenant_ids))", param_count));
        param_count += 1;
    }
    
    // Department filter for Managers - they can only see users in their departments
    let dept_filter: Option<Uuid> = if auth.role == "Manager" {
        // If a specific department is requested, validate it's one they have access to
        if let Some(ref dept_str) = filters.department_id {
            let requested_dept = Uuid::parse_str(dept_str).ok();
            if let Some(rd) = requested_dept {
                if let Some(ref mgr_depts) = manager_departments {
                    if mgr_depts.contains(&rd) {
                        Some(rd)
                    } else {
                        return Err(StatusCode::FORBIDDEN);
                    }
                } else {
                    return Err(StatusCode::FORBIDDEN);
                }
            } else {
                None
            }
        } else {
            None // Will use manager_departments array filter
        }
    } else {
        // Admins can filter by any department
        filters.department_id.as_ref().and_then(|d| Uuid::parse_str(d).ok())
    };
    
    if let Some(_) = dept_filter {
        query.push_str(&format!(" AND department_id = ${}", param_count));
        param_count += 1;
    } else if let Some(ref _mgr_depts) = manager_departments {
        // Manager viewing all their departments
        query.push_str(&format!(" AND department_id = ANY(${})", param_count));
        param_count += 1;
    }
    
    // For Managers: only show Employee-level users (not other Managers, Admins, or SuperAdmins)
    if auth.role == "Manager" {
        query.push_str(" AND role = 'Employee'");
    }
    
    if filters.role.is_some() {
        query.push_str(&format!(" AND role = ${}", param_count));
        param_count += 1;
    }
    if filters.status.is_some() {
        query.push_str(&format!(" AND status = ${}", param_count));
        param_count += 1;
    }
    if filters.search.is_some() {
        query.push_str(&format!(" AND (name ILIKE ${} OR email ILIKE ${})", param_count, param_count));
        param_count += 1;
    }
    
    query.push_str(" ORDER BY created_at DESC");
    query.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));

    // Execute query
    let mut db_query = sqlx::query(&query);

    if let Some(tid) = tenant_id_filter {
        db_query = db_query.bind(tid);
    }
    
    if let Some(df) = dept_filter {
        db_query = db_query.bind(df);
    } else if let Some(ref mgr_depts) = manager_departments {
        db_query = db_query.bind(mgr_depts);
    }

    if let Some(role) = filters.role {
        db_query = db_query.bind(role);
    }
    if let Some(status) = filters.status {
        db_query = db_query.bind(status);
    }
    if let Some(search) = filters.search {
        let search_pattern = format!("%{}%", search);
        db_query = db_query.bind(search_pattern.clone());
    }

    let users = db_query
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list users: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Convert to JSON manually
    let result: Vec<Value> = users.iter().map(|row| {
        json!({
            "id": row.get::<Uuid, _>("id"),
            "email": row.get::<String, _>("email"),
            "name": row.get::<String, _>("name"),
            "role": row.get::<String, _>("role"),
            "status": row.get::<String, _>("status"),
            "avatar_url": row.get::<Option<String>, _>("avatar_url"),
            "last_active_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_active_at"),
            "department_id": row.get::<Option<Uuid>, _>("department_id"),
            "dashboard_layout": row.get::<Option<Value>, _>("dashboard_layout"),
            "widget_config": row.get::<Option<Value>, _>("widget_config"),
            "allowed_tenant_ids": row.get::<Option<Vec<Uuid>>, _>("allowed_tenant_ids"),
            "allowed_department_ids": row.get::<Option<Vec<Uuid>>, _>("allowed_department_ids"),
            "suspended_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("suspended_at"),
            "suspended_until": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("suspended_until"),
            "suspension_reason": row.get::<Option<String>, _>("suspension_reason"),
            "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        })
    }).collect();

    Ok(Json(json!(result)))
}

/// Create/invite new user
/// POST /api/users
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<CreateUserInput>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions
    // Check permissions - Managers can invite users too now
    require_manager(&auth)?;

    // Validate role assignment
    validate_role_assignment(&auth.role, &input.role)?;

    // Get tenant to check compliance mode
    let tenant_id = if auth.role == "SuperAdmin" {
        input.tenant_id.unwrap_or(auth.tenant_id)
    } else {
        auth.tenant_id
    };

    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Hash password if provided (not required for SSO-only users)
    let password_hash: Option<String> = if input.identity_provider == "oidc" || input.identity_provider == "saml" {
        // SSO-only users don't need a password
        None
    } else {
        let password = input.password.as_deref().ok_or_else(|| {
            tracing::error!("Password required for local/hybrid auth");
            StatusCode::BAD_REQUEST
        })?;

        // Validate password against tenant's password policy
        validate_password_against_policy(&state.pool, tenant_id, password)
            .await
            .map_err(|(status, _json)| status)?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = get_argon2();
        Some(argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                tracing::error!("Failed to hash password: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .to_string())
    };

    // Determine tenant_id
    let tenant_id = if auth.role == "SuperAdmin" {
        input.tenant_id.unwrap_or(auth.tenant_id)
    } else {
        auth.tenant_id
    };

    // Insert user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (tenant_id, email, name, password_hash, role, department_id, identity_provider)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#
    )
    .bind(tenant_id)
    .bind(&input.email)
    .bind(&input.name)
    .bind(&password_hash)
    .bind(&input.role)
    .bind(input.department_id)
    .bind(&input.identity_provider)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create user: {:?}", e);
        if e.to_string().contains("unique") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    // Notify all admins about the new user
    let _ = notification_service::notify_all_admins(
        &state.pool,
        &tenant,
        notification_service::NotificationType::UserCreated,
        "New user added",
        &format!("{} ({}) was added as {} to the organization.", user.name, user.email, user.role),
        Some(serde_json::json!({
            "new_user_id": user.id,
            "new_user_email": user.email,
            "new_user_name": user.name,
            "new_user_role": user.role
        })),
    ).await;

    Ok(Json(json!({
        "id": user.id,
        "email": user.email,
        "name": user.name,
        "role": user.role,
        "department_id": user.department_id,
        "status": user.status,
        "created_at": user.created_at,
    })))
}

/// Update user
/// PUT /api/users/:id
pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateUserInput>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions
    // Check permissions
    require_manager(&auth)?;

    // If role is being updated, validate it
    if let Some(new_role) = &input.role {
        validate_role_assignment(&auth.role, new_role)?;
    }

    // Fetch the user before update to track role changes
    let old_user: Option<(String, String)> = sqlx::query_as(
        "SELECT role, email FROM users WHERE id = $1 AND tenant_id = $2"
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (old_role, user_email) = old_user.ok_or(StatusCode::NOT_FOUND)?;
    let role_changing = input.role.as_ref().map(|r| r != &old_role).unwrap_or(false);

    // Require password confirmation for role changes
    if role_changing {
        let confirm_password = input.confirm_password.as_ref()
            .ok_or(StatusCode::BAD_REQUEST)?;
        
        // Fetch the admin's password hash to verify
        let admin_password_hash: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let hash = admin_password_hash.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let parsed_hash = PasswordHash::new(&hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Verify the admin's password
        get_argon2()
            .verify_password(confirm_password.as_bytes(), &parsed_hash)
            .map_err(|_| {
                tracing::warn!("Role change password verification failed for admin {}", auth.user_id);
                StatusCode::FORBIDDEN
            })?;
    }
    let new_role_value = input.role.clone();

    // Build update query dynamically
    let mut updates = Vec::new();
    let mut param_count = 3; // $1 is id, $2 is tenant_id

    if let Some(_name) = &input.name {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if let Some(_role) = &input.role {
        updates.push(format!("role = ${}", param_count));
        param_count += 1;
    }
    if let Some(_status) = &input.status {
        updates.push(format!("status = ${}", param_count));
        param_count += 1;
    }
    if let Some(_department_id) = &input.department_id {
        updates.push(format!("department_id = ${}", param_count));
        param_count += 1;
    }
    if let Some(_dashboard_layout) = &input.dashboard_layout {
        updates.push(format!("dashboard_layout = ${}", param_count));
        param_count += 1;
    }
    if let Some(_widget_config) = &input.widget_config {
        updates.push(format!("widget_config = ${}", param_count));
        param_count += 1;
    }
    if let Some(_allowed_tenant_ids) = &input.allowed_tenant_ids {
        if auth.role != "SuperAdmin" {
            return Err(StatusCode::FORBIDDEN);
        }
        updates.push(format!("allowed_tenant_ids = ${}", param_count));
        param_count += 1;
    }
    if let Some(_allowed_department_ids) = &input.allowed_department_ids {
        updates.push(format!("allowed_department_ids = ${}", param_count));
    }

    if updates.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    updates.push("updated_at = NOW()".to_string());
    let query = format!(
        "UPDATE users SET {} WHERE id = $1 AND tenant_id = $2 RETURNING id, tenant_id, department_id, email, name, role, status, avatar_url, last_active_at, dashboard_layout, widget_config, allowed_tenant_ids, allowed_department_ids, created_at, updated_at",
        updates.join(", ")
    );

    let mut db_query = sqlx::query(&query)
        .bind(id)
        .bind(auth.tenant_id);

    if let Some(name) = input.name {
        db_query = db_query.bind(name);
    }
    if let Some(role) = input.role {
        db_query = db_query.bind(role);
    }
    if let Some(status) = input.status {
        db_query = db_query.bind(status);
    }
    if let Some(department_id) = input.department_id {
        db_query = db_query.bind(department_id);
    }
    if let Some(dashboard_layout) = input.dashboard_layout {
        db_query = db_query.bind(dashboard_layout);
    }
    if let Some(widget_config) = input.widget_config {
        db_query = db_query.bind(widget_config);
    }
    if let Some(allowed_tenant_ids) = input.allowed_tenant_ids {
        db_query = db_query.bind(allowed_tenant_ids);
    }
    if let Some(allowed_department_ids) = input.allowed_department_ids {
        db_query = db_query.bind(allowed_department_ids);
    }

    let row = db_query
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Invalidate user cache for the updated user
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let cache_key = keys::user(id);
        if let Err(e) = cache.delete(&cache_key).await {
            tracing::warn!("Failed to invalidate user cache: {}", e);
        }
    }

    // Notify user if their role was changed
    if role_changing {
        if let Some(ref new_role) = new_role_value {
            // Check for permission escalation (to Admin or SuperAdmin)
            let is_escalation = matches!(new_role.as_str(), "Admin" | "SuperAdmin") 
                && !matches!(old_role.as_str(), "Admin" | "SuperAdmin");
            
            if is_escalation {
                let _ = security_service::alert_permission_escalation(
                    &state.pool,
                    auth.tenant_id,
                    id,
                    auth.user_id,
                    &user_email,
                    &old_role,
                    new_role,
                    auth.ip_address.as_deref(),
                ).await;
            }

            // Get tenant for email
            if let Ok(tenant) = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
                .bind(auth.tenant_id)
                .fetch_one(&state.pool)
                .await
            {
                let _ = notification_service::notify_role_changed(
                    &state.pool,
                    &tenant,
                    id,
                    &user_email,
                    &old_role,
                    new_role,
                ).await;
            }
        }
    }

    Ok(Json(json!({
        "id": row.get::<Uuid, _>("id"),
        "email": row.get::<String, _>("email"),
        "name": row.get::<String, _>("name"),
        "role": row.get::<String, _>("role"),
        "department_id": row.get::<Option<Uuid>, _>("department_id"),
        "dashboard_layout": row.get::<Option<Value>, _>("dashboard_layout"),
        "widget_config": row.get::<Option<Value>, _>("widget_config"),
        "allowed_tenant_ids": row.get::<Option<Vec<Uuid>>, _>("allowed_tenant_ids"),
        "allowed_department_ids": row.get::<Option<Vec<Uuid>>, _>("allowed_department_ids"),
        "status": row.get::<String, _>("status"),
        "updated_at": row.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
    })))
}


/// Deactivate user (soft delete)
/// DELETE /api/users/:id
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions - must be at least Admin
    require_admin(&auth)?;

    // Cannot delete yourself
    if auth.user_id == id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get the target user to check their role
    let target_user = sqlx::query("SELECT role, tenant_id FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");

    // Validate role hierarchy - can only delete users with lower roles
    validate_role_assignment(&auth.role, &target_role)?;

    // Non-SuperAdmins can only delete users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Soft delete by setting status to inactive
    sqlx::query(
        "UPDATE users SET status = 'inactive', updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"success": true, "message": "User deactivated"})))
}

/// Permanently delete user (hard delete)
/// DELETE /api/users/:id/permanent
pub async fn permanent_delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions - must be at least Admin
    require_admin(&auth)?;

    // Cannot delete yourself
    if auth.user_id == id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get the target user to check their role
    let target_user = sqlx::query("SELECT role, tenant_id, email, name FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");
    let target_email: String = target_user.get("email");
    let target_name: String = target_user.get("name");

    // Validate role hierarchy - can only delete users with lower roles
    validate_role_assignment(&auth.role, &target_role)?;

    // Non-SuperAdmins can only delete users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Delete user sessions first
    sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok(); // Ignore errors if table doesn't exist

    // Delete user preferences
    sqlx::query("DELETE FROM user_preferences WHERE user_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // Update audit logs to remove user_id reference (keep logs for compliance)
    sqlx::query("UPDATE audit_logs SET user_id = NULL, metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{deleted_user}', $1::jsonb) WHERE user_id = $2")
        .bind(json!({"email": target_email, "name": target_name}))
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // Update file requests created_by to NULL
    sqlx::query("UPDATE file_requests SET created_by = NULL WHERE created_by = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // Update files owner_id to NULL
    sqlx::query("UPDATE files SET owner_id = NULL WHERE owner_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // Finally, delete the user
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete user: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Log audit event for permanent deletion
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (id, tenant_id, user_id, action, resource_type, metadata, ip_address)
        VALUES ($1, $2, $3, 'user_permanently_deleted', 'user', $4, $5::inet)
        "#
    )
    .bind(Uuid::new_v4())
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(json!({"deleted_user_email": target_email, "deleted_user_name": target_name}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({"success": true, "message": "User permanently deleted"})))
}

/// Export user data (GDPR)
/// GET /api/users/me/export
pub async fn export_data(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    // Rate limit: 5 exports per hour per user
    if let Some(ref cache) = state.cache {
        let rate_key = format!("ratelimit:export:{}", auth.user_id);
        let config = RateLimitConfig::export();
        
        match check_rate_limit_atomic(cache, &rate_key, &config).await {
            Ok((allowed, count, _)) => {
                if !allowed {
                    tracing::warn!("Export rate limit exceeded for user: {} (count: {})", auth.user_id, count);
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(e) => {
                tracing::error!("Export rate limit check failed: {}", e);
                // Allow request on error (fail open for availability)
            }
        }
    }

    // Get user profile
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get tenant info
    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(auth.tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if data export is enabled for this tenant
    if !tenant.data_export_enabled.unwrap_or(true) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get recent activity
    let activities = sqlx::query(
        "SELECT action, resource_type, created_at FROM audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50"
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let activity_list: Vec<Value> = activities.iter().map(|row| {
        json!({
            "action": row.get::<String, _>("action"),
            "resource_type": row.get::<String, _>("resource_type"),
            "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
        })
    }).collect();

    Ok(Json(json!({
        "profile": {
            "name": user.name,
            "email": user.email,
            "role": user.role,
            "created_at": user.created_at,
        },
        "tenant": {
            "name": tenant.name,
            "domain": tenant.domain,
        },
        "recent_activity": activity_list
    })))
}

/// Validate password against tenant's password policy
pub async fn validate_password_against_policy(
    pool: &sqlx::PgPool,
    tenant_id: uuid::Uuid,
    password: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    use crate::settings::PasswordPolicy;
    
    // Fetch tenant's password policy
    let policy_result: Option<(Value,)> = sqlx::query_as(
        "SELECT password_policy FROM tenants WHERE id = $1"
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    let policy: PasswordPolicy = match policy_result {
        Some((json_value,)) => serde_json::from_value(json_value).unwrap_or_default(),
        None => PasswordPolicy::default(),
    };

    // Validate against policy
    match crate::settings::validate_password(&password, &policy) {
        Ok(()) => Ok(()),
        Err(errors) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Password does not meet requirements",
                "requirements": errors
            }))
        ))
    }
}

// ==================== Profile Management Endpoints ====================

#[derive(Debug, Deserialize)]
pub struct UpdateProfileInput {
    pub name: Option<String>,
    pub email: Option<String>,
    pub totp_code: Option<String>,  // Required if changing email and 2FA is enabled
}

#[derive(Deserialize)]
pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
    pub totp_code: Option<String>,  // Required if user has 2FA enabled
}

/// Update current user's profile (name, email)
/// PUT /api/users/me/profile
pub async fn update_my_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<UpdateProfileInput>,
) -> Result<Json<Value>, StatusCode> {
    tracing::debug!("update_my_profile called for user {} with input: {:?}", auth.user_id, input);
    
    // If email is being changed, verify 2FA if enabled
    if input.email.is_some() {
        // Check if user has 2FA enabled
        let totp_secret: Option<String> = sqlx::query_scalar(
            "SELECT totp_secret FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if let Some(secret_str) = totp_secret {
            // 2FA is enabled, require verification
            let code = input.totp_code.as_ref().ok_or_else(|| {
                tracing::warn!("Email change attempted without 2FA code for user {}", auth.user_id);
                StatusCode::FORBIDDEN
            })?;
            
            // Verify the TOTP code
            let secret = Secret::Encoded(secret_str);
            let totp = TOTP::new(
                Algorithm::SHA1,
                6,
                1,
                30,
                secret.to_bytes().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                None,
                "".to_string()
            ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if !totp.check_current(code).unwrap_or(false) {
                tracing::warn!("Invalid 2FA code for email change by user {}", auth.user_id);
                return Err(StatusCode::UNAUTHORIZED);
            }
            
            tracing::info!("2FA verified for email change by user {}", auth.user_id);
        }
    }
    
    let mut updates = Vec::new();
    let mut param_count = 2; // $1 is user_id

    if let Some(_) = &input.name {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if let Some(_) = &input.email {
        updates.push(format!("email = ${}", param_count));
    }

    // If nothing to update, return success (no error)
    if updates.is_empty() {
        tracing::debug!("No profile changes to update for user {}", auth.user_id);
        return Ok(Json(json!({
            "message": "No changes to update"
        })));
    }

    updates.push("updated_at = NOW()".to_string());
    let query = format!(
        "UPDATE users SET {} WHERE id = $1 RETURNING id, email, name, role, avatar_url",
        updates.join(", ")
    );

    let mut db_query = sqlx::query(&query).bind(auth.user_id);

    if let Some(name) = input.name {
        db_query = db_query.bind(name);
    }
    if let Some(email) = input.email {
        db_query = db_query.bind(email);
    }

    let row = db_query
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update profile: {:?}", e);
            if e.to_string().contains("unique") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;

    // Invalidate user cache
    if let Some(ref cache) = state.cache {
        use clovalink_core::cache::keys;
        let cache_key = keys::user(auth.user_id);
        if let Err(e) = cache.delete(&cache_key).await {
            tracing::warn!("Failed to invalidate user cache: {}", e);
        }
    }

    Ok(Json(json!({
        "id": row.get::<Uuid, _>("id"),
        "email": row.get::<String, _>("email"),
        "name": row.get::<String, _>("name"),
        "role": row.get::<String, _>("role"),
        "avatar_url": row.get::<Option<String>, _>("avatar_url"),
    })))
}

/// Change current user's password
/// PUT /api/users/me/password
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<ChangePasswordInput>,
) -> Result<Json<Value>, StatusCode> {
    // Get current user with password hash and 2FA secret
    let user = sqlx::query("SELECT password_hash, totp_secret FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let current_hash: Option<String> = user.get("password_hash");
    let current_hash = current_hash.ok_or(StatusCode::BAD_REQUEST)?; // OIDC-only users can't change password
    let totp_secret: Option<String> = user.get("totp_secret");

    // Check if user has 2FA enabled - require TOTP code
    if let Some(secret_str) = totp_secret {
        match input.totp_code {
            Some(code) => {
                // Verify the TOTP code
                let secret = Secret::Encoded(secret_str);
                let totp = TOTP::new(
                    Algorithm::SHA1, 
                    6, 
                    1, 
                    30, 
                    secret.to_bytes().unwrap(), 
                    None, 
                    "".to_string()
                ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                
                if !totp.check_current(&code).unwrap_or(false) {
                    return Ok(Json(json!({
                        "error": "invalid_2fa_code",
                        "message": "Invalid 2FA code"
                    })));
                }
            }
            None => {
                // 2FA is enabled but no code provided
                return Ok(Json(json!({
                    "error": "2fa_required",
                    "message": "2FA verification is required to change your password",
                    "require_2fa": true
                })));
            }
        }
    }

    // Verify current password
    let argon2 = get_argon2();
    let parsed_hash = PasswordHash::new(&current_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if argon2.verify_password(input.current_password.as_bytes(), &parsed_hash).is_err() {
        return Err(StatusCode::UNAUTHORIZED); // Current password is wrong
    }

    // Validate new password against tenant's password policy
    validate_password_against_policy(&state.pool, auth.tenant_id, &input.new_password)
        .await
        .map_err(|(status, _json)| status)?;

    // Hash new password
    let salt = SaltString::generate(&mut OsRng);
    let new_hash = argon2.hash_password(input.new_password.as_bytes(), &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_string();

    // Update password
    sqlx::query("UPDATE users SET password_hash = $1, password_changed_at = NOW(), updated_at = NOW() WHERE id = $2")
        .bind(&new_hash)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "success": true, "message": "Password changed successfully" })))
}

/// Upload avatar for current user
/// POST /api/users/me/avatar
pub async fn upload_avatar(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    tracing::debug!("upload_avatar called for user {}", auth.user_id);
    
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Failed to get multipart field: {:?}", e);
        StatusCode::BAD_REQUEST
    })? {
        let name = field.name().unwrap_or("").to_string();
        tracing::debug!("Received multipart field: name={}", name);
        
        if name == "avatar" || name == "file" {
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
            tracing::debug!("Avatar field content_type: {}", content_type);
            
            // Validate it's an image
            if !content_type.starts_with("image/") {
                tracing::warn!("Invalid avatar content type: {}", content_type);
                return Err(StatusCode::BAD_REQUEST);
            }

            let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            
            // Limit size to 5MB
            if data.len() > 5 * 1024 * 1024 {
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }

            // Generate filename
            let extension = match content_type.as_str() {
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "jpg",
            };
            let filename = format!("avatars/{}.{}", auth.user_id, extension);

            // Upload to storage
            state.storage.upload(&filename, data.to_vec()).await
                .map_err(|e| {
                    tracing::error!("Failed to upload avatar: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Get the public URL (for local storage, construct it; for S3, get presigned)
            let avatar_url = format!("/uploads/{}", filename);

            // Update user
            sqlx::query("UPDATE users SET avatar_url = $1, updated_at = NOW() WHERE id = $2")
                .bind(&avatar_url)
                .bind(auth.user_id)
                .execute(&state.pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            return Ok(Json(json!({
                "success": true,
                "avatar_url": avatar_url
            })));
        } else {
            tracing::debug!("Skipping multipart field with name: {}", name);
        }
    }

    tracing::warn!("No avatar field found in multipart request for user {}", auth.user_id);
    Err(StatusCode::BAD_REQUEST)
}

/// List active sessions for current user
/// GET /api/users/me/sessions
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let sessions = sqlx::query(
        r#"
        SELECT id, device_info, ip_address::text as ip_address, last_active_at, created_at
        FROM user_sessions
        WHERE user_id = $1 AND is_revoked = false AND expires_at > NOW()
        ORDER BY last_active_at DESC
        "#
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let result: Vec<Value> = sessions.iter().map(|row| {
        json!({
            "id": row.get::<Uuid, _>("id"),
            "device_info": row.get::<Option<String>, _>("device_info"),
            "ip_address": row.get::<Option<String>, _>("ip_address"),
            "last_active_at": row.get::<DateTime<Utc>, _>("last_active_at"),
            "created_at": row.get::<DateTime<Utc>, _>("created_at"),
        })
    }).collect();

    Ok(Json(json!({ "sessions": result })))
}

/// Revoke a specific session
/// DELETE /api/users/me/sessions/:id
pub async fn revoke_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query(
        "UPDATE user_sessions SET is_revoked = true WHERE id = $1 AND user_id = $2"
    )
    .bind(session_id)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== User Preferences Endpoints ====================

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesInput {
    /// Settings to merge into existing preferences (e.g., {"keyboard_shortcut_preset": "vim"})
    pub settings: Value,
}

/// Get current user's preferences
/// GET /api/users/me/preferences
pub async fn get_preferences(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    // Try to get existing preferences
    let prefs: Option<(Value,)> = sqlx::query_as(
        "SELECT settings FROM user_preferences WHERE user_id = $1"
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user preferences: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match prefs {
        Some((settings,)) => Ok(Json(settings)),
        None => {
            // Return empty settings if no preferences exist
            Ok(Json(json!({})))
        }
    }
}

/// Update current user's preferences (merges with existing)
/// PUT /api/users/me/preferences
pub async fn update_preferences(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<UpdatePreferencesInput>,
) -> Result<Json<Value>, StatusCode> {
    // Upsert preferences - merge new settings with existing
    let result: (Value,) = sqlx::query_as(
        r#"
        INSERT INTO user_preferences (user_id, settings, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (user_id) 
        DO UPDATE SET 
            settings = user_preferences.settings || $2,
            updated_at = NOW()
        RETURNING settings
        "#
    )
    .bind(auth.user_id)
    .bind(&input.settings)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update user preferences: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(result.0))
}

// ==================== User Suspension Endpoints ====================

/// Suspend a user
/// POST /api/users/:id/suspend
pub async fn suspend_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<SuspendUserInput>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions - at least manager required
    require_manager(&auth)?;

    // Cannot suspend yourself
    if auth.user_id == id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get the target user to check their role
    let target_user = sqlx::query("SELECT role, tenant_id FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");

    // Validate role hierarchy - can only suspend users with lower roles
    validate_role_assignment(&auth.role, &target_role)?;

    // Non-SuperAdmins can only suspend users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Suspend the user
    sqlx::query(
        r#"
        UPDATE users 
        SET suspended_at = NOW(), 
            suspended_until = $1, 
            suspension_reason = $2,
            updated_at = NOW() 
        WHERE id = $3
        "#
    )
    .bind(input.until)
    .bind(&input.reason)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Invalidate all active sessions for this user
    sqlx::query("UPDATE user_sessions SET is_revoked = true WHERE user_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok(); // Ignore errors - sessions table might not exist

    Ok(Json(json!({
        "success": true,
        "message": "User suspended successfully",
        "suspended_until": input.until
    })))
}

/// Unsuspend a user
/// POST /api/users/:id/unsuspend
pub async fn unsuspend_user(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions - at least manager required
    require_manager(&auth)?;

    // Get the target user to check their role
    let target_user = sqlx::query("SELECT role, tenant_id, suspended_at FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");
    let suspended_at: Option<DateTime<Utc>> = target_user.get("suspended_at");

    // Check if user is actually suspended
    if suspended_at.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate role hierarchy
    validate_role_assignment(&auth.role, &target_role)?;

    // Non-SuperAdmins can only unsuspend users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Unsuspend the user
    sqlx::query(
        r#"
        UPDATE users 
        SET suspended_at = NULL, 
            suspended_until = NULL, 
            suspension_reason = NULL,
            updated_at = NOW() 
        WHERE id = $1
        "#
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "success": true,
        "message": "User unsuspended successfully"
    })))
}

/// Get user suspension status
/// GET /api/users/:id/suspension
pub async fn get_suspension_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Check permissions
    require_manager(&auth)?;

    let user = sqlx::query(
        "SELECT suspended_at, suspended_until, suspension_reason, tenant_id FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let target_tenant_id: Uuid = user.get("tenant_id");

    // Non-SuperAdmins can only view users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let suspended_at: Option<DateTime<Utc>> = user.get("suspended_at");
    let suspended_until: Option<DateTime<Utc>> = user.get("suspended_until");
    let suspension_reason: Option<String> = user.get("suspension_reason");

    Ok(Json(json!({
        "is_suspended": suspended_at.is_some(),
        "suspended_at": suspended_at,
        "suspended_until": suspended_until,
        "suspension_reason": suspension_reason
    })))
}

/// Check if admin_role can reset password for target_role
fn can_reset_password(admin_role: &str, target_role: &str) -> bool {
    match admin_role {
        "SuperAdmin" => true, // SuperAdmin can reset anyone
        "Admin" => matches!(target_role, "Manager" | "Employee"),
        "Manager" => target_role == "Employee",
        _ => false
    }
}

#[derive(Deserialize)]
pub struct AdminResetPasswordInput {
    pub new_password: String,
}

/// Admin reset password - set password directly
/// POST /api/users/:id/reset-password
/// Role hierarchy: SuperAdmin > Admin > Manager > Employee
pub async fn admin_reset_password(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminResetPasswordInput>,
) -> Result<Json<Value>, StatusCode> {
    // Must be at least a Manager
    require_manager(&auth)?;

    // Cannot reset your own password through this endpoint
    if id == auth.user_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get target user
    let target_user = sqlx::query(
        "SELECT id, email, name, role, tenant_id FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");
    let target_email: String = target_user.get("email");
    let target_name: String = target_user.get("name");

    // Non-SuperAdmins can only reset passwords for users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check role hierarchy
    if !can_reset_password(&auth.role, &target_role) {
        tracing::warn!(
            "User {} ({}) attempted to reset password for user {} ({})",
            auth.user_id, auth.role, id, target_role
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate password against tenant's password policy
    validate_password_against_policy(&state.pool, target_tenant_id, &input.new_password)
        .await
        .map_err(|(status, _json)| status)?;

    // Hash the new password with tuned parameters
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = get_argon2();
    let password_hash = argon2
        .hash_password(input.new_password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Failed to hash password: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .to_string();

    // Update the password
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&password_hash)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'admin_reset_password', 'user', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "target_user_email": target_email,
        "target_user_name": target_name,
        "target_user_role": target_role
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    tracing::info!(
        "Admin {} ({}) reset password for user {} ({})",
        auth.user_id, auth.role, id, target_role
    );

    Ok(Json(json!({
        "success": true,
        "message": "Password reset successfully"
    })))
}

/// Send password reset email to user
/// POST /api/users/:id/send-reset-email
/// Role hierarchy: SuperAdmin > Admin > Manager > Employee
pub async fn send_password_reset_email(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Must be at least a Manager
    require_manager(&auth)?;

    // Get target user
    let target_user = sqlx::query(
        "SELECT id, email, name, role, tenant_id FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");
    let target_email: String = target_user.get("email");
    let target_name: String = target_user.get("name");

    // Non-SuperAdmins can only send reset emails for users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check role hierarchy
    if !can_reset_password(&auth.role, &target_role) {
        tracing::warn!(
            "User {} ({}) attempted to send reset email to user {} ({})",
            auth.user_id, auth.role, id, target_role
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Generate a secure random token
    let token = Uuid::new_v4().to_string();
    
    // Hash the token for storage (we'll send the plain token in the email)
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = get_argon2();
    let token_hash = argon2
        .hash_password(token.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Failed to hash reset token: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .to_string();

    // Token expires in 24 hours
    let expires_at = Utc::now() + chrono::Duration::hours(24);

    // Invalidate any existing tokens for this user
    sqlx::query("UPDATE password_reset_tokens SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    // Store the new token
    sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at, created_by) 
         VALUES ($1, $2, $3, $4)"
    )
    .bind(id)
    .bind(&token_hash)
    .bind(expires_at)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get tenant for SMTP config
    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(target_tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build reset URL (frontend will handle this route)
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| format!("https://{}", tenant.domain));
    let reset_url = format!("{}/reset-password?token={}", frontend_url, token);

    // Build variables for email template
    let mut variables = std::collections::HashMap::new();
    variables.insert("user_name".to_string(), target_name.clone());
    variables.insert("reset_link".to_string(), reset_url);
    variables.insert("company_name".to_string(), tenant.name.clone());

    // Send templated email
    let email_sent = clovalink_core::notification_service::send_templated_email(
        &state.pool,
        &tenant,
        &target_email,
        "password_reset",
        variables,
    ).await.is_ok();

    // Audit log
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'send_password_reset_email', 'user', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "target_user_email": target_email,
        "target_user_name": target_name,
        "email_sent": email_sent
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    tracing::info!(
        "Admin {} ({}) sent password reset email to user {} (email_sent: {})",
        auth.user_id, auth.role, id, email_sent
    );

    Ok(Json(json!({
        "success": true,
        "message": if email_sent { 
            "Password reset email sent successfully" 
        } else { 
            "Password reset token created but email could not be sent. Please check SMTP configuration." 
        },
        "email_sent": email_sent
    })))
}

#[derive(Deserialize)]
pub struct ChangeEmailInput {
    pub email: String,
}

/// Admin change user email
/// POST /api/users/:id/change-email
/// Role hierarchy: SuperAdmin > Admin > Manager > Employee
pub async fn admin_change_email(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<ChangeEmailInput>,
) -> Result<Json<Value>, StatusCode> {
    // Must be at least a Manager
    require_manager(&auth)?;

    // Get target user
    let target_user = sqlx::query(
        "SELECT id, email, name, role, tenant_id FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let target_role: String = target_user.get("role");
    let target_tenant_id: Uuid = target_user.get("tenant_id");
    let old_email: String = target_user.get("email");
    let target_name: String = target_user.get("name");

    // Non-SuperAdmins can only change emails for users in their own tenant
    if auth.role != "SuperAdmin" && target_tenant_id != auth.tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check role hierarchy
    if !can_reset_password(&auth.role, &target_role) {
        tracing::warn!(
            "User {} ({}) attempted to change email for user {} ({})",
            auth.user_id, auth.role, id, target_role
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate email format
    if !input.email.contains('@') || input.email.len() < 5 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if email is already in use
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = $1 AND id != $2"
    )
    .bind(&input.email)
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    // Update the email
    sqlx::query("UPDATE users SET email = $1, updated_at = NOW() WHERE id = $2")
        .bind(&input.email)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    sqlx::query(
        "INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address) 
         VALUES ($1, $2, 'admin_change_email', 'user', $3, $4, $5::inet)"
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(id)
    .bind(json!({
        "target_user_name": target_name,
        "old_email": old_email,
        "new_email": input.email,
        "target_user_role": target_role
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await
    .ok();

    tracing::info!(
        "Admin {} ({}) changed email for user {} from {} to {}",
        auth.user_id, auth.role, id, old_email, input.email
    );

    Ok(Json(json!({
        "success": true,
        "message": "Email updated successfully"
    })))
}
