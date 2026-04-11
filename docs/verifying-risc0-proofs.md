[&larr; Back to docs](README.md) | [Main README](../README.md)

# Verify a RISC Zero Groth16 proof on Stellar (Soroban)

**Jump to**: [Generate a proof](#step-1--generate-a-groth16-proof-risc-zero-zkvm) | [Deploy locally](#step-2--deploy-the-verifier-system-on-stellar-local) | [Verify (CLI)](#step-3--verify-the-proof-cli) | [Verify (contract)](#verify-from-another-soroban-contract-integration)

The short version: call the router’s `verify()` with `seal`, `image_id`, and `journal_digest`.

For deployment/governance (timelock, roles, upgrades), see the [operations reference](../scripts/README.md). This page covers the verification path only.

## Prerequisites

- Rust + Cargo: https://rustup.rs/
- Stellar CLI (for deploy/invoke; for example: `cargo install stellar-cli --locked`)
- RISC Zero toolchain (`rzup`) and `cargo risczero`
- Docker + an **x86_64** machine to *generate* Groth16 proofs

> [!NOTE]
> **Apple Silicon (arm64 macOS)**: Groth16 proof generation typically requires x86_64. If you’re on arm64 macOS, generate the proof on an x86_64 Linux box (or VM/CI runner) and run the Stellar verification steps from anywhere.

Install RISC Zero (example):

```bash
curl -L https://risczero.com/install | bash
rzup install
```

On the machine that generates proofs, also install the Groth16 component:

```bash
rzup install risc0-groth16
```

## What the verifier expects

Three values:

1) **`image_id`** (32 bytes): the guest program ID you expect to have been executed.
2) **`journal_digest`** (32 bytes): `sha256(journal_bytes)`, i.e. the digest of the guest journal.
3) **`seal`** (bytes): the Groth16 seal from `encode_seal()` (`risc0-ethereum-contracts`). Includes a routing prefix the router uses to dispatch to the right verifier.

## Step 1 - Generate a Groth16 proof (RISC Zero zkVM)

Standard RISC Zero quickstart flow, but with Groth16 enabled and outputs saved in a format the CLI can consume.

### 1) Create a zkVM project

```bash
cargo risczero new my_project --guest-name guest_program
cd my_project
```

### 2) Add host dependencies

In `host/Cargo.toml`, you need `risc0-ethereum-contracts` (to encode the seal), `hex`, and `sha2`:

```toml
[dependencies]
methods = { path = "../methods" }
risc0-zkvm = { version = "^3.0" }
risc0-ethereum-contracts = "^3.0"
sha2 = "0.10"
hex = "0.4"
```

### 3) Prove with Groth16 and write `proof.txt`

In `host/src/main.rs`, switch to Groth16 and write out three hex lines (`seal`, `image_id`, `journal_digest`). Adapt the `ELF` / `ID` constant names to your guest:

```rust
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use risc0_ethereum_contracts::encode_seal;
use sha2::{Digest, Sha256};
use std::fs;

fn main() {
    let env = ExecutorEnv::builder().build().unwrap();
    let prover = default_prover();

    let opts = ProverOpts::groth16();
    let prove_info = prover
        .prove_with_opts(env, methods::GUEST_PROGRAM_ELF, &opts)
        .unwrap();
    let receipt = prove_info.receipt;

    let seal = encode_seal(&receipt).unwrap();
    let journal_digest: [u8; 32] = Sha256::digest(&receipt.journal).into();
    let image_id: [u8; 32] = methods::GUEST_PROGRAM_ID;

    let out = format!(
        "{}\n{}\n{}\n",
        hex::encode(seal),
        hex::encode(image_id),
        hex::encode(journal_digest)
    );
    fs::write("proof.txt", out).unwrap();
}
```

Run the host:

```bash
cargo run -p host
```

You should now have `proof.txt`. We write to a file here just to make it easy to pass the values
into the CLI later. In a real application you'd use the seal, image ID, and journal digest directly
in your Rust code (or however you build transactions).

## Step 2 - Deploy the verifier system on Stellar (local)

If you’re verifying against an existing deployment, skip this and just grab the router contract ID. For local testing, run this once. See the [operations reference](../scripts/README.md) for production deployments.

### 1) Start a local network and fund an identity

```bash
stellar container start local

stellar keys generate foo --network local
stellar keys fund foo --network local
```

### 2) Deploy router + verifier and register it

From the root of this repo:

```bash
./scripts/manage.sh deploy-router -n local -a foo --min-delay 0
./scripts/manage.sh deploy-verifier -n local -a foo

# deploy-verifier prints the selector and stores it in deployment.toml.
# Use it to register the verifier in the router:
SELECTOR=$(python3 ./scripts/toml_helper.py read deployment.toml chains.stellar-local.verifiers.0.selector)
./scripts/manage.sh schedule-add-verifier -n local -a foo --selector "$SELECTOR"
./scripts/manage.sh execute-add-verifier -n local -a foo --selector "$SELECTOR"
```

Check the router contract ID:

```bash
./scripts/manage.sh status -n local
```

## Verifying on an existing deployment (testnet/mainnet)

If deployments are tracked in `deployment.toml`, grab the router address with:

```bash
python3 ./scripts/toml_helper.py read deployment.toml chains.stellar-testnet.router
```

Then use that contract ID in the `stellar contract invoke ... verify ...` command below.

## Step 3 - Verify the proof (CLI)

Read the three hex lines from `proof.txt`:

```bash
SEAL_HEX=$(sed -n '1p' proof.txt)
IMAGE_ID_HEX=$(sed -n '2p' proof.txt)
JOURNAL_DIGEST_HEX=$(sed -n '3p' proof.txt)
```

Then invoke the **router**’s `verify`:

```bash
stellar contract invoke \
  --send=no \
  --network local \
  --source foo \
  --id <ROUTER_CONTRACT_ID> \
  -- \
  verify \
  --seal "$SEAL_HEX" \
  --image_id "$IMAGE_ID_HEX" \
  --journal "$JOURNAL_DIGEST_HEX"
```

If the simulation succeeds, the proof is valid. Common failures:

- proof was generated with a different RISC Zero version than the deployed verifier
- no verifier registered in the router for this selector
- `journal_digest` mismatch (you passed raw journal bytes instead of the hash, or hashed the wrong thing)

## Verify from another Soroban contract (integration)

Add the interface crate and call the router client:

```toml
[dependencies]
risc0-interface = { git = "https://github.com/NethermindEth/stellar-risc0-verifier", package = "risc0-interface" }
```

```rust
use risc0_interface::RiscZeroVerifierRouterClient;
use soroban_sdk::{Address, Bytes, BytesN, Env};

pub fn verify_risc0(
    env: &Env,
    router: &Address,
    seal: &Bytes,
    image_id: &BytesN<32>,
    journal_digest: &BytesN<32>,
) {
    let router = RiscZeroVerifierRouterClient::new(env, router);
    router.verify(seal, image_id, journal_digest);
}
```
