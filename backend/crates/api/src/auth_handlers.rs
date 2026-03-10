use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::Json,
};
use argon2::{PasswordHash, PasswordVerifier, password_hash::{rand_core::OsRng, PasswordHasher, SaltString}};
use crate::password::get_argon2;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use totp_rs::{Algorithm, TOTP, Secret};
use uuid::Uuid;
use chrono::{Utc, Duration};
use sha2::{Sha256, Digest};
use crate::AppState;
use clovalink_auth::{generate_token, generate_token_with_fingerprint, AuthUser};
use clovalink_core::models::{LoginInput, CreateUserInput, User, Tenant, get_base_permissions, ALL_PERMISSIONS};
use clovalink_core::security_service;

#[derive(Deserialize)]
pub struct ForgotPasswordInput {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordInput {
    pub token: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct Verify2faInput {
    pub code: String,
    pub secret: Option<String>, // Optional because if already enabled, we might just be verifying for other reasons? No, for setup it's required.
}

/// Login endpoint
/// POST /api/auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<LoginInput>,
) -> Result<Json<Value>, StatusCode> {
    // Extract IP address early for security tracking
    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
        });

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND status = 'active'"
    )
    .bind(&input.email)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Handle user not found - track failed login
    let user = match user {
        Some(u) => u,
        None => {
            // Track failed login attempt for security
            let _ = security_service::record_failed_login(
                &state.pool,
                &input.email,
                ip_address.as_deref(),
                "user_not_found",
            ).await;
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Check if user is suspended
    if user.suspended_at.is_some() {
        // Check if suspension has expired
        if let Some(until) = user.suspended_until {
            if until > Utc::now() {
                // Still suspended
                return Ok(Json(json!({
                    "error": "account_suspended",
                    "message": "Your account is suspended",
                    "suspended_until": until
                })));
            }
            // Suspension expired, clear it
            let _ = sqlx::query(
                "UPDATE users SET suspended_at = NULL, suspended_until = NULL, suspension_reason = NULL WHERE id = $1"
            )
            .bind(user.id)
            .execute(&state.pool)
            .await;
        } else {
            // Indefinitely suspended
            return Ok(Json(json!({
                "error": "account_suspended",
                "message": "Your account is suspended indefinitely"
            })));
        }
    }

    // Check if this user requires SSO login (no password set, or SSO-only)
    if user.identity_provider == "oidc" || user.identity_provider == "saml" || user.password_hash.is_none() || user.password_hash.as_deref() == Some("") {
        let oidc_providers: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, name, slug FROM tenant_oidc_providers WHERE tenant_id = $1 AND enabled = true"
        )
        .bind(user.tenant_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

        let saml_providers: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, name, slug FROM tenant_saml_providers WHERE tenant_id = $1 AND enabled = true"
        )
        .bind(user.tenant_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

        let mut provider_list: Vec<serde_json::Value> = oidc_providers
            .iter()
            .map(|(id, name, slug)| json!({"id": id, "name": name, "slug": slug, "protocol": "oidc"}))
            .collect();
        for (id, name, slug) in &saml_providers {
            provider_list.push(json!({"id": id, "name": name, "slug": slug, "protocol": "saml"}));
        }

        return Ok(Json(json!({
            "error": "sso_required",
            "message": "This account uses Single Sign-On. Please sign in with your identity provider.",
            "providers": provider_list,
        })));
    }

    // Verify password using Argon2 with tuned parameters
    let argon2 = get_argon2();
    let parsed_hash = PasswordHash::new(user.password_hash.as_deref().unwrap_or(""))
        .map_err(|e| {
            tracing::error!("Failed to parse password hash: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let password_valid = argon2.verify_password(input.password.as_bytes(), &parsed_hash).is_ok();

    if !password_valid {
        // Track failed login attempt for security
        let _ = security_service::record_failed_login(
            &state.pool,
            &input.email,
            ip_address.as_deref(),
            "invalid_password",
        ).await;
        return Err(StatusCode::UNAUTHORIZED);
    }

    let tenant = sqlx::query_as::<_, Tenant>(
        "SELECT * FROM tenants WHERE id = $1"
    )
    .bind(user.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if tenant/company is suspended
    let mut active_tenant = tenant.clone();
    let mut switched_tenant = false;
    
    if tenant.status == "suspended" {
        // Check if user has access to other active tenants
        let mut fallback_tenant: Option<Tenant> = None;
        
        if let Some(ref allowed_ids) = user.allowed_tenant_ids {
            for tenant_id in allowed_ids {
                if *tenant_id == user.tenant_id {
                    continue; // Skip the suspended primary tenant
                }
                let other_tenant: Option<Tenant> = sqlx::query_as(
                    "SELECT * FROM tenants WHERE id = $1 AND status = 'active'"
                )
                .bind(tenant_id)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten();
                
                if let Some(t) = other_tenant {
                    fallback_tenant = Some(t);
                    break;
                }
            }
        }
        
        if let Some(fb_tenant) = fallback_tenant {
            // User has access to another active tenant - use that instead
            active_tenant = fb_tenant;
            switched_tenant = true;
            tracing::info!(
                "User {} primary tenant {} is suspended, switching to fallback tenant {}",
                user.id, user.tenant_id, active_tenant.id
            );
        } else {
            // No fallback tenants available - user is locked out
            return Ok(Json(json!({
                "error": "company_suspended",
                "message": "Your company has been suspended. Please contact your administrator."
            })));
        }
    }

    // Check 2FA (use active_tenant which may be a fallback)
    if active_tenant.enable_totp.unwrap_or(false) && user.totp_secret.is_some() {
        if let Some(code) = input.code {
            let secret = Secret::Encoded(user.totp_secret.clone().unwrap());
            let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret.to_bytes().unwrap(), None, "".to_string()).unwrap();
            
            if !totp.check_current(&code).unwrap_or(false) {
                // Track failed 2FA attempt
                let _ = security_service::record_failed_login(
                    &state.pool,
                    &input.email,
                    ip_address.as_deref(),
                    "invalid_2fa_code",
                ).await;
                return Err(StatusCode::UNAUTHORIZED);
            }
        } else {
            // Require 2FA
            return Ok(Json(json!({
                "require_2fa": true,
                "user_id": user.id
            })));
        }
    }

    let _ = sqlx::query("UPDATE users SET last_active_at = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await;

    // Extract device info from User-Agent header
    let device_info = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    // Generate session fingerprint for theft detection BEFORE generating token
    // Combines: User-Agent + Accept-Language + partial IP (first 3 octets)
    let fingerprint_hash = {
        let accept_language = headers
            .get("accept-language")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        
        // Extract first 3 octets of IP (for privacy, don't use full IP)
        let partial_ip = ip_address
            .as_ref()
            .map(|ip| {
                let parts: Vec<&str> = ip.split('.').take(3).collect();
                if parts.len() == 3 {
                    parts.join(".")
                } else {
                    // IPv6 or invalid - use first segment
                    ip.split(':').next().unwrap_or("unknown").to_string()
                }
            })
            .unwrap_or_else(|| "unknown".to_string());
        
        let fingerprint_data = format!(
            "{}|{}|{}",
            device_info.as_deref().unwrap_or(""),
            accept_language,
            partial_ip
        );
        
        let mut hasher = Sha256::new();
        hasher.update(fingerprint_data.as_bytes());
        hex::encode(hasher.finalize())
    };

    // Generate token with fingerprint embedded for the active tenant
    let token = generate_token_with_fingerprint(
        user.id, 
        active_tenant.id, 
        user.role.clone(),
        Some(fingerprint_hash.clone()),
    )
        .map_err(|e| {
            tracing::error!("Token generation error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Create session record for tracking active sessions
    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    };
    
    // Upsert session: update existing session from same device or create new one
    // This prevents duplicate sessions from the same browser/device
    let session_result = sqlx::query(
        r#"
        INSERT INTO user_sessions (user_id, token_hash, device_info, ip_address, fingerprint_hash, expires_at)
        VALUES ($1, $2, $3, $4::inet, $5, NOW() + INTERVAL '7 days')
        ON CONFLICT (user_id, fingerprint_hash) WHERE is_revoked = false AND fingerprint_hash IS NOT NULL
        DO UPDATE SET 
            token_hash = EXCLUDED.token_hash,
            device_info = EXCLUDED.device_info,
            ip_address = EXCLUDED.ip_address,
            last_active_at = NOW(),
            expires_at = EXCLUDED.expires_at
        "#
    )
    .bind(user.id)
    .bind(&token_hash)
    .bind(&device_info)
    .bind(&ip_address)
    .bind(&fingerprint_hash)
    .execute(&state.pool)
    .await;
    
    if let Err(e) = session_result {
        tracing::warn!("Failed to create/update session record: {:?}", e);
        // Don't fail login if session tracking fails
    }

    // Track login IP for security (detect new IP logins)
    let _ = security_service::check_and_record_login_ip(
        &state.pool,
        user.id,
        active_tenant.id,
        ip_address.as_deref(),
        device_info.as_deref(),
        &user.email,
    ).await;

    // Get user's resolved permissions based on their role
    let permissions = get_user_permissions(&state.pool, active_tenant.id, &user.role).await?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user.id,
            "email": user.email,
            "name": user.name,
            "role": user.role,
            "avatar_url": user.avatar_url,
            "permissions": permissions,
        },
        "tenant": {
            "id": active_tenant.id,
            "name": active_tenant.name,
            "domain": active_tenant.domain,
            "plan": active_tenant.plan,
            "compliance_mode": active_tenant.compliance_mode,
            "retention_policy_days": active_tenant.retention_policy_days,
            "data_export_enabled": active_tenant.data_export_enabled.unwrap_or(true),
            "approval_workflow_enabled": active_tenant.approval_workflow_enabled.unwrap_or(false),
        },
        "primary_tenant_suspended": switched_tenant,
        "suspended_tenant_name": if switched_tenant { Some(&tenant.name) } else { None }
    })))
}

/// Forgot Password
/// POST /api/auth/forgot-password
pub async fn forgot_password(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ForgotPasswordInput>,
) -> Result<Json<Value>, StatusCode> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND status = 'active'"
    )
    .bind(&input.email)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(user) = user {
        let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
            .bind(user.tenant_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Generate recovery token
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(1);

        sqlx::query(
            "UPDATE users SET recovery_token = $1, recovery_token_expires_at = $2 WHERE id = $3"
        )
        .bind(&token)
        .bind(expires_at)
        .bind(user.id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Send email
        let reset_link = format!("https://{}/reset-password?token={}", tenant.domain, token);
        let body = format!("Click here to reset your password: {}", reset_link);
        
        let _ = clovalink_core::mailer::send_email(&tenant, &user.email, "Password Reset", &body).await;
    }

    Ok(Json(json!({"success": true})))
}

/// Reset Password
/// POST /api/auth/reset-password
pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ResetPasswordInput>,
) -> Result<Json<Value>, StatusCode> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE recovery_token = $1 AND recovery_token_expires_at > NOW()"
    )
    .bind(&input.token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::BAD_REQUEST)?;

    // Validate password against tenant's password policy
    crate::users::validate_password_against_policy(&state.pool, user.tenant_id, &input.new_password)
        .await
        .map_err(|(status, _json)| status)?;

    // Hash new password with tuned Argon2 parameters
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = get_argon2();
    let password_hash = argon2.hash_password(input.new_password.as_bytes(), &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_string();

    sqlx::query(
        "UPDATE users SET password_hash = $1, recovery_token = NULL, recovery_token_expires_at = NULL WHERE id = $2"
    )
    .bind(password_hash)
    .bind(user.id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"success": true})))
}

