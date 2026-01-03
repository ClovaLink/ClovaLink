use axum::{
    extract::{Path, Query, State, Multipart},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::{Utc, Duration};
use tokio::io::AsyncWriteExt;
use clovalink_auth::AuthUser;
use clovalink_core::models::{CreateFileRequestInput, FileRequest, FileRequestUpload, Tenant};
use clovalink_core::notification_service;
use clovalink_core::security_service;
use std::sync::Arc;
use crate::AppState;
use crate::compliance::{ComplianceRestrictions, get_tenant_compliance_mode, check_compliance_action, ComplianceAction};


#[derive(Deserialize)]
pub struct FileRequestFilters {
    pub status: Option<String>,
    pub created_after: Option<String>,
    pub created_before: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub visibility: Option<String>,      // 'department' (default) or 'private'
    pub department_id: Option<String>,   // Optional department filter (for admins)
}


/// Create a new file request
/// POST /api/file-requests
pub async fn create_file_request(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    body: String,
) -> Result<Json<Value>, StatusCode> {
    tracing::debug!("Received file request body: {}", body);
    
    let input: CreateFileRequestInput = serde_json::from_str(&body)
        .map_err(|e| {
            tracing::error!("Failed to parse file request JSON: {:?}", e);
            tracing::error!("Raw body was: {}", body);
            StatusCode::UNPROCESSABLE_ENTITY
        })?;
    
    // Check compliance restrictions for public sharing
    let compliance_mode = get_tenant_compliance_mode(&state.pool, auth.tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);
    
    // Block public sharing if compliance mode restricts it
    if restrictions.public_sharing_blocked {
        return Err(StatusCode::FORBIDDEN);
    }

    // Also check using the compliance action checker for more detailed error handling
    if let Err(violation) = check_compliance_action(&state.pool, auth.tenant_id, ComplianceAction::PublicShare).await {
        tracing::warn!("Compliance violation: {:?}", violation);
        return Err(violation.to_status_code());
    }

    let token = nanoid::nanoid!(16);
    let expires_at = Utc::now() + Duration::days(input.expires_in_days);
    
    // Validate and set visibility (default to 'department')
    let visibility = input.visibility.as_deref().unwrap_or("department");
    let visibility = if visibility == "private" { "private" } else { "department" };

    let request = sqlx::query_as::<_, FileRequest>(
        r#"
        INSERT INTO file_requests (tenant_id, department_id, name, destination_path, token, created_by, expires_at, max_uploads, visibility)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#
    )
    .bind(auth.tenant_id)
    .bind(input.department_id)
    .bind(&input.name)
    .bind(&input.destination_path)
    .bind(&token)
    .bind(auth.user_id)
    .bind(expires_at)
    .bind(input.max_uploads)
    .bind(visibility)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create file request: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Log creation of file request
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'file_request_created', 'file_request', $3, $4, $5::inet)
        "#
    )
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .bind(request.id)
    .bind(json!({
        "name": input.name,
        "destination_path": input.destination_path,
        "expires_at": expires_at,
        "visibility": visibility,
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    Ok(Json(json!({
        "id": request.id,
        "name": request.name,
        "destination": request.destination_path,
        "token": request.token,
        "link": format!("{}/upload/{}", base_url, request.token),
        "expires_at": request.expires_at,
        "status": request.status,
        "upload_count": request.upload_count,
        "max_uploads": request.max_uploads,
        "visibility": request.visibility,
        "created_at": request.created_at,
    })))
}

/// List file requests with optional filters
/// GET /api/file-requests
pub async fn list_file_requests(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(filters): Query<FileRequestFilters>,
) -> Result<Json<Value>, StatusCode> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    // Get user's department and role from database
    let user: Option<(Option<Uuid>, String)> = sqlx::query_as(
        "SELECT department_id, role FROM users WHERE id = $1 AND tenant_id = $2"
    )
    .bind(auth.user_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    let user_department_id = user.as_ref().and_then(|u| u.0);
    let role = user.as_ref().map(|u| u.1.clone()).unwrap_or_else(|| auth.role.clone());

    // Build query with visibility filtering
    let mut query = String::from(
        "SELECT * FROM file_requests WHERE tenant_id = $1"
    );
    
    // Visibility filter based on requested view mode
    let view_mode = filters.visibility.as_deref().unwrap_or("department");
    
    if view_mode == "private" {
        // Private view: only show requests created by the current user
        query.push_str(&format!(" AND visibility = 'private' AND created_by = '{}'", auth.user_id));
    } else {
        // Department view: show department requests with role-based access
        query.push_str(" AND visibility = 'department'");
        
        if role == "SuperAdmin" || role == "Admin" {
            // Admins can see all department requests, optionally filtered by department
            if let Some(dept_id_str) = &filters.department_id {
                if !dept_id_str.is_empty() {
                    if let Ok(dept_uuid) = Uuid::parse_str(dept_id_str) {
                        query.push_str(&format!(" AND department_id = '{}'", dept_uuid));
                    }
                }
            }
        } else {
            // Manager/Employee: filter by user's department
            if let Some(dept_id) = user_department_id {
                query.push_str(&format!(" AND department_id = '{}'", dept_id));
            } else {
                // User has no department, show requests with no department
                query.push_str(" AND department_id IS NULL");
            }
        }
    }
    
    let mut param_count = 2;
    if filters.status.is_some() {
        query.push_str(&format!(" AND status = ${}", param_count));
        param_count += 1;
    }
    if filters.created_after.is_some() {
        query.push_str(&format!(" AND created_at >= ${}", param_count));
        param_count += 1;
    }
    if filters.created_before.is_some() {
        query.push_str(&format!(" AND created_at <= ${}", param_count));
        param_count += 1;
    }
    
    query.push_str(" ORDER BY created_at DESC");
    query.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));

    let mut db_query = sqlx::query_as::<_, FileRequest>(&query)
        .bind(auth.tenant_id);

    if let Some(status) = filters.status {
        db_query = db_query.bind(status);
    }
    if let Some(created_after) = filters.created_after {
        db_query = db_query.bind(created_after);
    }
    if let Some(created_before) = filters.created_before {
        db_query = db_query.bind(created_before);
    }

    let requests = db_query
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list file requests: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let results: Vec<Value> = requests.iter().map(|r| json!({
        "id": r.id,
        "name": r.name,
        "destination": r.destination_path,
        "token": r.token,
        "link": format!("{}/upload/{}", base_url, r.token),
        "expires_at": r.expires_at,
        "status": r.status,
        "upload_count": r.upload_count,
        "max_uploads": r.max_uploads,
        "visibility": r.visibility,
        "created_at": r.created_at,
    })).collect();

    Ok(Json(json!(results)))
}

