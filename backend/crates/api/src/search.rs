use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppState;
use clovalink_auth::AuthUser;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub result_type: String, // "company", "user", "file"
    pub link: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub companies: Vec<SearchResult>,
    pub users: Vec<SearchResult>,
    pub files: Vec<SearchResult>,
    pub groups: Vec<SearchResult>,
    pub total: i64,
}

#[derive(FromRow)]
struct CompanyRow {
    id: Uuid,
    name: String,
    domain: String,
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    name: String,
    email: String,
}

#[derive(FromRow)]
struct FileRow {
    id: Uuid,
    name: String,
    parent_path: Option<String>,
    #[allow(dead_code)]
    tenant_id: Uuid,
    is_directory: bool,
}

#[derive(FromRow)]
#[allow(dead_code)]
struct GroupRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    parent_path: Option<String>,
    tenant_id: Uuid,
    department_id: Option<Uuid>,
    visibility: String,
    owner_id: Option<Uuid>,
    is_locked: Option<bool>,
}

/// Global search across companies, users, and files
/// GET /api/search?q=query
/// 
/// SECURITY: Results are filtered based on user role and permissions:
/// - Companies: SuperAdmin only
/// - Users: Admin/SuperAdmin only  
/// - Files: Filtered by department access and lock status
pub async fn global_search(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Value>, StatusCode> {
    let query = params.q.trim();
    if query.is_empty() || query.len() < 2 {
        return Ok(Json(json!(SearchResponse {
            companies: vec![],
            users: vec![],
            files: vec![],
            groups: vec![],
            total: 0,
        })));
    }

    let limit = params.limit.unwrap_or(5).min(20);
    let search_pattern = format!("%{}%", query.to_lowercase());
    
    // Determine effective role - check if this is a custom role and get its base_role
    let effective_role = get_effective_role(&state.pool, auth.tenant_id, &auth.role).await;
    
    // Determine role-based access using effective role
    let is_super_admin = effective_role == "SuperAdmin";
    let is_admin = effective_role == "Admin" || is_super_admin;
    let is_manager = effective_role == "Manager" || is_admin;

    // ========================================================================
    // COMPANY SEARCH - SuperAdmin only
    // ========================================================================
    let companies = if is_super_admin {
        sqlx::query_as::<_, CompanyRow>(
            r#"
            SELECT id, name, domain
            FROM tenants
            WHERE LOWER(name) LIKE $1 OR LOWER(domain) LIKE $1
            ORDER BY name ASC
            LIMIT $2
            "#
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        // Non-SuperAdmin cannot see company search results
        vec![]
    };

    // ========================================================================
    // USER SEARCH - Admin/SuperAdmin only
    // ========================================================================
    let users = if is_super_admin {
        // SuperAdmin sees all users
        sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, name, email
            FROM users
            WHERE LOWER(name) LIKE $1 OR LOWER(email) LIKE $1
            ORDER BY name ASC
            LIMIT $2
            "#
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else if is_admin {
        // Admin sees users in their tenant only
        sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, name, email
            FROM users
            WHERE tenant_id = $1 AND (LOWER(name) LIKE $2 OR LOWER(email) LIKE $2)
            ORDER BY name ASC
            LIMIT $3
            "#
        )
        .bind(auth.tenant_id)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        // Non-admin roles cannot see user search results
        vec![]
    };

    // ========================================================================
    // FILE SEARCH - Role-based with department and lock filtering
    // ========================================================================
    let files = if is_super_admin {
        // SuperAdmin sees all files across all tenants
        sqlx::query_as::<_, FileRow>(
            r#"
            SELECT id, name, parent_path, tenant_id, is_directory
            FROM files_metadata
            WHERE LOWER(name) LIKE $1 AND is_deleted = false
            ORDER BY name ASC
            LIMIT $2
            "#
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else if is_admin {
        // Admin sees all files in their tenant (including locked)
        sqlx::query_as::<_, FileRow>(
            r#"
            SELECT id, name, parent_path, tenant_id, is_directory
            FROM files_metadata
            WHERE tenant_id = $1 
              AND LOWER(name) LIKE $2 
              AND is_deleted = false
            ORDER BY name ASC
            LIMIT $3
            "#
        )
        .bind(auth.tenant_id)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        // Manager/Employee/Custom roles: filter by department access and lock status
        // First get user's department info
        let user_info: Option<(Option<Uuid>, Option<Vec<Uuid>>)> = sqlx::query_as(
            "SELECT department_id, allowed_department_ids FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
        
        let (user_dept_id, allowed_dept_ids) = user_info.unwrap_or((None, None));
        let allowed_depts = allowed_dept_ids.unwrap_or_default();
        
        // Build the file query with proper permission filtering
        // Files visible to non-admin:
        // 1. File has no department (root level / company-wide)
        // 2. File is in user's primary department
        // 3. File is in one of user's allowed departments
        // 4. User owns the file
        // 5. File visibility is not 'private' OR user owns it
        // AND for locked files: user must be locker, owner, or have manager+ role
        sqlx::query_as::<_, FileRow>(
            r#"
            SELECT f.id, f.name, f.parent_path, f.tenant_id, f.is_directory
            FROM files_metadata f
            WHERE f.tenant_id = $1 
              AND LOWER(f.name) LIKE $2 
              AND f.is_deleted = false
              AND (
                -- Department access check
                f.department_id IS NULL
                OR f.department_id = $3
                OR f.department_id = ANY($4)
                OR f.owner_id = $5
              )
              AND (
                -- Visibility check: private files only visible to owner
                f.visibility != 'private' OR f.owner_id = $5
              )
              AND (
                -- Lock check: locked files only visible to locker, owner, or managers
                f.is_locked = false
                OR f.locked_by = $5
                OR f.owner_id = $5
                OR $6 = true
              )
            ORDER BY f.name ASC
            LIMIT $7
            "#
        )
        .bind(auth.tenant_id)
        .bind(&search_pattern)
        .bind(user_dept_id)
        .bind(&allowed_depts)
        .bind(auth.user_id)
        .bind(is_manager) // Managers can see locked files
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    };

    // ========================================================================
    // FILE GROUPS SEARCH - Role-based with department, visibility, and lock filtering
    // ========================================================================
    let groups = if is_super_admin {
        // SuperAdmin sees all groups across all tenants
        sqlx::query_as::<_, GroupRow>(
            r#"
            SELECT id, name, description, parent_path, tenant_id, department_id, visibility, owner_id, is_locked
            FROM file_groups
            WHERE LOWER(name) LIKE $1
            ORDER BY name ASC
            LIMIT $2
            "#
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else if is_admin {
        // Admin sees all groups in their tenant
        sqlx::query_as::<_, GroupRow>(
            r#"
            SELECT id, name, description, parent_path, tenant_id, department_id, visibility, owner_id, is_locked
            FROM file_groups
            WHERE tenant_id = $1 
              AND LOWER(name) LIKE $2
            ORDER BY name ASC
            LIMIT $3
            "#
        )
        .bind(auth.tenant_id)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        // Manager/Employee/Custom roles: filter by department access, visibility, lock status
        // Get user's department info (reuse from files query if available)
        let user_info: Option<(Option<Uuid>, Option<Vec<Uuid>>)> = sqlx::query_as(
            "SELECT department_id, allowed_department_ids FROM users WHERE id = $1"
        )
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);
        
        let (user_dept_id, allowed_dept_ids) = user_info.unwrap_or((None, None));
        let allowed_depts = allowed_dept_ids.unwrap_or_default();
        
        // Groups visible to non-admin:
        // 1. Group is in user's primary department
        // 2. Group is in one of user's allowed departments
        // 3. User owns the group (for private groups)
        // 4. Group is inside a company folder (visible to all)
        // AND visibility = 'department' OR (visibility = 'private' AND user owns it)
        // AND for locked groups: user must be locker, owner, or have manager+ role
        sqlx::query_as::<_, GroupRow>(
            r#"
            SELECT g.id, g.name, g.description, g.parent_path, g.tenant_id, g.department_id, g.visibility, g.owner_id, g.is_locked
            FROM file_groups g
            WHERE g.tenant_id = $1 
              AND LOWER(g.name) LIKE $2
              AND (
                -- Department access check
                g.department_id = $3
                OR g.department_id = ANY($4)
                OR g.owner_id = $5
                -- Also include groups in company folders (visible to all)
                OR EXISTS (
                    SELECT 1 FROM files_metadata fm 
                    WHERE fm.tenant_id = g.tenant_id 
                    AND fm.is_directory = true 
                    AND fm.is_deleted = false 
                    AND COALESCE(fm.is_company_folder, false) = true
                    AND (
                        g.parent_path = fm.name 
                        OR g.parent_path LIKE fm.name || '/%'
                        OR (fm.parent_path IS NOT NULL AND g.parent_path LIKE fm.parent_path || '/' || fm.name || '%')
                    )
                )
              )
              AND (
                -- Visibility check: private groups only visible to owner
                g.visibility != 'private' OR g.owner_id = $5
              )
              AND (
                -- Lock check: locked groups only visible to locker, owner, or managers
                COALESCE(g.is_locked, false) = false
                OR g.locked_by = $5
                OR g.owner_id = $5
                OR $6 = true
              )
            ORDER BY g.name ASC
            LIMIT $7
            "#
        )
        .bind(auth.tenant_id)
        .bind(&search_pattern)
        .bind(user_dept_id)
        .bind(&allowed_depts)
        .bind(auth.user_id)
        .bind(is_manager) // Managers can see locked groups
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    };

    // Convert to response format
    let company_results: Vec<SearchResult> = companies
        .into_iter()
        .map(|c| SearchResult {
            id: c.id.to_string(),
            name: c.name.clone(),
            description: Some(c.domain),
            result_type: "company".to_string(),
            link: format!("/companies/{}", urlencoding::encode(&c.name)),
        })
        .collect();

    let user_results: Vec<SearchResult> = users
        .into_iter()
        .map(|u| SearchResult {
            id: u.id.to_string(),
            name: u.name.clone(),
            description: Some(u.email),
            result_type: "user".to_string(),
            link: format!("/users?id={}", u.id),
        })
        .collect();

    let file_results: Vec<SearchResult> = files
        .into_iter()
        .map(|f| {
            let parent = f.parent_path.as_deref().unwrap_or("");
            let link_path = if f.is_directory {
                // For folders, navigate INTO the folder
                if parent.is_empty() || parent == "/" {
                    format!("/{}", f.name)
                } else {
                    format!("{}/{}", parent, f.name)
                }
            } else {
                // For files, navigate to the containing folder
                parent.to_string()
            };
            SearchResult {
                id: f.id.to_string(),
                name: f.name.clone(),
                description: f.parent_path.clone(),
                result_type: if f.is_directory { "folder" } else { "file" }.to_string(),
                link: format!("/files?path={}", urlencoding::encode(&link_path)),
            }
        })
        .collect();

    let group_results: Vec<SearchResult> = groups
        .into_iter()
        .map(|g| {
            let parent = g.parent_path.as_deref().unwrap_or("");
            // Link navigates to the folder containing the group and opens the group
            let link = if parent.is_empty() {
                format!("/files?group={}", g.id)
            } else {
                format!("/files?path={}&group={}", urlencoding::encode(parent), g.id)
            };
            SearchResult {
                id: g.id.to_string(),
                name: g.name.clone(),
                description: g.description.or(Some(format!("in {}", if parent.is_empty() { "Home" } else { parent }))),
                result_type: "group".to_string(),
                link,
            }
        })
        .collect();

    let total = (company_results.len() + user_results.len() + file_results.len() + group_results.len()) as i64;

    Ok(Json(json!(SearchResponse {
        companies: company_results,
        users: user_results,
        files: file_results,
        groups: group_results,
        total,
    })))
}

/// Get the effective role for permission checking.
/// If the user has a custom role, returns its base_role.
/// Otherwise returns the role as-is.
async fn get_effective_role(pool: &sqlx::PgPool, tenant_id: Uuid, role: &str) -> String {
    // Standard roles are returned as-is
    let standard_roles = ["SuperAdmin", "Admin", "Manager", "Employee"];
    if standard_roles.contains(&role) {
        return role.to_string();
    }
    
    // Look up custom role's base_role
    let base_role: Option<(String,)> = sqlx::query_as(
        "SELECT base_role FROM roles WHERE tenant_id = $1 AND name = $2"
    )
    .bind(tenant_id)
    .bind(role)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    
    // Return base_role if found, otherwise default to Employee
    base_role.map(|r| r.0).unwrap_or_else(|| "Employee".to_string())
}
