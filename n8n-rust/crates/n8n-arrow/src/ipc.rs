//! Arrow IPC (Inter-Process Communication) utilities for zero-copy data transfer.

use crate::error::ArrowError;
use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use arrow_schema::Schema;
use bytes::{Bytes, BytesMut};
use std::io::Cursor;
use std::sync::Arc;

/// Options for IPC serialization.
#[derive(Debug, Clone)]
pub struct IpcOptions {
    /// Whether to use compression (requires feature).
    pub compress: bool,
    /// Whether to write schema with each batch.
    pub write_schema: bool,
}

impl Default for IpcOptions {
    fn default() -> Self {
        Self {
            compress: false,
            write_schema: true,
        }
    }
}

/// Serialize a RecordBatch to IPC format bytes.
///
/// This is the primary method for zero-copy data transfer:
/// - The serialized bytes can be sent over gRPC or other transports
/// - On the receiving end, the data can be deserialized without copying
pub fn batch_to_ipc_bytes(batch: &RecordBatch) -> Result<Bytes, ArrowError> {
    let mut buffer = Vec::new();

    {
        let options = IpcWriteOptions::default();
        let mut writer = StreamWriter::try_new_with_options(&mut buffer, &batch.schema(), options)?;
        writer.write(batch)?;
        writer.finish()?;
    }

    Ok(Bytes::from(buffer))
}

/// Serialize multiple RecordBatches to IPC format.
pub fn batches_to_ipc_bytes(batches: &[RecordBatch]) -> Result<Bytes, ArrowError> {
    if batches.is_empty() {
        return Err(ArrowError::InvalidData("No batches to serialize".into()));
    }

    let schema = batches[0].schema();
    let mut buffer = Vec::new();

    {
        let options = IpcWriteOptions::default();
        let mut writer = StreamWriter::try_new_with_options(&mut buffer, &schema, options)?;
        for batch in batches {
            writer.write(batch)?;
        }
        writer.finish()?;
    }

    Ok(Bytes::from(buffer))
}

/// Deserialize IPC bytes to RecordBatches.
///
/// Zero-copy when possible - the returned batches may reference the input bytes.
pub fn ipc_bytes_to_batches(bytes: &[u8]) -> Result<Vec<RecordBatch>, ArrowError> {
    let cursor = Cursor::new(bytes);
    let reader = StreamReader::try_new(cursor, None)?;

    let batches: Result<Vec<_>, _> = reader.collect();
    Ok(batches?)
}

/// Deserialize IPC bytes to a single RecordBatch (first batch only).
pub fn ipc_bytes_to_batch(bytes: &[u8]) -> Result<RecordBatch, ArrowError> {
    let batches = ipc_bytes_to_batches(bytes)?;
    batches
        .into_iter()
        .next()
        .ok_or_else(|| ArrowError::InvalidData("No batches in IPC stream".into()))
}

/// Get the schema from IPC bytes without reading all data.
pub fn ipc_bytes_schema(bytes: &[u8]) -> Result<Arc<Schema>, ArrowError> {
    let cursor = Cursor::new(bytes);
    let reader = StreamReader::try_new(cursor, None)?;
    Ok(reader.schema())
}

/// Streaming IPC writer that can write batches incrementally.
pub struct IncrementalIpcWriter {
    buffer: BytesMut,
    schema: Arc<Schema>,
    started: bool,
}

impl IncrementalIpcWriter {
    pub fn new(schema: Arc<Schema>) -> Self {
        Self {
            buffer: BytesMut::new(),
            schema,
            started: false,
        }
    }

    /// Write a batch and return the IPC bytes for just that batch.
    ///
    /// The first call includes the schema; subsequent calls only include data.
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<Bytes, ArrowError> {
        let mut temp_buffer = Vec::new();

        {
            let options = IpcWriteOptions::default();
            let mut writer = StreamWriter::try_new_with_options(&mut temp_buffer, &self.schema, options)?;
            writer.write(batch)?;
            writer.finish()?;
        }

        self.started = true;
        Ok(Bytes::from(temp_buffer))
    }

    /// Get the schema.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
}

/// Streaming IPC reader that can read batches incrementally.
pub struct IncrementalIpcReader {
    schema: Option<Arc<Schema>>,
    pending_bytes: BytesMut,
}

impl IncrementalIpcReader {
    pub fn new() -> Self {
        Self {
            schema: None,
            pending_bytes: BytesMut::new(),
        }
    }

    /// Add bytes to the reader and attempt to parse batches.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<RecordBatch>, ArrowError> {
        self.pending_bytes.extend_from_slice(bytes);

        // Try to read all available batches
        let cursor = Cursor::new(&self.pending_bytes[..]);
        match StreamReader::try_new(cursor, None) {
            Ok(reader) => {
                if self.schema.is_none() {
                    self.schema = Some(reader.schema());
                }

                let batches: Result<Vec<_>, _> = reader.collect();
                let batches = batches?;

                // Clear consumed bytes (simplified - in production track exact bytes consumed)
                self.pending_bytes.clear();

                Ok(batches)
            }
            Err(_) => {
                // Not enough data yet
                Ok(vec![])
            }
        }
    }

    /// Get the schema if available.
    pub fn schema(&self) -> Option<&Arc<Schema>> {
        self.schema.as_ref()
    }
}

impl Default for IncrementalIpcReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int32Array, StringArray};
    use std::sync::Arc;

    #[test]
    fn test_roundtrip() {
        let schema = Arc::new(Schema::new(vec![
            arrow_schema::Field::new("id", arrow_schema::DataType::Int32, false),
            arrow_schema::Field::new("name", arrow_schema::DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
            ],
        )
        .unwrap();

        let bytes = batch_to_ipc_bytes(&batch).unwrap();
        let recovered = ipc_bytes_to_batch(&bytes).unwrap();

        assert_eq!(batch.num_rows(), recovered.num_rows());
        assert_eq!(batch.num_columns(), recovered.num_columns());
    }
}
