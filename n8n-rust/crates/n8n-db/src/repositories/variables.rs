//! Variables repository - CRUD operations for variables.

use sqlx::PgPool;

use crate::entities::{InsertVariable, Variable};
use crate::error::DbError;

/// Repository for variable operations.
#[derive(Clone)]
pub struct VariablesRepository {
    pool: PgPool,
}

impl VariablesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a variable by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<Variable>, DbError> {
        let var = sqlx::query_as::<_, Variable>(
            "SELECT id, key, type as variable_type, value, project_id FROM variables WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(var)
    }

    /// Get a variable by key.
    pub async fn find_by_key(&self, key: &str, project_id: Option<&str>) -> Result<Option<Variable>, DbError> {
        let var = if let Some(pid) = project_id {
            sqlx::query_as::<_, Variable>(
                "SELECT id, key, type as variable_type, value, project_id FROM variables WHERE key = $1 AND project_id = $2",
            )
            .bind(key)
            .bind(pid)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Variable>(
                "SELECT id, key, type as variable_type, value, project_id FROM variables WHERE key = $1 AND project_id IS NULL",
            )
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
        };

        Ok(var)
    }

    /// List all global variables.
    pub async fn find_global(&self) -> Result<Vec<Variable>, DbError> {
        let vars = sqlx::query_as::<_, Variable>(
            "SELECT id, key, type as variable_type, value, project_id FROM variables WHERE project_id IS NULL ORDER BY key",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(vars)
    }

    /// List variables for a project.
    pub async fn find_by_project(&self, project_id: &str) -> Result<Vec<Variable>, DbError> {
        let vars = sqlx::query_as::<_, Variable>(
            "SELECT id, key, type as variable_type, value, project_id FROM variables WHERE project_id = $1 ORDER BY key",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(vars)
    }

    /// Create a new variable.
    pub async fn create(&self, var: &InsertVariable) -> Result<Variable, DbError> {
        let created = sqlx::query_as::<_, Variable>(
            r#"
            INSERT INTO variables (id, key, type, value, project_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, key, type as variable_type, value, project_id
            "#,
        )
        .bind(&var.id)
        .bind(&var.key)
        .bind(&var.variable_type)
        .bind(&var.value)
        .bind(&var.project_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Update a variable.
    pub async fn update(&self, id: &str, value: &str) -> Result<Variable, DbError> {
        let updated = sqlx::query_as::<_, Variable>(
            r#"
            UPDATE variables SET value = $2
            WHERE id = $1
            RETURNING id, key, type as variable_type, value, project_id
            "#,
        )
        .bind(id)
        .bind(value)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Delete a variable.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM variables WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
