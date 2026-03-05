//! OIDC SSO Handlers
//!
//! Provides endpoints for:
//! - Tenant OIDC provider management (CRUD)
//! - SSO login flow (discover, authorize, callback)
//! - Account linking/unlinking

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Json, Redirect},
    Extension,
};
use chrono::Utc;
use openidconnect::{
    core::{CoreClient, CoreIdTokenClaims, CoreProviderMetadata, CoreResponseType},
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::{require_super_admin, AuthUser};
use clovalink_core::models::{User, Tenant};
use crate::AppState;
use crate::sso_common::{
    self, SsoIdentityParams, SsoProvisionConfig, SsoSessionResult, SsoUserResolution,
};

// ==================== Models ====================

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OidcProvider {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub provider_type: String,
    pub issuer_url: String,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret_encrypted: String,
    pub scopes: String,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub auto_provision: bool,
    pub default_role: String,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Vec<String>,
    pub trust_idp_mfa: bool,
    pub enabled: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderInput {
    pub name: String,
    pub slug: String,
    pub provider_type: Option<String>,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Option<String>,
    pub auto_provision: Option<bool>,
    pub default_role: Option<String>,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Option<Vec<String>>,
    pub trust_idp_mfa: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub provider_type: Option<String>,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scopes: Option<String>,
    pub auto_provision: Option<bool>,
    pub default_role: Option<String>,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Option<Vec<String>>,
    pub trust_idp_mfa: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DiscoverParams {
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OidcIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub oidc_subject: String,
    pub oidc_issuer: String,
    pub oidc_email: Option<String>,
    pub oidc_name: Option<String>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub login_count: i32,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

// ==================== Provider Discovery (Public) ====================

/// Discover SSO providers (OIDC + SAML) by email domain
/// GET /api/auth/oidc/providers?email=user@acme.com
pub async fn discover_providers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DiscoverParams>,
) -> Result<Json<Value>, StatusCode> {
    let email = params.email.unwrap_or_default();
    let domain = email.split('@').nth(1).unwrap_or("");

    if domain.is_empty() {
        return Ok(Json(json!({ "providers": [], "sso_only": false })));
    }

    // Query OIDC providers
    let oidc_providers: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT p.id, p.name, p.slug, p.provider_type
        FROM tenant_oidc_providers p
        WHERE p.enabled = true AND $1 = ANY(p.email_domains)
        "#,
    )
    .bind(domain)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to discover OIDC providers: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Query SAML providers
    let saml_providers: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT p.id, p.name, p.slug, p.provider_type
        FROM tenant_saml_providers p
        WHERE p.enabled = true AND $1 = ANY(p.email_domains)
        "#,
    )
    .bind(domain)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to discover SAML providers: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if the tenant is SSO-only (no 'local' in auth_methods)
    let first_provider_id = oidc_providers.first().map(|p| p.0)
        .or_else(|| saml_providers.first().map(|p| p.0));
    let sso_only = if let Some(pid) = first_provider_id {
        // Try OIDC provider's tenant first, then SAML
        let tenant_auth: Option<(Option<Vec<String>>,)> = sqlx::query_as(
            r#"
            SELECT t.auth_methods FROM tenants t
            WHERE t.id = (
                SELECT tenant_id FROM tenant_oidc_providers WHERE id = $1
                UNION ALL
                SELECT tenant_id FROM tenant_saml_providers WHERE id = $1
                LIMIT 1
            )
            "#,
        )
        .bind(pid)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        tenant_auth
            .and_then(|t| t.0)
            .map(|methods| !methods.contains(&"local".to_string()))
            .unwrap_or(false)
    } else {
        false
    };

    // Build unified provider list with protocol field
    let mut provider_list: Vec<Value> = Vec::new();
    for (id, name, slug, provider_type) in &oidc_providers {
        provider_list.push(json!({
            "id": id, "name": name, "slug": slug,
            "provider_type": provider_type, "protocol": "oidc",
        }));
    }
    for (id, name, slug, provider_type) in &saml_providers {
        provider_list.push(json!({
            "id": id, "name": name, "slug": slug,
            "provider_type": provider_type, "protocol": "saml",
        }));
    }

    Ok(Json(json!({
        "providers": provider_list,
        "sso_only": sso_only,
    })))
}

// ==================== OIDC Auth Flow (Public) ====================

/// Start OIDC authorization flow
/// GET /api/auth/oidc/authorize/{provider_id}
pub async fn start_oidc_auth(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<Uuid>,
) -> Result<Redirect, (StatusCode, Json<Value>)> {
    let provider = sqlx::query_as::<_, OidcProvider>(
        "SELECT * FROM tenant_oidc_providers WHERE id = $1 AND enabled = true",
    )
    .bind(provider_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Provider not found or disabled"})),
        )
    })?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let callback_url = format!("{}/api/auth/oidc/callback", base_url);

    // Discover OIDC endpoints
    let issuer = IssuerUrl::new(provider.issuer_url.clone()).map_err(|e| {
        tracing::error!("Invalid issuer URL: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid provider issuer URL"})),
        )
    })?;

    let metadata = CoreProviderMetadata::discover_async(issuer, &reqwest::Client::new())
        .await
        .map_err(|e| {
            tracing::error!("OIDC discovery failed for {}: {:?}", provider.name, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "Failed to discover OIDC provider endpoints"})),
            )
        })?;

    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id.clone()),
        Some(ClientSecret::new(provider.client_secret_encrypted.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(callback_url).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Invalid callback URL"})),
        )
    })?);

    // Generate authorization URL
    let mut auth_request = client.authorize_url(
        AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );

    // Add scopes
    for scope in provider.scopes.split_whitespace() {
        if scope != "openid" {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }
    }

    let (auth_url, csrf_state, nonce) = auth_request.url();

    // Store state for callback validation
    sqlx::query(
        r#"
        INSERT INTO oidc_oauth_states (state, nonce, provider_id, tenant_id)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(csrf_state.secret())
    .bind(nonce.secret())
    .bind(provider.id)
    .bind(provider.tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to create OAuth state"})),
        )
    })?;

    Ok(Redirect::temporary(auth_url.as_str()))
}

