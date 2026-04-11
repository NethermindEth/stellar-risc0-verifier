[&larr; Back to docs](README.md) | [Main README](../README.md)

# Architecture

How the RISC Zero Stellar verifier system works: contract stack, governance, and the reasoning behind the design.

## Overview

Soroban contracts can verify [RISC Zero](https://www.risczero.com/) zkVM proofs on-chain through a single **router** address. Behind that address, verifier implementations can be added, paused, or removed over time. Applications always point at the same contract.

Three design priorities drove the architecture:

- **Immutability at the leaf**: verifier implementations are stateless with no admin functions.
- **Governed mutability at the root**: the router is owned by a timelock, so changes to accepted verifiers are delayed and publicly observable.
- **Independent emergency response**: each verifier has its own emergency stop proxy controlled by a guardian (not the timelock) for when you need to act fast.

## Contract stack

```text
                    ┌─────────────────────┐
                    │  TimelockController │──── proposer / executor / canceller roles
                    │   (owns the router) │
                    └──────────┬──────────┘
                               │ owner
                    ┌──────────▼──────────┐
                    │   VerifierRouter    │──── routes verify() by 4-byte selector
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

### TimelockController

Owns the router. Any privileged operation (adding/removing verifiers) must be scheduled through the timelock and can only execute after a configurable delay.

Roles:
- **Proposer**: schedules operations
- **Executor**: executes operations once the delay passes
- **Canceller**: cancels pending operations (proposers get this automatically)
- **Bootstrap admin**: optional initial admin for setup, should be renounced once governance is in place

The timelock is its own admin for self-targeting operations (delay updates, role grants/revocations). This follows the [OpenZeppelin TimelockController](https://docs.openzeppelin.com/contracts/5.x/api/governance#TimelockController) pattern.

### VerifierRouter

Routes `verify(seal, image_id, journal_digest)` to the right verifier based on a 4-byte **selector** prefix in the `seal`. Internally it's a `selector -> verifier address` mapping. Owned by the timelock.

### EmergencyStop

Thin wrapper around a verifier. A designated **guardian** (typically the deployer, not the timelock) can permanently pause it with no delay.

There are two ways to trigger it:
1. **Guardian call**: the guardian invokes `estop()` directly.
2. **Proof of exploit**: anyone submits a "known-bad" proof that the verifier would incorrectly accept, proving the vulnerability.

Once activated, the stop is **permanent**. There's no unpause.

### Groth16Verifier

Stateless, immutable. Verifies RISC Zero Groth16 proofs over BN254. The parameters (control IDs, verification key) are baked in at build time from [`contracts/groth16-verifier/parameters.json`](../contracts/groth16-verifier/parameters.json), and the 4-byte selector is derived from their hash.

### MockVerifier

Test-only, no cryptographic checks. Fixed selector (typically `00000000`). Can't be deployed to mainnet.

## Selector routing

The first 4 bytes of `seal` are the **selector**. When `router.verify(seal, image_id, journal_digest)` is called:

1. Extract `seal[0..4]` as the selector.
2. Look up the verifier address for that selector.
3. Forward the call through the emergency stop proxy to the verifier.

This is what lets **multiple verifier versions coexist** behind one router address. When RISC Zero ships new parameters (control IDs, verification key), you deploy a new verifier with a new selector. Proofs generated with the new params route to the new verifier; older proofs keep working with the old one.

A selector maps to exactly one verifier. Once removed (tombstoned), it can never be re-added, even with the same verifier address.

## Governance model

### Timelocked operations

Every router modification goes through three steps:

1. **Schedule**: a proposer submits the operation, starting a delay timer.
2. **Wait**: nothing happens until the delay passes. Developers and auditors can review.
3. **Execute**: an executor runs the operation. The router updates.

A canceller can kill a scheduled operation during the delay window.

### Self-administration

Operations targeting the timelock itself (updating the delay, granting/revoking roles) use a self-admin flow: scheduled normally, but execution goes through the timelock's own authorization (`__check_auth` with `OperationMeta`) instead of an external contract call.

### Delay configuration

- **Dev/testing**: `min_delay = 0` (immediate).
- **Production**: something meaningful, e.g. 7 days.

The delay itself can be updated via a timelocked self-admin operation.

## Emergency stop design

The emergency stop is deliberately **separate from the timelock**:

- A delay is great for governance but useless when you need to shut something down now.
- The guardian can pause a verifier **immediately**.
- There's no "unpause". Making it permanent means a compromised guardian key can't toggle the stop on and off.

After activation, the selector stays in the router but all verification calls through it revert. The operator should then schedule a removal via the timelock to tombstone it.

## Selector tombstoning

Removing a selector is timelocked (schedule/execute) and irreversible. Once tombstoned, the selector can never be re-registered, not even with the same verifier address.

Why? If a governance key is compromised, the attacker can't silently swap in a malicious verifier behind an existing selector.
