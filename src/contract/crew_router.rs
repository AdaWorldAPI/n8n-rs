//! Crew step router — routes crew.* steps to the crewAI HTTP runtime
//!
//! When ada-n8n encounters a workflow step with type "crew.*", it delegates
//! execution to the crewAI service over HTTP. The crewAI service runs
//! the agent task and returns a DataEnvelope with the result.

use anyhow::Result;
use serde_json::json;
use tracing::{info, warn};

use super::types::{DataEnvelope, UnifiedStep};

/// Routes crew.* workflow steps to the crewAI HTTP runtime.
pub struct CrewRouter {
    /// Base URL for the crewAI HTTP endpoint (CREWAI_ENDPOINT env var)
    crew_endpoint: String,
    /// Shared HTTP client
    client: reqwest::Client,
}

impl CrewRouter {
    /// Create a new CrewRouter pointing at the given crewAI service endpoint.
    pub fn new(crew_endpoint: String, client: reqwest::Client) -> Self {
        Self {
            crew_endpoint,
            client,
        }
    }

    /// Route a crew.* step to the crewAI runtime.
    ///
    /// POSTs the step definition and input envelope to the crewAI /execute
    /// endpoint. crewAI executes the agent task and returns an output envelope.
    pub async fn execute_crew_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        info!(
            step_id = %step.step_id,
            step_type = %step.step_type,
            "Routing step to crewAI: {}",
            self.crew_endpoint
        );

        let response = self
            .client
            .post(&format!("{}/execute", self.crew_endpoint))
            .json(&json!({
                "step": step,
                "input": input,
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            warn!(
                step_id = %step.step_id,
                status = %status,
                "crewAI returned error: {}",
                error_body
            );
            anyhow::bail!("crewAI returned {}: {}", status, error_body);
        }

        let envelope: DataEnvelope = response.json().await?;
        info!(
            step_id = %step.step_id,
            output_key = %envelope.output_key,
            "crewAI step completed"
        );

        Ok(envelope)
    }
}

/// Routes lb.* steps to the ladybug enrichment service (optional).
pub struct LadybugRouter {
    /// Base URL for the ladybug HTTP endpoint (LADYBUG_ENDPOINT env var)
    lb_endpoint: String,
    /// Shared HTTP client
    client: reqwest::Client,
}

impl LadybugRouter {
    /// Create a new LadybugRouter pointing at the given ladybug service endpoint.
    pub fn new(lb_endpoint: String, client: reqwest::Client) -> Self {
        Self {
            lb_endpoint,
            client,
        }
    }

    /// Route an lb.* step to the ladybug runtime.
    ///
    /// POSTs the step definition and input envelope to the ladybug /execute
    /// endpoint. Ladybug performs enrichment and returns the output envelope.
    pub async fn execute_lb_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        info!(
            step_id = %step.step_id,
            step_type = %step.step_type,
            "Routing step to ladybug: {}",
            self.lb_endpoint
        );

        let response = self
            .client
            .post(&format!("{}/execute", self.lb_endpoint))
            .json(&json!({
                "step": step,
                "input": input,
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            warn!(
                step_id = %step.step_id,
                status = %status,
                "ladybug returned error: {}",
                error_body
            );
            anyhow::bail!("ladybug returned {}: {}", status, error_body);
        }

        let envelope: DataEnvelope = response.json().await?;
        info!(
            step_id = %step.step_id,
            output_key = %envelope.output_key,
            "ladybug step completed"
        );

        Ok(envelope)
    }
}
