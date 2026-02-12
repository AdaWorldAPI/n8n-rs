//! User repository - CRUD operations for users.

use sqlx::PgPool;
use uuid::Uuid;

use crate::entities::{Role, User};
use crate::error::DbError;

/// Repository for user operations.
#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a user by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DbError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, email, first_name, last_name, password, personalization_answers,
                   settings, disabled, mfa_enabled, mfa_secret, mfa_recovery_codes,
                   last_active_at, role_id, created_at, updated_at
            FROM "user"
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get a user by email.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, DbError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, email, first_name, last_name, password, personalization_answers,
                   settings, disabled, mfa_enabled, mfa_secret, mfa_recovery_codes,
                   last_active_at, role_id, created_at, updated_at
            FROM "user"
            WHERE LOWER(email) = LOWER($1)
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// List all users.
    pub async fn find_all(&self) -> Result<Vec<User>, DbError> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, email, first_name, last_name, password, personalization_answers,
                   settings, disabled, mfa_enabled, mfa_secret, mfa_recovery_codes,
                   last_active_at, role_id, created_at, updated_at
            FROM "user"
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Create a new user.
    pub async fn create(&self, user: &User) -> Result<User, DbError> {
        let created = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO "user" (id, email, first_name, last_name, password, role_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, email, first_name, last_name, password, personalization_answers,
                      settings, disabled, mfa_enabled, mfa_secret, mfa_recovery_codes,
                      last_active_at, role_id, created_at, updated_at
            "#,
        )
        .bind(user.id)
        .bind(&user.email)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.password)
        .bind(user.role_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Delete a user.
    pub async fn delete(&self, id: Uuid) -> Result<bool, DbError> {
        let result = sqlx::query(r#"DELETE FROM "user" WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get role by name.
    pub async fn get_role(&self, name: &str) -> Result<Option<Role>, DbError> {
        let role = sqlx::query_as::<_, Role>(
            "SELECT id, name, scope, created_at, updated_at FROM role WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(role)
    }
}
