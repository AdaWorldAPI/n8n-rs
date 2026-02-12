//! Webhook entity - matches n8n's WebhookEntity.
//!
//! Reference: packages/@n8n/db/src/entities/webhook-entity.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::generate_nano_id;

/// WebhookEntity - webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookEntity {
    /// Primary key - nano ID.
    pub id: String,

    /// Workflow ID FK.
    pub workflow_id: String,

    /// Node name that defines this webhook.
    pub node: String,

    /// HTTP method (GET, POST, etc.).
    pub method: String,

    /// Webhook path.
    pub path: String,

    /// Optional webhook ID for uniqueness.
    #[sqlx(default)]
    pub webhook_id: Option<String>,

    /// Path length for routing optimization.
    #[sqlx(default)]
    pub path_length: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WebhookEntity {
    /// Create a new webhook.
    pub fn new(
        workflow_id: impl Into<String>,
        node: impl Into<String>,
        method: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let path = path.into();
        let path_length = path.matches('/').count() as i32;

        Self {
            id: generate_nano_id(),
            workflow_id: workflow_id.into(),
            node: node.into(),
            method: method.into(),
            path,
            webhook_id: None,
            path_length: Some(path_length),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Insert parameters for creating a webhook.
#[derive(Debug, Clone)]
pub struct InsertWebhook {
    pub id: String,
    pub workflow_id: String,
    pub node: String,
    pub method: String,
    pub path: String,
    pub webhook_id: Option<String>,
    pub path_length: Option<i32>,
}

impl From<&WebhookEntity> for InsertWebhook {
    fn from(w: &WebhookEntity) -> Self {
        Self {
            id: w.id.clone(),
            workflow_id: w.workflow_id.clone(),
            node: w.node.clone(),
            method: w.method.clone(),
            path: w.path.clone(),
            webhook_id: w.webhook_id.clone(),
            path_length: w.path_length,
        }
    }
}
