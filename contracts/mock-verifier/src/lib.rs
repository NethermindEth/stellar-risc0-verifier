#![no_std]

use soroban_sdk::{Bytes, BytesN, Env, contract, contractimpl};

use risc0_interface::{Receipt, ReceiptClaim, RiscZeroVerifierInterface, VerifierError};

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

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroMockVerifier {
    type Proof = ();

    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal);
        let receipt = Receipt {
            seal,
            claim_digest: claim.digest(&env),
        };
        Self::verify_integrity(env, receipt)
    }

    fn verify_integrity(env: Env, receipt: risc0_interface::Receipt) -> Result<(), VerifierError> {
        let selector = receipt.seal.slice(0..4);

        if selector != Bytes::from_array(&env, &SELECTOR) {
            return Err(VerifierError::InvalidSelector);
        }

        let selector_hash = env.crypto().keccak256(&selector).to_bytes();
        let claim_hash = env
            .crypto()
            .keccak256(&receipt.claim_digest.into())
            .to_bytes();

        if selector_hash != claim_hash {
            return Err(VerifierError::InvalidProof);
        }

        Ok(())
    }
}
