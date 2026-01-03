//! AI API Handlers
//!
//! Provides endpoints for AI features: summarization, Q&A, semantic search, and settings management.
//! All operations respect tenant settings, role permissions, and compliance requirements.

use axum::{
    extract::{State, Extension, Query},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use clovalink_ai::{
    AiService, AiError,
    models::{
        TenantAiSettingsResponse, UpdateAiSettingsInput, SummarizeRequest,
        AnswerRequest, SearchRequest, AiActionResponse, UsageStats,
    },
};
use clovalink_auth::middleware::AuthUser;
use crate::AppState;

/// Query params for tenant-scoped AI endpoints
#[derive(Debug, Deserialize)]
pub struct TenantQuery {
    pub tenant_id: Option<Uuid>,
}

/// Query params for usage with pagination
#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub tenant_id: Option<Uuid>,
    pub page: Option<i32>,
    pub per_page: Option<i32>,
}

/// Error response for AI endpoints
#[derive(Debug, Serialize)]
pub struct AiErrorResponse {
    pub error: String,
    pub code: String,
}

/// Convert AiError to HTTP response tuple
fn ai_error_response(err: AiError) -> (StatusCode, Json<AiErrorResponse>) {
    let code = match &err {
        AiError::Disabled => "AI_DISABLED",
        AiError::NoApiKey => "NO_API_KEY",
        AiError::Forbidden => "FORBIDDEN",
        AiError::MonthlyLimitExceeded => "MONTHLY_LIMIT_EXCEEDED",
        AiError::DailyLimitExceeded => "DAILY_LIMIT_EXCEEDED",
        AiError::HipaaNotApproved => "HIPAA_NOT_APPROVED",
        AiError::SoxReadOnly => "SOX_READ_ONLY",
        AiError::MaintenanceMode(_) => "MAINTENANCE_MODE",
        AiError::ProviderError(_) => "PROVIDER_ERROR",
        AiError::NetworkError(_) => "NETWORK_ERROR",
        AiError::InvalidResponse => "INVALID_RESPONSE",
        AiError::FileNotFound => "FILE_NOT_FOUND",
        AiError::ContentExtractionFailed => "CONTENT_EXTRACTION_FAILED",
        AiError::DatabaseError(_) => "DATABASE_ERROR",
        AiError::InternalError => "INTERNAL_ERROR",
    };
    
    let status = StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    (status, Json(AiErrorResponse {
        error: err.to_string(),
        code: code.to_string(),
    }))
}

/// Check if AI features are enabled for the current user's tenant
pub async fn get_ai_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<AiStatusResponse>, (StatusCode, Json<AiErrorResponse>)> {
    let service = AiService::new(state.pool.clone());
    
    let settings = service.get_settings(auth.tenant_id)
        .await
        .map_err(ai_error_response)?;
    
    // Check if user's role has access
    let has_access = settings.allowed_roles.iter().any(|r| r == &auth.role);
    
    Ok(Json(AiStatusResponse {
        enabled: settings.enabled,
        has_access,
        provider: if settings.enabled { Some(settings.provider) } else { None },
    }))
}

#[derive(Debug, Serialize)]
pub struct AiStatusResponse {
    pub enabled: bool,
    pub has_access: bool,
    pub provider: Option<String>,
}

/// Get AI settings (Admin or SuperAdmin)
/// - Admin can only view their own tenant's settings
/// - SuperAdmin can view any tenant's settings via tenant_id query param
pub async fn get_ai_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<TenantAiSettingsResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // Admin or SuperAdmin only
    if auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(ai_error_response(AiError::Forbidden));
    }
    
    // Determine which tenant to get settings for
    let target_tenant_id = if let Some(tid) = query.tenant_id {
        // Only SuperAdmin can view other tenants' settings
        if auth.role != "SuperAdmin" && tid != auth.tenant_id {
            return Err(ai_error_response(AiError::Forbidden));
        }
        tid
    } else {
        auth.tenant_id
    };
    
    let service = AiService::new(state.pool.clone());
    let settings = service.get_settings(target_tenant_id).await.map_err(ai_error_response)?;
    
    Ok(Json(TenantAiSettingsResponse::from(settings)))
}

