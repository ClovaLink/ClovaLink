#![allow(deprecated)] // TimeoutLayer::new is deprecated but replacement API not stable

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use clovalink_storage::{S3Storage, LocalStorage, Storage};
use clovalink_extensions::routes::ExtensionState;
use clovalink_core::cache::Cache;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

mod handlers;
mod auth_handlers;
mod file_requests;
mod users;
mod tenants;
mod departments;
mod settings;
mod audit;
mod roles;
mod search;
mod cron;
mod extensions;
mod dashboard;
mod notifications;
mod global_settings;
mod email_templates;
mod security;
mod password;
mod health;
mod api_usage;
mod virus_scan;
mod ai;
mod text_extract;
mod discord;
mod comments;
mod sharing;
mod groups;
pub mod compliance;
pub mod middleware;

use middleware::{TransferScheduler, ApiUsageWriter, ApiUsageState};

/// Adapter to make the storage implement PrimaryStorageReader for replication
struct PrimaryStorageAdapter(Arc<dyn Storage>);

#[async_trait::async_trait]
impl clovalink_core::replication::PrimaryStorageReader for PrimaryStorageAdapter {
    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        self.0.download(key).await
    }
}

/// Adapter to make the storage implement FileStorageReader for virus scanning
struct VirusScanStorageAdapter(Arc<dyn Storage>);

#[async_trait::async_trait]
impl clovalink_core::virus_scan::FileStorageReader for VirusScanStorageAdapter {
    async fn download(&self, key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        self.0.download(key).await
    }
    
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.delete(key).await
    }
}

// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub storage: Arc<dyn Storage>,
    pub redis_url: String,
    pub cache: Option<Cache>,
    pub extension_webhook_timeout_ms: u64,
    // CDN / Presigned URL configuration
    pub use_presigned_urls: bool,
    pub presigned_url_expiry: u64,
    pub cdn_domain: Option<String>,
    // Transfer scheduling for downloads/uploads
    pub scheduler: Arc<TransferScheduler>,
    // S3 Replication configuration
    pub replication_config: clovalink_core::replication::ReplicationConfig,
    // Virus scanning configuration
    pub virus_scan_config: clovalink_core::virus_scan::VirusScanConfig,
    // ClamAV circuit breaker (shared across workers)
    pub clamav_circuit_breaker: Option<Arc<clovalink_core::circuit_breaker::CircuitBreaker>>,
}


