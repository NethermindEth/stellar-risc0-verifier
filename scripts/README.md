[&larr; Back to docs](../docs/README.md) | [Main README](../README.md)

# Operations Reference

How to deploy and manage the RISC Zero Stellar verifier contracts.

> [!NOTE]
> All commands in this guide assume your current working directory is the root of the repo.

> [!TIP]
> Looking for shorter, task-focused docs?
>
> - [Docs index](../docs/README.md)
> - [Deploy/manage with scripts](../docs/deploying-with-manage-sh.md)
> - [Verify proofs (router `verify()`)](../docs/verifying-risc0-proofs.md)

## Architecture

Four contracts make up the system:

```
                    ┌─────────────────────┐
                    │  TimelockController │──── proposer/executor/canceller roles
                    │   (owns the router) 
                    └──────────┬──────────┘
                               │ owner
                    ┌──────────▼──────────┐
                    │   VerifierRouter    │──── routes verify() calls by selector
                    └──────────┬──────────┘
                               │ selector lookup
              ┌────────────────┼─────────────────┐
              │                                  │
   ┌──────────▼──────────┐            ┌──────────▼──────────┐
   │   EmergencyStop     │            │   EmergencyStop     │
   │  (wraps verifier)   │            │  (wraps verifier)   │
   └──────────┬──────────┘            └──────────┬──────────┘
              │                                  │
   ┌──────────▼──────────┐            ┌──────────▼──────────┐
   │  Groth16Verifier    │            │  MockVerifier       │
   │  (production)       │            │  (testing only)     │
   └─────────────────────┘            └─────────────────────┘
```

- **TimelockController**: Enforces a delay on privileged operations (adding/removing verifiers, role changes). All router modifications go through here.
- **VerifierRouter**: Routes verification requests to the right verifier based on a 4-byte selector prefix in the proof seal.
- **EmergencyStop**: Lets the admin permanently pause a verifier. Owned by the admin (not the timelock) so it can act immediately.
- **Groth16Verifier**: Production verifier, validates RISC Zero Groth16 proofs over BN254.
- **MockVerifier**: Dev-only, accepts mock proofs without cryptographic verification.

## Dependencies

- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli) (`cargo install stellar-cli --locked`)
- Python 3.11+ (for config management)
- Rust toolchain with `wasm32v1-none` target

## Configuration

Deployment state lives in `deployment.toml` at the repo root. `manage.sh` updates it automatically after each operation.

```toml
[chains.stellar-testnet]
name = "Stellar Testnet"
admin = "GABC..."
router = "CABC..."
timelock-controller = "CDEF..."
timelock-delay = 0

[[chains.stellar-testnet.verifiers]]
name = "groth16-verifier"
version = "1.2.0"
selector = "abc123de"
verifier = "CGHI..."
estop = "CJKL..."
unroutable = false
```

## Setup

### [OTPIONAL] Create a Stellar account

```sh
stellar keys generate deployer --network testnet
```

To fund the account on testnet:

```sh
stellar keys fund deployer --network testnet
```

## Deploy the timelocked router

Deploys both `TimelockController` and `VerifierRouter`, with the router owned by the timelock.

> [!IMPORTANT]
> Adjust `--min-delay` to a value appropriate for the environment (e.g. `0` for testing, `604800` (7 days) for mainnet).

```sh
./scripts/manage.sh deploy-router -n testnet -a deployer --min-delay 0
```

The deployer gets `proposer`, `executor`, and bootstrap `admin` roles on the timelock.

Check it worked:

```sh
./scripts/manage.sh status -n testnet
```

## Deploy a Groth16 verifier with emergency stop

Multi-step, guarded by the timelock.

### Step 1: Deploy the verifier

Deploys the `Groth16Verifier` and an `EmergencyStop` wrapper:

```sh
./scripts/manage.sh deploy-verifier -n testnet -a deployer
```

> [!IMPORTANT]
> The emergency stop owner defaults to the deployer address (not the timelock).
> This is intentional: the estop must be activatable immediately without a timelock delay.
> To set a different owner, use `--estop-owner <address>`.

At this point the verifier is deployed but **unroutable** (not yet in the router).

### Step 2: Schedule adding the verifier to the router

```sh
./scripts/manage.sh schedule-add-verifier -n testnet -a deployer \
    --selector <selector>
```

The `<selector>` was printed during `deploy-verifier`. The estop address is resolved from `deployment.toml` automatically.

If the timelock delay is non-zero, wait for it to pass before executing.

### Step 3: Execute the add-verifier operation

```sh
./scripts/manage.sh execute-add-verifier -n testnet -a deployer \
    --selector <selector>
```

Done. The router now dispatches verification requests with this selector to the new verifier.

## Deploy a mock verifier (testing only)

> [!WARNING]
> The mock verifier provides no security guarantees and accepts any receipt matching the mock format.
> It cannot be deployed to mainnet.

```sh
./scripts/manage.sh deploy-mock-verifier -n testnet -a deployer
```

Selector defaults to `00000000`. To use a different one:

