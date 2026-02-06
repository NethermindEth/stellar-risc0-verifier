//! # RISC Zero Receipt Types
//!
//! This module defines the core data structures used for RISC Zero proof verification
//! on Soroban. These types represent the cryptographic proofs and claims that attest to the
//! correct execution of guest programs.
//!
//! ## Type Overview
//!
//! - [`Receipt`]: Complete proof package with seal and claim
//! - [`ReceiptClaim`]: Detailed execution claim including state and exit codes
//!
//! ## Verification Flow
//!
//! 1. The prover executes off-chain, producing a journal (public outputs) and cryptographic proof
//! 2. A [`Receipt`] is constructed with the seal (proof) and a `claim_digest` (hash of the [`ReceiptClaim`])
//! 3. The receipt is submitted to a Soroban verifier contract for validation
//! 4. The verifier cryptographically validates that the seal proves the claim

use soroban_sdk::{Address, Bytes, BytesN, Env, contracterror, contracttype};

/// Errors that can occur during Groth16 proof verification.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VerifierError {
    /// The proof verification failed (pairing check did not equal identity).
    InvalidProof = 0,
    /// The number of public inputs does not match the verification key.
    MalformedPublicInputs = 1,
    /// The seal data is malformed or has incorrect byte length.
    MalformedSeal = 2,
    /// The selector in the seal does not match this verifier.
    InvalidSelector = 3,
    /// The contract has already been initialized.
    AlreadyInitialized = 4,
    /// The selector was removed and can no longer be assigned.
    SelectorRemoved = 5,
    /// The selector is already assigned to a verifier.
    SelectorInUse = 6,
    /// The selector is not registered.
    SelectorUnknown = 7,
}

/// A receipt attesting to a claim using the RISC Zero proof system.
///
/// A receipt is the complete proof package that can be verified on-chain. It combines
/// a cryptographic proof (seal) with a claim about what was executed.
///
/// # Structure
///
/// - **[`seal`](Receipt::seal)**: A zero-knowledge proof attesting to knowledge of a witness for the claim
/// - **[`claim_digest`](Receipt::claim_digest)**: The SHA-256 hash of a [`ReceiptClaim`] struct containing
///   execution details (program ID, journal, exit code, etc.)
///
/// # Important: Claim Digest Validation
///
/// The `claim_digest` field **must** be correctly computed by the caller for verification to
/// have meaningful security guarantees. This is similar to verifying an ECDSA signature where
/// the message hash must be computed correctly.
///
/// For standard successful executions, use:
/// ```ignore
/// let claim = ReceiptClaim::new(&env, image_id, journal_digest);
/// let claim_digest = claim.digest(&env);
/// ```
///
/// # Example
///
/// ```ignore
/// use risc0_verifier_interface::{Receipt, ReceiptClaim, Seal};
///
/// let claim = ReceiptClaim::new(&env, image_id, journal_digest);
/// let receipt = Receipt {
///     seal: seal,
///     claim_digest: claim.digest(&env),
/// };
/// ```
#[contracttype]
pub struct Receipt {
    /// The zero-knowledge proof (SNARK) as raw bytes.
    pub seal: Bytes,
    /// SHA-256 digest of the [`ReceiptClaim`] struct.
    pub claim_digest: BytesN<32>,
}

/// A claim about the execution of a RISC Zero guest program.
///
/// This structure contains all the details about a program execution that the seal
/// cryptographically proves. It includes the program identifier, execution state,
/// exit status, and outputs.
///
/// # Fields
///
/// The claim follows RISC Zero's standard structure for zkVM execution:
///
/// - **pre_state_digest**: The image id of the guest program
/// - **post_state_digest**: Final state after execution (fixed constant for successful runs)
/// - **exit_code**: How the program terminated (system and user codes)
/// - **input**: Committed input digest (currently unused, set to zero)
/// - **output**: Digest of the [`Output`] containing journal and assumptions
///
/// # Usage
///
/// Most users should construct claims using [`ReceiptClaim::new()`] for standard
/// successful executions, which automatically sets appropriate defaults.
#[contracttype]
pub struct ReceiptClaim {
    /// Digest of the system state before execution (the program [`ImageId`]).
    ///
    /// This identifies which guest program was executed. It must match the expected
    /// program for verification to be meaningful.
    pre_state_digest: BytesN<32>,

    /// Digest of the system state after execution has completed.
    ///
    /// This is a fixed constant value
    /// (`0xa3acc27117418996340b84e5a90f3ef4c49d22c79e44aad822ec9c313e1eb8e2`)
    /// representing the halted state.
    post_state_digest: BytesN<32>,

    /// The exit code indicating how the execution terminated.
    ///
    /// Contains both a system-level code (Halted, Paused, SystemSplit) and a
    /// user-defined exit code from the guest program.
    exit_code: ExitCode,

    /// Digest of the input committed to the guest program.
    ///
    /// **Note**: This field is currently unused in the RISC Zero zkVM and must
    /// always be set to the zero digest (32 zero bytes).
    input: BytesN<32>,

