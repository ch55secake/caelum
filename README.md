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

Alternatively, use the Nix development shell for the Rust, Node.js, pnpm, and Make toolchain:

```bash
nix develop
make help
```

On Linux, the shell also provides Tauri's native build libraries. On macOS, install the Tauri platform prerequisites on the host.

## Run locally

```bash
make help
make dev
```

`make dev` installs the frontend dependencies if needed and starts the Tauri desktop app. The machine must also have the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/) installed.

Build the macOS app bundle, or just the frontend:

```bash
make app
make frontend
```

Build and run the CLI scaffold:

```bash
make cli
make cli-run PROJECT=.
```

The CLI currently validates and reports the project path; `chat` and `run` are not active until provider sessions exist. `sessions` reports that persistence is not available.

## Verification

```bash
make check
```

## Releases

See [docs/releasing.md](docs/releasing.md) for signed macOS release and Homebrew setup instructions.

Caelum is licensed under the [MIT License](LICENSE).

## Roadmap

See [docs/roadmap.md](docs/roadmap.md) for milestones and [docs/architecture.md](docs/architecture.md) for current boundaries and planned integrations.
