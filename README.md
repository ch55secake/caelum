# Caelum

Caelum is an AI coding workspace for macOS, with a native Rust core shared by its Tauri desktop app and standalone CLI. The desktop supports project-scoped Codex and OpenCode sessions, workspace restoration, file browsing, and Git review.

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

Session state and browser interaction checks:

```bash
pnpm --filter @caelum/desktop test
pnpm --filter @caelum/desktop exec playwright install chromium
pnpm --filter @caelum/desktop test:ui
```

Browser checks use a mocked desktop/provider boundary. Rust integration tests exercise the actual provider adapters against local fake servers.

## Sessions

Open a project and send a message to create a session. Choose the provider and any available model/agent before sending. Each session keeps its own draft, reading position, and expanded tools while the app is open. Provider-owned history is restored in pages; use **Load earlier messages** to read further back.

Responses and thinking update live. Expand a thinking row to read the provider's summary or trace; activity stays visible above the composer while you browse history. Sending a message returns the transcript to the latest response. OpenCode connects to its existing local background service when available.

The **Changes** pane shows project diffs beside the conversation and refreshes while the agent works. Toggle it from the sidebar or header. Model selection is searchable by name or ID, with arrow-key navigation and Enter to select. Shell notifications display their command, execution ID, status, and output as expandable tool activity.

- **Enter** or **⌘Enter** sends; **Shift+Enter** inserts a newline.
- **⌘N** opens a new draft; **⌘K** searches sessions; **⌘\\** toggles the sidebar. Ctrl works in place of ⌘.
- Draft the next message while a response runs. Stop interrupts the active turn.
- Approvals appear in the requesting conversation; the sidebar and header identify pending background requests.
- A disconnected session offers **Reconnect**. A failed submission restores its text in the composer.

## Releases

See [docs/releasing.md](docs/releasing.md) for macOS release and Homebrew setup instructions.

Caelum is licensed under the [MIT License](LICENSE).

## Roadmap

See [docs/roadmap.md](docs/roadmap.md) for milestones and [docs/architecture.md](docs/architecture.md) for current boundaries and planned integrations.
