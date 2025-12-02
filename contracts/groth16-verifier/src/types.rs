use core::array;

use soroban_sdk::{
    Bytes, BytesN, Env, contracttype,
    crypto::bn254::{G1Affine, G2Affine},
};

use crate::Groth16Error;

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

pub struct VerificationKeyBytes {
    pub alpha: [u8; G1_SIZE],
    pub beta: [u8; G2_SIZE],
    pub gamma: [u8; G2_SIZE],
    pub delta: [u8; G2_SIZE],
    pub ic: [[u8; G1_SIZE]; 6],
}

impl VerificationKeyBytes {
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

pub struct Groth16Seal {
    pub selector: BytesN<4>,
    pub proof: Groth16Proof,
}

const SELECTOR_SIZE: usize = 4;
const FIELD_ELEMENT_SIZE: usize = 32;
const G1_SIZE: usize = FIELD_ELEMENT_SIZE * 2; // x, y
const G2_SIZE: usize = FIELD_ELEMENT_SIZE * 4; // x_0, x_1, y_0, y_1
const PROOF_SIZE: usize = G1_SIZE + G2_SIZE + G1_SIZE; // a, b, c
const SEAL_SIZE: usize = SELECTOR_SIZE + PROOF_SIZE;

impl TryFrom<Bytes> for Groth16Seal {
    type Error = Groth16Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != SEAL_SIZE as u32 {
            return Err(Groth16Error::MalformedSeal);
        }

        let selector = value
            .slice(0..SELECTOR_SIZE as u32)
            .try_into()
            .map_err(|_| Groth16Error::MalformedSeal)?;

        let proof = value.slice(SELECTOR_SIZE as u32..).try_into()?;

        Ok(Self { selector, proof })
    }
}

impl TryFrom<Bytes> for Groth16Proof {
    type Error = Groth16Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != PROOF_SIZE as u32 {
            return Err(Groth16Error::MalformedSeal);
        }

        let a = G1Affine::from_bytes(value.slice(0..64).try_into().unwrap());
        let b = G2Affine::from_bytes(value.slice(64..192).try_into().unwrap());
        let c = G1Affine::from_bytes(value.slice(192..).try_into().unwrap());

        Ok(Self { a, b, c })
    }
}
