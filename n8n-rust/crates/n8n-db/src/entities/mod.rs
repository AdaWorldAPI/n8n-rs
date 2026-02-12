//! Database entities - faithful translation of n8n TypeORM entities.
//!
//! These structs map directly to the PostgreSQL tables and maintain
//! compatibility with the original n8n database schema.

pub mod credentials;
pub mod execution;
pub mod project;
pub mod settings;
pub mod tag;
pub mod user;
pub mod variables;
pub mod webhook;
pub mod workflow;

pub use credentials::*;
pub use execution::*;
pub use project::*;
pub use settings::*;
pub use tag::*;
pub use user::*;
pub use variables::*;
pub use webhook::*;
pub use workflow::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Generate a nano ID (21 characters) like n8n does.
pub fn generate_nano_id() -> String {
    nanoid::nanoid!(21)
}

/// Generate a version ID (36 characters) like n8n does.
pub fn generate_version_id() -> String {
    nanoid::nanoid!(36)
}

/// Common timestamp fields used by most entities.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Timestamps {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Timestamps {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
        }
    }
}