/// Handle OIDC callback from the identity provider
/// GET /api/auth/oidc/callback
pub async fn oidc_callback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<OidcCallbackParams>,
) -> Result<Redirect, (StatusCode, String)> {
    let frontend_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Handle IdP errors
    if let Some(error) = params.error {
        let desc = params.error_description.unwrap_or_default();
        tracing::warn!("OIDC auth error: {} - {}", error, desc);
        return Ok(Redirect::temporary(&format!(
            "{}/login?error=oidc_error&message={}",
            frontend_url,
            urlencoding::encode(&desc)
        )));
    }

    let code = params
        .code
        .ok_or((StatusCode::BAD_REQUEST, "Missing code".to_string()))?;
    let state_token = params
        .state
        .ok_or((StatusCode::BAD_REQUEST, "Missing state".to_string()))?;

    // Validate state and get associated data
    let state_record: Option<(String, Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT nonce, provider_id, tenant_id, user_id
        FROM oidc_oauth_states
        WHERE state = $1 AND expires_at > NOW()
        "#,
    )
    .bind(&state_token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;

    let (nonce_str, provider_id, tenant_id, linking_user_id) = state_record.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid or expired state".to_string(),
        )
    })?;

    // Delete used state (one-time use)
    let _ = sqlx::query("DELETE FROM oidc_oauth_states WHERE state = $1")
        .bind(&state_token)
        .execute(&state.pool)
        .await;

    // Load provider config
    let provider = sqlx::query_as::<_, OidcProvider>(
        "SELECT * FROM tenant_oidc_providers WHERE id = $1",
    )
    .bind(provider_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Provider not found".to_string(),
        )
    })?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let callback_url = format!("{}/api/auth/oidc/callback", base_url);

    // Discover OIDC endpoints and exchange code for tokens
    let issuer = IssuerUrl::new(provider.issuer_url.clone())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid issuer".to_string()))?;

    let metadata = CoreProviderMetadata::discover_async(issuer, &reqwest::Client::new())
        .await
        .map_err(|e| {
            tracing::error!("OIDC discovery failed: {:?}", e);
            (
                StatusCode::BAD_GATEWAY,
                "OIDC discovery failed".to_string(),
            )
        })?;

    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id.clone()),
        Some(ClientSecret::new(provider.client_secret_encrypted.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(callback_url)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid callback URL".to_string()))?,
    );

    // Exchange authorization code for tokens
    let http_client = reqwest::Client::new();
    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|e| {
            tracing::error!("Token exchange config error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Token exchange config error".to_string())
        })?
        .request_async(&http_client)
        .await
        .map_err(|e| {
            tracing::error!("Token exchange failed: {:?}", e);
            (
                StatusCode::BAD_GATEWAY,
                "Token exchange failed".to_string(),
            )
        })?;

    // Verify and extract ID token claims
    let id_token = token_response.id_token().ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            "No ID token in response".to_string(),
        )
    })?;

    let nonce = Nonce::new(nonce_str);
    let id_token_verifier = client.id_token_verifier();
    let claims: &CoreIdTokenClaims = id_token
        .claims(&id_token_verifier, &nonce)
        .map_err(|e| {
            tracing::error!("ID token verification failed: {:?}", e);
            (
                StatusCode::BAD_GATEWAY,
                "ID token verification failed".to_string(),
            )
        })?;

    // Extract user info from claims
    let oidc_subject = claims.subject().to_string();
    let oidc_email = claims
        .email()
        .map(|e| e.to_string());
    let oidc_name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.to_string());

    // === Account Linking Flow ===
    if let Some(linking_user_id) = linking_user_id {
        // Verify user still exists
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1 AND status = 'active'")
            .bind(linking_user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;

        let user = user.ok_or_else(|| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

        // Create identity link
        sqlx::query(
            r#"
            INSERT INTO user_oidc_identities (user_id, provider_id, oidc_subject, oidc_issuer, oidc_email, oidc_name)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (provider_id, oidc_subject) DO UPDATE SET
                oidc_email = EXCLUDED.oidc_email,
                oidc_name = EXCLUDED.oidc_name,
                updated_at = NOW()
            "#,
        )
        .bind(user.id)
        .bind(provider_id)
        .bind(&oidc_subject)
        .bind(&provider.issuer_url)
        .bind(&oidc_email)
        .bind(&oidc_name)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to link OIDC identity: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to link identity".to_string())
        })?;

        // Update identity_provider to hybrid if currently local
        if user.identity_provider == "local" {
            let _ = sqlx::query("UPDATE users SET identity_provider = 'hybrid', updated_at = NOW() WHERE id = $1")
                .bind(user.id)
                .execute(&state.pool)
                .await;
        }

        tracing::info!(user_id = %user.id, provider = %provider.name, "OIDC identity linked");
        return Ok(Redirect::temporary(&format!(
            "{}/profile?oidc=linked",
            frontend_url
        )));
    }

    // === Login Flow (via shared SSO logic) ===

    let identity = SsoIdentityParams {
        protocol: "oidc".to_string(),
        provider_id,
        tenant_id,
        subject: oidc_subject.clone(),
        issuer: provider.issuer_url.clone(),
        email: oidc_email.clone(),
        name: oidc_name.clone(),
    };

    let provision_config = SsoProvisionConfig {
        auto_provision: provider.auto_provision,
        default_role: provider.default_role.clone(),
        default_custom_role_id: None, // OIDC providers don't have this yet
        default_department_id: provider.default_department_id,
        trust_idp_mfa: provider.trust_idp_mfa,
        provider_name: provider.name.clone(),
        provider_slug: provider.slug.clone(),
    };

    // TODO: Extract custom claims (groups, roles) from ID token for attribute mapping
    let role_override = None;

    let user = match sso_common::resolve_sso_user(&state.pool, &identity, &provision_config, role_override.as_ref()).await? {
        SsoUserResolution::ExistingUser(u) | SsoUserResolution::NewUser(u) => u,
        SsoUserResolution::NoAccount => {
            return Ok(Redirect::temporary(&format!(
                "{}/login?error=no_account", frontend_url
            )));
        }
        SsoUserResolution::NoEmail => {
            return Ok(Redirect::temporary(&format!(
                "{}/login?error=no_email", frontend_url
            )));
        }
    };

    // Update OIDC identity login tracking
    let _ = sqlx::query(
        r#"
        UPDATE user_oidc_identities
        SET last_login_at = NOW(), login_count = login_count + 1, oidc_email = $3, oidc_name = $4, updated_at = NOW()
        WHERE provider_id = $1 AND oidc_subject = $2
        "#,
    )
    .bind(provider_id)
    .bind(&oidc_subject)
    .bind(&oidc_email)
    .bind(&oidc_name)
    .execute(&state.pool)
    .await;

    // Load tenant
    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Tenant not found".to_string()))?;

    // Create session via shared logic
    match sso_common::create_sso_session(&state.pool, &user, &tenant, &headers, &provision_config, &frontend_url).await? {
        SsoSessionResult::Token(token) => {
            Ok(Redirect::temporary(&format!(
                "{}/auth/oidc/complete?token={}", frontend_url, token
            )))
        }
        SsoSessionResult::Pending2fa { user_id, provider_slug } => {
            Ok(Redirect::temporary(&format!(
                "{}/login?pending_2fa=true&user_id={}&provider={}", frontend_url, user_id, provider_slug
            )))
        }
        SsoSessionResult::Suspended => {
            Ok(Redirect::temporary(&format!(
                "{}/login?error=suspended", frontend_url
            )))
        }
    }
}

