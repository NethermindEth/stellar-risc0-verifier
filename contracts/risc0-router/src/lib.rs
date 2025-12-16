#![no_std]
use soroban_sdk::{Bytes, BytesN, Env, contract, contracterror, contractimpl, contracttype};

use risc0_interface::{Receipt, RiscZeroVerifierInterface};

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Administrator address with permissions to change the router
    Admin,
    Verifier(BytesN<4>),
}

#[contracttype]
enum VerifierEntry {
    Active(Address),
    Tombstone,
}

// FIXME: Remove when PR#11 is merged
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VerifierError {
    AlreadyInitialized = 4,
    SelectorRemoved = 5,
    SelectorInUse = 6,
    SelectorUnknown = 7,
}

#[contract]
pub struct RiscZeroVerifierRouter;

// This is a sample contract. Replace this placeholder with your own contract logic.
// A corresponding test example is available in `test.rs`.
//
// For comprehensive examples, visit <https://github.com/stellar/soroban-examples>.
// The repository includes use cases for the Stellar ecosystem, such as data storage on
// the blockchain, token swaps, liquidity pools, and more.
//
// Refer to the official documentation:
// <https://developers.stellar.org/docs/build/smart-contracts/overview>.
#[contractimpl]
impl RiscZeroVerifierRouter {
    fn is_initialized(env: &Env) -> bool {
        env.storage().persistent().has(&DataKey::Admin)
    }

    pub fn init(env: Env, admin: Address) -> Result<(), VerifierError> {
        if Self::is_initialized(&env) {
            return Err(VerifierError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);

        Ok(())
    }
}

impl RiscZeroVerifierInterface for RiscZeroVerifierRouter {
    type Proof = ();

    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
        todo!()
    }

    fn verify_integrity(env: Env, receipt: Receipt) {
        todo!()
    }
}

fn require_admin(env: &Env) {
    let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
    admin.require_auth();
}
