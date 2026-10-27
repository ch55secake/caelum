# Contributing

## Local checks

Run the checks relevant to your change before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter @caelum/desktop build
```

Use small, focused changes. Keep `caelum-core` independent from Tauri and UI code, and do not claim incomplete integrations as working features. Add tests with core behavior and update the docs when architecture or setup changes.

## Security

Do not add credentials to source, logs, or SQLite. Avoid shell interpolation for user-controlled paths or prompts. Never discard, reset, or commit project changes automatically.
