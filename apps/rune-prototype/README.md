# Caelum Rune evaluation candidate

Native daily-workflow candidate, backed by Caelum's Rust application coordinator.
Tauri remains the shipping client until real-use evaluation and a cutover decision.

## Run

From the repository root (Rust 1.95; Rune is pinned in `Cargo.toml`):

```sh
nix develop -c cargo run --locked --manifest-path apps/rune-prototype/Cargo.toml
```

Use **Open project** (⌘O), or append `-- --project /path/to/project`. Install and
authenticate Codex/OpenCode for their sessions, and `gh` for GitHub workflows.
The composer chooses the provider for a new session and model/effort/agent
for the next message. Favourites appear first; **Session actions** saves defaults.

Development modes are isolated from providers and saved workspace data:

- `-- --fixtures`: original polished workspace and simulated replies.
- `-- --design-fixture`: reference conversation rendered by the live UI, offline.
- `-- --evaluation-workload`: six streaming sessions with 2,000 history entries
  each, plus a 300-file synthetic PR accessible from the sidebar.

## Workflows

- Native streaming transcript, paged history, expandable tools/reasoning,
  compaction, background shells, context counts, cancellation and approvals.
- Question forms with options, custom answers, defaults, conditional fields and
  shared Rust validation. Failed replies stay retryable in their originating session.
- Independent drafts and queues; session search (⌘K), archive/undo, grouped
  checkouts and remembered navigation/reading positions.
- **Files** / ⌘P: filename or content search, directory pagination, read-only tabs,
  syntax previews in 200-line pages, refresh and file/selection attachments.
  **Attach**, project-file drag/drop and trailing `@query` completion capture
  immutable text snapshots (16 per message, 64 KiB each).
- **Changes**: local Git status, unified/split diffs. **Worktrees**: discover,
  create and remove checkouts using the saved parent directory.
- PR discovery follows checkout/session branches and stacked bases. Reviews show
  overview/checks, paged files, discussions/reviews/threads/replies and selected
  feedback in the composer. Update and merge requests pin the expected head.
- Local `/new`, `/files`, `/changes`, `/settings`; discovered OpenCode commands;
  `/ask question` generates a transient answer in an existing OpenCode session.
- **Settings** / ⌘,: System/Light/Dark, Caelum/Catppuccin/Nord/Tokyo Night,
  six accents or the theme's own accent, notifications, worktree directory and
  explicit Tauri v1 JSON import. Export `caelum.settings.v1` from localStorage in
  Tauri's Web Inspector, save as JSON, then choose **Import Tauri v1 settings JSON**.
  Imported fields are appearance mode/family/accent, provider defaults, favourites, worktree
  directory and notification enablement.

## Persistence and recovery

State lives in `~/.caelum/rune/workspace-v1.json`; `CAELUM_RUNE_STATE` overrides it.
Branch attribution uses `branches.sqlite3` beside that file. Use a separate path
for evaluation runs against real providers. Provider-owned history remains with
the provider. Tauri's saved workspace is not automatically imported.

The coordinator atomically saves private JSON before dispatching submissions.
Failed saves block new submissions and expose **Retry save** in Settings;
cancellation and permission replies remain available. Invalid/future-version
state is preserved instead of overwritten. Normal shutdown flushes editor state.
Use one running candidate per state path.

Queues belong to Caelum, not OpenCode's native inbox. Each entry captures its own
model/agent settings and attachment text. Stop/failure/disconnection pauses the
queue; it advances after idle history reconciliation. Restored queues always
pause. An unacknowledged submission is recovered at the front of the paused queue;
inspect provider history before resuming, since the provider may have accepted it
before a crash. Existing failed prompts and newer drafts are retained separately.
Disconnect recovery attempts one resume; **Refresh / reconnect session** retries.

## Build and check

```sh
nix develop -c cargo test --locked --manifest-path apps/rune-prototype/Cargo.toml
nix develop -c cargo clippy --locked --manifest-path apps/rune-prototype/Cargo.toml --all-targets -- -D warnings
nix develop -c bash scripts/release/package_rune_candidate.sh /path/to/new/output
```

Packaging creates an ad-hoc-signed `Caelum Rune Evaluation.app` and architecture-
specific ZIP using only macOS system libraries, including when built with Nix.
The macOS candidate workflow publishes the same artifact. It is an
evaluation build, with no production updater or notarized release integration.

Real-provider smoke checks require a project containing history:

```sh
CAELUM_RUNE_STATE=/path/to/isolated/workspace.json \
CAELUM_RUNE_SMOKE_PROJECT=/path/to/caelum \
CAELUM_RUNE_SMOKE_PROVIDER=opencode \
nix develop -c cargo test --locked --manifest-path apps/rune-prototype/Cargo.toml real_provider_discovery_and_history_smoke -- --ignored --nocapture

CAELUM_RUNE_SMOKE_PROJECT=/path/to/caelum \
CAELUM_RUNE_SMOKE_PR=https://github.com/ch55secake/caelum/pull/92 \
nix develop -c cargo test --locked --manifest-path apps/rune-prototype/Cargo.toml real_workspace_services_smoke -- --ignored --nocapture
```

## Evaluation boundary

See the [migration status](../../docs/design/rune-migration.md) and
[measurement record](../../docs/design/rune-evaluation.md). Image/pasted-image
workflows, full command/completion/shortcut parity, richer theme/notification
options, accessibility acceptance and real-provider recovery/queue stress testing
remain gaps. The candidate delivers the consolidated evaluation surface; 90%
daily-use parity and replacement readiness still require actual evaluation.
