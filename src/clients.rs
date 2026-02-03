//! External HTTP clients for MCP, Point, and xAI services
//!
//! Matches the HTTP requests made in N8N workflows

use crate::config::AppState;
use crate::types::*;
use anyhow::{Context, Result};
use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════
// MCP Client (mcp.exo.red)
// ═══════════════════════════════════════════════════════════════════════════

pub struct McpClient<'a> {
    state: &'a AppState,
}

impl<'a> McpClient<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// POST /ingest/yaml - Execute YAML action
    pub async fn ingest_yaml(&self, action: &YamlAction) -> Result<Value> {
        let url = format!("{}/ingest/yaml", self.state.config.mcp_url);
        let response = self
            .state
            .http_client
            .post(&url)
            .json(action)
            .send()
            .await
            .context("Failed to send request to MCP")?;

        response
            .json()
            .await
            .context("Failed to parse MCP response")
    }

    /// POST /ingest/yaml with raw YAML string
    pub async fn ingest_yaml_raw(&self, yaml: &str) -> Result<Value> {
        let url = format!("{}/ingest/yaml", self.state.config.mcp_url);
        let response = self
            .state
            .http_client
            .post(&url)
            .header("Content-Type", "text/yaml")
            .body(yaml.to_string())
            .send()
            .await
            .context("Failed to send request to MCP")?;

        response
            .json()
            .await
            .context("Failed to parse MCP response")
    }

    /// GET /view/history - Get field state
    pub async fn get_history(&self) -> Result<FieldHistoryResponse> {
        let url = format!("{}/view/history", self.state.config.mcp_url);
        let response = self
            .state
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch field history")?;

        response
            .json()
            .await
            .context("Failed to parse field history response")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Point Client (point.exo.red)
// ═══════════════════════════════════════════════════════════════════════════

pub struct PointClient<'a> {
    state: &'a AppState,
}

impl<'a> PointClient<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// GET / - Get point service status
    pub async fn get_status(&self) -> Result<PointStatusResponse> {
        let url = format!("{}/", self.state.config.point_url);
        let response = self
            .state
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch point status")?;

        response
            .json()
            .await
            .context("Failed to parse point status response")
    }

    /// GET /kopfkino/{node} - Get node neighbors
    pub async fn get_kopfkino(&self, node: &str) -> Result<KopfkinoResponse> {
        let url = format!("{}/kopfkino/{}", self.state.config.point_url, node);
        let response = self
            .state
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch kopfkino")?;

        response
            .json()
            .await
            .context("Failed to parse kopfkino response")
    }

    /// POST /point/batch - Batch touch nodes
    pub async fn batch_touch(&self, request: &BatchTouchRequest) -> Result<Value> {
        let url = format!("{}/point/batch", self.state.config.point_url);
        let response = self
            .state
            .http_client
            .post(&url)
            .json(request)
            .send()
            .await
            .context("Failed to send batch touch")?;

        response
            .json()
            .await
            .context("Failed to parse batch touch response")
    }

    /// GET /point/{node}?s={strength} - Touch single node
    pub async fn touch_node(&self, node: &str, strength: f64) -> Result<Value> {
        let url = format!("{}/point/{}?s={}", self.state.config.point_url, node, strength);
        let response = self
            .state
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to touch node")?;

        response
            .json()
            .await
            .context("Failed to parse touch response")
    }

    /// GET /point/chat - Touch field on chat
    pub async fn touch_chat(&self) -> Result<Value> {
        let url = format!("{}/point/chat", self.state.config.point_url);
        let response = self
            .state
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to touch chat")?;

        let json_result: Result<Value, _> = response.json().await;
        Ok(json_result.unwrap_or(Value::Null))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// xAI Client (api.x.ai)
// ═══════════════════════════════════════════════════════════════════════════

pub struct XAIClient<'a> {
    state: &'a AppState,
}

impl<'a> XAIClient<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// POST /v1/chat/completions - Chat with Grok
    pub async fn chat(&self, messages: Vec<ChatMessage>, max_tokens: i32, temperature: f64) -> Result<String> {
        let request = XAIRequest {
            model: "grok-3-latest".to_string(),
            messages,
            max_tokens,
            temperature,
        };

        let response = self
            .state
            .http_client
            .post(&self.state.config.xai_url)
            .header("Authorization", format!("Bearer {}", self.state.config.xai_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to xAI")?;

        let xai_response: XAIResponse = response
            .json()
            .await
            .context("Failed to parse xAI response")?;

        Ok(xai_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "I am here.".to_string()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Generic HTTP Client for timer webhook actions
// ═══════════════════════════════════════════════════════════════════════════

pub struct GenericHttpClient<'a> {
    state: &'a AppState,
}

impl<'a> GenericHttpClient<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// Execute a generic HTTP request (for timer webhook actions)
    pub async fn execute(&self, url: &str, method: &str, body: Option<Value>) -> Result<Value> {
        let request = match method.to_uppercase().as_str() {
            "GET" => self.state.http_client.get(url),
            "POST" => self.state.http_client.post(url),
            "PUT" => self.state.http_client.put(url),
            "DELETE" => self.state.http_client.delete(url),
            "PATCH" => self.state.http_client.patch(url),
            _ => self.state.http_client.post(url),
        };

        let request = if let Some(body) = body {
            request.json(&body)
        } else {
            request
        };

        let response = request.send().await.context("Failed to execute HTTP request")?;

        let json_result: Result<Value, _> = response.json().await;
        Ok(json_result.unwrap_or(Value::Null))
    }
}
