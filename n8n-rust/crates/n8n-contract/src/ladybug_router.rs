//! HTTP client that delegates `lb.*` steps to ladybug-rs.
//!
//! ladybug-rs is expected to expose:
//!   POST {LADYBUG_ENDPOINT}/api/v1/resonate   — for `lb.resonate` steps
//!   POST {LADYBUG_ENDPOINT}/api/v1/collapse    — for `lb.collapse` steps
//!
//! Transport: HTTP/JSON.  Arrow Flight DoAction is an alternative for bulk
//! operations but HTTP is the default for single-step delegation.
//!
//! Request body:  [`StepDelegationRequest`]
//! Response body: [`StepDelegationResponse`]

use crate::types::{
    DataEnvelope, StepDelegationRequest, StepDelegationResponse, UnifiedStep,
};
use reqwest::Client;
use thiserror::Error;
use tracing::{debug, error};

#[derive(Debug, Error)]
pub enum LadybugRouterError {
    #[error("HTTP request to ladybug-rs failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("ladybug-rs returned error status {status}: {body}")]
    Status { status: u16, body: String },

    #[error("LADYBUG_ENDPOINT not configured")]
    NotConfigured,

    #[error("Unknown ladybug step type: {0}")]
    UnknownStepType(String),
}

/// Routes `lb.*` steps to a ladybug-rs HTTP endpoint.
#[derive(Clone)]
pub struct LadybugRouter {
    client: Client,
    endpoint: String,
}

impl LadybugRouter {
    /// Create a new router targeting the given ladybug-rs base URL.
    ///
    /// Example: `LadybugRouter::new("http://ladybug-rs-production.up.railway.app")`
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.into(),
        }
    }

    /// Create from the `LADYBUG_ENDPOINT` environment variable.
    pub fn from_env() -> Result<Self, LadybugRouterError> {
        let endpoint =
            std::env::var("LADYBUG_ENDPOINT").map_err(|_| LadybugRouterError::NotConfigured)?;
        Ok(Self::new(endpoint))
    }

    /// Delegate a ladybug step to ladybug-rs and return the output envelope.
    ///
    /// Routing:
    /// - `lb.resonate` → POST /api/v1/resonate
    /// - `lb.collapse` → POST /api/v1/collapse
    pub async fn execute(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<StepDelegationResponse, LadybugRouterError> {
        let path = match step.step_type.as_str() {
            "lb.resonate" => "/api/v1/resonate",
            "lb.collapse" => "/api/v1/collapse",
            other => return Err(LadybugRouterError::UnknownStepType(other.to_string())),
        };

        let url = format!(
            "{}{}",
            self.endpoint.trim_end_matches('/'),
            path
        );

        let request = StepDelegationRequest {
            step: step.clone(),
            input: input.clone(),
        };

        debug!(
            step_id = %step.step_id,
            step_type = %step.step_type,
            url = %url,
            "Delegating ladybug step"
        );

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            error!(status = %status, body = %body, "ladybug-rs returned error");
            return Err(LadybugRouterError::Status {
                status: status.as_u16(),
                body,
            });
        }

        let response: StepDelegationResponse = resp.json().await?;
        Ok(response)
    }

    /// Returns the configured endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ladybug_router_endpoint() {
        let router = LadybugRouter::new("http://localhost:9090");
        assert_eq!(router.endpoint(), "http://localhost:9090");
    }
}
