//! Credentials entity - matches n8n's CredentialsEntity.
//!
//! Reference: packages/@n8n/db/src/entities/credentials-entity.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::generate_nano_id;

/// CredentialsEntity - encrypted credential storage.
///
/// Note: The `data` field contains encrypted credential data.
/// Encryption/decryption is handled by the credentials service.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CredentialsEntity {
    /// Primary key - nano ID.
    pub id: String,

    /// Credential name (3-128 characters).
    pub name: String,

    /// Credential type (e.g., 'slack', 'github', 'httpBasicAuth').
    #[sqlx(rename = "type")]
    pub credential_type: String,

    /// Encrypted credential data.
    pub data: String,

    /// Whether this is an n8n-managed credential.
    pub is_managed: bool,

    /// Whether this credential is globally available.
    pub is_global: bool,

    /// Whether this credential can be dynamically resolved.
    pub is_resolvable: bool,

    /// Allow fallback to static data if resolution fails.
    pub resolvable_allow_fallback: bool,

    /// ID of dynamic credential resolver.
    #[sqlx(default)]
    pub resolver_id: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CredentialsEntity {
    /// Create a new credential.
    pub fn new(name: impl Into<String>, credential_type: impl Into<String>, data: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            name: name.into(),
            credential_type: credential_type.into(),
            data: data.into(),
            is_managed: false,
            is_global: false,
            is_resolvable: false,
            resolvable_allow_fallback: false,
            resolver_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// SharedCredentials - credential access control.
///
/// Reference: packages/@n8n/db/src/entities/shared-credentials.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SharedCredentials {
    pub credentials_id: String,
    pub project_id: String,
    pub role: CredentialSharingRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Credential sharing roles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum CredentialSharingRole {
    #[sqlx(rename = "credential:owner")]
    #[serde(rename = "credential:owner")]
    Owner,
    #[sqlx(rename = "credential:user")]
    #[serde(rename = "credential:user")]
    User,
}

impl Default for CredentialSharingRole {
    fn default() -> Self {
        Self::User
    }
}

/// Insert parameters for creating credentials.
#[derive(Debug, Clone)]
pub struct InsertCredentials {
    pub id: String,
    pub name: String,
    pub credential_type: String,
    pub data: String,
}

impl From<&CredentialsEntity> for InsertCredentials {
    fn from(c: &CredentialsEntity) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            credential_type: c.credential_type.clone(),
            data: c.data.clone(),
        }
    }
}

/// Update parameters for credentials.
#[derive(Debug, Clone, Default)]
pub struct UpdateCredentials {
    pub name: Option<String>,
    pub data: Option<String>,
    pub is_managed: Option<bool>,
    pub is_global: Option<bool>,
}

/// Query filters for credentials.
#[derive(Debug, Clone, Default)]
pub struct CredentialFilters {
    pub credential_type: Option<String>,
    pub name_like: Option<String>,
    pub is_global: Option<bool>,
    pub project_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
