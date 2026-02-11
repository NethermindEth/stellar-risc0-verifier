//! This is heavily from https://github.com/OpenZeppelin/stellar-contracts/blob/82f97111f9568d32653dd5b3563baf5b73509c89/examples/timelock-controller/src/contract.rs
//!
//! TimeLock Controller Contract.
//!
//! This contract implements a timelock controller with role-based access
//! control, following the OpenZeppelin pattern for Stellar/Soroban.
//!
//! # Architecture
//!
//! ```text
//!                ┌─────────┐
//!                │  Admin  │
//!                └────┬────┘
//!                     │
//!          ┌──────────┼──────────┐
//!          │          │          │
//!          │    update_delay()   │
//!          │          │          │
//!     ┌────▼───┐      │     ┌────▼────┐
//!     │Proposer│      │     │Executor │
//!     └────┬───┘      │     └────┬────┘
//!          │          │          │
//! schedule_op()       │    execute_op()
//! cancel_op()         │          │
//!          │          │          │
//!          │          ▼          │
//!          │   ┌─────────────┐   │
//!          └──►│  Timelock   │◄──┘
//!              │ Controller  │◄─── (self-admin when Admin == contract)
//!              └──────┬──────┘
//!                     │
//!              invoke (as owner)
//!                     │
//!                     ▼
//!              ┌─────────────┐
//!              │   Target    │
//!              │  Contract   │
//!              └─────────────┘
//! ```
//!
//! # Roles
//!
//! - **Admin**: Can manage all roles and update the minimum delay. By default,
//!   the contract itself is the admin, meaning admin operations must go through
//!   the timelock process.
//! - **Proposer**: Can schedule operations. Proposers are also automatically
//!   granted the Canceller role.
//! - **Executor**: Can execute operations that are ready. If no executors are
//!   configured, anyone can execute ready operations.
//! - **Canceller**: Can cancel pending operations.
//!
//! # Usage Pattern
//!
//! The timelock controller is typically set as the **owner** of target
//! contracts. This ensures that all privileged operations on those
//! contracts must go through the timelock's proposal lifecycle, providing
//! transparency and allowing time for review before execution.
//!
//! ## Operations on Target Contracts
//!
//! When the timelock controller "owns" a target contract, the proposal
//! lifecycle is:
//!
//! 1. Proposer schedules operations targeting owner-protected functions on the
//!    target contract with a delay >= minimum delay
//! 2. The delay period allows stakeholders to review the proposed changes
//! 3. After the delay passes, executor (or anyone if no executors are
//!    configured) calls `execute_op` to invoke the target contract function
//! 4. Canceller can cancel pending operations before execution
//!
//! ## Self-Administration Operations
//!
//! The contract address itself is the admin (self-administration). For
//! self-administration operations (e.g., updating the minimum delay, granting
//! and revoking roles), the proposal lifecycle is:
//!
//! 1. Proposer schedules the operation targeting the timelock contract itself
//! 2. After the delay passes, call the admin function directly (not via
//!    `execute_op`)
//! 3. The `CustomAccountInterface` implementation validates the operation is
//!    ready and marks it as executed by checking the executor's role and
//!    authorization
//!
//! This approach ensures administrative changes go through the timelock
//! process.
//!
//! **Note**: Self-administration requires special handling because Soroban does
//! not allow re-entrancy: a contract cannot call its own public functions
//! during execution (e.g., `execute_op` cannot internally call `update_delay`
//! on the same contract). To work around this, the `CustomAccountInterface`
//! implementation validates and marks operations as executed without performing
//! the cross-contract call, allowing admin functions to be called directly.
//!
//! ## Optional Bootstrap Admin
//!
//! An optional bootstrap admin can be provided during deployment to aid with
//! initial configuration of proposer/executor/canceller roles without being
//! subject to delay. This bootstrap role should be subsequently renounced in
//! favor of administration through timelocked proposals to ensure all
//! administrative actions have proper oversight and transparency.