#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize Storage
    let storage_type = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    let storage: Arc<dyn Storage> = if storage_type == "s3" {
        let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "clovalink-bucket".to_string());
        Arc::new(S3Storage::new(bucket).await)
    } else {
        Arc::new(LocalStorage::new("uploads"))
    };

    // Initialize Redis URL
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    // Initialize Redis Cache
    let cache = match Cache::new(&redis_url).await {
        Ok(c) => {
            tracing::info!("Redis cache initialized successfully");
            Some(c)
        }
        Err(e) => {
            tracing::warn!("Failed to initialize Redis cache (caching disabled): {}", e);
            None
        }
    };
    
    // Extension webhook timeout
    let extension_webhook_timeout_ms: u64 = std::env::var("EXTENSION_WEBHOOK_TIMEOUT_MS")
        .unwrap_or_else(|_| "5000".to_string())
        .parse()
        .unwrap_or(5000);

    // Initialize Database with production pool settings
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    // Pool configuration from environment or defaults
    let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "50".to_string())
        .parse()
        .unwrap_or(50);
    let min_connections: u32 = std::env::var("DB_MIN_CONNECTIONS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .unwrap_or(5);
    let acquire_timeout_secs: u64 = std::env::var("DB_ACQUIRE_TIMEOUT_SECS")
        .unwrap_or_else(|_| "3".to_string())
        .parse()
        .unwrap_or(3);
    let idle_timeout_secs: u64 = std::env::var("DB_IDLE_TIMEOUT_SECS")
        .unwrap_or_else(|_| "600".to_string())
        .parse()
        .unwrap_or(600);
    let max_lifetime_secs: u64 = std::env::var("DB_MAX_LIFETIME_SECS")
        .unwrap_or_else(|_| "1800".to_string())
        .parse()
        .unwrap_or(1800);
    
    tracing::info!("Connecting to database (max_conn: {}, min_conn: {})...", max_connections, min_connections);
    
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(idle_timeout_secs))
        .max_lifetime(Duration::from_secs(max_lifetime_secs))
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    tracing::info!("Database connected successfully with optimized pool settings");

    // Run migrations if needed
    sqlx::migrate!("../../migrations").run(&pool).await.expect("Failed to run migrations");

    // CDN / Presigned URL configuration (optional, disabled by default for backwards compatibility)
    let use_presigned_urls = std::env::var("USE_PRESIGNED_URLS")
        .map(|v| v == "true")
        .unwrap_or(false);
    let presigned_url_expiry: u64 = std::env::var("PRESIGNED_URL_EXPIRY_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600); // Default 1 hour
    let cdn_domain = std::env::var("CDN_DOMAIN").ok();
    
    if use_presigned_urls {
        tracing::info!("Presigned URLs enabled (expiry: {}s, CDN: {:?})", presigned_url_expiry, cdn_domain);
    }

    // Initialize transfer scheduler for prioritized downloads/uploads
    let scheduler = Arc::new(TransferScheduler::new());
    tracing::info!("Transfer scheduler initialized");
    
    // Load S3 replication configuration
    let replication_config = clovalink_core::replication::ReplicationConfig::from_env();
    if replication_config.enabled {
        if let Err(e) = replication_config.validate() {
            tracing::error!("Replication configuration error: {}. Disabling replication.", e);
        } else {
            tracing::info!(
                "S3 replication enabled: mode={:?}, bucket={}, workers={}",
                replication_config.mode,
                replication_config.bucket,
                replication_config.workers
            );
        }
    } else {
        tracing::info!("S3 replication disabled");
    }

    // Mark server start time for uptime tracking
    health::mark_server_start();

    // Initialize API usage tracking
    let api_usage_enabled = std::env::var("API_USAGE_TRACKING")
        .map(|v| v == "true")
        .unwrap_or(true); // Enabled by default
    
    let api_usage_writer = if api_usage_enabled {
        tracing::info!("API usage tracking enabled");
        Some(Arc::new(ApiUsageWriter::new(pool.clone())))
    } else {
        tracing::info!("API usage tracking disabled");
        None
    };

    // Load virus scan configuration
    let virus_scan_config = clovalink_core::virus_scan::VirusScanConfig::from_env();
    if virus_scan_config.enabled {
        tracing::info!(
            "ClamAV virus scanning enabled: host={}, port={}, workers={}",
            virus_scan_config.host,
            virus_scan_config.port,
            virus_scan_config.workers
        );
    } else {
        tracing::info!("ClamAV virus scanning disabled");
    }

    // Create ClamAV circuit breaker if virus scanning is enabled
    // This is shared across all workers and exposed via metrics API
    let clamav_circuit_breaker = if virus_scan_config.enabled {
        Some(Arc::new(clovalink_core::circuit_breaker::CircuitBreaker::new(
            "clamav",
            5,   // failure threshold - opens after 5 consecutive failures
            30,  // recovery timeout - tries half-open after 30 seconds
            3,   // success threshold - closes after 3 successes in half-open
        )))
    } else {
        None
    };

    let app_state = Arc::new(AppState {
        pool: pool.clone(),
        storage: storage.clone(),
        redis_url: redis_url.clone(),
        cache,
        extension_webhook_timeout_ms,
        use_presigned_urls,
        presigned_url_expiry,
        cdn_domain,
        scheduler,
        replication_config: replication_config.clone(),
        virus_scan_config: virus_scan_config.clone(),
        clamav_circuit_breaker: clamav_circuit_breaker.clone(),
    });

    // Extension state for extension routes
    let extension_state = Arc::new(ExtensionState {
        pool: pool.clone(),
        redis_url: redis_url.clone(),
        webhook_timeout_ms: extension_webhook_timeout_ms,
    });

    // Start automation scheduler in background
    let scheduler_pool = pool.clone();
    let scheduler_redis_url = redis_url.clone();
    tokio::spawn(async move {
        match clovalink_extensions::scheduler::Scheduler::new(
            scheduler_pool,
            &scheduler_redis_url,
            extension_webhook_timeout_ms,
        ).await {
            Ok(scheduler) => {
                tracing::info!("Starting automation scheduler...");
                scheduler.start().await;
            }
            Err(e) => {
                tracing::error!("Failed to start automation scheduler: {:?}", e);
            }
        }
    });
    
    // Start S3 replication workers if enabled
    if replication_config.enabled && replication_config.validate().is_ok() {
        let worker_count = replication_config.workers;
        tracing::info!("Starting {} replication workers...", worker_count);
        
        for worker_id in 0..worker_count {
            let worker_pool = pool.clone();
            let worker_config = replication_config.clone();
            let worker_storage = storage.clone();
            
            tokio::spawn(async move {
                // Wrap storage in PrimaryStorageAdapter
                let storage_reader = Arc::new(PrimaryStorageAdapter(worker_storage));
                
                match clovalink_core::replication::ReplicationWorker::new(
                    worker_pool,
                    worker_config,
                    storage_reader,
                    worker_id,
                ).await {
                    Ok(worker) => {
                        worker.run().await;
                    }
                    Err(e) => {
                        tracing::error!(
                            worker_id = worker_id,
                            error = %e,
                            "Failed to start replication worker"
                        );
                    }
                }
            });
        }
    }

    // Start virus scan workers if enabled
    if virus_scan_config.enabled {
        let worker_count = virus_scan_config.workers;
        tracing::info!("Starting {} virus scan workers...", worker_count);
        
        // Use the circuit breaker we created earlier (stored in AppState for metrics access)
        let cb = clamav_circuit_breaker.clone().expect("Circuit breaker should exist when ClamAV is enabled");
        
        for worker_id in 0..worker_count {
            let worker_pool = pool.clone();
            let worker_config = virus_scan_config.clone();
            let worker_storage = storage.clone();
            let worker_circuit_breaker = cb.clone();
            
            tokio::spawn(async move {
                let storage_reader = Arc::new(VirusScanStorageAdapter(worker_storage));
                let worker = clovalink_core::virus_scan::VirusScanWorker::new(
                    worker_pool,
                    worker_config,
                    storage_reader,
                    worker_id,
                    worker_circuit_breaker,
                );
                worker.run().await;
            });
        }
    }

    // Configure CORS - production-safe with allowlist
    let cors = configure_cors();

    // Build application routes
    // Login routes with strict rate limiting (5/min per IP)
    let login_routes = Router::new()
        .route("/api/auth/login", post(auth_handlers::login))
        .route("/api/auth/register", post(auth_handlers::register))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::rate_limit::rate_limit_login,
        ));

    // Health check routes (no rate limiting - needed for load balancers/monitoring)
    let health_routes = Router::new()
        .route("/health", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .with_state(app_state.clone());

    // Other public routes with moderate rate limiting (60/min per IP)
    let public_routes = Router::new()
        .route("/", get(root))
        .route("/api/version", get(health::get_current_version))
        .route("/api/auth/forgot-password", post(auth_handlers::forgot_password))
        .route("/api/auth/reset-password", post(auth_handlers::reset_password))
        .route("/api/auth/password-policy", get(auth_handlers::get_password_policy))
        .route("/api/public-upload/{token}", post(file_requests::public_upload))
        // File sharing public endpoints
        .route("/api/share/{token}", get(handlers::download_shared_file))
        .route("/api/share/{token}/info", get(handlers::get_share_info))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::rate_limit::rate_limit_public,
        ));

    let protected_routes = Router::new()
        .route("/api/auth/me", get(auth_handlers::me))
        .route("/api/auth/2fa/setup", post(auth_handlers::setup_2fa))
        .route("/api/auth/2fa/verify", post(auth_handlers::verify_2fa))
        .route("/api/users/me/export", get(users::export_data))
        .route("/api/users/me/profile", put(users::update_my_profile))
        .route("/api/users/me/password", put(users::change_password))
        .route("/api/users/me/avatar", post(users::upload_avatar))
        .route("/api/users/me/sessions", get(users::list_sessions))
        .route("/api/users/me/sessions/{id}", delete(users::revoke_session))
        .route("/api/users/me/preferences", get(users::get_preferences).put(users::update_preferences))
        
        // Admin endpoints
        .route("/api/admin/migrate-content-hashes", post(handlers::migrate_content_hashes))
        .route("/api/admin/health", get(health::detailed_health))
        .route("/api/admin/version", get(health::get_version_info))
        
        // API Usage / Performance Monitoring (SuperAdmin only)
        .route("/api/admin/usage/summary", get(api_usage::get_usage_summary))
        .route("/api/admin/usage/by-tenant", get(api_usage::get_usage_by_tenant))
        .route("/api/admin/usage/by-user", get(api_usage::get_usage_by_user))
        .route("/api/admin/usage/by-endpoint", get(api_usage::get_usage_by_endpoint))
        .route("/api/admin/usage/slow-requests", get(api_usage::get_slow_requests))
        .route("/api/admin/usage/timeseries", get(api_usage::get_usage_timeseries))
        .route("/api/admin/usage/errors", get(api_usage::get_recent_errors))
        .route("/api/admin/usage/error-summary", get(api_usage::get_error_summary))
        .route("/api/admin/usage/aggregate", post(api_usage::aggregate_hourly_stats))
        .route("/api/admin/usage/cleanup", post(api_usage::cleanup_old_usage))
        
        // Virus Scanning
        .route("/api/admin/virus-scan/settings", get(virus_scan::get_settings).put(virus_scan::update_settings))
        .route("/api/admin/virus-scan/metrics", get(virus_scan::get_metrics))
        .route("/api/admin/virus-scan/results", get(virus_scan::get_scan_results))
        .route("/api/admin/virus-scan/quarantine", get(virus_scan::get_quarantined_files))
        .route("/api/admin/virus-scan/quarantine/{id}", delete(virus_scan::delete_quarantined_file))
        .route("/api/admin/virus-scan/rescan/{file_id}", post(virus_scan::rescan_file))
        .route("/api/admin/virus-scan/config", get(virus_scan::get_global_config))
        
        // Global Search
        .route("/api/search", get(search::global_search))
        
        // File Requests
        .route("/api/file-requests", 
            get(file_requests::list_file_requests)
            .post(file_requests::create_file_request)
        )
        .route("/api/file-requests/{id}", 
            get(file_requests::get_file_request)
            .delete(file_requests::delete_file_request)
        )
        .route("/api/file-requests/{id}/uploads", 
            get(file_requests::get_file_request_uploads)
        )
        .route("/api/file-requests/{id}/permanent",
            delete(file_requests::permanent_delete_file_request)
        )
        
        // Users
        .route("/api/users", 
            get(users::list_users)
            .post(users::create_user)
        )
        .route("/api/users/{id}", 
            put(users::update_user)
            .delete(users::delete_user)
        )
        .route("/api/users/{id}/suspend", post(users::suspend_user))
        .route("/api/users/{id}/unsuspend", post(users::unsuspend_user))
        .route("/api/users/{id}/suspension", get(users::get_suspension_status))
        .route("/api/users/{id}/reset-password", post(users::admin_reset_password))
        .route("/api/users/{id}/send-reset-email", post(users::send_password_reset_email))
        .route("/api/users/{id}/change-email", post(users::admin_change_email))
        .route("/api/users/{id}/permanent", delete(users::permanent_delete_user))
        .route("/api/users/{id}/activity-logs", get(audit::get_user_activity_logs))
        
        // Tenants/Companies
        .route("/api/tenants", 
            get(tenants::list_tenants)
            .post(tenants::create_tenant)
        )
        // IMPORTANT: specific routes must come before parameterized routes
        .route("/api/tenants/accessible", get(tenants::accessible_tenants))
        .route("/api/tenants/switch/{tenant_id}", post(tenants::switch_tenant))
        .route("/api/tenants/{id}/smtp/test", post(tenants::test_smtp))
        .route("/api/tenants/{id}/edit", put(tenants::edit_my_company))
        .route("/api/tenants/{id}/suspend", post(tenants::suspend_tenant))
        .route("/api/tenants/{id}/unsuspend", post(tenants::unsuspend_tenant))
        .route("/api/tenants/{id}", put(tenants::update_tenant).delete(tenants::delete_tenant))
        
        // Departments
        .route("/api/departments", 
            get(departments::list_departments)
            .post(departments::create_department)
        )
        .route("/api/departments/{id}", 
            put(departments::update_department)
            .delete(departments::delete_department)
        )
        
        // Settings
        .route("/api/settings/compliance", 
            get(settings::get_compliance)
            .put(settings::update_compliance)
        )
        .route("/api/settings/blocked-extensions",
            get(settings::get_blocked_extensions)
            .put(settings::update_blocked_extensions)
        )
        .route("/api/settings/password-policy",
            get(settings::get_password_policy)
            .put(settings::update_password_policy)
        )
        .route("/api/settings/ip-restrictions",
            get(settings::get_ip_restrictions)
            .put(settings::update_ip_restrictions)
        )
        
        // Global Settings (app-wide, SuperAdmin only for updates)
        .route("/api/global-settings", 
            get(global_settings::get_global_settings)
            .put(global_settings::update_global_settings)
        )
        .route("/api/global-settings/logo", 
            post(global_settings::upload_logo)
            .delete(global_settings::delete_logo)
        )
        .route("/api/global-settings/favicon", 
            post(global_settings::upload_favicon)
            .delete(global_settings::delete_favicon)
        )
        
        // Global Email Templates (SuperAdmin)
        .route("/api/email-templates", get(email_templates::list_global_templates))
        .route("/api/email-templates/{key}", 
            get(email_templates::get_global_template)
            .put(email_templates::update_global_template)
        )
        
        // Tenant Email Templates (Admin)
        .route("/api/settings/email-templates", get(email_templates::list_tenant_templates))
        .route("/api/settings/email-templates/{key}", 
            get(email_templates::get_tenant_template)
            .put(email_templates::update_tenant_template)
            .delete(email_templates::reset_tenant_template)
        )
        .route("/api/settings/email-templates/{key}/preview", post(email_templates::preview_template))
        
        // Dashboard
        .route("/api/dashboard/stats", get(dashboard::get_dashboard_stats))
        .route("/api/dashboard/file-types", get(dashboard::get_file_types))
        
        // S3 Replication Admin (SuperAdmin only)
        .route("/api/admin/replication/status", get(dashboard::get_replication_status))
        .route("/api/admin/replication/pending", get(dashboard::get_replication_jobs))
        .route("/api/admin/replication/retry-failed", post(dashboard::retry_failed_jobs))
        
        // Notifications
        .route("/api/notifications", get(notifications::list_notifications))
        .route("/api/notifications/unread-count", get(notifications::get_unread_count))
        .route("/api/notifications/read-all", put(notifications::mark_all_as_read))
        .route("/api/notifications/preferences", 
            get(notifications::get_preferences)
            .put(notifications::update_preferences)
        )
        .route("/api/notifications/preferences-with-company", get(notifications::get_preferences_with_company_settings))
        .route("/api/notifications/preference-labels", get(notifications::get_preference_labels))
        .route("/api/notifications/{id}/read", put(notifications::mark_as_read))
        .route("/api/notifications/{id}", delete(notifications::delete_notification))
        
        // Tenant Notification Settings
        .route("/api/tenants/{id}/notification-settings", 
            get(notifications::get_tenant_notification_settings)
            .put(notifications::update_tenant_notification_settings)
        )
        
        // Compliance
        .route("/api/compliance/restrictions", get(compliance::get_compliance_restrictions))
        
        // Security Alerts
        .route("/api/security/alerts", get(security::list_alerts))
        .route("/api/security/alerts/stats", get(security::get_alert_stats))
        .route("/api/security/alerts/badge", get(security::get_alert_badge))
        .route("/api/security/alerts/bulk", post(security::bulk_alert_action))
        .route("/api/security/alerts/{id}/resolve", post(security::resolve_alert))
        .route("/api/security/alerts/{id}/dismiss", post(security::dismiss_alert))
        .route("/api/compliance/consent", post(compliance::record_consent))
        .route("/api/compliance/consent/user/{user_id}", get(compliance::get_consent_status))
        .route("/api/compliance/consent/revoke/{consent_type}", delete(compliance::revoke_consent))
        
        // GDPR
        .route("/api/gdpr/deletion-request", post(compliance::create_deletion_request))
        .route("/api/gdpr/deletion-requests", get(compliance::list_deletion_requests))
        .route("/api/gdpr/deletion-requests/{id}/process", post(compliance::process_deletion_request))
        
        // Audit Logs
        .route("/api/activity-logs", get(audit::list_activity_logs))
        .route("/api/activity-logs/export", get(audit::export_activity_logs))
        .route("/api/activity-logs/actions", get(audit::get_action_types))
        .route("/api/activity-logs/resource-types", get(audit::get_resource_types))
        .route("/api/audit-settings", 
            get(audit::get_audit_settings)
            .put(audit::update_audit_settings)
        )
        
        // Roles
        .route("/api/roles", 
            get(roles::list_roles)
            .post(roles::create_role)
        )
        .route("/api/roles/{id}", 
            get(roles::get_role)
            .put(roles::update_role)
            .delete(roles::delete_role)
        )
        .route("/api/roles/{id}/permissions", 
            get(roles::get_role_permissions_handler)
            .put(roles::update_role_permissions)
        )
        
        // File Management
        .route("/api/upload/{company_id}", 
            post(handlers::upload_file)
                .layer(axum::extract::DefaultBodyLimit::disable())
        )
        .route("/api/files/{company_id}", get(handlers::list_files))
        .route("/api/files/{company_id}/export", get(handlers::export_files))
        .route("/api/download/{company_id}/{file_id}", get(handlers::download_file))
        .route("/api/folders/{company_id}", post(handlers::create_folder))
        .route("/api/files/{company_id}/rename", post(handlers::rename_file))
        .route("/api/files/{company_id}/delete", post(handlers::delete_file))
        .route("/api/files/{company_id}/{file_id}/lock", post(handlers::lock_file))
        .route("/api/files/{company_id}/{file_id}/unlock", post(handlers::unlock_file))
        .route("/api/files/{company_id}/{file_id}/move", put(handlers::move_file))
        .route("/api/files/{company_id}/{file_id}/copy", post(handlers::copy_file))
        .route("/api/files/{company_id}/{file_id}/star", post(handlers::toggle_star))
        .route("/api/files/{company_id}/starred", get(handlers::get_starred))
        .route("/api/files/{company_id}/{file_id}/activity", get(handlers::get_file_activity))
        .route("/api/files/{company_id}/{file_id}/company-folder", put(handlers::toggle_company_folder))
        .route("/api/files/{company_id}/{file_id}/share", post(handlers::create_file_share))
        // File Comments
        .route("/api/files/{company_id}/{file_id}/comments", get(comments::list_comments).post(comments::create_comment))
        .route("/api/files/{company_id}/{file_id}/comments/count", get(comments::get_comment_count))
        .route("/api/files/{company_id}/{file_id}/comments/{comment_id}", put(comments::update_comment).delete(comments::delete_comment))
        // User-Specific Sharing
        .route("/api/users/{company_id}/shareable", get(sharing::list_shareable_users))
        .route("/api/shared-with-me", get(sharing::list_shared_with_me))
        .route("/api/shared-with-me/copy", post(sharing::copy_to_my_files))
        // File Groups
        .route("/api/groups/{company_id}", get(groups::list_groups).post(groups::create_group))
        .route("/api/groups/{company_id}/{group_id}", put(groups::update_group).delete(groups::delete_group))
        .route("/api/groups/{company_id}/{group_id}/files", get(groups::get_group_files))
        .route("/api/groups/{company_id}/{group_id}/move", put(groups::move_group_to_folder))
        .route("/api/groups/{company_id}/{group_id}/star", post(groups::toggle_group_star))
        .route("/api/groups/{company_id}/{group_id}/lock", post(groups::lock_group))
        .route("/api/groups/{company_id}/{group_id}/unlock", post(groups::unlock_group))
        .route("/api/files/{company_id}/{file_id}/group", post(groups::add_file_to_group).delete(groups::remove_file_from_group))
        .route("/api/trash/{company_id}", get(handlers::list_trash))
        .route("/api/trash/{company_id}/restore/{filename}", post(handlers::restore_file))
        .route("/api/trash/{company_id}/delete/{filename}", post(handlers::permanent_delete))
        .route("/api/prefs/{company_id}", get(handlers::get_prefs).post(handlers::update_prefs))
        
        // Cron Jobs
        .route("/api/cron/cleanup", post(cron::cleanup_expired_files))
        .route("/api/cron/expiring-requests", post(cron::notify_expiring_requests))
        .route("/api/cron/storage-warnings", post(cron::check_storage_quotas))
        
        // AI Features (per-tenant, role-based)
        .route("/api/ai/status", get(ai::get_ai_status))
        .route("/api/ai/settings", get(ai::get_ai_settings).put(ai::update_ai_settings))
        .route("/api/ai/test", post(ai::test_ai_connection))
        .route("/api/ai/usage", get(ai::get_ai_usage))
        .route("/api/ai/summarize", post(ai::summarize_file))
        .route("/api/ai/answer", post(ai::answer_question))
        .route("/api/ai/search", post(ai::semantic_search))
        .route("/api/ai/providers", get(ai::get_providers))
        
        // Discord OAuth & DM Notifications
        .route("/api/discord/settings", get(discord::get_discord_settings))
        .route("/api/discord/settings/update", post(discord::update_discord_settings))
        .route("/api/discord/status", get(discord::get_connection_status))
        .route("/api/discord/connect", get(discord::start_oauth))
        .route("/api/discord/callback", get(discord::oauth_callback))
        .route("/api/discord/disconnect", post(discord::disconnect))
        .route("/api/discord/preferences", post(discord::update_preferences))
        .route("/api/discord/test", post(discord::test_connection))
        
        // SECURITY: Use auth_middleware_with_db to check user suspension status on every request
        .layer(axum::middleware::from_fn_with_state(
            app_state.pool.clone(),
            clovalink_auth::middleware::auth_middleware_with_db
        ));

    // Extension routes (protected)
    let extension_routes = Router::new()
        .route("/api/extensions/register", post(clovalink_extensions::routes::register_extension))
        .route("/api/extensions/install/{extension_id}", post(clovalink_extensions::routes::install_extension))
        .route("/api/extensions/list", get(clovalink_extensions::routes::list_extensions))
        .route("/api/extensions/installed", get(clovalink_extensions::routes::list_installed_extensions))
        .route("/api/extensions/validate-manifest", post(clovalink_extensions::routes::validate_manifest))
        .route("/api/extensions/ui", get(clovalink_extensions::routes::get_ui_extensions))
        .route("/api/extensions/trigger/automation/{job_id}", post(clovalink_extensions::routes::trigger_automation))
        .route("/api/extensions/{id}/settings", put(clovalink_extensions::routes::update_extension_settings))
        .route("/api/extensions/{id}/access", put(clovalink_extensions::routes::update_extension_access))
        .route("/api/extensions/{id}", delete(clovalink_extensions::routes::uninstall_extension))
        .route("/api/extensions/{extension_id}/jobs", 
            get(clovalink_extensions::routes::list_jobs)
            .post(clovalink_extensions::routes::create_job)
        )
        .route("/api/extensions/{extension_id}/logs", get(clovalink_extensions::routes::get_webhook_logs))
        // SECURITY: Use auth_middleware_with_db to check user suspension status
        .layer(axum::middleware::from_fn_with_state(
            app_state.pool.clone(),
            clovalink_auth::middleware::auth_middleware_with_db
        ))
        .with_state(extension_state);

    // Apply state to protected routes before merging
    let protected_routes_with_state = protected_routes.with_state(app_state.clone());

    // Concurrency and timeout configuration from environment
    let max_concurrent_requests: usize = std::env::var("MAX_CONCURRENT_REQUESTS")
        .unwrap_or_else(|_| "1000".to_string())
        .parse()
        .unwrap_or(1000);
    let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
        .unwrap_or_else(|_| "300".to_string()) // 5 minutes default for file uploads
        .parse()
        .unwrap_or(300);

    tracing::info!(
        "Server configured: max_concurrent={}, request_timeout={}s",
        max_concurrent_requests,
        request_timeout_secs
    );

    // Public uploads route (avatars, etc.) - serves from storage (works with both local and S3)
    let uploads_routes = Router::new()
        .route("/uploads/{*path}", get(handlers::serve_upload))
        .with_state(app_state.clone());

    let mut app = Router::new()
        .merge(health_routes)  // Health checks first (no rate limiting)
        .merge(login_routes.with_state(app_state.clone()))
        .merge(public_routes.with_state(app_state.clone()))
        .merge(protected_routes_with_state)
        .merge(extension_routes)
        .merge(uploads_routes);

    // Add API usage tracking middleware if enabled
    if let Some(writer) = api_usage_writer {
        let usage_state = ApiUsageState { writer };
        app = app.layer(axum::middleware::from_fn_with_state(
            usage_state,
            middleware::api_usage::api_usage_middleware,
        ));
    }

    let app = app
        // Increase body size limit for file uploads (default is 2MB)
        .layer(tower_http::limit::RequestBodyLimitLayer::new(500 * 1024 * 1024)) // 500MB - match nginx config
        // Request timeout - reject requests that take too long
        .layer(tower_http::timeout::TimeoutLayer::new(Duration::from_secs(request_timeout_secs)))
        // Concurrency limit - prevent server overload (also provides load shedding by rejecting when at capacity)
        .layer(tower::limit::ConcurrencyLimitLayer::new(max_concurrent_requests))
        .layer(cors);

    // Run server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    
    tracing::info!("🚀 Server listening on {}", listener.local_addr().unwrap());
    // Use into_make_service_with_connect_info to make SocketAddr available to rate limiting middleware
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap();
}

