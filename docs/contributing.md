# Contributing

Keep changes focused and follow the existing Rust modules. Application state,
provider behavior and filesystem/Git/GitHub operations belong in `caelum-core`;
native editors, focus and rendering belong in `apps/desktop`.

## Checks

On macOS, from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=4
python3 -m unittest discover -s scripts/release -p 'test_*.py'
python3 -m unittest discover -s scripts/profile -p 'test_*.py'
```

On Linux, select `-p caelum-core -p caelum-protocol -p caelum-cli` in place of
`--workspace`. `make check` chooses the packages for the host automatically.

`apps/desktop/src/tests.rs` drives Rune's real runtime with keyboard/pointer
input and injected application events. It covers draft ownership, focus,
streaming, approvals/questions, bounded long histories and large reviews.
Core tests use fake provider processes and local HTTP servers for deterministic
protocol, recovery and lifecycle checks. Real-service smoke tests are opt-in;
see the [desktop development guide](../apps/desktop/README.md).

For visual changes, run `make previews` and inspect light/dark, inspector,
compact and settings scenes. Native frame measurements are described in
[performance](performance/README.md).

CI runs portable Rust checks on Linux, native checks/previews and packaging on
macOS, and Python release/profile checks. The historical required context
`desktop-frontend-build` gates the native-test job during the stacked cutover;
it runs no web build. `desktop-bundle-build` verifies the app, DMG and CLI archive.

Use Conventional Commits and the repository's pull-request template.
