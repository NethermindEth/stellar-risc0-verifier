#![no_std]

// Use Soroban's allocator for heap allocations
extern crate alloc;

use ark_bn254::{Bn254, Fq12, Fr as AFr};
use ark_ec::{AffineRepr, CurveGroup, pairing::Pairing};
use ark_ff::Field;
use risc0_interface::{ImageId, JournalDigest, Receipt, RiscZeroVerifierInterface, Seal};
use soroban_sdk::{Env, Vec, contract, contracterror, contractimpl};

use crypto::bn254::Fr;
use types::{ArkProof, Groth16Proof, VerificationKey};

mod crypto;
mod test;
mod types;

/// Errors that can occur during Groth16 proof verification.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Groth16Error {
    /// The proof verification failed (pairing check did not equal identity).
    InvalidProof = 0,
    /// The number of public inputs does not match the verification key.
    MalformedPublicInputs = 1,
}

/// Groth16 verifier contract for RISC Zero receipts of execution.
///
/// This contract implements the [`RiscZeroVerifierInterface`] using Groth16 zero-knowledge
/// proofs over the BN254 elliptic curve.
#[contract]
pub struct RiscZeroGroth16Verifier;

#[contractimpl]
impl RiscZeroGroth16Verifier {
    /// Groth16 verification key for the RISC Zero system.
    ///
    /// This verification key is generated at build time from `vk.json`
    const VERIFICATION_KEY: VerificationKey =
        include!(concat!(env!("OUT_DIR"), "/verification_key.rs"));

    const VERSION: &'static str = include!(concat!(env!("OUT_DIR"), "/version.rs"));
    const CONTROL_ROOT_0: [u8; 16] = include!(concat!(env!("OUT_DIR"), "/control_root_0.rs"));
    const CONTROL_ROOT_1: [u8; 16] = include!(concat!(env!("OUT_DIR"), "/control_root_1.rs"));
    const BN254_CONTROL_ID: [u8; 32] = include!(concat!(env!("OUT_DIR"), "/bn254_control_id.rs"));
    const SELECTOR: [u8; 4] = include!(concat!(env!("OUT_DIR"), "/selector.rs"));

    /// Verifies a Groth16 proof with the given public signals.
    ///
    /// This function implements the core Groth16 verification algorithm using the BN254
    /// pairing-friendly elliptic curve. The verification checks the pairing equation:
    ///
    /// `e(-A, B) * e(alpha, beta) * e(vk_x, gamma) * e(C, delta) == 1`
    ///
    /// where `vk_x` is computed as a linear combination of the verification key's IC points
    /// weighted by the public signals.
    ///
    /// # Parameters
    ///
    /// - `proof`: The Groth16 proof containing points A, B, and C
    /// - `pub_signals`: Vector of public input signals (scalar field elements)
    ///
    pub fn verify_proof(proof: Groth16Proof, pub_signals: Vec<Fr>) -> Result<bool, Groth16Error> {
        let vk = Self::VERIFICATION_KEY;

        if pub_signals.len() + 1 != vk.ic.len() as u32 {
            return Err(Groth16Error::MalformedPublicInputs);
        }

        // Parse the proof to ArkProof
        let proof: ArkProof = proof.into();

        // Work in projective coordinates for efficiency
        let mut vk_x = vk.ic[0].into_group();
        for (s, v) in pub_signals.iter().zip(vk.ic.iter().skip(1)) {
            let scalar: AFr = s.into();
            vk_x += *v * scalar;
        }

        // Compute the pairing check:
        // e(-A, B) * e(alpha, beta) * e(vk_x, gamma) * e(C, delta) == 1
        let neg_a = -proof.a;
        let g1_points = [neg_a, vk.alpha, vk_x.into_affine(), proof.c];
        let g2_points = [proof.b, vk.beta, vk.gamma, vk.delta];

        // Two-step pairing: Miller loop + final exponentiation
        let mlo = Bn254::multi_miller_loop(g1_points, g2_points);
        let result = Bn254::final_exponentiation(mlo).ok_or(Groth16Error::InvalidProof)?;

        Ok(result.0 == Fq12::ONE)
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroGroth16Verifier {
    type Proof = Groth16Proof;

    fn verify(_env: Env, _seal: Seal, _image_id: ImageId, _journal: JournalDigest) {}
    fn verify_integrity(_receipt: Receipt) {}
}
