//! # RISC Zero Verifier Interface
//!
//! This crate defines the standard interface for verifying RISC Zero zkVM receipts on Soroban.
//!
//! ## Core Components
//!
//! - [`Receipt`]: Contains a seal (cryptographic proof) and a claim digest
//! - [`Seal`]: The zero-knowledge proof (SNARK) that attests to correct execution
//! - [`ImageId`]: Identifies the guest program that was executed
//! - [`JournalDigest`]: SHA-256 hash of the public outputs from the execution

#![no_std]

use soroban_sdk::{Env, contractclient};

// Re-export types at crate root for convenience
pub use types::{
    ExitCode, ImageId, JournalDigest, Output, Receipt, ReceiptClaim, Seal, SystemExitCode,
};

pub mod types;

/// Verifier interface for RISC Zero zkVM receipts of execution.
///
/// This trait defines the standard interface that all RISC Zero verifier contracts must
/// implement on Soroban. Currently, only the Groth16 proof system is supported.
#[contractclient(name = "RiscZeroVerifierClient")]
pub trait RiscZeroVerifierInterface {
    /// The cryptographic proof system used by this verifier (e.g., Groth16).
    type Proof;

    /// Verifies a RISC Zero proof with standard execution parameters.
    ///
    /// This is a convenience method for the common case where a guest program executes
    /// successfully with no special configuration. It constructs and verifies a receipt claim
    /// with the following assumptions:
    ///
    /// - **Input hash**: All zeros (no committed input to the guest program)
    /// - **Exit code**: (SystemExitCode::Halted, 0) indicating successful completion
    /// - **Assumptions**: None (the receipt is unconditional and doesn't depend on other proofs)
    ///
    /// # Parameters
    ///
    /// - `env`: The Soroban environment providing access to cryptographic primitives
    /// - `seal`: The encoded zero-knowledge proof (SNARK) as raw bytes
    /// - `image_id`: A 32-byte identifier uniquely identifying the guest program that was executed
    /// - `journal`: The SHA-256 digest of the journal bytes (public outputs from the guest program)
    ///
    /// # Verification Process
    ///
    /// 1. Constructs a `ReceiptClaim` using the provided image ID and journal digest
    /// 2. Computes the claim digest according to RISC Zero's specification
    /// 3. Verifies the seal is a valid cryptographic proof for this claim
    ///
    /// # Returns
    ///
    /// This method returns nothing on success (unit type).
    ///
    /// # Panics
    ///
    /// Panics if any of the following occur:
    /// - The seal is malformed or cannot be decoded
    /// - The cryptographic verification fails
    /// - The proof does not correspond to the specified image ID and journal
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Verify a proof from a RISC Zero guest program
    /// verifier.verify(
    ///     &env,
    ///     seal,           // The proof bytes
    ///     image_id,       // Program identifier
    ///     journal_digest, // Hash of public outputs
    /// );
    /// ```
    fn verify(env: Env, seal: Seal, image_id: ImageId, journal: JournalDigest);

    /// Verifies a full RISC Zero receipt with arbitrary claim parameters.
    ///
    /// This method provides complete verification of a receipt, including validation of
    /// the claim digest. Unlike `verify()`, this method supports receipts with:
    ///
    /// - Custom input commitments
    /// - Non-standard exit codes
    /// - Assumptions (conditional receipts that depend on other proofs)
    ///
    /// # Parameters
    ///
    /// - `env`: The Soroban environment providing access to cryptographic primitives
    /// - `receipt`: A complete receipt containing:
    ///   - `seal`: The zero-knowledge proof (SNARK)
    ///   - `claim_digest`: The SHA-256 hash of the `ReceiptClaim` struct
    ///
    /// # Important: Claim Digest Validation
    ///
    /// The `claim_digest` field **must** be correctly computed by the caller. This is similar
    /// to how ECDSA signature verification requires the message hash to be computed correctly.
    /// An incorrect claim digest will result in verification failure even if the seal itself
    /// is valid.
    ///
    /// Use `ReceiptClaim::new(env, image_id, journal_digest).digest(env)` for standard
    /// successful executions.
    ///
    /// # Verification Process
    ///
    /// 1. Validates the receipt structure
    /// 2. Verifies the seal is a valid cryptographic proof
    /// 3. Ensures the proof corresponds to the claim digest in the receipt
    ///
    /// # Returns
    ///
    /// This method returns nothing on success (unit type).
    ///
    /// # Panics
    ///
    /// Panics if any of the following occur:
    /// - The receipt is malformed
    /// - The seal cannot be decoded
    /// - The cryptographic verification fails
    /// - The claim digest doesn't match the proven claim
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use risc0_verifier_interface::{Receipt, ReceiptClaim};
    ///
    /// // Create a receipt with the correct claim digest
    /// let claim = ReceiptClaim::new(&env, image_id, journal_digest);
    /// let receipt = Receipt {
    ///     seal: seal,
    ///     claim_digest: claim.digest(&env),
    /// };
    ///
    /// // Verify the full receipt
    /// verifier.verify_integrity(&env, receipt);
    /// ```
    fn verify_integrity(env: Env, receipt: Receipt);
}
