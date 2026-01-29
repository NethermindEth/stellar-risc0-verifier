#![cfg(test)]

use soroban_sdk::{
    Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
    auth::{Context, ContractContext},
    symbol_short,
    testutils::{Address as _, BytesN as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke},
    vec,
};

use crate::{OperationMeta, TimelockController, TimelockControllerClient};
use stellar_governance::timelock::TimelockError;

// A simple target contract for testing timelock operations
mod target_contract {
    use soroban_sdk::{Address, Env, contract, contractimpl, contracttype};
    use stellar_access::ownable::{Ownable, set_owner};
    use stellar_macros::only_owner;

    #[contracttype]
    enum DataKey {
        Value,
    }

    #[contract]
    pub struct TargetContract;

    #[contractimpl]
    impl TargetContract {
        pub fn __constructor(e: &Env, owner: Address) {
            set_owner(e, &owner);
        }

        #[only_owner]
        pub fn set_value(e: &Env, value: u32) {
            e.storage().persistent().set(&DataKey::Value, &value);
        }

        pub fn get_value(e: &Env) -> u32 {
            e.storage().persistent().get(&DataKey::Value).unwrap_or(0)
        }
    }

    #[contractimpl(contracttrait)]
    impl Ownable for TargetContract {}
}

use target_contract::{TargetContract, TargetContractClient};

/// Creates empty 32-byte predecessor/salt.
fn zero_bytes(e: &Env) -> BytesN<32> {
    BytesN::from_array(e, &[0u8; 32])
}

/// Helper to create a unique salt.
fn salt_from_u8(e: &Env, val: u8) -> BytesN<32> {
    let mut arr = [0u8; 32];
    arr[0] = val;
    BytesN::from_array(e, &arr)
}

type BatchTwoCalls = (Vec<Address>, Vec<Symbol>, Vec<Vec<Val>>, Vec<Val>, Vec<Val>);

fn batch_two_calls(e: &Env, target: &Address, value1: u32, value2: u32) -> BatchTwoCalls {
    let args1: Vec<Val> = vec![e, value1.into_val(e)];
    let args2: Vec<Val> = vec![e, value2.into_val(e)];
    let targets = vec![e, target.clone(), target.clone()];
    let functions = vec![e, symbol_short!("set_value"), symbol_short!("set_value")];
    let args_list = vec![e, args1.clone(), args2.clone()];

    (targets, functions, args_list, args1, args2)
}

/// Sets up a basic test environment with timelock and target contracts.
fn setup_with_external_admin(
    e: &Env,
) -> (
    TimelockControllerClient<'_>,
    TargetContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let admin = Address::generate(e);
    let proposer = Address::generate(e);
    let executor = Address::generate(e);

    let timelock_id = e.register(
        TimelockController,
        (
            60u32, // min_delay: 60 seconds
            vec![e, proposer.clone()],
            vec![e, executor.clone()],
            Some(admin.clone()),
        ),
    );
    let timelock = TimelockControllerClient::new(e, &timelock_id);

    // Deploy target contract owned by the timelock
    let target_id = e.register(TargetContract, (&timelock_id,));
    let target = TargetContractClient::new(e, &target_id);

    (timelock, target, admin, proposer, executor)
}

/// Sets up a timelock with self-administration (no external admin).
fn setup_self_admin(
    e: &Env,
) -> (
    TimelockControllerClient<'_>,
    TargetContractClient<'_>,
    Address,
    Address,
) {
    let proposer = Address::generate(e);
    let executor = Address::generate(e);

    let timelock_id = e.register(
        TimelockController,
        (
            60u32, // min_delay: 60 seconds
            vec![e, proposer.clone()],
            vec![e, executor.clone()],
            Option::<Address>::None,
        ),
    );
    let timelock = TimelockControllerClient::new(e, &timelock_id);

    // Deploy target contract owned by the timelock
    let target_id = e.register(TargetContract, (&timelock_id,));
    let target = TargetContractClient::new(e, &target_id);

    (timelock, target, proposer, executor)
}

/// Sets up a timelock with no executors (open execution).
fn setup_open_execution(
    e: &Env,
) -> (
    TimelockControllerClient<'_>,
    TargetContractClient<'_>,
    Address,
) {
    let proposer = Address::generate(e);

    let timelock_id = e.register(
        TimelockController,
        (
            60u32,
            vec![e, proposer.clone()],
            Vec::<Address>::new(e), // no executors
            Option::<Address>::None,
        ),
    );
    let timelock = TimelockControllerClient::new(e, &timelock_id);

    let target_id = e.register(TargetContract, (&timelock_id,));
    let target = TargetContractClient::new(e, &target_id);

    (timelock, target, proposer)
}