#![no_std]

use soroban_sdk::{
    Address, Bytes, BytesN, Env, IntoVal, Symbol, Val, Vec,
    auth::{Context, ContractContext, CustomAccountInterface},
    contract, contracterror, contractevent, contractimpl, contracttype,
    crypto::Hash,
    panic_with_error, symbol_short,
    xdr::ToXdr,
};
use stellar_access::access_control::{
    AccessControl, ensure_role, get_role_member_count, grant_role_no_auth, set_admin,
    set_role_admin_no_auth,
};
use stellar_governance::timelock::{
    Operation, OperationState, TimelockError, cancel_operation, execute_operation,
    get_min_delay as timelock_get_min_delay, get_operation_state, get_timestamp,
    hash_operation as timelock_hash_operation, is_operation_done, is_operation_pending,
    is_operation_ready, operation_exists, schedule_operation, set_execute_operation,
    set_min_delay as timelock_set_min_delay,
};
use stellar_macros::{only_admin, only_role};

#[cfg(test)]
mod test;

/// Role for accounts that can schedule operations.
const PROPOSER_ROLE: Symbol = symbol_short!("proposer");

/// Role for accounts that can execute ready operations.
const EXECUTOR_ROLE: Symbol = symbol_short!("executor");

/// Role for accounts that can cancel pending operations.
const CANCELLER_ROLE: Symbol = symbol_short!("canceller");

/// Role for bootstrap admins that can configure proposer/executor/canceller
/// roles.
const BOOTSTRAP_ADMIN_ROLE: Symbol = symbol_short!("bootstrap");

/// Metadata for self-administration operations.
///
/// This struct is used as the signature type in `CustomAccountInterface` to
/// provide the necessary context for validating and executing
/// self-administration operations.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OperationMeta {
    /// The predecessor operation ID (use all zeros for none).
    pub predecessor: BytesN<32>,
    /// Salt for uniqueness.
    pub salt: BytesN<32>,
    /// The executor address (must have executor role if executors are
    /// configured).
    pub executor: Option<Address>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TimelockControllerError {
    BatchLengthMismatch = 5000,
}

/// Event emitted when a scheduled operation uses a non-zero salt.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSalt {
    #[topic]
    pub id: BytesN<32>,
    pub salt: BytesN<32>,
}

/// Event emitted for each operation in a scheduled batch.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchCallScheduled {
    #[topic]
    pub id: BytesN<32>,
    #[topic]
    pub index: u32,
    pub target: Address,
    pub function: Symbol,
    pub args: Vec<Val>,
    pub predecessor: BytesN<32>,
    pub salt: BytesN<32>,
    pub delay: u32,
}

/// Event emitted for each operation in an executed batch.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchCallExecuted {
    #[topic]
    pub id: BytesN<32>,
    #[topic]
    pub index: u32,
    pub target: Address,
    pub function: Symbol,
    pub args: Vec<Val>,
    pub predecessor: BytesN<32>,
    pub salt: BytesN<32>,
}

/// TimeLock Controller contract.
#[contract]
pub struct TimelockController;

#[contractimpl]
impl CustomAccountInterface for TimelockController {
    type Error = TimelockError;
    type Signature = Vec<OperationMeta>;

