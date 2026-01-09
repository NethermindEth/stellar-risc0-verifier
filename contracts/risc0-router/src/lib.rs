#![no_std]

use risc0_interface::{
    Receipt, ReceiptClaim, RiscZeroVerifierClient, RiscZeroVerifierInterface, VerifierError,
};
use soroban_sdk::{Address, Bytes, BytesN, Env, contract, contractimpl, contracttype};

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Administrator address with permissions to change the router
    Admin,
    /// Selector-specific verifier entry.
    Verifier(BytesN<4>),
}

#[contracttype]
enum VerifierEntry {
    /// Active verifier for the selector.
    Active(Address),
    /// Selector is permanently removed.
    Tombstone,
}

#[contract]
/// Routes verification requests to selector-specific verifier contracts.
pub struct RiscZeroVerifierRouter;

#[contractimpl]
impl RiscZeroVerifierRouter {
    /// Initializes the router with the admin that can manage verifiers.
    pub fn __constructor(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Adds a verifier for the selector.
    pub fn add_verifier(
        env: Env,
        selector: BytesN<4>,
        verifier: Address,
    ) -> Result<(), VerifierError> {
        require_admin(&env);

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

    /// Returns the verifier for a selector.
    fn get_verifier(env: &Env, selector: &BytesN<4>) -> Result<Address, VerifierError> {
        let key = DataKey::Verifier(selector.clone());
        let verifier_address: Option<VerifierEntry> = env.storage().persistent().get(&key);

        match verifier_address {
            Some(VerifierEntry::Tombstone) => Err(VerifierError::SelectorRemoved),
            Some(VerifierEntry::Active(address)) => Ok(address),
            None => Err(VerifierError::SelectorUnknown),
        }
    }

    /// Returns the verifier for a selector.
    pub fn get_verifier_by_selector(
        env: Env,
        selector: BytesN<4>,
    ) -> Result<Address, VerifierError> {
        Self::get_verifier(&env, &selector)
    }

    /// Returns the verifier for the selector stored in the seal prefix.
    pub fn get_verifier_from_seal(env: Env, seal: Bytes) -> Result<Address, VerifierError> {
        let selector = selector_from_seal(&seal);
        Self::get_verifier(&env, &selector)
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroVerifierRouter {
    type Proof = ();

    /// Verifies a receipt from its components.
    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
        let claim = ReceiptClaim::new(&env, image_id, journal);
        let receipt = Receipt {
            seal,
            claim_digest: claim.digest(&env),
        };
        Self::verify_integrity(env, receipt);
    }

    /// Verifies receipt integrity using the selector's verifier.
    fn verify_integrity(env: Env, receipt: Receipt) {
        let selector = selector_from_seal(&receipt.seal);
        let verifier = Self::get_verifier(&env, &selector).unwrap();
        let verifier = RiscZeroVerifierClient::new(&env, &verifier);
        verifier.verify_integrity(&receipt);
    }
}

/// Ensures the stored admin authorized the call.
fn require_admin(env: &Env) {
    let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
    admin.require_auth();
}

/// Extracts the 4-byte selector from the seal prefix.
fn selector_from_seal(seal: &Bytes) -> BytesN<4> {
    seal.slice(0..4).try_into().unwrap()
}