/// Get single file request
/// GET /api/file-requests/:id
pub async fn get_file_request(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let request = sqlx::query_as::<_, FileRequest>(
        "SELECT * FROM file_requests WHERE id = $1 AND tenant_id = $2"
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Security check: enforce visibility rules
    let visibility = &request.visibility;
    
    if visibility == "private" {
        // Private requests: only the creator can access
        if request.created_by != auth.user_id {
            tracing::warn!(
                "User {} attempted to access private file request {} owned by {}",
                auth.user_id, request.id, request.created_by
            );
            return Err(StatusCode::FORBIDDEN);
        }
    } else {
        // Department visibility: check department membership or admin role
        if auth.role != "SuperAdmin" && auth.role != "Admin" {
            // Get user's department
            let user_dept: Option<(Option<Uuid>,)> = sqlx::query_as(
                "SELECT department_id FROM users WHERE id = $1 AND tenant_id = $2"
            )
            .bind(auth.user_id)
            .bind(auth.tenant_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
            
            let user_department_id = user_dept.and_then(|u| u.0);
            
            // If request has a department, user must be in that department
            if let Some(req_dept_id) = request.department_id {
                if user_department_id != Some(req_dept_id) {
                    tracing::warn!(
                        "User {} (dept {:?}) attempted to access file request {} in dept {}",
                        auth.user_id, user_department_id, request.id, req_dept_id
                    );
                    return Err(StatusCode::FORBIDDEN);
                }
            }
            // If request has no department, allow access (tenant-wide)
        }
    }

    // Get uploads for this request
    let uploads = sqlx::query_as::<_, FileRequestUpload>(
        "SELECT * FROM file_request_uploads WHERE file_request_id = $1 ORDER BY uploaded_at DESC"
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    Ok(Json(json!({
        "id": request.id,
        "name": request.name,
        "destination": request.destination_path,
        "token": request.token,
        "link": format!("{}/upload/{}", base_url, request.token),
        "expires_at": request.expires_at,
        "status": request.status,
        "upload_count": request.upload_count,
        "max_uploads": request.max_uploads,
        "visibility": request.visibility,
        "created_at": request.created_at,
        "uploads": uploads,
    })))
}

/// Revoke/Delete a file request
/// DELETE /api/file-requests/:id
pub async fn delete_file_request(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // First fetch the request to check permissions
    let request = sqlx::query_as::<_, FileRequest>(
        "SELECT * FROM file_requests WHERE id = $1 AND tenant_id = $2"
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Security check: enforce visibility/ownership rules for deletion
    let visibility = &request.visibility;
    
    // Admins can delete any request in their tenant
    if auth.role != "SuperAdmin" && auth.role != "Admin" {
        if visibility == "private" {
            // Private requests: only the creator can delete
            if request.created_by != auth.user_id {
                tracing::warn!(
                    "User {} attempted to delete private file request {} owned by {}",
                    auth.user_id, request.id, request.created_by
                );
                return Err(StatusCode::FORBIDDEN);
            }
        } else {
            // Department visibility: check department membership
            let user_dept: Option<(Option<Uuid>,)> = sqlx::query_as(
                "SELECT department_id FROM users WHERE id = $1 AND tenant_id = $2"
            )
            .bind(auth.user_id)
            .bind(auth.tenant_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
            
            let user_department_id = user_dept.and_then(|u| u.0);
            
            // If request has a department, user must be in that department
            if let Some(req_dept_id) = request.department_id {
                if user_department_id != Some(req_dept_id) {
                    tracing::warn!(
                        "User {} (dept {:?}) attempted to delete file request {} in dept {}",
                        auth.user_id, user_department_id, request.id, req_dept_id
                    );
                    return Err(StatusCode::FORBIDDEN);
                }
            }
        }
    }

    // Now perform the deletion
    sqlx::query!(
        "UPDATE file_requests SET status = 'revoked', updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        id,
        auth.tenant_id
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"success": true})))
}

/// Permanently delete a file request (removes from database)
/// DELETE /api/file-requests/:id/permanent
pub async fn permanent_delete_file_request(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Only allow SuperAdmin or Admin to permanently delete
    if auth.role != "SuperAdmin" && auth.role != "Admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    // First delete related uploads
    sqlx::query("DELETE FROM file_request_uploads WHERE file_request_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Then delete the file request
    let result = sqlx::query("DELETE FROM file_requests WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(auth.tenant_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(json!({"success": true, "permanently_deleted": true})))
}

/// Get uploads for a file request
/// GET /api/file-requests/:id/uploads
pub async fn get_file_request_uploads(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Verify the request belongs to this tenant
    let _ = sqlx::query!(
        "SELECT id FROM file_requests WHERE id = $1 AND tenant_id = $2",
        id,
        auth.tenant_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let uploads = sqlx::query_as::<_, FileRequestUpload>(
        "SELECT * FROM file_request_uploads WHERE file_request_id = $1 ORDER BY uploaded_at DESC"
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!(uploads)))
}

/// Public upload endpoint (no auth required)
/// Uses streaming upload with content-addressed storage (zero-copy, deduplication)
/// POST /api/public-upload/:token
pub async fn public_upload(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    // Find the file request by token
    let file_request = sqlx::query_as::<_, FileRequest>(
        "SELECT * FROM file_requests WHERE token = $1 AND status = 'active'"
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Check compliance mode - verify public sharing is still allowed
    let compliance_mode = get_tenant_compliance_mode(&state.pool, file_request.tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);
    
    // Block upload if compliance mode now restricts public sharing
    // (e.g., if mode was changed after the request was created)
    if restrictions.public_sharing_blocked {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if expired
    if file_request.expires_at < Utc::now() {
        return Err(StatusCode::GONE); // 410 Gone
    }

    // Check max uploads limit
    if let Some(max) = file_request.max_uploads {
        if file_request.upload_count >= max {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    
    // Get tenant upload limits for size validation
    let tenant_limits: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT storage_quota_bytes, max_upload_size_bytes FROM tenants WHERE id = $1"
    )
    .bind(file_request.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get blocked extensions for this tenant
    let blocked_extensions: Vec<String> = sqlx::query_scalar(
        "SELECT COALESCE(blocked_extensions, ARRAY[]::TEXT[]) FROM tenants WHERE id = $1"
    )
    .bind(file_request.tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or_default();

    let mut uploaded_files = vec![];

    while let Some(mut field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let file_name = field.file_name()
            .ok_or(StatusCode::BAD_REQUEST)?
            .to_string();
        
        // Check for blocked file extensions
        if !blocked_extensions.is_empty() {
            if let Some(ext) = std::path::Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
            {
                let ext_lower = ext.to_lowercase();
                if blocked_extensions.iter().any(|b| b.to_lowercase() == ext_lower) {
                    tracing::warn!(
                        "Public upload blocked: attempted to upload blocked extension .{} (file: {}, request: {})",
                        ext_lower, file_name, token
                    );
                    // Create security alert for blocked extension attempt
                    let _ = security_service::alert_blocked_extension(
                        &state.pool,
                        file_request.tenant_id,
                        None, // No authenticated user
                        None, // No user email
                        &file_name,
                        &ext_lower,
                        None, // Could extract from headers if needed
                        true, // Is public upload
                    ).await;
                    return Ok(Json(json!({
                        "error": "blocked_extension",
                        "message": format!("File type .{} is not allowed", ext_lower),
                        "extension": ext_lower
                    })));
                }
            }
        }
        
        let content_type = field.content_type()
            .map(|s| s.to_string());

        // === STREAMING UPLOAD: Stream to temp file while computing Blake3 hash ===
        let temp_dir = std::env::temp_dir();
        let temp_file_name = format!("clovalink_public_upload_{}_{}", Uuid::new_v4(), &file_name);
        let temp_path = temp_dir.join(&temp_file_name);
        
        let mut temp_file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create temp file for public upload: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        let mut size: i64 = 0;
        let mut hasher = blake3::Hasher::new();
        
        // Stream chunks to temp file while computing hash (constant memory usage)
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            tracing::error!("Failed to read chunk in public upload: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
            size += chunk.len() as i64;
            
            // Check max upload size limit during streaming
            if let Some((_, Some(max_size))) = tenant_limits {
                if size > max_size {
                    // Clean up temp file before returning error
                    drop(temp_file);
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    tracing::warn!("Public upload exceeded max size: {} > {}", size, max_size);
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
            }
            
            hasher.update(&chunk);
            temp_file.write_all(&chunk).await.map_err(|e| {
                tracing::error!("Failed to write chunk to temp file: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        
        // Finalize Blake3 hash
        let content_hash = hasher.finalize().to_hex().to_string();
        
        // Flush and close temp file
        temp_file.flush().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        drop(temp_file);

        // Generate unique filename for display (keeps original name readable)
        let unique_filename = format!("{}-{}", nanoid::nanoid!(8), file_name);
        
        // Content-addressed storage path: tenant_id/department_id/content_hash
        // This enables deduplication - same content stored once
        let department_id = file_request.department_id.unwrap_or(Uuid::nil());
        let storage_path = format!("{}/{}/{}", file_request.tenant_id, department_id, content_hash);

        // Check if content already exists in storage (deduplication)
        let content_exists = state.storage.exists(&storage_path).await.unwrap_or(false);
        
        if !content_exists {
            // Acquire transfer scheduler permit based on file size (prioritizes small files)
            let transfer_permit = state.scheduler.acquire_upload_permit(Some(size)).await;
            tracing::debug!(
                "Public upload permit acquired: token={}, size={}, class={}",
                token, size, transfer_permit.size_class.name()
            );
            
            // Upload from temp file (streaming, zero-copy)
            state.storage.upload_from_path(&storage_path, &temp_path).await
                .map_err(|e| {
                    tracing::error!("Storage error in public upload: {:?}", e);
                    // Clean up temp file on error
                    let _ = std::fs::remove_file(&temp_path);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            tracing::debug!("Uploaded new content to storage: {}", storage_path);
            
            // Permit is released here when upload completes
            drop(transfer_permit);
        } else {
            tracing::debug!("Content already exists, deduplicating: {}", storage_path);
        }
        
        // Clean up temp file after successful upload
        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!("Failed to remove temp file: {:?}", e);
        }
        
        // Enqueue S3 replication if enabled (only for new content, not deduplicated)
        if state.replication_config.enabled && !content_exists {
            let replication_pool = state.pool.clone();
            let storage_key = storage_path.clone();
            let tenant_id = file_request.tenant_id;
            tokio::spawn(async move {
                if let Err(e) = clovalink_core::replication::enqueue_upload(
                    &replication_pool,
                    &storage_key,
                    tenant_id,
                    Some(size),
                ).await {
                    tracing::warn!(
                        target: "replication",
                        storage_path = %storage_key,
                        error = %e,
                        "Failed to enqueue replication job for public upload"
                    );
                }
            });
        }

        // 1. Create FileMetadata entry so it shows up in the file manager
        // Include content_hash for deduplication tracking
        let file_metadata = sqlx::query_as::<_, clovalink_core::models::FileMetadata>(
            r#"
            INSERT INTO files_metadata (tenant_id, department_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, content_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(file_request.tenant_id)
        .bind(file_request.department_id)
        .bind(&unique_filename)
        .bind(&storage_path)
        .bind(size)
        .bind(&content_type)
        .bind(false) // is_directory
        .bind(file_request.created_by) // Owner is the request creator
        .bind(&content_hash)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create file metadata: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Enqueue virus scan job if enabled (non-blocking)
        if state.virus_scan_config.enabled {
            let scan_pool = state.pool.clone();
            let file_id = file_metadata.id;
            let tenant_id = file_request.tenant_id;
            let max_queue_size = state.virus_scan_config.max_queue_size;
            tokio::spawn(async move {
                if let Err(e) = clovalink_core::virus_scan::enqueue_scan_with_backpressure(
                    &scan_pool,
                    file_id,
                    tenant_id,
                    0, // Normal priority
                    max_queue_size,
                ).await {
                    tracing::warn!(
                        target: "virus_scan",
                        file_id = %file_id,
                        error = %e,
                        "Failed to enqueue virus scan job for public upload"
                    );
                }
            });
        }

        // 2. Save upload record linked to metadata
        let upload = sqlx::query_as::<_, FileRequestUpload>(
            r#"
            INSERT INTO file_request_uploads (file_request_id, file_metadata_id, filename, original_filename, size_bytes, content_type, storage_path)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#
        )
        .bind(file_request.id)
        .bind(file_metadata.id)
        .bind(&unique_filename)
        .bind(&file_name)
        .bind(size)
        .bind(&content_type)
        .bind(&storage_path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create upload record: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        uploaded_files.push(upload);
    }

    // Update upload count
    sqlx::query!(
        "UPDATE file_requests SET upload_count = upload_count + $1, updated_at = NOW() WHERE id = $2",
        uploaded_files.len() as i32,
        file_request.id
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Send notification to the file request owner
    if !uploaded_files.is_empty() {
        // Get request owner details
        let owner: Option<(String, String)> = sqlx::query_as(
            "SELECT email, role FROM users WHERE id = $1"
        )
        .bind(file_request.created_by)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();

        if let Some((owner_email, owner_role)) = owner {
            // Get tenant
            if let Ok(tenant) = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
                .bind(file_request.tenant_id)
                .fetch_one(&state.pool)
                .await
            {
                // Notify about the first uploaded file (or summarize if multiple)
                let first_file = &uploaded_files[0];
                let uploader_name = "External user"; // Public uploads don't have a known uploader
                
                if let Some(file_id) = first_file.file_metadata_id {
                    let _ = notification_service::notify_file_upload(
                        &state.pool,
                        &tenant,
                        file_request.created_by,
                        &owner_email,
                        &owner_role,
                        &file_request.name,
                        uploader_name,
                        &first_file.original_filename,
                        file_id,
                        file_request.id,
                    ).await;
                    
                    // Also send Discord DM notification (fire-and-forget)
                    let pool_clone = state.pool.clone();
                    let tenant_id = file_request.tenant_id;
                    let owner_id = file_request.created_by;
                    let file_name = first_file.original_filename.clone();
                    let request_name = file_request.name.clone();
                    tokio::spawn(async move {
                        crate::discord::notify_file_upload(
                            &pool_clone,
                            tenant_id,
                            owner_id,
                            &file_name,
                            "External user",
                            &request_name,
                        ).await;
                    });
                }
            }
        }
    }

    Ok(Json(json!({
        "success": true,
        "uploaded": uploaded_files.len(),
        "files": uploaded_files,
    })))
}