// ==================== Provider Management (Admin) ====================

/// List OIDC providers for the current tenant
/// GET /api/oidc/providers
pub async fn list_providers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    let providers: Vec<OidcProvider> = sqlx::query_as(
        "SELECT * FROM tenant_oidc_providers WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "providers": providers })))
}

/// Create a new OIDC provider
/// POST /api/oidc/providers
pub async fn create_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<CreateProviderInput>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    let provider: OidcProvider = sqlx::query_as(
        r#"
        INSERT INTO tenant_oidc_providers
            (tenant_id, name, slug, provider_type, issuer_url, client_id, client_secret_encrypted,
             scopes, auto_provision, default_role, default_department_id, email_domains, trust_idp_mfa, enabled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING *
        "#,
    )
    .bind(auth.tenant_id)
    .bind(&input.name)
    .bind(&input.slug)
    .bind(input.provider_type.as_deref().unwrap_or("generic"))
    .bind(&input.issuer_url)
    .bind(&input.client_id)
    .bind(&input.client_secret) // SECURITY TODO: Encrypt at rest using ENCRYPTION_KEY. Requires key mgmt design + migration for existing plaintext values.
    .bind(input.scopes.as_deref().unwrap_or("openid email profile"))
    .bind(input.auto_provision.unwrap_or(false))
    .bind(input.default_role.as_deref().unwrap_or("Employee"))
    .bind(input.default_department_id)
    .bind(&input.email_domains.unwrap_or_default())
    .bind(input.trust_idp_mfa.unwrap_or(true))
    .bind(input.enabled.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create OIDC provider: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Ensure tenant has 'oidc' in auth_methods
    let _ = sqlx::query(
        r#"
        UPDATE tenants
        SET auth_methods = CASE
            WHEN NOT ('oidc' = ANY(auth_methods)) THEN array_append(auth_methods, 'oidc')
            ELSE auth_methods
        END,
        updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(auth.tenant_id)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({ "provider": provider })))
}

