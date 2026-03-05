//! SAML 2.0 SSO Handlers
//!
//! Provides endpoints for:
//! - Tenant SAML provider management (CRUD, SuperAdmin only)
//! - SP metadata generation
//! - SSO login flow (authorize, ACS callback)
//! - Account linking/unlinking

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Json, Redirect},
    Extension, Form,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::{require_super_admin, AuthUser};
use clovalink_core::models::{Tenant, User};
use crate::AppState;
use crate::saml_crypto;
use crate::saml_xml;
use crate::sso_common::{
    self, SsoIdentityParams, SsoProvisionConfig, SsoSessionResult, SsoUserResolution,
};

// ==================== Models ====================

#[derive(Debug, Serialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SamlProvider {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub provider_type: String,
    // IdP Configuration
    pub idp_entity_id: String,
    pub idp_sso_url: String,
    pub idp_slo_url: Option<String>,
    pub idp_metadata_url: Option<String>,
    pub idp_metadata_xml: Option<String>,
    #[serde(skip_serializing)]
    pub idp_signing_certificate: String,
    // SP Configuration
    pub sp_entity_id: String,
    pub nameid_format: String,
    pub request_signing: bool,
    pub want_assertions_signed: bool,
    pub want_response_signed: bool,
    #[serde(skip_serializing)]
    pub sp_signing_key_encrypted: Option<String>,
    pub sp_signing_cert: Option<String>,
    pub sso_binding: String,
    // Attribute hints
    pub attribute_email: String,
    pub attribute_name: String,
    // Behavior
    pub auto_provision: bool,
    pub default_role: String,
    pub default_custom_role_id: Option<Uuid>,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Vec<String>,
    pub trust_idp_mfa: bool,
    pub enabled: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSamlProviderInput {
    pub name: String,
    pub slug: String,
    pub provider_type: Option<String>,
    pub idp_entity_id: String,
    pub idp_sso_url: String,
    pub idp_slo_url: Option<String>,
    pub idp_metadata_url: Option<String>,
    pub idp_signing_certificate: String,
    pub nameid_format: Option<String>,
    pub sso_binding: Option<String>,
    pub attribute_email: Option<String>,
    pub attribute_name: Option<String>,
    pub auto_provision: Option<bool>,
    pub default_role: Option<String>,
    pub default_custom_role_id: Option<Uuid>,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Option<Vec<String>>,
    pub trust_idp_mfa: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSamlProviderInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub provider_type: Option<String>,
    pub idp_entity_id: Option<String>,
    pub idp_sso_url: Option<String>,
    pub idp_slo_url: Option<String>,
    pub idp_metadata_url: Option<String>,
    pub idp_signing_certificate: Option<String>,
    pub nameid_format: Option<String>,
    pub sso_binding: Option<String>,
    pub attribute_email: Option<String>,
    pub attribute_name: Option<String>,
    pub auto_provision: Option<bool>,
    pub default_role: Option<String>,
    pub default_custom_role_id: Option<Uuid>,
    pub default_department_id: Option<Uuid>,
    pub email_domains: Option<Vec<String>>,
    pub trust_idp_mfa: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SamlIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub saml_name_id: String,
    pub saml_name_id_format: Option<String>,
    pub saml_session_index: Option<String>,
    pub saml_email: Option<String>,
    pub saml_name: Option<String>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub login_count: i32,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

/// SAML ACS POST form data
#[derive(Debug, Deserialize)]
pub struct SamlAcsForm {
    #[serde(rename = "SAMLResponse")]
    pub saml_response: String,
    #[serde(rename = "RelayState")]
    pub relay_state: Option<String>,
}

// ==================== SP Metadata (Public) ====================

/// Return SAML SP metadata XML for a provider.
/// GET /api/auth/saml/metadata/{provider_id}
pub async fn sp_metadata(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<Uuid>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String), StatusCode> {
    let provider = sqlx::query_as::<_, SamlProvider>(
        "SELECT * FROM tenant_saml_providers WHERE id = $1 AND enabled = true",
    )
    .bind(provider_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let acs_url = format!("{}/api/auth/saml/acs", base_url);

    let metadata = saml_xml::generate_sp_metadata(
        &provider.sp_entity_id,
        &acs_url,
        &provider.nameid_format,
        provider.sp_signing_cert.as_deref(),
    );

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        metadata,
    ))
}

