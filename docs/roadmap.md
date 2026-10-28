# Roadmap

## 1. Foundation — current

- [x] Cargo workspace with a desktop app, CLI, core, and protocol crate
- [x] Tauri 2 + React/TypeScript desktop shell
- [x] Native project picker with Rust-side path validation
- [x] CLI scaffold sharing the core library
- [x] CI checks and initial architecture documentation
- [ ] Generate and check in the pnpm lockfile
- [ ] Verify desktop and CLI builds in CI

## 2. Codex vertical slice

- Verify the current official Codex app-server protocol and executable requirements
- Add a provider adapter in `caelum-core`
- Start/resume a thread, submit prompts, stream events, cancel work, and handle approvals
- Present responses and tool activity in the desktop UI
- Add focused protocol and lifecycle tests

## 3. OpenCode provider

- [x] Verify the local v2 HTTP API, SSE events, and optional Basic authentication
- [x] Manage the per-project server process and HTTP/SSE lifecycle
- [x] Stream sessions, prompts, tool events, permissions, cancellation, and restoration

## 4. Workspace MVP

- Versioned SQLite migrations and session metadata
- Git status and reviewable diffs
- File browsing and editor integration
- PTY-backed interactive terminal
- Session restoration and multi-project UX

## 5. Production readiness

- Security review, crash recovery, and performance profiling
- macOS Apple Silicon and Intel packaging, signing, and notarization
- Release artifacts, checksums, and Homebrew formula/cask automation
- Add Windows and Linux builds after the macOS experience is stable

Items are not considered complete until their relevant automated checks or documented manual verification pass.
