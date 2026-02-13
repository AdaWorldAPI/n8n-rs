//! REST API transport with content negotiation.
//!
//! Provides HTTP/REST endpoints that automatically negotiate between
//! JSON, Arrow IPC, and protobuf based on Accept headers and format hints.

use axum::{
    body::{Body, Bytes},
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::negotiate::{ContentFormat, FormatNegotiator, TransportCapabilities};

/// REST API state.
#[derive(Clone)]
pub struct RestState<S> {
    pub service: Arc<S>,
    pub negotiator: Arc<FormatNegotiator>,
}

/// Format query parameter for explicit format selection.
#[derive(Debug, Deserialize)]
pub struct FormatQuery {
    /// Explicit format: json, arrow, protobuf, msgpack.
    #[serde(rename = "fmt")]
    pub format: Option<String>,
    /// Compression: none, gzip, zstd, lz4.
    #[serde(rename = "compress")]
    pub compression: Option<String>,
    /// Pretty print (JSON only).
    pub pretty: Option<bool>,
}

/// Response wrapper that handles content negotiation.
pub struct NegotiatedResponse {
    pub format: ContentFormat,
    pub data: ResponseData,
    pub status: StatusCode,
}

/// Response data variants.
pub enum ResponseData {
    Json(serde_json::Value),
    Arrow(Bytes),
    Protobuf(Bytes),
    Binary(Bytes),
}

impl IntoResponse for NegotiatedResponse {
    fn into_response(self) -> Response {
        let (content_type, body) = match self.data {
            ResponseData::Json(value) => {
                let json = serde_json::to_vec(&value).unwrap_or_default();
                ("application/json", json)
            }
            ResponseData::Arrow(bytes) => {
                ("application/vnd.apache.arrow.stream", bytes.to_vec())
            }
            ResponseData::Protobuf(bytes) => {
                ("application/x-protobuf", bytes.to_vec())
            }
            ResponseData::Binary(bytes) => {
                ("application/octet-stream", bytes.to_vec())
            }
        };

        let mut response = Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, content_type)
            .header("X-Content-Format", self.format.as_str())
            .header("X-Format-Negotiated", "true");

        // Add upgrade hints
        if self.format == ContentFormat::Json {
            response = response.header(
                "X-Upgrade-Available",
                "arrow-flight; grpc; arrow-ipc",
            );
        }

        response.body(Body::from(body)).unwrap()
    }
}

/// API error response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiatedApiError {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Hint for better transport/format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_hint: Option<FormatHint>,
}

/// Hint for client about better formats/transports.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatHint {
    /// Recommended format for this data.
    pub recommended: String,
    /// Why this format is better.
    pub reason: String,
    /// Endpoint to use for upgrade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_endpoint: Option<String>,
}

