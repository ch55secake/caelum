# Caelum

**Your code. New horizons.**

Caelum is a local-first AI coding workspace for macOS, with a native Rust core shared by its Tauri desktop app and standalone CLI. The project is in its foundation milestone: the desktop shell can select and validate a local project, while agent sessions, persistence, and code review are not implemented yet.

## Project status

- **Desktop:** Tauri 2 + React + TypeScript + Vite + Tailwind CSS v4; native folder picker and Rust-validated project paths.
- **CLI:** Rust/clap executable scaffold sharing `caelum-core`; currently reports project information and clearly marks agent features as unavailable.
- **Core:** independent Rust library for shared application behavior; currently includes project path validation.
- **Providers:** Codex and OpenCode integrations are planned, not yet implemented.

## Repository layout

```text
crates/caelum-core/       Shared, GUI-independent application logic
crates/caelum-protocol/   Serializable types shared by interfaces
apps/desktop/             Tauri 2 and React desktop application
apps/cli/                 Standalone Rust CLI
docs/                     Architecture and development notes
.github/workflows/        Continuous integration
```

## Development prerequisites

- Rust stable with `rustfmt` and `clippy`
- Node.js 22 and pnpm 10
- Tauri 2 platform prerequisites ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

## Run locally

```bash
pnpm install
pnpm tauri:dev
```

Build the frontend without launching Tauri:

```bash
pnpm desktop:build
```

Build and run the CLI:

```bash
cargo run -p caelum-cli -- --help
cargo run -p caelum-cli -- .
```

The CLI's `chat` and `run` commands are intentionally not active until provider sessions exist. `sessions` currently reports that persistence is not available.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter @caelum/desktop build
```

## Roadmap

See [docs/roadmap.md](docs/roadmap.md) for milestones and [docs/architecture.md](docs/architecture.md) for current boundaries and planned integrations.
