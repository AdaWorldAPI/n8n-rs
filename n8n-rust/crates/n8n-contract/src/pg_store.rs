//! PostgreSQL persistence for unified executions and steps.
//!
//! Requires the `postgres` feature flag.

use crate::types::{StepStatus, UnifiedExecution, UnifiedStep};
use sqlx::PgPool;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum PgStoreError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// PostgreSQL store for unified execution data.
#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run migrations to create/update the unified_executions and unified_steps tables.
    pub async fn migrate(&self) -> Result<(), PgStoreError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS unified_executions (
                execution_id TEXT PRIMARY KEY,
                workflow_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at TIMESTAMPTZ,
                finished_at TIMESTAMPTZ,
                fork_id TEXT,
                fork_parent TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS unified_steps (
                step_id TEXT PRIMARY KEY,
                execution_id TEXT NOT NULL REFERENCES unified_executions(execution_id),
                step_type TEXT NOT NULL,
                name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                sequence INTEGER NOT NULL DEFAULT 0,
                input JSONB NOT NULL DEFAULT 'null'::jsonb,
                output JSONB NOT NULL DEFAULT 'null'::jsonb,
                error TEXT,
                started_at TIMESTAMPTZ,
                finished_at TIMESTAMPTZ,
                reasoning TEXT,
                confidence REAL,
                alternatives JSONB
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Additive migrations for existing tables (idempotent).
        sqlx::query(
            r#"
            ALTER TABLE unified_executions ADD COLUMN IF NOT EXISTS fork_id TEXT;
            ALTER TABLE unified_executions ADD COLUMN IF NOT EXISTS fork_parent TEXT;
            ALTER TABLE unified_steps ADD COLUMN IF NOT EXISTS reasoning TEXT;
            ALTER TABLE unified_steps ADD COLUMN IF NOT EXISTS confidence REAL;
            ALTER TABLE unified_steps ADD COLUMN IF NOT EXISTS alternatives JSONB;
            "#,
        )
        .execute(&self.pool)
        .await?;

        debug!("Unified contract tables migrated");
        Ok(())
    }

    /// Insert a new execution.
    pub async fn write_execution(&self, exec: &UnifiedExecution) -> Result<(), PgStoreError> {
        sqlx::query(
            r#"
            INSERT INTO unified_executions
                (execution_id, workflow_name, status, started_at, finished_at, fork_id, fork_parent)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (execution_id) DO UPDATE SET
                status = EXCLUDED.status,
                started_at = EXCLUDED.started_at,
                finished_at = EXCLUDED.finished_at,
                fork_id = EXCLUDED.fork_id,
                fork_parent = EXCLUDED.fork_parent
            "#,
        )
        .bind(&exec.execution_id)
        .bind(&exec.workflow_name)
        .bind(status_to_str(exec.status))
        .bind(exec.started_at)
        .bind(exec.finished_at)
        .bind(&exec.fork_id)
        .bind(&exec.fork_parent)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert or update a step.
    pub async fn write_step(&self, step: &UnifiedStep) -> Result<(), PgStoreError> {
        sqlx::query(
            r#"
            INSERT INTO unified_steps
                (step_id, execution_id, step_type, name, status, sequence,
                 input, output, error, started_at, finished_at,
                 reasoning, confidence, alternatives)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (step_id) DO UPDATE SET
                status = EXCLUDED.status,
                output = EXCLUDED.output,
                error = EXCLUDED.error,
                started_at = EXCLUDED.started_at,
                finished_at = EXCLUDED.finished_at,
                reasoning = EXCLUDED.reasoning,
                confidence = EXCLUDED.confidence,
                alternatives = EXCLUDED.alternatives
            "#,
        )
        .bind(&step.step_id)
        .bind(&step.execution_id)
        .bind(&step.step_type)
        .bind(&step.name)
        .bind(status_to_str(step.status))
        .bind(step.sequence)
        .bind(&step.input)
        .bind(&step.output)
        .bind(&step.error)
        .bind(step.started_at)
        .bind(step.finished_at)
        .bind(&step.reasoning)
        .bind(step.confidence.map(|c| c as f32))
        .bind(&step.alternatives)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update just the status of an execution.
    pub async fn update_status(
        &self,
        execution_id: &str,
        status: StepStatus,
    ) -> Result<(), PgStoreError> {
        let now = chrono::Utc::now();
        let finished = matches!(status, StepStatus::Completed | StepStatus::Failed);

        sqlx::query(
            r#"
            UPDATE unified_executions
            SET status = $1,
                finished_at = CASE WHEN $2 THEN $3 ELSE finished_at END
            WHERE execution_id = $4
            "#,
        )
        .bind(status_to_str(status))
        .bind(finished)
        .bind(now)
        .bind(execution_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load an execution by ID.
    pub async fn get_execution(
        &self,
        execution_id: &str,
    ) -> Result<Option<UnifiedExecution>, PgStoreError> {
        let row = sqlx::query_as::<_, ExecRow>(
            r#"
            SELECT execution_id, workflow_name, status, started_at, finished_at, fork_id, fork_parent
            FROM unified_executions
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(UnifiedExecution {
                execution_id: r.execution_id,
                workflow_name: r.workflow_name,
                status: str_to_status(&r.status),
                started_at: r.started_at,
                finished_at: r.finished_at,
                steps: Vec::new(), // caller must load steps separately if needed
                fork_id: r.fork_id,
                fork_parent: r.fork_parent,
            })),
            None => Ok(None),
        }
    }

    /// Load all steps for an execution.
    pub async fn get_steps(
        &self,
        execution_id: &str,
    ) -> Result<Vec<UnifiedStep>, PgStoreError> {
        let rows = sqlx::query_as::<_, StepRow>(
            r#"
            SELECT step_id, execution_id, step_type, name, status, sequence,
                   input, output, error, started_at, finished_at,
                   reasoning, confidence, alternatives
            FROM unified_steps
            WHERE execution_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| UnifiedStep {
                step_id: r.step_id,
                execution_id: r.execution_id,
                step_type: r.step_type,
                name: r.name,
                status: str_to_status(&r.status),
                sequence: r.sequence,
                input: r.input,
                output: r.output,
                error: r.error,
                started_at: r.started_at,
                finished_at: r.finished_at,
                reasoning: r.reasoning,
                confidence: r.confidence.map(|c| c as f64),
                alternatives: r.alternatives,
            })
            .collect())
    }
}

// Internal row types for sqlx::FromRow
#[derive(sqlx::FromRow)]
struct ExecRow {
    execution_id: String,
    workflow_name: String,
    status: String,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    fork_id: Option<String>,
    fork_parent: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StepRow {
    step_id: String,
    execution_id: String,
    step_type: String,
    name: String,
    status: String,
    sequence: i32,
    input: serde_json::Value,
    output: serde_json::Value,
    error: Option<String>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    reasoning: Option<String>,
    confidence: Option<f32>,
    alternatives: Option<serde_json::Value>,
}

fn status_to_str(s: StepStatus) -> &'static str {
    match s {
        StepStatus::Pending => "pending",
        StepStatus::Running => "running",
        StepStatus::Completed => "completed",
        StepStatus::Failed => "failed",
        StepStatus::Skipped => "skipped",
    }
}

fn str_to_status(s: &str) -> StepStatus {
    match s {
        "running" => StepStatus::Running,
        "completed" => StepStatus::Completed,
        "failed" => StepStatus::Failed,
        "skipped" => StepStatus::Skipped,
        _ => StepStatus::Pending,
    }
}
