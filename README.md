# Push

Push is ArachOS PID 1: a measured, bounded service supervisor with typed
capability brokerage and explicit recovery states. It owns lifecycle policy,
not hardware mappings, and pins its Slope ABI to an immutable revision.

The service catalog includes Corinth and the full COSMIC session chain:
`seatd`, `dbus-broker`, `pipewire`, `wireplumber`, `cosmic-comp`,
`cosmic-greeter`, `cosmic-session`, and the COSMIC desktop portal. A service
enters the measured boot set only when its artifact and dependencies are
present in the signed ArachOS image.

The default `os-bin` image remains the measured C0 bootstrap profile and
launches only its probe service. A production desktop image must be built
with `--features os-bin,cosmic-boot`; that immutable profile promotes the
complete seat, D-Bus, audio, compositor, greeter, session, and portal chain
together after the signed COSMIC bundle has been assembled. Push will not
infer a partial desktop from files that happen to exist on disk.

Rust implements supervision and the capability boundary. Fortran supplies an
optional bounded priority scorer. Idris 2 gives the boot sequence a total
transition model, and Agda proves that the greeter and session cannot skip
their required predecessor stages.

## Validation

```sh
cargo fmt --all -- --check
cargo test --features fortran-policy
## production desktop supervisor profile
cargo check --features os-bin,cosmic-boot
scripts/check-formal-models.sh
```