    /// Custom authorization check for self-administration operations.
    ///
    /// This enables the timelock contract to execute operations on itself when
    /// the admin is set to the contract's own address. Unlike external
    /// operations which use `execute_op`, self-administration operations are
    /// executed by calling the admin function directly (e.g., `update_delay`,
    /// `grant_role`).
    ///
    /// The `__check_auth` implementation validates that:
    /// - The operation targets the timelock contract itself
    /// - The operation was properly scheduled and is ready for execution
    /// - The predecessor and salt match the scheduled operation
    /// - The executor (if any) has role and has authorized the invocation
    ///
    /// The caller must construct an `OperationMeta` signature containing the
    /// `predecessor` and `salt` values that were used when scheduling the
    /// operation, allowing this function to validate and mark the operation as
    /// executed.
    #[allow(clippy::needless_pass_by_value)]
    fn __check_auth(
        e: Env,
        _signature_payload: Hash<32>,
        context_meta: Vec<OperationMeta>,
        auth_contexts: Vec<Context>,
    ) -> Result<(), Self::Error> {
        if auth_contexts.len() != context_meta.len() {
            panic_with_error!(&e, TimelockError::Unauthorized);
        }

        for (context, meta) in auth_contexts.iter().zip(context_meta) {
            match context.clone() {
                Context::Contract(ContractContext {
                    contract,
                    fn_name,
                    args,
                }) => {
                    // Allow only for self-administration
                    if contract != e.current_contract_address() {
                        panic_with_error!(&e, TimelockError::Unauthorized)
                    }

                    // If no accounts have EXECUTOR_ROLE, anyone can execute a ready operation
                    if get_role_member_count(&e, &EXECUTOR_ROLE) != 0 {
                        // Check the role and the authorization of the executor
                        let args_for_auth = (
                            // adding an additional symbol argument so that intention for the
                            // authorizer is more explicit
                            Symbol::new(&e, "execute_op"),
                            contract.clone(),
                            fn_name.clone(),
                            args.clone(),
                            meta.predecessor.clone(),
                            meta.salt.clone(),
                        )
                            .into_val(&e);

                        let executor = match meta.executor.clone() {
                            Some(exec) => exec,
                            None => panic_with_error!(&e, TimelockError::Unauthorized),
                        };

                        ensure_role(&e, &EXECUTOR_ROLE, &executor);
                        executor.require_auth_for_args(args_for_auth);
                    }

                    let op = Operation {
                        target: contract,
                        function: fn_name,
                        args,
                        predecessor: meta.predecessor,
                        salt: meta.salt,
                    };
                    set_execute_operation(&e, &op);
                }
                _ => panic_with_error!(&e, TimelockError::Unauthorized),
            }
        }
        Ok(())
    }
}

#[contractimpl]
impl TimelockController {
    /// Initializes the timelock controller.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `min_delay` - Initial minimum delay in seconds for operations.
    /// * `proposers` - Accounts to be granted proposer and canceller roles.
    /// * `executors` - Accounts to be granted executor role.
    /// * `admin` - Optional account to be granted a bootstrap role for initial
    ///   setup. The contract itself is always the admin (self-administration).
    ///
    /// # Notes
    ///
    /// - The contract itself is always the admin.
    /// - Proposers are automatically granted the canceller role.
    /// - If an external admin is provided, they receive the bootstrap role that
    ///   can manage proposer/executor/canceller roles and should renounce it
    ///   after initial configuration.
    pub fn __constructor(
        e: &Env,
        min_delay: u32,
        proposers: Vec<Address>,
        executors: Vec<Address>,
        admin: Option<Address>,
    ) {
        let admin_addr = e.current_contract_address();
        set_admin(e, &admin_addr);

        if let Some(bootstrap_admin) = admin {
            // Make BOOTSTRAP_ADMIN_ROLE the admin role for the proposer, executor, and
            // canceller roles
            set_role_admin_no_auth(e, &PROPOSER_ROLE, &BOOTSTRAP_ADMIN_ROLE);
            set_role_admin_no_auth(e, &EXECUTOR_ROLE, &BOOTSTRAP_ADMIN_ROLE);
            set_role_admin_no_auth(e, &CANCELLER_ROLE, &BOOTSTRAP_ADMIN_ROLE);

            grant_role_no_auth(e, &bootstrap_admin, &BOOTSTRAP_ADMIN_ROLE, &admin_addr);
        }

        // Grant proposers both proposer and canceller roles
        for proposer in proposers.iter() {
            grant_role_no_auth(e, &proposer, &PROPOSER_ROLE, &admin_addr);
            grant_role_no_auth(e, &proposer, &CANCELLER_ROLE, &admin_addr);
        }

        for executor in executors.iter() {
            grant_role_no_auth(e, &executor, &EXECUTOR_ROLE, &admin_addr);
        }

        timelock_set_min_delay(e, min_delay);
    }

