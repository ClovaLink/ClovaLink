//! Automation job scheduler using Redis for distributed job queue

use chrono::{DateTime, Utc};
use cron::Schedule;
use redis::AsyncCommands;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::AutomationJob;
use crate::webhook::{dispatch_webhook, AutomationEventPayload};
use crate::permissions::{require_permission, Permission};

const REDIS_AUTOMATION_LOCK_PREFIX: &str = "clovalink:automation:lock:";
const LOCK_TTL_SECONDS: u64 = 300; // 5 minutes

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Redis error: {0}")]
    RedisError(String),
    #[error("Job not found")]
    JobNotFound,
    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),
}

/// Parse a cron expression and get the next run time
pub fn next_run_from_cron(cron_expr: &str) -> Result<DateTime<Utc>, SchedulerError> {
    let schedule = Schedule::from_str(cron_expr)
        .map_err(|e| SchedulerError::InvalidCron(e.to_string()))?;
    
    schedule
        .upcoming(Utc)
        .next()
        .ok_or_else(|| SchedulerError::InvalidCron("No upcoming execution time".to_string()))
}

/// Validate a cron expression
pub fn validate_cron(cron_expr: &str) -> Result<(), SchedulerError> {
    Schedule::from_str(cron_expr)
        .map_err(|e| SchedulerError::InvalidCron(e.to_string()))?;
    Ok(())
}

/// Scheduler state
pub struct Scheduler {
    pool: PgPool,
    redis: redis::aio::ConnectionManager,
    running: Arc<RwLock<bool>>,
    webhook_timeout_ms: u64,
}

