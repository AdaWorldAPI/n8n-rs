//! HTTP client that delegates `crew.*` steps to crewai-rust.
//!
//! crewai-rust is expected to expose:
//!   POST {CREWAI_ENDPOINT}/execute
//!
//! Request body:  [`StepDelegationRequest`] = `{ "step": UnifiedStep, "input": DataEnvelope }`
//! Response body: [`StepDelegationResponse`] = `{ "output": DataEnvelope, "step": Option<UnifiedStep> }`

use crate::types::{
    DataEnvelope, StepDelegationRequest, StepDelegationResponse, UnifiedStep,
};
use reqwest::Client;
use thiserror::Error;
use tracing::{debug, error};

#[derive(Debug, Error)]
pub enum CrewRouterError {
    #[error("HTTP request to crewai-rust failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("crewai-rust returned error status {status}: {body}")]
    Status { status: u16, body: String },

    #[error("CREWAI_ENDPOINT not configured")]
    NotConfigured,
}

/// Routes `crew.*` steps to a crewai-rust HTTP endpoint.
#[derive(Clone)]
pub struct CrewRouter {
    client: Client,
    endpoint: String,
}

impl CrewRouter {
    /// Create a new router targeting the given crewai-rust base URL.
    ///
    /// Example: `CrewRouter::new("http://crewai-rust.railway.internal:8080")`
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.into(),
        }
    }

    /// Create from the `CREWAI_ENDPOINT` environment variable.
    pub fn from_env() -> Result<Self, CrewRouterError> {
        let endpoint =
            std::env::var("CREWAI_ENDPOINT").map_err(|_| CrewRouterError::NotConfigured)?;
        Ok(Self::new(endpoint))
    }

    /// Delegate a crew step to crewai-rust and return the output envelope.
    ///
    /// The step's `reasoning`, `confidence`, and `alternatives` fields in
    /// the response will be populated by the AI agent.
    pub async fn execute(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<StepDelegationResponse, CrewRouterError> {
        let url = format!("{}/execute", self.endpoint.trim_end_matches('/'));

        let request = StepDelegationRequest {
            step: step.clone(),
            input: input.clone(),
        };

        debug!(
            step_id = %step.step_id,
            step_type = %step.step_type,
            url = %url,
            "Delegating crew step"
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
            error!(status = %status, body = %body, "crewai-rust returned error");
            return Err(CrewRouterError::Status {
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
    fn test_crew_router_endpoint() {
        let router = CrewRouter::new("http://localhost:8080");
        assert_eq!(router.endpoint(), "http://localhost:8080");
    }
}
