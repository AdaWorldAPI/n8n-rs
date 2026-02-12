//! User entity - matches n8n's User.
//!
//! Reference: packages/@n8n/db/src/entities/user.ts

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User entity.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    /// Primary key - UUID.
    pub id: Uuid,

    /// Email address (unique, lowercased).
    #[sqlx(default)]
    pub email: Option<String>,

    /// First name.
    #[sqlx(default)]
    pub first_name: Option<String>,

    /// Last name.
    #[sqlx(default)]
    pub last_name: Option<String>,

    /// Hashed password (NULL for external auth).
    #[sqlx(default)]
    #[serde(skip_serializing)]
    pub password: Option<String>,

    /// Personalization survey answers.
    #[sqlx(json)]
    #[sqlx(default)]
    pub personalization_answers: Option<serde_json::Value>,

    /// User settings.
    #[sqlx(json)]
    #[sqlx(default)]
    pub settings: Option<UserSettings>,

    /// Account disabled flag.
    pub disabled: bool,

    /// MFA enabled flag.
    pub mfa_enabled: bool,

    /// TOTP secret for MFA.
    #[sqlx(default)]
    #[serde(skip_serializing)]
    pub mfa_secret: Option<String>,

    /// MFA recovery codes.
    #[sqlx(default)]
    #[serde(skip_serializing)]
    pub mfa_recovery_codes: Vec<String>,

    /// Last activity date.
    #[sqlx(default)]
    pub last_active_at: Option<NaiveDate>,

    /// Role FK.
    #[sqlx(default)]
    pub role_id: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create a new user.
    pub fn new(email: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            email: Some(email.into().to_lowercase()),
            first_name: None,
            last_name: None,
            password: None,
            personalization_answers: None,
            settings: None,
            disabled: false,
            mfa_enabled: false,
            mfa_secret: None,
            mfa_recovery_codes: Vec::new(),
            last_active_at: None,
            role_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get full name.
    pub fn full_name(&self) -> String {
        match (&self.first_name, &self.last_name) {
            (Some(first), Some(last)) => format!("{} {}", first, last),
            (Some(first), None) => first.clone(),
            (None, Some(last)) => last.clone(),
            (None, None) => self.email.clone().unwrap_or_default(),
        }
    }

    /// Check if user is pending (no password and no external auth).
    pub fn is_pending(&self) -> bool {
        self.password.is_none()
    }
}

/// User settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_onboarded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_successful_workflow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_activated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_activated_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_rc: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Role entity.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: i32,
    pub name: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Global role names (as used in n8n).
pub mod role_names {
    pub const GLOBAL_OWNER: &str = "global:owner";
    pub const GLOBAL_ADMIN: &str = "global:admin";
    pub const GLOBAL_MEMBER: &str = "global:member";
}

/// AuthIdentity - external authentication identity.
///
/// Reference: packages/@n8n/db/src/entities/auth-identity.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuthIdentity {
    pub id: String,
    pub user_id: Uuid,
    pub provider_type: String,
    pub provider_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// ApiKey - user API keys.
///
/// Reference: packages/@n8n/db/src/entities/api-key.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: String,
    pub user_id: Uuid,
    pub label: String,
    #[serde(skip_serializing)]
    pub api_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