// ============================================================================
// Constructor Tests
// ============================================================================

#[test]
fn test_constructor_variants() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, _, admin, proposer, executor) = setup_with_external_admin(&e);

    // Verify min delay
    assert_eq!(timelock.get_min_delay(), 60);

    // Verify roles are assigned
    assert!(
        timelock
            .has_role(&proposer, &symbol_short!("proposer"))
            .is_some()
    );
    assert!(
        timelock
            .has_role(&proposer, &symbol_short!("canceller"))
            .is_some()
    );
    assert!(
        timelock
            .has_role(&executor, &symbol_short!("executor"))
            .is_some()
    );

    // Admin should be the timelock contract
    assert_eq!(timelock.get_admin(), Some(timelock.address.clone()));

    // External admin should be granted the bootstrap role
    assert!(
        timelock
            .has_role(&admin, &symbol_short!("bootstrap"))
            .is_some()
    );

    // Bootstrap role should be the admin role for proposer/executor/canceller
    assert_eq!(
        timelock.get_role_admin(&symbol_short!("proposer")),
        Some(symbol_short!("bootstrap"))
    );
    assert_eq!(
        timelock.get_role_admin(&symbol_short!("executor")),
        Some(symbol_short!("bootstrap"))
    );
    assert_eq!(
        timelock.get_role_admin(&symbol_short!("canceller")),
        Some(symbol_short!("bootstrap"))
    );

    let (timelock, _, proposer, executor) = setup_self_admin(&e);

    // Admin should be the contract itself
    assert_eq!(timelock.get_admin(), Some(timelock.address.clone()));

    // Verify roles
    assert!(
        timelock
            .has_role(&proposer, &symbol_short!("proposer"))
            .is_some()
    );
    assert!(
        timelock
            .has_role(&executor, &symbol_short!("executor"))
            .is_some()
    );

    let (timelock, _, proposer) = setup_open_execution(&e);

    assert!(
        timelock
            .has_role(&proposer, &symbol_short!("proposer"))
            .is_some()
    );
    // No executors configured
}

// ============================================================================
// Schedule Operation Tests
// ============================================================================

#[test]
fn test_schedule_operation_delays() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32, // delay >= min_delay
        &proposer,
    );

    // Operation should exist and be pending
    assert!(timelock.operation_exists(&op_id));
    assert!(timelock.is_operation_pending(&op_id));
    assert!(!timelock.is_operation_ready(&op_id)); // Not ready yet (no time passed)
    assert!(!timelock.is_operation_done(&op_id));

    // Schedule with exactly min_delay should work
    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt_from_u8(&e, 1),
        &60u32, // exactly min_delay
        &proposer,
    );

    assert!(timelock.operation_exists(&op_id));
}

#[test]
#[should_panic(expected = "#2000")]
fn test_schedule_without_proposer_role() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, _, _) = setup_with_external_admin(&e);
    let non_proposer = Address::generate(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    // Should fail - non_proposer doesn't have proposer role
    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &non_proposer,
    );
}

#[test]
fn test_schedule_op_insufficient_delay() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let Err(Ok(err)) = timelock.try_schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &1u32, // less than min_delay
        &proposer,
    ) else {
        panic!("expected InsufficientDelay");
    };
    assert_eq!(err, TimelockError::InsufficientDelay.into());
}

// ============================================================================
// Execute Operation Tests
// ============================================================================

#[test]
fn test_execute_operation() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    // Schedule
    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    // Advance time past the delay
    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    // Should be ready now
    assert!(timelock.is_operation_ready(&op_id));

    // Execute
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &Some(executor),
    );

    // Verify operation is done
    assert!(timelock.is_operation_done(&op_id));
    assert!(!timelock.is_operation_pending(&op_id));

    // Verify target was updated
    assert_eq!(target.get_value(), 42);
}

#[test]
fn test_execute_open_execution() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, proposer) = setup_open_execution(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 99u32.into_val(&e)];

    // Schedule
    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    // Anyone can execute when no executors are configured
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &None, // No executor needed
    );

    assert_eq!(target.get_value(), 99);
}

