use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use clovalink_auth::{AuthUser, require_admin};
use clovalink_core::models::Tenant;
use clovalink_core::notification_service;
use std::sync::Arc;
use crate::AppState;

// ==================== Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApprovalPolicy {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub scope: String,
    pub scope_value: Option<String>,
    pub required_approvals: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub file_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub requested_by: Uuid,
    pub status: String,
    pub step: i32,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub department_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyInput {
    pub name: String,
    pub scope: String,
    pub scope_value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyInput {
    pub name: Option<String>,
    pub scope: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub scope_value: Option<Option<String>>,  // None = not sent, Some(None) = explicitly null, Some(Some(v)) = value
    pub is_active: Option<bool>,
}

fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
pub struct RejectInput {
    pub reason: String,
}

// ==================== Policy Matching ====================

/// Context for matching approval policies against an uploaded file
pub struct FileUploadContext {
    pub department_id: Option<Uuid>,
    pub is_company_folder: bool,
    pub file_name: String,
    pub file_size: i64,
    pub visibility: String,
    pub uploader_role: String,
}

/// Find a matching active approval policy for a file upload.
/// Priority: specific scopes (department, file_type, file_size, role, private, company_folder) > all
pub async fn find_matching_policy(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    ctx: &FileUploadContext,
) -> Option<ApprovalPolicy> {
    // Fetch all active policies for this tenant in one query
    let policies: Vec<ApprovalPolicy> = sqlx::query_as(
        "SELECT * FROM approval_policies WHERE tenant_id = $1 AND is_active = true ORDER BY scope ASC"
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Check specific scopes first, then catch-all
    for policy in &policies {
        match policy.scope.as_str() {
            "department" => {
                if let (Some(dept_id), Some(ref sv)) = (ctx.department_id, &policy.scope_value) {
                    if dept_id.to_string() == *sv {
                        return Some(policy.clone());
                    }
                }
            }
            "company_folder" => {
                if ctx.is_company_folder {
                    return Some(policy.clone());
                }
            }
            "file_type" => {
                if let Some(ref sv) = policy.scope_value {
                    let file_ext = ctx.file_name.rsplit('.').next().unwrap_or("").to_lowercase();
                    if sv.split(',').any(|ext| ext.trim().eq_ignore_ascii_case(&file_ext)) {
                        return Some(policy.clone());
                    }
                }
            }
            "file_size" => {
                if let Some(ref sv) = policy.scope_value {
                    if let Ok(threshold) = sv.parse::<i64>() {
                        if ctx.file_size >= threshold {
                            return Some(policy.clone());
                        }
                    }
                }
            }
            "role" => {
                if let Some(ref sv) = policy.scope_value {
                    if ctx.uploader_role == *sv {
                        return Some(policy.clone());
                    }
                }
            }
            "private_files" => {
                if ctx.visibility == "private" {
                    return Some(policy.clone());
                }
            }
            _ => {} // Skip unknown scopes, check "all" last
        }
    }

    // Check catch-all last
    for policy in &policies {
        if policy.scope == "all" {
            return Some(policy.clone());
        }
    }

    None
}

/// Legacy wrapper for simpler callers (resubmit)
pub async fn find_matching_policy_simple(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    _department_id: Option<Uuid>,
    _is_company_folder: bool,
) -> Option<ApprovalPolicy> {
    // For resubmit, we just need any active policy — use a basic "all" check
    if let Ok(Some(policy)) = sqlx::query_as::<_, ApprovalPolicy>(
        "SELECT * FROM approval_policies WHERE tenant_id = $1 AND is_active = true LIMIT 1"
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    {
        return Some(policy);
    }

    None
}

// ==================== Approval Handlers ====================

/// GET /api/approvals/:company_id/pending
/// List files pending approval (requires approvals.view)
pub async fn list_pending(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    // Manager+ have approvals.view by default
    if !matches!(auth.role.as_str(), "Manager" | "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.page.unwrap_or(0) * limit;

    let rows: Vec<(Uuid, Uuid, Uuid, Option<Uuid>, Uuid, String, Option<Uuid>, Option<DateTime<Utc>>, Option<String>, DateTime<Utc>, String, i64, Option<String>, Option<Uuid>, String, Option<String>)> = if let Some(dept_id) = query.department_id {
        sqlx::query_as(
            r#"SELECT ar.id, ar.file_id, ar.tenant_id, ar.policy_id, ar.requested_by, ar.status,
                      ar.decided_by, ar.decided_at, ar.rejection_reason, ar.created_at,
                      fm.name as file_name, fm.size_bytes, fm.content_type, fm.department_id,
                      u.email as uploader_email, u.name as uploader_name
               FROM approval_requests ar
               JOIN files_metadata fm ON fm.id = ar.file_id
               JOIN users u ON u.id = ar.requested_by
               WHERE ar.tenant_id = $1 AND ar.status = 'pending' AND fm.department_id = $4
               ORDER BY ar.created_at DESC
               LIMIT $2 OFFSET $3"#
        )
        .bind(company_id).bind(limit).bind(offset).bind(dept_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| { tracing::error!("Failed to list pending approvals: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?
    } else {
        sqlx::query_as(
            r#"SELECT ar.id, ar.file_id, ar.tenant_id, ar.policy_id, ar.requested_by, ar.status,
                      ar.decided_by, ar.decided_at, ar.rejection_reason, ar.created_at,
                      fm.name as file_name, fm.size_bytes, fm.content_type, fm.department_id,
                      u.email as uploader_email, u.name as uploader_name
               FROM approval_requests ar
               JOIN files_metadata fm ON fm.id = ar.file_id
               JOIN users u ON u.id = ar.requested_by
               WHERE ar.tenant_id = $1 AND ar.status = 'pending'
               ORDER BY ar.created_at DESC
               LIMIT $2 OFFSET $3"#
        )
        .bind(company_id).bind(limit).bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| { tracing::error!("Failed to list pending approvals: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?
    };

    let approvals: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.0,
            "file_id": r.1,
            "tenant_id": r.2,
            "policy_id": r.3,
            "requested_by": r.4,
            "status": r.5,
            "decided_by": r.6,
            "decided_at": r.7,
            "rejection_reason": r.8,
            "created_at": r.9,
            "file_name": r.10,
            "file_size": r.11,
            "content_type": r.12,
            "department_id": r.13,
            "uploader_email": r.14,
            "uploader_name": r.15,
        })
    }).collect();

    Ok(Json(json!({ "approvals": approvals })))
}

/// GET /api/approvals/:company_id/history
/// List completed approval requests (requires approvals.view)
pub async fn list_history(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    if !matches!(auth.role.as_str(), "Manager" | "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.page.unwrap_or(0) * limit;

    let rows: Vec<(Uuid, Uuid, Uuid, Uuid, String, Option<Uuid>, Option<DateTime<Utc>>, Option<String>, DateTime<Utc>, String, i64, Option<String>, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT ar.id, ar.file_id, ar.tenant_id, ar.requested_by, ar.status,
                  ar.decided_by, ar.decided_at, ar.rejection_reason, ar.created_at,
                  fm.name as file_name, fm.size_bytes, fm.content_type,
                  uploader.email as uploader_email, uploader.name as uploader_name,
                  decider.email as decider_email
           FROM approval_requests ar
           JOIN files_metadata fm ON fm.id = ar.file_id
           JOIN users uploader ON uploader.id = ar.requested_by
           LEFT JOIN users decider ON decider.id = ar.decided_by
           WHERE ar.tenant_id = $1 AND ar.status != 'pending'
           ORDER BY ar.decided_at DESC NULLS LAST
           LIMIT $2 OFFSET $3"#
    )
    .bind(company_id).bind(limit).bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let history: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.0,
            "file_id": r.1,
            "tenant_id": r.2,
            "requested_by": r.3,
            "status": r.4,
            "decided_by": r.5,
            "decided_at": r.6,
            "rejection_reason": r.7,
            "created_at": r.8,
            "file_name": r.9,
            "file_size": r.10,
            "content_type": r.11,
            "uploader_email": r.12,
            "uploader_name": r.13,
            "decider_email": r.14,
        })
    }).collect();

    Ok(Json(json!({ "history": history })))
}

/// GET /api/approvals/:company_id/my-pending
/// List current user's files that are pending approval
pub async fn list_my_pending(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let rows: Vec<(Uuid, Uuid, String, DateTime<Utc>, String, i64, String, Option<String>)> = sqlx::query_as(
        r#"SELECT ar.id, ar.file_id, ar.status, ar.created_at,
                  fm.name as file_name, fm.size_bytes, fm.content_type, ar.rejection_reason
           FROM approval_requests ar
           JOIN files_metadata fm ON fm.id = ar.file_id
           WHERE ar.tenant_id = $1 AND ar.requested_by = $2
           AND ar.status IN ('pending', 'rejected')
           ORDER BY ar.created_at DESC"#
    )
    .bind(company_id).bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let items: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.0,
            "file_id": r.1,
            "status": r.2,
            "created_at": r.3,
            "file_name": r.4,
            "file_size": r.5,
            "content_type": r.6,
            "rejection_reason": r.7,
        })
    }).collect();

    Ok(Json(json!({ "items": items })))
}

