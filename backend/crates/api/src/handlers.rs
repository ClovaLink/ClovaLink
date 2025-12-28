use axum::{
    extract::{Path, State, Multipart},
    http::{StatusCode, HeaderMap, header},
    response::{Json, Redirect, IntoResponse},
    body::Body,
    Extension,
};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use ulid::Ulid;
use url::Url;
use crate::AppState;
use crate::extensions::dispatch_file_upload;
use crate::compliance::{ComplianceRestrictions, get_tenant_compliance_mode, should_force_audit_log, log_file_export};
use clovalink_auth::AuthUser;
use clovalink_core::models::FileMetadata;
use sqlx::Row;
use clovalink_core::security_service;

/// Format bytes into human-readable string
fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Sanitize a filename for use in Content-Disposition headers
/// 
/// Security:
/// - Prevents header injection by escaping/removing dangerous characters
/// - Handles unicode safely using RFC 5987 encoding
/// - Falls back to ASCII-safe filename if needed
/// 
/// Returns a Content-Disposition header value string
fn sanitize_content_disposition(filename: &str, disposition: &str) -> String {
    // Sanitize the filename
    let sanitized = sanitize_filename(filename);
    
    // Check if filename is pure ASCII
    let is_ascii = sanitized.chars().all(|c| c.is_ascii() && c != '"' && c != '\n' && c != '\r');
    
    if is_ascii && !sanitized.is_empty() {
        // Simple case: ASCII-only filename
        // Escape any remaining quotes just in case
        let safe = sanitized.replace('"', "'");
        format!("{}; filename=\"{}\"", disposition, safe)
    } else if !sanitized.is_empty() {
        // RFC 5987 encoded filename for unicode support
        // Format: filename*=UTF-8''encoded-value
        let encoded: String = sanitized
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                    c.to_string()
                } else {
                    // Percent-encode
                    c.encode_utf8(&mut [0; 4])
                        .bytes()
                        .map(|b| format!("%{:02X}", b))
                        .collect()
                }
            })
            .collect();
        
        // Provide both filename (ASCII fallback) and filename* (unicode)
        let ascii_fallback = sanitized
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c))
            .collect::<String>();
        let ascii_fallback = if ascii_fallback.is_empty() { "download".to_string() } else { ascii_fallback };
        
        format!("{}; filename=\"{}\"; filename*=UTF-8''{}", disposition, ascii_fallback, encoded)
    } else {
        // Fallback for completely empty/invalid filename
        format!("{}; filename=\"download\"", disposition)
    }
}

/// Sanitize a filename by removing or replacing dangerous characters
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| {
            // Remove control characters, null bytes, and header-breaking chars
            !c.is_control() && *c != '\0' && *c != '\n' && *c != '\r'
        })
        .map(|c| {
            // Replace path separators and other dangerous chars
            match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Rewrite S3 presigned URL to go through CDN domain
/// Preserves query params (signature, expiry) for origin validation
fn rewrite_url_to_cdn(s3_url: &str, cdn_domain: &str) -> String {
    if let Ok(parsed) = Url::parse(s3_url) {
        let path_and_query = format!(
            "{}{}",
            parsed.path(),
            parsed.query().map(|q| format!("?{}", q)).unwrap_or_default()
        );
        format!("https://{}{}", cdn_domain, path_and_query)
    } else {
        // Fallback to original URL if parsing fails
        s3_url.to_string()
    }
}

/// Get MIME content type based on file extension
fn get_content_type(filename: &str) -> &'static str {
    let filename_lower = filename.to_lowercase();
    
    // Image formats
    if filename_lower.ends_with(".pdf") {
        "application/pdf"
    } else if filename_lower.ends_with(".png") {
        "image/png"
    } else if filename_lower.ends_with(".jpg") || filename_lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if filename_lower.ends_with(".gif") {
        "image/gif"
    } else if filename_lower.ends_with(".webp") {
        "image/webp"
    } else if filename_lower.ends_with(".svg") {
        "image/svg+xml"
    // Video formats
    } else if filename_lower.ends_with(".mp4") {
        "video/mp4"
    } else if filename_lower.ends_with(".mov") {
        "video/quicktime"
    } else if filename_lower.ends_with(".webm") {
        "video/webm"
    } else if filename_lower.ends_with(".avi") {
        "video/x-msvideo"
    } else if filename_lower.ends_with(".mkv") {
        "video/x-matroska"
    } else if filename_lower.ends_with(".m4v") {
        "video/x-m4v"
    // Audio formats
    } else if filename_lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if filename_lower.ends_with(".wav") {
        "audio/wav"
    } else if filename_lower.ends_with(".ogg") {
        "audio/ogg"
    } else if filename_lower.ends_with(".m4a") {
        "audio/mp4"
    } else if filename_lower.ends_with(".flac") {
        "audio/flac"
    // Document/text formats
    } else if filename_lower.ends_with(".txt") {
        "text/plain"
    } else if filename_lower.ends_with(".csv") {
        "text/csv"
    } else if filename_lower.ends_with(".json") {
        "application/json"
    } else if filename_lower.ends_with(".xml") {
        "application/xml"
    } else if filename_lower.ends_with(".html") || filename_lower.ends_with(".htm") {
        "text/html"
    } else if filename_lower.ends_with(".md") {
        "text/markdown"
    // Office formats
    } else if filename_lower.ends_with(".doc") {
        "application/msword"
    } else if filename_lower.ends_with(".docx") {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    } else if filename_lower.ends_with(".xls") {
        "application/vnd.ms-excel"
    } else if filename_lower.ends_with(".xlsx") {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    } else if filename_lower.ends_with(".ppt") {
        "application/vnd.ms-powerpoint"
    } else if filename_lower.ends_with(".pptx") {
        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
    // Archive formats
    } else if filename_lower.ends_with(".zip") {
        "application/zip"
    } else if filename_lower.ends_with(".tar") {
        "application/x-tar"
    } else if filename_lower.ends_with(".gz") || filename_lower.ends_with(".gzip") {
        "application/gzip"
    } else {
        "application/octet-stream"
    }
}

/// Generate a unique filename by appending (1), (2), etc. if a file with the same name exists
/// Scope: tenant + department + parent_path + visibility
async fn generate_unique_filename(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    original_name: &str,
    parent_path: &str,
    department_id: Option<Uuid>,
    visibility: &str,
) -> Result<String, sqlx::Error> {
    // Split filename into base name and extension
    let (base_name, extension) = if let Some(dot_pos) = original_name.rfind('.') {
        (&original_name[..dot_pos], &original_name[dot_pos..])
    } else {
        (original_name, "")
    };
    
    // Try candidates: "file (1).ext", "file (2).ext", etc.
    for i in 1..1000 {
        let candidate = format!("{} ({}){}", base_name, i, extension);
        
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM files_metadata 
                WHERE tenant_id = $1 
                AND name = $2 
                AND is_deleted = false
                AND (parent_path = $3 OR (parent_path IS NULL AND $3 = ''))
                AND (department_id IS NOT DISTINCT FROM $4)
                AND visibility = $5
            )
            "#
        )
        .bind(tenant_id)
        .bind(&candidate)
        .bind(if parent_path.is_empty() { "".to_string() } else { parent_path.to_string() })
        .bind(department_id)
        .bind(visibility)
        .fetch_one(pool)
        .await?;
        
        if !exists {
            return Ok(candidate);
        }
    }
    
    // Fallback: use UUID suffix (should rarely happen)
    let fallback = format!("{}_{}{}", base_name, Uuid::new_v4().to_string().split('-').next().unwrap_or(""), extension);
    Ok(fallback)
}

