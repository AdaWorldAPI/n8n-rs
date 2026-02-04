//! # n8n-hamming
//!
//! 10kbit bitpacked Hamming vectors for efficient similarity operations.
//! Inspired by the Firefly and Ladybug-rs projects for cognitive fingerprinting.
//!
//! Features:
//! - 10,000-bit vectors packed into 1,250 bytes
//! - SIMD-accelerated Hamming distance using POPCNT
//! - Zero-copy operations where possible
//! - XOR-based binding/unbinding for associative operations
//! - SHA256-based seed generation

pub mod error;
pub mod vector;

pub use error::*;
pub use vector::*;

/// Size of Hamming vectors in bits.
pub const VECTOR_BITS: usize = 10_000;

/// Size of Hamming vectors in bytes (1,250 bytes = 10,000 bits).
pub const VECTOR_BYTES: usize = VECTOR_BITS / 8 + if VECTOR_BITS % 8 != 0 { 1 } else { 0 };

/// Number of u64 words needed to store the vector (157 words, with last partial).
pub const VECTOR_WORDS: usize = VECTOR_BYTES / 8 + if VECTOR_BYTES % 8 != 0 { 1 } else { 0 };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(VECTOR_BITS, 10_000);
        assert_eq!(VECTOR_BYTES, 1250);
        assert_eq!(VECTOR_WORDS, 157);
    }

    #[test]
    fn test_vector_creation() {
        let v1 = HammingVector::from_seed("hello");
        let v2 = HammingVector::from_seed("hello");
        let v3 = HammingVector::from_seed("world");

        // Same seed produces same vector
        assert_eq!(v1.distance(&v2), 0);

        // Different seeds produce different vectors
        assert!(v1.distance(&v3) > 0);
    }

    #[test]
    fn test_xor_binding() {
        let cat = HammingVector::from_seed("cat");
        let dog = HammingVector::from_seed("dog");

        // Bind cat and dog
        let bound = cat.bind(&dog);

        // Unbind with cat to recover dog
        let recovered = bound.unbind(&cat);

        // Should recover dog exactly
        assert_eq!(recovered.distance(&dog), 0);
    }

    #[test]
    fn test_similarity() {
        let v1 = HammingVector::from_seed("apple");
        let v2 = HammingVector::from_seed("apple");

        assert!((v1.similarity(&v2) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_serialization() {
        let v1 = HammingVector::from_seed("test");
        let bytes = v1.to_bytes();
        let v2 = HammingVector::from_bytes(&bytes).unwrap();

        assert_eq!(v1.distance(&v2), 0);
    }
}