    /// Digest of the execution output.
    ///
    /// This is the SHA-256 hash of an [`Output`] struct containing the journal
    /// digest and assumptions digest. See [`Output::digest()`] for the hashing scheme.
    output: BytesN<32>,
}

/// Exit code indicating how a guest program execution terminated.
///
/// The exit code consists of two parts:
/// - **System code**: Indicates the execution mode (halted, paused, or split)
/// - **User code**: Application-specific exit code (8 bytes)
///
/// For standard successful executions, the system code is [`SystemExitCode::Halted`]
/// and the user code is zero.
#[contracttype]
pub struct ExitCode {
    /// System-level exit code indicating the execution termination mode.
    system: SystemExitCode,
    /// User-defined exit code (8 bytes) set by the guest program.
    user: BytesN<8>,
}

/// System-level exit codes for RISC Zero execution.
///
/// These codes indicate different execution termination modes.
///
/// # Variants
///
/// - **Halted**: Normal termination - the program completed successfully
/// - **Paused**: Execution paused (used for continuations and multi-segment proofs)
/// - **SystemSplit**: Execution split for parallel proving
///
/// # Encoding
///
/// These values are encoded as `u32` in the receipt claim digest computation,
/// shifted left by 24 bits.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SystemExitCode {
    /// Program execution completed successfully.
    Halted = 0,
    /// Program execution paused (for continuations).
    Paused = 1,
    /// Execution split for parallel proving.
    SystemSplit = 2,
}

/// Output of a RISC Zero guest program execution.
///
/// The output contains the public results of execution (journal) and any
/// assumptions (dependencies on other proofs). This structure is hashed
/// to produce the `output` field in [`ReceiptClaim`].
///
/// # Fields
///
/// - **journal_digest**: SHA-256 hash of the journal (public outputs)
/// - **assumptions_digest**: SHA-256 hash of assumptions (zero for unconditional proofs)
#[contracttype]
pub struct Output {
    /// SHA-256 digest of the journal bytes (public outputs from the guest program).
    journal_digest: BytesN<32>,
    /// SHA-256 digest of assumptions (dependencies on other receipts).
    ///
    /// For unconditional receipts (the common case), this is the zero digest.
    assumptions_digest: BytesN<32>,
}

impl Output {
    /// Pre-computed SHA-256("risc0.Output") tag digest.
    /// This constant avoids computing the tag hash on every call.
    const TAG_DIGEST: [u8; 32] = [
        0x77, 0xea, 0xfe, 0xb3, 0x66, 0xa7, 0x8b, 0x47, 0x74, 0x7d, 0xe0, 0xd7, 0xbb, 0x17, 0x62,
        0x84, 0x08, 0x5f, 0xf5, 0x56, 0x48, 0x87, 0x00, 0x9a, 0x5b, 0xe6, 0x3d, 0xa3, 0x2d, 0x35,
        0x59, 0xd4,
    ];

    /// Computes the SHA-256 digest of this [`Output`] struct.
    ///
    /// This digest is used as the `output` field in a [`ReceiptClaim`]. The hashing
    /// scheme follows RISC Zero's tagged hash specification to prevent cross-protocol attacks.
    ///
    /// # Hash Construction
    ///
    /// The digest is computed as:
    /// ```text
    /// SHA-256(tag_digest || journal_digest || assumptions_digest || length)
    /// ```
    ///
    /// Where:
    /// - `tag_digest` = SHA-256("risc0.Output")
    /// - `length` = 0x02 0x00 (2 fields in little-endian u16)
    ///
    /// # Returns
    ///
    /// A 32-byte SHA-256 digest of the output structure.
    pub fn digest(&self, env: &Env) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_array(env, &Self::TAG_DIGEST));
        data.append(&self.journal_digest.clone().into());
        data.append(&self.assumptions_digest.clone().into());
        data.append(&Bytes::from_array(env, &[0x02, 0x00]));

        env.crypto().sha256(&data).into()
    }
}

impl ReceiptClaim {
    /// Pre-computed SHA-256("risc0.ReceiptClaim") tag digest.
    /// This constant avoids computing the tag hash on every call.
    const TAG_DIGEST: [u8; 32] = [
        0xcb, 0x1f, 0xef, 0xcd, 0x1f, 0x2d, 0x9a, 0x64, 0x97, 0x5c, 0xbb, 0xbf, 0x6e, 0x16, 0x1e,
        0x29, 0x14, 0x43, 0x4b, 0x0c, 0xbb, 0x99, 0x60, 0xb8, 0x4d, 0xf5, 0xd7, 0x17, 0xe8, 0x6b,
        0x48, 0xaf,
    ];

    /// Fixed post-state digest for a halted execution.
    ///
    /// This is a protocol constant used in standard successful receipt claims.
    const POST_STATE_DIGEST_HALTED: [u8; 32] = [
        0xa3, 0xac, 0xc2, 0x71, 0x17, 0x41, 0x89, 0x96, 0x34, 0x0b, 0x84, 0xe5, 0xa9, 0x0f, 0x3e,
        0xf4, 0xc4, 0x9d, 0x22, 0xc7, 0x9e, 0x44, 0xaa, 0xd8, 0x22, 0xec, 0x9c, 0x31, 0x3e, 0x1e,
        0xb8, 0xe2,
    ];

