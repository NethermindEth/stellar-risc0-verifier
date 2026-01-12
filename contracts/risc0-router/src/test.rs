use super::*;
use risc0_interface::{Receipt, ReceiptClaim};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, IntoVal, Symbol, contract, contractimpl, symbol_short,
    testutils::Address as _,
};

// =============================================================================
// Mock Verifier Contract
// =============================================================================
// A simple mock verifier that implements the RiscZeroVerifierInterface for
// testing. It stores verification calls so we can assert they were routed
// correctly.

mod mock_verifier {
    use super::*;
    use risc0_interface::{Receipt, RiscZeroVerifierInterface};

    #[contract]
    pub struct MockVerifier;

    #[contractimpl]
    impl MockVerifier {
        /// Returns true if this mock was called (for testing routing)
        pub fn was_called(env: Env) -> bool {
            env.storage().temporary().has(&"called")
        }

        /// Get the receipt that was verified
        pub fn get_verified_receipt(env: Env) -> Option<Receipt> {
            env.storage().temporary().get(&"receipt")
        }
    }

    #[contractimpl]
    impl RiscZeroVerifierInterface for MockVerifier {
        type Proof = ();

        fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
            let claim = ReceiptClaim::new(&env, image_id, journal);
            let receipt = Receipt {
                seal,
                claim_digest: claim.digest(&env),
            };
            Self::verify_integrity(env, receipt);
        }

        fn verify_integrity(env: Env, receipt: Receipt) {
            env.storage().temporary().set(&"called", &true);
            env.storage().temporary().set(&"receipt", &receipt);
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_env() -> (Env, Address, RiscZeroVerifierRouterClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);

    (env, admin, client)
}

fn create_selector(env: &Env, bytes: [u8; 4]) -> BytesN<4> {
    BytesN::from_array(env, &bytes)
}

fn create_seal_with_selector(env: &Env, selector: &BytesN<4>) -> Bytes {
    let mut seal_bytes = selector.to_array().to_vec();
    // Add some dummy proof data after the selector
    seal_bytes.extend_from_slice(&[0u8; 32]);
    Bytes::from_slice(env, &seal_bytes)
}

/// Helper to extract VerifierError from the nested Result type
fn unwrap_verifier_error<T: core::fmt::Debug>(
    result: Result<
        Result<T, soroban_sdk::ConversionError>,
        Result<VerifierError, soroban_sdk::InvokeError>,
    >,
) -> VerifierError {
    match result {
        Err(Ok(e)) => e,
        _ => panic!("Expected VerifierError but got {:?}", result),
    }
}

// =============================================================================
// Constructor Tests
// =============================================================================

#[test]
fn test_constructor_sets_owner() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);

    assert_eq!(client.get_owner(), Some(admin));
}

// =============================================================================
// Add Verifier Tests
// =============================================================================

#[test]
fn test_add_verifier_success() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let verifier_address = Address::generate(&env);

    // Non-try version - will panic on error
    client.add_verifier(&selector, &verifier_address);

    // Verify it was added
    let result = client.get_verifier_by_selector(&selector);
    assert_eq!(result, verifier_address);
}

#[test]
fn test_add_verifier_selector_in_use() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let verifier1 = Address::generate(&env);
    let verifier2 = Address::generate(&env);

    // First add should succeed
    client.add_verifier(&selector, &verifier1);

    // Second add with same selector should fail - use try_ to capture error
    let result = client.try_add_verifier(&selector, &verifier2);
    assert_eq!(unwrap_verifier_error(result), VerifierError::SelectorInUse);
}

#[test]
fn test_add_verifier_tombstone_selector() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let verifier = Address::generate(&env);

    // Manually set a tombstone entry
    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::Verifier(selector.clone()),
            &VerifierEntry::Tombstone,
        );
    });

    // Adding to tombstoned selector should fail - use try_ to capture error
    let result = client.try_add_verifier(&selector, &verifier);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