#[test]
#[should_panic(expected = "#2000")]
fn test_execute_without_executor_role() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);
    let non_executor = Address::generate(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    // Should fail - non_executor doesn't have executor role
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &Some(non_executor),
    );
}

#[test]
fn test_execute_before_ready_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    let Err(Ok(err)) = timelock.try_execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &Some(executor),
    ) else {
        panic!("expected InvalidOperationState");
    };
    assert_eq!(err, TimelockError::InvalidOperationState.into());
}

#[test]
#[should_panic(expected = "executor must be present when executors are configured")]
fn test_execute_requires_executor_when_configured() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &None,
    );
}

// ============================================================================
// Cancel Operation Tests
// ============================================================================

#[test]
fn test_cancel_operation() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    assert!(timelock.is_operation_pending(&op_id));

    // Cancel (proposer is also canceller)
    timelock.cancel_op(&op_id, &proposer);

    // Operation should no longer exist
    assert!(!timelock.operation_exists(&op_id));
    assert!(!timelock.is_operation_pending(&op_id));
}

#[test]
#[should_panic(expected = "#2000")]
fn test_cancel_without_canceller_role() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    // Executor doesn't have canceller role
    timelock.cancel_op(&op_id, &executor);
}

#[test]
fn test_cancel_after_execution_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &Some(executor),
    );

    let Err(Ok(err)) = timelock.try_cancel_op(&op_id, &proposer) else {
        panic!("expected InvalidOperationState");
    };
    assert_eq!(err, TimelockError::InvalidOperationState.into());
}

// ============================================================================
// Update Delay Tests
// ============================================================================

#[test]
fn test_update_delay_self_admin_flow() {
    let e = Env::default();

    let proposer = Address::generate(&e);
    let executor = Address::generate(&e);

    let timelock_id = e.register(
        TimelockController,
        (
            60u32,
            vec![&e, proposer.clone()],
            vec![&e, executor.clone()],
            None::<Address>,
        ),
    );
    let timelock = TimelockControllerClient::new(&e, &timelock_id);

    let new_delay = 120u32;
    let args: Vec<Val> = vec![&e, new_delay.into_val(&e)];
    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);

    let op_id = timelock
        .mock_auths(&[MockAuth {
            address: &proposer,
            invoke: &MockAuthInvoke {
                contract: &timelock_id,
                fn_name: "schedule_op",
                args: (
                    timelock_id.clone(),
                    Symbol::new(&e, "update_delay"),
                    args.clone(),
                    predecessor.clone(),
                    salt.clone(),
                    60u32,
                    proposer.clone(),
                )
                    .into_val(&e),
                sub_invokes: &[],
            },
        }])
        .schedule_op(
            &timelock_id,
            &Symbol::new(&e, "update_delay"),
            &args,
            &predecessor,
            &salt,
            &60u32,
            &proposer,
        );

    e.ledger().with_mut(|li| li.timestamp += 61);

    // Mock executor's require_auth_for_args() that is checked in __check_auth
    e.mock_auths(&[MockAuth {
        address: &executor,
        invoke: &MockAuthInvoke {
            contract: &timelock_id,
            fn_name: "__check_auth",
            args: (
                Symbol::new(&e, "execute_op"),
                timelock_id.clone(),
                Symbol::new(&e, "update_delay"),
                args.clone(),
                predecessor.clone(),
                salt.clone(),
            )
                .into_val(&e),
            sub_invokes: &[],
        },
    }]);

    e.try_invoke_contract_check_auth::<TimelockError>(
        &timelock_id,
        &BytesN::random(&e),
        vec![
            &e,
            OperationMeta {
                predecessor: predecessor.clone(),
                salt: salt.clone(),
                executor: Some(executor),
            },
        ]
        .into_val(&e),
        &vec![
            &e,
            Context::Contract(ContractContext {
                contract: timelock_id.clone(),
                fn_name: Symbol::new(&e, "update_delay"),
                args,
            }),
        ],
    )
    .unwrap();

    assert!(timelock.is_operation_done(&op_id));
}

// ============================================================================
// Hash Operation Tests
// ============================================================================

#[test]
fn test_hash_operation_deterministic_and_salt() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, _, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let hash1 = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
    );
    let hash2 = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
    );
    let hash3 = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt_from_u8(&e, 1),
    );

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, hash3);
}

