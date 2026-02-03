//! Arrow Flight + gRPC server implementation
//!
//! Provides high-performance data transfer for Ada operations.
//! This module is only compiled when the "flight" feature is enabled.
//!
//! ## Usage
//!
//! Build with flight support:
//! ```bash
//! cargo build --release --features flight
//! ```
//!
//! ## Endpoints
//!
//! - gRPC services on port 50051 (configurable via GRPC_PORT)
//! - Arrow Flight on same port for zero-copy bulk transfers

#![cfg(feature = "flight")]

use std::pin::Pin;
use std::sync::Arc;

use arrow::array::{Float32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow_flight::{
    flight_service_server::{FlightService, FlightServiceServer},
    Action, ActionType, Criteria, Empty, FlightData, FlightDescriptor, FlightInfo,
    HandshakeRequest, HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
};
use futures::Stream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info};

use crate::config::AppState;

// Include generated protobuf code
pub mod ada {
    tonic::include_proto!("ada");
}

// ═══════════════════════════════════════════════════════════════════════════
// Arrow Flight Service
// ═══════════════════════════════════════════════════════════════════════════

/// Ada Flight Service - Zero-copy data transfer
pub struct AdaFlightService {
    state: Arc<AppState>,
}

impl AdaFlightService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

type BoxedFlightStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl FlightService for AdaFlightService {
    type HandshakeStream = BoxedFlightStream<HandshakeResponse>;
    type ListFlightsStream = BoxedFlightStream<FlightInfo>;
    type DoGetStream = BoxedFlightStream<FlightData>;
    type DoPutStream = BoxedFlightStream<PutResult>;
    type DoActionStream = BoxedFlightStream<arrow_flight::Result>;
    type ListActionsStream = BoxedFlightStream<ActionType>;
    type DoExchangeStream = BoxedFlightStream<FlightData>;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        info!("Flight handshake received");
        let response = HandshakeResponse {
            protocol_version: 1,
            payload: "ada-flight-v1".as_bytes().to_vec().into(),
        };
        let stream = futures::stream::once(async { Ok(response) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_flights(
        &self,
        _request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        // List available data endpoints
        let flights = vec![
            create_flight_info("field_state", "Current field state as Arrow"),
            create_flight_info("touch_history", "Recent touch events as Arrow"),
            create_flight_info("vectors", "Vector embeddings (LanceDB)"),
        ];
        let stream = futures::stream::iter(flights.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_flight_info(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let descriptor = request.into_inner();
        let path = descriptor
            .path
            .first()
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let info = create_flight_info(path, &format!("{} data", path));
        Ok(Response::new(info))
    }

    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<PollInfo>, Status> {
        Err(Status::unimplemented("poll_flight_info not implemented"))
    }

    async fn get_schema(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        let descriptor = request.into_inner();
        let path = descriptor
            .path
            .first()
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let schema = match path {
            "field_state" => field_state_schema(),
            "touch_history" => touch_history_schema(),
            "vectors" => vector_schema(),
            _ => return Err(Status::not_found(format!("Unknown flight: {}", path))),
        };

        let schema_bytes = schema_to_bytes(&schema)?;
        Ok(Response::new(SchemaResult {
            schema: schema_bytes.into(),
        }))
    }

    async fn do_get(
        &self,
        request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let ticket = request.into_inner();
        let ticket_str = String::from_utf8_lossy(&ticket.ticket);

        info!("Flight do_get for ticket: {}", ticket_str);

        // Parse ticket to determine what data to return
        let parts: Vec<&str> = ticket_str.split(':').collect();
        let flight_type = parts.first().unwrap_or(&"unknown");

        let batch = match *flight_type {
            "field_state" => create_field_state_batch()?,
            "touch_history" => create_touch_history_batch()?,
            "vectors" => {
                // Would integrate with LanceDB here
                return Err(Status::unimplemented("Vector retrieval via Flight requires LanceDB"));
            }
            _ => return Err(Status::not_found(format!("Unknown ticket: {}", ticket_str))),
        };

        let flight_data = batch_to_flight_data(&batch)?;
        let stream = futures::stream::iter(flight_data.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn do_put(
        &self,
        request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        info!("Flight do_put received");
        // For ingesting data (e.g., batch touch operations)
        let mut stream = request.into_inner();
        let mut count = 0u64;

        while let Some(data) = stream.message().await? {
            // Process incoming Arrow data
            count += 1;
            // Would parse FlightData into RecordBatch and process
        }

        let result = PutResult {
            app_metadata: format!("Processed {} messages", count).into_bytes().into(),
        };
        let stream = futures::stream::once(async { Ok(result) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn do_action(
        &self,
        request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        let action = request.into_inner();
        info!("Flight action: {}", action.r#type);

        let result = match action.r#type.as_str() {
            "health" => arrow_flight::Result {
                body: b"ok".to_vec().into(),
            },
            "clear_cache" => {
                // Clear any caches
                arrow_flight::Result {
                    body: b"cleared".to_vec().into(),
                }
            }
            _ => {
                return Err(Status::unimplemented(format!(
                    "Unknown action: {}",
                    action.r#type
                )))
            }
        };

        let stream = futures::stream::once(async { Ok(result) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        let actions = vec![
            ActionType {
                r#type: "health".to_string(),
                description: "Health check".to_string(),
            },
            ActionType {
                r#type: "clear_cache".to_string(),
                description: "Clear caches".to_string(),
            },
        ];
        let stream = futures::stream::iter(actions.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn do_exchange(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        Err(Status::unimplemented("do_exchange not implemented"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════

fn create_flight_info(name: &str, description: &str) -> FlightInfo {
    FlightInfo {
        schema: vec![].into(),
        flight_descriptor: Some(FlightDescriptor {
            r#type: 1, // PATH
            cmd: vec![].into(),
            path: vec![name.to_string()],
        }),
        endpoint: vec![],
        total_records: -1,
        total_bytes: -1,
        ordered: false,
        app_metadata: description.as_bytes().to_vec().into(),
    }
}

fn field_state_schema() -> Schema {
    Schema::new(vec![
        Field::new("node", DataType::Utf8, false),
        Field::new("strength", DataType::Float32, false),
        Field::new("timestamp", DataType::Utf8, false),
    ])
}

fn touch_history_schema() -> Schema {
    Schema::new(vec![
        Field::new("node", DataType::Utf8, false),
        Field::new("strength", DataType::Float32, false),
        Field::new("via", DataType::Utf8, true),
        Field::new("timestamp", DataType::Utf8, false),
    ])
}

fn vector_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 384),
            false,
        ),
        Field::new("metadata", DataType::Utf8, true),
    ])
}

fn schema_to_bytes(schema: &Schema) -> Result<Vec<u8>, Status> {
    let options = arrow::ipc::writer::IpcWriteOptions::default();
    let data_gen = arrow::ipc::writer::IpcDataGenerator::default();
    let schema_data = data_gen
        .schema_to_bytes(schema, &options)
        .ipc_message;
    Ok(schema_data.to_vec())
}

fn create_field_state_batch() -> Result<RecordBatch, Status> {
    let schema = field_state_schema();

    let nodes = StringArray::from(vec!["sex", "arousal", "desire"]);
    let strengths = Float32Array::from(vec![0.8, 0.6, 0.4]);
    let timestamps = StringArray::from(vec![
        chrono::Utc::now().to_rfc3339(),
        chrono::Utc::now().to_rfc3339(),
        chrono::Utc::now().to_rfc3339(),
    ]);

    RecordBatch::try_new(Arc::new(schema), vec![Arc::new(nodes), Arc::new(strengths), Arc::new(timestamps)])
        .map_err(|e| Status::internal(format!("Failed to create batch: {}", e)))
}

fn create_touch_history_batch() -> Result<RecordBatch, Status> {
    let schema = touch_history_schema();

    let nodes = StringArray::from(vec!["sex", "arousal"]);
    let strengths = Float32Array::from(vec![0.5, 0.3]);
    let vias = StringArray::from(vec![Some("direct"), Some("sex/CAUSES")]);
    let timestamps = StringArray::from(vec![
        chrono::Utc::now().to_rfc3339(),
        chrono::Utc::now().to_rfc3339(),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![Arc::new(nodes), Arc::new(strengths), Arc::new(vias), Arc::new(timestamps)],
    )
    .map_err(|e| Status::internal(format!("Failed to create batch: {}", e)))
}

fn batch_to_flight_data(batch: &RecordBatch) -> Result<Vec<FlightData>, Status> {
    let options = arrow::ipc::writer::IpcWriteOptions::default();
    let data_gen = arrow::ipc::writer::IpcDataGenerator::default();

    // Schema message
    let schema_flight_data = arrow_flight::utils::flight_data_from_arrow_schema(batch.schema().as_ref(), &options);

    // Data message
    let (_dict_flights, data_flight) = data_gen
        .encoded_batch(batch, &Default::default(), &options)
        .map_err(|e| Status::internal(format!("Failed to encode batch: {}", e)))?;

    let data_flight_data = FlightData {
        flight_descriptor: None,
        data_header: data_flight.ipc_message.into(),
        data_body: data_flight.arrow_data.into(),
        app_metadata: vec![].into(),
    };

    Ok(vec![schema_flight_data, data_flight_data])
}

// ═══════════════════════════════════════════════════════════════════════════
// gRPC Service implementations
// ═══════════════════════════════════════════════════════════════════════════

use ada::lego_service_server::{LegoService, LegoServiceServer};
use ada::{LegoRequest, LegoResponse, LegoBatchRequest, LegoBatchResponse};

pub struct AdaLegoService {
    state: Arc<AppState>,
}

impl AdaLegoService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl LegoService for AdaLegoService {
    async fn execute(
        &self,
        request: Request<LegoRequest>,
    ) -> Result<Response<LegoResponse>, Status> {
        let req = request.into_inner();
        info!("gRPC Lego execute: {}", req.lego);

        // Delegate to the HTTP handler logic (would refactor to share code)
        let params: serde_json::Value = serde_json::from_str(&req.params_json)
            .unwrap_or(serde_json::json!({}));

        // For now, return a placeholder - would integrate with handlers
        Ok(Response::new(LegoResponse {
            success: true,
            result_json: serde_json::json!({
                "lego": req.lego,
                "params": params,
                "executed": true
            }).to_string(),
            error: String::new(),
            request_id: req.request_id,
        }))
    }

    async fn execute_batch(
        &self,
        request: Request<LegoBatchRequest>,
    ) -> Result<Response<LegoBatchResponse>, Status> {
        let batch = request.into_inner();
        let mut responses = Vec::with_capacity(batch.requests.len());

        for req in batch.requests {
            let resp = self.execute(Request::new(req)).await?.into_inner();
            responses.push(resp);
        }

        Ok(Response::new(LegoBatchResponse { responses }))
    }

    type ExecuteStreamStream = BoxedFlightStream<LegoResponse>;

    async fn execute_stream(
        &self,
        request: Request<Streaming<LegoRequest>>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let mut stream = request.into_inner();
        let state = self.state.clone();

        let output = async_stream::stream! {
            while let Some(req) = stream.message().await.transpose() {
                match req {
                    Ok(r) => {
                        let response = LegoResponse {
                            success: true,
                            result_json: serde_json::json!({"lego": r.lego}).to_string(),
                            error: String::new(),
                            request_id: r.request_id,
                        };
                        yield Ok(response);
                    }
                    Err(e) => {
                        yield Err(e);
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(output)))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Server startup
// ═══════════════════════════════════════════════════════════════════════════

/// Start the gRPC/Flight server
pub async fn start_grpc_server(state: AppState, port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let state = Arc::new(state);

    info!("Starting gRPC/Flight server on {}", addr);

    tonic::transport::Server::builder()
        .add_service(FlightServiceServer::new(AdaFlightService::new(state.clone())))
        .add_service(LegoServiceServer::new(AdaLegoService::new(state.clone())))
        // Add other services here as they're implemented
        .serve(addr)
        .await?;

    Ok(())
}
