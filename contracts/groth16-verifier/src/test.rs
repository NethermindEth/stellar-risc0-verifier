extern crate std;

use soroban_sdk::{Bytes, BytesN, Env};
use std::println;

use crate::{RiscZeroGroth16Verifier, RiscZeroGroth16VerifierClient};

/// Test seal data for benchmarks
const TEST_SEAL: [u8; 260] = [
    115, 196, 87, 186, 0, 237, 128, 235, 234, 82, 162, 215, 108, 219, 83, 253, 51, 151, 104, 190,
    16, 27, 191, 115, 52, 20, 229, 22, 168, 155, 98, 214, 70, 109, 143, 168, 39, 163, 217, 215,
    117, 155, 119, 189, 172, 46, 218, 8, 164, 36, 138, 163, 47, 66, 185, 51, 132, 186, 120, 68,
    221, 173, 16, 91, 83, 154, 236, 240, 16, 135, 147, 199, 205, 147, 71, 212, 179, 74, 227, 197,
    227, 148, 79, 255, 80, 116, 63, 60, 170, 174, 73, 33, 155, 190, 178, 211, 40, 104, 86, 133, 10,
    5, 96, 15, 143, 195, 135, 173, 205, 13, 185, 87, 103, 138, 0, 115, 115, 112, 161, 19, 129, 254,
    146, 216, 198, 153, 50, 139, 200, 104, 181, 15, 38, 239, 108, 112, 252, 67, 176, 221, 131, 101,
    167, 44, 11, 201, 135, 216, 18, 128, 33, 146, 39, 28, 36, 140, 236, 249, 13, 70, 58, 47, 111,
    147, 24, 26, 248, 151, 128, 30, 5, 148, 41, 172, 252, 33, 245, 34, 165, 60, 97, 133, 128, 111,
    105, 241, 23, 184, 109, 191, 86, 40, 187, 198, 73, 117, 2, 109, 28, 132, 149, 6, 243, 7, 121,
    100, 208, 124, 26, 204, 213, 137, 61, 33, 83, 93, 40, 164, 222, 86, 35, 238, 99, 177, 16, 168,
    241, 210, 8, 57, 248, 143, 79, 105, 86, 248, 56, 157, 41, 90, 192, 78, 112, 102, 135, 217, 204,
    56, 22, 57, 168, 230, 57, 33, 30, 155, 70, 128, 49, 27,
];

/// Test image ID (hex decoded)
const TEST_IMAGE_ID: [u8; 32] = [
    0xa7, 0x7e, 0x54, 0x91, 0x0c, 0x79, 0x2d, 0xdc, 0x3f, 0x14, 0x87, 0x8f, 0x3f, 0x13, 0x60, 0xaf,
    0x96, 0x61, 0x24, 0x08, 0xd6, 0x90, 0x74, 0xe8, 0x73, 0x89, 0xa2, 0x15, 0xf5, 0x75, 0x95, 0xb9,
];

/// Test journal data
const TEST_JOURNAL: [u8; 4] = [0x01, 0x00, 0x00, 0x78];

/// Helper to setup test environment and client
fn setup_test() -> (Env, RiscZeroGroth16VerifierClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(RiscZeroGroth16Verifier, ());
    let client = RiscZeroGroth16VerifierClient::new(&env, &contract_id);
    (env, client)
}

/// Helper to prepare test inputs
fn prepare_inputs(env: &Env) -> (Bytes, BytesN<32>, BytesN<32>) {
    let seal = Bytes::from_slice(env, &TEST_SEAL);
    let image_id = BytesN::from_array(env, &TEST_IMAGE_ID);
    let journal_digest = env.crypto().sha256(&Bytes::from_slice(env, &TEST_JOURNAL));
    (seal, image_id, journal_digest.into())
}

#[test]
fn test_verify_proof() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    assert_eq!(client.verify(&seal, &image_id, &journal_digest), ());
}

// ============================================================================
// BENCHMARKS - Gas Consumption Tracking
// ============================================================================

/// Prints full budget in a formatted way
fn print_budget(env: &Env, label: &str) {
    let budget = env.cost_estimate().budget();

    println!("\n========== BENCHMARK: {} ==========", label);
    budget.print();
    println!("==========================================\n");
}

#[test]
fn bench_verify() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    // Run verification
    assert_eq!(client.verify(&seal, &image_id, &journal_digest), ());

    // Print results
    print_budget(&env, "verify()");
}

#[test]
fn bench_verify_integrity() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    // Build receipt manually
    let claim = risc0_interface::ReceiptClaim::new(&env, image_id, journal_digest);
    let receipt = risc0_interface::Receipt {
        seal,
        claim_digest: claim.digest(&env),
    };

    // Run verification
    assert_eq!(client.verify_integrity(&receipt), ());

    // Print results
    print_budget(&env, "verify_integrity()");
}

#[test]
fn bench_receipt_claim_digest() {
    let (env, _client) = setup_test();
    let image_id = BytesN::from_array(&env, &TEST_IMAGE_ID);
    let journal_digest: BytesN<32> = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, &TEST_JOURNAL))
        .into();

    // Build claim and compute digest
    let claim = risc0_interface::ReceiptClaim::new(&env, image_id, journal_digest);
    let _digest = claim.digest(&env);

    // Print results
    print_budget(&env, "ReceiptClaim::digest()");
}