/// GET /api/approvals/:company_id/stats
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    if !matches!(auth.role.as_str(), "Manager" | "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    let pending_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM approval_requests WHERE tenant_id = $1 AND status = 'pending'"
    )
    .bind(company_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let approved_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM approval_requests WHERE tenant_id = $1 AND status = 'approved'"
    )
    .bind(company_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rejected_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM approval_requests WHERE tenant_id = $1 AND status = 'rejected'"
    )
    .bind(company_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "pending": pending_count.0,
        "approved": approved_count.0,
        "rejected": rejected_count.0,
    })))
}

/// POST /api/approvals/:company_id/:request_id/approve
pub async fn approve_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, request_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    if !matches!(auth.role.as_str(), "Manager" | "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    // Atomic update — only succeeds if status is currently 'pending'
    let result: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"UPDATE approval_requests
           SET status = 'approved', decided_by = $1, decided_at = NOW(), updated_at = NOW()
           WHERE id = $2 AND tenant_id = $3 AND status = 'pending'
           RETURNING file_id, requested_by"#
    )
    .bind(auth.user_id)
    .bind(request_id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (file_id, requested_by) = result.ok_or(StatusCode::NOT_FOUND)?;

    // Update file status
    sqlx::query("UPDATE files_metadata SET approval_status = 'approved' WHERE id = $1")
        .bind(file_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
           VALUES ($1, $2, 'file_approved', 'file', $3, $4, $5::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(file_id)
    .bind(json!({"approval_request_id": request_id}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Notify uploader
    if let Ok(Some(tenant)) = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(company_id)
        .fetch_optional(&state.pool)
        .await
    {
        if let Ok(Some((file_name, email, role))) = sqlx::query_as::<_, (String, String, String)>(
            "SELECT fm.name, u.email, u.role FROM files_metadata fm JOIN users u ON u.id = $2 WHERE fm.id = $1"
        ).bind(file_id).bind(requested_by).fetch_optional(&state.pool).await {
            let _ = notification_service::create_notification(
                &state.pool,
                &tenant,
                requested_by,
                &role,
                notification_service::NotificationType::ApprovalDecision,
                "File Approved",
                &format!("Your file \"{}\" has been approved and is now accessible.", file_name),
                Some(json!({"file_id": file_id, "status": "approved"})),
                Some(&email),
            ).await;
        }
    }

    Ok(Json(json!({ "status": "approved", "file_id": file_id })))
}

/// POST /api/approvals/:company_id/:request_id/reject
pub async fn reject_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, request_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RejectInput>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    if !matches!(auth.role.as_str(), "Manager" | "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    if input.reason.trim().is_empty() || input.reason.len() > 2000 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Atomic update
    let result: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"UPDATE approval_requests
           SET status = 'rejected', decided_by = $1, decided_at = NOW(),
               rejection_reason = $4, updated_at = NOW()
           WHERE id = $2 AND tenant_id = $3 AND status = 'pending'
           RETURNING file_id, requested_by"#
    )
    .bind(auth.user_id)
    .bind(request_id)
    .bind(company_id)
    .bind(&input.reason)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (file_id, requested_by) = result.ok_or(StatusCode::NOT_FOUND)?;

    // Update file status
    sqlx::query("UPDATE files_metadata SET approval_status = 'rejected' WHERE id = $1")
        .bind(file_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
           VALUES ($1, $2, 'file_rejected', 'file', $3, $4, $5::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(file_id)
    .bind(json!({"approval_request_id": request_id, "reason": &input.reason}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Notify uploader
    if let Ok(Some(tenant)) = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(company_id)
        .fetch_optional(&state.pool)
        .await
    {
        if let Ok(Some((file_name, email, role))) = sqlx::query_as::<_, (String, String, String)>(
            "SELECT fm.name, u.email, u.role FROM files_metadata fm JOIN users u ON u.id = $2 WHERE fm.id = $1"
        ).bind(file_id).bind(requested_by).fetch_optional(&state.pool).await {
            let _ = notification_service::create_notification(
                &state.pool,
                &tenant,
                requested_by,
                &role,
                notification_service::NotificationType::ApprovalDecision,
                "File Rejected",
                &format!("Your file \"{}\" was rejected. Reason: {}", file_name, input.reason),
                Some(json!({"file_id": file_id, "status": "rejected", "reason": &input.reason})),
                Some(&email),
            ).await;
        }
    }

    Ok(Json(json!({ "status": "rejected", "file_id": file_id })))
}

/// POST /api/approvals/:company_id/:file_id/send
/// Manually send an existing approved file for approval review
pub async fn send_for_approval(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify the file exists and belongs to this tenant
    let file: Option<(Uuid, String, Option<Uuid>, bool)> = sqlx::query_as(
        "SELECT owner_id, approval_status, department_id, is_company_folder FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (owner_id, approval_status, department_id, is_company_folder) = file.ok_or(StatusCode::NOT_FOUND)?;

    // Only the file owner or an admin can send for approval
    if owner_id != auth.user_id && !matches!(auth.role.as_str(), "Admin" | "SuperAdmin") {
        return Err(StatusCode::FORBIDDEN);
    }

    // File must currently be approved (not already pending or rejected)
    if approval_status != "approved" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Find matching policy
    let policy = find_matching_policy_simple(&state.pool, company_id, department_id, is_company_folder).await;
    let policy_id = policy.map(|p| p.id);

    // Set file to pending
    sqlx::query("UPDATE files_metadata SET approval_status = 'pending' WHERE id = $1")
        .bind(file_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create approval request
    let new_request: (Uuid,) = sqlx::query_as(
        "INSERT INTO approval_requests (tenant_id, file_id, policy_id, requested_by) VALUES ($1, $2, $3, $4) RETURNING id"
    )
    .bind(company_id)
    .bind(file_id)
    .bind(policy_id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
           VALUES ($1, $2, 'file_sent_for_approval', 'file', $3, $4, $5::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(file_id)
    .bind(json!({"approval_request_id": new_request.0}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Notify approvers
    if let Ok(Some(tenant)) = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(company_id).fetch_optional(&state.pool).await
    {
        if let Ok(Some((file_name,))) = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM files_metadata WHERE id = $1"
        ).bind(file_id).fetch_optional(&state.pool).await {
            let _ = notification_service::notify_all_admins(
                &state.pool,
                &tenant,
                notification_service::NotificationType::ApprovalRequired,
                "File Sent for Approval",
                &format!("\"{}\" has been manually sent for approval.", file_name),
                Some(json!({"file_id": file_id, "file_name": &file_name, "sender_id": auth.user_id})),
            ).await;
        }
    }

    Ok(Json(json!({ "status": "pending", "approval_request_id": new_request.0 })))
}

/// POST /api/approvals/:company_id/:file_id/resubmit
/// Owner resubmits a rejected file for approval
pub async fn resubmit(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify the file exists, belongs to this tenant, and the user owns it
    let file: Option<(Uuid, String, Option<Uuid>, bool)> = sqlx::query_as(
        "SELECT owner_id, approval_status, department_id, is_company_folder FROM files_metadata WHERE id = $1 AND tenant_id = $2"
    )
    .bind(file_id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (owner_id, approval_status, department_id, is_company_folder) = file.ok_or(StatusCode::NOT_FOUND)?;

    if owner_id != auth.user_id {
        return Err(StatusCode::FORBIDDEN);
    }
    if approval_status != "rejected" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Find matching policy
    let policy = find_matching_policy_simple(&state.pool, company_id, department_id, is_company_folder).await;
    let policy_id = policy.map(|p| p.id);

    // Reset file status to pending
    sqlx::query("UPDATE files_metadata SET approval_status = 'pending' WHERE id = $1")
        .bind(file_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create new approval request
    let new_request: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO approval_requests (tenant_id, file_id, policy_id, requested_by)
           VALUES ($1, $2, $3, $4)
           RETURNING id"#
    )
    .bind(company_id)
    .bind(file_id)
    .bind(policy_id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
           VALUES ($1, $2, 'file_resubmitted', 'file', $3, $4, $5::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(file_id)
    .bind(json!({"approval_request_id": new_request.0}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({ "status": "pending", "approval_request_id": new_request.0 })))
}

// ==================== Policy CRUD ====================

/// GET /api/approvals/:company_id/policies
pub async fn list_policies(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    require_admin(&auth)?;

    let policies: Vec<ApprovalPolicy> = sqlx::query_as(
        "SELECT * FROM approval_policies WHERE tenant_id = $1 ORDER BY created_at ASC"
    )
    .bind(company_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "policies": policies })))
}

/// POST /api/approvals/:company_id/policies
pub async fn create_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreatePolicyInput>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    require_admin(&auth)?;

    // Validate scope
    if !matches!(input.scope.as_str(), "all" | "department" | "company_folder" | "file_type" | "file_size" | "role" | "private_files") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if matches!(input.scope.as_str(), "department" | "file_type" | "file_size" | "role") && input.scope_value.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if input.name.trim().is_empty() || input.name.len() > 255 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let policy: ApprovalPolicy = sqlx::query_as(
        r#"INSERT INTO approval_policies (tenant_id, name, scope, scope_value)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#
    )
    .bind(company_id)
    .bind(input.name.trim())
    .bind(&input.scope)
    .bind(&input.scope_value)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create approval policy: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
           VALUES ($1, $2, 'approval_policy_created', 'approval_policy', $3, $4, $5::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(policy.id)
    .bind(json!({"name": &policy.name, "scope": &policy.scope}))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({ "policy": policy })))
}

/// PUT /api/approvals/:company_id/policies/:id
pub async fn update_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, policy_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdatePolicyInput>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    require_admin(&auth)?;

    if let Some(ref scope) = input.scope {
        if !matches!(scope.as_str(), "all" | "department" | "company_folder" | "file_type" | "file_size" | "role" | "private_files") {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let mut updates = Vec::new();
    let mut param_count = 3;

    if let Some(_) = &input.name {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if let Some(_) = &input.scope {
        updates.push(format!("scope = ${}", param_count));
        param_count += 1;
    }
    // Always include scope_value if it was explicitly sent (even as null)
    if let Some(_) = &input.scope_value {
        updates.push(format!("scope_value = ${}", param_count));
        param_count += 1;
    }
    if let Some(_) = &input.is_active {
        updates.push(format!("is_active = ${}", param_count));
    }

    if updates.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    updates.push("updated_at = NOW()".to_string());
    let query = format!(
        "UPDATE approval_policies SET {} WHERE id = $1 AND tenant_id = $2 RETURNING *",
        updates.join(", ")
    );

    let mut db_query = sqlx::query_as::<_, ApprovalPolicy>(&query)
        .bind(policy_id)
        .bind(company_id);

    if let Some(name) = input.name {
        db_query = db_query.bind(name);
    }
    if let Some(scope) = input.scope {
        db_query = db_query.bind(scope);
    }
    if let Some(scope_value) = input.scope_value {
        // scope_value is Option<Option<String>> — flatten to Option<String> for binding
        db_query = db_query.bind(scope_value);
    }
    if let Some(is_active) = input.is_active {
        db_query = db_query.bind(is_active);
    }

    let policy = db_query
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({ "policy": policy })))
}

/// DELETE /api/approvals/:company_id/policies/:id
pub async fn delete_policy(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, policy_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, StatusCode> {
    if auth.tenant_id != company_id {
        return Err(StatusCode::FORBIDDEN);
    }
    require_admin(&auth)?;

    let result = sqlx::query(
        "DELETE FROM approval_policies WHERE id = $1 AND tenant_id = $2"
    )
    .bind(policy_id)
    .bind(company_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Audit log
    let _ = sqlx::query(
        r#"INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, ip_address)
           VALUES ($1, $2, 'approval_policy_deleted', 'approval_policy', $3, $4::inet)"#
    )
    .bind(company_id)
    .bind(auth.user_id)
    .bind(policy_id)
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({ "deleted": true })))
}
