> [!IMPORTANT]
> This project has **not been audited**.

# Stellar RISC Zero Verifier

[![License][license-badge]][license-url]
[![Docs][docs-badge]][docs-url]
[![Build][build-badge]][build-url]
[![Lint][lint-badge]][lint-url]
[![Coverage][coverage-badge]][coverage-url]
[![Dependencies][deps-badge]][deps-url]
[![UB][ub-badge]][ub-url]

On-chain [RISC Zero][risczero] proof verification for [Stellar][stellar]. The contract
architecture mirrors the version-management pattern from [risc0-ethereum][risc0-ethereum].

## Getting started

- **[Verify a proof](docs/verifying-risc0-proofs.md)**: integrate from your Soroban contract or verify via CLI
- **[Deploy & operate](scripts/README.md)**: deploy the verifier stack, manage roles, delays, emergency stop
- **[Upgrade Groth16 parameters](docs/upgrading-groth16-verifier.md)**: deploy a new verifier version when RISC Zero params change
- **[Architecture](docs/architecture.md)**: how it all fits together

More in the [docs index](docs/README.md).


## Architecture

```text
                    ┌─────────────────────┐
                    │  TimelockController │──── proposer / executor / canceller
                    │   (owns the router) │
                    └──────────┬──────────┘
                               │ owner
                    ┌──────────▼──────────┐
                    │   VerifierRouter    │──── routes verify() by 4-byte selector
                    └──────────┬──────────┘
                               │ selector lookup
              ┌──────────────────────────────────┐
              │                                  │
   ┌──────────▼──────────┐            ┌──────────▼──────────┐
   │   EmergencyStop     │            │   EmergencyStop     │
   │  (wraps verifier)   │            │  (wraps verifier)   │
   └──────────┬──────────┘            └──────────┬──────────┘
              │                                  │
   ┌──────────▼──────────┐            ┌──────────▼──────────┐
   │  Groth16Verifier    │            │  (other verifiers)  │
   └─────────────────────┘            └─────────────────────┘
```

- **TimelockController**: governance; all router mutations go through a delay
- **VerifierRouter**: routes `verify()` by the first 4 bytes of the proof seal
- **EmergencyStop**: per-verifier circuit breaker, permanently disables a verifier
- **Groth16Verifier**: verifies RISC Zero Groth16 (BN254) proofs

See [architecture](docs/architecture.md) for the full design.


## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

## Security

To report a vulnerability, see [SECURITY.md](SECURITY.md).

## License

[Apache 2.0](LICENSE)

<!-- badge links -->
[license-badge]: https://img.shields.io/badge/License-Apache_2.0-blue.svg
[license-url]: LICENSE
[docs-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/docs.yml/badge.svg
[docs-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/docs.yml
[build-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/build-and-test.yml/badge.svg
[build-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/build-and-test.yml
[lint-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/linter.yml/badge.svg
[lint-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/linter.yml
[coverage-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/coverage.yml/badge.svg
[coverage-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/coverage.yml
[deps-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/dependency-audit.yml/badge.svg
[deps-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/dependency-audit.yml
[ub-badge]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/ub-detection.yml/badge.svg
[ub-url]: https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/ub-detection.yml

<!-- external links -->
[risczero]: https://www.risczero.com/
[stellar]: https://stellar.org/
[risc0-ethereum]: https://github.com/risc0/risc0-ethereum
