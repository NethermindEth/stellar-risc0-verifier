[&larr; Back to docs](README.md) | [Main README](../README.md)

# Upgrading the Groth16 verifier (new RISC Zero parameters)

When RISC Zero ships new verifier parameters (control IDs and/or Groth16 verification key), you need to deploy a new Groth16 verifier version with a new selector.

## Why upgrades create a new selector

The router dispatches by the first 4 bytes of `seal` (the selector), which is derived from the verifier parameters. Different parameters = different selector. This means:

- multiple verifier versions coexist behind one router address
- upgrades are safe: add the new selector via timelock, optionally deprecate the old ones

## Step 1 - Update the parameters file

Edit [`contracts/groth16-verifier/parameters.json`](../contracts/groth16-verifier/parameters.json).

This gets embedded at build time. The Groth16 verifier’s `build.rs` derives everything from it:

- `selector()` and `version()` on-chain
- control root / control ID constants
- the verification key

## Step 2 - Build and confirm the new selector

```bash
stellar contract build --optimize
```

The build runs `build.rs` which computes the selector. You can confirm it from the build output (it prints the selector) or after deploy by querying:

```bash
stellar contract invoke --send=no --network <net> --source <identity> --id <verifier_contract_id> -- selector
stellar contract invoke --send=no --network <net> --source <identity> --id <verifier_contract_id> -- version
```

## Step 3 - Deploy the new verifier + emergency stop

```bash
./scripts/manage.sh deploy-verifier -n <network> -a <identity>
```

This deploys the Groth16 verifier and an emergency-stop wrapper around it (guardian defaults to the deployer; override with `--estop-owner`). The verifier is recorded in `deployment.toml` as `unroutable=true` until it’s added to the router.

## Step 4 - Add the new selector to the router (timelocked)

```bash
# schedule
./scripts/manage.sh schedule-add-verifier -n <network> -a <identity> --selector <selector>

# execute (after the timelock delay)
./scripts/manage.sh execute-add-verifier -n <network> -a <identity> --selector <selector>
```

Once executed, proofs carrying that selector will verify through the router.

## Step 5 - What about the old selectors?

- **Routine upgrade** (no vulnerability): keep older selectors active for backward compatibility as long as you want.
- **Security deprecation**: activate the emergency stop (immediate), and/or remove the selector from the router via timelock (permanent tombstone).

> [!CAUTION]
> Removal is irreversible. Once a selector is removed, it cannot be re-added.

## References

- Selector derivation: [`contracts/groth16-verifier/build.rs`](../contracts/groth16-verifier/build.rs)
- Parameters file: [`contracts/groth16-verifier/parameters.json`](../contracts/groth16-verifier/parameters.json)
- Original design doc: [`risc0-ethereum/contracts/version-management-design.md`](../risc0-ethereum/contracts/version-management-design.md)
