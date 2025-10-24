#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn test_risczero() {
    let env = Env::default();
    let contract_id = env.register(RiscZeroVerifier, ());

    let _ = RiscZeroVerifierClient::new(&env, &contract_id);
}
