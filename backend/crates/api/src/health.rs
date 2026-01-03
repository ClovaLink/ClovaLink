use axum::{
    extract::State,
    http::StatusCode,
    Json, Extension,
};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use clovalink_auth::AuthUser;

use crate::AppState;

/// Current application version from Cargo.toml
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Store server start time as a lazy static
static SERVER_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn mark_server_start() {
    SERVER_START.get_or_init(Instant::now);
}

fn get_uptime() -> Duration {
    SERVER_START.get().map(|start| start.elapsed()).unwrap_or_default()
}

/// Basic liveness check - just returns 200 OK
/// GET /health
pub async fn liveness() -> StatusCode {
    StatusCode::OK
}

/// Readiness check - verifies DB and Redis connections
/// GET /health/ready
pub async fn readiness(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let mut checks = Vec::new();
    let mut all_healthy = true;

    // Check database connection
    let db_healthy = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Database health check failed: {:?}", e);
            false
        }
    };
    checks.push(json!({
        "name": "database",
        "status": if db_healthy { "healthy" } else { "unhealthy" }
    }));
    if !db_healthy {
        all_healthy = false;
    }

    // Check Redis connection
    let redis_healthy = match state.cache.as_ref() {
        Some(cache) => cache.is_available().await,
        None => false,
    };
    checks.push(json!({
        "name": "redis",
        "status": if redis_healthy { "healthy" } else { "unhealthy" }
    }));
    if !redis_healthy {
        all_healthy = false;
    }

    if all_healthy {
        Ok(Json(json!({
            "status": "ready",
            "checks": checks
        })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

#[derive(Debug, Serialize)]
pub struct DetailedHealth {
    pub status: String,
    pub uptime_seconds: u64,
    pub uptime_formatted: String,
    pub version: String,
    pub timestamp: u64,
    pub checks: Vec<HealthCheck>,
    pub database: DatabaseHealth,
    pub redis: RedisHealth,
    pub storage: StorageHealth,
    pub virus_scan: VirusScanHealth,
    pub memory: MemoryInfo,
}

#[derive(Debug, Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub details: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseHealth {
    pub connected: bool,
    pub pool_size: u32,
    pub pool_idle: u32,
    pub pool_in_use: u32,
    pub latency_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct RedisHealth {
    pub connected: bool,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StorageHealth {
    pub backend: String,
    pub connected: bool,
    pub latency_ms: Option<u64>,
    pub bucket: Option<String>,
    pub replication_enabled: bool,
    pub replication_mode: Option<String>,
    pub replication_bucket: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VirusScanHealth {
    pub enabled: bool,
    pub connected: bool,
    pub version: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct MemoryInfo {
    pub rss_mb: Option<f64>,
    pub heap_mb: Option<f64>,
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Detailed health check for SuperAdmins
/// GET /api/admin/health
pub async fn detailed_health(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<DetailedHealth>, StatusCode> {
    // Only SuperAdmin can access detailed health
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut checks = Vec::new();
    let mut all_healthy = true;

    // Database check with latency
    let db_start = Instant::now();
    let db_result = sqlx::query("SELECT 1").execute(&state.pool).await;
    let db_latency = db_start.elapsed().as_millis() as u64;
    
    let db_connected = db_result.is_ok();
    if !db_connected {
        all_healthy = false;
    }

    // Get pool statistics
    let pool_size = state.pool.size();
    let pool_idle = state.pool.num_idle() as u32;
    let pool_in_use = pool_size.saturating_sub(pool_idle);

    checks.push(HealthCheck {
        name: "database".to_string(),
        status: if db_connected { "healthy" } else { "unhealthy" }.to_string(),
        latency_ms: Some(db_latency),
        details: Some(json!({
            "pool_size": pool_size,
            "pool_idle": pool_idle,
            "pool_in_use": pool_in_use
        })),
    });

    // Redis check with latency
    let (redis_connected, redis_latency) = match state.cache.as_ref() {
        Some(cache) => {
            let redis_start = Instant::now();
            let connected = cache.is_available().await;
            let latency = redis_start.elapsed().as_millis() as u64;
            (connected, Some(latency))
        }
        None => (false, None),
    };

    if !redis_connected {
        all_healthy = false;
    }

    checks.push(HealthCheck {
        name: "redis".to_string(),
        status: if redis_connected { "healthy" } else { "unhealthy" }.to_string(),
        latency_ms: redis_latency,
        details: None,
    });

    // Storage check with actual connectivity test
    let storage_type = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    let storage_bucket = if storage_type == "s3" {
        std::env::var("S3_BUCKET").ok()
    } else {
        None
    };

    // Actually test storage connectivity
    let (storage_connected, storage_latency) = match state.storage.health_check().await {
        Ok(latency) => (true, Some(latency)),
        Err(e) => {
            tracing::warn!("Storage health check failed: {}", e);
            all_healthy = false;
            (false, None)
        }
    };

    checks.push(HealthCheck {
        name: "storage".to_string(),
        status: if storage_connected { "healthy" } else { "unhealthy" }.to_string(),
        latency_ms: storage_latency,
        details: Some(json!({
            "backend": storage_type,
            "bucket": storage_bucket
        })),
    });

    // ClamAV virus scan check
    let (virus_scan_connected, virus_scan_latency, virus_scan_version) = if state.virus_scan_config.enabled {
        let client = clovalink_core::virus_scan::ClamAvClient::new(state.virus_scan_config.clone());
        let scan_start = Instant::now();
        match client.ping().await {
            Ok(true) => {
                let latency = scan_start.elapsed().as_millis() as u64;
                let version = client.version().await.ok();
                (true, Some(latency), version)
            }
            _ => (false, None, None),
        }
    } else {
        (false, None, None)
    };

    if state.virus_scan_config.enabled && !virus_scan_connected {
        // ClamAV being down is degraded, not critical
        checks.push(HealthCheck {
            name: "clamav".to_string(),
            status: "unhealthy".to_string(),
            latency_ms: virus_scan_latency,
            details: Some(json!({
                "enabled": true,
                "host": state.virus_scan_config.host,
                "port": state.virus_scan_config.port
            })),
        });
    } else if state.virus_scan_config.enabled {
        checks.push(HealthCheck {
            name: "clamav".to_string(),
            status: "healthy".to_string(),
            latency_ms: virus_scan_latency,
            details: Some(json!({
                "enabled": true,
                "version": virus_scan_version
            })),
        });
    }

    // Memory info (platform-dependent)
    let memory = get_memory_info();

    let uptime = get_uptime();
    let uptime_seconds = uptime.as_secs();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(Json(DetailedHealth {
        status: if all_healthy { "healthy" } else { "degraded" }.to_string(),
        uptime_seconds,
        uptime_formatted: format_uptime(uptime_seconds),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp,
        checks,
        database: DatabaseHealth {
            connected: db_connected,
            pool_size,
            pool_idle,
            pool_in_use,
            latency_ms: db_latency,
        },
        redis: RedisHealth {
            connected: redis_connected,
            latency_ms: redis_latency,
        },
        storage: StorageHealth {
            backend: storage_type,
            connected: storage_connected,
            latency_ms: storage_latency,
            bucket: storage_bucket,
            replication_enabled: state.replication_config.enabled,
            replication_mode: if state.replication_config.enabled {
                Some(format!("{:?}", state.replication_config.mode).to_lowercase())
            } else {
                None
            },
            replication_bucket: if state.replication_config.enabled {
                Some(state.replication_config.bucket.clone())
            } else {
                None
            },
        },
        virus_scan: VirusScanHealth {
            enabled: state.virus_scan_config.enabled,
            connected: virus_scan_connected,
            version: virus_scan_version,
            latency_ms: virus_scan_latency,
        },
        memory,
    }))
}

#[cfg(target_os = "linux")]
fn get_memory_info() -> MemoryInfo {
    use std::fs;
    
    // Read from /proc/self/status for RSS
    let rss_mb = fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|kb| kb.parse::<f64>().ok())
                        .map(|kb| kb / 1024.0)
                })
        });

    MemoryInfo {
        rss_mb,
        heap_mb: None, // Would require jemalloc stats
    }
}

#[cfg(target_os = "macos")]
fn get_memory_info() -> MemoryInfo {
    // macOS: use mach APIs or just return None
    // For simplicity, we'll return None
    MemoryInfo {
        rss_mb: None,
        heap_mb: None,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn get_memory_info() -> MemoryInfo {
    MemoryInfo {
        rss_mb: None,
        heap_mb: None,
    }
}

// =============================================================================
// Version & Update Check
// =============================================================================

#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    pub published_at: Option<String>,
    pub check_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
}

/// Get current version and check for updates from GitHub
/// GET /api/admin/version
pub async fn get_version_info(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthUser>,
) -> Result<Json<VersionInfo>, StatusCode> {
    // Only SuperAdmin can check version
    if auth.role != "SuperAdmin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let current_version = CURRENT_VERSION.to_string();

    // Try to get github_repo from global settings
    let github_repo: Option<String> = sqlx::query_scalar(
        "SELECT value::text FROM global_settings WHERE key = 'github_repo'"
    )
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    .and_then(|v: String| {
        // Remove quotes from JSON string
        let trimmed = v.trim_matches('"');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    // If no repo configured, just return current version
    let Some(repo) = github_repo else {
        return Ok(Json(VersionInfo {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            release_notes: None,
            published_at: None,
            check_error: Some("No GitHub repository configured".to_string()),
        }));
    };

    // Fetch latest release from GitHub
    let github_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    
    let client = match reqwest::Client::builder()
        .user_agent("ClovaLink-UpdateChecker/1.0")
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(Json(VersionInfo {
                current_version,
                latest_version: None,
                update_available: false,
                release_url: None,
                release_notes: None,
                published_at: None,
                check_error: Some(format!("Failed to create HTTP client: {}", e)),
            }));
        }
    };

    let response = match client.get(&github_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(Json(VersionInfo {
                current_version,
                latest_version: None,
                update_available: false,
                release_url: None,
                release_notes: None,
                published_at: None,
                check_error: Some(format!("Failed to fetch releases: {}", e)),
            }));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        return Ok(Json(VersionInfo {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            release_notes: None,
            published_at: None,
            check_error: Some(format!("GitHub API returned status {}", status)),
        }));
    }

    let release: GitHubRelease = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(Json(VersionInfo {
                current_version,
                latest_version: None,
                update_available: false,
                release_url: None,
                release_notes: None,
                published_at: None,
                check_error: Some(format!("Failed to parse release info: {}", e)),
            }));
        }
    };

    // Parse version from tag (strip leading 'v' if present)
    let latest_version = release.tag_name.trim_start_matches('v').to_string();

    // Compare versions using semver
    let update_available = match (
        semver::Version::parse(&current_version),
        semver::Version::parse(&latest_version),
    ) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => {
            // Fallback to string comparison if semver parsing fails
            latest_version != current_version
        }
    };

    // Truncate release notes if too long
    let release_notes = release.body.map(|notes| {
        if notes.len() > 500 {
            format!("{}...", &notes[..497])
        } else {
            notes
        }
    });

    Ok(Json(VersionInfo {
        current_version,
        latest_version: Some(latest_version),
        update_available,
        release_url: Some(release.html_url),
        release_notes,
        published_at: release.published_at,
        check_error: None,
    }))
}

/// Get just the current version (no GitHub check)
/// GET /api/version
pub async fn get_current_version() -> Json<Value> {
    Json(json!({
        "version": CURRENT_VERSION,
        "name": "ClovaLink"
    }))
}

