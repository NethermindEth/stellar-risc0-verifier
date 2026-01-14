#![no_std]

use soroban_sdk::{Bytes, BytesN, Env, contract, contractimpl, contracttype};

use risc0_interface::{Receipt, ReceiptClaim, RiscZeroVerifierInterface, VerifierError};

#[contracttype]
enum DataKey {
    Selector,
}

fn read_selector(env: &Env) -> Result<Bytes, VerifierError> {
    env.storage()
        .persistent()
        .get(&DataKey::Selector)
        .ok_or(VerifierError::InvalidSelector)
}

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
impl RiscZeroMockVerifier {
    pub fn __constructor(env: Env, selector: BytesN<4>) {
        let selector: Bytes = selector.into();
        env.storage()
            .persistent()
            .set(&DataKey::Selector, &selector);
    }

    pub fn selector(env: Env) -> Result<BytesN<4>, VerifierError> {
        let selector = read_selector(&env)?;
        BytesN::try_from(&selector).map_err(|_| VerifierError::InvalidSelector)
    }

    pub fn mock_prove(
        env: Env,
        image_id: BytesN<32>,
        journal_digest: BytesN<32>,
    ) -> Result<Receipt, VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal_digest);
        let claim_digest = claim.digest(&env);
        Self::mock_prove_claim(env, claim_digest)
    }

    pub fn mock_prove_claim(env: Env, claim_digest: BytesN<32>) -> Result<Receipt, VerifierError> {
        let selector = read_selector(&env)?;
        let mut seal = Bytes::new(&env);
        seal.append(&selector);
        seal.append(&Bytes::from_array(&env, &claim_digest.to_array()));

        Ok(Receipt { seal, claim_digest })
    }
}

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
        let expected_selector = read_selector(&env)?;
        let selector = receipt.seal.slice(0..4);

        if selector != expected_selector {
            return Err(VerifierError::InvalidSelector);
        }

        let seal_hash = env.crypto().keccak256(&receipt.seal.slice(4..)).to_bytes();
        let claim_hash = env
            .crypto()
            .keccak256(&receipt.claim_digest.into())
            .to_bytes();

        if seal_hash != claim_hash {
            return Err(VerifierError::InvalidProof);
        }

        Ok(())
    }
}