    /// Constructs a standard [`ReceiptClaim`] for a successful guest program execution.
    ///
    /// This convenience method creates a claim with standard assumptions suitable for
    /// most verification scenarios:
    ///
    /// - **Input**: Zero digest (no committed input)
    /// - **Exit code**: (Halted, 0) indicating successful completion
    /// - **Assumptions**: Zero digest (unconditional proof)
    /// - **Post-state**: Fixed constant for halted state
    ///
    /// # Parameters
    ///
    /// - `env`: Soroban environment for cryptographic operations
    /// - `image_id`: The 32-byte identifier of the guest program
    /// - `journal_digest`: SHA-256 digest of the journal (public outputs)
    ///
    /// # Returns
    ///
    /// A [`ReceiptClaim`] configured for standard successful execution.
    pub fn new(env: &Env, image_id: BytesN<32>, journal_digest: BytesN<32>) -> Self {
        let output = Output {
            journal_digest,
            assumptions_digest: BytesN::from_array(env, &[0u8; 32]),
        };
        let post_state: BytesN<32> = BytesN::from_array(env, &Self::POST_STATE_DIGEST_HALTED);

        Self {
            pre_state_digest: image_id,
            post_state_digest: post_state,
            exit_code: ExitCode {
                system: SystemExitCode::Halted,
                user: BytesN::from_array(env, &[0u8; 8]),
            },
            input: BytesN::from_array(env, &[0u8; 32]),
            output: output.digest(env),
        }
    }

    /// Computes the SHA-256 digest of this [`ReceiptClaim`].
    ///
    /// This digest becomes the `claim_digest` field in a [`Receipt`] and is what the
    /// cryptographic proof (seal) actually attests to. The hashing scheme follows RISC Zero's
    /// tagged hash specification.
    ///
    /// # Hash Construction
    ///
    /// The digest is computed as:
    /// ```text
    /// SHA-256(
    ///     tag_digest ||
    ///     input ||
    ///     pre_state_digest ||
    ///     post_state_digest ||
    ///     output ||
    ///     system_exit_code ||
    ///     user_exit_code ||
    ///     length
    /// )
    /// ```
    ///
    /// Where:
    /// - `tag_digest` = SHA-256("risc0.ReceiptClaim")
    /// - Exit codes are encoded as big-endian u32, shifted left by 24 bits
    /// - `length` = 0x04 0x00 (4 state fields in little-endian u16)
    ///
    /// # Parameters
    ///
    /// - `env`: Soroban environment for cryptographic operations
    ///
    /// # Returns
    ///
    /// A 32-byte SHA-256 digest that uniquely identifies this claim.
    ///
    /// # Security Note
    ///
    /// This digest must be computed correctly for verification to be secure. Always use
    /// this method rather than implementing custom hashing.
    pub fn digest(&self, env: &Env) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_array(env, &Self::TAG_DIGEST));
        data.append(&self.input.clone().into());
        data.append(&self.pre_state_digest.clone().into());
        data.append(&self.post_state_digest.clone().into());
        data.append(&self.output.clone().into());

        // System exit code encoding: (value as u32) << 24, then to_be_bytes()
        //
        // | Value           | as u32 | << 24        | to_be_bytes()             |
        // |-----------------|--------|--------------|---------------------------|
        // | Halted = 0      | 0      | 0x00000000   | [0x00, 0x00, 0x00, 0x00]  |
        // | Paused = 1      | 1      | 0x01000000   | [0x01, 0x00, 0x00, 0x00]  |
        // | SystemSplit = 2 | 2      | 0x02000000   | [0x02, 0x00, 0x00, 0x00]  |
        //
        // Shifting left by 24 bits moves the value into the MSB of the u32.
        // to_be_bytes() outputs the MSB first, so the result is [value, 0, 0, 0].
        // Since all variants fit in one byte, we write this directly.
        data.append(&Bytes::from_array(
            env,
            &[self.exit_code.system as u8, 0, 0, 0],
        ));

        // User exit code: first 4 bytes interpreted as BE u32, then << 24
        // This effectively keeps only the 4th byte (index 3) at position 0
        let user_bytes = self.exit_code.user.to_array();
        data.append(&Bytes::from_array(env, &[user_bytes[3], 0, 0, 0]));

        // Length: uint16(4) << 8 encoded as 2 bytes
        data.append(&Bytes::from_array(env, &[0x04, 0x00]));

        env.crypto().sha256(&data).into()
    }
}

/// Router mapping entry for a verifier selector.
///
/// This enum represents the raw state stored in the router mapping:
/// - `Active(Address)` means the selector routes to that verifier contract.
/// - `Tombstone` means the selector was removed and can never be reused.
///
/// The router `verifiers` getter returns `None` when a selector has never been set,
/// allowing callers to distinguish "unset" vs "removed" without relying on errors.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifierEntry {
    /// Active verifier for the selector.
    Active(Address),
    /// Selector is permanently removed.
    Tombstone,
}
