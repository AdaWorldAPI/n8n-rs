//! Ada N8N Orchestrator - Rust Implementation
//!
//! A high-performance orchestrator for Ada field operations.
//!
//! ## Protocols
//!
//! - **HTTP/REST** (default) - Port 8080
//! - **gRPC/Arrow Flight** (with `flight` feature) - Port 50051
//!
//! ## HTTP Endpoints
//!
//! - `POST /webhook/lego` - Execute lego template actions
//! - `POST /webhook/propagate` - Propagate touch to neighbors
//! - `GET /webhook/field-status` - Get combined field status
//! - `POST /webhook/timer` - Create timer
//! - `GET /webhook/timer/:id` - Get timer
//! - `DELETE /webhook/timer/:id` - Cancel timer
//! - `POST /webhook/chat` - Chat with Ada via xAI
//! - `GET /healthz` - Health check
//!
//! ## Vector Endpoints (with `lance` feature)
//!
//! - `POST /webhook/vectors/upsert` - Store embeddings
//! - `POST /webhook/vectors/search` - Semantic search
//!
//! ## Background Tasks
//!
//! - Timer processor (30s interval)
//! - Field warmth loop (30s interval)
//!
//! ## Build Options
//!
//! ```bash
//! # Default (HTTP only)
//! cargo build --release
//!
//! # With gRPC/Arrow Flight
//! cargo build --release --features flight
//!
//! # With LanceDB vectors
//! cargo build --release --features lance
//!
//! # All features
//! cargo build --release --features full
//! ```

mod clients;
mod config;
mod handlers;
mod redis;
mod tasks;
mod types;

// Optional modules
#[cfg(feature = "flight")]
mod flight;
#[cfg(feature = "lance")]
mod lance;

use axum::{
    routing::{delete, get, post},
    Router,
};
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
#[cfg(feature = "flight")]
use tracing::error;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{AppState, Config};
use crate::handlers::*;
use crate::tasks::{start_field_loop, start_timer_processor};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ada_n8n=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let bind_addr = config.bind_addr();

    info!("Starting Ada N8N Orchestrator");
    info!("MCP URL: {}", config.mcp_url);
    info!("Point URL: {}", config.point_url);
    info!("xAI URL: {}", config.xai_url);
    info!("HTTP binding to: {}", bind_addr);

    // Log enabled features
    #[cfg(feature = "flight")]
    info!("Arrow Flight/gRPC: ENABLED");
    #[cfg(not(feature = "flight"))]
    info!("Arrow Flight/gRPC: disabled (enable with --features flight)");

    #[cfg(feature = "lance")]
    info!("LanceDB vectors: ENABLED");
    #[cfg(not(feature = "lance"))]
    info!("LanceDB vectors: disabled (enable with --features lance)");

    // Validate critical config
    if config.redis_url.is_empty() {
        warn!("UPSTASH_REDIS_REST_URL not set - timer and chat features will fail");
    }
    if config.xai_key.is_empty() {
        warn!("ADA_XAI_KEY not set - chat and field loop features will fail");
    }

    // Create shared state
    let state = AppState::new(config);

    // Build router with all webhook endpoints
    #[allow(unused_mut)]
    let mut app = Router::new()
        // Lego executor
        .route("/webhook/lego", post(lego_handler))
        // Propagate touch
        .route("/webhook/propagate", post(propagate_handler))
        // Field monitor
        .route("/webhook/field-status", get(field_status_handler))
        // Timer API
        .route("/webhook/timer", post(create_timer_handler))
        .route("/webhook/timer/:id", get(get_timer_handler))
        .route("/webhook/timer/:id", delete(cancel_timer_handler))
        // Chat
        .route("/webhook/chat", post(chat_handler))
        // Health check
        .route("/healthz", get(health_handler));

    // Add vector endpoints if lance feature is enabled
    #[cfg(feature = "lance")]
    {
        app = app
            .route("/webhook/vectors/upsert", post(lance::vector_upsert_handler))
            .route("/webhook/vectors/search", post(lance::vector_search_handler));
    }

    let app = app
        // Add CORS support
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        // Add request tracing
        .layer(TraceLayer::new_for_http())
        // Add shared state
        .with_state(state.clone());

    // Start background tasks
    let timer_state = state.clone();
    let timer_handle = tokio::spawn(async move {
        start_timer_processor(timer_state).await;
    });

    let field_state = state.clone();
    let field_handle = tokio::spawn(async move {
        start_field_loop(field_state).await;
    });

    info!("Background tasks started (timer processor, field loop)");

    // Start gRPC server if flight feature is enabled
    #[cfg(feature = "flight")]
    let grpc_handle = {
        let grpc_port: u16 = std::env::var("GRPC_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50051);

        let grpc_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = flight::start_grpc_server(grpc_state, grpc_port).await {
                error!("gRPC server error: {}", e);
            }
        })
    };

    // Start HTTP server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("HTTP server listening on {}", bind_addr);

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server failed to start");

    // Shutdown sequence
    info!("Shutdown signal received, stopping background tasks...");

    // Abort background tasks
    timer_handle.abort();
    field_handle.abort();

    #[cfg(feature = "flight")]
    grpc_handle.abort();

    info!("Ada N8N Orchestrator shutdown complete");
}

/// Wait for shutdown signal (SIGINT or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