/// Update an OIDC provider
/// PUT /api/oidc/providers/{id}
pub async fn update_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProviderInput>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    // Verify provider belongs to tenant
    let existing: Option<OidcProvider> = sqlx::query_as(
        "SELECT * FROM tenant_oidc_providers WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _existing = existing.ok_or(StatusCode::NOT_FOUND)?;

    let provider: OidcProvider = sqlx::query_as(
        r#"
        UPDATE tenant_oidc_providers SET
            name = COALESCE($1, name),
            slug = COALESCE($2, slug),
            provider_type = COALESCE($3, provider_type),
            issuer_url = COALESCE($4, issuer_url),
            client_id = COALESCE($5, client_id),
            client_secret_encrypted = COALESCE($6, client_secret_encrypted),
            scopes = COALESCE($7, scopes),
            auto_provision = COALESCE($8, auto_provision),
            default_role = COALESCE($9, default_role),
            default_department_id = COALESCE($10, default_department_id),
            email_domains = COALESCE($11, email_domains),
            trust_idp_mfa = COALESCE($12, trust_idp_mfa),
            enabled = COALESCE($13, enabled),
            updated_at = NOW()
        WHERE id = $14 AND tenant_id = $15
        RETURNING *
        "#,
    )
    .bind(input.name.as_deref())
    .bind(input.slug.as_deref())
    .bind(input.provider_type.as_deref())
    .bind(input.issuer_url.as_deref())
    .bind(input.client_id.as_deref())
    .bind(input.client_secret.as_deref())
    .bind(input.scopes.as_deref())
    .bind(input.auto_provision)
    .bind(input.default_role.as_deref())
    .bind(input.default_department_id)
    .bind(input.email_domains.as_deref())
    .bind(input.trust_idp_mfa)
    .bind(input.enabled)
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update OIDC provider: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "provider": provider })))
}

