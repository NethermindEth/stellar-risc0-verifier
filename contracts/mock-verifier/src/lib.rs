#![no_std]

use soroban_sdk::{Bytes, BytesN, Env, contract, contractimpl, contracttype};

use risc0_interface::{Receipt, ReceiptClaim, RiscZeroVerifierInterface, VerifierError};

#[cfg(test)]
mod test;

const DAY_IN_LEDGERS: u32 = 17_280;
const VERIFIER_EXTEND_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
const VERIFIER_TTL_THRESHOLD: u32 = VERIFIER_EXTEND_AMOUNT - DAY_IN_LEDGERS;

#[contracttype]
enum DataKey {
    Selector,
}

fn read_selector(env: &Env) -> Result<Bytes, VerifierError> {
    let key = DataKey::Selector;
    env.storage()
        .persistent()
        .get(&key)
        .inspect(|_| {
            env.storage().persistent().extend_ttl(
                &key,
                VERIFIER_TTL_THRESHOLD,
                VERIFIER_EXTEND_AMOUNT,
            );
        })
        .ok_or(VerifierError::InvalidSelector)
}

/// Mock verifier intended only for development with RISC Zero `DEV_MODE=1`.
///
/// !!! DANGER: USE IT ONLY FOR TESTING.
///
/// This verifier accepts a mock seal and does not perform any cryptographic proof verification. It
/// is meant for local development, integration tests, and end-to-end testing flows where real
/// proofs are not yet available or are intentionally bypassed.
///
/// Do not deploy or rely on this contract in production environments. It provides no security
/// guarantees and will accept any receipt that matches the mock format.
#[contract]
pub struct RiscZeroMockVerifier;

#[contractimpl]
impl RiscZeroMockVerifier {
    pub fn __constructor(env: Env, selector: BytesN<4>) {
        let selector: Bytes = selector.into();
        env.storage()
            .persistent()
            .set(&DataKey::Selector, &selector);
    }

    /// Returns the configured selector as `BytesN<4>`.
    ///
    /// Returns [`VerifierError::InvalidSelector`] if the stored value is missing or malformed.
    pub fn selector(env: Env) -> Result<BytesN<4>, VerifierError> {
        let selector = read_selector(&env)?;
        BytesN::try_from(&selector).map_err(|_| VerifierError::InvalidSelector)
    }

    /// Build a mock receipt for the given image ID and journal digest.
    ///
    /// The seal format matches the Ethereum mock verifier: `selector || claim_digest`.
    pub fn mock_prove(
        env: Env,
        image_id: BytesN<32>,
        journal_digest: BytesN<32>,
    ) -> Result<Receipt, VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal_digest);
        let claim_digest = claim.digest(&env);
        Self::mock_prove_claim(env, claim_digest)
    }

    /// Build a mock receipt for a precomputed claim digest.
    ///
    /// The seal format matches the Ethereum mock verifier: `selector || claim_digest`.
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

    /// Verify a mock seal by reconstructing the claim digest from inputs.
    ///
    /// Returns a structured [`VerifierError`] on selector mismatch or invalid proof.
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
        if receipt.seal.len() < 4 {
            return Err(VerifierError::MalformedSeal);
        }

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
