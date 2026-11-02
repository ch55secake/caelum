# Development

## Prerequisites

- Rust stable with `rustfmt` and `clippy`
- Node.js 22 and pnpm 10
- [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/)
- A configured [Codex](codex.md) or [OpenCode](opencode.md) installation for live sessions

Alternatively, run `nix develop` for the Rust, Node.js, pnpm and Make toolchain. The shell includes Tauri's Linux libraries; macOS platform prerequisites must be installed on the host.

## Commands

```sh
make help                         # Available tasks
make dev                          # Install dependencies and run the desktop
make app                          # Build the macOS app bundle
make frontend                     # Build only the frontend
make cli                          # Build the standalone CLI
make cli-run PROJECT=.            # Open a project through the CLI
make check                        # Rust formatting, lint, tests and frontend build
```

See [contributing](contributing.md) for frontend unit tests, browser tests and focused checks. Browser tests mock the desktop/provider boundary; Rust integration tests exercise adapters against local fake servers.

See [architecture](architecture.md) for frontend feature folders, native command modules and shared-core boundaries, and [releasing](releasing.md) for distributable packages.
