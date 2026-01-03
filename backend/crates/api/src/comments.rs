//! File Comments API Handlers
//!
//! Provides CRUD operations for file comments with proper access control.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::middleware::AuthUser;
use crate::AppState;
use crate::handlers::can_access_file;

// ==================== Models ====================

#[derive(Debug, Serialize, FromRow)]
pub struct FileComment {
    pub id: Uuid,
    pub file_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub parent_id: Option<Uuid>,
    pub is_edited: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentWithUser {
    pub id: Uuid,
    pub file_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_avatar: Option<String>,
    pub content: String,
    pub parent_id: Option<Uuid>,
    pub is_edited: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies: Vec<CommentWithUser>,
    pub can_edit: bool,
    pub can_delete: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentInput {
    pub content: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentInput {
    pub content: String,
}

// ==================== Handlers ====================

/// List all comments for a file
/// GET /api/files/{company_id}/{file_id}/comments
pub async fn list_comments(
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
    
    // Check if user can access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "read").await? {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Fetch all comments with user info
    let comments: Vec<(Uuid, Uuid, Uuid, String, Option<String>, String, Option<Uuid>, bool, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT 
            c.id, c.file_id, c.user_id, u.name as user_name, u.avatar_url,
            c.content, c.parent_id, c.is_edited, c.created_at, c.updated_at
        FROM file_comments c
        JOIN users u ON c.user_id = u.id
        WHERE c.file_id = $1 AND c.tenant_id = $2
        ORDER BY c.created_at ASC
        "#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch comments: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Build threaded structure
    let mut top_level: Vec<CommentWithUser> = Vec::new();
    let mut replies_map: std::collections::HashMap<Uuid, Vec<CommentWithUser>> = std::collections::HashMap::new();
    
    for (id, file_id, user_id, user_name, user_avatar, content, parent_id, is_edited, created_at, updated_at) in comments {
        let comment = CommentWithUser {
            id,
            file_id,
            user_id,
            user_name,
            user_avatar,
            content,
            parent_id,
            is_edited,
            created_at,
            updated_at,
            replies: Vec::new(),
            can_edit: user_id == auth.user_id,
            can_delete: user_id == auth.user_id || auth.role == "Admin" || auth.role == "SuperAdmin",
        };
        
        if let Some(pid) = parent_id {
            replies_map.entry(pid).or_default().push(comment);
        } else {
            top_level.push(comment);
        }
    }
    
    // Attach replies to parent comments
    for comment in &mut top_level {
        if let Some(replies) = replies_map.remove(&comment.id) {
            comment.replies = replies;
        }
    }
    
    Ok(Json(json!({
        "comments": top_level,
        "total": top_level.len()
    })))
}

/// Add a comment to a file
/// POST /api/files/{company_id}/{file_id}/comments
pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Parse input
    let content = input["content"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
    let parent_id = input["parent_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
    
    if content.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Check if user can access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "read").await? {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Verify parent comment exists if provided
    if let Some(pid) = parent_id {
        let parent_exists: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM file_comments WHERE id = $1 AND file_id = $2"
        )
        .bind(pid)
        .bind(file_uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if parent_exists.is_none() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    
    // Get file info for notification
    let file_info: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT name, owner_id FROM files_metadata WHERE id = $1"
    )
    .bind(file_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_name, file_owner_id) = file_info.ok_or(StatusCode::NOT_FOUND)?;
    
    // Create the comment
    let comment_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO file_comments (file_id, tenant_id, user_id, content, parent_id)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(content)
    .bind(parent_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create comment: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Audit log
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'comment_added', 'file', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(file_uuid)
    .bind(json!({
        "comment_id": comment_id,
        "file_name": file_name,
        "is_reply": parent_id.is_some()
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;
    
    // Send Discord notification to file owner (if not commenting on own file)
    if file_owner_id != auth.user_id {
        let pool_clone = state.pool.clone();
        let commenter_name = auth.email.split('@').next().unwrap_or("Someone").to_string();
        let content_preview = if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.to_string()
        };
        
        tokio::spawn(async move {
            crate::discord::notify_comment(
                &pool_clone,
                tenant_id,
                file_owner_id,
                &file_name,
                &commenter_name,
                &content_preview,
            ).await;
        });
    }
    
    // Get user info for response
    let user_name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or_else(|_| "Unknown".to_string());
    
    Ok(Json(json!({
        "id": comment_id,
        "file_id": file_uuid,
        "user_id": auth.user_id,
        "user_name": user_name,
        "content": content,
        "parent_id": parent_id,
        "is_edited": false,
        "created_at": Utc::now(),
        "can_edit": true,
        "can_delete": true
    })))
}

/// Update a comment
/// PUT /api/files/{company_id}/{file_id}/comments/{comment_id}
pub async fn update_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id, comment_id)): Path<(String, String, String)>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let comment_uuid = Uuid::parse_str(&comment_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let content = input["content"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
    
    if content.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Check if comment exists and user owns it
    let comment: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM file_comments WHERE id = $1 AND file_id = $2 AND tenant_id = $3"
    )
    .bind(comment_uuid)
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (owner_id,) = comment.ok_or(StatusCode::NOT_FOUND)?;
    
    // Only comment owner can edit
    if owner_id != auth.user_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Update the comment
    sqlx::query(
        r#"
        UPDATE file_comments
        SET content = $1, is_edited = true, updated_at = NOW()
        WHERE id = $2
        "#
    )
    .bind(content)
    .bind(comment_uuid)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "success": true,
        "id": comment_uuid,
        "content": content,
        "is_edited": true
    })))
}

/// Delete a comment
/// DELETE /api/files/{company_id}/{file_id}/comments/{comment_id}
pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id, comment_id)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let comment_uuid = Uuid::parse_str(&comment_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Check if comment exists and get owner
    let comment: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM file_comments WHERE id = $1 AND file_id = $2 AND tenant_id = $3"
    )
    .bind(comment_uuid)
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (owner_id,) = comment.ok_or(StatusCode::NOT_FOUND)?;
    
    // Only comment owner or admins can delete
    if owner_id != auth.user_id && auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Delete the comment (cascades to replies)
    sqlx::query("DELETE FROM file_comments WHERE id = $1")
        .bind(comment_uuid)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({ "success": true })))
}

/// Get comment count for a file
/// GET /api/files/{company_id}/{file_id}/comments/count
pub async fn get_comment_count(
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
    
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM file_comments WHERE file_id = $1 AND tenant_id = $2"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({ "count": count })))
}

