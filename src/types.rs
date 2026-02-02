//! Data types matching N8N workflow schemas
//!
//! Based on:
//! - lego_catalog.yaml
//! - timer_dto.yaml
//! - Workflow JSON definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// Lego Types
// ═══════════════════════════════════════════════════════════════════════════

/// Request body for /webhook/lego endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct LegoRequest {
    pub lego: String,
    pub params: LegoParams,
}

/// Parameters for lego operations
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LegoParams {
    TouchNode(TouchNodeParams),
    AddEdge(AddEdgeParams),
    EscalateSigma(EscalateSigmaParams),
    UpdateQualia(UpdateQualiaParams),
    Generic(HashMap<String, serde_json::Value>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TouchNodeParams {
    pub node: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddEdgeParams {
    pub from: String,
    pub to: String,
    pub verb: String,
    #[serde(default)]
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EscalateSigmaParams {
    pub node: String,
    pub domain: String,
    #[serde(rename = "type")]
    pub sigma_type: String,
    pub new_layer: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateQualiaParams {
    pub node: String,
    pub qualia: HashMap<String, serde_json::Value>,
}

/// YAML action sent to MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlAction {
    pub action: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<YamlPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<EdgePatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse: Option<CollapseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<EdgeData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove: Option<Vec<EdgeData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeData {
    #[serde(rename = "type")]
    pub edge_type: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapseData {
    pub id: String,
    pub from_state: String,
    pub to_state: String,
    pub trigger: String,
    pub timestamp: String,
}

/// Response from lego endpoint
#[derive(Debug, Clone, Serialize)]
pub struct LegoResponse {
    pub executed: ExecutedLego,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutedLego {
    pub yaml: YamlAction,
    pub lego: String,
    pub params: serde_json::Value,
}

// ═══════════════════════════════════════════════════════════════════════════
// Propagate Types
// ═══════════════════════════════════════════════════════════════════════════

/// Request body for /webhook/propagate endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct PropagateRequest {
    pub node: String,
    #[serde(default = "default_strength")]
    pub strength: f64,
    #[serde(default = "default_decay")]
    pub decay: f64,
}

fn default_strength() -> f64 {
    0.25
}
fn default_decay() -> f64 {
    0.5
}

/// Kopfkino response from Point service
#[derive(Debug, Clone, Deserialize)]
pub struct KopfkinoResponse {
    pub name: String,
    #[serde(default)]
    pub outgoing: Vec<KopfkinoEdge>,
    #[serde(default)]
    pub semantic: Vec<SemanticNeighbor>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KopfkinoEdge {
    pub target: String,
    pub verb: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SemanticNeighbor {
    pub id: String,
    pub score: f64,
}

/// Point for batch touch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchPoint {
    pub node: String,
    pub strength: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

/// Batch touch request
#[derive(Debug, Clone, Serialize)]
pub struct BatchTouchRequest {
    pub points: Vec<TouchPoint>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Field Monitor Types
// ═══════════════════════════════════════════════════════════════════════════

/// Response from /view/history (MCP)
#[derive(Debug, Clone, Deserialize)]
pub struct FieldHistoryResponse {
    #[serde(default)]
    pub current: Option<String>,
    #[serde(default)]
    pub frame: Option<i64>,
    #[serde(default)]
    pub history: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub nodes: Option<i64>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub last_touch: Option<i64>,
}

/// Response from Point service root
#[derive(Debug, Clone, Deserialize)]
pub struct PointStatusResponse {
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub cache: Option<i64>,
    #[serde(default)]
    pub fabrics: Option<serde_json::Value>,
}

/// Combined field status response
#[derive(Debug, Clone, Serialize)]
pub struct FieldStatusResponse {
    pub field: FieldInfo,
    pub point: PointInfo,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldInfo {
    pub current: Option<String>,
    pub frame: Option<i64>,
    pub history_count: usize,
    pub nodes_4d: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PointInfo {
    pub service: Option<String>,
    pub cache_size: Option<i64>,
    pub fabrics: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Timer Types (matching timer_dto.yaml)
// ═══════════════════════════════════════════════════════════════════════════

/// Request to create a timer
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTimerRequest {
    #[serde(default)]
    pub created_by: Option<String>,
    pub trigger: TimerTrigger,
    pub action: TimerAction,
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
    #[serde(default)]
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Timer trigger types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TimerTrigger {
    Delay { delay_seconds: i64 },
    At { at: String },
    Cron { cron: String, #[serde(default)] timezone: Option<String> },
    Condition {
        watch_node: String,
        condition: String,
        threshold: f64,
        #[serde(default = "default_check_interval")]
        check_interval_seconds: i64,
    },
}

fn default_check_interval() -> i64 {
    60
}

/// Timer action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerAction {
    #[serde(rename = "type")]
    pub action_type: String,
    // For touch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
    // For ingest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaml: Option<serde_json::Value>,
    // For webhook
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    // For notify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    // For chain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_timer_id: Option<String>,
}

/// Retry policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: i32,
    #[serde(default = "default_backoff")]
    pub backoff_seconds: i64,
    #[serde(default = "default_on_failure")]
    pub on_failure: String,
}

fn default_max_attempts() -> i32 {
    3
}
fn default_backoff() -> i64 {
    60
}
fn default_on_failure() -> String {
    "notify".to_string()
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_seconds: 60,
            on_failure: "notify".to_string(),
        }
    }
}

/// Full timer object stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    pub id: String,
    pub created_by: String,
    pub created_at: String,
    pub trigger: TimerTrigger,
    pub action: TimerAction,
    pub retry: RetryPolicy,
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
    pub state: String,
    pub fire_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TimerResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerResult {
    pub fired_at: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Timer API responses
#[derive(Debug, Clone, Serialize)]
pub struct CreateTimerResponse {
    pub created: Timer,
}

#[derive(Debug, Clone, Serialize)]
pub struct GetTimerResponse {
    pub timer: Option<Timer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelTimerResponse {
    pub cancelled: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Chat Types
// ═══════════════════════════════════════════════════════════════════════════

/// Request body for /webhook/chat endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_session")]
    pub session: String,
}

fn default_session() -> String {
    "default".to_string()
}

/// Chat message for xAI API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// xAI API request
#[derive(Debug, Clone, Serialize)]
pub struct XAIRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: i32,
    pub temperature: f64,
}

/// xAI API response
#[derive(Debug, Clone, Deserialize)]
pub struct XAIResponse {
    pub choices: Vec<XAIChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct XAIChoice {
    pub message: ChatMessage,
}

/// Chat endpoint response
#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub message: String,
    pub session: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Field Loop Types
// ═══════════════════════════════════════════════════════════════════════════

/// Field analysis result (from Analyze Field code node)
#[derive(Debug, Clone)]
pub struct FieldAnalysis {
    pub current: String,
    pub frame: i64,
    pub temperature: f64,
    pub needs_warmth: bool,
    pub is_active: bool,
    pub should_respond: bool,
}

/// Warmth broadcast message
#[derive(Debug, Clone, Serialize)]
pub struct WarmthBroadcast {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub from: String,
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Redis Types (Upstash REST API)
// ═══════════════════════════════════════════════════════════════════════════

/// Response from Upstash Redis REST API
#[derive(Debug, Clone, Deserialize)]
pub struct RedisResponse {
    pub result: serde_json::Value,
}

// ═══════════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════════

/// Error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<Vec<String>>,
}