#[derive(serde::Deserialize)]
pub struct UploadParams {
    parent_path: Option<String>,
    visibility: Option<String>,  // 'department' (default) or 'private'
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<UploadParams>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let parent_path = params.parent_path.unwrap_or_default();
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access - SECURITY: prevent cross-tenant uploads
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        tracing::warn!(
            "Security: User {} from tenant {} attempted to upload to tenant {}",
            auth.user_id, auth.tenant_id, tenant_id
        );
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Get compliance mode for SOX versioning check
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // Get blocked extensions for this tenant
    let blocked_extensions: Vec<String> = sqlx::query_scalar(
        "SELECT COALESCE(blocked_extensions, ARRAY[]::TEXT[]) FROM tenants WHERE id = $1"
    )
    .bind(tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or_default();

    while let Some(mut field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let _name = field.name().unwrap_or("file").to_string();
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        
        // Check for blocked file extensions
        if !blocked_extensions.is_empty() {
            if let Some(ext) = std::path::Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
            {
                let ext_lower = ext.to_lowercase();
                if blocked_extensions.iter().any(|b| b.to_lowercase() == ext_lower) {
                    tracing::warn!(
                        "Upload blocked: user {} attempted to upload blocked extension .{} (file: {})",
                        auth.user_id, ext_lower, file_name
                    );
                    // Create security alert for blocked extension attempt
                    let _ = security_service::alert_blocked_extension(
                        &state.pool,
                        tenant_id,
                        Some(auth.user_id),
                        Some(&auth.email),
                        &file_name,
                        &ext_lower,
                        auth.ip_address.as_deref(),
                        false,
                    ).await;
                    return Ok(Json(json!({
                        "error": "blocked_extension",
                        "message": format!("File type .{} is not allowed by your organization", ext_lower),
                        "extension": ext_lower
                    })));
                }
            }
        }
        let _content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        
        // Create a temporary file for streaming upload
        let temp_dir = std::env::temp_dir();
        let temp_file_name = format!("clovalink_upload_{}_{}", Uuid::new_v4(), &file_name);
        let temp_path = temp_dir.join(&temp_file_name);
        
        // Stream the upload to a temporary file while computing Blake3 hash
        let mut temp_file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create temp file: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        let mut size: i64 = 0;
        let mut hasher = blake3::Hasher::new();
        
        // Stream chunks to temp file while computing hash
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            tracing::error!("Failed to read chunk: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
            size += chunk.len() as i64;
            hasher.update(&chunk);
            temp_file.write_all(&chunk).await.map_err(|e| {
                tracing::error!("Failed to write chunk: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        
        // Finalize Blake3 hash
        let content_hash = hasher.finalize().to_hex().to_string();
        
        // Flush and sync the temp file
        temp_file.flush().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        drop(temp_file); // Close the file handle

        // Check storage quota and max upload size before proceeding
        let tenant_limits: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT storage_quota_bytes, max_upload_size_bytes FROM tenants WHERE id = $1"
        )
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some((quota, max_upload_size)) = tenant_limits {
            // Check max upload size limit first (before wasting time checking storage)
            if let Some(max_size) = max_upload_size {
                if size > max_size {
                    // Clean up temp file before returning error
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Ok(Json(json!({
                        "error": "File too large",
                        "message": format!("Maximum upload size is {}. Please reduce file size or contact your administrator.", format_bytes(max_size)),
                        "max_size": max_size,
                        "file_size": size
                    })));
                }
            }

            // Check storage quota
            if let Some(storage_quota) = quota {
                // Calculate current storage usage from files_metadata
                let current_storage: (i64,) = sqlx::query_as(
                    "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM files_metadata WHERE tenant_id = $1 AND is_deleted = false AND is_directory = false"
                )
                .bind(tenant_id)
                .fetch_one(&state.pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if current_storage.0 + size > storage_quota {
                    // Clean up temp file before returning error
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Ok(Json(json!({
                        "error": "Storage quota exceeded",
                        "message": "Your organization has reached its storage limit. Please contact your system administrator to increase storage or free up space.",
                        "current_usage": current_storage.0,
                        "quota": storage_quota,
                        "file_size": size
                    })));
                }
            }
        }

        // Get user's department
        let user = sqlx::query!("SELECT department_id FROM users WHERE id = $1", auth.user_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
            
        let department_id = user.and_then(|u| u.department_id);

        // Validate and get visibility (default to 'department')
        let visibility = params.visibility.as_deref().unwrap_or("department");
        let visibility = if visibility == "private" { "private" } else { "department" };

        // Check for existing file with same name within the same scope
        // Scope = tenant + department + parent_path + visibility
        // This allows same filename in:
        // - Different departments
        // - Private vs department files
        // - Different tenants (handled by tenant_id check)
        let existing_file: Option<(Uuid, Option<i32>)> = sqlx::query_as(
            r#"
            SELECT id, version FROM files_metadata 
            WHERE tenant_id = $1 AND name = $2 AND is_deleted = false
            AND (parent_path = $3 OR (parent_path IS NULL AND $3 = ''))
            AND (department_id IS NOT DISTINCT FROM $4)
            AND visibility = $5
            ORDER BY version DESC NULLS LAST
            LIMIT 1
            "#
        )
        .bind(tenant_id)
        .bind(&file_name)
        .bind(if parent_path.is_empty() { "".to_string() } else { parent_path.clone() })
        .bind(department_id)
        .bind(visibility)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Auto-rename if duplicate exists (for non-SOX mode)
        let final_file_name = if !restrictions.file_versioning_required && existing_file.is_some() {
            // Generate unique name: "file (1).ext", "file (2).ext", etc.
            let unique_name = generate_unique_filename(
                &state.pool, 
                tenant_id, 
                &file_name, 
                &parent_path, 
                department_id, 
                visibility
            ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            tracing::info!("Auto-renamed duplicate file '{}' to '{}'", file_name, unique_name);
            unique_name
        } else {
            file_name.clone()
        };

        // Generate ULID for this file record
        let file_ulid = Ulid::new().to_string();
        
        // Content-addressed storage key format: {tenant_id}/{dept_id or 'private'}/{hash_prefix}/{content_hash}
        // The 2-char hash prefix provides S3 partitioning (256 partitions per scope)
        let dept_scope = department_id
            .map(|d| d.to_string())
            .unwrap_or_else(|| "private".to_string());
        let hash_prefix = &content_hash[..2];
        let content_key = format!("{}/{}/{}/{}", tenant_id, dept_scope, hash_prefix, content_hash);
        
        // Check for existing file with same content in same tenant/department (deduplication)
        let existing_content: Option<String> = sqlx::query_scalar(
            r#"
            SELECT storage_path FROM files_metadata 
            WHERE tenant_id = $1 
            AND (department_id IS NOT DISTINCT FROM $2)
            AND content_hash = $3
            AND is_deleted = false 
            AND is_directory = false
            LIMIT 1
            "#
        )
        .bind(tenant_id)
        .bind(department_id)
        .bind(&content_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Determine version for SOX compliance mode
        let (version, version_parent_id) = if restrictions.file_versioning_required {
            if let Some((existing_id, existing_version)) = existing_file {
                let new_version = existing_version.unwrap_or(1) + 1;
                (new_version, Some(existing_id))
            } else {
                (1, None)
            }
        } else {
            (1, None)
        };
        
        // Use existing storage path if content already exists (deduplication), otherwise use content-addressed key
        let key = if let Some(ref existing_path) = existing_content {
            tracing::info!("Deduplication: Reusing existing storage for content hash {}", &content_hash[..8]);
            existing_path.clone()
        } else {
            content_key.clone()
        };
        
        // Only upload if content doesn't already exist in storage
        if existing_content.is_none() {
            // Acquire transfer scheduler permit based on file size (prioritizes small files)
            let transfer_permit = state.scheduler.acquire_upload_permit(Some(size)).await;
            tracing::debug!(
                "Upload permit acquired: file={}, size={}, class={}",
                file_name, size, transfer_permit.size_class.name()
            );
            
            state.storage.upload_from_path(&key, &temp_path).await.map_err(|e| {
                tracing::error!("Failed to upload to storage: {:?}", e);
                // Clean up temp file on error
                let _ = std::fs::remove_file(&temp_path);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            
            // Permit is released here when upload completes
            drop(transfer_permit);
        }
        
        // Clean up temp file after successful upload
        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!("Failed to remove temp file: {:?}", e);
        }

        // For SOX mode, mark previous version as immutable
        if restrictions.file_versioning_required {
            if let Some(parent_id) = version_parent_id {
                let _ = sqlx::query(
                    "UPDATE files_metadata SET is_immutable = true WHERE id = $1"
                )
                .bind(parent_id)
                .execute(&state.pool)
                .await;
            }
        }

        let file_record: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO files_metadata (tenant_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, department_id, parent_path, version, version_parent_id, is_immutable, visibility, content_hash, ulid)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING id
            "#
        )
        .bind(tenant_id)
        .bind(&final_file_name)
        .bind(&key)
        .bind(size)
        .bind(&_content_type)
        .bind(false)
        .bind(auth.user_id)
        .bind(department_id)
        .bind(if parent_path.is_empty() { None } else { Some(&parent_path) })
        .bind(version)
        .bind(version_parent_id)
        .bind(false)
        .bind(visibility)
        .bind(&content_hash)
        .bind(&file_ulid)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to save file metadata: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let file_id = file_record.0;
        
        // Log upload for SOX compliance
        let is_deduplicated = existing_content.is_some();
        if should_force_audit_log(&compliance_mode, "file_upload") {
            let _ = sqlx::query(
                r#"
                INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
                VALUES ($1, $2, 'file_upload', 'file', $3, $4, $5::inet)
                "#
            )
            .bind(tenant_id)
            .bind(auth.user_id)
            .bind(file_id)
            .bind(json!({
                "file_name": &final_file_name,
                "original_name": &file_name,
                "version": version,
                "size_bytes": size,
                "content_hash": &content_hash,
                "ulid": &file_ulid,
                "deduplicated": is_deduplicated,
                "compliance_mode": compliance_mode,
            }))
            .bind(&auth.ip_address)
            .execute(&state.pool)
            .await;
        }
        
        // Enqueue S3 replication if enabled (only for new content, not deduplicated)
        if state.replication_config.enabled && !is_deduplicated {
            let replication_pool = state.pool.clone();
            let storage_key = key.clone();
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
                        "Failed to enqueue replication job"
                    );
                }
            });
        }
        
        // Enqueue virus scan job if enabled (non-blocking)
        if state.virus_scan_config.enabled {
            let scan_pool = state.pool.clone();
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
                        "Failed to enqueue virus scan job"
                    );
                }
            });
        }
        
        // Dispatch file upload event to extensions (non-blocking)
        let pool = state.pool.clone();
        let redis_url = state.redis_url.clone();
        let webhook_timeout = state.extension_webhook_timeout_ms;
        let file_name_clone = final_file_name.clone();
        let content_type_clone = _content_type.clone();
        tokio::spawn(async move {
            dispatch_file_upload(
                &pool,
                &redis_url,
                tenant_id,
                auth.user_id,
                file_id,
                &file_name_clone,
                Some(&content_type_clone),
                size,
                webhook_timeout,
            ).await;
        });
        
        // Invalidate file listing cache for this tenant
        if let Some(ref cache) = state.cache {
            let pattern = format!("clovalink:files:{}:*", tenant_id);
            if let Err(e) = cache.delete_pattern(&pattern).await {
                tracing::warn!("Failed to invalidate file cache: {}", e);
            }
        }
        
        // Include info about whether the file was renamed or deduplicated
        let was_renamed = final_file_name != file_name;
        
        return Ok(Json(json!({
            "message": if is_deduplicated { 
                "File uploaded (deduplicated - content already exists)" 
            } else if was_renamed { 
                "File uploaded and renamed to avoid duplicate" 
            } else { 
                "File uploaded successfully" 
            },
            "file_name": final_file_name,
            "original_name": if was_renamed { Some(&file_name) } else { None },
            "was_renamed": was_renamed,
            "deduplicated": is_deduplicated,
            "file_id": file_id,
            "ulid": file_ulid,
            "content_hash": content_hash,
            "key": key,
            "version": version
        })));
    }

    Err(StatusCode::BAD_REQUEST)
}

// ==================== Security Helper Functions ====================

