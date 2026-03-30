# Contributing

## Commit signing

[Commit signing](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits) is required:

```sh
git config commit.gpgsign true
```

## Prerequisites

* [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
* Soroban WASM target: `rustup target add wasm32v1-none`
* [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli): `cargo install stellar-cli --locked`
* [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
* [typos](https://github.com/crate-ci/typos)
* [cargo-sort](https://github.com/DevinR528/cargo-sort)

## Workspace structure

Cargo workspace with these crates:

| Crate | Path | Description |
|-------|------|-------------|
| `risc0-router` | `contracts/risc0-router` | Routes `verify()` calls by selector |
| `groth16-verifier` | `contracts/groth16-verifier` | Groth16 proof verification (BN254) |
| `emergency-stop` | `contracts/emergency-stop` | Guardian-controlled circuit breaker |
| `timelock` | `contracts/timelock` | Governance contract (delayed execution) |
| `risc0-interface` | `contracts/interface` | Soroban client interfaces and shared types |
| `mock-verifier` | `contracts/mock-verifier` | Test-only verifier |
| `build-utils` | `tools/build-utils` | Build-time utilities |

## Code quality

Install the pre-push hook:

```sh
git config core.hooksPath .githooks
```

## Run CI locally

Fastest way to run the full suite:

```sh
make check          # lint + test
```

Or run individual steps:

```sh
make lint           # cargo sort, fmt, clippy, deny, typos
make test           # build + cargo test
```

Without Make:

```sh
cargo sort --workspace --check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo deny check
typos
```

## Docs

Generate and open the Rust API docs:

```sh
cargo doc --workspace --no-deps --open
```

## Tests

```sh
cargo test --workspace
```

CI also runs a [gas benchmark](.github/workflows/gas-benchmark.yml) to track contract execution costs over time.
