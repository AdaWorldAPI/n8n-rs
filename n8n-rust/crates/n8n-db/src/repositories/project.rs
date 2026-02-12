//! Project repository - CRUD operations for projects.

use sqlx::PgPool;
use uuid::Uuid;

use crate::entities::{Project, ProjectRelation};
use crate::error::DbError;

/// Repository for project operations.
#[derive(Clone)]
pub struct ProjectRepository {
    pool: PgPool,
}

impl ProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a project by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<Project>, DbError> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT id, name, type as project_type, icon, description, creator_id, created_at, updated_at
            FROM project WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(project)
    }

    /// List projects for a user.
    pub async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<Project>, DbError> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.id, p.name, p.type as project_type, p.icon, p.description, p.creator_id, p.created_at, p.updated_at
            FROM project p
            INNER JOIN project_relation pr ON p.id = pr.project_id
            WHERE pr.user_id = $1
            ORDER BY p.name ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(projects)
    }

    /// Create a new project.
    pub async fn create(&self, project: &Project) -> Result<Project, DbError> {
        let created = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO project (id, name, type, icon, description, creator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, name, type as project_type, icon, description, creator_id, created_at, updated_at
            "#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.project_type)
        .bind(&project.icon)
        .bind(&project.description)
        .bind(project.creator_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Delete a project.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM project WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Add a user to a project.
    pub async fn add_member(&self, project_id: &str, user_id: Uuid, role: &str) -> Result<ProjectRelation, DbError> {
        let relation = sqlx::query_as::<_, ProjectRelation>(
            r#"
            INSERT INTO project_relation (project_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (project_id, user_id) DO UPDATE SET role = $3
            RETURNING project_id, user_id, role, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;

        Ok(relation)
    }

    /// Remove a user from a project.
    pub async fn remove_member(&self, project_id: &str, user_id: Uuid) -> Result<bool, DbError> {
        let result = sqlx::query(
            "DELETE FROM project_relation WHERE project_id = $1 AND user_id = $2",
        )
        .bind(project_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
