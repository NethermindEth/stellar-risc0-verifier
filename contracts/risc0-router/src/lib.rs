//! # RISC Zero Verifier Router
//!
//! Routes verification requests to the appropriate verifier contract based on
//! a 4-byte **selector** prefix in the seal.
//!
//! ## Overview
//!
//! The router provides a **stable verification endpoint** for applications.
//! Behind it, individual verifier implementations can be added or permanently
//! removed without changing the address that applications integrate against.
//!
//! ## Selector Routing
//!
//! When an application calls `verify(seal, image_id, journal_digest)`:
//!
//! 1. The router extracts `seal[0..4]` as the selector.
//! 2. It looks up the verifier address mapped to that selector.
//! 3. It forwards the call to that verifier (typically through an
//!    emergency-stop proxy).
//!
//! This design allows **multiple verifier versions to coexist**. When RISC Zero
//! updates its parameters, a new verifier is deployed with a new selector.
//! Applications using the new parameters automatically route to the new
//! verifier; applications using older parameters continue to work.
//!
//! ## Selector Lifecycle
//!
//! Each selector has three possible states:
//!
//! - **Unset** -- never registered (`verifiers()` returns `None`)
//! - **Active** -- routes to a verifier address
//! - **Tombstone** -- permanently removed, cannot be re-registered
//!
//! ## Ownership
//!
//! The router is owned by the timelock controller. All privileged operations
//! (`add_verifier`, `remove_verifier`) require owner authorization, which in
//! production means they must go through the timelock's schedule/execute flow.
//!
//! ## Related Crates
//!
//! - [`risc0_interface`] -- trait definitions and receipt types
//! - `groth16-verifier` -- production Groth16 verifier implementation
//! - `emergency-stop` -- pausable wrapper for emergency response
//! - `timelock` -- governance controller (typically the router owner)

#![no_std]

use risc0_interface::{
    Receipt, RiscZeroVerifierClient, RiscZeroVerifierRouterInterface, VerifierEntry, VerifierError,
};
use soroban_sdk::{Address, Bytes, BytesN, Env, contract, contractimpl, contracttype};
use stellar_access::ownable::{Ownable, set_owner};
use stellar_macros::only_owner;

#[cfg(test)]
mod test;

/// Approximate number of ledgers per day (5-second close time).
const DAY_IN_LEDGERS: u32 = 17_280;

/// TTL extension amount for persistent storage (90 days).
const VERIFIER_EXTEND_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;

/// TTL threshold that triggers an extension when storage is accessed.
const VERIFIER_TTL_THRESHOLD: u32 = VERIFIER_EXTEND_AMOUNT - DAY_IN_LEDGERS;

/// Storage keys for the verifier router.
#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Mapping from a 4-byte selector to a [`VerifierEntry`].
    Verifier(BytesN<4>),
}

/// Routes verification requests to selector-specific verifier contracts.
///
/// Implements [`RiscZeroVerifierRouterInterface`] for verification dispatch and
/// [`Ownable`] for access control. The owner (typically a timelock controller)
/// can add and remove verifier mappings.
#[contract]
pub struct RiscZeroVerifierRouter;

#[contractimpl]
impl RiscZeroVerifierRouter {
    /// Reads a verifier entry from persistent storage and refreshes its TTL.
    ///
    /// Returns `None` if no entry exists for the given key (the selector has
    /// never been registered).
    fn read_verifier_entry(env: &Env, key: &DataKey) -> Option<VerifierEntry> {
        env.storage().persistent().get(key).inspect(|_| {
            env.storage().persistent().extend_ttl(
                key,
                VERIFIER_TTL_THRESHOLD,
                VERIFIER_EXTEND_AMOUNT,
            );
        })
    }

    /// Initializes the router with the owner that can manage verifiers.
    ///
    /// The owner is typically a timelock controller contract address, so
    /// that all verifier management operations go through the timelock's
    /// schedule/execute flow.
    pub fn __constructor(env: Env, owner: Address) {
        set_owner(&env, &owner);
    }

    /// Registers a verifier for the given selector.
    ///
    /// Once registered, any seal beginning with this selector will be routed
    /// to the specified verifier address.
    ///
    /// # Authorization
    ///
    /// Requires `owner.require_auth()`.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::SelectorRemoved`] -- the selector was previously
    ///   removed (tombstoned) and cannot be re-registered
    /// - [`VerifierError::SelectorInUse`] -- the selector is already mapped to
    ///   an active verifier
    #[only_owner]
    pub fn add_verifier(
        env: Env,
        selector: BytesN<4>,
        verifier: Address,
    ) -> Result<(), VerifierError> {
        let key = DataKey::Verifier(selector);
        let verifier_address: Option<VerifierEntry> = env.storage().persistent().get(&key);

        if let Some(entry) = verifier_address {
            match entry {
                VerifierEntry::Tombstone => return Err(VerifierError::SelectorRemoved),
                VerifierEntry::Active(_) => return Err(VerifierError::SelectorInUse),
            }
        }

        env.storage()
            .persistent()
            .set(&key, &VerifierEntry::Active(verifier));

        Ok(())
    }

