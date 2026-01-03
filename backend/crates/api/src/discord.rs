//! Discord OAuth and DM Notification Handlers
//!
//! Provides endpoints for:
//! - OAuth flow (connect/disconnect Discord account)
//! - Notification preferences
//! - Sending DM notifications

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json, Redirect},
    Extension,
};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::middleware::AuthUser;
use crate::AppState;

// ==================== Configuration ====================

/// Discord OAuth configuration from environment
#[derive(Clone)]
pub struct DiscordConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl DiscordConfig {
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("DISCORD_CLIENT_ID").ok()?;
        let client_secret = std::env::var("DISCORD_CLIENT_SECRET").ok()?;
        let redirect_uri = std::env::var("DISCORD_REDIRECT_URI").ok()?;
        
        Some(Self {
            client_id,
            client_secret,
            redirect_uri,
        })
    }
}

// ==================== Models ====================

#[derive(Debug, Serialize)]
pub struct DiscordConnectionStatus {
    pub connected: bool,
    pub discord_username: Option<String>,
    pub discord_avatar_url: Option<String>,
    pub dm_notifications_enabled: bool,
    pub notify_file_shared: bool,
    pub notify_file_uploaded: bool,
    pub notify_comments: bool,
    pub notify_file_requests: bool,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesInput {
    pub dm_notifications_enabled: Option<bool>,
    pub notify_file_shared: Option<bool>,
    pub notify_file_uploaded: Option<bool>,
    pub notify_comments: Option<bool>,
    pub notify_file_requests: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TenantDiscordSettings {
    pub enabled: bool,
}

// Discord API response types
#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    discriminator: String,
    avatar: Option<String>,
}

// ==================== Tenant Settings ====================

/// Check if Discord is enabled for the tenant
/// GET /api/discord/settings
pub async fn get_discord_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<TenantDiscordSettings>, StatusCode> {
    let settings: Option<(bool,)> = sqlx::query_as(
        "SELECT enabled FROM tenant_discord_settings WHERE tenant_id = $1"
    )
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(TenantDiscordSettings {
        enabled: settings.map(|s| s.0).unwrap_or(false),
    }))
}

/// Update Discord settings (Admin only)
/// POST /api/discord/settings/update
pub async fn update_discord_settings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if auth.role != "Admin" && auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let enabled = input["enabled"].as_bool().unwrap_or(false);
    
    sqlx::query(
        r#"
        INSERT INTO tenant_discord_settings (tenant_id, enabled)
        VALUES ($1, $2)
        ON CONFLICT (tenant_id) DO UPDATE SET
            enabled = EXCLUDED.enabled,
            updated_at = NOW()
        "#
    )
    .bind(auth.tenant_id)
    .bind(enabled)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({ "enabled": enabled })))
}

// ==================== User Connection Status ====================

/// Get user's Discord connection status
/// GET /api/discord/status
pub async fn get_connection_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<DiscordConnectionStatus>, StatusCode> {
    let connection: Option<(String, Option<String>, bool, bool, bool, bool, bool)> = sqlx::query_as(
        r#"
        SELECT 
            discord_username,
            discord_avatar,
            dm_notifications_enabled,
            notify_file_shared,
            notify_file_uploaded,
            notify_comments,
            notify_file_requests
        FROM user_discord_connections
        WHERE user_id = $1
        "#
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match connection {
        Some((username, avatar, dm_enabled, file_shared, file_uploaded, comments, file_requests)) => {
            let avatar_url = avatar.map(|a| format!("https://cdn.discordapp.com/avatars/{}/{}.png", username, a));
            Ok(Json(DiscordConnectionStatus {
                connected: true,
                discord_username: Some(username),
                discord_avatar_url: avatar_url,
                dm_notifications_enabled: dm_enabled,
                notify_file_shared: file_shared,
                notify_file_uploaded: file_uploaded,
                notify_comments: comments,
                notify_file_requests: file_requests,
            }))
        }
        None => {
            Ok(Json(DiscordConnectionStatus {
                connected: false,
                discord_username: None,
                discord_avatar_url: None,
                dm_notifications_enabled: true,
                notify_file_shared: true,
                notify_file_uploaded: true,
                notify_comments: true,
                notify_file_requests: true,
            }))
        }
    }
}

// ==================== OAuth Flow ====================

/// Start Discord OAuth flow
/// GET /api/discord/connect
pub async fn start_oauth(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Redirect, (StatusCode, Json<Value>)> {
    // Check if Discord is configured
    let config = DiscordConfig::from_env().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Discord integration is not configured"
        })))
    })?;
    
    // Check if Discord is enabled for tenant
    let enabled: Option<(bool,)> = sqlx::query_as(
        "SELECT enabled FROM tenant_discord_settings WHERE tenant_id = $1"
    )
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;
    
    if !enabled.map(|e| e.0).unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, Json(json!({
            "error": "Discord is not enabled for your organization"
        }))));
    }
    
    // Generate state token for CSRF protection
    let state_token = format!("{}", Uuid::new_v4());
    let expires_at = Utc::now() + Duration::minutes(10);
    
    sqlx::query(
        "INSERT INTO discord_oauth_states (state, user_id, tenant_id, expires_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(&state_token)
    .bind(auth.user_id)
    .bind(auth.tenant_id)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create OAuth state"}))))?;
    
    // Build Discord OAuth URL
    let oauth_url = format!(
        "https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify&state={}",
        config.client_id,
        urlencoding::encode(&config.redirect_uri),
        state_token
    );
    
    Ok(Redirect::temporary(&oauth_url))
}

