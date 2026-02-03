//! LanceDB integration for zero-copy vector storage
//!
//! Provides embedded vector database functionality for Ada's semantic operations.
//! This module is only compiled when the "lance" feature is enabled.
//!
//! ## Usage
//!
//! Build with LanceDB support:
//! ```bash
//! cargo build --release --features lance
//! ```
//!
//! ## Features
//!
//! - Zero-copy Arrow integration
//! - Semantic search for node embeddings
//! - Efficient batch upserts
//! - SQL-like filtering

#![cfg(feature = "lance")]

use std::sync::Arc;

use arrow::array::{Array, FixedSizeListArray, Float32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use anyhow::{Context, Result};
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

/// LanceDB client wrapper
pub struct VectorStore {
    db: lancedb::Connection,
    embedding_dim: i32,
}

impl VectorStore {
    /// Create or connect to a LanceDB instance
    ///
    /// # Arguments
    /// * `path` - Database path (local directory or S3 URI)
    /// * `embedding_dim` - Dimension of embedding vectors (default: 384)
    pub async fn new(path: &str, embedding_dim: i32) -> Result<Self> {
        info!("Connecting to LanceDB at: {}", path);

        let db = connect(path)
            .execute()
            .await
            .context("Failed to connect to LanceDB")?;

        Ok(Self { db, embedding_dim })
    }

    /// Get or create a table for storing vectors
    pub async fn ensure_table(&self, name: &str) -> Result<lancedb::Table> {
        let tables = self.db.table_names().execute().await?;

        if tables.contains(&name.to_string()) {
            info!("Opening existing table: {}", name);
            self.db.open_table(name).execute().await.context("Failed to open table")
        } else {
            info!("Creating new table: {}", name);
            let schema = self.vector_schema();
            let empty_batch = self.empty_batch(&schema)?;

            self.db
                .create_table(name, Box::new(std::iter::once(Ok(empty_batch))))
                .execute()
                .await
                .context("Failed to create table")
        }
    }

    /// Vector table schema
    fn vector_schema(&self) -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, false)),
                    self.embedding_dim,
                ),
                false,
            ),
            Field::new("node", DataType::Utf8, true),
            Field::new("content", DataType::Utf8, true),
            Field::new("metadata", DataType::Utf8, true),
        ])
    }

    /// Create empty batch with schema
    fn empty_batch(&self, schema: &Schema) -> Result<RecordBatch> {
        let ids = StringArray::from(Vec::<String>::new());
        let vectors = create_empty_vector_array(self.embedding_dim);
        let nodes = StringArray::from(Vec::<Option<String>>::new());
        let contents = StringArray::from(Vec::<Option<String>>::new());
        let metadata = StringArray::from(Vec::<Option<String>>::new());

        RecordBatch::try_new(
            Arc::new(schema.clone()),
            vec![
                Arc::new(ids),
                Arc::new(vectors),
                Arc::new(nodes),
                Arc::new(contents),
                Arc::new(metadata),
            ],
        )
        .context("Failed to create empty batch")
    }

    /// Upsert vectors into a table
    ///
    /// # Arguments
    /// * `table_name` - Target table
    /// * `records` - Vector records to upsert
    pub async fn upsert(&self, table_name: &str, records: Vec<VectorRecord>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }

        let table = self.ensure_table(table_name).await?;
        let batch = self.records_to_batch(&records)?;

        table
            .add(Box::new(std::iter::once(Ok(batch))))
            .execute()
            .await
            .context("Failed to upsert vectors")?;

        info!("Upserted {} vectors to {}", records.len(), table_name);
        Ok(records.len())
    }

    /// Search for similar vectors
    ///
    /// # Arguments
    /// * `table_name` - Table to search
    /// * `query_vector` - Query embedding
    /// * `limit` - Maximum results
    /// * `filter` - Optional SQL filter
    pub async fn search(
        &self,
        table_name: &str,
        query_vector: Vec<f32>,
        limit: usize,
        filter: Option<&str>,
    ) -> Result<Vec<VectorMatch>> {
        let table = self.db.open_table(table_name).execute().await?;

        let mut query = table
            .vector_search(query_vector)
            .context("Failed to create vector search")?
            .limit(limit);

        if let Some(f) = filter {
            query = query.only_if(f);
        }

        let results = query
            .execute()
            .await
            .context("Failed to execute search")?
            .try_collect::<Vec<_>>()
            .await
            .context("Failed to collect results")?;

        let mut matches = Vec::new();
        for batch in results {
            matches.extend(self.batch_to_matches(&batch)?);
        }

        Ok(matches)
    }

    /// Delete vectors by ID
    pub async fn delete(&self, table_name: &str, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let table = self.db.open_table(table_name).execute().await?;

        let id_list = ids
            .iter()
            .map(|id| format!("'{}'", id.replace("'", "''")))
            .collect::<Vec<_>>()
            .join(", ");

        let filter = format!("id IN ({})", id_list);

        table
            .delete(&filter)
            .await
            .context("Failed to delete vectors")?;

        info!("Deleted {} vectors from {}", ids.len(), table_name);
        Ok(ids.len())
    }

    /// Convert records to Arrow batch
    fn records_to_batch(&self, records: &[VectorRecord]) -> Result<RecordBatch> {
        let schema = self.vector_schema();

        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let ids_array = StringArray::from(ids);

        let vectors_array = create_vector_array(
            records.iter().map(|r| r.vector.clone()).collect(),
            self.embedding_dim,
        )?;

        let nodes: Vec<Option<&str>> = records.iter().map(|r| r.node.as_deref()).collect();
        let nodes_array = StringArray::from(nodes);

        let contents: Vec<Option<&str>> = records.iter().map(|r| r.content.as_deref()).collect();
        let contents_array = StringArray::from(contents);

        let metadata: Vec<Option<String>> = records
            .iter()
            .map(|r| r.metadata.as_ref().map(|m| serde_json::to_string(m).ok()).flatten())
            .collect();
        let metadata_array = StringArray::from(metadata);

        RecordBatch::try_new(
            Arc::new(schema),
            vec![
                Arc::new(ids_array),
                Arc::new(vectors_array),
                Arc::new(nodes_array),
                Arc::new(contents_array),
                Arc::new(metadata_array),
            ],
        )
        .context("Failed to create batch")
    }

    /// Convert batch to vector matches
    fn batch_to_matches(&self, batch: &RecordBatch) -> Result<Vec<VectorMatch>> {
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Failed to get ids")?;

        // Distance column is added by search
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

        let metadata = batch
            .column(4)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Failed to get metadata")?;

        let mut matches = Vec::new();
        for i in 0..batch.num_rows() {
            matches.push(VectorMatch {
                id: ids.value(i).to_string(),
                distance: distances.map(|d| d.value(i)).unwrap_or(0.0),
                metadata: metadata.is_valid(i).then(|| {
                    serde_json::from_str(metadata.value(i)).ok()
                }).flatten(),
            });
        }

        Ok(matches)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════════════════════════════════

/// Vector record for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatch {
    pub id: String,
    pub distance: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Arrow helpers
// ═══════════════════════════════════════════════════════════════════════════

fn create_empty_vector_array(dim: i32) -> FixedSizeListArray {
    let values = Float32Array::from(Vec::<f32>::new());
    let field = Arc::new(Field::new("item", DataType::Float32, false));
    FixedSizeListArray::new(field, dim, Arc::new(values), None)
}

fn create_vector_array(vectors: Vec<Vec<f32>>, dim: i32) -> Result<FixedSizeListArray> {
    let flat: Vec<f32> = vectors.into_iter().flatten().collect();
    let values = Float32Array::from(flat);
    let field = Arc::new(Field::new("item", DataType::Float32, false));
    Ok(FixedSizeListArray::new(field, dim, Arc::new(values), None))
}

// ═══════════════════════════════════════════════════════════════════════════
// Shared instance
// ═══════════════════════════════════════════════════════════════════════════

use tokio::sync::OnceCell;

static VECTOR_STORE: OnceCell<VectorStore> = OnceCell::const_new();

/// Get or initialize the global vector store
pub async fn get_vector_store() -> Result<&'static VectorStore> {
    VECTOR_STORE
        .get_or_try_init(|| async {
            let path = std::env::var("LANCEDB_PATH").unwrap_or_else(|_| "./lancedb".to_string());
            let dim = std::env::var("EMBEDDING_DIM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(384);

            VectorStore::new(&path, dim).await
        })
        .await
}

// ═══════════════════════════════════════════════════════════════════════════
// HTTP handlers (for integration with existing API)
// ═══════════════════════════════════════════════════════════════════════════

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::config::AppState;

/// POST /webhook/vectors/upsert
pub async fn vector_upsert_handler(
    State(_state): State<AppState>,
    Json(body): Json<VectorUpsertRequest>,
) -> impl IntoResponse {
    let store = match get_vector_store().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    match store.upsert(&body.table, body.records).await {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({ "upserted": count })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /webhook/vectors/search
pub async fn vector_search_handler(
    State(_state): State<AppState>,
    Json(body): Json<VectorSearchRequest>,
) -> impl IntoResponse {
    let store = match get_vector_store().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    match store
        .search(
            &body.table,
            body.query_vector,
            body.limit.unwrap_or(10),
            body.filter.as_deref(),
        )
        .await
    {
        Ok(matches) => (StatusCode::OK, Json(serde_json::json!({ "matches": matches }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct VectorUpsertRequest {
    pub table: String,
    pub records: Vec<VectorRecord>,
}

#[derive(Debug, Deserialize)]
pub struct VectorSearchRequest {
    pub table: String,
    pub query_vector: Vec<f32>,
    pub limit: Option<usize>,
    pub filter: Option<String>,
}
