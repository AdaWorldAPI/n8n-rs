//! Protocol and format negotiation.
//!
//! This module provides intelligent selection of transport protocols and
//! serialization formats based on client capabilities, data characteristics,
//! and runtime conditions.

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::rest::DataHints;

/// Content format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentFormat {
    #[default]
    Json,
    JsonPretty,
    Ndjson,
    ArrowIpc,
    ArrowFlight,
    Protobuf,
    MessagePack,
    Cbor,
}

impl ContentFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "json-pretty" | "pretty" => Self::JsonPretty,
            "ndjson" | "jsonl" => Self::Ndjson,
            "arrow" | "arrow-ipc" => Self::ArrowIpc,
            "arrow-flight" | "flight" => Self::ArrowFlight,
            "protobuf" | "proto" | "pb" => Self::Protobuf,
            "msgpack" | "messagepack" => Self::MessagePack,
            "cbor" => Self::Cbor,
            _ => Self::Json,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::JsonPretty => "json-pretty",
            Self::Ndjson => "ndjson",
            Self::ArrowIpc => "arrow-ipc",
            Self::ArrowFlight => "arrow-flight",
            Self::Protobuf => "protobuf",
            Self::MessagePack => "msgpack",
            Self::Cbor => "cbor",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Json | Self::JsonPretty => "application/json",
            Self::Ndjson => "application/x-ndjson",
            Self::ArrowIpc => "application/vnd.apache.arrow.stream",
            Self::ArrowFlight => "application/vnd.apache.arrow.flight",
            Self::Protobuf => "application/x-protobuf",
            Self::MessagePack => "application/msgpack",
            Self::Cbor => "application/cbor",
        }
    }

    pub fn is_binary(&self) -> bool {
        !matches!(self, Self::Json | Self::JsonPretty | Self::Ndjson)
    }

    pub fn supports_streaming(&self) -> bool {
        matches!(self, Self::Ndjson | Self::ArrowIpc | Self::ArrowFlight)
    }

    pub fn supports_zero_copy(&self) -> bool {
        matches!(self, Self::ArrowIpc | Self::ArrowFlight)
    }

    /// Estimated overhead factor (1.0 = baseline JSON).
    pub fn overhead_factor(&self) -> f64 {
        match self {
            Self::Json | Self::JsonPretty => 1.0,
            Self::Ndjson => 1.0,
            Self::ArrowIpc => 0.3, // Much more efficient for columnar data
            Self::ArrowFlight => 0.25,
            Self::Protobuf => 0.5,
            Self::MessagePack => 0.6,
            Self::Cbor => 0.65,
        }
    }
}

/// Transport protocol options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Transport {
    #[default]
    Rest,
    Grpc,
    Flight,
    Stdio,
    WebSocket,
}

impl Transport {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rest" | "http" => Self::Rest,
            "grpc" => Self::Grpc,
            "flight" | "arrow-flight" => Self::Flight,
            "stdio" | "pipe" => Self::Stdio,
            "ws" | "websocket" => Self::WebSocket,
            _ => Self::Rest,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Grpc => "grpc",
            Self::Flight => "flight",
            Self::Stdio => "stdio",
            Self::WebSocket => "websocket",
        }
    }

    pub fn supports_streaming(&self) -> bool {
        matches!(self, Self::Grpc | Self::Flight | Self::WebSocket)
    }

    pub fn supports_bidirectional(&self) -> bool {
        matches!(self, Self::Grpc | Self::Flight | Self::Stdio | Self::WebSocket)
    }
}

/// Server capabilities advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportCapabilities {
    /// Available formats in preference order.
    pub formats: Vec<FormatCapability>,
    /// Available transports in preference order.
    pub transports: Vec<TransportCapability>,
    /// Server features.
    pub features: Vec<String>,
    /// Version info.
    pub version: String,
}

