//! User-Specific File Sharing Handlers
//!
//! Provides endpoints for:
//! - Listing shareable users (respecting tenant/department boundaries)
//! - Listing files shared with the current user
//! - Filtering share recipients by access control rules

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::middleware::AuthUser;
use crate::AppState;

// ==================== Models ====================

#[derive(Debug, Serialize)]
pub struct ShareableUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct SharedFile {
    pub id: Uuid,
    pub name: String,
    pub size: i64,
    pub content_type: Option<String>,
    pub folder_path: Option<String>,
    pub shared_by_id: Uuid,
    pub shared_by_name: String,
    pub shared_at: DateTime<Utc>,
    pub share_token: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ShareableUsersQuery {
    pub search: Option<String>,
    pub department_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SharedWithMeQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

// ==================== Access Control Rules ====================

/// Gets the list of department IDs a user can share with.
/// - Admin/SuperAdmin: all departments in tenant
/// - Others: their own department + any explicitly accessible departments
async fn get_accessible_department_ids(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    tenant_id: Uuid,
    role: &str,
) -> Result<Vec<Uuid>, StatusCode> {
    // Admins can share with anyone in the tenant
    if role == "Admin" || role == "SuperAdmin" {
        let all_depts: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM departments WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        return Ok(all_depts.into_iter().map(|(id,)| id).collect());
    }
    
    // Get user's own department
    let user_dept: Option<(Option<Uuid>,)> = sqlx::query_as(
        "SELECT department_id FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let own_dept = user_dept.and_then(|(d,)| d);
    let mut dept_ids = Vec::new();
    
    if let Some(d) = own_dept {
        dept_ids.push(d);
    }
    
    // Check for any additional department access (e.g., cross-department permissions)
    // This could be extended with a department_access table if needed
    // For now, users only have access to their own department
    
    Ok(dept_ids)
}

// ==================== Handlers ====================

/// List users available for sharing (respects tenant/department boundaries)
/// GET /api/users/{company_id}/shareable
pub async fn list_shareable_users(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Query(query): Query<ShareableUsersQuery>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Get accessible department IDs based on user's role
    let accessible_depts = get_accessible_department_ids(&state.pool, auth.user_id, tenant_id, &auth.role).await?;
    
    // Build query for shareable users
    // Must be: same tenant, in accessible departments, not the current user
    let users: Vec<ShareableUser> = if auth.role == "Admin" || auth.role == "SuperAdmin" {
        // Admins can see all users in tenant
        if let Some(search) = &query.search {
            let search_pattern = format!("%{}%", search.to_lowercase());
            sqlx::query_as!(
                ShareableUser,
                r#"
                SELECT 
                    u.id,
                    u.name,
                    u.email,
                    u.department_id,
                    d.name as "department_name?",
                    u.role
                FROM users u
                LEFT JOIN departments d ON u.department_id = d.id
                WHERE u.tenant_id = $1 
                  AND u.id != $2
                  AND u.status = 'active'
                  AND (LOWER(u.name) LIKE $3 OR LOWER(u.email) LIKE $3)
                ORDER BY u.name
                LIMIT 50
                "#,
                tenant_id,
                auth.user_id,
                search_pattern
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch shareable users: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        } else {
            sqlx::query_as!(
                ShareableUser,
                r#"
                SELECT 
                    u.id,
                    u.name,
                    u.email,
                    u.department_id,
                    d.name as "department_name?",
                    u.role
                FROM users u
                LEFT JOIN departments d ON u.department_id = d.id
                WHERE u.tenant_id = $1 
                  AND u.id != $2
                  AND u.status = 'active'
                ORDER BY u.name
                LIMIT 50
                "#,
                tenant_id,
                auth.user_id
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch shareable users: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        }
    } else {
        // Non-admins can only see users in their accessible departments
        if accessible_depts.is_empty() {
            Vec::new()
        } else if let Some(search) = &query.search {
            let search_pattern = format!("%{}%", search.to_lowercase());
            sqlx::query_as!(
                ShareableUser,
                r#"
                SELECT 
                    u.id,
                    u.name,
                    u.email,
                    u.department_id,
                    d.name as "department_name?",
                    u.role
                FROM users u
                LEFT JOIN departments d ON u.department_id = d.id
                WHERE u.tenant_id = $1 
                  AND u.id != $2
                  AND u.status = 'active'
                  AND u.department_id = ANY($3)
                  AND (LOWER(u.name) LIKE $4 OR LOWER(u.email) LIKE $4)
                ORDER BY u.name
                LIMIT 50
                "#,
                tenant_id,
                auth.user_id,
                &accessible_depts,
                search_pattern
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch shareable users: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        } else {
            sqlx::query_as!(
                ShareableUser,
                r#"
                SELECT 
                    u.id,
                    u.name,
                    u.email,
                    u.department_id,
                    d.name as "department_name?",
                    u.role
                FROM users u
                LEFT JOIN departments d ON u.department_id = d.id
                WHERE u.tenant_id = $1 
                  AND u.id != $2
                  AND u.status = 'active'
                  AND u.department_id = ANY($3)
                ORDER BY u.name
                LIMIT 50
                "#,
                tenant_id,
                auth.user_id,
                &accessible_depts
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch shareable users: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        }
    };
    
    Ok(Json(json!({
        "users": users,
        "total": users.len()
    })))
}

/// List files shared with the current user
/// GET /api/shared-with-me
pub async fn list_shared_with_me(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(query): Query<SharedWithMeQuery>,
) -> Result<Json<Value>, StatusCode> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;
    
    // Count total files shared with user
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM file_shares fs
        JOIN files_metadata fm ON fs.file_id = fm.id
        WHERE fs.shared_with_user_id = $1 
          AND fs.tenant_id = $2
          AND (fs.expires_at IS NULL OR fs.expires_at > NOW())
        "#
    )
    .bind(auth.user_id)
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Fetch shared files
    let files: Vec<(Uuid, String, i64, Option<String>, Option<String>, Uuid, String, DateTime<Utc>, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT 
            fm.id,
            fm.name,
            fm.size_bytes,
            fm.content_type,
            fm.parent_path,
            u.id as shared_by_id,
            u.name as shared_by_name,
            fs.created_at as shared_at,
            fs.token,
            fs.expires_at
        FROM file_shares fs
        JOIN files_metadata fm ON fs.file_id = fm.id
        JOIN users u ON fs.created_by = u.id
        WHERE fs.shared_with_user_id = $1 
          AND fs.tenant_id = $2
          AND (fs.expires_at IS NULL OR fs.expires_at > NOW())
        ORDER BY fs.created_at DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(auth.user_id)
    .bind(auth.tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch shared files: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let shared_files: Vec<SharedFile> = files.into_iter().map(|(id, name, size, content_type, folder_path, shared_by_id, shared_by_name, shared_at, share_token, expires_at)| {
        SharedFile {
            id,
            name,
            size,
            content_type,
            folder_path,
            shared_by_id,
            shared_by_name,
            shared_at,
            share_token,
            expires_at,
        }
    }).collect();
    
    let total_pages = (total as f64 / per_page as f64).ceil() as i64;
    
    Ok(Json(json!({
        "files": shared_files,
        "total": total,
        "page": page,
        "per_page": per_page,
        "total_pages": total_pages
    })))
}

// ==================== Copy to My Files ====================

#[derive(Debug, Deserialize)]
pub struct CopyToMyFilesInput {
    pub file_id: Uuid,
    pub share_token: String,
}

/// Copy a shared file to the user's private files
/// POST /api/shared-with-me/copy
pub async fn copy_to_my_files(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<CopyToMyFilesInput>,
) -> Result<Json<Value>, StatusCode> {
    // Verify the share exists and is valid for this user
    let share: Option<(Uuid, Uuid, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT file_id, tenant_id, expires_at
        FROM file_shares 
        WHERE token = $1 
          AND shared_with_user_id = $2
          AND tenant_id = $3
        "#
    )
    .bind(&input.share_token)
    .bind(auth.user_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch share: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let (file_id, tenant_id, expires_at) = share.ok_or(StatusCode::NOT_FOUND)?;
    
    // Check if share has expired
    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(StatusCode::GONE);
        }
    }
    
    // Verify file_id matches
    if file_id != input.file_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Get original file metadata
    let original: Option<(String, String, i64, Option<String>, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT name, storage_path, size_bytes, content_type, department_id
        FROM files_metadata 
        WHERE id = $1 
          AND tenant_id = $2 
          AND is_deleted = false
        "#
    )
    .bind(file_id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch file metadata: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let (file_name, storage_path, size_bytes, content_type, _original_dept) = original.ok_or(StatusCode::NOT_FOUND)?;
    
    // Get user's department for the new file
    let user_dept: Option<Uuid> = sqlx::query_scalar(
        "SELECT department_id FROM users WHERE id = $1"
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Generate new storage path for the copy
    let new_file_id = Uuid::new_v4();
    let new_ulid = ulid::Ulid::new().to_string();
    let extension = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let new_storage_path = format!("{}/{}/{}{}", tenant_id, auth.user_id, new_file_id, extension);
    
    // Download the original file
    let file_data = state.storage.download(&storage_path).await.map_err(|e| {
        tracing::error!("Failed to download original file: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Upload to the new location
    state.storage.upload(&new_storage_path, file_data).await.map_err(|e| {
        tracing::error!("Failed to upload copied file: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Create new file metadata entry
    let new_file: (Uuid, String, DateTime<Utc>) = sqlx::query_as(
        r#"
        INSERT INTO files_metadata (
            id, tenant_id, department_id, name, storage_path, size_bytes, 
            content_type, is_directory, owner_id, parent_path, visibility, ulid
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, $9, 'private', $10)
        RETURNING id, name, created_at
        "#
    )
    .bind(new_file_id)
    .bind(auth.tenant_id)
    .bind(user_dept)
    .bind(&file_name)
    .bind(&new_storage_path)
    .bind(size_bytes)
    .bind(&content_type)
    .bind(auth.user_id)
    .bind::<Option<String>>(None) // Root level of private files
    .bind(&new_ulid)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create file metadata: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Log the action
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'file_copied_from_share', 'file', $3, $4, $5::inet)
        "#
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(new_file_id)
    .bind(serde_json::json!({
        "original_file_id": file_id,
        "file_name": file_name,
        "share_token": input.share_token,
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;
    
    tracing::info!(
        user_id = %auth.user_id,
        original_file = %file_id,
        new_file = %new_file_id,
        "File copied from share to private files"
    );
    
    Ok(Json(serde_json::json!({
        "success": true,
        "file": {
            "id": new_file.0,
            "name": new_file.1,
            "created_at": new_file.2,
        },
        "message": format!("\"{}\" has been saved to your files", file_name)
    })))
}

/// Validate that a user can share with another user
/// Returns true if sharing is allowed
pub async fn can_share_with_user(
    pool: &sqlx::PgPool,
    sharer_id: Uuid,
    sharer_tenant_id: Uuid,
    sharer_role: &str,
    recipient_id: Uuid,
) -> Result<bool, StatusCode> {
    // Get recipient's tenant and department
    let recipient: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT tenant_id, department_id FROM users WHERE id = $1 AND status = 'active'"
    )
    .bind(recipient_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (recipient_tenant_id, recipient_dept_id) = match recipient {
        Some(r) => r,
        None => return Ok(false), // User doesn't exist or is inactive
    };
    
    // CRITICAL: Must be same tenant
    if recipient_tenant_id != sharer_tenant_id {
        tracing::warn!(
            sharer_id = %sharer_id, 
            recipient_id = %recipient_id, 
            "Cross-tenant share attempt blocked"
        );
        return Ok(false);
    }
    
    // Admins can share with anyone in their tenant
    if sharer_role == "Admin" || sharer_role == "SuperAdmin" {
        return Ok(true);
    }
    
    // For regular users, check department access
    let accessible_depts = get_accessible_department_ids(pool, sharer_id, sharer_tenant_id, sharer_role).await?;
    
    // Check if recipient is in an accessible department
    if let Some(dept_id) = recipient_dept_id {
        Ok(accessible_depts.contains(&dept_id))
    } else {
        // User has no department - only allow if sharer also has no department
        let sharer_dept: Option<(Option<Uuid>,)> = sqlx::query_as(
            "SELECT department_id FROM users WHERE id = $1"
        )
        .bind(sharer_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        Ok(sharer_dept.and_then(|(d,)| d).is_none())
    }
}

