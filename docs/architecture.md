# Architecture

## Ownership

- `crates/caelum-core`: renderer-independent application state, provider adapters,
  persistence, scoped filesystem access, Git/worktrees and GitHub operations.
- `crates/caelum-protocol`: serializable values shared by interfaces and adapters.
- `apps/desktop`: Rune rendering, native editors/focus/dialogs, pane state and
  macOS lifecycle integration. This is the shipping desktop application.
- `apps/cli`: clap-based terminal workflows and desktop project launching, using
  the same Rust providers and project validation.

The root Cargo workspace uses Rust 1.95.0 and one lockfile. Rune/rune-kit are pinned
to `ddf14140253bd1b963993895e3fa025d1ec58537`.

## Application runtime

`caelum-core::application::Workspace` owns sessions, options, drafts, queues,
approvals/questions, project selection and service requests. Commands/results
carry their originating project/provider/session and stable local session slot.
Request generations reject stale history, project, file and review responses.
`Conversation` reconciles provider history with live text, reasoning and tool
events without duplicating or overwriting newer activity.

`Runtime` uses eight normal workers, two control workers, bounded command queues,
a 1,024-event inbox and 256-event UI drains. Arrival wakes Rune; provider I/O
does not run on the UI thread. Cancellation and permission replies bypass the
normal persistence gate. The shared `ProviderRuntime` owns providers, brokers,
process registration and session-branch attribution.

Codex uses one `codex app-server --listen stdio://` JSONL connection per project.
OpenCode uses its verified local registered service or an owned loopback server.
Only owned processes are terminated on shutdown. See [Codex](codex.md),
[OpenCode](opencode.md) and [security boundaries](security.md).

## Desktop modules

- `main.rs`: entry point, shell, keyboard routing and offline render modes.
- `lifecycle.rs`: standard macOS menus and `caelum://open?path=…` delivery.
- `live.rs`: editor/application synchronization and runtime wake/drain, with
  sidebar, windowed transcript, composer and session menus in `live/`.
- `state.rs`: pane/view state and the original simulated interaction fixture.
- `panels.rs`: panel state, synchronization and routing. `panels/` owns the
  file/change inspector, review/feedback, appearance, launcher, queue and overlay
  views. Local and review diffs have independent state.
- `questions.rs`: form editors backed by shared Rust validation.
- `theme.rs` / `ui.rs`: semantic palettes, typography and shared surfaces.
- `evaluation.rs`: offline design and demanding-workload fixtures.
- `tests.rs`: shared native-runtime helpers, with capability suites in `tests/`.

Each session has an independent native editor and reading position. Settings,
worktree and session dialogs scope focus. Long transcripts, file previews and
review patches are windowed/bounded. Provider/model/effort/agent choices are
captured when a message is queued. Rendering state never owns provider processes.

## Persistence and migration

`~/.caelum/workspace-v1.json` stores versioned projects, sessions, drafts,
attachments, options, queues and preferences. `CAELUM_STATE` overrides the path;
`CAELUM_RUNE_STATE` remains a compatibility override. A lifetime file lock
prevents concurrent desktop writers. Writes use a private temporary file,
fsync and atomic replacement; submissions wait for their own durable snapshot.
Invalid/future state blocks saving instead of becoming an empty workspace.

On the first production-default launch, migration prefers the evaluation
snapshot at `~/.caelum/rune/workspace-v1.json`. Otherwise it imports workspace
metadata from `~/Library/Application Support/dev.caelum.desktop/workspace.sqlite3`.
SQLite backup includes committed WAL data; migration runs only on the copied
database. Sources remain available. Explicit state overrides skip migration.
Legacy appearance/provider preferences can be imported from exported v1 JSON.

`branches.sqlite3` beside the snapshot retains session-specific branch history.
Conversation history remains provider-owned. Closing the window keeps the
runtime and state alive; Dock activation or a project URL reopens it. Quitting
stops workers/owned providers before flushing the latest editors and releasing
the lock. Restored queues and unacknowledged submissions pause for explicit resume.

## Projects and services

Core validates project roots and scoped paths. Checkouts group under a project
while retaining independent provider sessions and drafts. Worktree creation uses
Git's native APIs and stable per-repository storage. Removal checks active work,
pending requests and Git dirty/locked state; it preserves branches and commits.

`SessionBranches` records branches during unambiguous execution, retaining earlier
branches across restart/archive. Concurrent sessions in the same checkout do not
claim each other's changes. Linked worktrees remain independent. Provider-native
metadata can seed Codex history; idle sessions keep their historical associations.

GitHub operations use authenticated `gh` through bounded subprocesses. Discovery
follows recorded branches and stacked bases, deduplicating canonical PR URLs.
Files and feedback paginate against the expected head/base. Review generations
reject late pages even after a same-head refresh. Mutations remain locked across
navigation/refresh and pin the displayed head. GitHub enforces merge permissions
and repository rules. Selected feedback becomes editable composer context.
