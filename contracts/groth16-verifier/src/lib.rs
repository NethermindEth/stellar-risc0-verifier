#![no_std]

// Use Soroban's allocator for heap allocations
extern crate alloc;

use ark_bn254::{Bn254, Fq12, Fr as AFr};
use ark_ec::{AffineRepr, CurveGroup, pairing::Pairing};
use ark_ff::Field;
use risc0_interface::{
    ImageId, JournalDigest, Receipt, ReceiptClaim, RiscZeroVerifierInterface, Seal,
};
use soroban_sdk::{BytesN, Env, Vec, contract, contracterror, contractimpl};

use crypto::bn254::Fr;
use types::{ArkProof, Groth16Proof, Groth16Seal, VerificationKey};

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
    /// The seal data is malformed or has incorrect byte length.
    MalformedSeal = 2,
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

    fn verify(env: Env, seal: Seal, image_id: ImageId, journal: JournalDigest) {
        let claim = ReceiptClaim::new(&env, image_id, journal);
        let receipt = Receipt {
            seal,
            claim_digest: claim.digest(&env),
        };
        Self::verify_integrity(env, receipt);
    }

    fn verify_integrity(env: Env, receipt: Receipt) {
        let seal = Groth16Seal::try_from(receipt.seal).unwrap();

        if seal.selector != Self::SELECTOR {
            panic!("bad selector"); // TODO: Add missing error
        }

        let (claim_0, claim_1) = split_digest(&env, receipt.claim_digest);

        let control_root_0 = {
            let mut bytes = [0u8; 32];
            bytes[16..32].copy_from_slice(&Self::CONTROL_ROOT_0);
            BytesN::from_array(&env, &bytes)
        };

        let control_root_1 = {
            let mut bytes = [0u8; 32];
            bytes[16..32].copy_from_slice(&Self::CONTROL_ROOT_1);
            BytesN::from_array(&env, &bytes)
        };

        // Convert BN254_CONTROL_ID to BytesN<32>
        let bn254_control_id: BytesN<32> = BytesN::from_array(&env, &Self::BN254_CONTROL_ID);

        // Create public signals as Fr field elements
        let mut pub_signals = Vec::new(&env);
        pub_signals.push_back(Fr {
            value: control_root_0,
        });
        pub_signals.push_back(Fr {
            value: control_root_1,
        });
        pub_signals.push_back(Fr { value: claim_0 });
        pub_signals.push_back(Fr { value: claim_1 });
        pub_signals.push_back(Fr {
            value: bn254_control_id,
        });

        // Verify the proof and panic if invalid
        match Self::verify_proof(seal.proof, pub_signals) {
            Ok(true) => {} // Proof is valid
            Ok(false) => panic!("Proof verification failed"),
            Err(e) => panic!("Proof verification error: {:?}", e),
        }
    }
}

/// Splits a digest into two 32-byte parts after reversing byte order.
///
/// This function reverses the byte order of the input digest and splits it into
/// two 32-byte values (zero-padded on the left), matching Solidity's convention
/// where claim_0 gets the upper 128 bits and claim_1 gets the lower 128 bits.
///
/// # Parameters
///
/// - `digest`: A 32-byte digest to split
///
/// # Returns
///
/// A tuple of two 32-byte values: (upper 128 bits, lower 128 bits) zero-padded
fn split_digest(env: &Env, digest: BytesN<32>) -> (BytesN<32>, BytesN<32>) {
    // Get the digest as a byte array
    let mut bytes = digest.to_array();

    // Reverse the byte order (equivalent to reverseByteOrderUint256)
    bytes.reverse();

    // Split into two 16-byte parts and convert to 32-byte (zero-padded on left)
    // Note: Solidity assigns upper bits to claim_0, lower bits to claim_1
    let mut claim_0 = [0u8; 32];
    let mut claim_1 = [0u8; 32];

    // Copy the upper 16 bytes to claim_0 (zero-pad left)
    claim_0[16..32].copy_from_slice(&bytes[16..32]);
    // Copy the lower 16 bytes to claim_1 (zero-pad left)
    claim_1[16..32].copy_from_slice(&bytes[0..16]);

    (
        BytesN::from_array(env, &claim_0),
        BytesN::from_array(env, &claim_1),
    )
}
