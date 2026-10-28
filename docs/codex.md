# Codex provider

Caelum integrates with the locally installed Codex CLI through its official app-server protocol over stdio JSONL. The provider starts one `codex app-server --listen stdio://` child process per project, sends `initialize` followed by the `initialized` notification, and uses Codex-owned threads for session history. It does not use the experimental WebSocket transport.

## Setup

1. Install the [Codex CLI](https://developers.openai.com/codex/cli) and make `codex` available on `PATH`. Caelum also checks common Homebrew and user-local install paths. If the executable is in a non-standard location, add it to `PATH` before launching Caelum.
2. Sign in from a terminal with `codex login`. Check the current auth state with `codex login status`.
3. Open a project in Caelum. Codex sessions are listed by the selected project's working directory; new threads use Codex's `workspaceWrite` sandbox and `unlessTrusted` approval policy.

Caelum relies on Codex-managed credentials and does not copy or store them. Missing installation and authentication are reported in the app with setup guidance.

## Protocol behavior

The implementation follows the current [Codex app-server documentation](https://developers.openai.com/codex/app-server) and [authentication documentation](https://developers.openai.com/codex/auth). JSON-RPC responses are correlated independently from notifications; output deltas and item lifecycle events are forwarded to the desktop. Command execution and file-change approval requests are shown in Caelum's one-shot approval dialog. Cancelling a turn sends `turn/interrupt`; exited app-server processes are reaped and recreated on the next project operation.

The app-server command and protocol can change with Codex CLI releases. Live CLI/auth behavior should be checked with an installed Codex CLI in addition to the fake-server protocol and lifecycle tests.