/// Update AI settings (Admin or SuperAdmin)
/// - Admin can only update their own tenant's settings
/// - SuperAdmin can update any tenant's settings via tenant_id in body
pub async fn update_ai_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<UpdateAiSettingsInput>,
) -> Result<Json<TenantAiSettingsResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // Admin or SuperAdmin only
    if auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(ai_error_response(AiError::Forbidden));
    }
    
    // Determine which tenant to update settings for
    let target_tenant_id = if let Some(tid) = input.tenant_id {
        // Only SuperAdmin can update other tenants' settings
        if auth.role != "SuperAdmin" && tid != auth.tenant_id {
            return Err(ai_error_response(AiError::Forbidden));
        }
        tid
    } else {
        auth.tenant_id
    };
    
    let service = AiService::new(state.pool.clone());
    let settings = service.update_settings(target_tenant_id, input).await.map_err(ai_error_response)?;
    
    tracing::info!(
        tenant_id = %target_tenant_id,
        user_id = %auth.user_id,
        "AI settings updated"
    );
    
    Ok(Json(TenantAiSettingsResponse::from(settings)))
}

/// Test AI provider connection (Admin or SuperAdmin)
/// - Admin can only test their own tenant's connection
/// - SuperAdmin can test any tenant's connection via tenant_id query param
pub async fn test_ai_connection(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<TestConnectionResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // Admin or SuperAdmin only
    if auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(ai_error_response(AiError::Forbidden));
    }
    
    // Determine which tenant to test connection for
    let target_tenant_id = if let Some(tid) = query.tenant_id {
        // Only SuperAdmin can test other tenants' connections
        if auth.role != "SuperAdmin" && tid != auth.tenant_id {
            return Err(ai_error_response(AiError::Forbidden));
        }
        tid
    } else {
        auth.tenant_id
    };
    
    let service = AiService::new(state.pool.clone());
    let success = service.test_connection(target_tenant_id).await.map_err(ai_error_response)?;
    
    Ok(Json(TestConnectionResponse { success }))
}

#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
}

/// Get AI usage statistics with pagination (Admin or SuperAdmin)
/// - Admin can only view their own tenant's usage
/// - SuperAdmin can view any tenant's usage via tenant_id query param
pub async fn get_ai_usage(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsageStats>, (StatusCode, Json<AiErrorResponse>)> {
    // Admin or SuperAdmin only
    if auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(ai_error_response(AiError::Forbidden));
    }
    
    // Determine which tenant to get usage for
    let target_tenant_id = if let Some(tid) = query.tenant_id {
        // Only SuperAdmin can view other tenants' usage
        if auth.role != "SuperAdmin" && tid != auth.tenant_id {
            return Err(ai_error_response(AiError::Forbidden));
        }
        tid
    } else {
        auth.tenant_id
    };
    
    // Pagination defaults
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);
    
    let service = AiService::new(state.pool.clone());
    let stats = service.get_usage_stats(target_tenant_id, page, per_page).await.map_err(ai_error_response)?;
    
    Ok(Json(stats))
}

