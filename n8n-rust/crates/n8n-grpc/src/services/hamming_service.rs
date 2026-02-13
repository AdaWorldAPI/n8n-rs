//! Hamming similarity gRPC service.

use n8n_hamming::{HammingIndex, HammingVector};
use bytes::Bytes;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::Status;

/// Hamming similarity service for fingerprint operations.
#[derive(Clone)]
pub struct HammingGrpcService {
    indices: Arc<RwLock<HashMap<String, HammingIndex>>>,
}

impl HammingGrpcService {
    pub fn new() -> Self {
        let mut indices = HashMap::new();
        indices.insert("default".to_string(), HammingIndex::new());

        Self {
            indices: Arc::new(RwLock::new(indices)),
        }
    }

    /// Create a fingerprint from seed.
    pub fn create_fingerprint_from_seed(&self, seed: &str) -> FingerprintResult {
        let vector = HammingVector::from_seed(seed);
        FingerprintResult {
            fingerprint: vector.to_bytes().into(),
            id: None,
        }
    }

    /// Create a fingerprint from JSON data.
    pub fn create_fingerprint_from_json(
        &self,
        json: &serde_json::Value,
    ) -> FingerprintResult {
        let vector = HammingVector::from_json(json);
        FingerprintResult {
            fingerprint: vector.to_bytes().into(),
            id: None,
        }
    }

    /// Create a fingerprint from raw bytes.
    pub fn create_fingerprint_from_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<FingerprintResult, Status> {
        let vector =
            HammingVector::from_bytes(bytes).map_err(|e| Status::invalid_argument(e.to_string()))?;
        Ok(FingerprintResult {
            fingerprint: vector.to_bytes().into(),
            id: None,
        })
    }

    /// Add a fingerprint to an index.
    pub async fn add_to_index(
        &self,
        collection: &str,
        id: &str,
        fingerprint: &[u8],
    ) -> Result<(), Status> {
        let vector = HammingVector::from_bytes(fingerprint)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let mut indices = self.indices.write().await;
        let index = indices
            .entry(collection.to_string())
            .or_insert_with(HammingIndex::new);
        index.insert(id, vector);

        Ok(())
    }

    /// Find similar fingerprints.
    pub async fn find_similar(
        &self,
        collection: &str,
        query: &[u8],
        top_k: usize,
        max_distance: Option<u32>,
    ) -> Result<Vec<SimilarityMatch>, Status> {
        let query_vector = HammingVector::from_bytes(query)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let indices = self.indices.read().await;
        let index = indices
            .get(collection)
            .ok_or_else(|| Status::not_found(format!("Collection {} not found", collection)))?;

        let results = index.search(&query_vector, top_k, max_distance);

        Ok(results
            .into_iter()
            .map(|(id, distance)| SimilarityMatch {
                id: id.to_string(),
                distance,
                similarity: 1.0 - (distance as f64 / 10000.0),
            })
            .collect())
    }

    /// Stream similarity search results.
    pub async fn find_similar_stream(
        &self,
        collection: &str,
        query: &[u8],
        top_k: usize,
        max_distance: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SimilarityMatch, Status>> + Send>>, Status> {
        let matches = self
            .find_similar(collection, query, top_k, max_distance)
            .await?;

        let (tx, rx) = mpsc::channel(top_k.min(100));

        tokio::spawn(async move {
            for m in matches {
                if tx.send(Ok(m)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Bind two fingerprints using XOR.
    pub fn bind_fingerprints(&self, a: &[u8], b: &[u8]) -> Result<FingerprintResult, Status> {
        let vec_a =
            HammingVector::from_bytes(a).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let vec_b =
            HammingVector::from_bytes(b).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let bound = vec_a.bind(&vec_b);

        Ok(FingerprintResult {
            fingerprint: bound.to_bytes().into(),
            id: None,
        })
    }

    /// Unbind a fingerprint using XOR.
    pub fn unbind_fingerprint(&self, bound: &[u8], key: &[u8]) -> Result<FingerprintResult, Status> {
        let vec_bound =
            HammingVector::from_bytes(bound).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let vec_key =
            HammingVector::from_bytes(key).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let unbound = vec_bound.unbind(&vec_key);

        Ok(FingerprintResult {
            fingerprint: unbound.to_bytes().into(),
            id: None,
        })
    }

    /// Calculate Hamming distance between two fingerprints.
    pub fn calculate_distance(&self, a: &[u8], b: &[u8]) -> Result<DistanceResult, Status> {
        let vec_a =
            HammingVector::from_bytes(a).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let vec_b =
            HammingVector::from_bytes(b).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let distance = vec_a.distance(&vec_b);
        let similarity = vec_a.similarity(&vec_b);

        Ok(DistanceResult {
            hamming_distance: distance,
            similarity,
        })
    }

    /// Get index statistics.
    pub async fn get_index_stats(&self, collection: &str) -> Result<IndexStats, Status> {
        let indices = self.indices.read().await;
        let index = indices
            .get(collection)
            .ok_or_else(|| Status::not_found(format!("Collection {} not found", collection)))?;

        Ok(IndexStats {
            collection: collection.to_string(),
            vector_count: index.len(),
        })
    }

    /// List all collections.
    pub async fn list_collections(&self) -> Vec<String> {
        self.indices.read().await.keys().cloned().collect()
    }

    /// Create a new collection.
    pub async fn create_collection(&self, name: &str) -> Result<(), Status> {
        let mut indices = self.indices.write().await;
        if indices.contains_key(name) {
            return Err(Status::already_exists(format!(
                "Collection {} already exists",
                name
            )));
        }
        indices.insert(name.to_string(), HammingIndex::new());
        Ok(())
    }

    /// Delete a collection.
    pub async fn delete_collection(&self, name: &str) -> Result<bool, Status> {
        let mut indices = self.indices.write().await;
        Ok(indices.remove(name).is_some())
    }
}

impl Default for HammingGrpcService {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of creating a fingerprint.
#[derive(Debug, Clone)]
pub struct FingerprintResult {
    pub fingerprint: Bytes,
    pub id: Option<String>,
}

/// Result of similarity search.
#[derive(Debug, Clone)]
pub struct SimilarityMatch {
    pub id: String,
    pub distance: u32,
    pub similarity: f64,
}

/// Result of distance calculation.
#[derive(Debug, Clone)]
pub struct DistanceResult {
    pub hamming_distance: u32,
    pub similarity: f64,
}

/// Index statistics.
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub collection: String,
    pub vector_count: usize,
}
