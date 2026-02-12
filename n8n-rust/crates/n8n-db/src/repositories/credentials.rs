//! Credentials repository - CRUD operations for credentials.

use sqlx::PgPool;

use crate::entities::{
    CredentialFilters, CredentialSharingRole, CredentialsEntity, InsertCredentials,
    SharedCredentials, UpdateCredentials,
};
use crate::error::DbError;

/// Repository for credentials operations.
#[derive(Clone)]
pub struct CredentialsRepository {
    pool: PgPool,
}

impl CredentialsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get credentials by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<CredentialsEntity>, DbError> {
        let creds = sqlx::query_as::<_, CredentialsEntity>(
            r#"
            SELECT id, name, type as credential_type, data, is_managed, is_global,
                   is_resolvable, resolvable_allow_fallback, resolver_id, created_at, updated_at
            FROM credentials_entity
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(creds)
    }

    /// List credentials with filters.
    pub async fn find_all(&self, filters: &CredentialFilters) -> Result<Vec<CredentialsEntity>, DbError> {
        let mut conditions = vec!["1=1".to_string()];

        if filters.credential_type.is_some() {
            conditions.push("type = $1".to_string());
        }
        if filters.is_global.is_some() {
            conditions.push("is_global = $2".to_string());
        }

        let creds = sqlx::query_as::<_, CredentialsEntity>(
            r#"
            SELECT id, name, type as credential_type, data, is_managed, is_global,
                   is_resolvable, resolvable_allow_fallback, resolver_id, created_at, updated_at
            FROM credentials_entity
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(creds)
    }

    /// List credentials by type.
    pub async fn find_by_type(&self, credential_type: &str) -> Result<Vec<CredentialsEntity>, DbError> {
        let creds = sqlx::query_as::<_, CredentialsEntity>(
            r#"
            SELECT id, name, type as credential_type, data, is_managed, is_global,
                   is_resolvable, resolvable_allow_fallback, resolver_id, created_at, updated_at
            FROM credentials_entity
            WHERE type = $1
            ORDER BY name ASC
            "#,
        )
        .bind(credential_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(creds)
    }

    /// Create new credentials.
    pub async fn create(&self, creds: &InsertCredentials) -> Result<CredentialsEntity, DbError> {
        let created = sqlx::query_as::<_, CredentialsEntity>(
            r#"
            INSERT INTO credentials_entity (id, name, type, data)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, type as credential_type, data, is_managed, is_global,
                      is_resolvable, resolvable_allow_fallback, resolver_id, created_at, updated_at
            "#,
        )
        .bind(&creds.id)
        .bind(&creds.name)
        .bind(&creds.credential_type)
        .bind(&creds.data)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Update credentials.
    pub async fn update(&self, id: &str, update: &UpdateCredentials) -> Result<CredentialsEntity, DbError> {
        let updated = sqlx::query_as::<_, CredentialsEntity>(
            r#"
            UPDATE credentials_entity
            SET name = COALESCE($2, name),
                data = COALESCE($3, data),
                is_managed = COALESCE($4, is_managed),
                is_global = COALESCE($5, is_global),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, type as credential_type, data, is_managed, is_global,
                      is_resolvable, resolvable_allow_fallback, resolver_id, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&update.name)
        .bind(&update.data)
        .bind(update.is_managed)
        .bind(update.is_global)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Delete credentials.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM credentials_entity WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // Sharing
    // =========================================================================

    /// Share credentials with a project.
    pub async fn share(
        &self,
        credentials_id: &str,
        project_id: &str,
        role: CredentialSharingRole,
    ) -> Result<SharedCredentials, DbError> {
        let role_str = match role {
            CredentialSharingRole::Owner => "credential:owner",
            CredentialSharingRole::User => "credential:user",
        };

        let shared = sqlx::query_as::<_, SharedCredentials>(
            r#"
            INSERT INTO shared_credentials (credentials_id, project_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (credentials_id, project_id) DO UPDATE SET role = $3
            RETURNING credentials_id, project_id, role, created_at, updated_at
            "#,
        )
        .bind(credentials_id)
        .bind(project_id)
        .bind(role_str)
        .fetch_one(&self.pool)
        .await?;

        Ok(shared)
    }

    /// Unshare credentials from a project.
    pub async fn unshare(&self, credentials_id: &str, project_id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            "DELETE FROM shared_credentials WHERE credentials_id = $1 AND project_id = $2",
        )
        .bind(credentials_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sharing info for credentials.
    pub async fn get_sharing(&self, credentials_id: &str) -> Result<Vec<SharedCredentials>, DbError> {
        let shared = sqlx::query_as::<_, SharedCredentials>(
            r#"
            SELECT credentials_id, project_id, role, created_at, updated_at
            FROM shared_credentials
            WHERE credentials_id = $1
            "#,
        )
        .bind(credentials_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(shared)
    }
}
