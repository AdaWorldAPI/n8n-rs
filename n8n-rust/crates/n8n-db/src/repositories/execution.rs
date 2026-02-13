//! Execution repository - CRUD operations for executions.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::entities::{
    ExecutionData, ExecutionEntity, ExecutionFilters, ExecutionMetadata,
    ExecutionWithData, InsertExecution, UpdateExecution,
};
use crate::error::DbError;
use n8n_workflow::ExecutionStatus;

/// Repository for execution operations.
#[derive(Clone)]
pub struct ExecutionRepository {
    pool: PgPool,
}

impl ExecutionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get an execution by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<ExecutionEntity>, DbError> {
        let execution = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            SELECT id, finished, mode, status, created_at, started_at, stopped_at,
                   deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            FROM execution_entity
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(execution)
    }

    /// Get an execution with its data.
    pub async fn find_by_id_with_data(&self, id: &str) -> Result<Option<ExecutionWithData>, DbError> {
        let execution = self.find_by_id(id).await?;

        if let Some(execution) = execution {
            let data = self.get_data(&execution.id).await?;
            let metadata = self.get_metadata(&execution.id).await?;

            Ok(Some(ExecutionWithData {
                execution,
                data,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    /// List executions with filters.
    pub async fn find_all(&self, filters: &ExecutionFilters) -> Result<Vec<ExecutionEntity>, DbError> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 1;

        if !filters.include_deleted {
            conditions.push("deleted_at IS NULL".to_string());
        }

        if filters.workflow_id.is_some() {
            conditions.push(format!("workflow_id = ${}", param_idx));
            param_idx += 1;
        }

        if filters.finished.is_some() {
            conditions.push(format!("finished = ${}", param_idx));
            param_idx += 1;
        }

        let query = format!(
            r#"
            SELECT id, finished, mode, status, created_at, started_at, stopped_at,
                   deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            FROM execution_entity
            WHERE {}
            ORDER BY created_at DESC
            LIMIT ${}
            OFFSET ${}
            "#,
            conditions.join(" AND "),
            param_idx,
            param_idx + 1
        );

        let mut query = sqlx::query_as::<_, ExecutionEntity>(&query);

        if let Some(ref workflow_id) = filters.workflow_id {
            query = query.bind(workflow_id);
        }
        if let Some(finished) = filters.finished {
            query = query.bind(finished);
        }

        query = query
            .bind(filters.limit.unwrap_or(100))
            .bind(filters.offset.unwrap_or(0));

        let executions = query.fetch_all(&self.pool).await?;
        Ok(executions)
    }

    /// List executions for a workflow.
    pub async fn find_by_workflow(
        &self,
        workflow_id: &str,
        limit: i64,
    ) -> Result<Vec<ExecutionEntity>, DbError> {
        let executions = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            SELECT id, finished, mode, status, created_at, started_at, stopped_at,
                   deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            FROM execution_entity
            WHERE workflow_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(workflow_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(executions)
    }

    /// List running executions.
    pub async fn find_running(&self) -> Result<Vec<ExecutionEntity>, DbError> {
        let executions = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            SELECT id, finished, mode, status, created_at, started_at, stopped_at,
                   deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            FROM execution_entity
            WHERE status IN ('new', 'running') AND deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(executions)
    }

    /// List waiting executions (for resume).
    pub async fn find_waiting(&self) -> Result<Vec<ExecutionEntity>, DbError> {
        let executions = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            SELECT id, finished, mode, status, created_at, started_at, stopped_at,
                   deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            FROM execution_entity
            WHERE status = 'waiting' AND deleted_at IS NULL
                  AND (wait_till IS NULL OR wait_till <= NOW())
            ORDER BY wait_till ASC NULLS FIRST
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(executions)
    }

    /// Create a new execution.
    pub async fn create(&self, execution: &InsertExecution) -> Result<ExecutionEntity, DbError> {
        let created = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            INSERT INTO execution_entity (id, workflow_id, mode, status)
            VALUES ($1, $2, $3, $4)
            RETURNING id, finished, mode, status, created_at, started_at, stopped_at,
                      deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            "#,
        )
        .bind(&execution.id)
        .bind(&execution.workflow_id)
        .bind(&execution.mode)
        .bind(&execution.status)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Update an execution.
    pub async fn update(&self, id: &str, update: &UpdateExecution) -> Result<ExecutionEntity, DbError> {
        let mut set_clauses = Vec::new();

        if update.finished.is_some() {
            set_clauses.push(format!("finished = ${}", set_clauses.len() + 2));
        }
        if update.status.is_some() {
            set_clauses.push(format!("status = ${}", set_clauses.len() + 2));
        }
        if update.started_at.is_some() {
            set_clauses.push(format!("started_at = ${}", set_clauses.len() + 2));
        }
        if update.stopped_at.is_some() {
            set_clauses.push(format!("stopped_at = ${}", set_clauses.len() + 2));
        }

        if set_clauses.is_empty() {
            return self.find_by_id(id).await?.ok_or(DbError::NotFound);
        }

        // Use a simpler approach with explicit query building
        let updated = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            UPDATE execution_entity
            SET finished = COALESCE($2, finished),
                status = COALESCE($3, status),
                started_at = COALESCE($4, started_at),
                stopped_at = COALESCE($5, stopped_at)
            WHERE id = $1
            RETURNING id, finished, mode, status, created_at, started_at, stopped_at,
                      deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            "#,
        )
        .bind(id)
        .bind(update.finished)
        .bind(&update.status)
        .bind(update.started_at)
        .bind(update.stopped_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Mark execution as started.
    pub async fn start(&self, id: &str) -> Result<ExecutionEntity, DbError> {
        let updated = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            UPDATE execution_entity
            SET status = 'running', started_at = NOW()
            WHERE id = $1
            RETURNING id, finished, mode, status, created_at, started_at, stopped_at,
                      deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Mark execution as completed.
    pub async fn complete(&self, id: &str, status: ExecutionStatus) -> Result<ExecutionEntity, DbError> {
        let finished = status.is_finished();
        let status_str = status.as_str();

        let updated = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            UPDATE execution_entity
            SET status = $2, finished = $3, stopped_at = NOW()
            WHERE id = $1
            RETURNING id, finished, mode, status, created_at, started_at, stopped_at,
                      deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            "#,
        )
        .bind(id)
        .bind(status_str)
        .bind(finished)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Mark execution as waiting.
    pub async fn wait_until(&self, id: &str, wait_till: Option<DateTime<Utc>>) -> Result<ExecutionEntity, DbError> {
        let updated = sqlx::query_as::<_, ExecutionEntity>(
            r#"
            UPDATE execution_entity
            SET status = 'waiting', wait_till = $2
            WHERE id = $1
            RETURNING id, finished, mode, status, created_at, started_at, stopped_at,
                      deleted_at, workflow_id, retry_of, retry_success_id, wait_till, stored_at
            "#,
        )
        .bind(id)
        .bind(wait_till)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Soft delete an execution.
    pub async fn soft_delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            "UPDATE execution_entity SET deleted_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Permanently delete an execution.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM execution_entity WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete old executions.
    pub async fn delete_older_than(&self, before: DateTime<Utc>) -> Result<u64, DbError> {
        let result = sqlx::query(
            "DELETE FROM execution_entity WHERE created_at < $1",
        )
        .bind(before)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // =========================================================================
    // Execution Data
    // =========================================================================

    /// Get execution data.
    pub async fn get_data(&self, execution_id: &str) -> Result<Option<ExecutionData>, DbError> {
        let data = sqlx::query_as::<_, ExecutionData>(
            r#"
            SELECT execution_id, data, workflow_data, workflow_version_id
            FROM execution_data
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(data)
    }

    /// Save execution data.
    pub async fn save_data(&self, data: &ExecutionData) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO execution_data (execution_id, data, workflow_data, workflow_version_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (execution_id) DO UPDATE SET
                data = EXCLUDED.data,
                workflow_data = EXCLUDED.workflow_data,
                workflow_version_id = EXCLUDED.workflow_version_id
            "#,
        )
        .bind(&data.execution_id)
        .bind(&data.data)
        .bind(&data.workflow_data)
        .bind(&data.workflow_version_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // Execution Metadata
    // =========================================================================

    /// Get metadata for an execution.
    pub async fn get_metadata(&self, execution_id: &str) -> Result<Vec<ExecutionMetadata>, DbError> {
        let metadata = sqlx::query_as::<_, ExecutionMetadata>(
            "SELECT id, execution_id, key, value FROM execution_metadata WHERE execution_id = $1",
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(metadata)
    }

    /// Add metadata to an execution.
    pub async fn add_metadata(&self, execution_id: &str, key: &str, value: &str) -> Result<ExecutionMetadata, DbError> {
        let metadata = sqlx::query_as::<_, ExecutionMetadata>(
            r#"
            INSERT INTO execution_metadata (execution_id, key, value)
            VALUES ($1, $2, $3)
            RETURNING id, execution_id, key, value
            "#,
        )
        .bind(execution_id)
        .bind(key)
        .bind(value)
        .fetch_one(&self.pool)
        .await?;

        Ok(metadata)
    }

    /// Count executions by status.
    pub async fn count_by_status(&self) -> Result<Vec<(String, i64)>, DbError> {
        let counts = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT status, COUNT(*) as count
            FROM execution_entity
            WHERE deleted_at IS NULL
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(counts)
    }
}