impl Default for TransportCapabilities {
    fn default() -> Self {
        Self {
            formats: vec![
                FormatCapability {
                    name: "arrow-flight".to_string(),
                    mime_type: "application/vnd.apache.arrow.flight".to_string(),
                    binary: true,
                    streaming: true,
                    zero_copy: true,
                    description: "Apache Arrow Flight - best for large datasets".to_string(),
                },
                FormatCapability {
                    name: "arrow-ipc".to_string(),
                    mime_type: "application/vnd.apache.arrow.stream".to_string(),
                    binary: true,
                    streaming: true,
                    zero_copy: true,
                    description: "Apache Arrow IPC - efficient columnar format".to_string(),
                },
                FormatCapability {
                    name: "protobuf".to_string(),
                    mime_type: "application/x-protobuf".to_string(),
                    binary: true,
                    streaming: false,
                    zero_copy: false,
                    description: "Protocol Buffers - compact binary format".to_string(),
                },
                FormatCapability {
                    name: "json".to_string(),
                    mime_type: "application/json".to_string(),
                    binary: false,
                    streaming: false,
                    zero_copy: false,
                    description: "JSON - universal compatibility".to_string(),
                },
                FormatCapability {
                    name: "ndjson".to_string(),
                    mime_type: "application/x-ndjson".to_string(),
                    binary: false,
                    streaming: true,
                    zero_copy: false,
                    description: "Newline-delimited JSON - streaming compatible".to_string(),
                },
            ],
            transports: vec![
                TransportCapability {
                    name: "flight".to_string(),
                    endpoint: "/flight".to_string(),
                    streaming: true,
                    bidirectional: true,
                    description: "Arrow Flight - high-performance data streaming".to_string(),
                },
                TransportCapability {
                    name: "grpc".to_string(),
                    endpoint: "/grpc".to_string(),
                    streaming: true,
                    bidirectional: true,
                    description: "gRPC - efficient RPC with streaming".to_string(),
                },
                TransportCapability {
                    name: "rest".to_string(),
                    endpoint: "/api/v1".to_string(),
                    streaming: false,
                    bidirectional: false,
                    description: "REST - universal HTTP compatibility".to_string(),
                },
                TransportCapability {
                    name: "stdio".to_string(),
                    endpoint: "stdio://".to_string(),
                    streaming: true,
                    bidirectional: true,
                    description: "STDIO - CLI and pipe integration".to_string(),
                },
            ],
            features: vec![
                "workflow-execution".to_string(),
                "hamming-similarity".to_string(),
                "zero-copy-transfer".to_string(),
                "streaming-results".to_string(),
                "format-negotiation".to_string(),
            ],
            version: "1.0.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatCapability {
    pub name: String,
    pub mime_type: String,
    pub binary: bool,
    pub streaming: bool,
    pub zero_copy: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportCapability {
    pub name: String,
    pub endpoint: String,
    pub streaming: bool,
    pub bidirectional: bool,
    pub description: String,
}

/// Format negotiator with health tracking.
pub struct FormatNegotiator {
    /// Health scores for each transport (0-100).
    transport_health: RwLock<HashMap<Transport, TransportHealth>>,
    /// Request counts for metrics.
    request_counts: HashMap<Transport, AtomicU64>,
    /// Configuration.
    config: NegotiatorConfig,
}

#[derive(Debug, Clone)]
pub struct TransportHealth {
    pub score: u32,
    pub last_success: Option<Instant>,
    pub last_failure: Option<Instant>,
    pub consecutive_failures: u32,
    pub avg_latency_ms: f64,
}

impl Default for TransportHealth {
    fn default() -> Self {
        Self {
            score: 100,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            avg_latency_ms: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NegotiatorConfig {
    /// Prefer zero-copy when data exceeds this size.
    pub zero_copy_threshold_bytes: u64,
    /// Prefer streaming when records exceed this count.
    pub streaming_threshold_records: u64,
    /// How long before retrying a failed transport.
    pub transport_retry_delay: Duration,
    /// Score below which transport is considered unhealthy.
    pub unhealthy_threshold: u32,
}

impl Default for NegotiatorConfig {
    fn default() -> Self {
        Self {
            zero_copy_threshold_bytes: 1024 * 1024, // 1MB
            streaming_threshold_records: 1000,
            transport_retry_delay: Duration::from_secs(30),
            unhealthy_threshold: 50,
        }
    }
}

impl FormatNegotiator {
    pub fn new() -> Self {
        Self::with_config(NegotiatorConfig::default())
    }

    pub fn with_config(config: NegotiatorConfig) -> Self {
        let mut transport_health = HashMap::new();
        transport_health.insert(Transport::Rest, TransportHealth::default());
        transport_health.insert(Transport::Grpc, TransportHealth::default());
        transport_health.insert(Transport::Flight, TransportHealth::default());
        transport_health.insert(Transport::Stdio, TransportHealth::default());

        let mut request_counts = HashMap::new();
        request_counts.insert(Transport::Rest, AtomicU64::new(0));
        request_counts.insert(Transport::Grpc, AtomicU64::new(0));
        request_counts.insert(Transport::Flight, AtomicU64::new(0));
        request_counts.insert(Transport::Stdio, AtomicU64::new(0));

        Self {
            transport_health: RwLock::new(transport_health),
            request_counts,
            config,
        }
    }

    /// Negotiate format from HTTP Accept headers.
    pub fn negotiate_from_headers(&self, headers: &HeaderMap) -> ContentFormat {
        // Check explicit format header first
        if let Some(fmt) = headers.get("x-preferred-format") {
            if let Ok(s) = fmt.to_str() {
                return ContentFormat::from_str(s);
            }
        }

        // Parse Accept header
        if let Some(accept) = headers.get("accept") {
            if let Ok(s) = accept.to_str() {
                return self.parse_accept_header(s);
            }
        }

        ContentFormat::Json
    }

    fn parse_accept_header(&self, accept: &str) -> ContentFormat {
        // Parse Accept header with quality values
        let mut formats: Vec<(ContentFormat, f32)> = accept
            .split(',')
            .filter_map(|part| {
                let mut parts = part.trim().split(';');
                let mime = parts.next()?.trim();
                let quality = parts
                    .find_map(|p| {
                        let mut kv = p.trim().split('=');
                        if kv.next()? == "q" {
                            kv.next()?.parse().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1.0);

                let format = match mime {
                    "application/vnd.apache.arrow.stream" => Some(ContentFormat::ArrowIpc),
                    "application/vnd.apache.arrow.flight" => Some(ContentFormat::ArrowFlight),
                    "application/x-protobuf" => Some(ContentFormat::Protobuf),
                    "application/json" => Some(ContentFormat::Json),
                    "application/x-ndjson" => Some(ContentFormat::Ndjson),
                    "application/msgpack" => Some(ContentFormat::MessagePack),
                    "application/cbor" => Some(ContentFormat::Cbor),
                    "*/*" => Some(ContentFormat::Json),
                    _ => None,
                };

                format.map(|f| (f, quality))
            })
            .collect();

        // Sort by quality (highest first)
        formats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        formats.first().map(|(f, _)| *f).unwrap_or(ContentFormat::Json)
    }

    /// Negotiate best format and transport based on client capabilities and data hints.
    pub fn negotiate(
        &self,
        client_formats: &[String],
        client_transports: &[String],
        data_hints: &DataHints,
    ) -> (String, String) {
        let client_formats: Vec<ContentFormat> = client_formats
            .iter()
            .map(|s| ContentFormat::from_str(s))
            .collect();

        let client_transports: Vec<Transport> = client_transports
            .iter()
            .map(|s| Transport::from_str(s))
            .collect();

        // Score each combination
        let mut best_score = 0.0;
        let mut best_format = ContentFormat::Json;
        let mut best_transport = Transport::Rest;

        for format in &client_formats {
            for transport in &client_transports {
                let score = self.score_combination(*format, *transport, data_hints);
                if score > best_score {
                    best_score = score;
                    best_format = *format;
                    best_transport = *transport;
                }
            }
        }

        (
            best_format.as_str().to_string(),
            best_transport.as_str().to_string(),
        )
    }

    fn score_combination(
        &self,
        format: ContentFormat,
        transport: Transport,
        hints: &DataHints,
    ) -> f64 {
        let mut score = 50.0;

        // Data size considerations
        if let Some(size) = hints.size_bytes {
            if size > self.config.zero_copy_threshold_bytes {
                if format.supports_zero_copy() {
                    score += 30.0;
                } else {
                    score -= 20.0;
                }
            }
        }

        // Streaming considerations
        if hints.streaming.unwrap_or(false) {
            if format.supports_streaming() && transport.supports_streaming() {
                score += 25.0;
            } else {
                score -= 30.0;
            }
        }

        // Columnar data
        if hints.columnar.unwrap_or(false) {
            if matches!(format, ContentFormat::ArrowIpc | ContentFormat::ArrowFlight) {
                score += 20.0;
            }
        }

        // Record count
        if let Some(count) = hints.record_count {
            if count > self.config.streaming_threshold_records {
                if format.supports_streaming() {
                    score += 15.0;
                }
            }
        }

        // Format efficiency
        score += (1.0 - format.overhead_factor()) * 20.0;

        // Format-transport compatibility bonuses
        match (format, transport) {
            (ContentFormat::ArrowFlight, Transport::Flight) => score += 20.0,
            (ContentFormat::ArrowIpc, Transport::Flight) => score += 15.0,
            (ContentFormat::Protobuf, Transport::Grpc) => score += 15.0,
            (ContentFormat::Json, Transport::Rest) => score += 5.0,
            (ContentFormat::Ndjson, Transport::Rest) => score += 10.0,
            _ => {}
        }

        score.max(0.0)
    }

    /// Record a successful request.
    pub async fn record_success(&self, transport: Transport, latency_ms: f64) {
        let mut health = self.transport_health.write().await;
        if let Some(h) = health.get_mut(&transport) {
            h.last_success = Some(Instant::now());
            h.consecutive_failures = 0;
            h.score = (h.score + 10).min(100);
            // Exponential moving average for latency
            h.avg_latency_ms = h.avg_latency_ms * 0.9 + latency_ms * 0.1;
        }

        if let Some(counter) = self.request_counts.get(&transport) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a failed request.
    pub async fn record_failure(&self, transport: Transport) {
        let mut health = self.transport_health.write().await;
        if let Some(h) = health.get_mut(&transport) {
            h.last_failure = Some(Instant::now());
            h.consecutive_failures += 1;
            h.score = h.score.saturating_sub(20);
        }
    }

    /// Check if transport is healthy.
    pub async fn is_healthy(&self, transport: Transport) -> bool {
        let health = self.transport_health.read().await;
        health
            .get(&transport)
            .map(|h| h.score >= self.config.unhealthy_threshold)
            .unwrap_or(false)
    }

    /// Get fallback transport if primary fails.
    pub async fn get_fallback(&self, failed: Transport) -> Transport {
        let health = self.transport_health.read().await;

        // Order of fallback preference
        let fallbacks = match failed {
            Transport::Flight => vec![Transport::Grpc, Transport::Rest, Transport::Stdio],
            Transport::Grpc => vec![Transport::Rest, Transport::Flight, Transport::Stdio],
            Transport::Rest => vec![Transport::Grpc, Transport::Stdio],
            Transport::Stdio => vec![Transport::Rest, Transport::Grpc],
            Transport::WebSocket => vec![Transport::Rest, Transport::Grpc],
        };

        for fb in fallbacks {
            if let Some(h) = health.get(&fb) {
                if h.score >= self.config.unhealthy_threshold {
                    return fb;
                }
            }
        }

        // Last resort
        Transport::Rest
    }

    /// Get health stats for all transports.
    pub async fn get_health_stats(&self) -> HashMap<String, TransportHealthStats> {
        let health = self.transport_health.read().await;
        let mut stats = HashMap::new();

        for (transport, h) in health.iter() {
            let count = self
                .request_counts
                .get(transport)
                .map(|c| c.load(Ordering::Relaxed))
                .unwrap_or(0);

            stats.insert(
                transport.as_str().to_string(),
                TransportHealthStats {
                    score: h.score,
                    healthy: h.score >= self.config.unhealthy_threshold,
                    avg_latency_ms: h.avg_latency_ms,
                    consecutive_failures: h.consecutive_failures,
                    total_requests: count,
                },
            );
        }

        stats
    }
}

impl Default for FormatNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportHealthStats {
    pub score: u32,
    pub healthy: bool,
    pub avg_latency_ms: f64,
    pub consecutive_failures: u32,
    pub total_requests: u64,
}

/// Macro for graceful fallback with format switching.
#[macro_export]
macro_rules! with_fallback {
    ($negotiator:expr, $transport:expr, $op:expr) => {{
        let start = std::time::Instant::now();
        match $op.await {
            Ok(result) => {
                let latency = start.elapsed().as_secs_f64() * 1000.0;
                $negotiator.record_success($transport, latency).await;
                Ok(result)
            }
            Err(e) => {
                $negotiator.record_failure($transport).await;
                let fallback = $negotiator.get_fallback($transport).await;
                tracing::warn!(
                    "Transport {:?} failed, falling back to {:?}: {}",
                    $transport,
                    fallback,
                    e
                );
                Err((e, fallback))
            }
        }
    }};
}