```sh
./scripts/manage.sh deploy-mock-verifier -n testnet -a deployer --selector deadbeef
```

Then add it to the router with the same `schedule-add-verifier` / `execute-add-verifier` flow above.

## Remove a verifier

Two-step, timelocked.

### Schedule

```sh
./scripts/manage.sh schedule-remove-verifier -n testnet -a deployer \
    --selector <selector>
```

### Execute (after the delay)

```sh
./scripts/manage.sh execute-remove-verifier -n testnet -a deployer \
    --selector <selector>
```

> [!NOTE]
> Removal is permanent. The selector is tombstoned and can't be re-registered.

## Update the timelock minimum delay

Two-step self-admin process.

### Schedule

```sh
./scripts/manage.sh schedule-update-delay -n testnet -a deployer \
    --new-delay 3600
```

### Execute

```sh
./scripts/manage.sh execute-update-delay -n testnet -a deployer \
    --new-delay 3600
```

Uses the self-admin auth-entry flow automatically.

> [!TIP]
> You can pass `--salt` (and optional `--predecessor`) on both schedule and execute to disambiguate operations when needed.
> The same self-admin auth-entry execution path is used by `execute-grant-role` and `execute-revoke-role`.
> For CLI compatibility, use a 64-char hex salt that includes at least one letter (`a-f`); numeric-only values may be rejected.

## Grant a role on the timelock

Two-step self-admin process. Supported roles: `proposer`, `executor`, `canceller`.
`--target-account` takes a Stellar address directly (`G...`) or you can resolve an alias inline:

```sh
./scripts/manage.sh schedule-grant-role --role proposer --target-account $(stellar keys address deployer)
```

### Schedule

```sh
./scripts/manage.sh schedule-grant-role -n testnet -a deployer \
    --role executor --target-account GABC... \
    --salt abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890
```

### Execute

```sh
./scripts/manage.sh execute-grant-role -n testnet -a deployer \
    --role executor --target-account GABC... \
    --salt abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890
```

## Revoke a role on the timelock

### Schedule

```sh
./scripts/manage.sh schedule-revoke-role -n testnet -a deployer \
    --role executor --target-account GABC... \
    --salt fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321
```

### Execute

```sh
./scripts/manage.sh execute-revoke-role -n testnet -a deployer \
    --role executor --target-account GABC... \
    --salt fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321
```

> [!TIP]
> If scheduling fails with `Error(Contract, #4000)`, that operation hash is already scheduled.
> Either execute the existing operation or schedule with a new `--salt`, then pass the same `--salt` on execute.
> Use salts with at least one hex letter (`a-f`) to avoid parser issues on numeric-only values.

## Renounce a role

If a key is compromised, you can drop your own roles immediately (no timelock delay):

> [!WARNING]
> Renouncing roles on the timelock may make it permanently inoperable if no other accounts hold the required roles.

```sh
./scripts/manage.sh renounce-role -n testnet -a deployer --role proposer
```

Repeat for each role (`proposer`, `executor`, `canceller`, `bootstrap`).

## Cancel a pending operation

Kill a scheduled operation before it executes:

```sh
./scripts/manage.sh cancel-operation -n testnet -a deployer \
    --operation-id <op-id>
```

Requires the `canceller` role (proposers get it automatically).

## Activate the emergency stop

> [!WARNING]
> Activating the emergency stop **permanently** pauses the verifier. This cannot be undone.

By selector (resolves the estop address from config automatically):

```sh
./scripts/manage.sh activate-estop -n testnet -a deployer \
    --selector <selector>
```

Or by estop contract address directly:

```sh
./scripts/manage.sh activate-estop -n testnet -a deployer \
    --estop <estop-contract-id>
```

Confirm it's active:

```sh
./scripts/manage.sh status -n testnet
```

## Check deployment status

Shows all deployed contracts, on-chain state, and verifier status:

```sh
./scripts/manage.sh status -n testnet
```

## Command reference

Run `./scripts/manage.sh --help` for the full list of subcommands and flags.

| Command | Description |
|---------|-------------|
| `deploy-router` | Deploy timelock + router |
| `deploy-verifier` | Deploy groth16 verifier + emergency stop |
| `deploy-mock-verifier` | Deploy mock verifier (testing only) |
| `schedule-add-verifier` | Schedule adding a verifier to the router |
| `execute-add-verifier` | Execute the add-verifier operation |
| `schedule-remove-verifier` | Schedule removing a verifier |
| `execute-remove-verifier` | Execute the remove-verifier operation |
| `schedule-update-delay` | Schedule updating the timelock delay |
| `execute-update-delay` | Execute the delay update |
| `schedule-grant-role` | Schedule granting a role |
| `execute-grant-role` | Execute the grant-role operation |
| `schedule-revoke-role` | Schedule revoking a role |
| `execute-revoke-role` | Execute the revoke-role operation |
| `renounce-role` | Renounce a role (immediate, no timelock) |
| `cancel-operation` | Cancel a pending timelock operation |
| `activate-estop` | Activate the emergency stop |
| `status` | Show deployment status |