// ==================== SAML Auth Flow (Public) ====================

/// Start SAML authorization flow — redirect user to IdP.
/// GET /api/auth/saml/authorize/{provider_id}
pub async fn start_saml_auth(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<Uuid>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String), (StatusCode, Json<Value>)> {
    start_saml_flow(&state, provider_id, None).await
}

/// Internal: start SAML flow (used for both login and account linking).
async fn start_saml_flow(
    state: &AppState,
    provider_id: Uuid,
    linking_user_id: Option<Uuid>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String), (StatusCode, Json<Value>)> {
    let provider = sqlx::query_as::<_, SamlProvider>(
        "SELECT * FROM tenant_saml_providers WHERE id = $1 AND enabled = true",
    )
    .bind(provider_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Provider not found or disabled"}))))?;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let acs_url = format!("{}/api/auth/saml/acs", base_url);

    // Generate request ID and relay state
    let request_id = format!("_cl_{}", Uuid::new_v4().to_string().replace('-', ""));
    let relay_state = nanoid::nanoid!(32);

    let authn_request = saml_xml::generate_authn_request(
        &request_id,
        &provider.sp_entity_id,
        &provider.idp_sso_url,
        &acs_url,
        &provider.nameid_format,
    );

    // Store auth state for callback validation
    sqlx::query(
        r#"
        INSERT INTO saml_auth_states (relay_state, authn_request_id, provider_id, tenant_id, user_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&relay_state)
    .bind(&request_id)
    .bind(provider.id)
    .bind(provider.tenant_id)
    .bind(linking_user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create auth state"}))))?;

    // Choose binding method
    match provider.sso_binding.as_str() {
        "HTTP-Redirect" => {
            let encoded = saml_xml::encode_authn_request_redirect(&authn_request);
            let redirect_url = format!(
                "{}?SAMLRequest={}&RelayState={}",
                provider.idp_sso_url,
                encoded,
                urlencoding::encode(&relay_state),
            );
            // Return 302 redirect via HTML meta refresh (axum Redirect doesn't support custom headers easily)
            let html = format!(
                r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head><body>Redirecting...</body></html>"#,
                html_escape(&redirect_url),
            );
            Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html")],
                html,
            ))
        }
        _ => {
            // HTTP-POST (default)
            let b64 = saml_xml::encode_authn_request_post(&authn_request);
            let form_html = saml_xml::generate_post_form(&provider.idp_sso_url, &b64, &relay_state);
            Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html")],
                form_html,
            ))
        }
    }
}

