#![no_std]

use soroban_sdk::{Env, contractclient};

// Re-export types at crate root for convenience
pub use types::{ImageId, JournalDigest, Receipt, Seal};

pub mod types;

/// Verifier interface for RISC Zero receipts of execution.
#[contractclient(name = "RiscZeroVerifierClient")]
pub trait RiscZeroVerifierInterface {
    /// The cryptographic proof system used by this verifier (e.g., Groth16).
    type Proof;

    /// Verifies that the given seal is a valid RISC Zero proof of execution with the
    /// given image ID and journal digest.
    ///
    /// This method additionally ensures that the input hash is all-zeros (i.e. no
    /// committed input), the exit code is (Halted, 0), and there are no assumptions (i.e. the
    /// receipt is unconditional).
    ///
    /// # Parameters
    ///
    /// - `seal`: The encoded cryptographic proof (i.e. SNARK)
    /// - `image_id`: The identifier for the guest program
    /// - `journal`: The SHA-256 digest of the journal bytes
    ///
    /// # Panics
    ///
    /// Panics if the seal is invalid or verification fails.
    fn verify(env: Env, seal: Seal, image_id: ImageId, journal: JournalDigest);

    /// Verifies that the given receipt is a valid RISC Zero receipt, ensuring the seal is
    /// a valid cryptographic proof of the execution with the given claim.
    ///
    /// # Parameters
    ///
    /// - `receipt`: The receipt to be verified
    ///
    /// # Panics
    ///
    /// Panics if the receipt is invalid or verification fails.
    fn verify_integrity(receipt: Receipt);
}
