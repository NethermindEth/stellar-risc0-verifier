//! # RISC Zero Receipt Types
//!
//! This module defines the core data structures used for RISC Zero proof verification
//! on Soroban. These types represent the cryptographic proofs and claims that attest to the
//! correct execution of guest programs.
//!
//! ## Type Overview
//!
//! - [`ImageId`]: Uniquely identifies a guest program
//! - [`JournalDigest`]: Hash of the public outputs from execution
//! - [`Seal`]: The zero-knowledge proof itself
//! - [`Receipt`]: Complete proof package with seal and claim
//! - [`ReceiptClaim`]: Detailed execution claim including state and exit codes
//!
//! ## Verification Flow
//!
//! 1. The prover executes off-chain, producing a journal (public outputs) and cryptographic proof
//! 2. A [`Receipt`] is constructed with the [`Seal`] (proof) and a `claim_digest` (hash of the [`ReceiptClaim`])
//! 3. The receipt is submitted to a Soroban verifier contract for validation
//! 4. The verifier cryptographically validates that the seal proves the claim

use soroban_sdk::{Bytes, BytesN, Env, bytesn, contracttype};

/// Identifier for a RISC Zero guest program.
///
/// This is a 32-byte digest that uniquely identifies the compiled guest program binary.
/// It serves as the "pre-state digest" in the RISC Zero proof system, ensuring that
/// the proof corresponds to execution of a specific, known program.
///
/// The image ID is deterministically derived from the guest program's ELF binary and
/// is stable across builds if the program logic remains unchanged.
pub type ImageId = BytesN<32>;

/// SHA-256 digest of the journal bytes.
///
/// The journal contains the public outputs of a guest program's execution. This 32-byte
/// digest is computed over the raw journal bytes and becomes part of the receipt claim.
///
/// # Security Note
///
/// The journal digest must be computed correctly using SHA-256. An incorrect digest
/// will cause verification to fail even if the seal is valid.
pub type JournalDigest = BytesN<32>;

/// Encoded cryptographic proof (SNARK) as raw bytes.
///
/// The seal is a zero-knowledge proof that attests to correct execution of a guest program.
/// It contains the cryptographic evidence that can be efficiently verified on-chain without
/// revealing the execution trace or private inputs.
///
/// The seal format depends on the proof system used (e.g., Groth16). It is serialized as
/// raw bytes for storage and transmission in Soroban contracts.
pub type Seal = Bytes;

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
    pub seal: Seal,
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
/// - **pre_state_digest**: The [`ImageId`] of the guest program
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
    journal_digest: JournalDigest,
    /// SHA-256 digest of assumptions (dependencies on other receipts).
    ///
    /// For unconditional receipts (the common case), this is the zero digest.
    assumptions_digest: BytesN<32>,
}

impl Output {
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
        let tag_bytes = Bytes::from_slice(env, b"risc0.Output");
        let tag_digest = env.crypto().sha256(&tag_bytes);

        let mut data = Bytes::new(env);
        data.append(&tag_digest.into());
        data.append(&self.journal_digest.clone().into());
        data.append(&self.assumptions_digest.clone().into());

        let length_bytes = Bytes::from_array(env, &[0x02, 0x00]);
        data.append(&length_bytes);

        env.crypto().sha256(&data).into()
    }
}

impl ReceiptClaim {
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
    pub fn new(env: &Env, image_id: ImageId, journal_digest: JournalDigest) -> Self {
        let output = Output {
            journal_digest,
            assumptions_digest: BytesN::from_array(env, &[0u8; 32]),
        };
        let post_state: BytesN<32> = bytesn!(
            env,
            0xa3acc27117418996340b84e5a90f3ef4c49d22c79e44aad822ec9c313e1eb8e2
        );

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
        let tag_bytes = Bytes::from_slice(env, b"risc0.ReceiptClaim");
        let tag_digest = env.crypto().sha256(&tag_bytes);

        let mut data = Bytes::new(env);
        data.append(&tag_digest.into());
        data.append(&self.input.clone().into());
        data.append(&self.pre_state_digest.clone().into());
        data.append(&self.post_state_digest.clone().into());
        data.append(&self.output.clone().into());

        // uint32(claim.exitCode.system) << 24
        let system_exit_code = (self.exit_code.system as u32) << 24;
        let system_bytes = Bytes::from_array(env, &system_exit_code.to_be_bytes());
        data.append(&system_bytes);

        // uint32(claim.exitCode.user) << 24 - user is BytesN<8>, take first 4 bytes as u32
        let user_bytes = self.exit_code.user.to_array();
        let user_u32 =
            u32::from_be_bytes([user_bytes[0], user_bytes[1], user_bytes[2], user_bytes[3]]);
        let user_shifted = user_u32 << 24;
        let user_shifted_bytes = Bytes::from_array(env, &user_shifted.to_be_bytes());
        data.append(&user_shifted_bytes);

        // uint16(4) << 8 - down.length
        let length_bytes = Bytes::from_array(env, &[0x04, 0x00]);
        data.append(&length_bytes);

        env.crypto().sha256(&data).into()
    }
}
