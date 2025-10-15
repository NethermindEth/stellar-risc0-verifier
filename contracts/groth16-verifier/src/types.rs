use ark_bn254::{G1Affine as AG1Affine, G2Affine as AG2Affine};
use soroban_sdk::contracttype;

use crate::crypto::bn254::{G1Affine, G2Affine};

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

impl From<Groth16Proof> for ArkProof {
    fn from(value: Groth16Proof) -> Self {
        Self {
            a: (&value.a).into(),
            b: (&value.b).into(),
            c: (&value.c).into(),
        }
    }
}
