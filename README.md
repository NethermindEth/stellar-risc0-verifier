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

On-chain [RISC Zero][risczero] proof verification for the [Stellar][stellar] blockchain ([Soroban][soroban] smart contracts), built by [NethermindEth][nethermind]. The contract architecture — selector-based router, emergency stop proxies, and timelocked governance — follows the version-management pattern established in [risc0-ethereum][risc0-ethereum].

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
              ┌────────────────┼─────────────────┐
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

- **TimelockController** — governance contract; all router mutations are delayed.
- **VerifierRouter** — routes `verify()` calls by the first 4 bytes of the proof seal.
- **EmergencyStop** — per-verifier circuit breaker; permanently disables a verifier.
- **Groth16Verifier** — production verifier for RISC Zero Groth16 (BN254) proofs.

For a deeper design discussion, see the [architecture document](docs/architecture.md).

## Getting started

- **[Verify a proof](docs/verifying-risc0-proofs.md)** — integrate from your Soroban contract or verify via CLI.
- **[Deploy the system](docs/deploying-with-manage-sh.md)** — deploy the full verifier stack with `manage.sh`.
- **[Operations reference](scripts/README.md)** — roles, delay updates, emergency stop, removal.
- **[Upgrade Groth16 parameters](docs/upgrading-groth16-verifier.md)** — deploy a new verifier version.
- **[Architecture](docs/architecture.md)** — system design, governance model, security considerations.

See the [docs index](docs/README.md) for the full list.

## Contributing

Contributions are welcome — please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

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
[soroban]: https://soroban.stellar.org/
[nethermind]: https://www.nethermind.io/
[risc0-ethereum]: https://github.com/risc0/risc0-ethereum
