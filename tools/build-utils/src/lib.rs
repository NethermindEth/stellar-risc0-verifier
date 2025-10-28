//! Build-time utilities for cryptographic hashing operations.
//!
//! This crate provides low-level cryptographic hashing utilities for build scripts
//! across the stellar-risc0-verifier workspace.
//!
//! ## Overview
//!
//! The crate implements cryptographic hashing utilities used in the RISC Zero verification system:
//!
//! - **Tagged Hashing**: A scheme for creating unique, collision-resistant hashes for structured data
//!   by combining type tags with content digests
//! - **Point Hashing**: SHA-256 hashing of elliptic curve points
//! - **List Hashing**: Cons-list based hashing for collections of digests
//!
//! ## Main Functions
//!
//! - [`hash_point()`]: Hashes elliptic curve points in a standardized format
//! - [`tagged_struct()`]: Creates hashes for tagged structs with named fields
//! - [`tagged_iter()`]: Creates hashes for tagged lists from iterators
//!
//! ## Usage in Build Scripts
//!
//! These utilities serve as building blocks for higher-level cryptographic operations
//! in build scripts, such as generating verification keys and computing control roots
//!
//! ## Example
//!
//! ```ignore
//! use build_utils::{tagged_struct, Sha256Digest};
//!
//! // Create a simple tagged struct
//! let field1: Sha256Digest = [0u8; 32];
//! let field2: Sha256Digest = [1u8; 32];
//! let struct_hash = tagged_struct("MyStruct", &[field1, field2]);
//! ```

use ark_ec::AffineRepr;
use ark_serialize::CanonicalSerialize;
use sha2::{Digest, Sha256};

/// The size of a SHA-256 digest in bytes.
const DIGEST_SIZE: usize = 32;

/// A 32-byte SHA-256 digest.
pub type Sha256Digest = [u8; DIGEST_SIZE];

/// Hashes an elliptic curve point using SHA-256.
///
/// # Panics
///
/// Panics if the point cannot be serialized or if it has no xy coordinates
pub fn hash_point(p: impl AffineRepr) -> Sha256Digest {
    let mut buffer = Vec::new();
    let (x, y) = p.xy().unwrap();
    x.serialize_uncompressed(&mut buffer).unwrap();
    y.serialize_uncompressed(&mut buffer).unwrap();
    buffer.reverse();
    Sha256::digest(&buffer).into()
}

/// Creates a tagged struct hash from a tag and a list of field digests.
///
/// This function implements a tagged hashing scheme where a struct is identified by a tag
/// and contains zero or more fields (represented as digests). The resulting hash is computed
/// by concatenating the tag digest, all field digests, and the field count (as a little-endian u16).
///
/// # Arguments
///
/// * `tag` - A string tag identifying the struct type
/// * `down` - A slice of SHA-256 digests representing the struct's fields
///
/// # Panics
///
/// Panics if the number of fields exceeds 65535 (2^16 - 1)
///
/// # Example
///
/// ```ignore
/// let field1 = [0u8; 32];
/// let field2 = [1u8; 32];
/// let struct_hash = tagged_struct("MyStruct", &[field1, field2]);
/// ```
pub fn tagged_struct(tag: &str, down: &[Sha256Digest]) -> Sha256Digest {
    let tag_digest = Sha256::digest(tag.as_bytes());

    let capacity = DIGEST_SIZE + (down.len() * DIGEST_SIZE) + size_of::<u16>();
    let mut tag_struct = Vec::with_capacity(capacity);
    tag_struct.extend_from_slice(&tag_digest);

    for digest in down {
        tag_struct.extend_from_slice(digest);
    }

    let down_count: u16 = down
        .len()
        .try_into()
        .expect("struct defined with more than 2^16 fields");
    tag_struct.extend_from_slice(&down_count.to_le_bytes());

    Sha256::digest(tag_struct).into()
}

/// Creates a tagged list hash from a tag and an iterator of digests.
///
/// This function implements a tagged hashing scheme for lists, processing elements
/// from right to left (using `rfold`) to build a cons-list structure. Each element
/// is combined with the accumulated list digest using the `tagged_list_cons` function.
///
/// # Arguments
///
/// * `tag` - A string tag identifying the list type
/// * `iter` - A double-ended iterator yielding SHA-256 digests
///
/// # Example
///
/// ```ignore
/// let items = vec![[0u8; 32], [1u8; 32], [2u8; 32]];
/// let list_hash = tagged_iter("MyList", items.into_iter());
/// ```
pub fn tagged_iter(tag: &str, iter: impl DoubleEndedIterator<Item = Sha256Digest>) -> Sha256Digest {
    iter.rfold([0u8; 32], |list_digest, elem| {
        tagged_list_cons(tag, elem, list_digest)
    })
}

/// Constructs a cons cell for a tagged list.
///
/// This is a helper function that creates a tagged struct representing a cons cell
/// in a linked list structure. A cons cell consists of a head element and a tail
/// (the rest of the list).
///
/// # Arguments
///
/// * `tag` - A string tag identifying the list type
/// * `head` - The SHA-256 digest of the current list element
/// * `tail` - The SHA-256 digest of the rest of the list
fn tagged_list_cons(tag: &str, head: Sha256Digest, tail: Sha256Digest) -> Sha256Digest {
    tagged_struct(tag, &[head, tail])
}

#[cfg(test)]
mod tests {
    use super::{tagged_iter, tagged_struct};

    #[test]
    fn test_tagged_struct() {
        let digest1 = tagged_struct("foo", &[]);
        let digest2 = tagged_struct("bar", &[digest1, digest1]);
        let digest3 = tagged_struct("baz", &[digest1, digest2, digest1]);

        assert_eq!(
            hex::encode(digest3),
            "2228eb06bfbeaeb2cc12de86fd13373cb5ccdc8afac9af4299dd5a86a72afc4b"
        );
    }

    #[test]
    fn test_tagged_iter() {
        let items = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let list_hash = tagged_iter("test_list", items.into_iter());

        // Should produce a deterministic hash for the same input
        assert_eq!(
            hex::encode(list_hash),
            "ce5bab9f0463274273c20a25618514bf4643a5964034a153c1244e48653e1354"
        );
    }

    #[test]
    fn test_tagged_iter_empty() {
        let empty: Vec<[u8; 32]> = vec![];
        let list_hash = tagged_iter("empty_list", empty.into_iter());

        // Empty list should hash to zero-filled array
        assert_eq!(list_hash, [0u8; 32]);
    }
}
