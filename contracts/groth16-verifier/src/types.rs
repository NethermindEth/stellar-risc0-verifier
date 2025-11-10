use ark_bn254::{G1Affine as AG1Affine, G2Affine as AG2Affine};
use soroban_sdk::{Bytes, BytesN, contracttype};

use crate::{
    Groth16Error,
    crypto::bn254::{G1Affine, G2Affine},
};

/// Groth16 verification key for BN254 curve.
///
/// Contains the public parameters needed to verify a Groth16 proof:
/// - `alpha`, `beta`, `gamma`, `delta`: Fixed elliptic curve points from the trusted setup
/// - `ic`: Array of G1 points used for computing the public input component
///
/// This structure uses arkworks types internally and is not serializable for contract storage.
#[derive(Clone)]
pub struct VerificationKey {
    pub alpha: AG1Affine,
    pub beta: AG2Affine,
    pub gamma: AG2Affine,
    pub delta: AG2Affine,
    pub ic: [AG1Affine; 6],
}

/// Groth16 proof with XDR serialization support.
///
/// Contains three elliptic curve points that constitute a Groth16 zero-knowledge proof:
///
/// This structure uses Soroban-compatible types and can be passed across contract boundaries.
#[derive(Clone)]
#[contracttype]
pub struct Groth16Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

/// Groth16 proof using arkworks types for internal verification.
///
/// Contains the same three elliptic curve points as `Groth16Proof`, but uses arkworks
/// types that are compatible with the arkworks Groth16 verification algorithm.
#[derive(Clone)]
pub struct ArkProof {
    pub a: AG1Affine,
    pub b: AG2Affine,
    pub c: AG1Affine,
}

pub struct Groth16Seal {
    pub selector: BytesN<4>,
    pub proof: Groth16Proof,
}

impl From<Groth16Proof> for ArkProof {
    fn from(value: Groth16Proof) -> Self {
        Self {
            a: (&value.a).into(),
            b: (&value.b).into(),
            c: (&value.c).into(),
        }
    }
}

// Byte layout constants
const SELECTOR_SIZE: u32 = 4;
const FIELD_ELEMENT_SIZE: u32 = 32;
const G1_SIZE: u32 = FIELD_ELEMENT_SIZE * 2;  // x, y
const G2_SIZE: u32 = FIELD_ELEMENT_SIZE * 4;  // x_0, x_1, y_0, y_1
const PROOF_SIZE: u32 = G1_SIZE + G2_SIZE + G1_SIZE;  // a, b, c
const SEAL_SIZE: u32 = SELECTOR_SIZE + PROOF_SIZE;

/// Helper to extract a 32-byte field element at a given offset
#[inline]
fn extract_field_element(bytes: &Bytes, offset: u32) -> Result<BytesN<32>, Groth16Error> {
    bytes
        .slice(offset..offset + FIELD_ELEMENT_SIZE)
        .try_into()
        .map_err(|_| Groth16Error::MalformedSeal)
}

impl TryFrom<Bytes> for Groth16Seal {
    type Error = Groth16Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != SEAL_SIZE {
            return Err(Groth16Error::MalformedSeal);
        }

        let selector = value
            .slice(0..SELECTOR_SIZE)
            .try_into()
            .map_err(|_| Groth16Error::MalformedSeal)?;

        let proof = value.slice(SELECTOR_SIZE..).try_into()?;

        Ok(Self { selector, proof })
    }
}

impl TryFrom<Bytes> for Groth16Proof {
    type Error = Groth16Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != PROOF_SIZE {
            return Err(Groth16Error::MalformedSeal);
        }

        let mut offset = 0u32;

        // Parse G1Affine point 'a'
        let a = G1Affine {
            x: extract_field_element(&value, offset)?,
            y: extract_field_element(&value, offset + FIELD_ELEMENT_SIZE)?,
        };
        offset += G1_SIZE;

        // Parse G2Affine point 'b'
        // risc0 encodes G2 in Ethereum-style format: (imaginary, real) for each Fq2
        // But arkworks expects (c0=real, c1=imaginary), so we swap during parsing
        let b = G2Affine {
            x_0: extract_field_element(&value, offset + FIELD_ELEMENT_SIZE)?, // real part (second)
            x_1: extract_field_element(&value, offset)?,                      // imag part (first)
            y_0: extract_field_element(&value, offset + FIELD_ELEMENT_SIZE * 3)?, // real part (fourth)
            y_1: extract_field_element(&value, offset + FIELD_ELEMENT_SIZE * 2)?, // imag part (third)
        };
        offset += G2_SIZE;

        // Parse G1Affine point 'c'
        let c = G1Affine {
            x: extract_field_element(&value, offset)?,
            y: extract_field_element(&value, offset + FIELD_ELEMENT_SIZE)?,
        };

        Ok(Self { a, b, c })
    }
}
