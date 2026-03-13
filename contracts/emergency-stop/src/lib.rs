//! # Emergency Stop for RISC Zero Verifiers
//!
//! This crate implements a pausable wrapper around a RISC Zero verifier
//! contract. It provides an immediate, **permanent** kill switch for
//! emergency response to discovered vulnerabilities.
//!
//! ## Design Rationale
//!
//! The emergency stop is **separate from the timelock** by design:
//!
//! - The timelock imposes a delay, which is valuable for governance but
//!   unacceptable for emergency response.
//! - The guardian can pause a verifier **immediately** without waiting.
//! - Once activated, the stop is **permanent** -- there is no unpause
//!   function, preventing an attacker who compromises the guardian key
//!   from toggling the stop.
//!
//! ## Architecture
//!
//! In the verification stack the emergency stop sits between the router
//! and the actual verifier:
//!
//! ```text
//! Router --> EmergencyStop (this crate) --> Groth16Verifier
//! ```
//!
//! ## Activation Paths
//!
//! 1. **Guardian call** -- the owner invokes `estop()` directly.
//! 2. **Proof of exploit** -- anyone invokes `estop_with_receipt()` with
//!    a receipt whose `claim_digest` is the zero digest. The receipt is
//!    verified against the underlying verifier to prove the exploit is real.
//!
//! ## After Activation
//!
//! Once paused, all `verify()` and `verify_integrity()` calls revert. The
//! operator should schedule a selector removal via the timelock to tombstone
//! the affected selector in the router.
//!
//! ## Related Crates
//!
//! - [`risc0_interface`] -- trait definition and receipt types
//! - `risc0-router` -- selector-based routing to verifiers
//! - `groth16-verifier` -- production Groth16 verifier
//! - `timelock` -- governance controller for the router

#![no_std]

use risc0_interface::{Receipt, RiscZeroVerifierClient, RiscZeroVerifierInterface, VerifierError};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};
use stellar_access::ownable::{self, Ownable};
use stellar_contract_utils::pausable::{self, Pausable};
use stellar_macros::{only_owner, when_not_paused};

#[cfg(test)]
mod test;

/// The all-zeros digest used as the sentinel value for proof-of-exploit
/// activation. A receipt with this claim digest indicates a vulnerability.
const ZERO_DIGEST: [u8; 32] = [0u8; 32];

/// Storage keys used by the emergency stop contract.
#[contracttype]
pub enum DataKey {
    /// Address of the underlying verifier implementation being wrapped.
    Verifier,
}

/// Errors emitted by the emergency stop wrapper.
///
/// These are in addition to the [`VerifierError`] variants that may be
/// returned by the underlying verifier.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EmergencyStopError {
    /// Caller is not authorized to perform the requested action.
    Unauthorized = 1,
    /// Verifier address is not configured (contract not initialized).
    VerifierNotSet = 5,
    /// The receipt submitted to `estop_with_receipt` does not prove a
    /// circuit-breaker exploit (its `claim_digest` is not the zero digest).
    InvalidProofOfExploit = 1001,
    /// Unpause is permanently disallowed by the emergency stop wrapper.
    UnpauseNotAllowed = 1002,
}

/// Emergency-stop wrapper for a RISC Zero verifier contract.
///
/// Implements [`RiscZeroVerifierInterface`] by delegating to an underlying
/// verifier, but adds a permanent pause mechanism controlled by a guardian
/// (the contract owner).
///
/// The contract also implements [`Ownable`] (for guardian management) and
/// [`Pausable`] (for pause state queries), with [`unpause`](Pausable::unpause)
/// permanently disabled.
#[contract]
pub struct RiscZeroVerifierEmergencyStop;

#[contractimpl]
impl RiscZeroVerifierEmergencyStop {
    /// Initializes the emergency stop wrapper.
    ///
    /// # Parameters
    ///
    /// - `verifier` -- address of the underlying verifier contract to wrap
    ///   (e.g., a Groth16 verifier)
    /// - `owner` -- address of the guardian who can trigger the emergency stop
    pub fn __constructor(env: Env, verifier: Address, owner: Address) {
        env.storage().instance().set(&DataKey::Verifier, &verifier);
        ownable::set_owner(&env, &owner);
    }