/// Delete an OIDC provider
/// DELETE /api/oidc/providers/{id}
pub async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    // Check for OIDC-only users that would be locked out
    let oidc_only_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM users u
        JOIN user_oidc_identities i ON i.user_id = u.id
        WHERE i.provider_id = $1 AND u.identity_provider = 'oidc' AND u.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if oidc_only_count.0 > 0 {
        return Ok(Json(json!({
            "error": "provider_has_sso_only_users",
            "message": format!("{} user(s) use only this provider for login and would be locked out. Set passwords for them first.", oidc_only_count.0),
            "affected_count": oidc_only_count.0,
        })));
    }

    sqlx::query("DELETE FROM tenant_oidc_providers WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(auth.tenant_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if tenant still has OIDC providers; if not, remove 'oidc' from auth_methods
    let remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tenant_oidc_providers WHERE tenant_id = $1",
    )
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if remaining.0 == 0 {
        let _ = sqlx::query(
            "UPDATE tenants SET auth_methods = array_remove(auth_methods, 'oidc'), updated_at = NOW() WHERE id = $1",
        )
        .bind(auth.tenant_id)
        .execute(&state.pool)
        .await;
    }

    Ok(Json(json!({ "success": true })))
}

/// Test an OIDC provider by fetching its discovery document
/// POST /api/oidc/providers/{id}/test
pub async fn test_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    let provider: OidcProvider = sqlx::query_as(
        "SELECT * FROM tenant_oidc_providers WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let issuer = IssuerUrl::new(provider.issuer_url.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;

    match CoreProviderMetadata::discover_async(issuer, &reqwest::Client::new()).await {
        Ok(metadata) => {
            let auth_endpoint = metadata
                .authorization_endpoint()
                .to_string();
            let token_endpoint = metadata
                .token_endpoint()
                .map(|e| e.to_string());

            Ok(Json(json!({
                "success": true,
                "authorization_endpoint": auth_endpoint,
                "token_endpoint": token_endpoint,
            })))
        }
        Err(e) => Ok(Json(json!({
            "success": false,
            "error": format!("Discovery failed: {}", e),
        }))),
    }
}

// ==================== Account Linking (Authenticated) ====================

