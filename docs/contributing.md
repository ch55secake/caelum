# Contributing

## Local checks

Run the checks relevant to your change before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter @caelum/desktop build
pnpm --filter @caelum/desktop test
pnpm --filter @caelum/desktop test:ui
```

Use small, focused changes. Keep `caelum-core` independent from Tauri and UI code, and do not claim incomplete integrations as working features. Add tests with core behavior and update the docs when architecture or setup changes.

## Browser tests

Playwright scenarios live in feature-specific `apps/desktop/tests/*.spec.ts` files. `tests/fixtures/desktop.ts` supplies an isolated mock of the native command/event boundary for each page; `tests/fixtures/helpers.ts` contains shared navigation and provider-history helpers. Specs must not import other specs or depend on their execution order.

Run one area with `pnpm --filter @caelum/desktop test:ui tests/images.spec.ts`, or use `--grep` to select a scenario. Keep provider-specific variants in the same feature spec and use scoped fixture failures and gates for race-condition checks.

CI runs browser tests in the official Playwright container with browsers and Linux libraries preinstalled. The image version comes from the installed `@playwright/test` package, so lockfile updates select the matching image automatically. The container reuses the checkout's installed dependencies and the pnpm version declared in `package.json`.

## Security

Do not add credentials to source, logs, or SQLite. Avoid shell interpolation for user-controlled paths or prompts. Never discard, reset, or commit project changes automatically.
