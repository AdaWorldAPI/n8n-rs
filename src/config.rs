//! Configuration module - Environment-based configuration
//!
//! Matches the environment variables from docker-compose.yml and Dockerfile

use std::env;
use std::sync::Arc;

/// Application configuration loaded from environment variables
///
/// Note: Some fields are loaded for completeness but may not be actively used
/// in all code paths (e.g., auth fields for future middleware).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    // Server config
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub webhook_url: String,

    // External service URLs
    pub mcp_url: String,
    pub point_url: String,
    pub xai_url: String,

    // Redis (Upstash)
    pub redis_url: String,
    pub redis_token: String,

    // xAI API
    pub xai_key: String,

    // Auth (for future middleware)
    pub basic_auth_user: Option<String>,
    pub basic_auth_password: Option<String>,

    // Timezone
    pub timezone: String,

    // Unified contract — cross-runtime routing
    pub crewai_endpoint: Option<String>,
    pub ladybug_endpoint: Option<String>,
    pub database_url: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let host = env::var("N8N_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("N8N_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);
        let protocol = env::var("N8N_PROTOCOL").unwrap_or_else(|_| "http".to_string());

        let webhook_url = env::var("WEBHOOK_URL")
            .unwrap_or_else(|_| format!("{}://{}:{}/", protocol, host, port));

        Self {
            host,
            port,
            protocol,
            webhook_url,
            mcp_url: env::var("ADA_MCP_URL").unwrap_or_else(|_| "https://mcp.exo.red".to_string()),
            point_url: env::var("ADA_POINT_URL")
                .unwrap_or_else(|_| "https://point.exo.red".to_string()),
            xai_url: env::var("ADA_XAI_URL")
                .unwrap_or_else(|_| "https://api.x.ai/v1/chat/completions".to_string()),
            redis_url: env::var("UPSTASH_REDIS_REST_URL")
                .or_else(|_| env::var("ADA_REDIS_URL"))
                .unwrap_or_default(),
            redis_token: env::var("UPSTASH_REDIS_REST_TOKEN").unwrap_or_default(),
            xai_key: env::var("ADA_XAI_KEY").unwrap_or_default(),
            basic_auth_user: env::var("N8N_BASIC_AUTH_USER").ok(),
            basic_auth_password: env::var("N8N_BASIC_AUTH_PASSWORD").ok(),
            timezone: env::var("GENERIC_TIMEZONE").unwrap_or_else(|_| "Europe/Berlin".to_string()),
            crewai_endpoint: env::var("CREWAI_ENDPOINT").ok(),
            ladybug_endpoint: env::var("LADYBUG_ENDPOINT").ok(),
            database_url: env::var("DATABASE_URL").ok(),
        }
    }

    /// Get server bind address
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            http_client,
        }
    }
}
