//! # Groth16 Proof Types
//!
//! Defines the data structures for Groth16 proofs and seals used by the
//! [`RiscZeroGroth16Verifier`](super::RiscZeroGroth16Verifier).
//!
//! ## Seal Layout
//!
//! A RISC Zero Groth16 seal is `SEAL_SIZE` (260) bytes:
//!
//! ```text
//! | selector (4B) | A (G1, 64B) | B (G2, 128B) | C (G1, 64B) |
//! ```
//!
//! ## Type Hierarchy
//!
//! - [`Groth16Seal`] -- selector + proof, deserialized from raw seal bytes
//! - [`Groth16Proof`] -- the three elliptic curve points (A, B, C)
//! - [`VerificationKey`] -- the Groth16 verification key (runtime form)
//! - [`VerificationKeyBytes`] -- byte-oriented key for compile-time embedding

use core::array;

use soroban_sdk::{
    Bytes, BytesN, Env, contracttype,
    crypto::bn254::{Bn254G1Affine as G1Affine, Bn254G2Affine as G2Affine},
};

use risc0_interface::VerifierError;

/// Size of the 4-byte selector prefix in a seal.
const SELECTOR_SIZE: usize = 4;

/// Size of a single BN254 field element (Fq) in bytes.
const FIELD_ELEMENT_SIZE: usize = 32;

/// Size of a G1 affine point: two field elements (x, y).
const G1_SIZE: usize = FIELD_ELEMENT_SIZE * 2; // x, y

/// Size of a G2 affine point: four field elements (x_0, x_1, y_0, y_1).
const G2_SIZE: usize = FIELD_ELEMENT_SIZE * 4; // x_0, x_1, y_0, y_1

/// Size of a Groth16 proof: A (G1) + B (G2) + C (G1) = 256 bytes.
const PROOF_SIZE: usize = G1_SIZE + G2_SIZE + G1_SIZE; // a, b, c

/// Total size of a RISC Zero Groth16 seal: selector + proof = 260 bytes.
const SEAL_SIZE: usize = SELECTOR_SIZE + PROOF_SIZE;

/// Groth16 verification key for BN254 curve.
///
/// Contains the public parameters needed to verify a Groth16 proof:
/// - `alpha`, `beta`, `gamma`, `delta`: Fixed elliptic curve points from the trusted setup
/// - `ic`: Array of G1 points used for computing the public input component
///
/// This structure uses arkworks types internally and is not serializable for contract storage.
#[derive(Clone)]
pub struct VerificationKey {
    pub alpha: G1Affine,
    pub beta: G2Affine,
    pub gamma: G2Affine,
    pub delta: G2Affine,
    pub ic: [G1Affine; 6],
}

/// Byte-oriented version of the verification key generated at build time.
///
/// Soroban's BN254 affine types are not `const` constructible, so we emit the
/// key as raw byte arrays in `build.rs` and reconstruct the affine points at
/// runtime inside the contract via [`verification_key`]. This keeps the key
/// embeddable with `include!` while still avoiding any serialization support on
/// the `VerificationKey` itself.
pub struct VerificationKeyBytes {
    pub alpha: [u8; G1_SIZE],
    pub beta: [u8; G2_SIZE],
    pub gamma: [u8; G2_SIZE],
    pub delta: [u8; G2_SIZE],
    pub ic: [[u8; G1_SIZE]; 6],
}

impl VerificationKeyBytes {
    /// Converts the byte-oriented key into a runtime [`VerificationKey`].
    ///
    /// Reconstructs the BN254 affine points from their raw byte
    /// representations using the Soroban crypto API.
    pub fn verification_key(&self, env: &Env) -> VerificationKey {
        VerificationKey {
            alpha: G1Affine::from_array(env, &self.alpha),
            beta: G2Affine::from_array(env, &self.beta),
            gamma: G2Affine::from_array(env, &self.gamma),
            delta: G2Affine::from_array(env, &self.delta),
            ic: array::from_fn(|i| G1Affine::from_array(env, &self.ic[i])),
        }
    }
}

/// Groth16 proof containing three elliptic curve points.
///
/// This is the core cryptographic proof verified by the pairing check:
///
/// - **A** (G1) -- first proof element
/// - **B** (G2) -- second proof element
/// - **C** (G1) -- third proof element
///
/// These points are produced by the prover and must satisfy the Groth16
/// pairing equation for verification to succeed. The structure uses
/// Soroban-compatible XDR types and can be passed across contract boundaries.
#[derive(Clone)]
#[contracttype]
pub struct Groth16Proof {
    /// First proof element (G1 affine point).
    pub a: G1Affine,
    /// Second proof element (G2 affine point).
    pub b: G2Affine,
    /// Third proof element (G1 affine point).
    pub c: G1Affine,
}

/// A Groth16 seal combining a verifier selector with a proof.
///
/// The seal is the on-chain representation of a RISC Zero Groth16 proof.
/// The first 4 bytes identify which verifier should process the proof
/// (the selector), followed by the proof points.
///
/// # Wire Format
///
/// ```text
/// | selector (4 bytes) | proof (256 bytes) |
/// ```
///
/// Total: 260 bytes (`SEAL_SIZE`).
#[derive(Clone)]
#[contracttype]
pub struct Groth16Seal {
    /// 4-byte selector identifying the target verifier.
    pub selector: BytesN<4>,
    /// The Groth16 proof (curve points A, B, C).
    pub proof: Groth16Proof,
}

/// Decodes a [`Groth16Seal`] from raw seal bytes.
///
/// Expects exactly `SEAL_SIZE` (260) bytes. The first 4 bytes are the
/// selector and the remaining 256 bytes are parsed as a [`Groth16Proof`].
///
/// # Errors
///
/// Returns [`VerifierError::MalformedSeal`] if the byte length is wrong.
impl TryFrom<Bytes> for Groth16Seal {
    type Error = VerifierError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != SEAL_SIZE as u32 {
            return Err(VerifierError::MalformedSeal);
        }

        let selector = value
            .slice(0..SELECTOR_SIZE as u32)
            .try_into()
            .map_err(|_| VerifierError::MalformedSeal)?;

        let proof = value.slice(SELECTOR_SIZE as u32..).try_into()?;

        Ok(Self { selector, proof })
    }
}

/// Decodes a [`Groth16Proof`] from raw bytes.
///
/// Expects exactly `PROOF_SIZE` (256) bytes laid out as:
///
/// ```text
/// | A (G1, 64B) | B (G2, 128B) | C (G1, 64B) |
/// ```
///
/// # Errors
///
/// Returns [`VerifierError::MalformedSeal`] if the byte length is wrong
/// or if any sub-slice cannot be converted to a curve point.
impl TryFrom<Bytes> for Groth16Proof {
    type Error = VerifierError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != PROOF_SIZE as u32 {
            return Err(VerifierError::MalformedSeal);
        }

        let a = G1Affine::from_bytes(
            value
                .slice(0..G1_SIZE as u32)
                .try_into()
                .map_err(|_| VerifierError::MalformedSeal)?,
        );
        let b = G2Affine::from_bytes(
            value
                .slice(G1_SIZE as u32..G1_SIZE as u32 + G2_SIZE as u32)
                .try_into()
                .map_err(|_| VerifierError::MalformedSeal)?,
        );
        let c = G1Affine::from_bytes(
            value
                .slice(G1_SIZE as u32 + G2_SIZE as u32..)
                .try_into()
                .map_err(|_| VerifierError::MalformedSeal)?,
        );

        Ok(Self { a, b, c })
    }
}
