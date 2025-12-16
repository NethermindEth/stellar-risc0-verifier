#![no_std]
use soroban_sdk::{Address, Bytes, BytesN, Env, contract, contractimpl, contracttype};

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
impl RiscZeroVerifierRouter {}

impl RiscZeroVerifierInterface for RiscZeroVerifierRouter {
    type Proof = ();

    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
        todo!()
    }

    fn verify_integrity(env: Env, receipt: Receipt) {
        todo!()
    }
}
