# Rune replacement migration

## Decision and delivery

Target roughly **90% of Caelum's daily workflows**, evaluate the native client in
real use, then decide on cutover. Tauri remains the shipping client. The goal is
one supported desktop client, with flush workspace surfaces, pane dividers and
the polished typography, sidebar, composer and motion.

The real-session foundation is PR #92, stacked on workspace polish/fixtures in
PR #91. The remaining candidate implementation is consolidated into one PR based
on `feat/rune-real-sessions`, rather than separate milestone PRs. It includes
conversation/composer reliability, workspace/files/Git/GitHub, persistence,
preferences, demanding workloads and macOS evaluation packaging.

This delivery is an evaluation candidate, not a completed cutover or an assertion
that 90% parity has been measured. [Run/package instructions](../../apps/rune-prototype/README.md)
and [evaluation evidence](rune-evaluation.md) accompany it.

## Ownership

- `caelum-core::application`: scoped requests, conversation reconciliation,
  persistence, drafts/queues/settings, question validation and workspace services.
- Existing Rust adapters: provider I/O, process lifecycle, files, Git/worktrees,
  GitHub and persisted session-branch attribution.
- Rune: rendering, native editing/focus/dialogs, panes and view-local state.
- `caelum-protocol`: renderer-independent boundary types.

Tauri shares provider lifecycle and Rust question validation. Its conversation and
application reducers still live in TypeScript; this candidate does not claim that
all Tauri behaviours have moved. New candidate business logic belongs in Rust.
The cutover decision must retire the duplicate shipping path rather than turn
this transition into two indefinitely supported clients.

The candidate remains in `apps/rune-prototype`, an independent workspace pinned
to Rune `ddf14140253bd1b963993895e3fa025d1ec58537` and Rust 1.95. The root workspace
still declares Rust 1.78. Toolchain/release migration accompanies an approved cutover.

## Implemented candidate workflows

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
| Settings | Saved mode/default provider/model/agent/effort/favourites/worktree directory/notifications; explicit Tauri v1 JSON import |
| Evaluation build | Isolated fixtures and heavy workload; native macOS app/ZIP packaging and candidate CI |

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
app crash is not claimed. Use one process per state path.

File and review results remain scoped after navigation. Review reads are guarded
by request generation and head; refresh invalidates even same-head responses.
In-flight mutations stay locked across refresh. Linked-checkout removal respects
provider activity and Git's dirty/locked checks.

## Remaining acceptance and cutover work

- Exercise complete real Codex and OpenCode runs: approvals/questions, queued
  prompts, network/process failure, restart, large histories and concurrent work.
- Compare native frame pacing, input latency, CPU and process memory with Tauri
  on the same hardware, project, history and review workload. Current synthetic
  measurements are a baseline, not a matched comparison.
- Complete manual compact-layout, keyboard, focus, selection, screen-reader and
  reduced-motion acceptance. Rendering accessibility support alone is insufficient.
- Close material workflow gaps: images/pasted images/viewer; PR mentions;
  timeline/goal/usage/statistics commands; richer side-question interaction;
  caret-aware keyboard completions and shortcut/context-menu coverage; persisted
  file-panel state, live file refresh and richer tool previews; theme-family/accent
  and notification-option parity. OpenCode native inbox coordination is separate
  from the current durable local queue.
- Decide whether observed gaps fit the final stretch. Draft integrity, approvals
  and recovery remain mandatory. If approved, finish required gaps, migrate
  signing/updating/release packaging and retire the Tauri client and its reducers.
