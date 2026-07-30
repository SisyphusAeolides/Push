# Push

Push is Arach OS PID 1: a measured, bounded service supervisor with typed
capability brokerage and explicit recovery states. It owns lifecycle policy,
not hardware mappings, and pins its Slope ABI to an immutable revision.

The service catalog includes Corinth and the full COSMIC session chain:
`dbus-broker`, `cosmic-comp`, `cosmic-greeter`, `cosmic-session`, and the
COSMIC desktop portal. A service enters the measured boot set only when its
artifact and dependencies are present in the signed Arach OS image.

Rust implements supervision and the capability boundary. Fortran supplies an
optional bounded priority scorer. Idris 2 gives the boot sequence a total
transition model, and Agda proves that the greeter and session cannot skip
their required predecessor stages.

## Validation

```sh
cargo fmt --all -- --check
cargo test --features fortran-policy
scripts/check-formal-models.sh
```
