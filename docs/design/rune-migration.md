# Rune replacement migration

## Decision and delivery

Rune is the shipping client in `apps/desktop`, with shared Rust application
behavior, native editing and the polished Caelum workspace surfaces. The cutover
was approved after the functional candidate; design parity and production
promotion are the final two sequential PRs.

Delivery order: workspace polish/fixtures (#91), shared runtime/real sessions
(#92), functional candidate (#93), design parity (#94), then native cutover.
The cutover removes the Tauri/web application and its Node/TypeScript build/test
stack, promotes native CI and replaces packaging with Rust-built app/DMG assets.

The original target was roughly 90% of daily workflows. Cutover approval does not
turn that target into a measured result; [evaluation evidence](rune-evaluation.md)
and remaining gaps below describe the acceptance boundary. [Run/package instructions](../../apps/desktop/README.md)
now use the production app.

## Ownership

- `caelum-core::application`: scoped requests, conversation reconciliation,
  persistence, drafts/queues/settings, question validation and workspace services.
- Existing Rust adapters: provider I/O, process lifecycle, files, Git/worktrees,
  GitHub and persisted session-branch attribution.
- Rune: rendering, native editing/focus/dialogs, panes and view-local state.
- `caelum-protocol`: renderer-independent boundary types.

The desktop, CLI, core and protocol use one Cargo workspace and lockfile, pinned
to Rust 1.95.0 and Rune `ddf14140253bd1b963993895e3fa025d1ec58537`.

## Implemented workflows

| Workflow | Implementation and evidence boundary |
| --- | --- |
| Native editing, panes and appearance | Flush surfaces, keyboard editing, navigation, theme modes and pane tests |
| Projects and real sessions | Both Rust providers, list/resume/history/stream/cancel; real OpenCode discovery/history smoke; Codex executable unavailable locally |
| Conversations | Scoped concurrent activity, paged/virtualized transcript, reasoning/tools/statuses, compaction, shells and context counts; synthetic six-session stress workload |
| Approvals and questions | Scoped permission replies and native forms with defaults/custom/conditional validation; reducer, broker and native interaction tests |
| Drafts and queues | Atomic private saves, save-before-dispatch, independent drafts/failed submissions/pending admissions, paused restore, per-message options, edit/remove/pause/resume |
| Recovery | One automatic reconnect attempt, explicit retry, history reconciliation, cancellation admission guards; race tests; extended real-provider fault testing pending |
| Composer | Text snapshots, drag/drop, file mentions, local/discovered commands, `/ask`, next-message model/effort/agent choices and favourites |
| Workspace | Persisted projects/checkout groups, remembered sessions/reading, session search, archive/undo, worktree discovery/create/remove |
| Files and Git | Paged directories, bounded filename/content search, read-only tabs, syntax previews/selection context, local unified/split diffs |
| GitHub | Branch/stack discovery, overview/checks, paginated files/feedback/replies, selected-comment prompts, head-pinned update/merge and stale-generation guards |
| Settings | Saved mode/family/accent/default provider/model/agent/effort/favourites/worktree directory/notifications; explicit legacy v1 JSON import |
| Production lifecycle | Standard menus, close/reopen, cold/running project URLs, lifetime state lock and shutdown flush |
| Distribution | Native app, architecture-specific DMGs/CLI archives, ad-hoc signatures, checksums, release/version and Homebrew automation |

Implementation and automated checks are recorded separately from real-use
acceptance. Existing API coverage or a rendered screen is not proof of daily-use
parity. In particular, remote merge/update actions use the tested Rust APIs but
are not exercised by the read-only real-project smoke checks.

## Reliability contract

Commands retain their originating project/provider/session. The runtime uses
eight normal workers and two control workers, bounded command queues, a 1,024-event
inbox and 256-event UI drains. Provider work does not run on Rune's UI thread.
Persistence is serialized; a submission waits for its own saved snapshot without
waiting for later typing. Disk errors cannot block cancellation or permission replies.

Saved state is versioned JSON; invalid/future files are preserved. Normal shutdown
stops workers before the final editor flush. App-owned queues retain captured
settings and snapshots. Restored/unacknowledged submissions require explicit
queue resume after checking history; exactly-once provider admission across an
app crash is not claimed. A lifetime file lock enforces one desktop writer per state path.

File and review results remain scoped after navigation. Review reads are guarded
by request generation and head; refresh invalidates even same-head responses.
In-flight mutations stay locked across refresh. Linked-checkout removal respects
provider activity and Git's dirty/locked checks.

## Production state migration

The new default is `~/.caelum/workspace-v1.json`. `CAELUM_STATE` takes priority over
the compatibility `CAELUM_RUNE_STATE`; both explicit overrides skip migration.
Branch history lives in `branches.sqlite3` beside the selected snapshot.

With no production snapshot, first launch prefers `~/.caelum/rune/workspace-v1.json`
and its branch DB, retaining drafts, attachments, failed/pending sends and queues.
Otherwise it imports `~/Library/Application Support/dev.caelum.desktop/workspace.sqlite3`,
including projects, selected providers/sessions, checkout groups and branch history.
SQLite backup captures committed WAL data. Originals remain untouched; migration
does not run again once the destination exists. Corrupt/future data produces a
visible persistence error and is preserved.

Legacy web preferences require the exported `caelum.settings.v1` JSON. From an
older app's Web Inspector, export that localStorage value to a JSON file, then
choose **Import legacy v1 settings JSON** in native Settings. Conversation content
remains provider-owned. Live unsaved web-editor text is not in the legacy SQLite
database and cannot be reconstructed by this migration.

The production identity remains `Caelum.app` / `dev.caelum.desktop`. The CLI sends
validated project URLs to cold or running instances. Closing the window keeps
work alive; quitting stops owned workers/providers, saves editors and releases
the state lock.

## Remaining workflow and acceptance work

- Exercise complete real Codex and OpenCode runs: approvals/questions, queued
  prompts, network/process failure, restart, large histories and concurrent work.
- Compare native frame pacing, input latency, CPU and process memory with Tauri
  on the same hardware, project, history and review workload. Current synthetic
  measurements are a baseline, not a matched comparison.
- Complete manual compact-layout, keyboard, focus, selection, screen-reader and
  reduced-motion acceptance. Rendering accessibility support alone is insufficient.
- Workflow gaps: images/pasted images/viewer; PR mentions;
  timeline/goal/usage/statistics commands; richer side-question interaction;
  caret-aware keyboard completions and shortcut/context-menu coverage; persisted
  file-panel state, live file refresh and richer tool previews; richer
  notification options. OpenCode native inbox coordination is separate
  from the current durable local queue.
- Continue measuring native interaction and real-use parity after cutover. Draft
  integrity, approvals and recovery remain mandatory.