/// Get password policy for a tenant (public endpoint)
/// GET /api/auth/password-policy
/// Can be called without auth - uses tenant domain or logged-in user's tenant
#[derive(Deserialize)]
pub struct PasswordPolicyQuery {
    pub domain: Option<String>,
}

pub async fn get_password_policy(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<PasswordPolicyQuery>,
    auth: Option<axum::Extension<AuthUser>>,
) -> Result<Json<Value>, StatusCode> {
    use crate::settings::PasswordPolicy;
    
    // Determine tenant_id: from auth if logged in, or from domain query param
    let tenant_id = if let Some(axum::Extension(auth_user)) = auth {
        Some(auth_user.tenant_id)
    } else if let Some(domain) = query.domain {
        // Look up tenant by domain
        let tenant_id: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM tenants WHERE domain = $1 OR name = $1"
        )
        .bind(&domain)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        tenant_id.map(|(id,)| id)
    } else {
        None
    };
    
    // Fetch the password policy
    let policy: PasswordPolicy = if let Some(tid) = tenant_id {
        let policy_result: Option<(Value,)> = sqlx::query_as(
            "SELECT password_policy FROM tenants WHERE id = $1"
        )
        .bind(tid)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        match policy_result {
            Some((json_value,)) => serde_json::from_value(json_value).unwrap_or_default(),
            None => PasswordPolicy::default(),
        }
    } else {
        // Return default policy if no tenant specified
        PasswordPolicy::default()
    };
    
    Ok(Json(json!({
        "min_length": policy.min_length,
        "require_uppercase": policy.require_uppercase,
        "require_lowercase": policy.require_lowercase,
        "require_number": policy.require_number,
        "require_special": policy.require_special,
        "max_age_days": policy.max_age_days,
        "prevent_reuse": policy.prevent_reuse
    })))
}

