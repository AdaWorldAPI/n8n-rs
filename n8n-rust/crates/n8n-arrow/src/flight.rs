//! Arrow Flight integration for high-performance data transfer.
//!
//! Arrow Flight provides a gRPC-based protocol specifically designed for
//! bulk data transfer with minimal overhead.

use crate::error::ArrowError;
use arrow_array::RecordBatch;
use arrow_flight::{
    flight_service_server::FlightService, Action, ActionType, Criteria, Empty,
    FlightData, FlightDescriptor, FlightEndpoint, FlightInfo, HandshakeRequest,
    HandshakeResponse, PutResult, SchemaAsIpc, SchemaResult, Ticket,
};
use arrow_schema::Schema;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use std::sync::Arc;
use tonic::{Request, Response, Status, Streaming};

/// Convert a RecordBatch to FlightData for streaming.
pub fn batch_to_flight_data(batch: &RecordBatch) -> Result<Vec<FlightData>, ArrowError> {
    use arrow_flight::encode::FlightDataEncoderBuilder;

    let schema = batch.schema();
    let encoder = FlightDataEncoderBuilder::new()
        .with_schema(schema)
        .build(futures::stream::iter(vec![Ok(batch.clone())]));

    // Collect synchronously by blocking
    let data: Vec<FlightData> = futures::executor::block_on(async {
        encoder
            .map(|r| r.map_err(|e| ArrowError::FlightError(e.to_string())))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect()
    });

    Ok(data)
}

/// Convert FlightData stream to RecordBatches.
pub async fn flight_data_to_batches<S>(stream: S) -> Result<Vec<RecordBatch>, ArrowError>
where
    S: Stream<Item = Result<FlightData, Status>> + Unpin + Send + 'static,
{
    use arrow_flight::decode::FlightRecordBatchStream;
    use arrow_flight::error::FlightError;

    let flight_stream = stream.map(|r: Result<FlightData, Status>| {
        r.map_err(|e: Status| FlightError::Tonic(Box::new(e)))
    });

    let mut decoder = FlightRecordBatchStream::new_from_flight_data(flight_stream);

    let mut batches = Vec::new();
    while let Some(batch_result) = decoder.next().await {
        match batch_result {
            Ok(batch) => batches.push(batch),
            Err(e) => return Err(ArrowError::FlightError(e.to_string())),
        }
    }

    Ok(batches)
}

/// Trait for implementing workflow data as a Flight service.
#[async_trait]
pub trait WorkflowFlightService: Send + Sync {
    /// Get execution data as RecordBatches.
    async fn get_execution_data(
        &self,
        execution_id: &str,
        node_name: Option<&str>,
    ) -> Result<Vec<RecordBatch>, ArrowError>;

    /// Stream execution data.
    async fn stream_execution_data(
        &self,
        execution_id: &str,
    ) -> Result<BoxStream<'static, Result<RecordBatch, ArrowError>>, ArrowError>;

    /// Store execution data from stream.
    async fn put_execution_data(
        &self,
        execution_id: &str,
        batches: Vec<RecordBatch>,
    ) -> Result<u64, ArrowError>;
}

/// Flight service implementation for n8n workflow data.
pub struct N8nFlightService<T: WorkflowFlightService> {
    inner: Arc<T>,
}

impl<T: WorkflowFlightService> N8nFlightService<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

