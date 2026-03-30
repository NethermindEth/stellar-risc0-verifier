//! # Mock Verifier for RISC Zero (Testing Only)
//!
//! **WARNING: This contract provides NO cryptographic security. Use it only for
//! local development, integration tests, and end-to-end testing flows.**
//!
//! ## Purpose
//!
//! The mock verifier implements [`RiscZeroVerifierInterface`] without
//! performing any real proof verification. It is the Soroban counterpart of the
//! Ethereum mock verifier used with `DEV_MODE=1` in the RISC Zero toolchain.
//!
//! ## Seal Format
//!
//! Mock seals follow the format: `selector || claim_digest` (36 bytes total).
//! Verification checks that `keccak256(seal[4..]) == keccak256(claim_digest)`,
//! which is trivially satisfied when the seal is produced by
//! [`RiscZeroMockVerifier::mock_prove`] or
//! [`RiscZeroMockVerifier::mock_prove_claim`].
//!
//! ## Typical Usage
//!
//! ```text
//! 1. Deploy mock verifier with a chosen selector
//! 2. Register it in the router
//! 3. Use `mock_prove()` to generate test receipts
//! 4. Submit receipts through the normal verification flow
//! ```
//!
//! ## Related Crates
//!
//! - [`risc0_interface`] -- trait definition and receipt types
//! - `groth16-verifier` -- production Groth16 verifier

#![no_std]

use soroban_sdk::{Bytes, BytesN, Env, contract, contractimpl, contracttype};

use risc0_interface::{Receipt, ReceiptClaim, RiscZeroVerifierInterface, VerifierError};

#[cfg(test)]
mod test;

/// Approximate number of ledgers per day (5-second close time).
const DAY_IN_LEDGERS: u32 = 17_280;

/// TTL extension amount for persistent storage (90 days).
const VERIFIER_EXTEND_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;

/// TTL threshold that triggers an extension when storage is accessed.
const VERIFIER_TTL_THRESHOLD: u32 = VERIFIER_EXTEND_AMOUNT - DAY_IN_LEDGERS;

/// Storage keys for the mock verifier.
#[contracttype]
enum DataKey {
    /// The 4-byte selector identifying this verifier in the router.
    Selector,
}

/// Reads the selector from persistent storage and refreshes its TTL.
///
/// Returns [`VerifierError::InvalidSelector`] if the selector has not been set
/// (i.e., the contract was not properly initialized).
fn read_selector(env: &Env) -> Result<Bytes, VerifierError> {
    let key = DataKey::Selector;
    env.storage()
        .persistent()
        .get(&key)
        .inspect(|_| {
            env.storage().persistent().extend_ttl(
                &key,
                VERIFIER_TTL_THRESHOLD,
                VERIFIER_EXTEND_AMOUNT,
            );
        })
        .ok_or(VerifierError::InvalidSelector)
}

/// Mock verifier intended only for development with RISC Zero `DEV_MODE=1`.
///
/// **DANGER: DO NOT deploy this contract in production.** It provides no
/// cryptographic security guarantees and will accept any receipt that matches
/// the mock seal format (`selector || claim_digest`).
///
/// This verifier is useful for:
///
/// - Local development where real proofs are not yet available
/// - Integration tests that exercise the full verification flow
/// - End-to-end testing without the overhead of proof generation
///
/// Unlike the Groth16 verifier, this contract stores its
/// selector in persistent storage (set at construction time) rather than
/// embedding it at compile time.
#[contract]
pub struct RiscZeroMockVerifier;

#[contractimpl]
impl RiscZeroMockVerifier {
    /// Initializes the mock verifier with the given selector.
    ///
    /// The selector is stored in persistent storage and must match the first
    /// 4 bytes of any seal submitted for verification.
    pub fn __constructor(env: Env, selector: BytesN<4>) {
        let selector: Bytes = selector.into();
        env.storage()
            .persistent()
            .set(&DataKey::Selector, &selector);
    }

    /// Returns the configured selector as `BytesN<4>`.
    ///
    /// Returns [`VerifierError::InvalidSelector`] if the stored value is
    /// missing or malformed.
    pub fn selector(env: Env) -> Result<BytesN<4>, VerifierError> {
        let selector = read_selector(&env)?;
        BytesN::try_from(&selector).map_err(|_| VerifierError::InvalidSelector)
    }

    /// Build a mock receipt for the given image ID and journal digest.
    ///
    /// The seal format matches the Ethereum mock verifier: `selector ||
    /// claim_digest`.
    pub fn mock_prove(
        env: Env,
        image_id: BytesN<32>,
        journal_digest: BytesN<32>,
    ) -> Result<Receipt, VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal_digest);
        let claim_digest = claim.digest(&env);
        Self::mock_prove_claim(env, claim_digest)
    }

    /// Build a mock receipt for a precomputed claim digest.
    ///
    /// The seal format matches the Ethereum mock verifier: `selector ||
    /// claim_digest`.
    pub fn mock_prove_claim(env: Env, claim_digest: BytesN<32>) -> Result<Receipt, VerifierError> {
        let selector = read_selector(&env)?;
        let mut seal = Bytes::new(&env);
        seal.append(&selector);
        seal.append(&Bytes::from_array(&env, &claim_digest.to_array()));

        Ok(Receipt { seal, claim_digest })
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroMockVerifier {
    type Proof = ();

    /// Verifies a mock seal by reconstructing the claim digest from inputs.
    ///
    /// Constructs a standard [`ReceiptClaim`] from the image ID and journal
    /// digest, then delegates to `verify_integrity`.
    ///
    /// # Errors
    ///
    /// Returns a [`VerifierError`] on selector mismatch or invalid proof
    /// (the claim digest encoded in the seal does not match the provided
    /// inputs).
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal);
        let receipt = Receipt {
            seal,
            claim_digest: claim.digest(&env),
        };
        Self::verify_integrity(env, receipt)
    }

    /// Verifies a mock receipt by checking the selector and claim digest.
    ///
    /// The mock verification algorithm:
    /// 1. Checks that the seal is at least 4 bytes long
    /// 2. Verifies the selector prefix matches this verifier
    /// 3. Compares `keccak256(seal[4..])` with `keccak256(claim_digest)`
    ///
    /// # Errors
    ///
    /// - [`VerifierError::MalformedSeal`] -- seal is shorter than 4 bytes
    /// - [`VerifierError::InvalidSelector`] -- selector does not match
    /// - [`VerifierError::InvalidProof`] -- claim digest mismatch
    fn verify_integrity(env: Env, receipt: risc0_interface::Receipt) -> Result<(), VerifierError> {
        if receipt.seal.len() < 4 {
            return Err(VerifierError::MalformedSeal);
        }

        let expected_selector = read_selector(&env)?;
        let selector = receipt.seal.slice(0..4);

        if selector != expected_selector {
            return Err(VerifierError::InvalidSelector);
        }

        let seal_hash = env.crypto().keccak256(&receipt.seal.slice(4..)).to_bytes();
        let claim_hash = env
            .crypto()
            .keccak256(&receipt.claim_digest.into())
            .to_bytes();

        if seal_hash != claim_hash {
            return Err(VerifierError::InvalidProof);
        }

        Ok(())
    }
}