    /// Returns the address of the underlying verifier being wrapped.
    pub fn get_verifier(env: Env) -> Address {
        get_verifier(&env)
    }

    /// Permanently pauses all verification through this wrapper.
    ///
    /// Only the guardian (contract owner) can call this. Once activated, all
    /// subsequent `verify()` and `verify_integrity()` calls will revert.
    ///
    /// # Authorization
    ///
    /// Requires `owner.require_auth()`.
    ///
    /// # Panics
    ///
    /// Panics if the caller is not the owner.
    #[only_owner]
    pub fn estop(env: Env) {
        pausable::pause(&env);
    }

    /// Permanently pauses verification by submitting a proof of exploit.
    ///
    /// Anyone can call this if they can produce a valid receipt whose
    /// `claim_digest` is the zero digest (all zeros). Such a receipt
    /// indicates a vulnerability in the verifier because a zero claim
    /// digest should never be provable.
    ///
    /// # Process
    ///
    /// 1. Checks the contract is not already paused
    /// 2. Verifies `receipt.claim_digest == [0u8; 32]`
    /// 3. Forwards the receipt to the underlying verifier for validation
    /// 4. If the verifier accepts it (proving the exploit), pauses permanently
    ///
    /// # Panics
    ///
    /// - Panics with [`EmergencyStopError::InvalidProofOfExploit`] if the
    ///   receipt's claim digest is not the zero digest.
    /// - Panics if the contract is already paused.
    #[when_not_paused]
    pub fn estop_with_receipt(env: Env, receipt: Receipt) {
        let zero_digest = BytesN::from_array(&env, &ZERO_DIGEST);
        if receipt.claim_digest != zero_digest {
            panic_with_error!(&env, EmergencyStopError::InvalidProofOfExploit);
        }

        // Ensure the proof-of-exploit receipt is valid.
        let _ = Self::verify_integrity(env.clone(), receipt);

        pausable::pause(&env);
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroVerifierEmergencyStop {
    type Proof = Bytes;

    /// Forwards verification to the underlying verifier.
    ///
    /// Reverts if the contract is paused (emergency stop activated).
    #[when_not_paused]
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let verifier = get_verifier(&env);
        let client = RiscZeroVerifierClient::new(&env, &verifier);
        client.verify(&seal, &image_id, &journal);
        Ok(())
    }

    /// Forwards receipt integrity verification to the underlying verifier.
    ///
    /// Reverts if the contract is paused (emergency stop activated).
    #[when_not_paused]
    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let verifier = get_verifier(&env);
        let client = RiscZeroVerifierClient::new(&env, &verifier);
        client.verify_integrity(&receipt);
        Ok(())
    }
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierEmergencyStop {}

#[contractimpl]
impl Pausable for RiscZeroVerifierEmergencyStop {
    /// Returns whether the emergency stop is currently activated.
    fn paused(env: &Env) -> bool {
        pausable::paused(env)
    }

    /// Pauses verification. Only the owner (guardian) can call this.
    ///
    /// # Panics
    ///
    /// Panics with [`EmergencyStopError::Unauthorized`] if `caller` is not
    /// the owner.
    fn pause(env: &Env, caller: Address) {
        let owner = ownable::enforce_owner_auth(env);
        if owner != caller {
            panic_with_error!(env, EmergencyStopError::Unauthorized);
        }
        pausable::pause(env);
    }

    /// Always panics -- unpausing is permanently disallowed.
    ///
    /// # Panics
    ///
    /// Always panics with [`EmergencyStopError::UnpauseNotAllowed`].
    fn unpause(env: &Env, _caller: Address) {
        panic_with_error!(env, EmergencyStopError::UnpauseNotAllowed);
    }
}

/// Reads the underlying verifier address from instance storage.
///
/// # Panics
///
/// Panics with [`EmergencyStopError::VerifierNotSet`] if the verifier
/// address has not been configured.
fn get_verifier(env: &Env) -> Address {
    match env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Verifier)
    {
        Some(verifier) => verifier,
        None => panic_with_error!(env, EmergencyStopError::VerifierNotSet),
    }
}