#[async_trait]
impl<T: WorkflowFlightService + 'static> FlightService for N8nFlightService<T> {
    type HandshakeStream = BoxStream<'static, Result<HandshakeResponse, Status>>;
    type ListFlightsStream = BoxStream<'static, Result<FlightInfo, Status>>;
    type DoGetStream = BoxStream<'static, Result<FlightData, Status>>;
    type DoPutStream = BoxStream<'static, Result<PutResult, Status>>;
    type DoActionStream = BoxStream<'static, Result<arrow_flight::Result, Status>>;
    type ListActionsStream = BoxStream<'static, Result<ActionType, Status>>;
    type DoExchangeStream = BoxStream<'static, Result<FlightData, Status>>;

    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        let response = HandshakeResponse {
            protocol_version: 1,
            payload: bytes::Bytes::new(),
        };
        let stream = futures::stream::once(async { Ok(response) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_flights(
        &self,
        _request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        // List available execution data
        let stream = futures::stream::empty();
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_flight_info(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let descriptor = request.into_inner();

        // Parse the path to get execution_id
        let path = String::from_utf8(descriptor.path.first().cloned().unwrap_or_default().into())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let batches = self
            .inner
            .get_execution_data(&path, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let schema = if !batches.is_empty() {
            batches[0].schema()
        } else {
            Arc::new(Schema::empty())
        };

        let total_records: usize = batches.iter().map(|b: &RecordBatch| b.num_rows()).sum();
        let total_bytes: i64 = batches
            .iter()
            .map(|b: &RecordBatch| b.get_array_memory_size() as i64)
            .sum();

        let info = FlightInfo::new()
            .try_with_schema(&schema)
            .map_err(|e| Status::internal(e.to_string()))?
            .with_descriptor(descriptor)
            .with_endpoint(FlightEndpoint::new().with_ticket(Ticket::new(path.clone())))
            .with_total_records(total_records as i64)
            .with_total_bytes(total_bytes);

        Ok(Response::new(info))
    }

    async fn get_schema(
        &self,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        let descriptor = request.into_inner();
        let path = String::from_utf8(descriptor.path.first().cloned().unwrap_or_default().into())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let batches = self
            .inner
            .get_execution_data(&path, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let schema = if !batches.is_empty() {
            batches[0].schema()
        } else {
            Arc::new(Schema::empty())
        };

        let options = crate::ipc::aligned_ipc_options();
        let schema_as_ipc = SchemaAsIpc::new(&schema, &options);
        let result = SchemaResult::try_from(schema_as_ipc)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(result))
    }

    async fn do_get(
        &self,
        request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let ticket = request.into_inner();
        let execution_id = String::from_utf8(ticket.ticket.to_vec())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let inner = self.inner.clone();
        let stream = async_stream::try_stream! {
            let batches = inner
                .get_execution_data(&execution_id, None)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            for batch in batches {
                let flight_data = batch_to_flight_data(&batch)
                    .map_err(|e| Status::internal(e.to_string()))?;
                for fd in flight_data {
                    yield fd;
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn do_put(
        &self,
        request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        let stream = request.into_inner();

        // Collect flight data into a vector first
        let flight_data: Vec<FlightData> = stream
            .try_collect()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Decode the flight data into record batches
        let batches = if !flight_data.is_empty() {
            use arrow_flight::decode::FlightRecordBatchStream;
            use arrow_flight::error::FlightError;

            let data_stream = futures::stream::iter(
                flight_data.into_iter().map(Ok::<_, FlightError>)
            );
            let mut decoder = FlightRecordBatchStream::new_from_flight_data(data_stream);

            let mut batches = Vec::new();
            while let Some(batch_result) = decoder.next().await {
                match batch_result {
                    Ok(batch) => batches.push(batch),
                    Err(e) => return Err(Status::internal(e.to_string())),
                }
            }
            batches
        } else {
            Vec::new()
        };

        // For now, just acknowledge receipt
        let result = PutResult {
            app_metadata: bytes::Bytes::from(format!("Received {} batches", batches.len())),
        };

        let stream = futures::stream::once(async { Ok(result) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn do_action(
        &self,
        _request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        let stream = futures::stream::empty();
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        let actions = vec![
            ActionType {
                r#type: "clear_cache".to_string(),
                description: "Clear execution data cache".to_string(),
            },
            ActionType {
                r#type: "get_stats".to_string(),
                description: "Get execution statistics".to_string(),
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

    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<arrow_flight::PollInfo>, Status> {
        Err(Status::unimplemented("poll_flight_info not implemented"))
    }
}