/// Handle Discord OAuth callback
/// GET /api/discord/callback
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OAuthCallbackParams>,
) -> Result<Redirect, (StatusCode, String)> {
    // Check for OAuth error
    if let Some(error) = params.error {
        tracing::warn!("Discord OAuth error: {}", error);
        return Ok(Redirect::temporary("/settings?discord=error"));
    }
    
    let code = params.code.ok_or((StatusCode::BAD_REQUEST, "Missing code".to_string()))?;
    let state_token = params.state.ok_or((StatusCode::BAD_REQUEST, "Missing state".to_string()))?;
    
    // Validate state token
    let state_record: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT user_id, tenant_id FROM discord_oauth_states WHERE state = $1 AND expires_at > NOW()"
    )
    .bind(&state_token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;
    
    let (user_id, tenant_id) = state_record.ok_or((StatusCode::BAD_REQUEST, "Invalid or expired state".to_string()))?;
    
    // Delete used state token
    let _ = sqlx::query("DELETE FROM discord_oauth_states WHERE state = $1")
        .bind(&state_token)
        .execute(&state.pool)
        .await;
    
    // Get Discord config
    let config = DiscordConfig::from_env().ok_or((StatusCode::SERVICE_UNAVAILABLE, "Discord not configured".to_string()))?;
    
    // Exchange code for token
    let client = Client::new();
    let token_response = client
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &config.redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Discord API error: {}", e)))?;
    
    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_default();
        tracing::error!("Discord token exchange failed: {}", error_text);
        return Ok(Redirect::temporary("/settings?discord=error"));
    }
    
    let tokens: DiscordTokenResponse = token_response
        .json()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "Invalid Discord response".to_string()))?;
    
    // Get user info from Discord
    let user_response = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bearer {}", tokens.access_token))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Discord API error: {}", e)))?;
    
    if !user_response.status().is_success() {
        return Ok(Redirect::temporary("/settings?discord=error"));
    }
    
    let discord_user: DiscordUser = user_response
        .json()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "Invalid Discord user response".to_string()))?;
    
    // Calculate token expiration
    let expires_at = Utc::now() + Duration::seconds(tokens.expires_in);
    
    // Store connection (upsert)
    // NOTE: In production, encrypt access_token and refresh_token before storing
    sqlx::query(
        r#"
        INSERT INTO user_discord_connections 
        (user_id, tenant_id, discord_user_id, discord_username, discord_discriminator, discord_avatar, 
         access_token_encrypted, refresh_token_encrypted, token_expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (user_id) DO UPDATE SET
            discord_user_id = EXCLUDED.discord_user_id,
            discord_username = EXCLUDED.discord_username,
            discord_discriminator = EXCLUDED.discord_discriminator,
            discord_avatar = EXCLUDED.discord_avatar,
            access_token_encrypted = EXCLUDED.access_token_encrypted,
            refresh_token_encrypted = EXCLUDED.refresh_token_encrypted,
            token_expires_at = EXCLUDED.token_expires_at,
            updated_at = NOW()
        "#
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(&discord_user.id)
    .bind(&discord_user.username)
    .bind(&discord_user.discriminator)
    .bind(&discord_user.avatar)
    .bind(&tokens.access_token)  // TODO: Encrypt in production
    .bind(&tokens.refresh_token) // TODO: Encrypt in production
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store Discord connection: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save connection".to_string())
    })?;
    
    tracing::info!(user_id = %user_id, discord_user = %discord_user.username, "Discord account connected");
    
    Ok(Redirect::temporary("/settings?discord=connected"))
}

