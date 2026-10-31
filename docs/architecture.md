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

## Projects and worktrees

`caelum-core::worktrees` discovers checkouts with Git's NUL-delimited porcelain output and creates new branches with `git worktree add`. Storage is grouped by the original checkout name and a stable hash of the canonical common Git directory, so linked checkouts share an identity and separate clones stay independent. A project opened below the repository root retains its relative directory in other checkouts; unavailable directories and paths resolving outside a checkout cannot be selected.

The desktop groups checkout directories under a logical project while keeping provider connections, sessions, drafts, file tabs, and Git review keyed by the working directory. SQLite schema v4 records the root project and preferred checkout alongside the existing per-directory session metadata. Conversation history stays provider-owned.

Removal checks active provider sessions and pending requests, then uses `git worktree remove` without forcing or deleting the branch. Successful removal releases checkout-specific connections and app workspace records, including nested project directories. OpenCode's shared background service remains externally owned. Navigation and request revisions prevent delayed responses from switching the active checkout or restoring removed state.

## Context menus

`ContextMenuProvider` composes contextual actions and renders an app-owned React menu using the current theme. Menus are portaled into the active dialog when needed, positioned within the viewport, and support keyboard navigation, dismissal, and focus restoration. Navigation revisions invalidate stale actions. Text editing snapshots the original selection, uses the clipboard plugin for copy/paste, and retains browser undo history; delayed clipboard reads cannot edit a different draft or changed selection.

## GitHub pull requests

`caelum-core::github` uses the installed `gh` executable and its existing authentication through bounded, noninteractive processes. Visible branch indicators share lightweight discovery by checkout/branch. Hovering or focusing an indicator fetches richer PR metadata using its canonical HTTPS URL, preserving repository identity for forks and linked worktrees. Both GitHub Actions checks and legacy commit statuses cross the typed protocol boundary.

`PullRequestPreviewProvider` owns the sidebar's single portaled hover card and routes overview actions to `App`. Icons are separate buttons beside the project/session selection controls. Hover delays, viewport positioning, keyboard entry, focus restoration, and navigation-scoped dismissal keep previews usable without selecting a different checkout or session. `App` renders the selected PR as a dedicated main-workspace screen, keeping the conversation and inspector mounted but hidden. The sidebar remains available for direct navigation, the selected PR row is marked current, and opening a screen focuses its heading. Session/project navigation clears the selected PR; collapsing the sidebar leaves it open. Detail requests are deduplicated and cached by URL for a minute, with inactive cache entries bounded; unsubscribing a view prevents late results from replacing a different PR. Refresh failures preserve the last successful snapshot and expose retry locally.

`SessionPullRequests` lists unique PR URLs attached to the current checkout's unarchived sessions beneath the session list. It loads session branch metadata in small batches, including off-screen sessions, without connecting their conversations. `useBranchPullRequests` shares discovery subscriptions and a four-request queue with the inline indicators. Session filtering does not affect the PR list; project changes and archive/restore update its membership. Each list row opens the PR screen directly, preserving the active session and its draft.

`PullRequestChecks` uses shared check normalization for status icons, pending animations, summary counts, and check-detail links. The active screen refreshes pending checks or unknown mergeability on a 15-second cadence, with the normal one-minute cache for settled details. Reduced-motion preferences disable spinner animation.

`PullRequestMerge` presents GitHub's mergeability state and a merge/squash/rebase selector. The typed `github_merge_pull_request` command uses authenticated `gh api` against the canonical PR host/repository and pins the merge to the displayed head commit. GitHub enforces permissions, enabled merge methods, and repository rules; only a confirmed API merge counts as success. Merge progress and errors live in the URL-keyed detail cache, so navigation preserves in-flight actions and duplicate submissions are suppressed. Successful merges update the detail snapshot and remove matching discovery entries, including delayed branch lookups; failures retain the PR and reload its latest metadata for retry.

## Security boundaries

- Only Rust performs project path validation and future privileged operations.
- Frontend requests are treated as untrusted input.
- Secrets must not be stored in SQLite; use OS credential storage when provider authentication is added.
- Agent tool permissions and destructive Git operations require explicit, appropriately scoped approval.
- Keep Tauri capabilities narrow as commands and plugins are introduced.
