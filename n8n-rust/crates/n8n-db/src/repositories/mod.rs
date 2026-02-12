//! Repository implementations for database operations.
//!
//! These repositories provide async CRUD operations with compile-time
//! checked SQL queries via sqlx.

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

use sqlx::PgPool;
use std::sync::Arc;

/// Database context containing all repositories.
#[derive(Clone)]
pub struct DbContext {
    pub pool: PgPool,
    pub workflows: WorkflowRepository,
    pub executions: ExecutionRepository,
    pub credentials: CredentialsRepository,
    pub tags: TagRepository,
    pub users: UserRepository,
    pub projects: ProjectRepository,
    pub settings: SettingsRepository,
    pub variables: VariablesRepository,
    pub webhooks: WebhookRepository,
}

impl DbContext {
    /// Create a new database context from a connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            workflows: WorkflowRepository::new(pool.clone()),
            executions: ExecutionRepository::new(pool.clone()),
            credentials: CredentialsRepository::new(pool.clone()),
            tags: TagRepository::new(pool.clone()),
            users: UserRepository::new(pool.clone()),
            projects: ProjectRepository::new(pool.clone()),
            settings: SettingsRepository::new(pool.clone()),
            variables: VariablesRepository::new(pool.clone()),
            webhooks: WebhookRepository::new(pool.clone()),
            pool,
        }
    }

    /// Run database migrations.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.pool).await
    }
}
