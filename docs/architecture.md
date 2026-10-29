# Architecture

## Current structure

- `caelum-core` contains application logic that must remain independent of Tauri and the CLI. It validates project roots, resolves existing paths within the selected project, coordinates one-shot approvals, and owns managed child-process lifecycles.
- `caelum-protocol` contains serializable types that can cross the Rust/UI boundary without importing desktop framework types, including provider-preserving approval requests.
- `apps/desktop/src-tauri` is the privileged desktop boundary. It provides typed Tauri commands for app metadata, opening a validated project, and resolving explicit approval prompts; folder selection uses Tauri's native dialog plugin.
- `apps/desktop/src` is a React UI. It does not access the filesystem directly.
- `apps/cli` is a separate clap binary and links the same core library.

## Provider integration

Provider adapters, sessions, SQLite storage, Git, terminal/PTY, and filesystem operations belong in `caelum-core` or focused core submodules. Tauri commands should stay thin and validate inputs before calling core APIs. CLI commands should use those same APIs rather than duplicate behavior.

The Codex adapter in `caelum-core` starts one local `codex app-server --listen stdio://` process per project and speaks the official JSONL request/notification protocol. It performs the `initialize`/`initialized` handshake, checks CLI authentication with `codex login status`, scopes thread listing and resumption to the validated project directory, and forwards streamed app-server events to the desktop. Command and file-change requests use the shared one-shot approval broker. Processes are tracked for shutdown and restarted on the next request after an unexpected exit.

The OpenCode adapter in `caelum-core` attaches to the registered local background service after verifying `/api/info`, or starts an owned loopback `opencode serve` process per project. It scopes sessions to the validated project directory, waits for the `/api/event` subscription before fetching snapshots, and forwards permission requests through the shared one-shot approval broker. Only owned processes are terminated on shutdown. The desktop reconciles live text, thinking, and tool events with provider history and periodically refreshes selected sessions to recover missed updates.

See [security.md](security.md) for current permission boundaries and process lifecycle guarantees.
See [codex.md](codex.md) for Codex CLI setup and app-server protocol behavior.
See [opencode.md](opencode.md) for OpenCode server setup and HTTP/SSE behavior.

## Security boundaries

- Only Rust performs project path validation and future privileged operations.
- Frontend requests are treated as untrusted input.
- Secrets must not be stored in SQLite; use OS credential storage when provider authentication is added.
- Agent tool permissions and destructive Git operations require explicit, appropriately scoped approval.
- Keep Tauri capabilities narrow as commands and plugins are introduced.
