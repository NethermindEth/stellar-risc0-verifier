#![no_std]

use soroban_sdk::{Bytes, BytesN, Env, contract, contractimpl};

use risc0_interface::RiscZeroVerifierInterface;

const SELECTOR: [u8; 4] = [0, 0, 0, 0];

#[contract]
pub struct RiscZeroMockVerifier;

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
impl RiscZeroMockVerifier {}

impl RiscZeroVerifierInterface for RiscZeroMockVerifier {
    type Proof = ();

    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
        todo!()
    }

    fn verify_integrity(env: Env, receipt: risc0_interface::Receipt) {}
}
