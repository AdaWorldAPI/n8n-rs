//! PostgreSQL store for unified execution records
//!
//! Same schema as crewai-rust — both repos create the same tables using
//! `CREATE TABLE IF NOT EXISTS`. Shares the same Railway PostgreSQL database.

#![allow(dead_code)]

use super::types::StepStatus;

#[cfg(feature = "postgres")]
use anyhow::Result;
#[cfg(feature = "postgres")]
use chrono::Utc;
#[cfg(feature = "postgres")]
use serde_json::Value;
#[cfg(feature = "postgres")]
use super::types::{UnifiedExecution, UnifiedStep};

/// PostgreSQL store for unified execution contract data.
///
/// Writes execution and step records to shared tables that are
/// readable by all three runtimes (ada-n8n, crewai-rust, ladybug-rs).
#[cfg(feature = "postgres")]
pub struct PgStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PgStore {
    /// Create a new PgStore and run migrations.
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Run database migrations — creates tables if they don't exist.
    ///
    /// This SQL is identical across all three repos. Each repo calls
    /// `CREATE TABLE IF NOT EXISTS` so whichever starts first creates
    /// the tables, and the others are no-ops.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS unified_executions (
                execution_id   TEXT PRIMARY KEY,
                runtime        TEXT NOT NULL,
                workflow_name  TEXT NOT NULL,
                status         TEXT NOT NULL DEFAULT 'pending',
                trigger        TEXT NOT NULL DEFAULT 'manual',
                input          JSONB NOT NULL DEFAULT '{}',
                output         JSONB NOT NULL DEFAULT '{}',
                started_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                finished_at    TIMESTAMPTZ,
                step_count     INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS unified_steps (
                step_id        TEXT PRIMARY KEY,
                execution_id   TEXT NOT NULL REFERENCES unified_executions(execution_id),
                step_type      TEXT NOT NULL,
                runtime        TEXT NOT NULL,
                name           TEXT NOT NULL,
                status         TEXT NOT NULL DEFAULT 'pending',
                input          JSONB NOT NULL DEFAULT '{}',
                output         JSONB NOT NULL DEFAULT '{}',
                error          TEXT,
                started_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                finished_at    TIMESTAMPTZ,
                sequence       INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_unified_steps_execution
                ON unified_steps(execution_id);
            CREATE INDEX IF NOT EXISTS idx_unified_executions_runtime
                ON unified_executions(runtime);
            CREATE INDEX IF NOT EXISTS idx_unified_steps_runtime
                ON unified_steps(runtime);
            CREATE INDEX IF NOT EXISTS idx_unified_executions_status
                ON unified_executions(status);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Write an execution record to the database.
    pub async fn write_execution(&self, exec: &UnifiedExecution) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO unified_executions
                (execution_id, runtime, workflow_name, status, trigger,
                 input, output, started_at, finished_at, step_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (execution_id) DO UPDATE SET
                status = EXCLUDED.status,
                output = EXCLUDED.output,
                finished_at = EXCLUDED.finished_at,
                step_count = EXCLUDED.step_count
            "#,
        )
        .bind(&exec.execution_id)
        .bind(&exec.runtime)
        .bind(&exec.workflow_name)
        .bind(exec.status.to_string())
        .bind(&exec.trigger)
        .bind(&exec.input)
        .bind(&exec.output)
        .bind(exec.started_at)
        .bind(exec.finished_at)
        .bind(exec.step_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Write a step record to the database.
    pub async fn write_step(&self, step: &UnifiedStep) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO unified_steps
                (step_id, execution_id, step_type, runtime, name, status,
                 input, output, error, started_at, finished_at, sequence)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (step_id) DO UPDATE SET
                status = EXCLUDED.status,
                output = EXCLUDED.output,
                error = EXCLUDED.error,
                finished_at = EXCLUDED.finished_at
            "#,
        )
        .bind(&step.step_id)
        .bind(&step.execution_id)
        .bind(&step.step_type)
        .bind(&step.runtime)
        .bind(&step.name)
        .bind(step.status.to_string())
        .bind(&step.input)
        .bind(&step.output)
        .bind(&step.error)
        .bind(step.started_at)
        .bind(step.finished_at)
        .bind(step.sequence)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Read an execution record by ID.
    pub async fn read_execution(&self, execution_id: &str) -> Result<Option<UnifiedExecution>> {
        let row = sqlx::query_as::<_, ExecutionRow>(
            r#"
            SELECT execution_id, runtime, workflow_name, status, trigger,
                   input, output, started_at, finished_at, step_count
            FROM unified_executions
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// Read all steps for an execution, ordered by sequence.
    pub async fn read_steps(&self, execution_id: &str) -> Result<Vec<UnifiedStep>> {
        let rows = sqlx::query_as::<_, StepRow>(
            r#"
            SELECT step_id, execution_id, step_type, runtime, name, status,
                   input, output, error, started_at, finished_at, sequence
            FROM unified_steps
            WHERE execution_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Update execution status and optionally set output and finished_at.
    pub async fn finish_execution(
        &self,
        execution_id: &str,
        status: StepStatus,
        output: &Value,
        step_count: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE unified_executions
            SET status = $2, output = $3, finished_at = $4, step_count = $5
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .bind(status.to_string())
        .bind(output)
        .bind(Utc::now())
        .bind(step_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Row types for sqlx query_as
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct ExecutionRow {
    execution_id: String,
    runtime: String,
    workflow_name: String,
    status: String,
    trigger: String,
    input: Value,
    output: Value,
    started_at: chrono::DateTime<Utc>,
    finished_at: Option<chrono::DateTime<Utc>>,
    step_count: i32,
}

#[cfg(feature = "postgres")]
impl From<ExecutionRow> for UnifiedExecution {
    fn from(row: ExecutionRow) -> Self {
        Self {
            execution_id: row.execution_id,
            runtime: row.runtime,
            workflow_name: row.workflow_name,
            status: parse_status(&row.status),
            trigger: row.trigger,
            input: row.input,
            output: row.output,
            started_at: row.started_at,
            finished_at: row.finished_at,
            step_count: row.step_count,
        }
    }
}

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct StepRow {
    step_id: String,
    execution_id: String,
    step_type: String,
    runtime: String,
    name: String,
    status: String,
    input: Value,
    output: Value,
    error: Option<String>,
    started_at: chrono::DateTime<Utc>,
    finished_at: Option<chrono::DateTime<Utc>>,
    sequence: i32,
}

#[cfg(feature = "postgres")]
impl From<StepRow> for UnifiedStep {
    fn from(row: StepRow) -> Self {
        Self {
            step_id: row.step_id,
            execution_id: row.execution_id,
            step_type: row.step_type,
            runtime: row.runtime,
            name: row.name,
            status: parse_status(&row.status),
            input: row.input,
            output: row.output,
            error: row.error,
            started_at: row.started_at,
            finished_at: row.finished_at,
            sequence: row.sequence,
        }
    }
}

/// Parse a status string into a StepStatus enum.
fn parse_status(s: &str) -> StepStatus {
    match s {
        "pending" => StepStatus::Pending,
        "running" => StepStatus::Running,
        "completed" => StepStatus::Completed,
        "failed" => StepStatus::Failed,
        "skipped" => StepStatus::Skipped,
        _ => StepStatus::Pending,
    }
}
