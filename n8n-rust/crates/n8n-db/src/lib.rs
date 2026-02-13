//! # n8n-db
//!
//! PostgreSQL persistence layer for n8n-rust.
//!
//! This crate provides a faithful translation of n8n's TypeORM entities
//! to Rust structs and sqlx-based repositories for database operations.
//!
//! ## Features
//!
//! - **Entity definitions** matching n8n's TypeORM schema exactly
//! - **Repository pattern** with compile-time checked SQL queries
//! - **Transaction support** for atomic operations
//! - **Migration support** via sqlx migrations
//!
//! ## Usage
//!
//! ```rust,no_run
//! use n8n_db::{DbContext, connect};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to database
//!     let pool = connect("postgres://user:pass@localhost/n8n").await?;
//!     let db = DbContext::new(pool);
//!
//!     // Run migrations
//!     db.migrate().await?;
//!
//!     // Use repositories
//!     let workflows = db.workflows.find_all(false).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Schema Compatibility
//!
//! This crate maintains compatibility with the original n8n PostgreSQL schema.
//! All entity definitions match the TypeORM entities in `packages/@n8n/db/src/entities/`.

pub mod entities;
pub mod error;
pub mod repositories;

// Re-export entity types explicitly to avoid ambiguous glob re-exports
// (entities and repositories have submodules with the same names).
pub use entities::{
    generate_nano_id, generate_version_id, Timestamps,
    // Workflow entities
    WorkflowEntity, WorkflowMeta, WorkflowHistory, SharedWorkflow,
    WorkflowSharingRole, WorkflowTagMapping, InsertWorkflow, UpdateWorkflow,
    // Execution entities
    ExecutionEntity, ExecutionData, ExecutionMetadata, ExecutionFilters,
    ExecutionWithData, InsertExecution, UpdateExecution,
    // Credentials entities
    CredentialsEntity, SharedCredentials, CredentialSharingRole,
    InsertCredentials, UpdateCredentials, CredentialFilters,
    // User entities
    User, UserSettings, Role, AuthIdentity, ApiKey,
    // Project entities
    Project, ProjectIcon, ProjectRelation,
    // Tag entities
    TagEntity, InsertTag,
    // Webhook entities
    WebhookEntity, InsertWebhook,
    // Settings entities
    Setting,
    // Variables entities
    Variable, InsertVariable,
};

pub use error::*;

// Re-export repository types explicitly.
pub use repositories::{
    DbContext,
    WorkflowRepository, ExecutionRepository, CredentialsRepository,
    TagRepository, UserRepository, ProjectRepository, SettingsRepository,
    VariablesRepository, WebhookRepository,
};

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Connect to PostgreSQL database.
pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    Ok(pool)
}

/// Connect with custom pool options.
pub async fn connect_with_options(
    database_url: &str,
    max_connections: u32,
    min_connections: u32,
) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    Ok(pool)
}

/// Database configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// PostgreSQL connection URL.
    pub database_url: String,
    /// Maximum pool connections.
    pub max_connections: u32,
    /// Minimum pool connections.
    pub min_connections: u32,
    /// Connection acquire timeout in seconds.
    pub acquire_timeout_secs: u64,
    /// Idle connection timeout in seconds.
    pub idle_timeout_secs: u64,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://n8n:n8n@localhost:5432/n8n".to_string(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout_secs: 30,
            idle_timeout_secs: 600,
        }
    }
}

impl DbConfig {
    /// Create config from environment variables.
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .or_else(|_| std::env::var("N8N_DATABASE_URL"))
                .unwrap_or_else(|_| "postgres://n8n:n8n@localhost:5432/n8n".to_string()),
            max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            min_connections: std::env::var("DB_MIN_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            acquire_timeout_secs: std::env::var("DB_ACQUIRE_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            idle_timeout_secs: std::env::var("DB_IDLE_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
        }
    }

    /// Connect using this configuration.
    pub async fn connect(&self) -> Result<PgPool, DbError> {
        let pool = PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_timeout(Duration::from_secs(self.acquire_timeout_secs))
            .idle_timeout(Duration::from_secs(self.idle_timeout_secs))
            .connect(&self.database_url)
            .await?;

        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_config_default() {
        let config = DbConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 1);
    }
}
