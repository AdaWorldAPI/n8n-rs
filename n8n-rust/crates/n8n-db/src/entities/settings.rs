//! Settings entity - matches n8n's Settings.
//!
//! Reference: packages/@n8n/db/src/entities/settings.ts

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Settings - global key-value store.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    /// Setting key (primary key).
    pub key: String,

    /// Setting value (serialized).
    pub value: String,

    /// Whether to load on application startup.
    pub load_on_startup: bool,
}

impl Setting {
    /// Create a new setting.
    pub fn new(key: impl Into<String>, value: impl Into<String>, load_on_startup: bool) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            load_on_startup,
        }
    }

    /// Create a startup setting (loaded at boot).
    pub fn startup(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, value, true)
    }

    /// Create a runtime setting (loaded on demand).
    pub fn runtime(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, value, false)
    }

    /// Parse value as JSON.
    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.value)
    }

    /// Set value from JSON.
    pub fn set_value<T: serde::Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.value = serde_json::to_string(value)?;
        Ok(())
    }
}

/// Well-known settings keys (as used in n8n).
pub mod setting_keys {
    pub const INSTANCE_ID: &str = "instanceId";
    pub const FIRST_RUN: &str = "firstRun";
    pub const LDAP_SETTINGS: &str = "ldapSettings";
    pub const SAML_SETTINGS: &str = "samlSettings";
    pub const ENCRYPTION_KEY: &str = "encryptionKey";
    pub const COMMUNITY_REGISTERED: &str = "communityRegistered";
    pub const LICENSE: &str = "license";
    pub const FEATURES: &str = "features";
}
