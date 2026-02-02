//! Ada N8N Orchestrator - Rust Implementation
//!
//! A 1:1 transcode of the N8N workflow automation system to Rust.
//!
//! ## Endpoints
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
//! ## Background Tasks
//!
//! - Timer processor (30s interval)
//! - Field warmth loop (30s interval)

mod clients;
mod config;
mod handlers;
mod redis;
mod tasks;
mod types;

use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
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
    info!("Binding to: {}", bind_addr);

    // Create shared state
    let state = AppState::new(config);

    // Build router with all webhook endpoints
    let app = Router::new()
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
        .route("/healthz", get(health_handler))
        // Add CORS support
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        // Add request tracing
        .layer(TraceLayer::new_for_http())
        // Add shared state
        .with_state(state.clone());

    // Start background tasks
    let timer_state = state.clone();
    tokio::spawn(async move {
        start_timer_processor(timer_state).await;
    });

    let field_state = state.clone();
    tokio::spawn(async move {
        start_field_loop(field_state).await;
    });

    info!("Background tasks started (timer processor, field loop)");

    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("Server listening on {}", bind_addr);

    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}
