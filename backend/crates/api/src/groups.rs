//! File Groups API handlers
//! 
//! Allows users to create and manage file groups - manual collections of related files.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::AuthUser;
use crate::AppState;

/// Maximum number of files allowed per group
const MAX_FILES_PER_GROUP: i64 = 20;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Serialize, FromRow)]
pub struct FileGroup {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub department_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub parent_path: Option<String>, // Folder path where this group lives (null = root)
    // Locking fields
    pub is_locked: Option<bool>,
    pub locked_by: Option<Uuid>,
    pub locked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub lock_requires_role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileGroupWithCount {
    #[serde(flatten)]
    pub group: FileGroup,
    pub file_count: i64,
    pub total_size: i64, // Total size in bytes of all files in the group
    pub owner_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupInput {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub department_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListGroupsParams {
    pub department_id: Option<String>,
    pub parent_path: Option<String>, // Filter by folder path (empty string = root)
}

/// Row structure for list_groups query (to work around SQLx tuple size limit)
#[derive(Debug, FromRow)]
struct ListGroupsRow {
    id: Uuid,
    tenant_id: Uuid,
    department_id: Option<Uuid>,
    name: String,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
    created_by: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    parent_path: Option<String>,
    is_locked: Option<bool>,
    locked_by: Option<Uuid>,
    locked_at: Option<chrono::DateTime<chrono::Utc>>,
    lock_requires_role: Option<String>,
    file_count: i64,
    total_size: i64,
    owner_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddToGroupInput {
    pub group_id: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all groups for the tenant (filtered by department if applicable)
/// GET /api/groups/{company_id}
pub async fn list_groups(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Query(params): Query<ListGroupsParams>,
) -> Result<Json<Vec<FileGroupWithCount>>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Parse department filter
    let dept_filter: Option<Uuid> = params.department_id
        .as_ref()
        .and_then(|s| if s.is_empty() { None } else { Uuid::parse_str(s).ok() });

    // Parse parent_path filter (empty string means root, None means all)
    let path_filter = params.parent_path.as_deref();

    // Query groups with file counts, total size, owner names, and locking info
    // Filter by parent_path to only show groups in the current folder
    let groups: Vec<ListGroupsRow> = sqlx::query_as(
        r#"
        SELECT 
            g.id, g.tenant_id, g.department_id, g.name, g.description, g.color, g.icon,
            g.created_by, g.created_at, g.updated_at, g.parent_path,
            g.is_locked, g.locked_by, g.locked_at, g.lock_requires_role,
            COALESCE(COUNT(f.id), 0)::bigint as file_count,
            COALESCE(SUM(f.size_bytes), 0)::bigint as total_size,
            u.name as owner_name
        FROM file_groups g
        LEFT JOIN files_metadata f ON f.group_id = g.id AND f.is_deleted = false
        LEFT JOIN users u ON u.id = g.created_by
        WHERE g.tenant_id = $1
        AND ($2::uuid IS NULL OR g.department_id IS NULL OR g.department_id = $2)
        AND (
            $3::text IS NULL 
            OR ($3 = '' AND (g.parent_path IS NULL OR g.parent_path = ''))
            OR g.parent_path = $3
        )
        GROUP BY g.id, u.name
        ORDER BY g.name ASC
        "#
    )
    .bind(tenant_id)
    .bind(dept_filter)
    .bind(path_filter)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list groups: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let result: Vec<FileGroupWithCount> = groups.into_iter().map(|row| {
        FileGroupWithCount {
            group: FileGroup {
                id: row.id,
                tenant_id: row.tenant_id,
                department_id: row.department_id,
                name: row.name,
                description: row.description,
                color: row.color,
                icon: row.icon,
                created_by: row.created_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
                parent_path: row.parent_path,
                is_locked: row.is_locked,
                locked_by: row.locked_by,
                locked_at: row.locked_at,
                lock_requires_role: row.lock_requires_role,
            },
            file_count: row.file_count,
            total_size: row.total_size,
            owner_name: row.owner_name,
        }
    }).collect();

    Ok(Json(result))
}

/// Create a new file group
/// POST /api/groups/{company_id}
pub async fn create_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Json(input): Json<CreateGroupInput>,
) -> Result<Json<FileGroup>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate name
    let name = input.name.trim();
    if name.is_empty() || name.len() > 255 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse department_id
    let department_id: Option<Uuid> = input.department_id
        .as_ref()
        .and_then(|s| if s.is_empty() { None } else { Uuid::parse_str(s).ok() });

    // Validate color format if provided
    if let Some(ref color) = input.color {
        if !color.starts_with('#') || color.len() != 7 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Create the group
    let group: FileGroup = sqlx::query_as(
        r#"
        INSERT INTO file_groups (tenant_id, department_id, name, description, color, icon, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, tenant_id, department_id, name, description, color, icon, created_by, created_at, updated_at, parent_path, is_locked, locked_by, locked_at, lock_requires_role
        "#
    )
    .bind(tenant_id)
    .bind(department_id)
    .bind(name)
    .bind(&input.description)
    .bind(&input.color)
    .bind(input.icon.as_deref().unwrap_or("folder-kanban"))
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            tracing::warn!("Duplicate group name: {}", name);
            StatusCode::CONFLICT
        } else {
            tracing::error!("Failed to create group: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'group_created', 'file_group', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(group.id)
    .bind(json!({ "name": name }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    tracing::info!(user_id = %auth.user_id, group_id = %group.id, "File group created");

    Ok(Json(group))
}

/// Update a file group
/// PUT /api/groups/{company_id}/{group_id}
pub async fn update_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
    Json(input): Json<UpdateGroupInput>,
) -> Result<Json<FileGroup>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check group exists and belongs to tenant
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Validate color format if provided
    if let Some(ref color) = input.color {
        if !color.starts_with('#') || color.len() != 7 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Build update query dynamically
    let group: FileGroup = sqlx::query_as(
        r#"
        UPDATE file_groups SET
            name = COALESCE($3, name),
            description = COALESCE($4, description),
            color = COALESCE($5, color),
            icon = COALESCE($6, icon),
            updated_at = NOW()
        WHERE id = $1 AND tenant_id = $2
        RETURNING id, tenant_id, department_id, name, description, color, icon, created_by, created_at, updated_at, parent_path, is_locked, locked_by, locked_at, lock_requires_role
        "#
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .bind(&input.name)
    .bind(&input.description)
    .bind(&input.color)
    .bind(&input.icon)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            StatusCode::CONFLICT
        } else {
            tracing::error!("Failed to update group: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    tracing::info!(user_id = %auth.user_id, group_id = %group_uuid, "File group updated");

    Ok(Json(group))
}

/// Delete a file group (files are unlinked, not deleted)
/// DELETE /api/groups/{company_id}/{group_id}
pub async fn delete_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get group info for audit log
    let group_name: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((name,)) = group_name else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Delete the group (files will have group_id set to NULL due to ON DELETE SET NULL)
    sqlx::query("DELETE FROM file_groups WHERE id = $1 AND tenant_id = $2")
        .bind(group_uuid)
        .bind(tenant_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete group: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'group_deleted', 'file_group', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(group_uuid)
    .bind(json!({ "name": name }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    tracing::info!(user_id = %auth.user_id, group_id = %group_uuid, "File group deleted");

    Ok(Json(json!({ "success": true, "message": "Group deleted. Files have been unlinked." })))
}

/// Add a file to a group
/// POST /api/files/{company_id}/{file_id}/group
pub async fn add_file_to_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<AddToGroupInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&input.group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify file exists and belongs to tenant
    let file_exists: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false AND is_directory = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((file_name,)) = file_exists else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Verify group exists and belongs to tenant
    let group_exists: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((group_name,)) = group_exists else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Check if group already has max files
    let current_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM files_metadata WHERE group_id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if current_count.0 >= MAX_FILES_PER_GROUP {
        tracing::warn!("Group {} has reached max file limit of {}", group_uuid, MAX_FILES_PER_GROUP);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Update file's group_id
    sqlx::query("UPDATE files_metadata SET group_id = $1 WHERE id = $2 AND tenant_id = $3")
        .bind(group_uuid)
        .bind(file_uuid)
        .bind(tenant_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to add file to group: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        user_id = %auth.user_id,
        file_id = %file_uuid,
        group_id = %group_uuid,
        "File added to group"
    );

    // Invalidate file cache since grouped files are now hidden from main list
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }

    Ok(Json(json!({
        "success": true,
        "message": format!("'{}' added to group '{}'", file_name, group_name)
    })))
}

/// Remove a file from its group
/// DELETE /api/files/{company_id}/{file_id}/group
pub async fn remove_file_from_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Remove file from group
    let result = sqlx::query(
        "UPDATE files_metadata SET group_id = NULL WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to remove file from group: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    tracing::info!(user_id = %auth.user_id, file_id = %file_uuid, "File removed from group");

    // Invalidate file cache since file is now visible in main list again
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }

    Ok(Json(json!({ "success": true, "message": "File removed from group" })))
}

/// Get files in a specific group
/// GET /api/groups/{company_id}/{group_id}/files
pub async fn get_group_files(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get group info
    let group: Option<FileGroup> = sqlx::query_as(
        "SELECT id, tenant_id, department_id, name, description, color, icon, created_by, created_at, updated_at, parent_path, is_locked, locked_by, locked_at, lock_requires_role FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(group) = group else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Check if user can access locked group
    if group.is_locked.unwrap_or(false) {
        if !can_access_locked_group(
            auth.user_id,
            &auth.role,
            true,
            group.locked_by,
            group.created_by,
            group.lock_requires_role.as_deref(),
        ) {
            tracing::warn!(
                "Access denied: user {} (role: {}) attempted to access locked group {} (requires: {:?})",
                auth.user_id, auth.role, group_uuid, group.lock_requires_role
            );
            return Ok(Json(json!({
                "error": "Group is locked - access denied",
                "is_locked": true,
                "lock_requires_role": group.lock_requires_role
            })));
        }
    }

    // Get files in this group
    tracing::info!("Fetching files for group {} in tenant {}", group_uuid, tenant_id);
    
    let files: Vec<(Uuid, String, i64, Option<String>, Option<String>, Option<Uuid>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, name, size_bytes, content_type, parent_path, owner_id, created_at
        FROM files_metadata
        WHERE group_id = $1 AND tenant_id = $2 AND is_deleted = false AND is_directory = false
        ORDER BY name ASC
        "#
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get group files: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!("Found {} files in group {}", files.len(), group_uuid);

    let files_json: Vec<Value> = files.into_iter().map(|f| {
        json!({
            "id": f.0,
            "name": f.1,
            "size_bytes": f.2,
            "content_type": f.3,
            "parent_path": f.4,
            "owner_id": f.5,
            "created_at": f.6,
            "type": "file"
        })
    }).collect();

    Ok(Json(json!({
        "group": group,
        "files": files_json
    })))
}

#[derive(Debug, Deserialize)]
pub struct MoveGroupInput {
    pub target_folder_id: Option<String>,
    pub target_path: Option<String>,
}

/// Move a group to a folder (updates the group's parent_path, not the files)
/// PUT /api/groups/{company_id}/{group_id}/move
pub async fn move_group_to_folder(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
    Json(input): Json<MoveGroupInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get group info
    let group: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, parent_path FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((group_name, old_path)) = group else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Determine target path
    let target_path = if let Some(folder_id) = &input.target_folder_id {
        let folder_uuid = Uuid::parse_str(folder_id).map_err(|_| StatusCode::BAD_REQUEST)?;
        
        // Get folder's path
        let folder: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT name, parent_path FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_directory = true"
        )
        .bind(folder_uuid)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some((folder_name, parent_path)) = folder else {
            return Err(StatusCode::NOT_FOUND);
        };

        // Construct full path to folder
        match parent_path {
            Some(pp) if !pp.is_empty() => format!("{}/{}", pp, folder_name),
            _ => folder_name,
        }
    } else if let Some(path) = &input.target_path {
        path.clone()
    } else {
        // Move to root
        String::new()
    };

    // Update the GROUP's parent_path (not the files)
    sqlx::query(
        r#"
        UPDATE file_groups 
        SET parent_path = $3, updated_at = NOW()
        WHERE id = $1 AND tenant_id = $2
        "#
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .bind(if target_path.is_empty() { None } else { Some(&target_path) })
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to move group: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'group_moved', 'file_group', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(group_uuid)
    .bind(json!({ "name": group_name, "from": old_path, "to": target_path }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    tracing::info!(
        user_id = %auth.user_id,
        group_id = %group_uuid,
        target_path = %target_path,
        "Group moved to folder"
    );

    // Invalidate file cache
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }

    Ok(Json(json!({
        "success": true,
        "message": format!("Moved group '{}' to '{}'", group_name, if target_path.is_empty() { "root" } else { &target_path })
    })))
}

// ============================================================================
// Group Locking
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct LockGroupInput {
    pub password: Option<String>,      // Optional password for additional security
    pub required_role: Option<String>, // Optional role requirement (Admin, Manager, Employee)
}

#[derive(Debug, Deserialize)]
pub struct UnlockGroupInput {
    pub password: Option<String>, // Password if the group is password-locked
}

/// Lock a group (prevents access to files within)
/// POST /api/groups/{company_id}/{group_id}/lock
pub async fn lock_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
    Json(input): Json<LockGroupInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if user has lock permission (Manager, Admin, SuperAdmin)
    let has_lock_permission = ["SuperAdmin", "Admin", "Manager"].contains(&auth.role.as_str());
    if !has_lock_permission {
        // Check for custom role with files.lock permission
        let custom_role_has_perm: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM roles r
                WHERE r.tenant_id = $1 AND r.name = $2 AND r.permissions @> $3
            )
            "#
        )
        .bind(tenant_id)
        .bind(&auth.role)
        .bind(json!(["files.lock"]))
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if !custom_role_has_perm.map(|r| r.0).unwrap_or(false) {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Get current group status
    let group: Option<(String, bool, Option<Uuid>)> = sqlx::query_as(
        "SELECT name, COALESCE(is_locked, false), locked_by FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (group_name, is_locked, locked_by) = group.ok_or(StatusCode::NOT_FOUND)?;

    if is_locked {
        return Ok(Json(json!({
            "error": "Group is already locked",
            "locked_by": locked_by
        })));
    }

    // Process optional password and role requirement
    let password_hash: Option<String> = if let Some(ref pwd) = input.password {
        if !pwd.is_empty() {
            use argon2::{PasswordHasher, password_hash::SaltString};
            use argon2::password_hash::rand_core::OsRng;
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = crate::password::get_argon2();
            Some(argon2.hash_password(pwd.as_bytes(), &salt)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .to_string())
        } else {
            None
        }
    } else {
        None
    };
    
    let required_role = input.required_role.clone();

    // Lock the group
    sqlx::query(
        r#"
        UPDATE file_groups 
        SET is_locked = true, locked_by = $1, locked_at = NOW(), 
            lock_password_hash = $3, lock_requires_role = $4, updated_at = NOW()
        WHERE id = $2
        "#
    )
    .bind(auth.user_id)
    .bind(group_uuid)
    .bind(&password_hash)
    .bind(&required_role)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'group_locked', 'file_group', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(group_uuid)
    .bind(json!({ 
        "name": group_name, 
        "has_password": password_hash.is_some(),
        "requires_role": required_role
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    tracing::info!(
        user_id = %auth.user_id,
        group_id = %group_uuid,
        "Group locked"
    );

    Ok(Json(json!({
        "success": true,
        "message": format!("Group '{}' has been locked", group_name)
    })))
}

/// Unlock a group
/// POST /api/groups/{company_id}/{group_id}/unlock
pub async fn unlock_group(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
    Json(input): Json<UnlockGroupInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current group status including lock details
    let group: Option<(String, bool, Option<Uuid>, Option<String>, Option<String>, Uuid)> = sqlx::query_as(
        r#"
        SELECT name, COALESCE(is_locked, false), locked_by, lock_password_hash, lock_requires_role, created_by 
        FROM file_groups 
        WHERE id = $1 AND tenant_id = $2
        "#
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (group_name, is_locked, locked_by, password_hash, required_role, owner_id) = group.ok_or(StatusCode::NOT_FOUND)?;

    if !is_locked {
        return Ok(Json(json!({ "message": "Group is not locked" })));
    }

    // Role hierarchy for permission checking
    let role_hierarchy = |role: &str| -> i32 {
        match role {
            "SuperAdmin" => 100,
            "Admin" => 80,
            "Manager" => 60,
            "Employee" => 40,
            _ => 20, // Custom roles
        }
    };

    // Check if user can unlock based on role requirement
    let mut can_unlock = false;

    // Group owner can always unlock
    if owner_id == auth.user_id {
        can_unlock = true;
    }
    // User who locked it can always unlock
    else if locked_by == Some(auth.user_id) {
        can_unlock = true;
    }
    // SuperAdmin can always unlock
    else if auth.role == "SuperAdmin" {
        can_unlock = true;
    }
    // Check role requirement
    else if let Some(ref req_role) = required_role {
        let user_level = role_hierarchy(&auth.role);
        let required_level = role_hierarchy(req_role);
        
        if user_level >= required_level {
            can_unlock = true;
        } else {
            // Check if user has custom role with files.unlock permission
            let custom_role_has_perm: Option<(bool,)> = sqlx::query_as(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM roles r
                    WHERE r.tenant_id = $1 AND r.name = $2 AND r.permissions @> $3
                )
                "#
            )
            .bind(tenant_id)
            .bind(&auth.role)
            .bind(json!(["files.unlock"]))
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if custom_role_has_perm.map(|r| r.0).unwrap_or(false) {
                can_unlock = true;
            }
        }
    } else {
        // No role requirement - Admin or above can unlock
        if ["SuperAdmin", "Admin"].contains(&auth.role.as_str()) {
            can_unlock = true;
        }
    }

    if !can_unlock {
        return Ok(Json(json!({
            "error": "Insufficient permissions",
            "required_role": required_role,
            "has_password": password_hash.is_some()
        })));
    }

    // Check password if required
    if let Some(ref pwd_hash) = password_hash {
        let provided_password = input.password.as_deref().unwrap_or("");
        
        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        let parsed_hash = PasswordHash::new(pwd_hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if Argon2::default().verify_password(provided_password.as_bytes(), &parsed_hash).is_err() {
            return Ok(Json(json!({
                "error": "Invalid password",
                "has_password": true
            })));
        }
    }

    // Unlock the group
    sqlx::query(
        r#"
        UPDATE file_groups 
        SET is_locked = false, locked_by = NULL, locked_at = NULL, 
            lock_password_hash = NULL, lock_requires_role = NULL, updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(group_uuid)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'group_unlocked', 'file_group', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(group_uuid)
    .bind(json!({ "name": group_name }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    tracing::info!(
        user_id = %auth.user_id,
        group_id = %group_uuid,
        "Group unlocked"
    );

    Ok(Json(json!({
        "success": true,
        "message": format!("Group '{}' has been unlocked", group_name)
    })))
}

/// Check if user can access a locked group
pub fn can_access_locked_group(
    user_id: Uuid,
    user_role: &str,
    is_locked: bool,
    locked_by: Option<Uuid>,
    owner_id: Uuid,
    lock_requires_role: Option<&str>,
) -> bool {
    if !is_locked {
        return true;
    }

    // Owner can always access
    if owner_id == user_id {
        return true;
    }

    // User who locked it can always access
    if locked_by == Some(user_id) {
        return true;
    }

    // SuperAdmin can always access
    if user_role == "SuperAdmin" {
        return true;
    }

    // Check role requirement
    if let Some(req_role) = lock_requires_role {
        let role_level = |role: &str| -> i32 {
            match role {
                "SuperAdmin" => 100,
                "Admin" => 80,
                "Manager" => 60,
                "Employee" => 40,
                _ => 20,
            }
        };
        
        let user_level = role_level(user_role);
        let required_level = role_level(req_role);
        
        return user_level >= required_level;
    }

    false
}

/// Toggle star status for a group
/// POST /api/groups/{company_id}/{group_id}/star
pub async fn toggle_group_star(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, group_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let group_uuid = Uuid::parse_str(&group_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify group exists
    let group_exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM file_groups WHERE id = $1 AND tenant_id = $2"
    )
    .bind(group_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if group_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get current user prefs from S3
    let user_prefs_key = format!(".clovalink/{}/user_prefs/{}.json", tenant_id, auth.user_id);
    let mut prefs: Value = match state.storage.download(&user_prefs_key).await {
        Ok(data) => serde_json::from_slice(&data).unwrap_or(json!({ "starred": [] })),
        Err(_) => json!({ "starred": [] }),
    };

    let group_id_str = group_uuid.to_string();
    
    // Modify starred array in place
    {
        let starred_files = prefs["starred"].as_array_mut().unwrap();
        if let Some(pos) = starred_files.iter().position(|x| x.as_str() == Some(&group_id_str)) {
            starred_files.remove(pos); // Unstar
        } else {
            starred_files.push(json!(group_id_str)); // Star
        }
    }

    let data = serde_json::to_vec(&prefs).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.storage.upload(&user_prefs_key, data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "starred": prefs["starred"] })))
}