    /// Schedules an operation for execution after a delay.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `target` - The target contract address.
    /// * `function` - The function name to invoke.
    /// * `args` - The arguments to pass to the function.
    /// * `predecessor` - The predecessor operation ID (use all zeros for none).
    /// * `salt` - Salt for uniqueness (use all zeros for default).
    /// * `delay` - The delay in seconds before the operation can be executed.
    /// * `proposer` - The address proposing the operation (must have proposer
    ///   role).
    ///
    /// # Returns
    ///
    /// The unique identifier (hash) of the scheduled operation.
    ///
    /// # Notes
    ///
    /// * Authorization for `proposer` is required.
    /// * The proposer must have the PROPOSER_ROLE.
    /// * The delay must be >= the minimum delay.
    #[allow(clippy::too_many_arguments)]
    #[only_role(proposer, "proposer")]
    pub fn schedule_op(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        delay: u32,
        proposer: Address,
    ) -> BytesN<32> {
        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        let op_id = schedule_operation(e, &operation, delay);
        if !is_zero_bytes32(e, &operation.salt) {
            emit_call_salt(e, &op_id, &operation.salt);
        }
        op_id
    }

    /// Executes a scheduled operation that is ready.
    ///
    /// **Note**: This function is only for executing operations on external
    /// contracts. For self-administration operations (where target is this
    /// timelock contract), call the admin function directly instead.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `target` - The target contract address.
    /// * `function` - The function name to invoke.
    /// * `args` - The arguments to pass to the function.
    /// * `predecessor` - The predecessor operation ID.
    /// * `salt` - Salt for uniqueness.
    /// * `executor` - The address executing the operation (must have executor
    ///   role if configured).
    ///
    /// # Returns
    ///
    /// The return value from the executed operation.
    ///
    /// # Notes
    ///
    /// * If executors are configured (EXECUTOR_ROLE has members), authorization
    ///   for `executor` is required and the executor must have the
    ///   EXECUTOR_ROLE.
    /// * If no executors are configured, anyone can execute ready operations.
    pub fn execute_op(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        executor: Option<Address>,
    ) -> Val {
        if get_role_member_count(e, &EXECUTOR_ROLE) != 0 {
            let executor =
                executor.expect("executor must be present when executors are configured");
            ensure_role(e, &EXECUTOR_ROLE, &executor);
            executor.require_auth();
        }

        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        execute_operation(e, &operation)
    }

    /// Computes the unique identifier for a batch of operations.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `targets` - The target contract addresses.
    /// * `functions` - The function names to invoke.
    /// * `args_list` - The arguments for each function.
    /// * `predecessor` - The predecessor operation ID.
    /// * `salt` - Salt for uniqueness.
    ///
    /// # Returns
    ///
    /// The unique identifier (hash) for the batch.
    pub fn hash_operation_batch(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args_list: Vec<Vec<Val>>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        validate_batch_lengths(e, &targets, &functions, &args_list);
        hash_operation_batch_inner(e, &targets, &functions, &args_list, &predecessor, &salt)
    }