/// Disconnect Discord account
/// POST /api/discord/disconnect
pub async fn disconnect(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query("DELETE FROM user_discord_connections WHERE user_id = $1")
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    tracing::info!(user_id = %auth.user_id, "Discord account disconnected");
    
    Ok(Json(json!({ "success": true })))
}

// ==================== Notification Preferences ====================

/// Update notification preferences
/// POST /api/discord/preferences
pub async fn update_preferences(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let dm_notifications_enabled = input.get("dm_notifications_enabled").and_then(|v| v.as_bool());
    let notify_file_shared = input.get("notify_file_shared").and_then(|v| v.as_bool());
    let notify_file_uploaded = input.get("notify_file_uploaded").and_then(|v| v.as_bool());
    let notify_comments = input.get("notify_comments").and_then(|v| v.as_bool());
    let notify_file_requests = input.get("notify_file_requests").and_then(|v| v.as_bool());
    
    sqlx::query(
        r#"
        UPDATE user_discord_connections
        SET 
            dm_notifications_enabled = COALESCE($2, dm_notifications_enabled),
            notify_file_shared = COALESCE($3, notify_file_shared),
            notify_file_uploaded = COALESCE($4, notify_file_uploaded),
            notify_comments = COALESCE($5, notify_comments),
            notify_file_requests = COALESCE($6, notify_file_requests),
            updated_at = NOW()
        WHERE user_id = $1
        "#
    )
    .bind(auth.user_id)
    .bind(dm_notifications_enabled)
    .bind(notify_file_shared)
    .bind(notify_file_uploaded)
    .bind(notify_comments)
    .bind(notify_file_requests)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({ "success": true })))
}

// ==================== Send DM (Internal Service) ====================

// ==================== Internal DM Sending Functions ====================

/// Check if Discord is enabled for a tenant
pub async fn is_discord_enabled(pool: &sqlx::PgPool, tenant_id: Uuid) -> bool {
    let enabled: Option<(bool,)> = sqlx::query_as(
        "SELECT enabled FROM tenant_discord_settings WHERE tenant_id = $1"
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    
    enabled.map(|e| e.0).unwrap_or(false)
}

/// Send a Discord DM to a user (fire-and-forget, logs errors but doesn't fail)
/// This is called internally by event handlers, not exposed as API
pub async fn send_dm(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    event_type: &str,
    message: &str,
) -> Result<(), String> {
    // Get user's Discord connection
    let connection: Option<(String, String, bool, bool, bool, bool, bool)> = sqlx::query_as(
        r#"
        SELECT 
            discord_user_id,
            access_token_encrypted,
            dm_notifications_enabled,
            notify_file_shared,
            notify_file_uploaded,
            notify_comments,
            notify_file_requests
        FROM user_discord_connections
        WHERE user_id = $1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Database error: {}", e))?;
    
    let (discord_user_id, access_token, dm_enabled, file_shared, file_uploaded, comments, file_requests) = 
        connection.ok_or("User not connected to Discord")?;
    
    // Check if notifications are enabled for this event type
    if !dm_enabled {
        return Ok(()); // DMs disabled, silently skip
    }
    
    let should_send = match event_type {
        "file_shared" => file_shared,
        "file_uploaded" => file_uploaded,
        "comment" => comments,
        "file_request" => file_requests,
        _ => true,
    };
    
    if !should_send {
        return Ok(()); // This notification type disabled
    }
    
    // Create DM channel
    let client = Client::new();
    let channel_response = client
        .post("https://discord.com/api/v10/users/@me/channels")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({ "recipient_id": discord_user_id }))
        .send()
        .await
        .map_err(|e| format!("Discord API error: {}", e))?;
    
    if !channel_response.status().is_success() {
        let error = channel_response.text().await.unwrap_or_default();
        return Err(format!("Failed to create DM channel: {}", error));
    }
    
    let channel: Value = channel_response.json().await.map_err(|_| "Invalid channel response")?;
    let channel_id = channel["id"].as_str().ok_or("Missing channel ID")?;
    
    // Send message
    let message_response = client
        .post(format!("https://discord.com/api/v10/channels/{}/messages", channel_id))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({ "content": message }))
        .send()
        .await
        .map_err(|e| format!("Discord API error: {}", e))?;
    
    if !message_response.status().is_success() {
        let error = message_response.text().await.unwrap_or_default();
        return Err(format!("Failed to send message: {}", error));
    }
    
    tracing::info!(user_id = %user_id, event = %event_type, "Discord DM sent");
    Ok(())
}