// ============================================================================
// Operation State Tests
// ============================================================================

#[test]
fn test_operation_state_transitions() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    // Initially unset
    let op_id = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
    );

    use stellar_governance::timelock::OperationState;
    assert_eq!(timelock.get_operation_state(&op_id), OperationState::Unset);

    // Schedule -> Waiting
    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );
    assert_eq!(
        timelock.get_operation_state(&op_id),
        OperationState::Waiting
    );

    // Advance time -> Ready
    e.ledger().set_timestamp(e.ledger().timestamp() + 101);
    assert_eq!(timelock.get_operation_state(&op_id), OperationState::Ready);

    // Execute -> Done
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &Some(executor),
    );
    assert_eq!(timelock.get_operation_state(&op_id), OperationState::Done);
}

// ============================================================================
// Predecessor Tests
// ============================================================================

#[test]
fn test_predecessor_dependency() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let zero_pred = zero_bytes(&e);
    let salt1 = salt_from_u8(&e, 1);
    let salt2 = salt_from_u8(&e, 2);
    let args1: Vec<Val> = vec![&e, 10u32.into_val(&e)];
    let args2: Vec<Val> = vec![&e, 20u32.into_val(&e)];

    // Schedule first operation
    let op1_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args1,
        &zero_pred,
        &salt1,
        &100u32,
        &proposer,
    );

    // Schedule second operation with first as predecessor
    let _op2_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &op1_id,
        &salt2,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    // Execute first operation
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args1,
        &zero_pred,
        &salt1,
        &Some(executor.clone()),
    );
    assert_eq!(target.get_value(), 10);

    // Now execute second operation (predecessor is done)
    timelock.execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &op1_id,
        &salt2,
        &Some(executor),
    );
    assert_eq!(target.get_value(), 20);
}

#[test]
fn test_predecessor_not_done_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let zero_pred = zero_bytes(&e);
    let salt1 = salt_from_u8(&e, 1);
    let salt2 = salt_from_u8(&e, 2);
    let args1: Vec<Val> = vec![&e, 10u32.into_val(&e)];
    let args2: Vec<Val> = vec![&e, 20u32.into_val(&e)];

    let op1_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args1,
        &zero_pred,
        &salt1,
        &100u32,
        &proposer,
    );

    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &op1_id,
        &salt2,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    let Err(Ok(err)) = timelock.try_execute_op(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &op1_id,
        &salt2,
        &Some(executor),
    ) else {
        panic!("expected UnexecutedPredecessor");
    };
    assert_eq!(err, TimelockError::UnexecutedPredecessor.into());
}

// ============================================================================
// Multiple Operations Tests
// ============================================================================

#[test]
fn test_multiple_operations_different_salts() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    // Schedule multiple operations with different salts
    let op1_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt_from_u8(&e, 1),
        &100u32,
        &proposer,
    );

    let op2_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt_from_u8(&e, 2),
        &100u32,
        &proposer,
    );

    assert_ne!(op1_id, op2_id);
    assert!(timelock.operation_exists(&op1_id));
    assert!(timelock.operation_exists(&op2_id));
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[test]
fn test_schedule_batch() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let (targets, functions, args_list, args1, args2) =
        batch_two_calls(&e, &target.address, 10, 20);

    let batch_id = timelock.schedule_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    let expected_batch_id =
        timelock.hash_operation_batch(&targets, &functions, &args_list, &predecessor, &salt);
    assert_eq!(batch_id, expected_batch_id);

    let op1_id = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args1,
        &predecessor,
        &salt,
    );
    let op2_id = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &predecessor,
        &salt,
    );

    assert!(timelock.operation_exists(&op1_id));
    assert!(timelock.operation_exists(&op2_id));
}

#[test]
fn test_execute_batch() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let (targets, functions, args_list, args1, args2) =
        batch_two_calls(&e, &target.address, 10, 20);

    timelock.schedule_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    timelock.execute_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &Some(executor),
    );

    assert_eq!(target.get_value(), 20);

    let op1_id = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args1,
        &predecessor,
        &salt,
    );
    let op2_id = timelock.hash_operation(
        &target.address,
        &symbol_short!("set_value"),
        &args2,
        &predecessor,
        &salt,
    );

    assert!(timelock.is_operation_done(&op1_id));
    assert!(timelock.is_operation_done(&op2_id));
}

