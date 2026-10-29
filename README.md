# Caelum

**Your code. New horizons.**

Caelum is a local-first AI coding workspace for macOS, with a native Rust core shared by its Tauri desktop app and standalone CLI. The desktop supports project-scoped Codex and OpenCode sessions; Caelum-managed persistence and code review are not implemented yet.

## Project status

- **Desktop:** Tauri 2 + React + TypeScript + Vite + Tailwind CSS v4; native folder picker and Rust-validated project paths.
- **CLI:** Standalone Rust/clap executable sharing `caelum-core`; can launch the desktop app or run Codex and OpenCode workflows in a terminal.
- **Core:** independent Rust library for shared application behavior, project validation, and provider adapters.
- **Providers:** Codex and OpenCode integrations use their local server protocols and share the desktop session and approval experience.

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

Build and use the standalone CLI:

```bash
make cli
make cli-run PROJECT=.
caelum chat [PROJECT] [--provider codex|opencode]
caelum run "Fix the failing tests" --project .
caelum sessions [PROJECT] --provider opencode
```

With no subcommand, `caelum [PROJECT]` opens the project in the installed Caelum desktop app (macOS). Terminal workflows work without the desktop app. Use `--session ID` with `chat` or `run` to resume a provider-owned session; `/quit` exits interactive chat. Provider permissions are denied unless explicitly approved once in the terminal.

## Verification

```bash
make check
```

## Releases

See [docs/releasing.md](docs/releasing.md) for macOS release and Homebrew setup instructions.

Caelum is licensed under the [MIT License](LICENSE).

## Roadmap

See [docs/roadmap.md](docs/roadmap.md) for milestones and [docs/architecture.md](docs/architecture.md) for current boundaries and planned integrations.
