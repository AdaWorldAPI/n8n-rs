//! Hamming vector implementation.
//!
//! 10,000-bit vectors with SIMD-accelerated operations.

use crate::{HammingError, VECTOR_BITS, VECTOR_BYTES, VECTOR_WORDS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::{BitAnd, BitOr, BitXor, Not};

/// A 10,000-bit Hamming vector stored in a compact byte representation.
///
/// # Features
/// - Deterministic generation from seeds via SHA256
/// - SIMD-accelerated Hamming distance using POPCNT intrinsics
/// - XOR-based binding/unbinding for associative memory operations
/// - Zero-copy operations where possible
///
/// # Example
/// ```
/// use n8n_hamming::HammingVector;
///
/// let cat = HammingVector::from_seed("cat");
/// let dog = HammingVector::from_seed("dog");
///
/// // Bind two concepts
/// let bound = cat.bind(&dog);
///
/// // Unbind to recover
/// let recovered = bound.unbind(&cat);
/// assert_eq!(recovered.distance(&dog), 0);
/// ```
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct HammingVector {
    /// Internal storage as u64 words for efficient SIMD operations.
    words: [u64; VECTOR_WORDS],
}

impl HammingVector {
    /// Create a new zero vector.
    #[inline]
    pub fn zeros() -> Self {
        Self {
            words: [0u64; VECTOR_WORDS],
        }
    }

    /// Create a new vector with all bits set to 1.
    #[inline]
    pub fn ones() -> Self {
        let mut words = [u64::MAX; VECTOR_WORDS];
        // Mask the last word to only include valid bits
        let valid_bits_in_last_word = VECTOR_BITS % 64;
        if valid_bits_in_last_word > 0 {
            words[VECTOR_WORDS - 1] = (1u64 << valid_bits_in_last_word) - 1;
        }
        Self { words }
    }

    /// Create a random vector using the given RNG.
    pub fn random<R: rand::Rng>(rng: &mut R) -> Self {
        let mut words = [0u64; VECTOR_WORDS];
        for word in words.iter_mut() {
            *word = rng.gen();
        }
        // Mask the last word
        let valid_bits_in_last_word = VECTOR_BITS % 64;
        if valid_bits_in_last_word > 0 {
            words[VECTOR_WORDS - 1] &= (1u64 << valid_bits_in_last_word) - 1;
        }
        Self { words }
    }

    /// Create a vector from a seed string using SHA256.
    ///
    /// The seed is hashed and expanded to fill the 10,000-bit vector
    /// using a deterministic process.
    pub fn from_seed(seed: &str) -> Self {
        let mut words = [0u64; VECTOR_WORDS];
        let mut hasher = Sha256::new();

        // Generate enough hash data to fill the vector
        for chunk_idx in 0..((VECTOR_WORDS * 8 + 31) / 32) {
            hasher.update(seed.as_bytes());
            hasher.update(&(chunk_idx as u64).to_le_bytes());
            let hash = hasher.finalize_reset();

            // Copy hash bytes into words
            let start_word = chunk_idx * 4;
            for (i, chunk) in hash.chunks(8).enumerate() {
                let word_idx = start_word + i;
                if word_idx < VECTOR_WORDS {
                    words[word_idx] = u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8]));
                }
            }
        }

        // Mask the last word
        let valid_bits_in_last_word = VECTOR_BITS % 64;
        if valid_bits_in_last_word > 0 {
            words[VECTOR_WORDS - 1] &= (1u64 << valid_bits_in_last_word) - 1;
        }

        Self { words }
    }

    /// Create a vector from JSON data by hashing its canonical representation.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let canonical = serde_json::to_string(value).unwrap_or_default();
        Self::from_seed(&canonical)
    }

    /// Create a vector from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HammingError> {
        if bytes.len() != VECTOR_BYTES {
            return Err(HammingError::InvalidSize {
                expected: VECTOR_BYTES,
                actual: bytes.len(),
            });
        }

        let mut words = [0u64; VECTOR_WORDS];
        for (i, chunk) in bytes.chunks(8).enumerate() {
            if i < VECTOR_WORDS {
                let mut arr = [0u8; 8];
                arr[..chunk.len()].copy_from_slice(chunk);
                words[i] = u64::from_le_bytes(arr);
            }
        }

        Ok(Self { words })
    }

    /// Convert the vector to bytes.
    #[inline]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(VECTOR_BYTES);
        for word in &self.words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.truncate(VECTOR_BYTES);
        bytes
    }

    /// Get the raw bytes as a slice (zero-copy).
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.words.as_ptr() as *const u8,
                VECTOR_BYTES,
            )
        }
    }

    /// Calculate the Hamming distance to another vector.
    ///
    /// Uses POPCNT intrinsics for fast bit counting.
    /// Returns the number of differing bits (0 to 10,000).
    #[inline]
    pub fn distance(&self, other: &Self) -> u32 {
        self.words
            .iter()
            .zip(other.words.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }

    /// Calculate similarity score (0.0 to 1.0).
    ///
    /// Returns 1.0 for identical vectors, 0.0 for maximally different.
    #[inline]
    pub fn similarity(&self, other: &Self) -> f64 {
        1.0 - (self.distance(other) as f64 / VECTOR_BITS as f64)
    }

    /// Bind two vectors together using XOR.
    ///
    /// This creates a composite representation where:
    /// - `bound = a XOR b`
    /// - `a = bound XOR b` (recovers a)
    /// - `b = bound XOR a` (recovers b)
    #[inline]
    pub fn bind(&self, other: &Self) -> Self {
        let mut result = Self::zeros();
        for (i, (a, b)) in self.words.iter().zip(other.words.iter()).enumerate() {
            result.words[i] = a ^ b;
        }
        result
    }

    /// Unbind a vector using XOR (inverse of bind).
    ///
    /// If `bound = a.bind(b)`, then `bound.unbind(a) == b`.
    #[inline]
    pub fn unbind(&self, key: &Self) -> Self {
        // XOR is its own inverse
        self.bind(key)
    }

    /// Count the number of set bits (population count).
    #[inline]
    pub fn popcount(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    /// Check if a specific bit is set.
    #[inline]
    pub fn get_bit(&self, index: usize) -> bool {
        if index >= VECTOR_BITS {
            return false;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        (self.words[word_idx] >> bit_idx) & 1 == 1
    }

    /// Set a specific bit.
    #[inline]
    pub fn set_bit(&mut self, index: usize, value: bool) {
        if index >= VECTOR_BITS {
            return;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        if value {
            self.words[word_idx] |= 1u64 << bit_idx;
        } else {
            self.words[word_idx] &= !(1u64 << bit_idx);
        }
    }

    /// Create a majority vote vector from multiple vectors.
    ///
    /// Each bit is set if it's set in more than half of the input vectors.
    pub fn majority(vectors: &[&Self]) -> Self {
        if vectors.is_empty() {
            return Self::zeros();
        }

        let threshold = vectors.len() / 2;
        let mut result = Self::zeros();

        for bit_idx in 0..VECTOR_BITS {
            let count = vectors.iter().filter(|v| v.get_bit(bit_idx)).count();
            if count > threshold {
                result.set_bit(bit_idx, true);
            }
        }

        result
    }

    /// Convert to hex string representation.
    pub fn to_hex(&self) -> String {
        self.to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Create from hex string.
    pub fn from_hex(hex: &str) -> Result<Self, HammingError> {
        let bytes: Result<Vec<u8>, _> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect();

        match bytes {
            Ok(b) => Self::from_bytes(&b),
            Err(_) => Err(HammingError::InvalidHex(hex.to_string())),
        }
    }
}

impl BitXor for HammingVector {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        self.bind(&rhs)
    }
}

impl BitXor for &HammingVector {
    type Output = HammingVector;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        self.bind(rhs)
    }
}

impl BitAnd for HammingVector {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut result = Self::zeros();
        for (i, (a, b)) in self.words.iter().zip(rhs.words.iter()).enumerate() {
            result.words[i] = a & b;
        }
        result
    }
}

impl BitOr for HammingVector {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = Self::zeros();
        for (i, (a, b)) in self.words.iter().zip(rhs.words.iter()).enumerate() {
            result.words[i] = a | b;
        }
        result
    }
}

