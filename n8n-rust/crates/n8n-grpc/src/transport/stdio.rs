//! STDIO transport for CLI and pipe-based communication.
//!
//! This module provides a line-delimited JSON and binary protocol
//! over standard input/output for embedding in CLI tools and scripts.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// Message framing format for STDIO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameFormat {
    /// Line-delimited JSON (newline-terminated).
    #[default]
    JsonLines,
    /// Length-prefixed binary (4-byte big-endian length + payload).
    LengthPrefixed,
    /// Arrow IPC stream format.
    ArrowIpc,
}

impl FrameFormat {
    pub fn from_header(header: &[u8]) -> Self {
        // Detect format from first bytes
        if header.starts_with(b"ARROW") {
            FrameFormat::ArrowIpc
        } else if header.first().map(|b| *b < 0x20 && *b != b'\n' && *b != b'\r').unwrap_or(false) {
            // Likely binary (length prefix starts with non-printable)
            FrameFormat::LengthPrefixed
        } else {
            FrameFormat::JsonLines
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            FrameFormat::JsonLines => "application/x-ndjson",
            FrameFormat::LengthPrefixed => "application/octet-stream",
            FrameFormat::ArrowIpc => "application/vnd.apache.arrow.stream",
        }
    }
}

/// STDIO message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StdioMessage {
    /// Request message.
    Request(StdioRequest),
    /// Response message.
    Response(StdioResponse),
    /// Event/notification.
    Event(StdioEvent),
    /// Error message.
    Error(StdioError),
    /// Protocol negotiation.
    Negotiate(NegotiateMessage),
}

/// Request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioRequest {
    /// Request ID for correlation.
    pub id: String,
    /// Method name (e.g., "workflow.execute", "data.stream").
    pub method: String,
    /// Request parameters.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Hint for preferred response format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_hint: Option<String>,
}

/// Response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioResponse {
    /// Correlated request ID.
    pub id: String,
    /// Response data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Binary payload reference (for Arrow data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_ref: Option<String>,
    /// Actual format used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioEvent {
    /// Event name.
    pub event: String,
    /// Event data.
    pub data: serde_json::Value,
    /// Related request ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Error payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioError {
    /// Correlated request ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Protocol negotiation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiateMessage {
    /// Supported formats (in preference order).
    pub formats: Vec<String>,
    /// Supported transports.
    pub transports: Vec<String>,
    /// Protocol version.
    pub version: String,
    /// Capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl NegotiateMessage {
    pub fn server_capabilities() -> Self {
        Self {
            formats: vec![
                "arrow-ipc".to_string(),
                "arrow-flight".to_string(),
                "protobuf".to_string(),
                "json".to_string(),
                "ndjson".to_string(),
            ],
            transports: vec![
                "stdio".to_string(),
                "grpc".to_string(),
                "rest".to_string(),
                "flight".to_string(),
            ],
            version: "1.0.0".to_string(),
            capabilities: vec![
                "streaming".to_string(),
                "zero-copy".to_string(),
                "hamming-similarity".to_string(),
                "workflow-execution".to_string(),
            ],
        }
    }
}

/// STDIO transport handler.
pub struct StdioTransport {
    format: FrameFormat,
    request_tx: mpsc::Sender<StdioMessage>,
    response_rx: mpsc::Receiver<StdioMessage>,
}

impl StdioTransport {
    /// Create a new STDIO transport with default JSON lines format.
    pub fn new() -> (Self, StdioHandler) {
        Self::with_format(FrameFormat::JsonLines)
    }

    /// Create with specific frame format.
    pub fn with_format(format: FrameFormat) -> (Self, StdioHandler) {
        let (request_tx, request_rx) = mpsc::channel(100);
        let (response_tx, response_rx) = mpsc::channel(100);

        let transport = Self {
            format,
            request_tx,
            response_rx,
        };

        let handler = StdioHandler {
            format,
            request_rx,
            response_tx,
        };

        (transport, handler)
    }

    /// Run the transport, reading from stdin and writing to stdout.
    pub async fn run(mut self) -> io::Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut stdout = stdout;

        // Send initial negotiation
        let negotiate = StdioMessage::Negotiate(NegotiateMessage::server_capabilities());
        self.write_message(&mut stdout, &negotiate).await?;