/// Start OIDC flow for account linking
/// GET /api/auth/oidc/link/{provider_id}
pub async fn link_oidc_identity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<Redirect, (StatusCode, Json<Value>)> {
    let provider = sqlx::query_as::<_, OidcProvider>(
        "SELECT * FROM tenant_oidc_providers WHERE id = $1 AND tenant_id = $2 AND enabled = true",
    )
    .bind(provider_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Provider not found"}))))?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let callback_url = format!("{}/api/auth/oidc/callback", base_url);

    let issuer = IssuerUrl::new(provider.issuer_url.clone()).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid issuer URL"})))
    })?;

    let metadata = CoreProviderMetadata::discover_async(issuer, &reqwest::Client::new())
        .await
        .map_err(|_| {
            (StatusCode::BAD_GATEWAY, Json(json!({"error": "OIDC discovery failed"})))
        })?;

    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id.clone()),
        Some(ClientSecret::new(provider.client_secret_encrypted.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(callback_url).map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Invalid callback URL"})))
        })?,
    );

    let mut auth_request = client.authorize_url(
        AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );

    for scope in provider.scopes.split_whitespace() {
        if scope != "openid" {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }
    }

    let (auth_url, csrf_state, nonce) = auth_request.url();

    // Store state WITH user_id for account linking
    sqlx::query(
        r#"
        INSERT INTO oidc_oauth_states (state, nonce, provider_id, tenant_id, user_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(csrf_state.secret())
    .bind(nonce.secret())
    .bind(provider.id)
    .bind(auth.tenant_id)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create state"}))))?;

    Ok(Redirect::temporary(auth_url.as_str()))
}

/// Unlink an OIDC identity
/// DELETE /api/auth/oidc/unlink/{identity_id}
pub async fn unlink_oidc_identity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(identity_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Verify identity belongs to user
    let identity: Option<OidcIdentity> = sqlx::query_as(
        "SELECT * FROM user_oidc_identities WHERE id = $1 AND user_id = $2",
    )
    .bind(identity_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _identity = identity.ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has a password — prevent lockout
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Count remaining OIDC identities
    let identity_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_oidc_identities WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user.password_hash.is_none() && identity_count.0 <= 1 {
        return Ok(Json(json!({
            "error": "cannot_unlink",
            "message": "Cannot unlink your only login method. Set a password first.",
        })));
    }

    sqlx::query("DELETE FROM user_oidc_identities WHERE id = $1 AND user_id = $2")
        .bind(identity_id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update identity_provider if no more OIDC identities
    if identity_count.0 <= 1 && user.password_hash.is_some() {
        let _ = sqlx::query(
            "UPDATE users SET identity_provider = 'local', updated_at = NOW() WHERE id = $1",
        )
        .bind(auth.user_id)
        .execute(&state.pool)
        .await;
    }

    Ok(Json(json!({ "success": true })))
}

/// List current user's linked OIDC identities
/// GET /api/auth/oidc/identities
pub async fn list_my_identities(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let identities: Vec<OidcIdentity> = sqlx::query_as(
        r#"
        SELECT i.*
        FROM user_oidc_identities i
        WHERE i.user_id = $1
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Also fetch provider names for display
    let mut result = Vec::new();
    for identity in &identities {
        let provider_info: Option<(String, String, String)> = sqlx::query_as(
            "SELECT name, slug, provider_type FROM tenant_oidc_providers WHERE id = $1",
        )
        .bind(identity.provider_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let (provider_name, provider_slug, provider_type) =
            provider_info.unwrap_or(("Unknown".to_string(), "unknown".to_string(), "generic".to_string()));

        result.push(json!({
            "id": identity.id,
            "provider_id": identity.provider_id,
            "provider_name": provider_name,
            "provider_slug": provider_slug,
            "provider_type": provider_type,
            "oidc_email": identity.oidc_email,
            "oidc_name": identity.oidc_name,
            "last_login_at": identity.last_login_at,
            "login_count": identity.login_count,
            "created_at": identity.created_at,
        }));
    }

    Ok(Json(json!({ "identities": result })))
}
