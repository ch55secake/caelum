# Codex provider

Caelum integrates with the locally installed Codex CLI through its official app-server protocol over stdio JSONL. The provider starts one `codex app-server --listen stdio://` child process per project, sends `initialize` followed by the `initialized` notification, and uses Codex-owned threads for session history. It does not use the experimental WebSocket transport.

## Setup

1. Install the [Codex CLI](https://developers.openai.com/codex/cli) and make `codex` available on `PATH`. Caelum also checks common Homebrew and user-local install paths. If the executable is in a non-standard location, add it to `PATH` before launching Caelum.
2. Sign in from a terminal with `codex login`. Check the current auth state with `codex login status`.
3. Open a project in Caelum. Codex sessions are listed by the selected project's working directory; new threads use Codex's `workspaceWrite` sandbox and `unlessTrusted` approval policy.

Caelum relies on Codex-managed credentials and does not copy or store them. Missing installation and authentication are reported in the app with setup guidance.

## Protocol behavior

The implementation follows the current [Codex app-server documentation](https://developers.openai.com/codex/app-server) and [authentication documentation](https://developers.openai.com/codex/auth). JSON-RPC responses are correlated independently from notifications; output deltas and item lifecycle events are forwarded to the desktop. Command execution and file-change approval requests are shown inline in the requesting session. Cancelling a turn sends `turn/interrupt`; exited app-server processes are reaped and recreated on reconnect.

History uses `thread/turns/list` with `itemsView: "full"`, newest-first pages and opaque cursors. Native items cross the desktop boundary intact and share stable identities with live events. Model choices, supported reasoning efforts, and model-default effort come from `model/list`; `config/read` resolves the project’s default model. New sessions pass the model choice to `thread/start`. Pending model and effort selections go to the next `turn/start`, which makes them defaults for subsequent turns. The pickers restore `model` and `reasoningEffort` from `thread/start` and `thread/resume` responses. Selections survive failed sends and reconnects.

Reasoning summary sections stream by `summaryIndex`, with `contentIndex` tracking raw reasoning when available. The desktop prefers provider-supplied summaries, displays expandable thinking previews, and reconciles final items with in-flight history. The composer outline pulses throughout the turn, including long gaps without events. Running tools and commands show elapsed time, and reduced-motion preferences disable the pulse.

Sidebar details use the branch Caelum records at execution start/completion, falling back to native `thread.gitInfo.branch` from `thread/read` for unobserved sessions. This catches branches created during a turn and keeps historical sessions stable across checkout changes. Associations persist across restart and archive/undo. Added/deleted counts sum patch lines from completed `fileChange` items in the newest turn (`thread/turns/list`, `limit: 1`). Details load for session rows and PR discovery and refresh after turns; unavailable history leaves line counts absent. These counts cover provider-reported file edits, rather than the project’s entire working-tree diff.

Archiving uses `thread/archive`; Undo uses `thread/unarchive`. Both validate the thread’s canonical working directory, and active threads cannot be archived. Listings request `archived: false`. The desktop removes archived session caches and saved selection records, invalidates in-flight snapshots, and retains the latest archived draft/settings for Undo.

The app-server command and protocol can change with Codex CLI releases. Live CLI/auth behavior should be checked with an installed Codex CLI in addition to the fake-server protocol and lifecycle tests.