        let mut line = String::new();
        loop {
            line.clear();

            match self.format {
                FrameFormat::JsonLines => {
                    let n = reader.read_line(&mut line).await?;
                    if n == 0 {
                        break; // EOF
                    }

                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<StdioMessage>(line) {
                        Ok(msg) => {
                            if self.request_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let error = StdioMessage::Error(StdioError {
                                id: None,
                                code: -32700,
                                message: format!("Parse error: {}", e),
                                details: None,
                            });
                            self.write_message(&mut stdout, &error).await?;
                        }
                    }
                }
                FrameFormat::LengthPrefixed => {
                    let mut len_buf = [0u8; 4];
                    if reader.read_exact(&mut len_buf).await.is_err() {
                        break; // EOF
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;

                    let mut buf = vec![0u8; len];
                    reader.read_exact(&mut buf).await?;

                    match serde_json::from_slice::<StdioMessage>(&buf) {
                        Ok(msg) => {
                            if self.request_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let error = StdioMessage::Error(StdioError {
                                id: None,
                                code: -32700,
                                message: format!("Parse error: {}", e),
                                details: None,
                            });
                            self.write_message(&mut stdout, &error).await?;
                        }
                    }
                }
                FrameFormat::ArrowIpc => {
                    // Arrow IPC has its own framing
                    let mut magic = [0u8; 6];
                    if reader.read_exact(&mut magic).await.is_err() {
                        break;
                    }
                    // Handle Arrow IPC continuation...
                    // For now, send unsupported error
                    let error = StdioMessage::Error(StdioError {
                        id: None,
                        code: -32601,
                        message: "Arrow IPC over STDIO not yet implemented".to_string(),
                        details: None,
                    });
                    self.write_message(&mut stdout, &error).await?;
                }
            }

            // Check for responses to send
            while let Ok(response) = self.response_rx.try_recv() {
                self.write_message(&mut stdout, &response).await?;
            }
        }

        Ok(())
    }

    async fn write_message(
        &self,
        writer: &mut tokio::io::Stdout,
        msg: &StdioMessage,
    ) -> io::Result<()> {
        match self.format {
            FrameFormat::JsonLines => {
                let json = serde_json::to_string(msg)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            FrameFormat::LengthPrefixed => {
                let json = serde_json::to_vec(msg)?;
                let len = (json.len() as u32).to_be_bytes();
                writer.write_all(&len).await?;
                writer.write_all(&json).await?;
                writer.flush().await?;
            }
            FrameFormat::ArrowIpc => {
                // Fall back to JSON for non-data messages
                let json = serde_json::to_string(msg)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
        Ok(())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new().0
    }
}

/// Handler for processing STDIO requests.
pub struct StdioHandler {
    format: FrameFormat,
    request_rx: mpsc::Receiver<StdioMessage>,
    response_tx: mpsc::Sender<StdioMessage>,
}

impl StdioHandler {
    /// Receive the next request.
    pub async fn recv(&mut self) -> Option<StdioMessage> {
        self.request_rx.recv().await
    }

    /// Send a response.
    pub async fn send(&self, msg: StdioMessage) -> Result<(), mpsc::error::SendError<StdioMessage>> {
        self.response_tx.send(msg).await
    }

    /// Send a success response.
    pub async fn respond_ok(&self, id: &str, result: serde_json::Value) -> Result<(), mpsc::error::SendError<StdioMessage>> {
        self.send(StdioMessage::Response(StdioResponse {
            id: id.to_string(),
            result: Some(result),
            binary_ref: None,
            format: Some("json".to_string()),
        })).await
    }

    /// Send an error response.
    pub async fn respond_error(&self, id: Option<&str>, code: i32, message: &str) -> Result<(), mpsc::error::SendError<StdioMessage>> {
        self.send(StdioMessage::Error(StdioError {
            id: id.map(String::from),
            code,
            message: message.to_string(),
            details: None,
        })).await
    }

    /// Send an event.
    pub async fn emit_event(&self, event: &str, data: serde_json::Value, request_id: Option<&str>) -> Result<(), mpsc::error::SendError<StdioMessage>> {
        self.send(StdioMessage::Event(StdioEvent {
            event: event.to_string(),
            data,
            request_id: request_id.map(String::from),
        })).await
    }
}

/// Synchronous STDIO for simpler CLI usage.
pub mod sync {
    use super::*;

    /// Read a single message from stdin.
    pub fn read_message() -> io::Result<Option<StdioMessage>> {
        let stdin = io::stdin();
        let mut line = String::new();

        let n = stdin.lock().read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }

        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        serde_json::from_str(line).map(Some).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, e)
        })
    }

    /// Write a message to stdout.
    pub fn write_message(msg: &StdioMessage) -> io::Result<()> {
        let stdout = io::stdout();
        let json = serde_json::to_string(msg)?;

        let mut out = stdout.lock();
        writeln!(out, "{}", json)?;
        out.flush()?;

        Ok(())
    }

    /// Simple request-response over STDIO.
    pub fn call(method: &str, params: serde_json::Value) -> io::Result<serde_json::Value> {
        let id = uuid::Uuid::new_v4().to_string();

        let request = StdioMessage::Request(StdioRequest {
            id: id.clone(),
            method: method.to_string(),
            params,
            format_hint: None,
        });

        write_message(&request)?;

        // Read responses until we get ours
        loop {
            if let Some(msg) = read_message()? {
                match msg {
                    StdioMessage::Response(resp) if resp.id == id => {
                        return resp.result.ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "Empty response")
                        });
                    }
                    StdioMessage::Error(err) if err.id.as_deref() == Some(&id) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("Error {}: {}", err.code, err.message),
                        ));
                    }
                    _ => continue, // Skip other messages
                }
            }
        }
    }
}
