//! Tag entity - matches n8n's TagEntity.
//!
//! Reference: packages/@n8n/db/src/entities/tag-entity.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::generate_nano_id;

/// TagEntity - workflow tags.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TagEntity {
    /// Primary key - nano ID.
    pub id: String,

    /// Tag name (unique, 1-24 characters).
    pub name: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TagEntity {
    /// Create a new tag.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            name: name.into(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Insert parameters for creating a tag.
#[derive(Debug, Clone)]
pub struct InsertTag {
    pub id: String,
    pub name: String,
}

impl From<&TagEntity> for InsertTag {
    fn from(t: &TagEntity) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
        }
    }
}