/// Test Discord connection by sending a test DM
/// POST /api/discord/test
pub async fn test_connection(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = send_dm(
        &state.pool,
        auth.user_id,
        "test",
        "🎉 **ClovaLink Test Message**\n\nYour Discord notifications are working! You'll receive messages here when files are shared with you."
    ).await;
    
    match result {
        Ok(()) => Ok(Json(json!({ "success": true }))),
        Err(e) => {
            tracing::warn!(user_id = %auth.user_id, error = %e, "Discord test failed");
            Err((StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
        }
    }
}

// ==================== Convenience Notification Functions ====================

/// Notify a user about a file upload (fire-and-forget)
pub async fn notify_file_upload(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    file_name: &str,
    uploader_name: &str,
    request_name: &str,
) {
    if !is_discord_enabled(pool, tenant_id).await {
        return;
    }
    
    let message = format!(
        "📁 **New File Uploaded**\n\n**{}** uploaded `{}` to your file request \"{}\".",
        uploader_name, file_name, request_name
    );
    
    if let Err(e) = send_dm(pool, user_id, "file_uploaded", &message).await {
        tracing::debug!(user_id = %user_id, error = %e, "Discord file upload notification failed");
    }
}

/// Notify a user about a file being shared with them (fire-and-forget)
pub async fn notify_file_shared(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    recipient_id: Uuid,
    file_name: &str,
    sharer_name: &str,
    share_link: Option<&str>,
) {
    if !is_discord_enabled(pool, tenant_id).await {
        return;
    }
    
    let message = if let Some(link) = share_link {
        format!(
            "🔗 **File Shared With You**\n\n**{}** shared `{}` with you.\n\nView: {}",
            sharer_name, file_name, link
        )
    } else {
        format!(
            "🔗 **File Shared With You**\n\n**{}** shared `{}` with you.",
            sharer_name, file_name
        )
    };
    
    if let Err(e) = send_dm(pool, recipient_id, "file_shared", &message).await {
        tracing::debug!(user_id = %recipient_id, error = %e, "Discord file shared notification failed");
    }
}

/// Notify a user about a new comment on their file (fire-and-forget)
pub async fn notify_comment(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    owner_id: Uuid,
    file_name: &str,
    commenter_name: &str,
    comment_preview: &str,
) {
    if !is_discord_enabled(pool, tenant_id).await {
        return;
    }
    
    let preview = if comment_preview.len() > 100 {
        format!("{}...", &comment_preview[..100])
    } else {
        comment_preview.to_string()
    };
    
    let message = format!(
        "💬 **New Comment**\n\n**{}** commented on `{}`:\n> {}",
        commenter_name, file_name, preview
    );
    
    if let Err(e) = send_dm(pool, owner_id, "comment", &message).await {
        tracing::debug!(user_id = %owner_id, error = %e, "Discord comment notification failed");
    }
}

/// Notify a user about a file request they received (fire-and-forget)
pub async fn notify_file_request(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    recipient_id: Uuid,
    request_name: &str,
    requester_name: &str,
    expires_at: Option<&str>,
) {
    if !is_discord_enabled(pool, tenant_id).await {
        return;
    }
    
    let expiry_text = expires_at
        .map(|e| format!("\n\nExpires: {}", e))
        .unwrap_or_default();
    
    let message = format!(
        "📨 **New File Request**\n\n**{}** is requesting files for \"{}\"{}",
        requester_name, request_name, expiry_text
    );
    
    if let Err(e) = send_dm(pool, recipient_id, "file_request", &message).await {
        tracing::debug!(user_id = %recipient_id, error = %e, "Discord file request notification failed");
    }
}