/// SAML Assertion Consumer Service — handle IdP response.
/// POST /api/auth/saml/acs
pub async fn saml_acs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<SamlAcsForm>,
) -> Result<Redirect, (StatusCode, String)> {
    let frontend_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Step 1: Validate RelayState
    let relay_state = form.relay_state.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing RelayState".to_string())
    })?;

    let state_record: Option<(String, Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT authn_request_id, provider_id, tenant_id, user_id
        FROM saml_auth_states
        WHERE relay_state = $1 AND expires_at > NOW()
        "#,
    )
    .bind(&relay_state)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;

    let (authn_request_id, provider_id, tenant_id, linking_user_id) =
        state_record.ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Invalid or expired RelayState".to_string())
        })?;

    // Delete used state (one-time use)
    let _ = sqlx::query("DELETE FROM saml_auth_states WHERE relay_state = $1")
        .bind(&relay_state)
        .execute(&state.pool)
        .await;

    // Step 2: Load provider
    let provider = sqlx::query_as::<_, SamlProvider>(
        "SELECT * FROM tenant_saml_providers WHERE id = $1",
    )
    .bind(provider_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Provider not found".to_string()))?;

    // Step 3: Parse SAML Response
    let saml_response = saml_xml::parse_saml_response(&form.saml_response)
        .map_err(|e| {
            tracing::error!("Failed to parse SAML response: {}", e);
            (StatusCode::BAD_REQUEST, format!("Invalid SAML response: {}", e))
        })?;

    // Step 4: Verify status
    if !saml_response.status_code.contains("Success") {
        tracing::warn!("SAML response status: {}", saml_response.status_code);
        return Ok(Redirect::temporary(&format!(
            "{}/login?error=saml_error&message={}",
            frontend_url,
            urlencoding::encode(&format!("IdP returned: {}", saml_response.status_code)),
        )));
    }

    let assertion = saml_response.assertion.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "No assertion in SAML response".to_string())
    })?;

    // Step 5: Verify XML signature (if provider requires it)
    if provider.want_response_signed || provider.want_assertions_signed {
        match saml_crypto::verify_saml_signature(
            &saml_response.raw_xml,
            &provider.idp_signing_certificate,
            Some(&assertion.id),
        ) {
            Ok(true) => {}
            Ok(false) => {
                tracing::error!("SAML signature verification returned false");
                return Err((StatusCode::BAD_REQUEST, "Signature verification failed".to_string()));
            }
            Err(saml_crypto::SamlCryptoError::NoSignature) => {
                if provider.want_assertions_signed {
                    tracing::error!("SAML assertion not signed but provider requires it");
                    return Err((StatusCode::BAD_REQUEST, "Assertion not signed".to_string()));
                }
            }
            Err(e) => {
                tracing::error!("SAML signature error: {:?}", e);
                return Err((StatusCode::BAD_REQUEST, format!("Signature error: {}", e)));
            }
        }
    }

    // Step 6: Validate InResponseTo (REQUIRED — prevents response substitution attacks)
    let in_response_to = saml_response.in_response_to.as_deref().ok_or_else(|| {
        tracing::error!("SAML Response missing required InResponseTo attribute");
        (StatusCode::BAD_REQUEST, "Missing InResponseTo".to_string())
    })?;
    if in_response_to != authn_request_id {
        tracing::error!(
            "InResponseTo mismatch: expected {}, got {}",
            authn_request_id,
            in_response_to
        );
        return Err((StatusCode::BAD_REQUEST, "InResponseTo mismatch".to_string()));
    }

    // Step 7: Validate time window (60-sec clock skew tolerance)
    let now = Utc::now();
    let skew = chrono::Duration::seconds(60);

    if let Some(not_before) = assertion.not_before {
        if now < not_before - skew {
            tracing::error!("Assertion not yet valid: NotBefore={}", not_before);
            return Err((StatusCode::BAD_REQUEST, "Assertion not yet valid".to_string()));
        }
    }
    if let Some(not_on_or_after) = assertion.not_on_or_after {
        if now >= not_on_or_after + skew {
            tracing::error!("Assertion expired: NotOnOrAfter={}", not_on_or_after);
            return Err((StatusCode::BAD_REQUEST, "Assertion expired".to_string()));
        }
    }

    // Step 8: Validate audience
    if let Some(ref audience) = assertion.audience {
        if audience != &provider.sp_entity_id {
            tracing::error!(
                "Audience mismatch: expected {}, got {}",
                provider.sp_entity_id,
                audience
            );
            return Err((StatusCode::BAD_REQUEST, "Audience mismatch".to_string()));
        }
    }

    // Step 9: Replay protection — atomic insert to prevent race conditions
    let expiry = assertion.not_on_or_after.unwrap_or_else(|| now + chrono::Duration::hours(1));
    let inserted: Option<(String,)> = sqlx::query_as(
        "INSERT INTO saml_consumed_assertions (assertion_id, provider_id, consumed_at, not_on_or_after) VALUES ($1, $2, NOW(), $3) ON CONFLICT (assertion_id) DO NOTHING RETURNING assertion_id",
    )
    .bind(&assertion.id)
    .bind(provider_id)
    .bind(expiry)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;

    if inserted.is_none() {
        tracing::error!("Replay detected: assertion {} already consumed", assertion.id);
        return Err((StatusCode::BAD_REQUEST, "Assertion replay detected".to_string()));
    }

    // Step 10: Extract user attributes
    let name_id = assertion.name_id.clone();
    let saml_email = get_attribute_value(&assertion.attributes, &provider.attribute_email)
        .or_else(|| {
            // Fallback: NameID might be an email
            if name_id.contains('@') { Some(name_id.clone()) } else { None }
        });
    let saml_name = get_attribute_value(&assertion.attributes, &provider.attribute_name);

    // Step 11: Apply attribute mappings
    let role_override = sso_common::apply_attribute_mapping(
        &state.pool,
        "saml",
        provider_id,
        &assertion.attributes,
    )
    .await;

    // === Account Linking Flow ===
    if let Some(linking_user_id) = linking_user_id {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1 AND status = 'active'")
            .bind(linking_user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()))?;

        let user = user.ok_or_else(|| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

        // Create SAML identity link
        sqlx::query(
            r#"
            INSERT INTO user_saml_identities (user_id, provider_id, saml_name_id, saml_name_id_format, saml_session_index, saml_email, saml_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (provider_id, saml_name_id) DO UPDATE SET
                saml_email = EXCLUDED.saml_email,
                saml_name = EXCLUDED.saml_name,
                saml_session_index = EXCLUDED.saml_session_index,
                updated_at = NOW()
            "#,
        )
        .bind(user.id)
        .bind(provider_id)
        .bind(&name_id)
        .bind(&assertion.name_id_format)
        .bind(&assertion.session_index)
        .bind(&saml_email)
        .bind(&saml_name)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to link SAML identity: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to link identity".to_string())
        })?;

        // Update identity_provider to hybrid if currently local
        if user.identity_provider == "local" {
            let _ = sqlx::query("UPDATE users SET identity_provider = 'hybrid', updated_at = NOW() WHERE id = $1")
                .bind(user.id)
                .execute(&state.pool)
                .await;
        }

        tracing::info!(user_id = %user.id, provider = %provider.name, "SAML identity linked");
        return Ok(Redirect::temporary(&format!(
            "{}/profile?saml=linked",
            frontend_url,
        )));
    }

    // SECURITY: Validate assertion issuer matches configured IdP entity ID
    if assertion.issuer != provider.idp_entity_id {
        tracing::error!(
            "Assertion issuer mismatch: expected {}, got {}",
            provider.idp_entity_id,
            assertion.issuer
        );
        return Err((StatusCode::BAD_REQUEST, "Assertion issuer mismatch".to_string()));
    }

    // === Login Flow (via shared SSO logic) ===

    let identity = SsoIdentityParams {
        protocol: "saml".to_string(),
        provider_id,
        tenant_id,
        subject: name_id.clone(),
        issuer: assertion.issuer.clone(),
        email: saml_email.clone(),
        name: saml_name.clone(),
    };

    let provision_config = SsoProvisionConfig {
        auto_provision: provider.auto_provision,
        default_role: provider.default_role.clone(),
        default_custom_role_id: provider.default_custom_role_id,
        default_department_id: provider.default_department_id,
        trust_idp_mfa: provider.trust_idp_mfa,
        provider_name: provider.name.clone(),
        provider_slug: provider.slug.clone(),
    };

    let user = match sso_common::resolve_sso_user(
        &state.pool,
        &identity,
        &provision_config,
        role_override.as_ref(),
    )
    .await?
    {
        SsoUserResolution::ExistingUser(u) | SsoUserResolution::NewUser(u) => u,
        SsoUserResolution::NoAccount => {
            return Ok(Redirect::temporary(&format!(
                "{}/login?error=no_account",
                frontend_url,
            )));
        }
        SsoUserResolution::NoEmail => {
            return Ok(Redirect::temporary(&format!(
                "{}/login?error=no_email",
                frontend_url,
            )));
        }
    };

    // Update SAML identity login tracking
    let _ = sqlx::query(
        r#"
        UPDATE user_saml_identities
        SET last_login_at = NOW(), login_count = login_count + 1,
            saml_email = $3, saml_name = $4, saml_session_index = $5, updated_at = NOW()
        WHERE provider_id = $1 AND saml_name_id = $2
        "#,
    )
    .bind(provider_id)
    .bind(&name_id)
    .bind(&saml_email)
    .bind(&saml_name)
    .bind(&assertion.session_index)
    .execute(&state.pool)
    .await;

    // Load tenant
    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Tenant not found".to_string()))?;

    // Create session via shared logic
    match sso_common::create_sso_session(
        &state.pool,
        &user,
        &tenant,
        &headers,
        &provision_config,
        &frontend_url,
    )
    .await?
    {
        SsoSessionResult::Token(token) => Ok(Redirect::temporary(&format!(
            "{}/auth/sso/complete?token={}",
            frontend_url, token,
        ))),
        SsoSessionResult::Pending2fa {
            user_id,
            provider_slug,
        } => Ok(Redirect::temporary(&format!(
            "{}/login?pending_2fa=true&user_id={}&provider={}",
            frontend_url, user_id, provider_slug,
        ))),
        SsoSessionResult::Suspended => Ok(Redirect::temporary(&format!(
            "{}/login?error=suspended",
            frontend_url,
        ))),
    }
}

