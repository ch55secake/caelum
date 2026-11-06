# Rune replacement migration

## Decision and scope

Build toward roughly **90% of Caelum's daily workflows**, then evaluate the native
client in real use before deciding on cutover. Tauri remains the shipping client
during the transition. The goal is one supported desktop client.

Keep the main workspace flush, with pane dividers, and retain the typography,
sidebar, composer and motion improvements from the workspace-polish PR.

## Ownership

- `caelum-core`: provider adapters, process lifecycle, scoped requests, session
  coordination, history reconciliation, persistence, files, Git and GitHub.
- `caelum-protocol`: framework-independent boundary types.
- Rune: rendering, animation, text editing, focus and view-local interaction state.
- Tauri: shipping adapter; adopt shared Rust behaviour workflow by workflow as it
  replaces TypeScript behaviour. Do not maintain two copies of new business logic.

Evolve `apps/rune-prototype` into the candidate client, keeping deterministic
fixtures as an explicit development mode. Its independent workspace isolates
Rune's Rust 1.95 requirement until release/toolchain migration is approved.

## Stages and exit criteria

### A — Real-session foundation

Shared provider lifecycle and background command/event delivery. Open a real
project, list both providers' sessions, resume and page history, send/stream/cancel
and resolve approvals. No provider I/O on Rune's UI thread; late results retain
their originating project, provider and session. Fixture mode remains runnable.

### B — Reliable conversations and composer

Migrate conversation reducers, tools/thinking, history/live reconciliation,
queues, questions, reconnect recovery, attachments, completions and model/agent
controls. Switch projects during execution without losing drafts or duplicating
messages. Migrate each shared workflow into the Tauri adapter too.

### C — Workspace, files and local review

Remember projects/checkouts/sessions; worktree lifecycle; session search and
archive/undo; file tree/search/tabs, syntax previews, selection attachments and
Git diffs. Preserve per-session reading positions through navigation.

### D — GitHub and evaluation build

PR discovery/stacks, overview/checks, changed files, inline feedback, addressing
selected comments, branch update and merge. Saved preferences, model favourites
and a versioned import of Tauri's localStorage settings. Package a macOS candidate.

### E — Real-use evaluation and cutover decision

Exercise both providers against real projects, **2,000 history entries**,
concurrent foreground/background streams and large PR reviews. Compare native
frame pacing, input latency, CPU and memory with Tauri, using the same workloads.
Record functional gaps and observed regressions before deciding to switch.

If approved, complete remaining requirements, migrate release packaging and
retire Tauri. Advanced statistics and secondary visual refinements may fall in
the final stretch; draft integrity, approvals and recovery may not.

## Workflow checklist

Check a workflow only after its behaviour is implemented and exercised. A
rendered screen, mock response or existing core API alone does not count.

| Workflow | Candidate status |
| --- | --- |
| Native editing, panes, themes and sample diff | Fixture implementation |
| Open projects; list/resume both providers | Stage A implementation |
| History, streaming, cancellation, background activity | Stage A implementation; large-history evaluation pending |
| Scoped approvals and question forms | Approvals implemented; question answers pending (requests visible/dismissible) |
| Draft integrity, queueing, reconnect/recovery | Stage B |
| Models, agents, commands, attachments, completions | Model/effort/agent controls implemented; commands/attachments/completions pending |
| Projects/worktrees, remembered navigation, session search/archive | Stage C |
| Files, content search, previews, selection context, local diffs | Stage C |
| PR discovery, review/feedback, checks and mutations | Stage D |
| Saved settings, import and model favourites | Stage D |
| Packaged build and native workload measurements | Stages D–E |

Deliver runnable milestones in focused PRs. This checklist is a migration
baseline, not a claim that the current prototype has reached 90% parity.

## First milestone boundary

The shared runtime now owns provider construction/shutdown for Tauri and Rune.
Rune uses the new scoped request coordinator and conversation reducer; Tauri's
conversation reducer remains in TypeScript until the richer Stage B event contract
has parity. No renderer types enter `caelum-core`.

Background requests have stable owners, independent cancellation, bounded inbox
draining and event-driven wakeups. Submission failures retain the unsent prompt
separately from subsequent editing. The first reducer covers normal text,
reasoning and tools; compaction, background shells, usage and reconnect edge cases
still need the fuller Stage B port. No native performance claim is made from this
milestone; long-history virtualization and the evaluation workloads remain pending.
