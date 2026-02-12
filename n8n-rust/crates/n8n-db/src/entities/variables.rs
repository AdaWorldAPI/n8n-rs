//! Variables entity - matches n8n's Variables.
//!
//! Reference: packages/@n8n/db/src/entities/variables.ts

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::generate_nano_id;

/// Variables - global and project-scoped variables.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Variable {
    /// Primary key - nano ID.
    pub id: String,

    /// Variable key/name.
    pub key: String,

    /// Variable type (default: 'string').
    #[sqlx(rename = "type")]
    pub variable_type: String,

    /// Variable value.
    pub value: String,

    /// Project ID (NULL = global variable).
    #[sqlx(default)]
    pub project_id: Option<String>,
}

impl Variable {
    /// Create a new global variable.
    pub fn global(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            id: generate_nano_id(),
            key: key.into(),
            variable_type: "string".to_string(),
            value: value.into(),
            project_id: None,
        }
    }

    /// Create a new project-scoped variable.
    pub fn project(key: impl Into<String>, value: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            id: generate_nano_id(),
            key: key.into(),
            variable_type: "string".to_string(),
            value: value.into(),
            project_id: Some(project_id.into()),
        }
    }

    pub fn is_global(&self) -> bool {
        self.project_id.is_none()
    }
}

/// Insert parameters for creating a variable.
#[derive(Debug, Clone)]
pub struct InsertVariable {
    pub id: String,
    pub key: String,
    pub variable_type: String,
    pub value: String,
    pub project_id: Option<String>,
}

impl From<&Variable> for InsertVariable {
    fn from(v: &Variable) -> Self {
        Self {
            id: v.id.clone(),
            key: v.key.clone(),
            variable_type: v.variable_type.clone(),
            value: v.value.clone(),
            project_id: v.project_id.clone(),
        }
    }
}