// ==================== Provider Management (SuperAdmin) ====================

/// List SAML providers for the current tenant.
/// GET /api/saml/providers
pub async fn list_providers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    require_super_admin(&auth)?;

    let providers: Vec<SamlProvider> = sqlx::query_as(
        "SELECT * FROM tenant_saml_providers WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "providers": providers })))
}

/// Create a new SAML provider.
/// POST /api/saml/providers
pub async fn create_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Json(input): Json<CreateSamlProviderInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    // Validate the certificate is parseable
    if let Err(e) = saml_crypto::parse_x509_pem(&input.idp_signing_certificate) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Invalid IdP signing certificate: {}", e)})),
        ));
    }

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let sp_entity_id = format!("{}/api/auth/saml/metadata/{}", base_url, input.slug);

    let provider: SamlProvider = sqlx::query_as(
        r#"
        INSERT INTO tenant_saml_providers (
            tenant_id, name, slug, provider_type,
            idp_entity_id, idp_sso_url, idp_slo_url, idp_metadata_url,
            idp_signing_certificate, sp_entity_id,
            nameid_format, sso_binding,
            attribute_email, attribute_name,
            auto_provision, default_role, default_custom_role_id, default_department_id,
            email_domains, trust_idp_mfa, enabled
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
        RETURNING *
        "#,
    )
    .bind(auth.tenant_id)
    .bind(&input.name)
    .bind(&input.slug)
    .bind(input.provider_type.as_deref().unwrap_or("generic"))
    .bind(&input.idp_entity_id)
    .bind(&input.idp_sso_url)
    .bind(input.idp_slo_url.as_deref())
    .bind(input.idp_metadata_url.as_deref())
    .bind(&input.idp_signing_certificate)
    .bind(&sp_entity_id)
    .bind(input.nameid_format.as_deref().unwrap_or("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress"))
    .bind(input.sso_binding.as_deref().unwrap_or("HTTP-POST"))
    .bind(input.attribute_email.as_deref().unwrap_or("email"))
    .bind(input.attribute_name.as_deref().unwrap_or("displayName"))
    .bind(input.auto_provision.unwrap_or(false))
    .bind(input.default_role.as_deref().unwrap_or("Employee"))
    .bind(input.default_custom_role_id)
    .bind(input.default_department_id)
    .bind(&input.email_domains.unwrap_or_default())
    .bind(input.trust_idp_mfa.unwrap_or(true))
    .bind(input.enabled.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create SAML provider: {:?}", e);
        if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
            (StatusCode::CONFLICT, Json(json!({"error": "A SAML provider with this slug already exists"})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create SAML provider"})))
        }
    })?;

    // Ensure tenant has 'saml' in auth_methods
    let _ = sqlx::query(
        r#"
        UPDATE tenants
        SET auth_methods = CASE
            WHEN NOT ('saml' = ANY(auth_methods)) THEN array_append(auth_methods, 'saml')
            ELSE auth_methods
        END,
        updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(auth.tenant_id)
    .execute(&state.pool)
    .await;

    Ok(Json(json!({
        "provider": provider,
        "sp_info": {
            "sp_entity_id": provider.sp_entity_id,
            "acs_url": format!("{}/api/auth/saml/acs", base_url),
            "metadata_url": format!("{}/api/auth/saml/metadata/{}", base_url, provider.id),
        }
    })))
}

/// Update a SAML provider.
/// PUT /api/saml/providers/{id}
pub async fn update_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateSamlProviderInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    // Verify provider belongs to tenant
    let existing: Option<SamlProvider> = sqlx::query_as(
        "SELECT * FROM tenant_saml_providers WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    let _existing = existing.ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Provider not found"}))))?;

    // Validate certificate if being updated
    if let Some(ref cert) = input.idp_signing_certificate {
        if let Err(e) = saml_crypto::parse_x509_pem(cert) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Invalid IdP signing certificate: {}", e)})),
            ));
        }
    }

    let provider: SamlProvider = sqlx::query_as(
        r#"
        UPDATE tenant_saml_providers SET
            name = COALESCE($1, name),
            slug = COALESCE($2, slug),
            provider_type = COALESCE($3, provider_type),
            idp_entity_id = COALESCE($4, idp_entity_id),
            idp_sso_url = COALESCE($5, idp_sso_url),
            idp_slo_url = COALESCE($6, idp_slo_url),
            idp_metadata_url = COALESCE($7, idp_metadata_url),
            idp_signing_certificate = COALESCE($8, idp_signing_certificate),
            nameid_format = COALESCE($9, nameid_format),
            sso_binding = COALESCE($10, sso_binding),
            attribute_email = COALESCE($11, attribute_email),
            attribute_name = COALESCE($12, attribute_name),
            auto_provision = COALESCE($13, auto_provision),
            default_role = COALESCE($14, default_role),
            default_custom_role_id = COALESCE($15, default_custom_role_id),
            default_department_id = COALESCE($16, default_department_id),
            email_domains = COALESCE($17, email_domains),
            trust_idp_mfa = COALESCE($18, trust_idp_mfa),
            enabled = COALESCE($19, enabled),
            updated_at = NOW()
        WHERE id = $20 AND tenant_id = $21
        RETURNING *
        "#,
    )
    .bind(input.name.as_deref())
    .bind(input.slug.as_deref())
    .bind(input.provider_type.as_deref())
    .bind(input.idp_entity_id.as_deref())
    .bind(input.idp_sso_url.as_deref())
    .bind(input.idp_slo_url.as_deref())
    .bind(input.idp_metadata_url.as_deref())
    .bind(input.idp_signing_certificate.as_deref())
    .bind(input.nameid_format.as_deref())
    .bind(input.sso_binding.as_deref())
    .bind(input.attribute_email.as_deref())
    .bind(input.attribute_name.as_deref())
    .bind(input.auto_provision)
    .bind(input.default_role.as_deref())
    .bind(input.default_custom_role_id)
    .bind(input.default_department_id)
    .bind(input.email_domains.as_deref())
    .bind(input.trust_idp_mfa)
    .bind(input.enabled)
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update SAML provider: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to update SAML provider"})))
    })?;

    Ok(Json(json!({ "provider": provider })))
}