async fn root() -> &'static str {
    "ClovaLink Backend API v2.0 - Multi-Tenant Edition with Extensions"
}

/// Configure CORS with production-safe defaults
/// 
/// Environment variables:
/// - CORS_ALLOWED_ORIGINS: Comma-separated list of allowed origins (required in production)
/// - CORS_DEV_MODE: Set to "true" to allow localhost origins (for development)
/// - ENVIRONMENT: Set to "production" to enforce strict CORS (default behavior)
fn configure_cors() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{CorsLayer, AllowOrigin, AllowMethods, AllowHeaders};
    use axum::http::{Method, HeaderName, header};
    
    let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string());
    let dev_mode = std::env::var("CORS_DEV_MODE")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    let allowed_origins_str = std::env::var("CORS_ALLOWED_ORIGINS").ok();
    
    // Allowed methods - restrict to actual API methods
    let allowed_methods = AllowMethods::list([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
        Method::PATCH,
    ]);
    
    // Allowed headers - restrict to necessary ones
    let allowed_headers = AllowHeaders::list([
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::ORIGIN,
        HeaderName::from_static("x-requested-with"),
        HeaderName::from_static("x-tenant-id"),
    ]);
    
    // Build origin policy
    let allow_origin = if dev_mode || environment == "development" {
        // Development mode: allow localhost origins + any configured origins
        tracing::warn!("CORS: Development mode enabled - allowing localhost origins");
        
        let mut origins: Vec<String> = vec![
            "http://localhost:3000".to_string(),
            "http://localhost:5173".to_string(),
            "http://localhost:8080".to_string(),
            "http://127.0.0.1:3000".to_string(),
            "http://127.0.0.1:5173".to_string(),
            "http://127.0.0.1:8080".to_string(),
        ];
        
        // Add any explicitly configured origins
        if let Some(ref configured) = allowed_origins_str {
            for origin in configured.split(',') {
                let trimmed = origin.trim().to_string();
                if !trimmed.is_empty() && !origins.contains(&trimmed) {
                    origins.push(trimmed);
                }
            }
        }
        
        tracing::info!("CORS: Allowed origins: {:?}", origins);
        
        AllowOrigin::predicate(move |origin, _| {
            if let Ok(origin_str) = origin.to_str() {
                origins.iter().any(|allowed| allowed == origin_str)
            } else {
                false
            }
        })
    } else if let Some(ref configured) = allowed_origins_str {
        // Production mode with explicit allowlist
        let origins: Vec<String> = configured
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if origins.is_empty() {
            tracing::error!("CORS: CORS_ALLOWED_ORIGINS is empty in production mode!");
            // Fail safe - block all cross-origin requests
            AllowOrigin::predicate(|_, _| false)
        } else {
            tracing::info!("CORS: Production mode with {} allowed origins", origins.len());
            AllowOrigin::predicate(move |origin, _| {
                if let Ok(origin_str) = origin.to_str() {
                    origins.iter().any(|allowed| allowed == origin_str)
                } else {
                    false
                }
            })
        }
    } else {
        // Production mode without allowlist - fail safe
        tracing::error!(
            "CORS: No CORS_ALLOWED_ORIGINS configured in production mode! \
            Set CORS_ALLOWED_ORIGINS or enable CORS_DEV_MODE=true for development."
        );
        // Block all cross-origin requests
        AllowOrigin::predicate(|_, _| false)
    };
    
    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(allowed_methods)
        .allow_headers(allowed_headers)
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600)) // Cache preflight for 1 hour
}
