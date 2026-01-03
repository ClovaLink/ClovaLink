//! AI Service Layer
//! 
//! Orchestrates AI operations with all required guards:
//! - Tenant enablement check
//! - API key validation
//! - Role-based access
//! - Compliance enforcement (HIPAA/SOX)
//! - Usage limits
//! - PII redaction
//! - Audit logging

use sqlx::PgPool;
use uuid::Uuid;
use crate::error::AiError;
use crate::models::*;
use crate::provider::{AiProvider, ProviderRegistry};
use crate::redact::RedactionService;

/// AI Service - main entry point for AI operations
pub struct AiService {
    pool: PgPool,
}

impl AiService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get or create tenant AI settings
    pub async fn get_settings(&self, tenant_id: Uuid) -> Result<TenantAiSettings, AiError> {
        let settings = sqlx::query_as::<_, TenantAiSettings>(
            "SELECT * FROM tenant_ai_settings WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        match settings {
            Some(s) => Ok(s),
            None => {
                // Create default settings (disabled)
                sqlx::query(
                    "INSERT INTO tenant_ai_settings (tenant_id) VALUES ($1) ON CONFLICT DO NOTHING"
                )
                .bind(tenant_id)
                .execute(&self.pool)
                .await
                .map_err(|e| AiError::DatabaseError(e.to_string()))?;
                
                sqlx::query_as::<_, TenantAiSettings>(
                    "SELECT * FROM tenant_ai_settings WHERE tenant_id = $1"
                )
                .bind(tenant_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AiError::DatabaseError(e.to_string()))
            }
        }
    }
    
    /// Update tenant AI settings (SuperAdmin only - checked at handler level)
    pub async fn update_settings(
        &self,
        tenant_id: Uuid,
        input: UpdateAiSettingsInput,
    ) -> Result<TenantAiSettings, AiError> {
        // Ensure settings row exists
        let _ = self.get_settings(tenant_id).await?;
        
        // Build dynamic update
        let mut updates = Vec::new();
        let mut param_idx = 2;
        
        if input.enabled.is_some() {
            updates.push(format!("enabled = ${}", param_idx));
            param_idx += 1;
        }
        if input.provider.is_some() {
            updates.push(format!("provider = ${}", param_idx));
            param_idx += 1;
        }
        if input.api_key.is_some() {
            updates.push(format!("api_key_encrypted = ${}", param_idx));
            param_idx += 1;
        }
        if input.allowed_roles.is_some() {
            updates.push(format!("allowed_roles = ${}", param_idx));
            param_idx += 1;
        }
        if input.hipaa_approved_only.is_some() {
            updates.push(format!("hipaa_approved_only = ${}", param_idx));
            param_idx += 1;
        }
        if input.sox_read_only.is_some() {
            updates.push(format!("sox_read_only = ${}", param_idx));
            param_idx += 1;
        }
        if input.monthly_token_limit.is_some() {
            updates.push(format!("monthly_token_limit = ${}", param_idx));
            param_idx += 1;
        }
        if input.daily_request_limit.is_some() {
            updates.push(format!("daily_request_limit = ${}", param_idx));
            param_idx += 1;
        }
        if input.maintenance_mode.is_some() {
            updates.push(format!("maintenance_mode = ${}", param_idx));
            param_idx += 1;
        }
        if input.maintenance_message.is_some() {
            updates.push(format!("maintenance_message = ${}", param_idx));
            let _ = param_idx; // Suppress unused warning
        }
        
        if updates.is_empty() {
            return self.get_settings(tenant_id).await;
        }
        
        updates.push("updated_at = NOW()".to_string());
        
        let query = format!(
            "UPDATE tenant_ai_settings SET {} WHERE tenant_id = $1 RETURNING *",
            updates.join(", ")
        );
        
        let mut db_query = sqlx::query_as::<_, TenantAiSettings>(&query)
            .bind(tenant_id);
        
        if let Some(v) = input.enabled {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.provider {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.api_key {
            // In production, encrypt this before storing
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.allowed_roles {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.hipaa_approved_only {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.sox_read_only {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.monthly_token_limit {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.daily_request_limit {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.maintenance_mode {
            db_query = db_query.bind(v);
        }
        if let Some(v) = input.maintenance_message {
            db_query = db_query.bind(v);
        }
        
        db_query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AiError::DatabaseError(e.to_string()))
    }
    
    /// Run all pre-flight checks before an AI operation
    async fn preflight_check(
        &self,
        tenant_id: Uuid,
        _user_id: Uuid,
        user_role: &str,
        action: &str,
    ) -> Result<(TenantAiSettings, Box<dyn AiProvider>), AiError> {
        let settings = self.get_settings(tenant_id).await?;
        
        // 1. Check if AI is enabled
        if !settings.enabled {
            return Err(AiError::Disabled);
        }
        
        // 2. Check maintenance mode
        if settings.maintenance_mode {
            let msg = settings.maintenance_message.clone()
                .unwrap_or_else(|| "AI features are temporarily unavailable for maintenance. Please try again later.".to_string());
            return Err(AiError::MaintenanceMode(msg));
        }
        
        // 3. Check API key exists
        let api_key = settings.api_key_encrypted.as_ref()
            .filter(|k| !k.is_empty())
            .ok_or(AiError::NoApiKey)?;
        
        // 4. Check user role is allowed
        if !settings.allowed_roles.iter().any(|r| r == user_role) {
            return Err(AiError::Forbidden);
        }
        
        // 5. Get provider
        let provider = ProviderRegistry::get(&settings.provider, api_key)
            .ok_or_else(|| AiError::ProviderError(format!("Unknown provider: {}", settings.provider)))?;
        
        // 6. HIPAA compliance check
        if settings.hipaa_approved_only && !provider.is_hipaa_approved() {
            return Err(AiError::HipaaNotApproved);
        }
        
        // 7. SOX compliance check (read-only mode)
        if settings.sox_read_only && (action == "summarize" || action == "answer") {
            return Err(AiError::SoxReadOnly);
        }
        
        // 8. Reset daily counter if new day
        self.maybe_reset_daily_counter(tenant_id, &settings).await?;
        
        // 9. Check usage limits
        if settings.requests_today >= settings.daily_request_limit {
            return Err(AiError::DailyLimitExceeded);
        }
        if settings.tokens_used_this_month >= settings.monthly_token_limit {
            return Err(AiError::MonthlyLimitExceeded);
        }
        
        Ok((settings, provider))
    }
    
    /// Check if maintenance mode is active (for handlers that need to check before using cache)
    pub async fn is_maintenance_mode(&self, tenant_id: Uuid) -> Result<(bool, Option<String>), AiError> {
        let settings = self.get_settings(tenant_id).await?;
        Ok((settings.maintenance_mode, settings.maintenance_message))
    }
    
    async fn maybe_reset_daily_counter(&self, tenant_id: Uuid, settings: &TenantAiSettings) -> Result<(), AiError> {
        let today = chrono::Utc::now().date_naive();
        if settings.last_usage_reset.map(|d| d < today).unwrap_or(true) {
            sqlx::query(
                "UPDATE tenant_ai_settings SET requests_today = 0, last_usage_reset = $2 WHERE tenant_id = $1"
            )
            .bind(tenant_id)
            .bind(today)
            .execute(&self.pool)
            .await
            .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }
    
    /// Log AI usage (without content)
    pub async fn log_usage(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        file_id: Option<Uuid>,
        file_name: Option<&str>,
        action: &str,
        provider: &str,
        model: Option<&str>,
        tokens_used: i32,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), AiError> {
        sqlx::query(
            r#"
            INSERT INTO ai_usage_logs 
            (tenant_id, user_id, file_id, file_name, action, provider, model, tokens_used, status, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(file_id)
        .bind(file_name)
        .bind(action)
        .bind(provider)
        .bind(model)
        .bind(tokens_used)
        .bind(status)
        .bind(error_message)
        .execute(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Update usage counters
    async fn update_usage_counters(&self, tenant_id: Uuid, tokens: i32) -> Result<(), AiError> {
        sqlx::query(
            r#"
            UPDATE tenant_ai_settings 
            SET tokens_used_this_month = tokens_used_this_month + $2,
                requests_today = requests_today + 1,
                updated_at = NOW()
            WHERE tenant_id = $1
            "#
        )
        .bind(tenant_id)
        .bind(tokens)
        .execute(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Summarize file content
    pub async fn summarize(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        user_role: &str,
        file_id: Uuid,
        content: &str,
        max_tokens: Option<u32>,
    ) -> Result<AiActionResponse, AiError> {
        let (settings, provider) = self.preflight_check(tenant_id, user_id, user_role, "summarize").await?;
        
        // Redact PII
        let redacted_content = RedactionService::redact(content);
        
        // Chunk if too long (max ~4000 tokens for context)
        let chunks = RedactionService::chunk_text(&redacted_content, 4000);
        let chunk_to_summarize = chunks.first().cloned().unwrap_or_default();
        
        match provider.summarize(&chunk_to_summarize, max_tokens.unwrap_or(500)).await {
            Ok(response) => {
                self.update_usage_counters(tenant_id, response.tokens_used as i32).await?;
                self.log_usage(
                    tenant_id, user_id, Some(file_id), None, "summarize",
                    &settings.provider, Some(&response.model),
                    response.tokens_used as i32, "success", None
                ).await?;
                
                Ok(AiActionResponse {
                    success: true,
                    content: Some(response.content),
                    error: None,
                    tokens_used: Some(response.tokens_used),
                })
            }
            Err(e) => {
                self.log_usage(
                    tenant_id, user_id, Some(file_id), None, "summarize",
                    &settings.provider, None, 0, "error", Some(&e.to_string())
                ).await?;
                
                Err(e)
            }
        }
    }
    
    /// Answer question about file content
    pub async fn answer(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        user_role: &str,
        file_id: Uuid,
        content: &str,
        question: &str,
    ) -> Result<AiActionResponse, AiError> {
        let (settings, provider) = self.preflight_check(tenant_id, user_id, user_role, "answer").await?;
        
        // Redact PII from both content and question
        let redacted_content = RedactionService::redact(content);
        let redacted_question = RedactionService::redact(question);
        
        // Use only first chunk as context
        let chunks = RedactionService::chunk_text(&redacted_content, 3000);
        let context = chunks.first().cloned().unwrap_or_default();
        
        match provider.answer(&redacted_question, &context).await {
            Ok(response) => {
                self.update_usage_counters(tenant_id, response.tokens_used as i32).await?;
                self.log_usage(
                    tenant_id, user_id, Some(file_id), None, "answer",
                    &settings.provider, Some(&response.model),
                    response.tokens_used as i32, "success", None
                ).await?;
                
                Ok(AiActionResponse {
                    success: true,
                    content: Some(response.content),
                    error: None,
                    tokens_used: Some(response.tokens_used),
                })
            }
            Err(e) => {
                self.log_usage(
                    tenant_id, user_id, Some(file_id), None, "answer",
                    &settings.provider, None, 0, "error", Some(&e.to_string())
                ).await?;
                
                Err(e)
            }
        }
    }
    
    /// Test provider connection
    pub async fn test_connection(
        &self,
        tenant_id: Uuid,
    ) -> Result<bool, AiError> {
        let settings = self.get_settings(tenant_id).await?;
        
        let api_key = settings.api_key_encrypted.as_ref()
            .filter(|k| !k.is_empty())
            .ok_or(AiError::NoApiKey)?;
        
        let provider = ProviderRegistry::get(&settings.provider, api_key)
            .ok_or_else(|| AiError::ProviderError(format!("Unknown provider: {}", settings.provider)))?;
        
        provider.test_connection().await
    }
    
    /// Get usage statistics with pagination
    pub async fn get_usage_stats(&self, tenant_id: Uuid, page: i32, per_page: i32) -> Result<UsageStats, AiError> {
        let settings = self.get_settings(tenant_id).await?;
        
        // Get today's token usage
        let today_usage: Option<(i64,)> = sqlx::query_as(
            "SELECT COALESCE(SUM(tokens_used), 0) FROM ai_usage_logs WHERE tenant_id = $1 AND created_at >= CURRENT_DATE"
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        // Get total count for pagination
        let total_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ai_usage_logs WHERE tenant_id = $1"
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        let offset = (page - 1) * per_page;
        let total_pages = ((total_count.0 as f64) / (per_page as f64)).ceil() as i32;
        
        // Get paginated actions with user names
        let recent: Vec<AiUsageLogWithUser> = sqlx::query_as(
            r#"
            SELECT 
                l.id, l.tenant_id, l.user_id, l.file_id, l.action, l.provider, 
                l.model, l.tokens_used, l.status, l.error_message, l.file_name, l.created_at,
                u.name as user_name
            FROM ai_usage_logs l
            LEFT JOIN users u ON l.user_id = u.id
            WHERE l.tenant_id = $1 
            ORDER BY l.created_at DESC 
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(tenant_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AiError::DatabaseError(e.to_string()))?;
        
        Ok(UsageStats {
            tokens_used_today: today_usage.map(|t| t.0 as i32).unwrap_or(0),
            tokens_used_this_month: settings.tokens_used_this_month,
            requests_today: settings.requests_today,
            monthly_token_limit: settings.monthly_token_limit,
            daily_request_limit: settings.daily_request_limit,
            recent_actions: recent.into_iter().map(|log| UsageLogSummary {
                action: log.action,
                tokens_used: log.tokens_used,
                status: log.status,
                created_at: log.created_at,
                user_name: log.user_name,
                file_name: log.file_name,
            }).collect(),
            total_count: total_count.0,
            page,
            per_page,
            total_pages,
        })
    }
}