    /// Schedules a batch of operations for execution after a delay.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `targets` - The target contract addresses.
    /// * `functions` - The function names to invoke.
    /// * `args_list` - The arguments for each function.
    /// * `predecessor` - The predecessor operation ID (use all zeros for none).
    /// * `salt` - Salt for uniqueness (use all zeros for default).
    /// * `delay` - The delay in seconds before the operations can be executed.
    /// * `proposer` - The address proposing the operation (must have proposer
    ///   role).
    ///
    /// # Returns
    ///
    /// The unique identifier (hash) of the scheduled batch.
    ///
    /// # Notes
    ///
    /// * Authorization for `proposer` is required.
    /// * The proposer must have the PROPOSER_ROLE.
    /// * The delay must be >= the minimum delay.
    #[allow(clippy::too_many_arguments)]
    #[only_role(proposer, "proposer")]
    pub fn schedule_batch(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args_list: Vec<Vec<Val>>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        delay: u32,
        proposer: Address,
    ) -> BytesN<32> {
        validate_batch_lengths(e, &targets, &functions, &args_list);
        let batch_id =
            hash_operation_batch_inner(e, &targets, &functions, &args_list, &predecessor, &salt);

        for i in 0..targets.len() {
            let target = targets.get(i).unwrap();
            let function = functions.get(i).unwrap();
            let args = args_list.get(i).unwrap();
            let predecessor = predecessor.clone();
            let salt = salt.clone();
            let operation = Operation {
                target: target.clone(),
                function: function.clone(),
                args: args.clone(),
                predecessor: predecessor.clone(),
                salt: salt.clone(),
            };
            schedule_operation(e, &operation, delay);
            BatchCallScheduled {
                id: batch_id.clone(),
                index: i,
                target,
                function,
                args,
                predecessor,
                salt,
                delay,
            }
            .publish(e);
        }
        if !is_zero_bytes32(e, &salt) {
            emit_call_salt(e, &batch_id, &salt);
        }

        batch_id
    }

    /// Executes a ready batch of operations.
    ///
    /// **Note**: This function is only for executing operations on external
    /// contracts. For self-administration operations, call the admin function
    /// directly instead.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `targets` - The target contract addresses.
    /// * `functions` - The function names to invoke.
    /// * `args_list` - The arguments for each function.
    /// * `predecessor` - The predecessor operation ID.
    /// * `salt` - Salt for uniqueness.
    /// * `executor` - The address executing the operation (must have executor
    ///   role if configured).
    ///
    /// # Returns
    ///
    /// The return values from the executed operations.
    pub fn execute_batch(
        e: &Env,
        targets: Vec<Address>,
        functions: Vec<Symbol>,
        args_list: Vec<Vec<Val>>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        executor: Option<Address>,
    ) -> Vec<Val> {
        validate_batch_lengths(e, &targets, &functions, &args_list);
        let batch_id =
            hash_operation_batch_inner(e, &targets, &functions, &args_list, &predecessor, &salt);

        if get_role_member_count(e, &EXECUTOR_ROLE) != 0 {
            let executor =
                executor.expect("executor must be present when executors are configured");
            ensure_role(e, &EXECUTOR_ROLE, &executor);
            executor.require_auth();
        }

        let mut results = Vec::new(e);
        for i in 0..targets.len() {
            let target = targets.get(i).unwrap();
            let function = functions.get(i).unwrap();
            let args = args_list.get(i).unwrap();
            let predecessor = predecessor.clone();
            let salt = salt.clone();
            let operation = Operation {
                target: target.clone(),
                function: function.clone(),
                args: args.clone(),
                predecessor: predecessor.clone(),
                salt: salt.clone(),
            };
            let result = execute_operation(e, &operation);
            results.push_back(result);
            BatchCallExecuted {
                id: batch_id.clone(),
                index: i,
                target,
                function,
                args,
                predecessor,
                salt,
            }
            .publish(e);
        }

        results
    }

    /// Cancels a scheduled operation.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation to cancel.
    /// * `canceller` - The address cancelling the operation (must have
    ///   canceller role).
    ///
    /// # Notes
    ///
    /// * Authorization for `canceller` is required.
    /// * The canceller must have the CANCELLER_ROLE.
    #[only_role(canceller, "canceller")]
    pub fn cancel_op(e: &Env, operation_id: BytesN<32>, canceller: Address) {
        cancel_operation(e, &operation_id);
    }

    /// Updates the minimum delay for future operations.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `new_delay` - The new minimum delay in seconds.
    ///
    /// # Notes
    ///
    /// * Authorization for admin is required.
    /// * This function should typically be called through the timelock itself
    ///   (self-administration) to ensure transparency.
    #[only_admin]
    pub fn update_delay(e: &Env, new_delay: u32) {
        timelock_set_min_delay(e, new_delay);
    }

