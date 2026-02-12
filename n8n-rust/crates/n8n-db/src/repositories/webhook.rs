//! Webhook repository - CRUD operations for webhooks.

use sqlx::PgPool;

use crate::entities::{InsertWebhook, WebhookEntity};
use crate::error::DbError;

/// Repository for webhook operations.
#[derive(Clone)]
pub struct WebhookRepository {
    pool: PgPool,
}

impl WebhookRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a webhook by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<WebhookEntity>, DbError> {
        let webhook = sqlx::query_as::<_, WebhookEntity>(
            r#"
            SELECT id, workflow_id, node, method, path, webhook_id, path_length, created_at, updated_at
            FROM webhook_entity WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(webhook)
    }

    /// Find webhooks by path and method.
    pub async fn find_by_path(&self, method: &str, path: &str) -> Result<Vec<WebhookEntity>, DbError> {
        let webhooks = sqlx::query_as::<_, WebhookEntity>(
            r#"
            SELECT id, workflow_id, node, method, path, webhook_id, path_length, created_at, updated_at
            FROM webhook_entity
            WHERE method = $1 AND path = $2
            "#,
        )
        .bind(method)
        .bind(path)
        .fetch_all(&self.pool)
        .await?;

        Ok(webhooks)
    }

    /// Find webhooks for a workflow.
    pub async fn find_by_workflow(&self, workflow_id: &str) -> Result<Vec<WebhookEntity>, DbError> {
        let webhooks = sqlx::query_as::<_, WebhookEntity>(
            r#"
            SELECT id, workflow_id, node, method, path, webhook_id, path_length, created_at, updated_at
            FROM webhook_entity WHERE workflow_id = $1
            "#,
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(webhooks)
    }

    /// Create a webhook.
    pub async fn create(&self, webhook: &InsertWebhook) -> Result<WebhookEntity, DbError> {
        let created = sqlx::query_as::<_, WebhookEntity>(
            r#"
            INSERT INTO webhook_entity (id, workflow_id, node, method, path, webhook_id, path_length)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, workflow_id, node, method, path, webhook_id, path_length, created_at, updated_at
            "#,
        )
        .bind(&webhook.id)
        .bind(&webhook.workflow_id)
        .bind(&webhook.node)
        .bind(&webhook.method)
        .bind(&webhook.path)
        .bind(&webhook.webhook_id)
        .bind(webhook.path_length)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Delete a webhook.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM webhook_entity WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete all webhooks for a workflow.
    pub async fn delete_by_workflow(&self, workflow_id: &str) -> Result<u64, DbError> {
        let result = sqlx::query("DELETE FROM webhook_entity WHERE workflow_id = $1")
            .bind(workflow_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