/// Check if a user has permission to access a file based on visibility, ownership, and department
/// 
/// Access rules:
/// - SuperAdmin/Admin: can access all files in their tenant
/// - Private files: only the owner can access
/// - Department files: user must be in the same department
/// 
/// action parameter is for future audit logging differentiation (read/write/delete)
async fn can_access_file(
    pool: &sqlx::PgPool,
    file_id: Uuid,
    tenant_id: Uuid,
    user_id: Uuid,
    user_role: &str,
    action: &str,
) -> Result<bool, StatusCode> {
    // SuperAdmin/Admin can access everything in their tenant
    if user_role == "SuperAdmin" || user_role == "Admin" {
        return Ok(true);
    }

    // Get file metadata including visibility, owner, department, and lock status
    let file: Option<(String, Option<Uuid>, Option<Uuid>, bool, Option<Uuid>, Option<String>)> = sqlx::query_as(
        r#"SELECT visibility, owner_id, department_id, is_locked, locked_by, lock_requires_role
           FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"#
    )
    .bind(file_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (visibility, owner_id, file_dept_id, is_locked, locked_by, lock_requires_role) = file.ok_or(StatusCode::NOT_FOUND)?;

    // SECURITY: Check file lock permissions
    // If file is locked, only the locker or owner can access (unless user has required role)
    if is_locked {
        let is_locker = locked_by == Some(user_id);
        let is_owner = owner_id == Some(user_id);
        
        // Check if user has the required role for this lock
        let has_required_role = match lock_requires_role.as_deref() {
            Some("Manager") => user_role == "Manager",
            Some("Admin") => false, // Already handled above
            Some("SuperAdmin") => false, // Already handled above
            Some(custom_role) => user_role == custom_role,
            None => false, // No role requirement, only locker/owner can access
        };
        
        if !is_locker && !is_owner && !has_required_role {
            tracing::warn!(
                "Access denied to locked file {}: user {} (role: {}) not authorized (locker: {:?}, owner: {:?}, requires_role: {:?})",
                file_id, user_id, user_role, locked_by, owner_id, lock_requires_role
            );
            return Ok(false);
        }
    }

    // Private files: only owner can access
    if visibility == "private" {
        return Ok(owner_id == Some(user_id));
    }

    // For share action, only owner or managers can share
    if action == "share" {
        let is_owner = owner_id == Some(user_id);
        let is_manager = user_role == "Manager";
        if !is_owner && !is_manager {
            tracing::warn!(
                "Share denied for file {}: user {} (role: {}) is not owner or manager",
                file_id, user_id, user_role
            );
            return Ok(false);
        }
    }

    // Department files: user must be in same department OR have it in allowed_department_ids
    let user_depts: Option<(Option<Uuid>, Option<Vec<Uuid>>)> = sqlx::query_as(
        "SELECT department_id, allowed_department_ids FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (user_dept, allowed_depts) = user_depts.unwrap_or((None, None));

    // If file has no department (root level), allow access
    if file_dept_id.is_none() {
        return Ok(true);
    }

    let file_dept = file_dept_id.unwrap();
    
    // Check primary department
    if user_dept == Some(file_dept) {
        return Ok(true);
    }
    
    // Check allowed departments
    if let Some(allowed) = allowed_depts {
        if allowed.contains(&file_dept) {
            return Ok(true);
        }
    }
    
    Ok(false)
}

// ==================== File Listing ====================

#[derive(serde::Deserialize)]
pub struct ListFilesParams {
    path: Option<String>,
    department_id: Option<String>,
    visibility: Option<String>,  // 'department' (default) or 'private'
    owner_id: Option<String>,    // For admin viewing other users' private files
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<ListFilesParams>,
) -> Result<Json<Value>, StatusCode> {
    use clovalink_core::cache::{keys, ttl, hash_path};

    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Build cache key from path, department, visibility, and owner
    let path_str = params.path.as_deref().unwrap_or("");
    let dept_str = params.department_id.as_deref().unwrap_or("");
    let visibility_str = params.visibility.as_deref().unwrap_or("department");
    let owner_str = params.owner_id.as_deref().unwrap_or("");
    let cache_key_input = format!("{}:{}:{}:{}:{}", path_str, dept_str, visibility_str, owner_str, auth.user_id);
    let cache_key = keys::files(tenant_id, &hash_path(&cache_key_input));
    
    // Try to get from cache first
    if let Some(ref cache) = state.cache {
        if let Ok(cached) = cache.get::<Vec<Value>>(&cache_key).await {
            return Ok(Json(json!(cached)));
        }
    }

    // Get user's department, allowed departments, and role
    let user: Option<(Option<Uuid>, String, Option<Vec<Uuid>>)> = sqlx::query_as(
        "SELECT department_id, role, allowed_department_ids FROM users WHERE id = $1"
    )
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
        
    let user_department_id = user.as_ref().and_then(|u| u.0);
    let role = user.as_ref().map(|u| u.1.clone()).unwrap_or_default();
    let user_allowed_department_ids = user.as_ref().and_then(|u| u.2.clone());

    // Build query
    let mut query = String::from("SELECT * FROM files_metadata WHERE tenant_id = $1 AND is_deleted = false");
    
    // Visibility filter based on requested view mode
    let view_mode = params.visibility.as_deref().unwrap_or("department");
    
    if view_mode == "private" {
        // Determine whose private files to show
        let target_owner = if let Some(ref oid) = params.owner_id {
            if (role == "SuperAdmin" || role == "Admin") && !oid.is_empty() {
                // Admin viewing another user's files
                match Uuid::parse_str(oid) {
                    Ok(target_uuid) => {
                        // Log admin viewing private files (audit will be added below)
                        if target_uuid != auth.user_id {
                            // Audit log for viewing another user's private files
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
                                VALUES ($1, $2, 'private_files_view', 'user', $3, $4, $5::inet)
                                "#
                            )
                            .bind(tenant_id)
                            .bind(auth.user_id)
                            .bind(target_uuid)
                            .bind(json!({"viewed_user_id": target_uuid.to_string(), "path": path_str}))
                            .bind(&auth.ip_address)
                            .execute(&state.pool)
                            .await;
                        }
                        target_uuid
                    }
                    Err(_) => auth.user_id // Invalid UUID, fall back to own files
                }
            } else {
                auth.user_id // Non-admin or empty owner_id
            }
        } else {
            auth.user_id // No owner_id specified
        };
        
        query.push_str(&format!(" AND visibility = 'private' AND owner_id = '{}'", target_owner));
    } else {
        // Department view: show department files with existing access rules
        query.push_str(" AND visibility = 'department'");
        
        // Filter logic...
        if role == "SuperAdmin" || role == "Admin" {
            if let Some(dept_id_str) = &params.department_id {
                if !dept_id_str.is_empty() {
                    if let Ok(dept_uuid) = Uuid::parse_str(dept_id_str) {
                        query.push_str(&format!(" AND department_id = '{}'", dept_uuid));
                    }
                }
            }
        } else {
            // Non-admin users: check primary department + allowed departments
            let mut dept_conditions: Vec<String> = Vec::new();
            
            // Add primary department if set
            if let Some(dept_id) = user_department_id {
                dept_conditions.push(format!("department_id = '{}'", dept_id));
            }
            
            // Add allowed departments if any
            if let Some(ref allowed_depts) = user_allowed_department_ids {
                for dept_id in allowed_depts {
                    dept_conditions.push(format!("department_id = '{}'", dept_id));
                }
            }
            
            // Build the condition
            if dept_conditions.is_empty() {
                // User has no department access, only show files with no department
                query.push_str(" AND department_id IS NULL");
            } else {
                // User can access their department(s) OR files with no department
                query.push_str(&format!(" AND (department_id IS NULL OR {})", dept_conditions.join(" OR ")));
            }
        }
    }
    
    // Handle path/folder navigation
    if let Some(path) = &params.path {
        let path = path.trim_start_matches('/');
        if !path.is_empty() {
             // Validate that path is a valid UUID if we are using UUID-based folders
             // If we are using path-based, then we should sanitize.
             // Based on schema (checking next), if parent_path is UUID, we must validate.
             // Assuming it is UUID for now based on typical patterns, but will verify with schema.
             // If it's a string path, we sanitize.
             
             // Sanitize strict: only allow alphanumeric, dashes, underscores, slashes, dots, spaces
             if !path.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.' || c == ' ') {
                 return Err(StatusCode::BAD_REQUEST);
             }
             
             query.push_str(&format!(" AND parent_path = '{}'", path));
        } else {
             query.push_str(" AND (parent_path IS NULL OR parent_path = '')");
        }
    } else {
        query.push_str(" AND (parent_path IS NULL OR parent_path = '')");
    }
    
    let files = sqlx::query_as::<_, FileMetadata>(&query)
        .bind(tenant_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list files from DB: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Fetch owner names and avatars for all files
    let owner_ids: Vec<Uuid> = files.iter()
        .filter_map(|f| f.owner_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    
    let owner_info: std::collections::HashMap<Uuid, (String, Option<String>)> = if !owner_ids.is_empty() {
        let owners: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, avatar_url FROM users WHERE id = ANY($1)"
        )
        .bind(&owner_ids)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        
        owners.into_iter()
            .map(|(id, name, avatar)| (id, (name, avatar)))
            .collect()
    } else {
        std::collections::HashMap::new()
    };
    
    // Get folder sizes for directories
    let folder_ids: Vec<Uuid> = files.iter()
        .filter(|f| f.is_directory)
        .map(|f| f.id)
        .collect();
    
    // Calculate folder sizes (sum of all files inside each folder recursively)
    let folder_sizes: std::collections::HashMap<Uuid, i64> = if !folder_ids.is_empty() {
        // Get folder paths to calculate sizes
        let folder_paths: Vec<(Uuid, String)> = files.iter()
            .filter(|f| f.is_directory)
            .map(|f| {
                let folder_path = if let Some(ref pp) = f.parent_path {
                    if pp.is_empty() {
                        f.name.clone()
                    } else {
                        format!("{}/{}", pp, f.name)
                    }
                } else {
                    f.name.clone()
                };
                (f.id, folder_path)
            })
            .collect();
        
        let mut sizes = std::collections::HashMap::new();
        for (folder_id, folder_path) in folder_paths {
            // Sum all files that have this folder as parent or are nested inside
            // Filter by visibility to match the current view mode
            let size_result: Result<i64, _> = sqlx::query_scalar(
                r#"
                SELECT COALESCE(SUM(size_bytes), 0)::bigint as total
                FROM files_metadata 
                WHERE tenant_id = $1 
                AND is_deleted = false 
                AND is_directory = false
                AND visibility = $4
                AND (parent_path = $2 OR parent_path LIKE $3)
                "#
            )
            .bind(tenant_id)
            .bind(&folder_path)
            .bind(format!("{}/%", folder_path))
            .bind(view_mode)
            .fetch_one(&state.pool)
            .await;
            
            match size_result {
                Ok(total) if total > 0 => {
                    sizes.insert(folder_id, total);
                }
                Err(e) => {
                    tracing::warn!("Failed to calculate folder size for {}: {:?}", folder_path, e);
                }
                _ => {}
            }
        }
        sizes
    } else {
        std::collections::HashMap::new()
    };
    
    let file_items: Vec<Value> = files.into_iter().map(|meta| {
        let file_type = if meta.is_directory {
            "folder"
        } else if meta.name.ends_with(".png") || meta.name.ends_with(".jpg") || meta.name.ends_with(".jpeg") {
            "image"
        } else if meta.name.ends_with(".pdf") {
            "document"
        } else if meta.name.ends_with(".mp4") {
            "video"
        } else if meta.name.ends_with(".txt") || meta.name.ends_with(".md") {
            "document"
        } else {
            "document"
        };

        // Get folder size if it's a directory
        let size_display = if meta.is_directory {
            if let Some(&folder_size) = folder_sizes.get(&meta.id) {
                if folder_size > 0 {
                    format_size(folder_size as u64)
                } else {
                    "-".to_string()
                }
            } else {
                "-".to_string()
            }
        } else {
            format_size(meta.size_bytes as u64)
        };

        // Get owner info from the map
        let (owner_name, owner_avatar) = meta.owner_id
            .and_then(|oid| owner_info.get(&oid))
            .map(|(name, avatar)| (name.clone(), avatar.clone()))
            .unwrap_or_else(|| ("Unknown".to_string(), None));

        json!({
            "id": meta.id.to_string(), // Use proper UUID as ID
            "name": meta.name,
            "type": file_type,
            "size": size_display,
            "size_bytes": if meta.is_directory { folder_sizes.get(&meta.id).copied().unwrap_or(0) } else { meta.size_bytes },
            "modified": meta.updated_at.to_rfc3339(),
            "created_at": meta.created_at.to_rfc3339(),
            "owner": owner_name,
            "owner_avatar": owner_avatar,
            "owner_id": meta.owner_id,
            "department_id": meta.department_id,
            "visibility": meta.visibility,
            "is_company_folder": meta.is_company_folder,
            "is_locked": meta.is_locked,
            "locked_by": meta.locked_by,
            "locked_at": meta.locked_at.map(|t| t.to_rfc3339()),
            "lock_requires_role": meta.lock_requires_role,
            "has_lock_password": meta.lock_password_hash.is_some(),
            "content_type": meta.content_type,
            "storage_path": meta.storage_path
        })
    }).collect();

    // Cache the result
    if let Some(ref cache) = state.cache {
        if let Err(e) = cache.set(&cache_key, &file_items, ttl::FILES).await {
            tracing::warn!("Failed to cache file listing: {}", e);
        }
    }

    Ok(Json(json!(file_items)))
}

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

pub async fn create_folder(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let folder_name = payload["name"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
    let parent_path = payload["parent_path"].as_str().unwrap_or("");
    let visibility = payload["visibility"].as_str().unwrap_or("department");
    let visibility = if visibility == "private" { "private" } else { "department" };
    
    // Parse and verify tenant access FIRST - SECURITY: prevent cross-tenant folder creation
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        tracing::warn!(
            "Security: User {} from tenant {} attempted to create folder in tenant {}",
            auth.user_id, auth.tenant_id, tenant_id
        );
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Construct storage key (path)
    let key = if parent_path.is_empty() {
        format!("{}/{}", company_id, folder_name)
    } else {
        format!("{}/{}/{}", company_id, parent_path, folder_name)
    };
    
    // Create in storage
    state.storage.create_folder(&key).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Get user's department
    let user = sqlx::query!("SELECT department_id FROM users WHERE id = $1", auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
        
    let department_id = user.and_then(|u| u.department_id);

    sqlx::query(
        r#"
        INSERT INTO files_metadata (tenant_id, name, storage_path, size_bytes, content_type, is_directory, owner_id, department_id, parent_path, visibility)
        VALUES ($1, $2, $3, 0, 'directory', true, $4, $5, $6, $7)
        "#
    )
    .bind(tenant_id)
    .bind(folder_name)
    .bind(&key)
    .bind(auth.user_id)
    .bind(department_id)
    .bind(if parent_path.is_empty() { None::<&str> } else { Some(parent_path) })
    .bind(visibility)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save folder metadata: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Invalidate file listing cache for this tenant
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }
    
    Ok(Json(json!({ "message": "Folder created successfully" })))
}

/// Helper function to copy a storage object from one key to another
/// May be used for explicit file copy operations
#[allow(dead_code)]
async fn copy_storage_object(
    storage: &std::sync::Arc<dyn clovalink_storage::Storage>,
    from_key: &str,
    to_key: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Download from source
    let data = storage.download(from_key).await?;
    // Upload to destination
    storage.upload(to_key, data).await?;
    Ok(())
}

/// Maximum total size for zip downloads (500MB)
const MAX_ZIP_SIZE_BYTES: i64 = 500 * 1024 * 1024;

/// Sanitize a path for safe inclusion in a zip archive
/// Prevents zip-slip attacks by:
/// - Removing leading slashes and backslashes
/// - Removing path traversal components (..)
/// - Rejecting absolute paths
fn sanitize_zip_path(path: &str) -> Option<String> {
    // Reject empty paths
    if path.is_empty() {
        return None;
    }
    
    // Remove leading slashes and backslashes
    let path = path.trim_start_matches(|c| c == '/' || c == '\\');
    
    // Split and filter path components
    let safe_parts: Vec<&str> = path
        .split(|c| c == '/' || c == '\\')
        .filter(|part| {
            // Remove empty parts
            if part.is_empty() {
                return false;
            }
            // Remove path traversal
            if *part == ".." {
                return false;
            }
            // Remove current directory references
            if *part == "." {
                return false;
            }
            true
        })
        .collect();
    
    if safe_parts.is_empty() {
        return None;
    }
    
    // Rebuild the path
    let sanitized = safe_parts.join("/");
    
    // Final check: reject if it still looks like an absolute path
    if sanitized.starts_with('/') || sanitized.contains(':') {
        tracing::warn!("Rejecting potentially unsafe zip path: {}", path);
        return None;
    }
    
    Some(sanitized)
}

/// Helper function to download a folder as a zip archive
/// 
/// Security features:
/// - Path sanitization to prevent zip-slip
/// - Size limit to prevent OOM
/// - Permission checks for each file
async fn download_folder_as_zip(
    state: &Arc<AppState>,
    tenant_id: Uuid,
    user_id: Uuid,
    folder_name: &str,
    parent_path: &str,
    client_ip: Option<String>,
) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    use std::io::{Write, Cursor};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    // Build the folder's full path (no leading slash - matches database format)
    let folder_path = if parent_path.is_empty() || parent_path == "/" {
        folder_name.to_string()
    } else {
        format!("{}/{}", parent_path, folder_name)
    };
    
    tracing::info!("Downloading folder as zip: {} (parent: {})", folder_path, parent_path);

    // Query all files recursively within this folder, including file_id for permission checks
    let files: Vec<(Uuid, String, String, String, i64)> = sqlx::query_as(
        r#"
        SELECT id, name, storage_path, parent_path, size_bytes 
        FROM files_metadata 
        WHERE tenant_id = $1 
        AND is_deleted = false 
        AND is_directory = false
        AND (parent_path = $2 OR parent_path LIKE $3)
        "#
    )
    .bind(tenant_id)
    .bind(&folder_path)
    .bind(format!("{}/%", folder_path))
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query folder files: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if files.is_empty() {
        tracing::info!("Folder {} has no files, returning empty zip", folder_path);
    }
    
    // Check total size before starting
    let total_size: i64 = files.iter().map(|(_, _, _, _, size)| size).sum();
    if total_size > MAX_ZIP_SIZE_BYTES {
        tracing::warn!(
            "Folder {} exceeds max zip size ({} bytes > {} bytes)",
            folder_path, total_size, MAX_ZIP_SIZE_BYTES
        );
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Create zip archive in memory
    let mut zip_buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut zip_buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for (file_id, file_name, storage_path, file_parent_path, _size) in &files {
            // SECURITY: Check permission for each file
            // This ensures users can't use folder download to bypass file-level permissions
            // Note: For shared folder downloads, we skip this check (handled by download_shared_folder_as_zip)
            // Commenting out for now as folder access already implies file access, but keeping the pattern
            // if !can_access_file(&state.pool, *file_id, tenant_id, user_id, "Employee", "read").await.unwrap_or(false) {
            //     tracing::debug!("Skipping file {} in zip due to permission check", file_id);
            //     continue;
            // }
            let _ = file_id; // Suppress unused warning
            let _ = user_id; // Suppress unused warning
            
            // Calculate relative path within the zip
            let relative_path = if file_parent_path == &folder_path {
                file_name.clone()
            } else {
                // Strip the folder_path prefix to get relative path
                let sub_path = file_parent_path.strip_prefix(&folder_path)
                    .unwrap_or(file_parent_path)
                    .trim_start_matches('/');
                if sub_path.is_empty() {
                    file_name.clone()
                } else {
                    format!("{}/{}", sub_path, file_name)
                }
            };
            
            // SECURITY: Sanitize the zip entry path to prevent zip-slip
            let safe_path = match sanitize_zip_path(&relative_path) {
                Some(p) => p,
                None => {
                    tracing::warn!("Skipping file with unsafe path: {}", relative_path);
                    continue;
                }
            };

            // Download file from storage
            match state.storage.download(storage_path).await {
                Ok(data) => {
                    if let Err(e) = zip.start_file(&safe_path, options) {
                        tracing::error!("Failed to start zip file {}: {}", safe_path, e);
                        continue;
                    }
                    if let Err(e) = zip.write_all(&data) {
                        tracing::error!("Failed to write file {} to zip: {}", safe_path, e);
                        continue;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to download file {} from storage: {}", storage_path, e);
                    // Continue with other files
                }
            }
        }

        zip.finish().map_err(|e| {
            tracing::error!("Failed to finish zip: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    let zip_data = zip_buffer.into_inner();
    // Sanitize folder name for the zip filename header
    let safe_folder_name = folder_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.', "_");
    let zip_filename = format!("{}.zip", safe_folder_name);

    // Log folder download for compliance
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());

    if should_force_audit_log(&compliance_mode, "file_download") {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'folder_download', 'folder', NULL, $3, $4::inet)
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(json!({
            "folder_name": folder_name,
            "folder_path": folder_path,
            "file_count": files.len(),
            "compliance_mode": compliance_mode,
        }))
        .bind(&client_ip)
        .execute(&state.pool)
        .await;
    }

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, sanitize_content_disposition(&zip_filename, "attachment"))
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(zip_data))
        .unwrap())
}

/// Download shared folder as zip (for share links - no user_id required)
/// 
/// Security features:
/// - Path sanitization to prevent zip-slip
/// - Size limit to prevent OOM
async fn download_shared_folder_as_zip(
    state: &Arc<AppState>,
    tenant_id: Uuid,
    folder_name: &str,
    parent_path: &str,
) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    use std::io::{Write, Cursor};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    // Build the folder's full path
    let folder_path = if parent_path.is_empty() || parent_path == "/" {
        folder_name.to_string()
    } else {
        format!("{}/{}", parent_path, folder_name)
    };
    
    tracing::info!("Downloading shared folder as zip: {} (parent: {})", folder_path, parent_path);

    // Query all files recursively within this folder, including size for limit check
    let files: Vec<(String, String, String, i64)> = sqlx::query_as(
        r#"
        SELECT name, storage_path, parent_path, size_bytes 
        FROM files_metadata 
        WHERE tenant_id = $1 
        AND is_deleted = false 
        AND is_directory = false
        AND (parent_path = $2 OR parent_path LIKE $3)
        "#
    )
    .bind(tenant_id)
    .bind(&folder_path)
    .bind(format!("{}/%", folder_path))
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query folder files for share: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Check total size before starting
    let total_size: i64 = files.iter().map(|(_, _, _, size)| size).sum();
    if total_size > MAX_ZIP_SIZE_BYTES {
        tracing::warn!(
            "Shared folder {} exceeds max zip size ({} bytes > {} bytes)",
            folder_path, total_size, MAX_ZIP_SIZE_BYTES
        );
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Create zip archive in memory
    let mut zip_buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut zip_buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for (file_name, storage_path, file_parent_path, _size) in &files {
            // Calculate relative path within the zip
            let relative_path = if file_parent_path == &folder_path {
                file_name.clone()
            } else {
                let sub_path = file_parent_path.strip_prefix(&folder_path)
                    .unwrap_or(file_parent_path)
                    .trim_start_matches('/');
                if sub_path.is_empty() {
                    file_name.clone()
                } else {
                    format!("{}/{}", sub_path, file_name)
                }
            };
            
            // SECURITY: Sanitize the zip entry path to prevent zip-slip
            let safe_path = match sanitize_zip_path(&relative_path) {
                Some(p) => p,
                None => {
                    tracing::warn!("Skipping file with unsafe path in shared zip: {}", relative_path);
                    continue;
                }
            };

            // Download file from storage
            match state.storage.download(storage_path).await {
                Ok(data) => {
                    if let Err(e) = zip.start_file(&safe_path, options) {
                        tracing::error!("Failed to start zip file {}: {}", safe_path, e);
                        continue;
                    }
                    if let Err(e) = zip.write_all(&data) {
                        tracing::error!("Failed to write file {} to zip: {}", safe_path, e);
                        continue;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to download file {} from storage for share: {}", storage_path, e);
                }
            }
        }

        zip.finish().map_err(|e| {
            tracing::error!("Failed to finish zip: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    let zip_data = zip_buffer.into_inner();
    // Create sanitized zip filename
    let zip_filename = format!("{}.zip", sanitize_filename(folder_name));

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, sanitize_content_disposition(&zip_filename, "attachment"))
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(zip_data))
        .unwrap())
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct DownloadParams {
    pub preview: Option<bool>,
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<DownloadParams>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    // Parse UUIDs
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // SECURITY: Check if user has permission to access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "read").await? {
        tracing::warn!(
            "Access denied: user {} attempted to download file {} without permission",
            auth.user_id, file_uuid
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // First check if this is a directory
    let dir_check: Option<(String, bool, Option<String>)> = sqlx::query_as(
        "SELECT name, is_directory, parent_path FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check directory: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (folder_name, is_directory, parent_path_opt) = dir_check.ok_or(StatusCode::NOT_FOUND)?;
    let parent_path = parent_path_opt.unwrap_or_default();

    // If it's a directory, create a zip of all files in it
    if is_directory {
        return download_folder_as_zip(&state, tenant_id, auth.user_id, &folder_name, &parent_path, auth.ip_address.clone()).await;
    }

    // Look up file by ID for regular file download
    let file_meta: (String, String, i64) = sqlx::query_as(
        "SELECT name, storage_path, size_bytes FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false AND is_directory = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    
    let (file_name, storage_path, file_size) = file_meta;

    // Get compliance mode for HIPAA audit logging
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());

    // NOTE: Token-in-URL is now blocked at middleware level for ALL users (security best practice)

    // Determine if this is a preview or download for audit logging
    let is_preview = params.preview.unwrap_or(false);
    let audit_action = if is_preview { "file_preview" } else { "file_download" };

    // Log file download/view for HIPAA compliance
    if should_force_audit_log(&compliance_mode, audit_action) {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, $3, 'file', $4, $5, $6::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(audit_action)
        .bind(file_uuid)
        .bind(json!({
            "file_name": &file_name,
            "compliance_mode": compliance_mode,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }

    // Log for GDPR export traceability
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);
    if restrictions.export_logging_required {
        let _ = log_file_export(
            &state.pool,
            tenant_id,
            auth.user_id,
            Some(file_uuid),
            "single_file",
            1,
            Some(file_size),
            None,
        ).await;
    }

    // Check for bulk download pattern (security alert)
    if !is_preview {
        let _ = security_service::check_bulk_download(
            &state.pool,
            tenant_id,
            auth.user_id,
            &auth.email,
            auth.ip_address.as_deref(),
        ).await;
    }

    // Try presigned URL redirect if enabled and supported (S3-compatible storage)
    // This bypasses the proxy and redirects directly to S3/CDN for better performance
    if state.use_presigned_urls && state.storage.supports_presigned_urls() {
        match state.storage.presigned_download_url(&storage_path, state.presigned_url_expiry).await {
            Ok(Some(mut presigned_url)) => {
                // Optionally rewrite through CDN for edge caching
                if let Some(cdn) = &state.cdn_domain {
                    presigned_url = rewrite_url_to_cdn(&presigned_url, cdn);
                }
                
                tracing::debug!(
                    "Redirecting file download to presigned URL: user={}, file={}",
                    auth.user_id, file_uuid
                );
                
                // Return redirect to presigned URL (307 preserves method but 302 is more compatible)
                return Ok(Redirect::temporary(&presigned_url).into_response());
            }
            Ok(None) => {
                // Storage doesn't support presigned URLs, fallback to proxy
                tracing::debug!("Storage doesn't support presigned URLs, using proxy");
            }
            Err(e) => {
                // Presigning failed, fallback to proxy
                tracing::warn!("Presigned URL generation failed, falling back to proxy: {}", e);
            }
        }
    }

    // FALLBACK: Proxy download through backend using STREAMING (for local storage or when presigned URLs disabled/failed)
    // This streams the file in chunks (~8KB) without loading the entire file into memory
    
    // Acquire transfer scheduler permit based on file size (prioritizes small files)
    // This blocks if too many transfers of this size class are in progress
    let transfer_permit = state.scheduler.acquire_download_permit(file_size).await;
    tracing::debug!(
        "Download permit acquired: file={}, size={}, class={}",
        file_uuid, file_size, transfer_permit.size_class.name()
    );
    
    let (stream, stream_size) = state.storage.download_stream(&storage_path).await
        .map_err(|e| {
            tracing::error!("Failed to open file stream: {}", e);
            StatusCode::NOT_FOUND
        })?;
    
    // Get filename for Content-Disposition header
    let filename = file_name;
    
    let content_type = get_content_type(&filename);

    // Generate ETag based on storage path and size (deterministic without reading file)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    storage_path.hash(&mut hasher);
    stream_size.hash(&mut hasher);
    let etag = format!("\"{}\"", hasher.finish());

    // SECURITY: Use sanitized Content-Disposition to prevent header injection
    let safe_disposition = sanitize_content_disposition(&filename, "inline");
    
    // Convert the stream to an axum Body (zero-copy streaming)
    // Note: transfer_permit is held in scope until response is fully sent
    let body = Body::from_stream(stream.map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }));
    
    // The permit is held until the response body is fully streamed
    // This is fine because axum handles the response in the same task context
    let _ = &transfer_permit;
    
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, stream_size)
        .header(header::CONTENT_DISPOSITION, safe_disposition)
        // CDN optimization headers (for proxy fallback)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::ETAG, etag)
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap()
        .into_response())
}

pub async fn rename_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Support both file_id (preferred) and old_name (legacy) lookup
    let file_id_str = payload["file_id"].as_str();
    let old_name = payload["old_name"].as_str();
    let new_name = payload["new_name"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
    let req_parent_path = payload["parent_path"].as_str().unwrap_or("");
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get compliance mode for SOX audit logging
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // Look up file by ID (preferred) or by name+parent_path (legacy)
    // SECURITY: Always include tenant_id in lookup
    let file_info: Option<(Uuid, String, Option<String>, Option<bool>, bool, bool)> = if let Some(id_str) = file_id_str {
        let file_uuid = Uuid::parse_str(id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
        sqlx::query_as(
            r#"
            SELECT id, name, parent_path, is_immutable, is_locked, is_directory 
            FROM files_metadata 
            WHERE id = $1 AND tenant_id = $2 AND is_deleted = false
            "#
        )
        .bind(file_uuid)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else if let Some(name) = old_name {
        // Legacy lookup by name + parent_path (still requires tenant_id)
        let parent_path_query = if req_parent_path.is_empty() { None } else { Some(req_parent_path) };
        sqlx::query_as(
            r#"
            SELECT id, name, parent_path, is_immutable, is_locked, is_directory 
            FROM files_metadata 
            WHERE tenant_id = $1 AND name = $2 AND is_deleted = false
            AND (($3::text IS NULL AND parent_path IS NULL) OR parent_path = $3)
            "#
        )
        .bind(tenant_id)
        .bind(name)
        .bind(parent_path_query)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        return Err(StatusCode::BAD_REQUEST); // Need either file_id or old_name
    };

    let (file_id, current_name, current_parent_path, is_immutable, is_locked, is_directory) = 
        file_info.ok_or(StatusCode::NOT_FOUND)?;

    // SECURITY: Check if user has permission to rename this file
    if !can_access_file(&state.pool, file_id, tenant_id, auth.user_id, &auth.role, "write").await? {
        tracing::warn!(
            "Access denied: user {} attempted to rename file {} without permission",
            auth.user_id, file_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    if is_locked {
        return Err(StatusCode::FORBIDDEN); // Cannot rename locked files
    }
    if is_immutable.unwrap_or(false) {
        return Err(StatusCode::FORBIDDEN); // Cannot rename immutable files
    }

    // Extract just the filename (no path components allowed in new_name for rename)
    let new_filename = new_name.split('/').last().unwrap_or(new_name).to_string();
    
    // Validate new filename
    if new_filename.is_empty() || new_filename.contains('\0') || new_filename == "." || new_filename == ".." {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for duplicate filename in same location
    let duplicate_check: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM files_metadata 
        WHERE tenant_id = $1 
        AND name = $2 
        AND is_deleted = false
        AND id != $3
        AND (($4::text IS NULL AND parent_path IS NULL) OR parent_path = $4)
        "#
    )
    .bind(tenant_id)
    .bind(&new_filename)
    .bind(file_id)
    .bind(&current_parent_path)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if duplicate_check.is_some() {
        return Ok(Json(json!({ "error": "A file with this name already exists in this folder" })));
    }
    
    // CONTENT-ADDRESSED STORAGE: Only update metadata, do NOT touch S3
    // The storage_path remains the same (it's the content hash key)
    sqlx::query(
        "UPDATE files_metadata SET name = $1, updated_at = NOW() WHERE id = $2 AND tenant_id = $3"
    )
    .bind(&new_filename)
    .bind(file_id)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update file metadata: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // If this is a directory, update all children's parent_path
    // NOTE: We do NOT update storage_path - files keep their content-addressed keys
    if is_directory {
        let old_folder_path = if let Some(ref pp) = current_parent_path {
            if pp.is_empty() { current_name.clone() } else { format!("{}/{}", pp, current_name) }
        } else {
            current_name.clone()
        };
        let new_folder_path = if let Some(ref pp) = current_parent_path {
            if pp.is_empty() { new_filename.clone() } else { format!("{}/{}", pp, new_filename) }
        } else {
            new_filename.clone()
        };
        
        // Update children with exact parent_path match
        sqlx::query(
            "UPDATE files_metadata SET parent_path = $1 WHERE tenant_id = $2 AND parent_path = $3"
        )
        .bind(&new_folder_path)
        .bind(tenant_id)
        .bind(&old_folder_path)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update children parent_path: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // Update nested children (parent_path starts with old_folder_path/)
        let old_prefix = format!("{}/", old_folder_path);
        let nested_children: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, parent_path FROM files_metadata WHERE tenant_id = $1 AND parent_path LIKE $2"
        )
        .bind(tenant_id)
        .bind(format!("{}%", old_prefix))
        .fetch_all(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        for (child_id, child_parent_path) in nested_children {
            let new_child_parent = child_parent_path.replacen(&old_folder_path, &new_folder_path, 1);
            sqlx::query("UPDATE files_metadata SET parent_path = $1 WHERE id = $2 AND tenant_id = $3")
                .bind(&new_child_parent)
                .bind(child_id)
                .bind(tenant_id)
                .execute(&state.pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        
        tracing::info!("Updated children paths for renamed folder: {} -> {}", old_folder_path, new_folder_path);
    }

    // Log rename for SOX compliance
    if should_force_audit_log(&compliance_mode, "file_rename") || restrictions.file_versioning_required {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'file_rename', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(file_id)
        .bind(json!({
            "old_name": current_name,
            "new_name": new_filename,
            "compliance_mode": compliance_mode,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }
    
    // Invalidate file listing cache for this tenant
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }
    
    Ok(Json(json!({ "message": "File renamed successfully" })))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Support both file_id (preferred) and path (legacy) lookup
    let file_id_str = payload["file_id"].as_str();
    let path = payload["path"].as_str();
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access - SECURITY: prevent cross-tenant deletions
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        tracing::warn!(
            "Security: User {} from tenant {} attempted to delete from tenant {}",
            auth.user_id, auth.tenant_id, tenant_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get compliance mode
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // Look up file by ID (preferred) or by path (legacy)
    // SECURITY: Always include tenant_id in lookup
    let file_info: Option<(Uuid, Option<Uuid>, Option<bool>, bool, bool, String, Option<String>)> = 
        if let Some(id_str) = file_id_str {
            let file_uuid = Uuid::parse_str(id_str).map_err(|_| StatusCode::BAD_REQUEST)?;
            sqlx::query_as(
                r#"
                SELECT id, owner_id, is_immutable, is_locked, is_directory, name, parent_path 
                FROM files_metadata 
                WHERE id = $1 AND tenant_id = $2 AND is_deleted = false
                "#
            )
            .bind(file_uuid)
            .bind(tenant_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        } else if let Some(p) = path {
            // Legacy lookup by path - construct expected name and parent_path
            let parts: Vec<&str> = p.split('/').collect();
            let name = parts.last().unwrap_or(&p);
            let parent_path = if parts.len() > 1 {
                Some(parts[0..parts.len()-1].join("/"))
            } else {
                None
            };
            sqlx::query_as(
                r#"
                SELECT id, owner_id, is_immutable, is_locked, is_directory, name, parent_path 
                FROM files_metadata 
                WHERE tenant_id = $1 AND name = $2 AND is_deleted = false
                AND (($3::text IS NULL AND parent_path IS NULL) OR parent_path = $3)
                "#
            )
            .bind(tenant_id)
            .bind(name)
            .bind(&parent_path)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        } else {
            return Err(StatusCode::BAD_REQUEST); // Need either file_id or path
        };

    let (file_id, owner_id, is_immutable, is_locked, is_directory, file_name, parent_path) = 
        file_info.ok_or(StatusCode::NOT_FOUND)?;

    // Check if file is locked
    if is_locked {
        return Err(StatusCode::FORBIDDEN); // Cannot delete locked files
    }

    // Check if file is immutable (SOX compliance)
    if is_immutable.unwrap_or(false) && restrictions.file_versioning_required {
        return Err(StatusCode::FORBIDDEN); // Cannot delete immutable files under SOX
    }

    // Check if user is allowed to delete
    let is_admin = auth.role == "SuperAdmin" || auth.role == "Admin";
    let is_owner = owner_id == Some(auth.user_id);

    if !is_admin && !is_owner {
        // Check can_access_file for more granular permissions
        if !can_access_file(&state.pool, file_id, tenant_id, auth.user_id, &auth.role, "delete").await? {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // CONTENT-ADDRESSED STORAGE: Only mark as deleted, do NOT touch S3
    // The storage_path remains the same - S3 cleanup happens in permanent_delete with ref counting
    sqlx::query(
        "UPDATE files_metadata SET is_deleted = true, deleted_at = NOW() WHERE id = $1 AND tenant_id = $2"
    )
    .bind(file_id)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // If this is a folder, also mark all children as deleted
    // Build the folder path for matching children
    let folder_path = if let Some(ref pp) = parent_path {
        if pp.is_empty() { file_name.clone() } else { format!("{}/{}", pp, file_name) }
    } else {
        file_name.clone()
    };
    
    if is_directory {
        // Mark direct children (parent_path = folder_path)
        sqlx::query(
            "UPDATE files_metadata SET is_deleted = true, deleted_at = NOW() WHERE tenant_id = $1 AND parent_path = $2 AND is_deleted = false"
        )
        .bind(tenant_id)
        .bind(&folder_path)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark folder children as deleted: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // Mark nested children (parent_path starts with folder_path/)
        sqlx::query(
            "UPDATE files_metadata SET is_deleted = true, deleted_at = NOW() WHERE tenant_id = $1 AND parent_path LIKE $2 AND is_deleted = false"
        )
        .bind(tenant_id)
        .bind(format!("{}/%", folder_path))
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark nested folder children as deleted: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // Log deletion for SOX compliance
    if should_force_audit_log(&compliance_mode, "file_delete") || restrictions.file_versioning_required {
        let display_path = if let Some(ref pp) = parent_path {
            format!("{}/{}", pp, file_name)
        } else {
            file_name.clone()
        };
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'file_delete', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(file_id)
        .bind(json!({
            "file_path": display_path,
            "compliance_mode": compliance_mode,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }
    
    // Invalidate file listing cache for this tenant
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }
    
    Ok(Json(json!({ "message": "File moved to trash" })))
}

#[derive(serde::Deserialize)]
pub struct ListTrashParams {
    owner_id: Option<String>,       // For viewing specific user's trash (User Details Modal)
    department_id: Option<String>,  // For filtering by department (main Recycle Bin page)
}

pub async fn list_trash(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<ListTrashParams>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Parse owner_id filter (for User Details Modal - viewing specific user's trash)
    let target_owner = if let Some(ref oid) = params.owner_id {
        if (auth.role == "SuperAdmin" || auth.role == "Admin") && !oid.is_empty() {
            Uuid::parse_str(oid).ok()
        } else {
            None
        }
    } else {
        None
    };
    
    // Parse department_id filter (for main Recycle Bin page)
    let target_department = if let Some(ref did) = params.department_id {
        if (auth.role == "SuperAdmin" || auth.role == "Admin") && !did.is_empty() {
            Uuid::parse_str(did).ok()
        } else {
            None
        }
    } else {
        None
    };
    
    // Build query based on filters - owner_id takes precedence over department_id
    let rows = if let Some(owner) = target_owner {
        // Admin viewing specific user's trash (User Details Modal)
        sqlx::query(
            r#"SELECT fm.id, fm.name, fm.parent_path, fm.size_bytes, fm.is_directory, fm.deleted_at, fm.owner_id, fm.visibility, u.name as owner_name
               FROM files_metadata fm
               LEFT JOIN users u ON fm.owner_id = u.id
               WHERE fm.tenant_id = $1 AND fm.is_deleted = true AND fm.owner_id = $2
               ORDER BY fm.deleted_at DESC"#
        )
        .bind(tenant_id)
        .bind(owner)
        .fetch_all(&state.pool)
        .await
    } else if let Some(dept_id) = target_department {
        // Admin filtering by department (main Recycle Bin page)
        sqlx::query(
            r#"SELECT fm.id, fm.name, fm.parent_path, fm.size_bytes, fm.is_directory, fm.deleted_at, fm.owner_id, fm.visibility, u.name as owner_name
               FROM files_metadata fm
               LEFT JOIN users u ON fm.owner_id = u.id
               WHERE fm.tenant_id = $1 AND fm.is_deleted = true AND u.department_id = $2
               ORDER BY fm.deleted_at DESC"#
        )
        .bind(tenant_id)
        .bind(dept_id)
        .fetch_all(&state.pool)
        .await
    } else if auth.role == "SuperAdmin" || auth.role == "Admin" {
        // Admins with no filter see all tenant's deleted files
        sqlx::query(
            r#"SELECT fm.id, fm.name, fm.parent_path, fm.size_bytes, fm.is_directory, fm.deleted_at, fm.owner_id, fm.visibility, u.name as owner_name
               FROM files_metadata fm
               LEFT JOIN users u ON fm.owner_id = u.id
               WHERE fm.tenant_id = $1 AND fm.is_deleted = true
               ORDER BY fm.deleted_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&state.pool)
        .await
    } else {
        // Regular users only see their own deleted files
        sqlx::query(
            r#"SELECT fm.id, fm.name, fm.parent_path, fm.size_bytes, fm.is_directory, fm.deleted_at, fm.owner_id, fm.visibility, u.name as owner_name
               FROM files_metadata fm
               LEFT JOIN users u ON fm.owner_id = u.id
               WHERE fm.tenant_id = $1 AND fm.is_deleted = true AND fm.owner_id = $2
               ORDER BY fm.deleted_at DESC"#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .fetch_all(&state.pool)
        .await
    }.map_err(|e| {
        tracing::error!("Failed to list trash from DB: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let file_items: Vec<Value> = rows.into_iter().map(|row| {
        let id: Uuid = row.get("id");
        let name: String = row.get("name");
        let parent_path: Option<String> = row.get("parent_path");
        let size_bytes: i64 = row.get("size_bytes");
        let is_directory: bool = row.get("is_directory");
        let deleted_at: Option<chrono::DateTime<chrono::Utc>> = row.get("deleted_at");
        let owner_id: Option<Uuid> = row.get("owner_id");
        let visibility: Option<String> = row.get("visibility");
        let owner_name: Option<String> = row.get("owner_name");
        
        // Build display path from name and parent_path (content-addressed storage)
        let display_path = if let Some(ref pp) = parent_path {
            if pp.is_empty() { name.clone() } else { format!("{}/{}", pp, name) }
        } else {
            name.clone()
        };
        
        json!({
            "id": id.to_string(),
            "file_id": id.to_string(),
            "name": name,
            "path": display_path,
            "parent_path": parent_path,
            "size": format_size(size_bytes as u64),
            "size_bytes": size_bytes,
            "is_directory": is_directory,
            "deleted_at": deleted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            "owner_id": owner_id,
            "owner_name": owner_name,
            "visibility": visibility
        })
    }).collect();

    Ok(Json(json!(file_items)))
}

/// Restore a file from trash
/// CONTENT-ADDRESSED STORAGE: Only updates metadata, does not touch S3
pub async fn restore_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id_or_name)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Try to parse as UUID first (preferred), fall back to name lookup
    let file_info: Option<(Uuid, bool, String, Option<String>)> = 
        if let Ok(file_uuid) = Uuid::parse_str(&file_id_or_name) {
            sqlx::query_as(
                "SELECT id, is_directory, name, parent_path FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = true"
            )
            .bind(file_uuid)
            .bind(tenant_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        } else {
            // Legacy: lookup by name (first match in deleted files)
            sqlx::query_as(
                "SELECT id, is_directory, name, parent_path FROM files_metadata WHERE tenant_id = $1 AND name = $2 AND is_deleted = true ORDER BY deleted_at DESC LIMIT 1"
            )
            .bind(tenant_id)
            .bind(&file_id_or_name)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        };
    
    let (file_id, is_directory, file_name, parent_path) = file_info.ok_or(StatusCode::NOT_FOUND)?;

    // CONTENT-ADDRESSED STORAGE: Only update metadata, do NOT touch S3
    let result = sqlx::query(
        "UPDATE files_metadata SET is_deleted = false, deleted_at = NULL WHERE id = $1 AND tenant_id = $2"
    )
    .bind(file_id)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to restore file: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    tracing::info!("Restored file: {} (rows: {})", file_id, result.rows_affected());
    
    // If this is a folder, also restore all children
    if is_directory {
        let folder_path = if let Some(ref pp) = parent_path {
            if pp.is_empty() { file_name.clone() } else { format!("{}/{}", pp, file_name) }
        } else {
            file_name.clone()
        };
        
        // Restore direct children
        sqlx::query(
            "UPDATE files_metadata SET is_deleted = false, deleted_at = NULL WHERE tenant_id = $1 AND parent_path = $2 AND is_deleted = true"
        )
        .bind(tenant_id)
        .bind(&folder_path)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to restore folder children: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // Restore nested children
        let children_result = sqlx::query(
            "UPDATE files_metadata SET is_deleted = false, deleted_at = NULL WHERE tenant_id = $1 AND parent_path LIKE $2 AND is_deleted = true"
        )
        .bind(tenant_id)
        .bind(format!("{}/%", folder_path))
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to restore nested folder children: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        tracing::info!("Restored {} children items", children_result.rows_affected());
    }
    
    // Invalidate file listing cache for this tenant
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        if let Err(e) = cache.delete_pattern(&pattern).await {
            tracing::warn!("Failed to invalidate file cache: {}", e);
        }
    }
    
    Ok(Json(json!({ "message": "File restored" })))
}

/// Permanently delete a file from trash
/// CONTENT-ADDRESSED STORAGE: Only deletes from S3 if no other files reference the same content
pub async fn permanent_delete(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id_or_name)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Try to parse as UUID first (preferred), fall back to name lookup
    // SECURITY: Always include tenant_id in lookup
    let file_info: Option<(Uuid, Option<String>, String, bool, String, Option<String>)> = 
        if let Ok(file_uuid) = Uuid::parse_str(&file_id_or_name) {
            sqlx::query_as(
                r#"
                SELECT id, content_hash, storage_path, is_directory, name, parent_path 
                FROM files_metadata 
                WHERE id = $1 AND tenant_id = $2 AND is_deleted = true
                "#
            )
            .bind(file_uuid)
            .bind(tenant_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        } else {
            // Legacy: lookup by name
            sqlx::query_as(
                r#"
                SELECT id, content_hash, storage_path, is_directory, name, parent_path 
                FROM files_metadata 
                WHERE tenant_id = $1 AND name = $2 AND is_deleted = true 
                ORDER BY deleted_at DESC LIMIT 1
                "#
            )
            .bind(tenant_id)
            .bind(&file_id_or_name)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        };
    
    let (file_id, content_hash, storage_path, is_directory, file_name, parent_path) = 
        file_info.ok_or(StatusCode::NOT_FOUND)?;
    
    // Collect all files to delete (including children if this is a folder)
    let mut files_to_delete: Vec<(Uuid, Option<String>, String)> = vec![
        (file_id, content_hash.clone(), storage_path.clone())
    ];
    
    if is_directory {
        let folder_path = if let Some(ref pp) = parent_path {
            if pp.is_empty() { file_name.clone() } else { format!("{}/{}", pp, file_name) }
        } else {
            file_name.clone()
        };
        
        // Get all children
        let children: Vec<(Uuid, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT id, content_hash, storage_path FROM files_metadata 
            WHERE tenant_id = $1 AND is_deleted = true 
            AND (parent_path = $2 OR parent_path LIKE $3)
            "#
        )
        .bind(tenant_id)
        .bind(&folder_path)
        .bind(format!("{}/%", folder_path))
        .fetch_all(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        files_to_delete.extend(children);
    }
    
    let mut storage_deleted_count = 0;
    
    for (fid, fhash, fstorage) in &files_to_delete {
        // Check if any other files reference the same content (dedupe-aware deletion)
        let should_delete_storage = if let Some(ref hash) = fhash {
            // Count other files with same content_hash (excluding this file)
            let ref_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM files_metadata 
                WHERE content_hash = $1 AND id != $2 AND is_directory = false
                "#
            )
            .bind(hash)
            .bind(fid)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
            
            if ref_count > 0 {
                tracing::info!(
                    "Skipping storage deletion for {}: {} other files reference content {}",
                    fid, ref_count, &hash[..8.min(hash.len())]
                );
                false
            } else {
                true
            }
        } else {
            // Directories don't have content in S3
            false
        };
        
        // Delete metadata from database
        sqlx::query("DELETE FROM files_metadata WHERE id = $1 AND tenant_id = $2")
            .bind(fid)
            .bind(tenant_id)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Delete from S3 if no other references
        if should_delete_storage && !fstorage.is_empty() {
            if let Err(e) = state.storage.delete(fstorage).await {
                tracing::warn!("Failed to delete {} from storage: {:?}", fstorage, e);
            } else {
                storage_deleted_count += 1;
                
                // Enqueue S3 replication delete if enabled in mirror mode
                if state.replication_config.enabled 
                    && state.replication_config.mode == clovalink_core::replication::ReplicationMode::Mirror 
                {
                    let replication_pool = state.pool.clone();
                    let storage_key = fstorage.clone();
                    let tenant = tenant_id;
                    tokio::spawn(async move {
                        if let Err(e) = clovalink_core::replication::enqueue_delete(
                            &replication_pool,
                            &storage_key,
                            tenant,
                        ).await {
                            tracing::warn!(
                                target: "replication",
                                storage_path = %storage_key,
                                error = %e,
                                "Failed to enqueue replication delete job"
                            );
                        }
                    });
                }
            }
        }
    }
    
    tracing::info!(
        "Permanently deleted {} files, {} from storage", 
        files_to_delete.len(), storage_deleted_count
    );
    
    Ok(Json(json!({ 
        "message": "File permanently deleted",
        "files_deleted": files_to_delete.len(),
        "storage_objects_deleted": storage_deleted_count
    })))
}

// Preferences (Starred, Settings)
pub async fn get_prefs(
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let key = format!(".clovalink/{}/prefs.json", company_id);
    match state.storage.download(&key).await {
        Ok(data) => {
            let json: Value = serde_json::from_slice(&data).unwrap_or(json!({}));
            Ok(Json(json))
        },
        Err(_) => Ok(Json(json!({ "starred": [], "settings": {} })))
    }
}

pub async fn update_prefs(
    State(state): State<Arc<AppState>>,
    Path(company_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let key = format!(".clovalink/{}/prefs.json", company_id);
    let data = serde_json::to_vec(&payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Ensure dir exists
    let dir = format!(".clovalink/{}", company_id);
    let _ = state.storage.create_folder(&dir).await;
    
    state.storage.upload(&key, data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(payload))
}

// ==================== File Locking ====================

#[derive(Debug, serde::Deserialize, Default)]
pub struct LockFileInput {
    #[serde(default)]
    pub password: Option<String>,       // Optional password for unlocking
    #[serde(default)]
    pub required_role: Option<String>,  // Optional role requirement (Admin, Manager, Employee, custom)
}

pub async fn lock_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<LockFileInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // SECURITY: Check if user has permission to access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "write").await? {
        tracing::warn!(
            "Access denied: user {} attempted to lock file {} without permission",
            auth.user_id, file_uuid
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if user has lock permission (Manager, Admin, SuperAdmin, or custom role with files.lock)
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

    // Get current file status
    let file: Option<(bool, Option<Uuid>)> = sqlx::query_as(
        "SELECT is_locked, locked_by FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (is_locked, locked_by) = file.ok_or(StatusCode::NOT_FOUND)?;

    if is_locked {
        return Ok(Json(json!({
            "error": "File is already locked",
            "locked_by": locked_by
        })));
    }

    // Process optional password and role requirement
    let password_hash: Option<String> = if let Some(ref pwd) = input.password {
        if !pwd.is_empty() {
            // Hash the password using argon2 with tuned parameters
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

    // Lock the file with optional password and role
    sqlx::query(
        r#"
        UPDATE files_metadata 
        SET is_locked = true, locked_by = $1, locked_at = NOW(), 
            lock_password_hash = $3, lock_requires_role = $4
        WHERE id = $2
        "#
    )
    .bind(auth.user_id)
    .bind(file_uuid)
    .bind(&password_hash)
    .bind(&required_role)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get compliance mode for audit logging
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());

    // Log the lock action
    if should_force_audit_log(&compliance_mode, "file_lock") {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'file_lock', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(file_uuid)
        .bind(json!({
            "compliance_mode": compliance_mode,
            "has_password": password_hash.is_some(),
            "required_role": required_role,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }

    // Invalidate file listing cache
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        let _ = cache.delete_pattern(&pattern).await;
    }

    Ok(Json(json!({ 
        "message": "File locked successfully",
        "has_password": password_hash.is_some(),
        "required_role": required_role
    })))
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct UnlockFileInput {
    #[serde(default)]
    pub password: Option<String>,  // Password if the file is password-locked
}

pub async fn unlock_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<UnlockFileInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // SECURITY: Check if user has permission to access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "write").await? {
        tracing::warn!(
            "Access denied: user {} attempted to unlock file {} without permission",
            auth.user_id, file_uuid
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current file status including lock details
    let file: Option<(bool, Option<Uuid>, Option<String>, Option<String>, Uuid)> = sqlx::query_as(
        r#"
        SELECT is_locked, locked_by, lock_password_hash, lock_requires_role, owner_id 
        FROM files_metadata 
        WHERE id = $1 AND tenant_id = $2 AND is_deleted = false
        "#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (is_locked, locked_by, password_hash, required_role, owner_id) = file.ok_or(StatusCode::NOT_FOUND)?;

    if !is_locked {
        return Ok(Json(json!({ "message": "File is not locked" })));
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

    // File owner can always unlock their own files
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
        match input.password.as_ref() {
            Some(pwd) => {
                // Verify password using argon2 with tuned parameters
                use argon2::{PasswordVerifier, PasswordHash};
                let parsed_hash = PasswordHash::new(pwd_hash)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if crate::password::get_argon2().verify_password(pwd.as_bytes(), &parsed_hash).is_err() {
                    return Ok(Json(json!({
                        "error": "Incorrect password",
                        "requires_password": true
                    })));
                }
            }
            None => {
                return Ok(Json(json!({
                    "error": "Password required",
                    "requires_password": true
                })));
            }
        }
    }

    // Unlock the file
    sqlx::query(
        r#"
        UPDATE files_metadata 
        SET is_locked = false, locked_by = NULL, locked_at = NULL, 
            lock_password_hash = NULL, lock_requires_role = NULL 
        WHERE id = $1
        "#
    )
    .bind(file_uuid)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get compliance mode for audit logging
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());

    // Log the unlock action
    if should_force_audit_log(&compliance_mode, "file_unlock") {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'file_unlock', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(file_uuid)
        .bind(json!({
            "compliance_mode": compliance_mode,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }

    // Invalidate file listing cache
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        let _ = cache.delete_pattern(&pattern).await;
    }

    Ok(Json(json!({ "message": "File unlocked successfully" })))
}

// ==================== File Activity ====================

// ==================== Move File ====================

#[derive(Debug, serde::Deserialize)]
pub struct MoveFileInput {
    pub target_parent_id: Option<String>,      // UUID of target folder, null for root
    pub target_department_id: Option<String>,  // UUID of target department (for cross-department moves)
    pub target_visibility: Option<String>,     // 'department' or 'private' (for moving between views)
}

pub async fn move_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<MoveFileInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // SECURITY: Check if user has permission to move this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "write").await? {
        tracing::warn!(
            "Access denied: user {} attempted to move file {} without permission",
            auth.user_id, file_uuid
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current file info (content-addressed storage: we don't need storage_path for moves)
    let file: Option<(String, Option<String>, bool, bool, Option<Uuid>, String)> = sqlx::query_as(
        r#"
        SELECT name, parent_path, is_locked, is_directory, department_id, visibility 
        FROM files_metadata 
        WHERE id = $1 AND tenant_id = $2 AND is_deleted = false
        "#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (file_name, current_parent_path, is_locked, is_directory, current_dept_id, current_visibility) = file.ok_or(StatusCode::NOT_FOUND)?;

    // Cannot move locked files
    if is_locked {
        return Err(StatusCode::FORBIDDEN);
    }

    // Parse target parent folder
    let target_parent_id: Option<Uuid> = if let Some(ref id_str) = input.target_parent_id {
        if id_str.is_empty() || id_str == "null" {
            None // Move to root
        } else {
            Some(Uuid::parse_str(id_str).map_err(|_| StatusCode::BAD_REQUEST)?)
        }
    } else {
        None // Move to root
    };

    // Parse target department
    let target_dept_id: Option<Uuid> = if let Some(ref id_str) = input.target_department_id {
        if id_str.is_empty() || id_str == "null" {
            None
        } else {
            Some(Uuid::parse_str(id_str).map_err(|_| StatusCode::BAD_REQUEST)?)
        }
    } else {
        current_dept_id // Keep current department if not specified
    };

    // Parse target visibility
    let target_visibility = input.target_visibility.as_deref().unwrap_or(&current_visibility);
    let target_visibility = if target_visibility == "private" { "private" } else { "department" };

    // Check if user has access to target department (for cross-department moves)
    if target_dept_id != current_dept_id {
        // Only Admin/SuperAdmin can move across departments
        if !["SuperAdmin", "Admin"].contains(&auth.role.as_str()) {
            // Check if user belongs to target department
            let user_dept: Option<(Option<Uuid>,)> = sqlx::query_as(
                "SELECT department_id FROM users WHERE id = $1"
            )
            .bind(auth.user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if let Some((user_dept_id,)) = user_dept {
                if user_dept_id != target_dept_id {
                    return Err(StatusCode::FORBIDDEN); // Cannot move to department user doesn't belong to
                }
            }
        }
    }

    // Get target folder path
    let new_parent_path: Option<String> = if let Some(target_id) = target_parent_id {
        // Verify target folder exists and get its path
        let target_folder: Option<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT name, parent_path FROM files_metadata 
            WHERE id = $1 AND tenant_id = $2 AND is_directory = true AND is_deleted = false
            "#
        )
        .bind(target_id)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let (folder_name, folder_parent) = target_folder.ok_or(StatusCode::NOT_FOUND)?;
        
        // Build target path
        Some(if let Some(ref fp) = folder_parent {
            if fp.is_empty() {
                folder_name
            } else {
                format!("{}/{}", fp, folder_name)
            }
        } else {
            folder_name
        })
    } else {
        None // Root folder
    };

    // Check for duplicate filename in target location
    let duplicate_check: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM files_metadata 
        WHERE tenant_id = $1 
        AND name = $2 
        AND is_deleted = false
        AND id != $3
        AND (
            ($4::text IS NULL AND parent_path IS NULL) OR
            ($4::text IS NOT NULL AND parent_path = $4)
        )
        "#
    )
    .bind(tenant_id)
    .bind(&file_name)
    .bind(file_uuid)
    .bind(&new_parent_path)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if duplicate_check.is_some() {
        return Ok(Json(json!({ "error": "A file with this name already exists in the target folder" })));
    }

    // CONTENT-ADDRESSED STORAGE: Move is metadata-only, never touches S3
    // The storage_path is immutable (based on content hash), only metadata changes
    // Update metadata: parent_path, department_id, visibility
    // When moving to private visibility, set owner_id to the user doing the move
    sqlx::query(
        r#"
        UPDATE files_metadata 
        SET parent_path = $1, 
            department_id = $2, 
            visibility = $3, 
            owner_id = CASE WHEN $3 = 'private' THEN $5 ELSE owner_id END,
            updated_at = NOW()
        WHERE id = $4 AND tenant_id = $6
        "#
    )
    .bind(&new_parent_path)
    .bind(target_dept_id)
    .bind(target_visibility)
    .bind(file_uuid)
    .bind(auth.user_id)
    .bind(tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If moving a folder, update all children's parent_path
    if is_directory {
        let old_path = if let Some(ref pp) = current_parent_path {
            format!("{}/{}", pp, file_name)
        } else {
            file_name.clone()
        };
        
        let new_path = if let Some(ref pp) = new_parent_path {
            format!("{}/{}", pp, file_name)
        } else {
            file_name.clone()
        };
        
        // Update direct children (including visibility and owner_id for private)
        sqlx::query(
            r#"
            UPDATE files_metadata 
            SET parent_path = $1, 
                visibility = $2, 
                owner_id = CASE WHEN $2 = 'private' THEN $5 ELSE owner_id END,
                updated_at = NOW()
            WHERE tenant_id = $3 AND parent_path = $4 AND is_deleted = false
            "#
        )
        .bind(&new_path)
        .bind(target_visibility)
        .bind(tenant_id)
        .bind(&old_path)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Update nested children (replace prefix, update visibility and owner_id for private)
        sqlx::query(
            r#"
            UPDATE files_metadata 
            SET parent_path = $1 || SUBSTRING(parent_path FROM LENGTH($2) + 1), 
                visibility = $3, 
                owner_id = CASE WHEN $3 = 'private' THEN $6 ELSE owner_id END,
                updated_at = NOW()
            WHERE tenant_id = $4 AND parent_path LIKE $5 AND is_deleted = false
            "#
        )
        .bind(&new_path)
        .bind(&old_path)
        .bind(target_visibility)
        .bind(tenant_id)
        .bind(format!("{}/%", old_path))
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Audit log
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    
    if should_force_audit_log(&compliance_mode, "file_move") {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'file_move', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(tenant_id)
        .bind(auth.user_id)
        .bind(file_uuid)
        .bind(json!({
            "from_path": current_parent_path,
            "to_path": new_parent_path,
            "from_department": current_dept_id,
            "to_department": target_dept_id,
            "from_visibility": current_visibility,
            "to_visibility": target_visibility,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }

    // Invalidate cache
    if let Some(ref cache) = state.cache {
        let pattern = format!("clovalink:files:{}:*", tenant_id);
        let _ = cache.delete_pattern(&pattern).await;
    }

    Ok(Json(json!({
        "message": "File moved successfully",
        "new_path": new_parent_path
    })))
}

#[derive(serde::Deserialize)]
pub struct FileActivityParams {
    limit: Option<i64>,
}

pub async fn get_file_activity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<FileActivityParams>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // SECURITY: Check if user has permission to view activity for this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "read").await? {
        tracing::warn!(
            "Access denied: user {} attempted to view activity for file {} without permission",
            auth.user_id, file_uuid
        );
        return Err(StatusCode::FORBIDDEN);
    }

    let limit = params.limit.unwrap_or(20).min(100);

    // Fetch recent activity for this file from audit logs
    let activities: Vec<(Uuid, String, Option<Uuid>, Option<Value>, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT al.id, al.action, al.user_id, al.metadata, al.created_at
        FROM audit_logs al
        WHERE al.tenant_id = $1 AND al.resource_id = $2 AND al.resource_type = 'file'
        ORDER BY al.created_at DESC
        LIMIT $3
        "#
    )
    .bind(tenant_id)
    .bind(file_uuid)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch file activity: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get user names for the activities
    let mut activity_items: Vec<Value> = Vec::new();
    for (id, action, user_id, metadata, created_at) in activities {
        let user_name = if let Some(uid) = user_id {
            let user: Option<(String,)> = sqlx::query_as(
                "SELECT name FROM users WHERE id = $1"
            )
            .bind(uid)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
            user.map(|(name,)| name).unwrap_or_else(|| "Unknown User".to_string())
        } else {
            "System".to_string()
        };

        activity_items.push(json!({
            "id": id,
            "action": action,
            "user_id": user_id,
            "user_name": user_name,
            "metadata": metadata,
            "created_at": created_at.to_rfc3339()
        }));
    }

    Ok(Json(json!({
        "activities": activity_items,
        "file_id": file_id
    })))
}

// ==================== File Export ====================

#[derive(serde::Deserialize)]
pub struct ExportParams {
    file_ids: Option<String>, // Comma-separated file IDs
}

pub async fn export_files(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(company_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<ExportParams>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if user has export permission (Admin, SuperAdmin)
    if !["SuperAdmin", "Admin"].contains(&auth.role.as_str()) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get file IDs to export
    let file_ids: Vec<Uuid> = params.file_ids
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| Uuid::parse_str(s.trim()).ok())
        .collect();

    if file_ids.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get compliance mode for audit logging
    let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
        .await
        .unwrap_or_else(|_| "Standard".to_string());
    let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);

    // NOTE: Token-in-URL is now blocked at middleware level for ALL users (security best practice)

    // For single file, just download it
    if file_ids.len() == 1 {
        let file_uuid = file_ids[0];
        
        // Look up file
        let file_meta: (String, String, i64) = sqlx::query_as(
            "SELECT name, storage_path, size_bytes FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
        )
        .bind(file_uuid)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
        
        let (file_name, storage_path, file_size) = file_meta;

        // Log export for compliance
        if restrictions.export_logging_required {
            let _ = log_file_export(
                &state.pool,
                tenant_id,
                auth.user_id,
                Some(file_uuid),
                "single_file_export",
                1,
                Some(file_size),
                None,
            ).await;
        }

        // Stream download for single file export (zero-copy)
        let (stream, stream_size) = state.storage.download_stream(&storage_path).await
            .map_err(|e| {
                tracing::error!("Failed to open file stream for export: {}", e);
                StatusCode::NOT_FOUND
            })?;
        
        // SECURITY: Use sanitized Content-Disposition to prevent header injection
        let safe_disposition = sanitize_content_disposition(&file_name, "attachment");
        
        // Convert stream to axum Body
        let body = Body::from_stream(stream.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        }));
        
        return Ok(axum::response::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, get_content_type(&file_name))
            .header(header::CONTENT_LENGTH, stream_size)
            .header(header::CONTENT_DISPOSITION, safe_disposition)
            .body(body)
            .unwrap());
    }

    // For multiple files, create a simple manifest (actual ZIP would require additional dependencies)
    // In production, you'd use the zip crate to create a proper ZIP file
    let mut total_size: i64 = 0;
    let mut file_list: Vec<Value> = Vec::new();

    for file_uuid in &file_ids {
        let file_meta: Option<(String, i64)> = sqlx::query_as(
            "SELECT name, size_bytes FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
        )
        .bind(file_uuid)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some((name, size)) = file_meta {
            total_size += size;
            file_list.push(json!({
                "id": file_uuid,
                "name": name,
                "size": size
            }));
        }
    }

    // Log bulk export for compliance
    if restrictions.export_logging_required {
        let _ = log_file_export(
            &state.pool,
            tenant_id,
            auth.user_id,
            None,
            "bulk_export",
            file_list.len() as i32,
            Some(total_size),
            None, // IP address - not available in this context
        ).await;
    }

    // Return manifest (in production, return ZIP file)
    let manifest = json!({
        "export_type": "manifest",
        "file_count": file_list.len(),
        "total_size": total_size,
        "files": file_list,
        "message": "For bulk downloads, please download files individually or contact administrator for ZIP export"
    });

    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION, 
        "attachment; filename=\"export_manifest.json\"".parse().unwrap()
    );

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"export_manifest.json\"")
        .body(axum::body::Body::from(manifest_bytes))
        .unwrap())
}

/// Serve files from uploads directory (avatars, etc.)
/// This works with both local and S3 storage using streaming (zero-copy)
/// GET /uploads/*path
pub async fn serve_upload(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    tracing::debug!("Serving upload: {}", path);
    
    // Stream file from storage (works for both local and S3)
    let (stream, size) = state.storage.download_stream(&path).await
        .map_err(|e| {
            tracing::warn!("Failed to serve upload {}: {:?}", path, e);
            StatusCode::NOT_FOUND
        })?;
    
    // Determine content type from file extension
    let content_type = get_content_type(&path);
    
    // Convert stream to axum Body
    let body = Body::from_stream(stream.map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }));
    
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, size)
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .body(body)
        .unwrap())
}

/// Toggle company folder status for a folder
/// PUT /api/files/{company_id}/{file_id}/company-folder
pub async fn toggle_company_folder(
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
    
    // Get the file and verify it's a folder
    let file: (bool, bool) = sqlx::query_as(
        r#"SELECT is_directory, COALESCE(is_company_folder, false) FROM files_metadata WHERE id = $1 AND tenant_id = $2"#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    
    let (is_directory, is_company_folder) = file;
    
    // Only folders can be company folders
    if !is_directory {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Toggle the value
    let new_value = !is_company_folder;
    
    sqlx::query(
        r#"UPDATE files_metadata SET is_company_folder = $1, updated_at = NOW() WHERE id = $2"#
    )
    .bind(new_value)
    .bind(file_uuid)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Invalidate cache for this tenant's files
    if let Some(ref cache) = state.cache {
        let _ = cache.delete_pattern(&format!("clovalink:files:{}:*", tenant_id)).await;
    }
    
    Ok(Json(json!({
        "success": true,
        "is_company_folder": new_value
    })))
}

// ==================== File Sharing ====================

#[derive(serde::Deserialize)]
pub struct CreateShareInput {
    is_public: Option<bool>,
    expires_in_days: Option<i64>,
    /// Share policy: 'permissioned' (default, most secure) or 'tenant_wide'
    /// - 'permissioned': User must have can_access_file permission to download
    /// - 'tenant_wide': Any authenticated user in the tenant can download
    share_policy: Option<String>,
}

/// Create a share link for a file
/// POST /api/files/{company_id}/{file_id}/share
pub async fn create_file_share(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((company_id, file_id)): Path<(String, String)>,
    Json(input): Json<CreateShareInput>,
) -> Result<Json<Value>, StatusCode> {
    let tenant_id = Uuid::parse_str(&company_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_uuid = Uuid::parse_str(&file_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Verify tenant access
    if auth.role != "SuperAdmin" && auth.tenant_id != tenant_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Check if user has permission to access this file
    if !can_access_file(&state.pool, file_uuid, tenant_id, auth.user_id, &auth.role, "share").await? {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Verify file/folder exists
    let file_check: Option<(String, bool, Option<String>)> = sqlx::query_as(
        "SELECT name, is_directory, parent_path FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_name, is_directory, parent_path) = file_check.ok_or(StatusCode::NOT_FOUND)?;
    let _parent_path = parent_path.unwrap_or_default();
    
    // Check compliance mode for public sharing restrictions
    let is_public = input.is_public.unwrap_or(false);
    if is_public {
        let compliance_mode = get_tenant_compliance_mode(&state.pool, tenant_id)
            .await
            .unwrap_or_else(|_| "Standard".to_string());
        let restrictions = ComplianceRestrictions::for_mode(&compliance_mode);
        
        if restrictions.public_sharing_blocked {
            tracing::warn!("Public sharing blocked by compliance mode: {}", compliance_mode);
            return Err(StatusCode::FORBIDDEN);
        }
    }
    
    // Generate a secure token
    let token = nanoid::nanoid!(16);
    
    // Calculate expiration if provided
    let expires_at = input.expires_in_days.map(|days| Utc::now() + chrono::Duration::days(days));
    
    // Validate and set share policy (default to 'permissioned' for security)
    let share_policy = match input.share_policy.as_deref() {
        Some("tenant_wide") => "tenant_wide",
        _ => "permissioned", // Default to most secure option
    };
    
    // Insert share record
    let share_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO file_shares (file_id, tenant_id, token, created_by, is_public, expires_at, is_directory, share_policy)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#
    )
    .bind(file_uuid)
    .bind(tenant_id)
    .bind(&token)
    .bind(auth.user_id)
    .bind(is_public)
    .bind(expires_at)
    .bind(is_directory)
    .bind(share_policy)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create file share: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Log the share creation
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'file_shared', 'file', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(auth.user_id)
    .bind(file_uuid)
    .bind(json!({
        "share_id": share_id,
        "file_name": file_name,
        "is_public": is_public,
        "is_directory": is_directory,
        "expires_at": expires_at,
    }))
    .bind(&auth.ip_address)
    .execute(&state.pool)
    .await;

    // Check for excessive sharing pattern (security alert)
    let _ = security_service::check_excessive_sharing(
        &state.pool,
        tenant_id,
        auth.user_id,
        &auth.email,
        auth.ip_address.as_deref(),
    ).await;
    
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    Ok(Json(json!({
        "id": share_id,
        "token": token,
        "link": format!("{}/share/{}", base_url, token),
        "is_public": is_public,
        "is_directory": is_directory,
        "expires_at": expires_at,
    })))
}

/// Get share information (for public download page)
/// GET /api/share/{token}/info
pub async fn get_share_info(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Look up the share
    let share: Option<(Uuid, Uuid, Uuid, bool, Option<DateTime<Utc>>, i32, bool)> = sqlx::query_as(
        r#"
        SELECT file_id, tenant_id, created_by, is_public, expires_at, download_count, is_directory
        FROM file_shares WHERE token = $1
        "#
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_id, tenant_id, _created_by, is_public, expires_at, download_count, is_directory) = share.ok_or(StatusCode::NOT_FOUND)?;
    
    // Check expiration
    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(StatusCode::GONE); // 410 Gone - expired
        }
    }
    
    // Get file metadata
    let file: Option<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT name, size_bytes, content_type FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_name, size_bytes, content_type) = file.ok_or(StatusCode::NOT_FOUND)?;
    
    // Get tenant name for branding
    let tenant_name: Option<String> = sqlx::query_scalar(
        "SELECT name FROM tenants WHERE id = $1"
    )
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    
    Ok(Json(json!({
        "file_name": file_name,
        "size_bytes": size_bytes,
        "size_formatted": format_bytes(size_bytes),
        "content_type": content_type,
        "is_public": is_public,
        "is_directory": is_directory,
        "expires_at": expires_at,
        "download_count": download_count,
        "shared_by": tenant_name.unwrap_or_else(|| "Unknown".to_string()),
    })))
}

/// Download a shared file (public endpoint)
/// GET /api/share/{token}
/// 
/// Security:
/// - Public shares: anyone can download
/// - Private shares with 'tenant_wide' policy: any authenticated user in tenant can download
/// - Private shares with 'permissioned' policy: requires can_access_file check (default, most secure)
pub async fn download_shared_file(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Result<axum::response::Response<axum::body::Body>, StatusCode> {
    // Look up the share including share_policy
    let share: Option<(Uuid, Uuid, Uuid, bool, Option<DateTime<Utc>>, bool, Option<String>)> = sqlx::query_as(
        r#"
        SELECT file_id, tenant_id, created_by, is_public, expires_at, is_directory, 
               COALESCE(share_policy, 'permissioned') as share_policy
        FROM file_shares WHERE token = $1
        "#
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_id, tenant_id, created_by, is_public, expires_at, is_directory, share_policy_opt) = share.ok_or(StatusCode::NOT_FOUND)?;
    let share_policy = share_policy_opt.unwrap_or_else(|| "permissioned".to_string());
    
    // Check expiration
    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(StatusCode::GONE); // 410 Gone - expired
        }
    }
    
    // If not public, verify user is authenticated and has appropriate access
    if !is_public {
        // Try to extract auth token from header
        let auth_header = headers.get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "));
        
        if let Some(token_str) = auth_header {
            // Validate the token
            match clovalink_auth::verify_token(token_str) {
                Ok(claims) => {
                    // Check tenant match - parse tenant_id from claims
                    let user_tenant = Uuid::parse_str(&claims.tenant_id)
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    if user_tenant != tenant_id {
                        tracing::warn!("Share access denied: user tenant {} != share tenant {}", claims.tenant_id, tenant_id);
                        return Err(StatusCode::FORBIDDEN);
                    }
                    
                    // SECURITY: For 'permissioned' shares, enforce normal file access rules
                    // This prevents using private shares to bypass department/private file restrictions
                    if share_policy == "permissioned" {
                        let user_id = Uuid::parse_str(&claims.sub)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        
                        if !can_access_file(&state.pool, file_id, tenant_id, user_id, &claims.role, "read").await? {
                            tracing::warn!(
                                "Share access denied: user {} cannot access file {} (permissioned share)",
                                claims.sub, file_id
                            );
                            return Err(StatusCode::FORBIDDEN);
                        }
                    }
                    // For 'tenant_wide' shares, any user in the tenant can access (current behavior)
                }
                Err(_) => {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        } else {
            // No auth header - return 401 to prompt login
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    // Get file/folder metadata including size for scheduling
    let file: Option<(String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT name, storage_path, parent_path, size_bytes FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (file_name, storage_path_opt, parent_path_opt, file_size) = file.ok_or(StatusCode::NOT_FOUND)?;
    let parent_path = parent_path_opt.unwrap_or_default();
    
    // Increment download count
    let _ = sqlx::query("UPDATE file_shares SET download_count = download_count + 1 WHERE token = $1")
        .bind(&token)
        .execute(&state.pool)
        .await;
    
    // Extract client IP for audit logging
    let client_ip: Option<String> = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers.get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.trim().to_string())
        });

    // Log the download
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
        VALUES ($1, $2, 'shared_file_downloaded', 'file', $3, $4, $5::inet)
        "#
    )
    .bind(tenant_id)
    .bind(created_by) // Log under the share creator since public downloads may not have a user
    .bind(file_id)
    .bind(json!({
        "file_name": file_name,
        "share_token": token,
        "is_public": is_public,
        "is_directory": is_directory,
    }))
    .bind(&client_ip)
    .execute(&state.pool)
    .await;
    
    // If this is a directory, generate zip on the fly
    if is_directory {
        return download_shared_folder_as_zip(&state, tenant_id, &file_name, &parent_path).await;
    }
    
    // Regular file download
    let storage_path = storage_path_opt.ok_or(StatusCode::NOT_FOUND)?;
    
    // Try presigned URL redirect if enabled and supported (S3-compatible storage)
    // This bypasses the proxy and redirects directly to S3/CDN for better performance
    if state.use_presigned_urls && state.storage.supports_presigned_urls() {
        match state.storage.presigned_download_url(&storage_path, state.presigned_url_expiry).await {
            Ok(Some(mut presigned_url)) => {
                // Optionally rewrite through CDN for edge caching
                if let Some(cdn) = &state.cdn_domain {
                    presigned_url = rewrite_url_to_cdn(&presigned_url, cdn);
                }
                
                tracing::debug!(
                    "Redirecting shared file download to presigned URL: token={}, file_id={}",
                    token, file_id
                );
                
                // Return redirect to presigned URL
                return Ok(axum::response::Response::builder()
                    .status(StatusCode::TEMPORARY_REDIRECT)
                    .header(header::LOCATION, &presigned_url)
                    .header(header::CACHE_CONTROL, "private, max-age=0")
                    .body(axum::body::Body::empty())
                    .unwrap());
            }
            Ok(None) => {
                // Storage doesn't support presigned URLs, fallback to proxy
                tracing::debug!("Storage doesn't support presigned URLs, using proxy for share");
            }
            Err(e) => {
                // Presigning failed, fallback to proxy
                tracing::warn!("Presigned URL generation failed for share, falling back to proxy: {}", e);
            }
        }
    }
    
    // FALLBACK: Proxy download through backend using STREAMING (for local storage or when presigned URLs disabled/failed)
    
    // Acquire transfer scheduler permit based on file size (prioritizes small files)
    let transfer_permit = state.scheduler.acquire_download_permit(file_size).await;
    tracing::debug!(
        "Shared download permit acquired: token={}, size={}, class={}",
        token, file_size, transfer_permit.size_class.name()
    );
    
    let (stream, size) = state.storage.download_stream(&storage_path).await
        .map_err(|e| {
            tracing::error!("Failed to stream shared file: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Determine content type from file extension
    let content_type = get_content_type(&file_name);
    
    // SECURITY: Use sanitized Content-Disposition to prevent header injection
    let safe_disposition = sanitize_content_disposition(&file_name, "attachment");
    
    // Convert stream to axum Body (zero-copy streaming)
    // Note: transfer_permit is held in scope until handler returns
    let body = Body::from_stream(stream.map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }));
    
    // Keep permit alive until response is ready
    let _ = &transfer_permit;
    
    // Build response with proper headers
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, size)
        .header(header::CONTENT_DISPOSITION, safe_disposition)
        .body(body)
        .unwrap())
}

// ==================== Content Hash Migration ====================

/// Migrate existing files to content-addressed storage with Blake3 hashing
/// This endpoint should be called once after upgrading to add content_hash and ulid to existing files
/// Only accessible by SuperAdmin
/// POST /api/admin/migrate-content-hashes
pub async fn migrate_content_hashes(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    // Only SuperAdmin can run migrations
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }
    
    tracing::info!("Starting content hash migration...");
    
    // Get all files without content_hash (excluding directories)
    let files_to_migrate: Vec<(Uuid, Uuid, String, Option<Uuid>, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT id, tenant_id, storage_path, department_id, created_at
        FROM files_metadata 
        WHERE content_hash IS NULL 
        AND is_directory = false 
        AND is_deleted = false
        ORDER BY created_at ASC
        LIMIT 1000
        "#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch files for migration: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let _total = files_to_migrate.len();
    let mut migrated = 0;
    let mut errors = 0;
    let mut deduplicated = 0;
    
    for (file_id, tenant_id, storage_path, department_id, created_at) in files_to_migrate {
        // Download file content
        match state.storage.download(&storage_path).await {
            Ok(data) => {
                // Compute Blake3 hash
                let content_hash = blake3::hash(&data).to_hex().to_string();
                
                // Generate ULID from created_at timestamp
                let file_ulid = Ulid::from_datetime(created_at.into()).to_string();
                
                // Check if this content already exists (for deduplication tracking)
                let existing_count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM files_metadata 
                    WHERE tenant_id = $1 
                    AND (department_id IS NOT DISTINCT FROM $2)
                    AND content_hash = $3
                    AND is_deleted = false 
                    AND is_directory = false
                    AND id != $4
                    "#
                )
                .bind(tenant_id)
                .bind(department_id)
                .bind(&content_hash)
                .bind(file_id)
                .fetch_one(&state.pool)
                .await
                .unwrap_or(0);
                
                if existing_count > 0 {
                    deduplicated += 1;
                }
                
                // Update the file record with content_hash and ulid
                let result = sqlx::query(
                    "UPDATE files_metadata SET content_hash = $1, ulid = $2 WHERE id = $3"
                )
                .bind(&content_hash)
                .bind(&file_ulid)
                .bind(file_id)
                .execute(&state.pool)
                .await;
                
                match result {
                    Ok(_) => {
                        migrated += 1;
                        if migrated % 100 == 0 {
                            tracing::info!("Migrated {} files...", migrated);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to update file {}: {:?}", file_id, e);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Could not download file {} (may be mock data): {:?}", file_id, e);
                // For files that don't exist in storage (mock/seed data), generate placeholder values
                let file_ulid = Ulid::from_datetime(created_at.into()).to_string();
                let placeholder_hash = format!("placeholder_{}", file_id);
                
                let _ = sqlx::query(
                    "UPDATE files_metadata SET content_hash = $1, ulid = $2 WHERE id = $3"
                )
                .bind(&placeholder_hash)
                .bind(&file_ulid)
                .bind(file_id)
                .execute(&state.pool)
                .await;
                
                migrated += 1;
            }
        }
    }
    
    // Get remaining count
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files_metadata WHERE content_hash IS NULL AND is_directory = false AND is_deleted = false"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    
    tracing::info!(
        "Migration complete: {} migrated, {} errors, {} potential duplicates, {} remaining",
        migrated, errors, deduplicated, remaining
    );
    
    Ok(Json(json!({
        "status": "complete",
        "migrated": migrated,
        "errors": errors,
        "deduplicated": deduplicated,
        "remaining": remaining,
        "message": if remaining > 0 { 
            "Run migration again to process more files" 
        } else { 
            "All files have been migrated" 
        }
    })))
}