/// Setup 2FA
/// POST /api/auth/2fa/setup
pub async fn setup_2fa(
    axum::Extension(auth): axum::Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, Secret::generate_secret().to_bytes().unwrap(), None, format!("{}@clovalink.com", auth.user_id)).unwrap();
    let secret = totp.get_secret_base32();
    let qr = totp.get_qr_base64().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return secret to client, do not save yet to avoid lockout
    Ok(Json(json!({
        "qr_code": qr,
        "secret": secret
    })))
}

/// Verify 2FA
/// POST /api/auth/2fa/verify
pub async fn verify_2fa(
    State(state): State<Arc<AppState>>,
    axum::Extension(auth): axum::Extension<AuthUser>,
    Json(input): Json<Verify2faInput>,
) -> Result<Json<Value>, StatusCode> {
    let secret_str = input.secret.ok_or(StatusCode::BAD_REQUEST)?;
    let secret = Secret::Encoded(secret_str.clone());
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret.to_bytes().unwrap(), None, "".to_string()).unwrap();

    if !totp.check_current(&input.code).unwrap_or(false) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Save secret to user, enabling 2FA
    sqlx::query("UPDATE users SET totp_secret = $1 WHERE id = $2")
        .bind(secret_str)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"success": true})))
}

/// Register/Create user endpoint

