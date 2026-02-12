//! Settings repository - CRUD operations for global settings.

use sqlx::PgPool;

use crate::entities::Setting;
use crate::error::DbError;

/// Repository for settings operations.
#[derive(Clone)]
pub struct SettingsRepository {
    pool: PgPool,
}

impl SettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a setting by key.
    pub async fn get(&self, key: &str) -> Result<Option<Setting>, DbError> {
        let setting = sqlx::query_as::<_, Setting>(
            "SELECT key, value, load_on_startup FROM settings WHERE key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(setting)
    }

    /// Get all settings.
    pub async fn get_all(&self) -> Result<Vec<Setting>, DbError> {
        let settings = sqlx::query_as::<_, Setting>(
            "SELECT key, value, load_on_startup FROM settings ORDER BY key",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Get startup settings.
    pub async fn get_startup(&self) -> Result<Vec<Setting>, DbError> {
        let settings = sqlx::query_as::<_, Setting>(
            "SELECT key, value, load_on_startup FROM settings WHERE load_on_startup = true",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Set a setting (upsert).
    pub async fn set(&self, setting: &Setting) -> Result<Setting, DbError> {
        let saved = sqlx::query_as::<_, Setting>(
            r#"
            INSERT INTO settings (key, value, load_on_startup)
            VALUES ($1, $2, $3)
            ON CONFLICT (key) DO UPDATE SET value = $2, load_on_startup = $3
            RETURNING key, value, load_on_startup
            "#,
        )
        .bind(&setting.key)
        .bind(&setting.value)
        .bind(setting.load_on_startup)
        .fetch_one(&self.pool)
        .await?;

        Ok(saved)
    }

    /// Delete a setting.
    pub async fn delete(&self, key: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM settings WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
