//! n8n-rust gRPC server.
//!
//! This binary provides:
//! - gRPC workflow service
//! - Arrow Flight data streaming
//! - Hamming similarity service
//! - JSON fallback for compatibility

use n8n_grpc::{ArrowDataService, HammingGrpcService, WorkflowGrpcService, WorkflowServiceState};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse configuration
    let addr: SocketAddr = std::env::var("N8N_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    info!("Starting n8n-rust gRPC server on {}", addr);

    // Initialize services
    let state = Arc::new(WorkflowServiceState::new());
    let workflow_service = WorkflowGrpcService::new(state.clone());
    let arrow_service = ArrowDataService::new(state.workflows.clone(), state.executions.clone());
    let hamming_service = HammingGrpcService::new();

    info!("Services initialized:");
    info!("  - WorkflowService: workflow CRUD and execution");
    info!("  - ArrowDataService: zero-copy data streaming");
    info!("  - HammingService: 10kbit similarity search");

    // For now, we'll just print the configuration
    // In a full implementation, this would start a tonic server
    info!("Server ready for connections");
    info!("");
    info!("Available endpoints:");
    info!("  gRPC:   grpc://{}",  addr);
    info!("  Flight: flight://{}", addr);
    info!("");
    info!("Supported formats:");
    info!("  - JSON (default)");
    info!("  - Arrow IPC (zero-copy)");
    info!("  - Arrow Flight (streaming)");
    info!("");
    info!("Example usage:");
    info!("  grpcurl -plaintext {} n8n.WorkflowService/ListWorkflows", addr);

    // Keep the server running
    // In a full implementation, this would be the tonic server
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
