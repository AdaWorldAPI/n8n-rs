//! Project entity - matches n8n's Project.
//!
//! Reference: packages/@n8n/db/src/entities/project.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::generate_nano_id;

/// Project entity - team or personal projects.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    /// Primary key - nano ID.
    pub id: String,

    /// Project name.
    pub name: String,

    /// Project type: 'personal' or 'team'.
    #[sqlx(rename = "type")]
    pub project_type: String,

    /// Project icon.
    #[sqlx(json)]
    #[sqlx(default)]
    pub icon: Option<ProjectIcon>,

    /// Project description.
    #[sqlx(default)]
    pub description: Option<String>,

    /// Creator user ID.
    #[sqlx(default)]
    pub creator_id: Option<Uuid>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    /// Create a new personal project.
    pub fn personal(name: impl Into<String>, creator_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            name: name.into(),
            project_type: "personal".to_string(),
            icon: None,
            description: None,
            creator_id: Some(creator_id),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new team project.
    pub fn team(name: impl Into<String>, creator_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            name: name.into(),
            project_type: "team".to_string(),
            icon: None,
            description: None,
            creator_id: Some(creator_id),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_personal(&self) -> bool {
        self.project_type == "personal"
    }

    pub fn is_team(&self) -> bool {
        self.project_type == "team"
    }
}

/// Project icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProjectIcon {
    Emoji { value: String },
    Icon { value: String },
}

/// ProjectRelation - project membership.
///
/// Reference: packages/@n8n/db/src/entities/project-relation.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProjectRelation {
    pub project_id: String,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project relation roles.
pub mod project_roles {
    pub const ADMIN: &str = "project:admin";
    pub const EDITOR: &str = "project:editor";
    pub const VIEWER: &str = "project:viewer";
}
