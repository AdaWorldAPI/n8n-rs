//! Tag repository - CRUD operations for tags.

use sqlx::PgPool;

use crate::entities::{InsertTag, TagEntity};
use crate::error::DbError;

/// Repository for tag operations.
#[derive(Clone)]
pub struct TagRepository {
    pool: PgPool,
}

impl TagRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a tag by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<TagEntity>, DbError> {
        let tag = sqlx::query_as::<_, TagEntity>(
            "SELECT id, name, created_at, updated_at FROM tag_entity WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tag)
    }

    /// Get a tag by name.
    pub async fn find_by_name(&self, name: &str) -> Result<Option<TagEntity>, DbError> {
        let tag = sqlx::query_as::<_, TagEntity>(
            "SELECT id, name, created_at, updated_at FROM tag_entity WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tag)
    }

    /// List all tags.
    pub async fn find_all(&self) -> Result<Vec<TagEntity>, DbError> {
        let tags = sqlx::query_as::<_, TagEntity>(
            "SELECT id, name, created_at, updated_at FROM tag_entity ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tags)
    }

    /// Create a new tag.
    pub async fn create(&self, tag: &InsertTag) -> Result<TagEntity, DbError> {
        let created = sqlx::query_as::<_, TagEntity>(
            r#"
            INSERT INTO tag_entity (id, name)
            VALUES ($1, $2)
            RETURNING id, name, created_at, updated_at
            "#,
        )
        .bind(&tag.id)
        .bind(&tag.name)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Update a tag.
    pub async fn update(&self, id: &str, name: &str) -> Result<TagEntity, DbError> {
        let updated = sqlx::query_as::<_, TagEntity>(
            r#"
            UPDATE tag_entity
            SET name = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Delete a tag.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM tag_entity WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get or create a tag by name.
    pub async fn get_or_create(&self, name: &str) -> Result<TagEntity, DbError> {
        if let Some(tag) = self.find_by_name(name).await? {
            return Ok(tag);
        }

        let tag = TagEntity::new(name);
        let insert = InsertTag::from(&tag);
        self.create(&insert).await
    }
}
