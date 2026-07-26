# Pomotui

Pomotui is a reliable Pomodoro timer controlled through a terminal interface,
command line, and Waybar. Installation and usage are documented in
[`docs/user-guide.md`](docs/user-guide.md); the product language is defined in
[`CONTEXT.md`](CONTEXT.md), and accepted architecture decisions live in
[`docs/adr/`](docs/adr/).

## Workspace checks

Run the same checks locally and in CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

The crate dependency policy is documented in
[`docs/architecture/crate-boundaries.md`](docs/architecture/crate-boundaries.md).
