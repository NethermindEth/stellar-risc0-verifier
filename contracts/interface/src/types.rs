use soroban_sdk::{Bytes, BytesN, contracttype};

/// Identifier for a RISC Zero guest program (32 bytes).
pub type ImageId = BytesN<32>;

/// SHA-256 digest of the journal bytes (32 bytes).
pub type JournalDigest = BytesN<32>;

/// Encoded cryptographic proof (SNARK) as raw bytes.
pub type Seal = Bytes;

/// A receipt attesting to a claim using the RISC Zero proof system.
///
/// A receipt contains two parts:
/// - **Seal**: A zero-knowledge proof attesting to knowledge of a witness for the claim
/// - **Claim**: A set of public outputs; for zkVM execution, this is the hash of a `ReceiptClaim` struct
///
/// # Important
///
/// The `claim_digest` field must be a hash computed by the caller for verification to
/// have meaningful guarantees. Treat this similar to verifying an ECDSA signature, in that hashing
/// is a key operation in verification. The most common way to calculate this hash is to use
/// `ReceiptClaim::new(image_id, journal_digest).digest()` for successful executions.
#[contracttype]
pub struct Receipt {
    seal: Seal,
    claim_digest: BytesN<32>,
}