    /// Permanently removes a verifier for the given selector.
    ///
    /// The selector is marked as a tombstone and can **never** be
    /// re-registered, even with the same verifier address. This prevents a
    /// compromised governance key from silently replacing a verifier.
    ///
    /// # Authorization
    ///
    /// Requires `owner.require_auth()`.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::SelectorUnknown`] -- the selector has never been
    ///   registered
    #[only_owner]
    pub fn remove_verifier(env: Env, selector: BytesN<4>) -> Result<(), VerifierError> {
        let key = DataKey::Verifier(selector);
        let verifier_address: Option<VerifierEntry> = env.storage().persistent().get(&key);

        if verifier_address.is_none() {
            return Err(VerifierError::SelectorUnknown);
        }

        env.storage()
            .persistent()
            .set(&key, &VerifierEntry::Tombstone);

        Ok(())
    }

    /// Resolves a selector to a verifier address.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::SelectorRemoved`] -- selector is tombstoned
    /// - [`VerifierError::SelectorUnknown`] -- selector was never registered
    fn get_verifier(env: &Env, selector: &BytesN<4>) -> Result<Address, VerifierError> {
        let key = DataKey::Verifier(selector.clone());
        let verifier_address: Option<VerifierEntry> = Self::read_verifier_entry(env, &key);

        match verifier_address {
            Some(VerifierEntry::Tombstone) => Err(VerifierError::SelectorRemoved),
            Some(VerifierEntry::Active(address)) => Ok(address),
            None => Err(VerifierError::SelectorUnknown),
        }
    }
}

#[contractimpl]
impl RiscZeroVerifierRouterInterface for RiscZeroVerifierRouter {
    /// Returns the verifier address for a selector, reverting if unknown or
    /// removed.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::SelectorRemoved`] -- selector is tombstoned
    /// - [`VerifierError::SelectorUnknown`] -- selector was never registered
    fn get_verifier_by_selector(env: Env, selector: BytesN<4>) -> Result<Address, VerifierError> {
        Self::get_verifier(&env, &selector)
    }

    /// Returns the raw [`VerifierEntry`] for a selector.
    ///
    /// Unlike `get_verifier_by_selector`, this method never reverts.
    /// Returns `None` when a selector has never been set, allowing callers
    /// to distinguish "unset" vs "active" vs "tombstoned".
    fn verifiers(env: Env, selector: BytesN<4>) -> Option<VerifierEntry> {
        let key = DataKey::Verifier(selector);
        Self::read_verifier_entry(&env, &key)
    }

    /// Returns the verifier address for the selector embedded in the seal.
    ///
    /// Extracts the first 4 bytes of `seal` as the selector and resolves
    /// it to a verifier address.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::MalformedSeal`] -- seal is shorter than 4 bytes
    /// - [`VerifierError::SelectorRemoved`] -- selector is tombstoned
    /// - [`VerifierError::SelectorUnknown`] -- selector was never registered
    fn get_verifier_from_seal(env: Env, seal: Bytes) -> Result<Address, VerifierError> {
        let selector = selector_from_seal(&seal)?;
        Self::get_verifier(&env, &selector)
    }

    /// Verifies a receipt by routing to the selector-specific verifier.
    ///
    /// Extracts the selector from `seal[0..4]`, resolves the verifier
    /// address, and forwards the full verification call.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::MalformedSeal`] -- seal is shorter than 4 bytes
    /// - [`VerifierError::SelectorRemoved`] -- selector is tombstoned
    /// - [`VerifierError::SelectorUnknown`] -- selector was never registered
    /// - Any error from the underlying verifier (e.g. `InvalidProof`)
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let selector = selector_from_seal(&seal)?;
        let verifier = Self::get_verifier(&env, &selector)?;
        let verifier = RiscZeroVerifierClient::new(&env, &verifier);
        verifier.verify(&seal, &image_id, &journal);
        Ok(())
    }

    /// Verifies receipt integrity by routing to the selector-specific verifier.
    ///
    /// Extracts the selector from the receipt's seal prefix, resolves the
    /// verifier address, and forwards the integrity check.
    ///
    /// # Errors
    ///
    /// - [`VerifierError::MalformedSeal`] -- seal is shorter than 4 bytes
    /// - [`VerifierError::SelectorRemoved`] -- selector is tombstoned
    /// - [`VerifierError::SelectorUnknown`] -- selector was never registered
    /// - Any error from the underlying verifier (e.g. `InvalidProof`)
    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let selector = selector_from_seal(&receipt.seal)?;
        let verifier = Self::get_verifier(&env, &selector)?;
        let verifier = RiscZeroVerifierClient::new(&env, &verifier);
        verifier.verify_integrity(&receipt);
        Ok(())
    }
}

/// Extracts the 4-byte selector from the seal prefix.
///
/// # Errors
///
/// Returns [`VerifierError::MalformedSeal`] if the seal is shorter than 4
/// bytes.
fn selector_from_seal(seal: &Bytes) -> Result<BytesN<4>, VerifierError> {
    if seal.len() < 4 {
        return Err(VerifierError::MalformedSeal);
    }
    Ok(seal.slice(0..4).try_into().unwrap())
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierRouter {}