/// Delete a SAML provider.
/// DELETE /api/saml/providers/{id}
pub async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    // Check for SAML-only users that would be locked out
    let saml_only_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM users u
        JOIN user_saml_identities i ON i.user_id = u.id
        WHERE i.provider_id = $1 AND u.identity_provider = 'saml' AND u.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    if saml_only_count.0 > 0 {
        return Ok(Json(json!({
            "error": "provider_has_sso_only_users",
            "message": format!("{} user(s) use only this provider for login and would be locked out. Set passwords for them first.", saml_only_count.0),
            "affected_count": saml_only_count.0,
        })));
    }

    sqlx::query("DELETE FROM tenant_saml_providers WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(auth.tenant_id)
        .execute(&state.pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    // Check if tenant still has SAML providers; if not, remove 'saml' from auth_methods
    let remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tenant_saml_providers WHERE tenant_id = $1",
    )
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    if remaining.0 == 0 {
        let _ = sqlx::query(
            "UPDATE tenants SET auth_methods = array_remove(auth_methods, 'saml'), updated_at = NOW() WHERE id = $1",
        )
        .bind(auth.tenant_id)
        .execute(&state.pool)
        .await;
    }

    Ok(Json(json!({ "success": true })))
}

/// Test a SAML provider by validating its certificate.
/// POST /api/saml/providers/{id}/test
pub async fn test_provider(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    let provider: SamlProvider = sqlx::query_as(
        "SELECT * FROM tenant_saml_providers WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Provider not found"}))))?;

    // Validate certificate
    let cert_valid = match saml_crypto::parse_x509_pem(&provider.idp_signing_certificate) {
        Ok(_) => true,
        Err(e) => {
            return Ok(Json(json!({
                "success": false,
                "error": format!("Certificate parse error: {}", e),
            })));
        }
    };

    // If metadata URL is configured, try to fetch it
    let mut metadata_valid = None;
    if let Some(ref metadata_url) = provider.idp_metadata_url {
        match reqwest::Client::new().get(metadata_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                metadata_valid = Some(true);
            }
            Ok(resp) => {
                metadata_valid = Some(false);
                tracing::warn!("Metadata URL returned status {}", resp.status());
            }
            Err(e) => {
                metadata_valid = Some(false);
                tracing::warn!("Failed to fetch metadata URL: {:?}", e);
            }
        }
    }

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    Ok(Json(json!({
        "success": cert_valid,
        "certificate_valid": cert_valid,
        "metadata_url_reachable": metadata_valid,
        "sp_entity_id": provider.sp_entity_id,
        "acs_url": format!("{}/api/auth/saml/acs", base_url),
        "metadata_url": format!("{}/api/auth/saml/metadata/{}", base_url, provider.id),
    })))
}

