//! Multi-transport support with intelligent negotiation.
//!
//! This module provides:
//! - STDIO transport for CLI/pipe communication
//! - REST API with content negotiation
//! - Intelligent format/protocol negotiation
//! - Graceful fallback between transports
//!
//! # Protocol Selection
//!
//! The system automatically selects the best protocol based on:
//! - Client capabilities (advertised formats/transports)
//! - Data characteristics (size, streaming, columnar)
//! - Transport health (latency, error rates)
//!
//! # Format Negotiation
//!
//! Formats are selected in this priority order for optimal performance:
//! 1. Arrow Flight (streaming, zero-copy, columnar)
//! 2. Arrow IPC (zero-copy, columnar)
//! 3. Protobuf (compact binary)
//! 4. JSON (universal fallback)
//!
//! # Graceful Fallback
//!
//! When a transport fails, the system:
//! 1. Records the failure for health tracking
//! 2. Suggests an alternative transport
//! 3. Provides upgrade hints when using fallback
//!
//! # Example: Content Negotiation
//!
//! ```text
//! # Request with format preference
//! GET /api/v1/executions/123
//! Accept: application/vnd.apache.arrow.stream, application/json;q=0.5
//! X-Preferred-Format: arrow-ipc
//!
//! # Response with negotiation info
//! HTTP/1.1 200 OK
//! Content-Type: application/vnd.apache.arrow.stream
//! X-Content-Format: arrow-ipc
//! X-Format-Negotiated: true
//! X-Upgrade-Available: arrow-flight
//! ```
//!
//! # Example: Format Switch
//!
//! ```text
//! # Request to switch format mid-session
//! POST /api/v1/format/switch
//! {
//!   "format": "arrow-flight",
//!   "transport": "flight"
//! }
//!
//! # Response with new endpoint
//! {
//!   "success": true,
//!   "newFormat": "arrow-flight",
//!   "newTransport": "flight",
//!   "newEndpoint": "/flight",
//!   "headers": {
//!     "Accept": "application/vnd.apache.arrow.flight"
//!   }
//! }
//! ```

pub mod api;
pub mod negotiate;
pub mod rest;
pub mod stdio;

pub use api::*;
pub use negotiate::*;
pub use rest::*;
pub use stdio::*;

use std::sync::Arc;
use tokio::sync::watch;

/// Unified transport manager.
pub struct TransportManager {
    negotiator: Arc<FormatNegotiator>,
    shutdown: watch::Sender<bool>,
}

impl TransportManager {
    pub fn new() -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            negotiator: Arc::new(FormatNegotiator::new()),
            shutdown,
        }
    }

    pub fn negotiator(&self) -> Arc<FormatNegotiator> {
        self.negotiator.clone()
    }

    /// Start all transports.
    pub async fn start<S>(&self, service: Arc<S>, config: TransportConfig) -> anyhow::Result<()>
    where
        S: Send + Sync + 'static,
    {
        let mut handles = vec![];

        // Start REST server
        if config.rest_enabled {
            let router = create_router(self.negotiator.clone());
            let addr: std::net::SocketAddr = config.rest_addr.parse()?;

            let handle = tokio::spawn(async move {
                tracing::info!("REST server listening on {}", addr);
                let listener = tokio::net::TcpListener::bind(addr).await?;
                axum::serve(listener, router).await?;
                Ok::<_, anyhow::Error>(())
            });
            handles.push(handle);
        }

        // STDIO runs in foreground if enabled
        if config.stdio_enabled {
            let (transport, mut handler) = StdioTransport::new();
            let _service = service.clone();

            let handle = tokio::spawn(async move {
                // Handle STDIO messages
                while let Some(msg) = handler.recv().await {
                    match msg {
                        StdioMessage::Request(req) => {
                            // Process request...
                            let response = StdioMessage::Response(StdioResponse {
                                id: req.id,
                                result: Some(serde_json::json!({"status": "ok"})),
                                binary_ref: None,
                                format: Some("json".to_string()),
                            });
                            let _ = handler.send(response).await;
                        }
                        StdioMessage::Negotiate(_) => {
                            let caps = StdioMessage::Negotiate(NegotiateMessage::server_capabilities());
                            let _ = handler.send(caps).await;
                        }
                        _ => {}
                    }
                }
                Ok::<_, anyhow::Error>(())
            });
            handles.push(handle);

            // Run transport
            tokio::spawn(async move {
                if let Err(e) = transport.run().await {
                    tracing::error!("STDIO transport error: {}", e);
                }
            });
        }

        // Wait for shutdown or error
        let mut rx = self.shutdown.subscribe();
        tokio::select! {
            _ = rx.changed() => {
                tracing::info!("Shutdown signal received");
            }
            result = futures::future::try_join_all(handles) => {
                if let Err(e) = result {
                    tracing::error!("Transport error: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Shutdown all transports.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Transport configuration.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Enable REST API.
    pub rest_enabled: bool,
    /// REST API address.
    pub rest_addr: String,
    /// Enable gRPC.
    pub grpc_enabled: bool,
    /// gRPC address.
    pub grpc_addr: String,
    /// Enable Arrow Flight.
    pub flight_enabled: bool,
    /// Flight address.
    pub flight_addr: String,
    /// Enable STDIO.
    pub stdio_enabled: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            rest_enabled: true,
            rest_addr: "0.0.0.0:8080".to_string(),
            grpc_enabled: true,
            grpc_addr: "0.0.0.0:50051".to_string(),
            flight_enabled: true,
            flight_addr: "0.0.0.0:50052".to_string(),
            stdio_enabled: false,
        }
    }
}

/// Connection info for clients.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    /// REST API endpoint.
    pub rest_endpoint: Option<String>,
    /// gRPC endpoint.
    pub grpc_endpoint: Option<String>,
    /// Arrow Flight endpoint.
    pub flight_endpoint: Option<String>,
    /// STDIO available.
    pub stdio_available: bool,
    /// Recommended transport for current data.
    pub recommended: RecommendedTransport,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedTransport {
    pub transport: String,
    pub format: String,
    pub endpoint: String,
    pub reason: String,
}