impl IntoResponse for NegotiatedApiError {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "INVALID_ARGUMENT" => StatusCode::BAD_REQUEST,
            "PERMISSION_DENIED" => StatusCode::FORBIDDEN,
            "UNAUTHENTICATED" => StatusCode::UNAUTHORIZED,
            "UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

/// Middleware for content negotiation.
pub async fn negotiate_content(
    State(negotiator): State<Arc<FormatNegotiator>>,
    headers: HeaderMap,
    Query(query): Query<FormatQuery>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Determine preferred format
    let format = if let Some(fmt) = &query.format {
        ContentFormat::from_str(fmt)
    } else {
        negotiator.negotiate_from_headers(&headers)
    };

    // Store format in request extensions
    let mut request = request;
    request.extensions_mut().insert(format);

    let mut response = next.run(request).await;

    // Add negotiation headers to response
    response.headers_mut().insert(
        "X-Accepted-Formats",
        HeaderValue::from_static("json, arrow-ipc, arrow-flight, protobuf"),
    );

    response
}

/// Capabilities endpoint - returns server capabilities and available formats.
pub async fn get_capabilities() -> Json<TransportCapabilities> {
    Json(TransportCapabilities::default())
}

/// Format negotiation endpoint.
#[derive(Debug, Deserialize)]
pub struct NegotiateRequest {
    /// Client's supported formats (preference order).
    pub formats: Vec<String>,
    /// Client's supported transports.
    pub transports: Vec<String>,
    /// Data characteristics for optimization hints.
    #[serde(default)]
    pub data_hints: DataHints,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataHints {
    /// Expected data size (bytes).
    pub size_bytes: Option<u64>,
    /// Is data streaming?
    pub streaming: Option<bool>,
    /// Is data tabular/columnar?
    pub columnar: Option<bool>,
    /// Expected record count.
    pub record_count: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateResponse {
    /// Selected format.
    pub format: String,
    /// Selected transport.
    pub transport: String,
    /// Endpoint to use.
    pub endpoint: String,
    /// Why this was selected.
    pub reason: String,
    /// Alternative options.
    pub alternatives: Vec<AlternativeOption>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeOption {
    pub format: String,
    pub transport: String,
    pub endpoint: String,
    pub trade_off: String,
}

pub async fn negotiate_format(
    State(negotiator): State<Arc<FormatNegotiator>>,
    Json(request): Json<NegotiateRequest>,
) -> Json<NegotiateResponse> {
    let (format, transport) = negotiator.negotiate(&request.formats, &request.transports, &request.data_hints);

    let endpoint = match transport.as_str() {
        "grpc" => "/grpc",
        "flight" => "/flight",
        "stdio" => "stdio://",
        _ => "/api/v1",
    };

    let reason = match (format.as_str(), transport.as_str()) {
        ("arrow-ipc", "flight") => "Best for large tabular data with streaming".to_string(),
        ("arrow-ipc", _) => "Efficient columnar format with zero-copy".to_string(),
        ("protobuf", "grpc") => "Efficient binary format with strong typing".to_string(),
        ("json", _) => "Universal compatibility, human readable".to_string(),
        _ => "Selected based on client capabilities".to_string(),
    };

    let mut alternatives = vec![];

    // Always offer alternatives
    if format != "json" {
        alternatives.push(AlternativeOption {
            format: "json".to_string(),
            transport: "rest".to_string(),
            endpoint: "/api/v1".to_string(),
            trade_off: "More overhead but universal compatibility".to_string(),
        });
    }

    if format != "arrow-ipc" && request.data_hints.columnar.unwrap_or(false) {
        alternatives.push(AlternativeOption {
            format: "arrow-ipc".to_string(),
            transport: "flight".to_string(),
            endpoint: "/flight".to_string(),
            trade_off: "Zero-copy, better for large datasets".to_string(),
        });
    }

    Json(NegotiateResponse {
        format,
        transport,
        endpoint: endpoint.to_string(),
        reason,
        alternatives,
    })
}

/// Format switch endpoint - allows client to change format mid-session.
#[derive(Debug, Deserialize)]
pub struct SwitchFormatRequest {
    /// New format to use.
    pub format: String,
    /// New transport (optional).
    pub transport: Option<String>,
    /// Session/request ID to apply to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchFormatResponse {
    pub success: bool,
    pub new_format: String,
    pub new_transport: String,
    pub new_endpoint: String,
    /// Headers to include in future requests.
    pub headers: std::collections::HashMap<String, String>,
}

pub async fn switch_format(
    Json(request): Json<SwitchFormatRequest>,
) -> Json<SwitchFormatResponse> {
    let format = ContentFormat::from_str(&request.format);
    let transport = request.transport.unwrap_or_else(|| {
        match format {
            ContentFormat::ArrowIpc | ContentFormat::ArrowFlight => "flight".to_string(),
            ContentFormat::Protobuf => "grpc".to_string(),
            _ => "rest".to_string(),
        }
    });

    let endpoint = match transport.as_str() {
        "grpc" => "/grpc",
        "flight" => "/flight",
        _ => "/api/v1",
    };

    let mut headers = std::collections::HashMap::new();
    headers.insert("Accept".to_string(), format.mime_type().to_string());
    headers.insert("X-Preferred-Format".to_string(), format.as_str().to_string());

    Json(SwitchFormatResponse {
        success: true,
        new_format: format.as_str().to_string(),
        new_transport: transport,
        new_endpoint: endpoint.to_string(),
        headers,
    })
}

/// Create the REST API router.
pub fn create_router(negotiator: Arc<FormatNegotiator>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    Router::new()
        // Negotiation endpoints
        .route("/api/v1/capabilities", get(get_capabilities))
        .route("/api/v1/negotiate", post(negotiate_format))
        .route("/api/v1/format/switch", post(switch_format))
        // Health/status
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        // Add negotiation middleware
        .layer(middleware::from_fn_with_state(
            negotiator.clone(),
            negotiate_content,
        ))
        .layer(cors)
        .with_state(negotiator)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn readiness_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ready",
        "transports": {
            "rest": true,
            "grpc": true,
            "flight": true,
            "stdio": true
        },
        "formats": {
            "json": true,
            "arrow-ipc": true,
            "protobuf": true
        }
    }))
}

/// Workflow REST endpoints.
pub mod workflow {
    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct ListQuery {
        pub limit: Option<u32>,
        pub offset: Option<u32>,
        #[serde(rename = "fmt")]
        pub format: Option<String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ListResponse<T> {
        pub data: Vec<T>,
        pub total: u64,
        pub limit: u32,
        pub offset: u32,
        /// Format upgrade hint.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format_hint: Option<FormatHint>,
    }

    impl<T: Serialize> ListResponse<T> {
        pub fn with_hint(mut self, hint: FormatHint) -> Self {
            self.format_hint = Some(hint);
            self
        }
    }
}

/// Execution REST endpoints.
pub mod execution {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExecuteRequest {
        pub workflow_id: String,
        #[serde(default)]
        pub input_data: serde_json::Value,
        /// Preferred output format.
        #[serde(rename = "fmt")]
        pub format: Option<String>,
        /// Stream results as they complete.
        pub stream: Option<bool>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExecuteResponse {
        pub execution_id: String,
        pub status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result: Option<serde_json::Value>,
        /// If streaming, where to connect.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub stream_endpoint: Option<StreamEndpoint>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StreamEndpoint {
        /// Primary streaming endpoint (Flight for best performance).
        pub flight: String,
        /// WebSocket fallback.
        pub websocket: String,
        /// SSE fallback.
        pub sse: String,
        /// Recommended option.
        pub recommended: String,
    }
}
