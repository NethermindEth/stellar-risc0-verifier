//! This module provides temporary types for the bn254 curve until Stellar
//! introduces native precompiles.
//!
//! The `Fr`, `G1Affine`, and `G2Affine` types from the ark crate cannot be
//! directly used because they lack XDR (de)serialization support. As a
//! workaround, local [`Fr`](bn254::Fr), [`G1Affine`](bn254::G1Affine), and
//! [`G2Affine`](bn254::G2Affine) types are defined that can be converted to
//! their ark equivalents. Once Stellar's bn254 precompiles become available,
//! these types can be replaced with the native implementations.

pub mod bn254;
