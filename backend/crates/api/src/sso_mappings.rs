//! SSO Attribute-to-Role Mapping CRUD
//!
//! Protocol-agnostic mappings that work for both OIDC claims and SAML attributes.
//! SuperAdmin only.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use clovalink_auth::{require_super_admin, AuthUser};
use crate::AppState;

// ==================== Models ====================

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttributeMapping {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub protocol: String,
    pub provider_id: Uuid,
    pub attribute_name: String,
    pub attribute_value: String,
    pub match_type: String,
    pub target_role: String,
    pub target_custom_role_id: Option<Uuid>,
    pub target_department_id: Option<Uuid>,
    pub priority: i32,
    pub enabled: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMappingInput {
    pub attribute_name: String,
    pub attribute_value: String,
    pub match_type: Option<String>,
    pub target_role: String,
    pub target_custom_role_id: Option<Uuid>,
    pub target_department_id: Option<Uuid>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMappingInput {
    pub attribute_name: Option<String>,
    pub attribute_value: Option<String>,
    pub match_type: Option<String>,
    pub target_role: Option<String>,
    pub target_custom_role_id: Option<Uuid>,
    pub target_department_id: Option<Uuid>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

// ==================== Endpoints ====================

/// List attribute mappings for a provider
/// GET /api/sso/mappings/:protocol/:provider_id
pub async fn list_mappings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((protocol, provider_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<AttributeMapping>>, StatusCode> {
    require_super_admin(&auth)?;

    if protocol != "oidc" && protocol != "saml" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mappings = sqlx::query_as::<_, AttributeMapping>(
        r#"
        SELECT * FROM sso_attribute_mappings
        WHERE protocol = $1 AND provider_id = $2 AND tenant_id = $3
        ORDER BY priority DESC, created_at ASC
        "#,
    )
    .bind(&protocol)
    .bind(provider_id)
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list mappings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(mappings))
}

/// Create an attribute mapping
/// POST /api/sso/mappings/:protocol/:provider_id
pub async fn create_mapping(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path((protocol, provider_id)): Path<(String, Uuid)>,
    Json(input): Json<CreateMappingInput>,
) -> Result<(StatusCode, Json<AttributeMapping>), (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    if protocol != "oidc" && protocol != "saml" {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid protocol"}))));
    }

    let match_type = input.match_type.unwrap_or_else(|| "exact".to_string());
    if !["exact", "contains", "regex"].contains(&match_type.as_str()) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid match_type. Must be: exact, contains, regex"}))));
    }

    // Validate regex if match_type is regex (with size limit to prevent ReDoS)
    if match_type == "regex" {
        if let Err(e) = regex::RegexBuilder::new(&input.attribute_value)
            .size_limit(10_000)
            .build()
        {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid regex: {}", e)}))));
        }
    }

    let mapping = sqlx::query_as::<_, AttributeMapping>(
        r#"
        INSERT INTO sso_attribute_mappings (
            tenant_id, protocol, provider_id,
            attribute_name, attribute_value, match_type,
            target_role, target_custom_role_id, target_department_id,
            priority, enabled
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING *
        "#,
    )
    .bind(auth.tenant_id)
    .bind(&protocol)
    .bind(provider_id)
    .bind(&input.attribute_name)
    .bind(&input.attribute_value)
    .bind(&match_type)
    .bind(&input.target_role)
    .bind(input.target_custom_role_id)
    .bind(input.target_department_id)
    .bind(input.priority.unwrap_or(0))
    .bind(input.enabled.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create mapping: {:?}", e);
        if e.to_string().contains("duplicate") {
            (StatusCode::CONFLICT, Json(json!({"error": "Mapping already exists for this attribute name and value"})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create mapping"})))
        }
    })?;

    Ok((StatusCode::CREATED, Json(mapping)))
}

/// Update an attribute mapping
/// PUT /api/sso/mappings/:mapping_id
pub async fn update_mapping(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(mapping_id): Path<Uuid>,
    Json(input): Json<UpdateMappingInput>,
) -> Result<Json<AttributeMapping>, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    // Verify mapping belongs to this tenant
    let existing: Option<AttributeMapping> = sqlx::query_as(
        "SELECT * FROM sso_attribute_mappings WHERE id = $1 AND tenant_id = $2",
    )
    .bind(mapping_id)
    .bind(auth.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    let existing = existing.ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Mapping not found"}))))?;

    let match_type = input.match_type.unwrap_or(existing.match_type);
    if !["exact", "contains", "regex"].contains(&match_type.as_str()) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid match_type"}))));
    }

    let attr_value = input.attribute_value.unwrap_or(existing.attribute_value);
    if match_type == "regex" {
        if let Err(e) = regex::RegexBuilder::new(&attr_value)
            .size_limit(10_000)
            .build()
        {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid regex: {}", e)}))));
        }
    }

    let mapping = sqlx::query_as::<_, AttributeMapping>(
        r#"
        UPDATE sso_attribute_mappings SET
            attribute_name = $2,
            attribute_value = $3,
            match_type = $4,
            target_role = $5,
            target_custom_role_id = $6,
            target_department_id = $7,
            priority = $8,
            enabled = $9,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(mapping_id)
    .bind(input.attribute_name.unwrap_or(existing.attribute_name))
    .bind(&attr_value)
    .bind(&match_type)
    .bind(input.target_role.unwrap_or(existing.target_role))
    .bind(input.target_custom_role_id.or(existing.target_custom_role_id))
    .bind(input.target_department_id.or(existing.target_department_id))
    .bind(input.priority.unwrap_or(existing.priority))
    .bind(input.enabled.unwrap_or(existing.enabled))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update mapping: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to update mapping"})))
    })?;

    Ok(Json(mapping))
}

/// Delete an attribute mapping
/// DELETE /api/sso/mappings/:mapping_id
pub async fn delete_mapping(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Path(mapping_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    require_super_admin(&auth).map_err(|s| (s, Json(json!({"error": "Forbidden"}))))?;

    let result = sqlx::query(
        "DELETE FROM sso_attribute_mappings WHERE id = $1 AND tenant_id = $2",
    )
    .bind(mapping_id)
    .bind(auth.tenant_id)
    .execute(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Mapping not found"}))));
    }

    Ok(StatusCode::NO_CONTENT)
}