#[test]
#[should_panic(expected = "#5000")]
fn test_schedule_batch_length_mismatch() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args1: Vec<Val> = vec![&e, 10u32.into_val(&e)];

    let targets = vec![&e, target.address.clone(), target.address.clone()];
    let functions = vec![&e, symbol_short!("set_value")];
    let args_list = vec![&e, args1];

    timelock.schedule_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );
}

// ============================================================================
// Event Tests
// ============================================================================

#[test]
fn test_call_salt_emitted_for_schedule_op() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = salt_from_u8(&e, 1);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];

    let before = e.events().all().events().len();
    timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );
    let after = e.events().all().events().len();

    // OperationScheduled + CallSalt
    assert_eq!(after, before + 2);
}

#[test]
fn test_call_salt_emitted_for_schedule_batch() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = salt_from_u8(&e, 1);
    let (targets, functions, args_list, _args1, _args2) =
        batch_two_calls(&e, &target.address, 10, 20);

    let before = e.events().all().events().len();
    timelock.schedule_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );
    let after = e.events().all().events().len();

    // 2x OperationScheduled + 2x BatchCallScheduled + CallSalt
    assert_eq!(after, before + 5);
}

#[test]
fn test_execute_batch_emits_events() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, executor) = setup_with_external_admin(&e);

    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let (targets, functions, args_list, _args1, _args2) =
        batch_two_calls(&e, &target.address, 10, 20);

    timelock.schedule_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &100u32,
        &proposer,
    );

    e.ledger().set_timestamp(e.ledger().timestamp() + 101);

    timelock.execute_batch(
        &targets,
        &functions,
        &args_list,
        &predecessor,
        &salt,
        &Some(executor),
    );
    let after = e.events().all().events().len();

    // 2x OperationExecuted + 2x BatchCallExecuted
    assert_eq!(after, 4);
}

#[test]
fn test_execute_op_rejects_self_target() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, _, _, _proposer, _executor) = setup_with_external_admin(&e);
    let args = Vec::<Val>::new(&e);

    let result = timelock.try_execute_op(
        &timelock.address,
        &Symbol::new(&e, "update_delay"),
        &args,
        &zero_bytes(&e),
        &zero_bytes(&e),
        &None,
    );
    assert!(result.is_err());
}

#[test]
fn test_execute_batch_rejects_self_target() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, _target, _, _proposer, _executor) = setup_with_external_admin(&e);

    let targets = vec![&e, timelock.address.clone()];
    let functions = vec![&e, Symbol::new(&e, "update_delay")];
    let args_list = vec![&e, Vec::<Val>::new(&e)];

    let result = timelock.try_execute_batch(
        &targets,
        &functions,
        &args_list,
        &zero_bytes(&e),
        &zero_bytes(&e),
        &None,
    );
    assert!(result.is_err());
}

// ============================================================================
// Role Management Tests
// ============================================================================

#[test]
fn test_grant_and_revoke_role() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, _, admin, _, _) = setup_with_external_admin(&e);
    let new_proposer = Address::generate(&e);

    assert!(
        timelock
            .has_role(&new_proposer, &symbol_short!("proposer"))
            .is_none()
    );

    timelock.grant_role(&new_proposer, &symbol_short!("proposer"), &admin);
    assert!(
        timelock
            .has_role(&new_proposer, &symbol_short!("proposer"))
            .is_some()
    );

    timelock.revoke_role(&new_proposer, &symbol_short!("proposer"), &admin);
    assert!(
        timelock
            .has_role(&new_proposer, &symbol_short!("proposer"))
            .is_none()
    );
}

// ============================================================================
// Timestamp Tests
// ============================================================================

#[test]
fn test_get_timestamp() {
    let e = Env::default();
    e.mock_all_auths();

    let (timelock, target, _, proposer, _) = setup_with_external_admin(&e);

    let initial_timestamp = e.ledger().timestamp();
    let predecessor = zero_bytes(&e);
    let salt = zero_bytes(&e);
    let args: Vec<Val> = vec![&e, 42u32.into_val(&e)];
    let delay = 100u32;

    let op_id = timelock.schedule_op(
        &target.address,
        &symbol_short!("set_value"),
        &args,
        &predecessor,
        &salt,
        &delay,
        &proposer,
    );

    let ready_timestamp = timelock.get_timestamp(&op_id);
    assert_eq!(ready_timestamp, initial_timestamp + u64::from(delay));
}
