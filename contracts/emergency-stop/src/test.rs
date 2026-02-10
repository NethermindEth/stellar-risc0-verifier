extern crate std;

use risc0_interface::{Receipt, RiscZeroVerifierInterface, VerifierError};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, contract, contractimpl, contracttype, testutils::Address as _,
};

use crate::{RiscZeroVerifierEmergencyStop, RiscZeroVerifierEmergencyStopClient};

#[contract]
struct MockVerifier;

#[contracttype]
enum MockKey {
    IntegrityCalled,
}

#[contractimpl]
impl MockVerifier {
    pub fn integrity_called(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&MockKey::IntegrityCalled)
            .unwrap_or(false)
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for MockVerifier {
    type Proof = Bytes;

    fn verify(
        _env: Env,
        _seal: Bytes,
        _image_id: BytesN<32>,
        _journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        Ok(())
    }

    fn verify_integrity(env: Env, _receipt: Receipt) -> Result<(), VerifierError> {
        env.storage()
            .instance()
            .set(&MockKey::IntegrityCalled, &true);
        Ok(())
    }
}

fn setup() -> (
    Env,
    Address,
    RiscZeroVerifierEmergencyStopClient<'static>,
    MockVerifierClient<'static>,
) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let verifier_id = env.register(MockVerifier, ());
    let verifier_client = MockVerifierClient::new(&env, &verifier_id);
    let estop_id = env.register(RiscZeroVerifierEmergencyStop, (verifier_id, owner.clone()));
    let estop_client = RiscZeroVerifierEmergencyStopClient::new(&env, &estop_id);
    (env, owner, estop_client, verifier_client)
}

fn test_inputs(env: &Env) -> (Bytes, BytesN<32>, BytesN<32>) {
    let seal = Bytes::from_slice(env, &[1, 2, 3]);
    let image_id = BytesN::from_array(env, &[7u8; 32]);
    let journal = BytesN::from_array(env, &[9u8; 32]);
    (seal, image_id, journal)
}

#[test]
fn forwards_verify_when_unpaused() {
    let (env, _owner, client, _verifier_client) = setup();
    let (seal, image_id, journal) = test_inputs(&env);

    assert_eq!(client.verify(&seal, &image_id, &journal), ());
}

#[test]
fn estop_sets_paused() {
    let (env, _owner, client, _verifier_client) = setup();

    env.mock_all_auths();
    client.estop();

    assert!(client.paused());
}

#[test]
#[should_panic]
fn estop_rejects_non_owner() {
    let (_env, _owner, client, _verifier_client) = setup();
    client.estop();
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn verify_rejects_when_paused() {
    let (env, _owner, client, _verifier_client) = setup();
    let (seal, image_id, journal) = test_inputs(&env);

    env.mock_all_auths();
    client.estop();
    client.verify(&seal, &image_id, &journal);
}

#[test]
#[should_panic(expected = "Error(Contract, #1001)")]
fn estop_with_receipt_requires_zero_digest() {
    let (env, _owner, client, _verifier_client) = setup();
    let receipt = Receipt {
        seal: Bytes::from_slice(&env, &[0xAA]),
        claim_digest: BytesN::from_array(&env, &[1u8; 32]),
    };

    client.estop_with_receipt(&receipt);
}

#[test]
fn estop_with_receipt_pauses_and_calls_verifier() {
    let (env, _owner, client, verifier_client) = setup();
    let receipt = Receipt {
        seal: Bytes::from_slice(&env, &[0xBB]),
        claim_digest: BytesN::from_array(&env, &[0u8; 32]),
    };

    client.estop_with_receipt(&receipt);

    assert!(client.paused());
    assert!(verifier_client.integrity_called());
}
