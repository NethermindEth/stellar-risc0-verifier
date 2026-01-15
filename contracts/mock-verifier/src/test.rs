extern crate std;

use soroban_sdk::{Bytes, BytesN, Env};

use crate::{RiscZeroMockVerifier, RiscZeroMockVerifierClient};
use risc0_interface::{Receipt, ReceiptClaim, VerifierError};

fn bytes_from<const N: usize>(env: &Env, value: &BytesN<N>) -> Bytes {
    Bytes::from_array(env, &value.to_array())
}

fn setup() -> (Env, RiscZeroMockVerifierClient<'static>, BytesN<4>) {
    let env = Env::default();
    let selector = BytesN::from_array(&env, &[0x11, 0x22, 0x33, 0x44]);
    let contract_id = env.register(RiscZeroMockVerifier, (selector.clone(),));
    let client = RiscZeroMockVerifierClient::new(&env, &contract_id);
    (env, client, selector)
}

#[test]
fn test_mock_prove_claim_builds_seal() {
    let (env, client, selector) = setup();
    let claim_digest = BytesN::from_array(&env, &[0xAB; 32]);

    let receipt = client.mock_prove_claim(&claim_digest);

    assert_eq!(receipt.claim_digest, claim_digest);
    assert_eq!(receipt.seal.len(), 36);
    assert_eq!(receipt.seal.slice(0..4), bytes_from(&env, &selector));
    assert_eq!(receipt.seal.slice(4..), bytes_from(&env, &claim_digest));
}

#[test]
fn test_verify_integrity_ok() {
    let (env, client, _selector) = setup();

    let image_id = BytesN::from_array(&env, &[0x01; 32]);
    let journal_digest = BytesN::from_array(&env, &[0x02; 32]);

    let receipt = client.mock_prove(&image_id, &journal_digest);
    let expected_claim = ReceiptClaim::new(&env, image_id, journal_digest);
    assert_eq!(receipt.claim_digest, expected_claim.digest(&env));
    assert_eq!(client.verify_integrity(&receipt), ());
}

#[test]
fn test_verify_integrity_invalid_selector() {
    let (env, client, selector) = setup();
    let claim_digest = BytesN::from_array(&env, &[0xCD; 32]);

    let mut seal = Bytes::new(&env);
    let mut wrong_selector = selector.to_array();
    wrong_selector[0] ^= 0xFF;
    seal.append(&Bytes::from_array(&env, &wrong_selector));
    seal.append(&Bytes::from_array(&env, &claim_digest.to_array()));

    let receipt = Receipt { seal, claim_digest };

    let Err(Ok(VerifierError::InvalidSelector)) = client.try_verify_integrity(&receipt) else {
        panic!("expected InvalidSelector");
    };
}

#[test]
fn test_verify_integrity_invalid_proof() {
    let (env, client, _selector) = setup();
    let claim_digest = BytesN::from_array(&env, &[0xAA; 32]);

    let receipt = client.mock_prove_claim(&claim_digest);
    let wrong_claim = BytesN::from_array(&env, &[0xBB; 32]);
    let wrong_receipt = Receipt {
        seal: receipt.seal,
        claim_digest: wrong_claim,
    };

    let Err(Ok(VerifierError::InvalidProof)) = client.try_verify_integrity(&wrong_receipt) else {
        panic!("expected InvalidProof");
    };
}
