//! AI Data Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant AI settings from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TenantAiSettings {
    pub tenant_id: Uuid,
    pub enabled: bool,
    pub provider: String,
    #[serde(skip_serializing)]
    pub api_key_encrypted: Option<String>,
    pub allowed_roles: Vec<String>,
    pub hipaa_approved_only: bool,
    pub sox_read_only: bool,
    pub monthly_token_limit: i32,
    pub daily_request_limit: i32,
    pub tokens_used_this_month: i32,
    pub requests_today: i32,
    pub last_usage_reset: Option<chrono::NaiveDate>,
    pub maintenance_mode: bool,
    pub maintenance_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Settings response for API (with masked key)
#[derive(Debug, Clone, Serialize)]
pub struct TenantAiSettingsResponse {
    pub tenant_id: Uuid,
    pub enabled: bool,
    pub provider: String,
    pub api_key_masked: Option<String>,  // Show only last 4 chars
    pub allowed_roles: Vec<String>,
    pub hipaa_approved_only: bool,
    pub sox_read_only: bool,
    pub monthly_token_limit: i32,
    pub daily_request_limit: i32,
    pub tokens_used_this_month: i32,
    pub requests_today: i32,
    pub maintenance_mode: bool,
    pub maintenance_message: Option<String>,
}

impl From<TenantAiSettings> for TenantAiSettingsResponse {
    fn from(settings: TenantAiSettings) -> Self {
        let api_key_masked = settings.api_key_encrypted.as_ref().map(|key| {
            if key.len() > 4 {
                format!("••••••••{}", &key[key.len()-4..])
            } else {
                "••••".to_string()
            }
        });
        
        Self {
            tenant_id: settings.tenant_id,
            enabled: settings.enabled,
            provider: settings.provider,
            api_key_masked,
            allowed_roles: settings.allowed_roles,
            hipaa_approved_only: settings.hipaa_approved_only,
            sox_read_only: settings.sox_read_only,
            monthly_token_limit: settings.monthly_token_limit,
            daily_request_limit: settings.daily_request_limit,
            tokens_used_this_month: settings.tokens_used_this_month,
            requests_today: settings.requests_today,
            maintenance_mode: settings.maintenance_mode,
            maintenance_message: settings.maintenance_message,
        }
    }
}

/// Update settings input
#[derive(Debug, Deserialize)]
pub struct UpdateAiSettingsInput {
    /// Optional tenant ID for SuperAdmin to update specific tenant's settings
    pub tenant_id: Option<Uuid>,
    pub enabled: Option<bool>,
    pub provider: Option<String>,
    pub api_key: Option<String>,  // New key to set
    pub allowed_roles: Option<Vec<String>>,
    pub hipaa_approved_only: Option<bool>,
    pub sox_read_only: Option<bool>,
    pub monthly_token_limit: Option<i32>,
    pub daily_request_limit: Option<i32>,
    pub maintenance_mode: Option<bool>,
    pub maintenance_message: Option<String>,
}

/// AI usage log entry
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AiUsageLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub file_id: Option<Uuid>,
    pub action: String,
    pub provider: String,
    pub model: Option<String>,
    pub tokens_used: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub file_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Extended log with user name for display
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AiUsageLogWithUser {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub file_id: Option<Uuid>,
    pub action: String,
    pub provider: String,
    pub model: Option<String>,
    pub tokens_used: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub file_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user_name: Option<String>,
}

/// Summarize request
#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    pub file_id: Uuid,
    pub max_length: Option<u32>,  // Optional max tokens for summary
}

/// Answer request
#[derive(Debug, Deserialize)]
pub struct AnswerRequest {
    pub file_id: Uuid,
    pub question: String,
}

/// Semantic search request
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<i32>,
}

/// AI response for frontend
#[derive(Debug, Serialize)]
pub struct AiActionResponse {
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
    pub tokens_used: Option<u32>,
}

/// Search result item
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub file_id: Uuid,
    pub file_name: String,
    pub file_path: Option<String>,
    pub score: f32,
    pub snippet: Option<String>,
}

/// Usage statistics with pagination
#[derive(Debug, Serialize)]
pub struct UsageStats {
    pub tokens_used_today: i32,
    pub tokens_used_this_month: i32,
    pub requests_today: i32,
    pub monthly_token_limit: i32,
    pub daily_request_limit: i32,
    pub recent_actions: Vec<UsageLogSummary>,
    pub total_count: i64,
    pub page: i32,
    pub per_page: i32,
    pub total_pages: i32,
}

#[derive(Debug, Serialize)]
pub struct UsageLogSummary {
    pub action: String,
    pub tokens_used: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub user_name: Option<String>,
    pub file_name: Option<String>,
}

