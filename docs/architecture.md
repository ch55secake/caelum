# Architecture

## Current structure

- `caelum-core` contains application logic that must remain independent of Tauri and the CLI. Its first feature validates a local path, canonicalizes it, and exposes project metadata.
- `caelum-protocol` contains serializable types that can cross the Rust/UI boundary without importing desktop framework types.
- `apps/desktop/src-tauri` is the privileged desktop boundary. It provides typed Tauri commands for app metadata and opening a validated project; folder selection uses Tauri's native dialog plugin.
- `apps/desktop/src` is a React UI. It does not access the filesystem directly.
- `apps/cli` is a separate clap binary and links the same core library.

## Planned core modules

Provider adapters, sessions, SQLite storage, Git, terminal/PTY, filesystem access, and permission handling belong in `caelum-core` or focused core submodules. Tauri commands should stay thin and validate inputs before calling core APIs. CLI commands should use those same APIs rather than duplicate behavior.

Codex and OpenCode adapters will preserve provider-specific capabilities behind shared session/event abstractions. Their protocol behavior must be verified against official documentation during implementation; no provider protocol is assumed by this foundation.

## Security boundaries

- Only Rust performs project path validation and future privileged operations.
- Frontend requests are treated as untrusted input.
- Secrets must not be stored in SQLite; use OS credential storage when provider authentication is added.
- Agent tool permissions and destructive Git operations require explicit, appropriately scoped approval.
- Keep Tauri capabilities narrow as commands and plugins are introduced.
