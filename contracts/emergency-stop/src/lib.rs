#![no_std]

use risc0_interface::{Receipt, RiscZeroVerifierClient, RiscZeroVerifierInterface, VerifierError};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};
use stellar_access::ownable::{self, Ownable};
use stellar_contract_utils::pausable::{self, Pausable};
use stellar_macros::{only_owner, when_not_paused};

#[cfg(test)]
mod test;

const ZERO_DIGEST: [u8; 32] = [0u8; 32];

/// Storage keys used by the emergency stop contract.
#[contracttype]
pub enum DataKey {
    /// Address of the verifier implementation being wrapped.
    Verifier,
}

/// Errors emitted by the emergency stop wrapper.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EmergencyStopError {
    /// Caller is not authorized to perform the requested action.
    Unauthorized = 1,
    /// Verifier address is not configured.
    VerifierNotSet = 5,
    /// Receipt does not prove a circuit-breaker exploit.
    InvalidProofOfExploit = 1001,
    /// Unpause is not supported by the emergency stop wrapper.
    UnpauseNotAllowed = 1002,
}

/// Emergency-stop wrapper for a RISC Zero verifier contract.
#[contract]
pub struct RiscZeroVerifierEmergencyStop;

#[contractimpl]
impl RiscZeroVerifierEmergencyStop {
    /// Initializes the wrapper with an underlying verifier and guardian owner.
    pub fn __constructor(env: Env, verifier: Address, owner: Address) {
        env.storage().instance().set(&DataKey::Verifier, &verifier);
        ownable::set_owner(&env, &owner);
    }

    /// Returns the verifier address wrapped by this contract.
    pub fn get_verifier(env: Env) -> Address {
        get_verifier(&env)
    }

    /// Permanently pauses verification. Only the guardian can call this.
    #[only_owner]
    pub fn estop(env: Env) {
        pausable::pause(&env);
    }

    /// Permanently pauses verification via the circuit-breaker receipt.
    pub fn estop_with_receipt(env: Env, receipt: Receipt) {
        pausable::when_not_paused(&env);

        let zero_digest = BytesN::from_array(&env, &ZERO_DIGEST);
        if receipt.claim_digest != zero_digest {
            panic_with_error!(&env, EmergencyStopError::InvalidProofOfExploit);
        }

        // Ensure the proof-of-exploit receipt is valid.
        let _ = Self::verify_integrity(env.clone(), receipt);

        pausable::pause(&env);
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroVerifierEmergencyStop {
    type Proof = Bytes;

    #[when_not_paused]
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let verifier = get_verifier(&env);
        let client = RiscZeroVerifierClient::new(&env, &verifier);
        client.verify(&seal, &image_id, &journal);
        Ok(())
    }

    #[when_not_paused]
    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let verifier = get_verifier(&env);
        let client = RiscZeroVerifierClient::new(&env, &verifier);
        client.verify_integrity(&receipt);
        Ok(())
    }
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierEmergencyStop {}

#[contractimpl]
impl Pausable for RiscZeroVerifierEmergencyStop {
    fn paused(env: &Env) -> bool {
        pausable::paused(env)
    }

    fn pause(env: &Env, caller: Address) {
        let owner = ownable::enforce_owner_auth(env);
        if owner != caller {
            panic_with_error!(env, EmergencyStopError::Unauthorized);
        }
        pausable::pause(env);
    }

    fn unpause(env: &Env, _caller: Address) {
        panic_with_error!(env, EmergencyStopError::UnpauseNotAllowed);
    }
}

fn get_verifier(env: &Env) -> Address {
    match env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Verifier)
    {
        Some(verifier) => verifier,
        None => panic_with_error!(env, EmergencyStopError::VerifierNotSet),
    }
}