// =============================================================================
// Get Verifier Tests
// =============================================================================

#[test]
fn test_get_verifier_by_selector_unknown() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);

    // Use try_ to capture error
    let result = client.try_get_verifier_by_selector(&selector);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorUnknown
    );
}

#[test]
fn test_get_verifier_by_selector_tombstone() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);

    // Manually set a tombstone entry
    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::Verifier(selector.clone()),
            &VerifierEntry::Tombstone,
        );
    });

    // Use try_ to capture error
    let result = client.try_get_verifier_by_selector(&selector);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

#[test]
fn test_get_verifier_from_seal() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0xDE, 0xAD, 0xBE, 0xEF]);
    let verifier_address = Address::generate(&env);

    client.add_verifier(&selector, &verifier_address);

    let seal = create_seal_with_selector(&env, &selector);
    let result = client.get_verifier_from_seal(&seal);
    assert_eq!(result, verifier_address);
}

#[test]
fn test_get_verifier_from_seal_unknown() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0xDE, 0xAD, 0xBE, 0xEF]);
    let seal = create_seal_with_selector(&env, &selector);

    // Use try_ to capture error
    let result = client.try_get_verifier_from_seal(&seal);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorUnknown
    );
}

// =============================================================================
// Verification Routing Tests
// =============================================================================

#[test]
fn test_verify_routes_to_correct_verifier() {
    let (env, _admin, client) = setup_env();

    // Register a mock verifier
    let mock_verifier_id = env.register(mock_verifier::MockVerifier, ());
    let mock_client = mock_verifier::MockVerifierClient::new(&env, &mock_verifier_id);

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    client.add_verifier(&selector, &mock_verifier_id);

    // Create a seal with the correct selector
    let seal = create_seal_with_selector(&env, &selector);
    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    // Verify through the router by invoking the contract function directly
    let _: () = env.invoke_contract(
        &client.address,
        &symbol_short!("verify"),
        (seal, image_id, journal_digest).into_val(&env),
    );

    // Check that the mock verifier was called
    assert!(mock_client.was_called());
}

#[test]
fn test_verify_integrity_routes_to_correct_verifier() {
    let (env, _admin, client) = setup_env();

    // Register a mock verifier
    let mock_verifier_id = env.register(mock_verifier::MockVerifier, ());
    let mock_client = mock_verifier::MockVerifierClient::new(&env, &mock_verifier_id);

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    client.add_verifier(&selector, &mock_verifier_id);

    // Create a receipt with the correct selector in the seal
    let seal = create_seal_with_selector(&env, &selector);
    let claim_digest = BytesN::from_array(&env, &[0u8; 32]);
    let receipt = Receipt {
        seal,
        claim_digest: claim_digest.clone(),
    };

    // Verify integrity through the router by invoking the contract function
    // directly
    let _: () = env.invoke_contract(
        &client.address,
        &Symbol::new(&env, "verify_integrity"),
        (receipt,).into_val(&env),
    );

    // Check that the mock verifier was called with the correct receipt
    assert!(mock_client.was_called());
    let verified_receipt = mock_client.get_verified_receipt().unwrap();
    assert_eq!(verified_receipt.claim_digest, claim_digest);
}

#[test]
#[should_panic]
fn test_verify_panics_on_unknown_selector() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let seal = create_seal_with_selector(&env, &selector);
    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    // This should panic because no verifier is registered for this selector
    let _: () = env.invoke_contract(
        &client.address,
        &symbol_short!("verify"),
        (seal, image_id, journal_digest).into_val(&env),
    );
}

// =============================================================================
// Admin Authorization Tests
// =============================================================================

#[test]
#[should_panic]
fn test_add_verifier_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);
    env.set_auths(&[]);

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let verifier = Address::generate(&env);

    // Should trap on admin.require_auth().
    client.add_verifier(&selector, &verifier);
}