    /// Returns the minimum delay in seconds required for operations.
    pub fn get_min_delay(e: &Env) -> u32 {
        timelock_get_min_delay(e)
    }

    /// Computes the unique identifier for an operation.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `target` - The target contract address.
    /// * `function` - The function name to invoke.
    /// * `args` - The arguments to pass to the function.
    /// * `predecessor` - The predecessor operation ID.
    /// * `salt` - Salt for uniqueness.
    ///
    /// # Returns
    ///
    /// The unique identifier (hash) for the operation.
    pub fn hash_operation(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        timelock_hash_operation(e, &operation)
    }

    /// Returns the timestamp at which an operation becomes ready.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// The timestamp (in seconds) when the operation becomes ready.
    /// Returns 0 if the operation doesn't exist or is done.
    pub fn get_timestamp(e: &Env, operation_id: BytesN<32>) -> u64 {
        get_timestamp(e, &operation_id)
    }

    /// Returns the current state of an operation.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// The current state: Unset, Waiting, Ready, or Done.
    pub fn get_operation_state(e: &Env, operation_id: BytesN<32>) -> OperationState {
        get_operation_state(e, &operation_id)
    }

    /// Returns whether an operation exists (scheduled or done).
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// `true` if the operation exists, `false` otherwise.
    pub fn operation_exists(e: &Env, operation_id: BytesN<32>) -> bool {
        operation_exists(e, &operation_id)
    }

    /// Returns whether an operation is pending (waiting or ready).
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// `true` if the operation is pending, `false` otherwise.
    pub fn is_operation_pending(e: &Env, operation_id: BytesN<32>) -> bool {
        is_operation_pending(e, &operation_id)
    }

    /// Returns whether an operation is ready for execution.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// `true` if the operation is ready, `false` otherwise.
    pub fn is_operation_ready(e: &Env, operation_id: BytesN<32>) -> bool {
        is_operation_ready(e, &operation_id)
    }

    /// Returns whether an operation has been executed.
    ///
    /// # Arguments
    ///
    /// * `e` - Access to Soroban environment.
    /// * `operation_id` - The unique identifier of the operation.
    ///
    /// # Returns
    ///
    /// `true` if the operation has been executed, `false` otherwise.
    pub fn is_operation_done(e: &Env, operation_id: BytesN<32>) -> bool {
        is_operation_done(e, &operation_id)
    }
}

fn validate_batch_lengths(
    e: &Env,
    targets: &Vec<Address>,
    functions: &Vec<Symbol>,
    args_list: &Vec<Vec<Val>>,
) {
    if targets.len() != functions.len() || targets.len() != args_list.len() {
        panic_with_error!(e, TimelockControllerError::BatchLengthMismatch);
    }
}

fn hash_operation_batch_inner(
    e: &Env,
    targets: &Vec<Address>,
    functions: &Vec<Symbol>,
    args_list: &Vec<Vec<Val>>,
    predecessor: &BytesN<32>,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let mut data = Bytes::new(e);

    data.append(&targets.to_xdr(e));
    data.append(&functions.to_xdr(e));
    data.append(&args_list.to_xdr(e));
    data.append(&predecessor.clone().into());
    data.append(&salt.clone().into());

    e.crypto().keccak256(&data).into()
}

fn emit_call_salt(e: &Env, id: &BytesN<32>, salt: &BytesN<32>) {
    CallSalt {
        id: id.clone(),
        salt: salt.clone(),
    }
    .publish(e);
}

fn is_zero_bytes32(e: &Env, value: &BytesN<32>) -> bool {
    *value == BytesN::<32>::from_array(e, &[0u8; 32])
}

// Expose role management functions via AccessControl trait
#[contractimpl(contracttrait)]
impl AccessControl for TimelockController {}
