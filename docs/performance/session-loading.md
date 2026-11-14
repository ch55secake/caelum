# Session history loading

The runtime reserves two workers for resume, history pagination and OpenCode inbox
recovery. Eight normal workers continue catalogues, session diffs, Git and GitHub
discovery; two control workers handle cancellation, approvals and persistence.
Background I/O cannot consume the conversation workers or the control workers.

OpenCode connection initialization is single-flight **per checkout**. The shared
map is locked only to find a connection slot. Service discovery, owned-process
startup, SSE readiness and pending-question recovery run outside that map lock.
Other checkouts can connect or use an existing connection during a slow attempt.
Release/shutdown closes the slot and its attempt identity; late results cannot
replace a newer connection or restore questions from a retired attempt. Failed
attempts and disconnected servers can retry.

## Reproduction

Build first, then run without simultaneous builds/tests:

```sh
nix develop -c cargo build --locked --release -p caelum-core --example session_load
target/release/examples/session_load path/to/workspace.json oldest-open 5 \
  > apps/desktop/profile-results/session-loading.jsonl
```

Targets are `selected`, `oldest-open` and `oldest-archived`; they must still belong
to accessible OpenCode project directories. The supplied snapshot is read only.
The example completes workspace load/save in memory and admits only read commands.
It prints counts, sizes and timings, without paths, IDs or conversation contents.
Provider connections can start owned processes when no registered service exists.

Each run uses separate fresh provider adapters for:

- **Direct:** resume (including connection/SSE/question initialization), first
  50-message history fetch/JSON decoding, then conversation reconciliation.
- **Restoration:** the real workspace restoration and command runtime, with
  competing catalogue, detail, Git/worktree and PR discovery reads. Readiness is
  measured when the selected session's history has been applied. Per-command
  elapsed time includes worker queueing and provider work; `apply_ms` measures
  completion reconciliation separately.

The registered service, OS caches and provider database can already be warm.
These are headless core timings: executable startup, disk persistence, native
layout/rendering, images and presentation are excluded. They do not measure the
entire visible app restart or distinguish individual HTTP stages within a command.

## Local measurements, 14 November 2026

Five alternating before/after release runs on the same macOS host compared
`2370251` plus the identical example against this change. No builds or tests ran
alongside measurement. The workspace had nine projects and 27 saved OpenCode
sessions, including 16 archived sessions. The registered provider was warm.

| Selected history | Median readiness, before → after | Range, before → after |
| --- | --- | --- |
| Oldest open session | 43.83 → 30.85 ms | 40.61–48.88 → 27.90–35.44 ms |
| Previously selected session | 47.96 → 35.92 ms | 45.08–52.25 → 34.85–36.95 ms |

The older session returned 50 messages / 500,922 JSON bytes / 166 transcript
entries consistently: approximately **30% lower core readiness time**. The selected
session was live, with 5.09–5.91 MB and 130–135 entries; matching before/after pairs
had the same reported size, but that row is a changing workload. It improved by
approximately 25% in this sample.

Isolated HTTP probes of selected, older open and older archived histories took
roughly 4–27 ms, including JSON decoding. The oldest archived target failed the
provider adapter's resume step, so it was excluded from restoration timing.
The reported multi-second native delay was **not reproduced** by this headless
workload. These results establish reduced core contention, not a complete native
startup/rendering diagnosis. The blocked-request regressions cover the potentially
long wait independently of the warm-service measurements.

Raw, content-free results remain in ignored
`apps/desktop/profile-results/session-loading-2026-11-14/comparison.json` and
`provider-stages.json` in the same directory.

## Regressions

- Fill every normal worker with blocked detail reads; resume, history and inbox
  must finish before those reads are released. Fill the conversation workers too;
  cancellation must still finish independently.
- Block a checkout's initial form recovery on a local HTTP fixture. History from
  both an existing and a newly connected checkout must finish before release.
  Concurrent requests for the blocked checkout share a single SSE subscription.
- Release or shut down during initialization, then let the old request finish.
  Waiters are unblocked, stale questions stay discarded and a replacement
  connection remains live. Failed/disconnected attempts remain retryable.

Existing provider, history-reconciliation, pagination, ownership and submission
tests retain their original registration and contracts.