// ==================== Account Linking (Authenticated) ====================

/// Start SAML flow for account linking.
/// GET /api/auth/saml/link/{provider_id}
pub async fn link_saml_identity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(provider_id): Path<Uuid>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String), (StatusCode, Json<Value>)> {
    // Verify provider belongs to user's tenant
    let _provider = sqlx::query_as::<_, SamlProvider>(
        "SELECT * FROM tenant_saml_providers WHERE id = $1 AND tenant_id = $2 AND enabled = true",
    )
    .bind(provider_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Provider not found"}))))?;

    start_saml_flow(&state, provider_id, Some(auth.user_id)).await
}

/// Unlink a SAML identity.
/// DELETE /api/auth/saml/unlink/{identity_id}
pub async fn unlink_saml_identity(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(identity_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    // Verify identity belongs to user
    let identity: Option<SamlIdentity> = sqlx::query_as(
        "SELECT * FROM user_saml_identities WHERE id = $1 AND user_id = $2",
    )
    .bind(identity_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _identity = identity.ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has a password or other identities — prevent lockout
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Count remaining SSO identities (both OIDC + SAML)
    let saml_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_saml_identities WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let oidc_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_oidc_identities WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_sso = saml_count.0 + oidc_count.0;

    if user.password_hash.is_none() && total_sso <= 1 {
        return Ok(Json(json!({
            "error": "cannot_unlink",
            "message": "Cannot unlink your only login method. Set a password first.",
        })));
    }

    sqlx::query("DELETE FROM user_saml_identities WHERE id = $1 AND user_id = $2")
        .bind(identity_id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update identity_provider if no more SSO identities
    if total_sso <= 1 && user.password_hash.is_some() {
        // Check if any OIDC identities remain
        if oidc_count.0 == 0 {
            let _ = sqlx::query(
                "UPDATE users SET identity_provider = 'local', updated_at = NOW() WHERE id = $1",
            )
            .bind(auth.user_id)
            .execute(&state.pool)
            .await;
        }
    }

    Ok(Json(json!({ "success": true })))
}

/// List current user's linked SAML identities.
/// GET /api/auth/saml/identities
pub async fn list_my_identities(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let identities: Vec<SamlIdentity> = sqlx::query_as(
        "SELECT * FROM user_saml_identities WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut result = Vec::new();
    for identity in &identities {
        let provider_info: Option<(String, String, String)> = sqlx::query_as(
            "SELECT name, slug, provider_type FROM tenant_saml_providers WHERE id = $1",
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
            "protocol": "saml",
            "saml_email": identity.saml_email,
            "saml_name": identity.saml_name,
            "last_login_at": identity.last_login_at,
            "login_count": identity.login_count,
            "created_at": identity.created_at,
        }));
    }

    Ok(Json(json!({ "identities": result })))
}

// ==================== Helpers ====================

/// Get the first value for an attribute name from the SAML assertion attributes.
fn get_attribute_value(attributes: &HashMap<String, Vec<String>>, name: &str) -> Option<String> {
    // Try exact match first
    if let Some(values) = attributes.get(name) {
        return values.first().cloned();
    }
    // Try common OID-format attribute names
    let common_aliases: &[(&str, &[&str])] = &[
        ("email", &[
            "urn:oid:0.9.2342.19200300.100.1.3",
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
            "mail",
        ]),
        ("displayName", &[
            "urn:oid:2.16.840.1.113730.3.1.241",
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name",
            "cn",
        ]),
    ];
    for (canonical, aliases) in common_aliases {
        if *canonical == name {
            for alias in *aliases {
                if let Some(values) = attributes.get(*alias) {
                    return values.first().cloned();
                }
            }
        }
    }
    None
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