impl Scheduler {
    pub async fn new(
        pool: PgPool,
        redis_url: &str,
        webhook_timeout_ms: u64,
    ) -> Result<Self, SchedulerError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| SchedulerError::RedisError(e.to_string()))?;
        
        let redis = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| SchedulerError::RedisError(e.to_string()))?;

        Ok(Self {
            pool,
            redis,
            running: Arc::new(RwLock::new(false)),
            webhook_timeout_ms,
        })
    }

    /// Start the scheduler loop
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            tracing::warn!("Scheduler already running");
            return;
        }
        *running = true;
        drop(running);

        tracing::info!("Starting automation scheduler");

        loop {
            {
                let running = self.running.read().await;
                if !*running {
                    break;
                }
            }

            if let Err(e) = self.poll_and_execute().await {
                tracing::error!("Scheduler poll error: {:?}", e);
            }

            // Sleep before next poll
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        tracing::info!("Scheduler stopped");
    }

    /// Stop the scheduler
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Poll for due jobs and execute them
    async fn poll_and_execute(&self) -> Result<(), SchedulerError> {
        let now = Utc::now();

        // Find jobs that are due
        let due_jobs = sqlx::query_as!(
            AutomationJob,
            r#"
            SELECT id, extension_id, tenant_id, name, cron_expression, 
                   next_run_at, last_run_at, last_status, last_error, 
                   enabled, config, created_at, updated_at
            FROM automation_jobs
            WHERE enabled = true AND next_run_at <= $1
            ORDER BY next_run_at ASC
            LIMIT 10
            "#,
            now
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

        for job in due_jobs {
            // Try to acquire lock for this job
            if !self.acquire_lock(&job.id).await? {
                continue; // Another instance is processing this job
            }

            match self.execute_job(&job).await {
                Ok(_) => {
                    tracing::info!("Successfully executed automation job: {}", job.id);
                }
                Err(e) => {
                    tracing::error!("Failed to execute automation job {}: {:?}", job.id, e);
                    // Update job with error status
                    let _ = self.update_job_status(&job.id, "failed", Some(&e.to_string())).await;
                }
            }

            // Release lock
            let _ = self.release_lock(&job.id).await;
        }

        Ok(())
    }

    /// Acquire a distributed lock for a job
    async fn acquire_lock(&self, job_id: &Uuid) -> Result<bool, SchedulerError> {
        let key = format!("{}{}", REDIS_AUTOMATION_LOCK_PREFIX, job_id);
        let mut conn = self.redis.clone();

        let result: Option<String> = conn
            .set_ex(&key, "locked", LOCK_TTL_SECONDS)
            .await
            .map_err(|e| SchedulerError::RedisError(e.to_string()))?;

        Ok(result.is_some())
    }

    /// Release a distributed lock
    async fn release_lock(&self, job_id: &Uuid) -> Result<(), SchedulerError> {
        let key = format!("{}{}", REDIS_AUTOMATION_LOCK_PREFIX, job_id);
        let mut conn = self.redis.clone();

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| SchedulerError::RedisError(e.to_string()))?;

        Ok(())
    }

    /// Execute a single automation job
    async fn execute_job(&self, job: &AutomationJob) -> Result<(), SchedulerError> {
        // Check permission
        require_permission(
            &self.pool,
            job.extension_id,
            job.tenant_id,
            Permission::AutomationRun,
        )
        .await
        .map_err(|e| SchedulerError::ExecutionFailed(e.to_string()))?;

        // Get extension details
        let extension = sqlx::query_as!(
            crate::models::Extension,
            r#"
            SELECT id, tenant_id, name, slug, description, extension_type,
                   manifest_url, webhook_url, public_key, signature_algorithm,
                   status, allowed_tenant_ids, created_at, updated_at
            FROM extensions
            WHERE id = $1 AND status = 'active'
            "#,
            job.extension_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?
        .ok_or(SchedulerError::ExecutionFailed("Extension not found or inactive".to_string()))?;

        // Build payload
        let payload = AutomationEventPayload {
            company_id: job.tenant_id.to_string(),
            extension_id: job.extension_id.to_string(),
            job_id: job.id.to_string(),
            event: "automation_trigger".to_string(),
            config: job.config.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };

        // Dispatch webhook
        dispatch_webhook(
            &self.pool,
            &extension,
            "automation_trigger",
            &payload,
            self.webhook_timeout_ms,
        )
        .await
        .map_err(|e| SchedulerError::ExecutionFailed(e.to_string()))?;

        // Update job status and schedule next run
        self.update_job_status(&job.id, "success", None).await?;
        self.schedule_next_run(job).await?;

        Ok(())
    }

    /// Update job status after execution
    async fn update_job_status(
        &self,
        job_id: &Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), SchedulerError> {
        sqlx::query!(
            r#"
            UPDATE automation_jobs
            SET last_run_at = NOW(),
                last_status = $2,
                last_error = $3
            WHERE id = $1
            "#,
            job_id,
            status,
            error
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Schedule the next run for a job
    async fn schedule_next_run(&self, job: &AutomationJob) -> Result<(), SchedulerError> {
        if let Some(cron_expr) = &job.cron_expression {
            let next_run = next_run_from_cron(cron_expr)?;

            sqlx::query!(
                r#"
                UPDATE automation_jobs
                SET next_run_at = $2
                WHERE id = $1
                "#,
                job.id,
                next_run
            )
            .execute(&self.pool)
            .await
            .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    /// Manually trigger a job
    pub async fn trigger_job(&self, job_id: Uuid) -> Result<(), SchedulerError> {
        let job = sqlx::query_as!(
            AutomationJob,
            r#"
            SELECT id, extension_id, tenant_id, name, cron_expression, 
                   next_run_at, last_run_at, last_status, last_error, 
                   enabled, config, created_at, updated_at
            FROM automation_jobs
            WHERE id = $1
            "#,
            job_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?
        .ok_or(SchedulerError::JobNotFound)?;

        self.execute_job(&job).await
    }
}

/// Create a new automation job
pub async fn create_automation_job(
    pool: &PgPool,
    extension_id: Uuid,
    tenant_id: Uuid,
    name: &str,
    cron_expression: &str,
    config: serde_json::Value,
) -> Result<AutomationJob, SchedulerError> {
    // Validate cron expression
    validate_cron(cron_expression)?;

    let next_run = next_run_from_cron(cron_expression)?;

    let job: AutomationJob = sqlx::query_as(
        r#"
        INSERT INTO automation_jobs (extension_id, tenant_id, name, cron_expression, next_run_at, config)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, extension_id, tenant_id, name, cron_expression, 
                  next_run_at, last_run_at, last_status, last_error, 
                  enabled, config, created_at, updated_at
        "#
    )
    .bind(extension_id)
    .bind(tenant_id)
    .bind(name)
    .bind(cron_expression)
    .bind(next_run)
    .bind(config)
    .fetch_one(pool)
    .await
    .map_err(|e: sqlx::Error| SchedulerError::DatabaseError(e.to_string()))?;

    Ok(job)
}

/// Get automation jobs for an extension
pub async fn get_automation_jobs(
    pool: &PgPool,
    extension_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<AutomationJob>, SchedulerError> {
    let jobs = sqlx::query_as!(
        AutomationJob,
        r#"
        SELECT id, extension_id, tenant_id, name, cron_expression, 
               next_run_at, last_run_at, last_status, last_error, 
               enabled, config, created_at, updated_at
        FROM automation_jobs
        WHERE extension_id = $1 AND tenant_id = $2
        ORDER BY created_at DESC
        "#,
        extension_id,
        tenant_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

    Ok(jobs)
}

/// Enable or disable an automation job
pub async fn set_job_enabled(
    pool: &PgPool,
    job_id: Uuid,
    enabled: bool,
) -> Result<(), SchedulerError> {
    sqlx::query!(
        r#"
        UPDATE automation_jobs
        SET enabled = $2
        WHERE id = $1
        "#,
        job_id,
        enabled
    )
    .execute(pool)
    .await
    .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete an automation job
pub async fn delete_automation_job(
    pool: &PgPool,
    job_id: Uuid,
) -> Result<(), SchedulerError> {
    sqlx::query!(
        r#"
        DELETE FROM automation_jobs
        WHERE id = $1
        "#,
        job_id
    )
    .execute(pool)
    .await
    .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

    Ok(())
}