/// Summarize a file (with caching to avoid repeated API calls)
/// Cache is served even during maintenance mode to avoid re-calling the API
pub async fn summarize_file(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(request): Json<SummarizeRequest>,
) -> Result<Json<AiActionResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // Get file info for logging
    let file_info = sqlx::query_as::<_, FileNameRecord>(
        "SELECT name FROM files_metadata WHERE id = $1 AND tenant_id = $2"
    )
    .bind(request.file_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let file_name = file_info.map(|f| f.name);
    
    // Get file content (includes permission check)
    let content = get_file_content(&state, auth.tenant_id, request.file_id, auth.user_id, &auth.role).await.map_err(ai_error_response)?;
    
    // Calculate content hash to detect changes
    use sha2::{Sha256, Digest};
    let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    
    // Check for cached summary BEFORE maintenance mode check
    // This allows returning cached summaries even during maintenance
    let cached = sqlx::query_as::<_, CachedSummary>(
        "SELECT summary, content_hash FROM file_summaries WHERE file_id = $1 AND tenant_id = $2"
    )
    .bind(request.file_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ai_error_response(AiError::DatabaseError(e.to_string())))?;
    
    // Return cached summary if content hasn't changed
    if let Some(cache) = cached {
        if cache.content_hash == content_hash {
            // Log the view even for cached summaries
            let _ = sqlx::query(
                r#"
                INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
                VALUES ($1, $2, 'ai_summary_viewed', 'file', $3, $4, $5::inet)
                "#
            )
            .bind(auth.tenant_id)
            .bind(auth.user_id)
            .bind(request.file_id)
            .bind(serde_json::json!({
                "file_name": file_name,
                "cached": true,
            }))
            .bind(&auth.ip_address)
            .execute(&state.pool)
            .await;
            
            return Ok(Json(AiActionResponse {
                success: true,
                content: Some(cache.summary),
                error: None,
                tokens_used: Some(0), // Cached, no tokens used
            }));
        }
    }
    
    // No valid cache - check maintenance mode before making new API call
    let service = AiService::new(state.pool.clone());
    let (is_maintenance, maintenance_msg) = service.is_maintenance_mode(auth.tenant_id)
        .await
        .map_err(ai_error_response)?;
    
    if is_maintenance {
        let msg = maintenance_msg.unwrap_or_else(|| 
            "AI features are temporarily unavailable for maintenance. Please try again later.".to_string()
        );
        return Err(ai_error_response(AiError::MaintenanceMode(msg)));
    }
    
    // Call AI
    let response = service.summarize(
        auth.tenant_id,
        auth.user_id,
        &auth.role,
        request.file_id,
        &content,
        request.max_length,
    ).await.map_err(ai_error_response)?;
    
    // Cache the new summary
    if response.success {
        if let Some(ref summary) = response.content {
            let _ = sqlx::query(
                r#"
                INSERT INTO file_summaries (file_id, tenant_id, summary, content_hash)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (file_id) DO UPDATE SET
                    summary = EXCLUDED.summary,
                    content_hash = EXCLUDED.content_hash,
                    updated_at = NOW()
                "#
            )
            .bind(request.file_id)
            .bind(auth.tenant_id)
            .bind(summary)
            .bind(&content_hash)
            .execute(&state.pool)
            .await;
        }
        
        // Log to main audit_logs table
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'ai_summarize', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(auth.tenant_id)
        .bind(auth.user_id)
        .bind(request.file_id)
        .bind(serde_json::json!({
            "file_name": file_name,
            "tokens_used": response.tokens_used,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }
    
    Ok(Json(response))
}

#[derive(Debug, sqlx::FromRow)]
struct FileNameRecord {
    name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct CachedSummary {
    summary: String,
    content_hash: String,
}

/// Answer a question about a file
pub async fn answer_question(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(request): Json<AnswerRequest>,
) -> Result<Json<AiActionResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // Get file info for logging
    let file_info = sqlx::query_as::<_, FileNameRecord>(
        "SELECT name FROM files_metadata WHERE id = $1 AND tenant_id = $2"
    )
    .bind(request.file_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let file_name = file_info.map(|f| f.name);
    
    // Get file content (includes permission check)
    let content = get_file_content(&state, auth.tenant_id, request.file_id, auth.user_id, &auth.role).await.map_err(ai_error_response)?;
    
    let service = AiService::new(state.pool.clone());
    let response = service.answer(
        auth.tenant_id,
        auth.user_id,
        &auth.role,
        request.file_id,
        &content,
        &request.question,
    ).await.map_err(ai_error_response)?;
    
    // Log to main audit_logs table (without the question content for privacy)
    if response.success {
        let _ = sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, metadata, ip_address)
            VALUES ($1, $2, 'ai_answer', 'file', $3, $4, $5::inet)
            "#
        )
        .bind(auth.tenant_id)
        .bind(auth.user_id)
        .bind(request.file_id)
        .bind(serde_json::json!({
            "file_name": file_name,
            "tokens_used": response.tokens_used,
        }))
        .bind(&auth.ip_address)
        .execute(&state.pool)
        .await;
    }
    
    Ok(Json(response))
}

/// Semantic search across files
pub async fn semantic_search(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(_request): Json<SearchRequest>,
) -> Result<Json<SemanticSearchResponse>, (StatusCode, Json<AiErrorResponse>)> {
    // For now, return a placeholder - full implementation requires embeddings index
    // This would need to:
    // 1. Embed the query
    // 2. Search file_embeddings table using vector similarity
    // 3. Return matching files
    
    let service = AiService::new(state.pool.clone());
    
    // Check if user has access first
    let settings = service.get_settings(auth.tenant_id).await.map_err(ai_error_response)?;
    if !settings.enabled {
        return Err(ai_error_response(AiError::Disabled));
    }
    if !settings.allowed_roles.iter().any(|r| r == &auth.role) {
        return Err(ai_error_response(AiError::Forbidden));
    }
    
    // Placeholder: return empty results
    // Full implementation would query file_embeddings with vector similarity
    Ok(Json(SemanticSearchResponse {
        results: vec![],
        message: Some("Semantic search is available once files have been indexed. Use the 'Summarize' feature on files to build the search index.".to_string()),
    }))
}

#[derive(Debug, Serialize)]
pub struct SemanticSearchResponse {
    pub results: Vec<SearchResultItem>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResultItem {
    pub file_id: String,
    pub file_name: String,
    pub file_path: Option<String>,
    pub score: f32,
    pub snippet: Option<String>,
}

/// Get available AI providers
pub async fn get_providers() -> Json<ProvidersResponse> {
    Json(ProvidersResponse {
        providers: vec![
            ProviderInfo {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                hipaa_approved: true,
                models: vec!["gpt-4o-mini".to_string(), "gpt-4o".to_string()],
            },
            // Add more providers here as they're implemented
        ],
    })
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub hipaa_approved: bool,
    pub models: Vec<String>,
}

/// Helper: Get file content from storage
/// Includes security check to ensure user has permission to access the file
async fn get_file_content(
    state: &AppState,
    tenant_id: Uuid,
    file_id: Uuid,
    user_id: Uuid,
    user_role: &str,
) -> Result<String, AiError> {
    // SECURITY: Verify user has permission to access this file
    // This checks: Admin bypass, file locks, private file ownership, department membership
    let has_access = crate::handlers::can_access_file(
        &state.pool,
        file_id,
        tenant_id,
        user_id,
        user_role,
        "read",
    )
    .await
    .map_err(|_| AiError::Forbidden)?;
    
    if !has_access {
        tracing::warn!(
            "AI access denied: user {} attempted to access file {} without permission",
            user_id, file_id
        );
        return Err(AiError::Forbidden);
    }
    
    // Get file metadata
    let file = sqlx::query_as::<_, FileRecord>(
        "SELECT id, name, storage_path, content_type FROM files_metadata WHERE id = $1 AND tenant_id = $2 AND is_deleted = false"
    )
    .bind(file_id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AiError::DatabaseError(e.to_string()))?
    .ok_or(AiError::FileNotFound)?;
    
    // Check if format is supported for text extraction
    let mime = file.content_type.as_deref().unwrap_or("application/octet-stream");
    if !crate::text_extract::is_extractable(mime) {
        return Err(AiError::ContentExtractionFailed);
    }
    
    // Download content
    let bytes = state.storage.download(&file.storage_path)
        .await
        .map_err(|e| {
            tracing::error!("Failed to download file for AI: {:?}", e);
            AiError::ContentExtractionFailed
        })?;
    
    // Extract text based on file type (PDF, Office docs, plain text, etc.)
    let content = crate::text_extract::extract_text(&bytes, mime)
        .map_err(|e| {
            tracing::warn!("Text extraction failed for {}: {}", mime, e);
            AiError::ContentExtractionFailed
        })?;
    
    // Limit content size (max 100KB for AI processing)
    const MAX_CONTENT_SIZE: usize = 100 * 1024;
    if content.len() > MAX_CONTENT_SIZE {
        Ok(content.chars().take(MAX_CONTENT_SIZE).collect())
    } else {
        Ok(content)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct FileRecord {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    name: String,
    storage_path: String,
    content_type: Option<String>,
}