/// POST /api/auth/register
/// Note: In production, you might want to restrict this or require admin auth
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateUserInput>,
) -> Result<Json<Value>, StatusCode> {
    // Hash the password using Argon2
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = get_argon2();
    let password = input.password.ok_or(StatusCode::BAD_REQUEST)?;
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Failed to hash password: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .to_string();

    let tenant = sqlx::query!("SELECT id FROM tenants WHERE status = 'active' LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Insert user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (tenant_id, email, name, password_hash, role)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#
    )
    .bind(tenant.id)
    .bind(&input.email)
    .bind(&input.name)
    .bind(&password_hash)
    .bind(&input.role)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create user: {:?}", e);
        // Check if it's a unique constraint violation (duplicate email)
        if e.to_string().contains("unique") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    // Generate JWT token
    let token = generate_token(user.id, user.tenant_id, user.role.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "token": token,
        "user": {
            "id": user.id,
            "email": user.email,
            "name": user.name,
            "role": user.role,
        }
    })))
}

/// Get current user info

/// GET /api/auth/me
/// Cached user/tenant response for /api/auth/me
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct MeResponse {
    user: UserInfo,
    tenant: TenantInfo,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct UserInfo {
    id: Uuid,
    email: String,
    name: String,
    role: String,
    avatar_url: Option<String>,
    last_active_at: Option<chrono::DateTime<chrono::Utc>>,
    dashboard_layout: Option<Value>,
    widget_config: Option<Value>,
    permissions: Vec<String>,
}

/// Get resolved permissions for a user based on their role
/// Returns only the permissions that are granted (either by base role or custom override)
async fn get_user_permissions(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    role_name: &str,
) -> Result<Vec<String>, StatusCode> {
    // SuperAdmin always has all permissions
    if role_name == "SuperAdmin" {
        return Ok(ALL_PERMISSIONS.iter().map(|s| s.to_string()).collect());
    }
    
    // Look up the role in the roles table
    // First check for tenant-specific role, then global role
    let role: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, base_role FROM roles 
        WHERE name = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
        ORDER BY tenant_id DESC NULLS LAST
        LIMIT 1
        "#
    )
    .bind(role_name)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch role: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let (role_id, base_role) = match role {
        Some(r) => r,
        None => {
            // No role found - use role_name as base_role for backwards compatibility
            tracing::warn!("Role '{}' not found in roles table, using as base role", role_name);
            return Ok(get_base_permissions(role_name).iter().map(|s| s.to_string()).collect());
        }
    };
    
    // Get base permissions for this role level
    let base_perms: Vec<&str> = get_base_permissions(&base_role);
    
    // Get custom permission overrides for this role
    let custom_perms: Vec<(String, bool)> = sqlx::query_as(
        "SELECT permission, granted FROM role_permissions WHERE role_id = $1"
    )
    .bind(role_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch role permissions: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Build the list of granted permissions
    let mut granted_permissions: Vec<String> = vec![];
    
    for perm in ALL_PERMISSIONS {
        let is_base = base_perms.contains(perm);
        let custom = custom_perms.iter().find(|(p, _)| p == *perm);
        
        let granted = match custom {
            Some((_, g)) => *g,  // Custom override
            None => is_base,     // Use base permission
        };
        
        if granted {
            granted_permissions.push(perm.to_string());
        }
    }
    
    Ok(granted_permissions)
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct TenantInfo {
    id: Uuid,
    name: String,
    domain: String,
    plan: String,
    compliance_mode: String,
    retention_policy_days: i32,
    data_export_enabled: bool,
    approval_workflow_enabled: bool,
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    axum::Extension(auth): axum::Extension<clovalink_auth::AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    use clovalink_core::cache::{keys, ttl};
    
    let cache_key = keys::user(auth.user_id);
    
    // Try to get from cache first
    if let Some(ref cache) = state.cache {
        if let Ok(cached) = cache.get::<MeResponse>(&cache_key).await {
            return Ok(Json(json!(cached)));
        }
    }
    
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Use tenant_id from JWT (auth.tenant_id) not from user record
    // This ensures we show the correct tenant after switching
    let tenant: (Uuid, String, String, String, String, i32, Option<bool>, Option<bool>) = sqlx::query_as(
        "SELECT id, name, domain, plan, compliance_mode, retention_policy_days, data_export_enabled, approval_workflow_enabled FROM tenants WHERE id = $1"
    )
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get user's resolved permissions based on their role
    let permissions = get_user_permissions(&state.pool, auth.tenant_id, &user.role).await?;

    let response = MeResponse {
        user: UserInfo {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role,
            avatar_url: user.avatar_url,
            last_active_at: user.last_active_at,
            dashboard_layout: user.dashboard_layout,
            widget_config: user.widget_config,
            permissions,
        },
        tenant: TenantInfo {
            id: tenant.0,
            name: tenant.1,
            domain: tenant.2,
            plan: tenant.3,
            compliance_mode: tenant.4,
            retention_policy_days: tenant.5,
            data_export_enabled: tenant.6.unwrap_or(true),
            approval_workflow_enabled: tenant.7.unwrap_or(false),
        },
    };
    
    // Cache the result
    if let Some(ref cache) = state.cache {
        if let Err(e) = cache.set(&cache_key, &response, ttl::USER).await {
            tracing::warn!("Failed to cache user me response: {}", e);
        }
    }

    Ok(Json(json!(response)))
}