impl Not for HammingVector {
    type Output = Self;

    fn not(self) -> Self::Output {
        let mut result = Self::zeros();
        for (i, w) in self.words.iter().enumerate() {
            result.words[i] = !w;
        }
        // Mask the last word
        let valid_bits_in_last_word = VECTOR_BITS % 64;
        if valid_bits_in_last_word > 0 {
            result.words[VECTOR_WORDS - 1] &= (1u64 << valid_bits_in_last_word) - 1;
        }
        result
    }
}

impl Default for HammingVector {
    fn default() -> Self {
        Self::zeros()
    }
}

impl std::fmt::Debug for HammingVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HammingVector(popcount={}, first_word={:#018x})",
            self.popcount(),
            self.words[0]
        )
    }
}

impl Serialize for HammingVector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_hex())
        } else {
            serializer.serialize_bytes(&self.to_bytes())
        }
    }
}

impl<'de> Deserialize<'de> for HammingVector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let hex = String::deserialize(deserializer)?;
            Self::from_hex(&hex).map_err(serde::de::Error::custom)
        } else {
            let bytes = Vec::<u8>::deserialize(deserializer)?;
            Self::from_bytes(&bytes).map_err(serde::de::Error::custom)
        }
    }
}

/// A collection of Hamming vectors with efficient similarity search.
#[derive(Debug, Clone, Default)]
pub struct HammingIndex {
    vectors: Vec<(String, HammingVector)>,
}

impl HammingIndex {
    pub fn new() -> Self {
        Self {
            vectors: Vec::new(),
        }
    }

    /// Add a vector to the index.
    pub fn insert(&mut self, id: impl Into<String>, vector: HammingVector) {
        self.vectors.push((id.into(), vector));
    }

    /// Find the k most similar vectors to the query.
    pub fn search(&self, query: &HammingVector, k: usize, max_distance: Option<u32>) -> Vec<(&str, u32)> {
        let mut results: Vec<_> = self
            .vectors
            .iter()
            .map(|(id, v)| (id.as_str(), query.distance(v)))
            .filter(|(_, d)| max_distance.map_or(true, |max| *d <= max))
            .collect();

        results.sort_by_key(|(_, d)| *d);
        results.truncate(k);
        results
    }

    /// Get the number of vectors in the index.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Get a vector by ID.
    pub fn get(&self, id: &str) -> Option<&HammingVector> {
        self.vectors
            .iter()
            .find(|(vid, _)| vid == id)
            .map(|(_, v)| v)
    }
}
