# Feature-stack measurements — 2026-11-03

The completed feature stack plus bounded render publication reduces single-stream
React render time by **70% in Chromium / 58% in WebKit** against the
[release baseline](baseline-0.1.12.md). Output p95 rises from **5.7 / 7 ms** to
**47.1 / 45 ms**. All 500 visible tokens are observed in every repeat.

## Revisions and method

- Release baseline: `a265e4f`, with the opt-in harness from #67.
- Feature stack before publication changes: `a214963` (#76).
- Publication/metadata changes: `f0ceda1`.
- Same M5 Pro, 24 GiB, macOS 26.5.1, battery power, 1440 × 960 viewport,
  Chromium 156.0.8078.4 and Playwright WebKit 27.2. Existing development/provider
  processes remained present; no builds or other test suites ran during samples.
- The deterministic workload and event schedule are unchanged. Each frontend
  phase has three repeats per engine. Medians below are descriptive, not CI gates.
- After frontend runs used the exact patch subsequently committed as `f0ceda1`;
  the raw harness HEAD field still records its parent. Native runs used clean
  `f0ceda1`. [Repeat-level results](stack-0.1.12.json) preserve that distinction.

Raw runs are in ignored `apps/desktop/profile-results/stack-before` (Chromium),
`stack-before-webkit`, `stack-after`, `native-stack`, and `native-stack-traces`.
An outer command timeout interrupted the first combined pre-change run after
three Chromium repeats and one WebKit repeat. That WebKit repeat is excluded;
all three WebKit repeats were rerun together in `stack-before-webkit`.

## Why this change

The feature stack still produced roughly two commits per visible delta. PR
metadata discovery depended on session `updatedAt`, so each token restarted its
loading state even when metadata reads hit the cache. Discovery now depends on
stable session membership/availability; execution boundaries, Git events and
explicit refreshes continue to refresh metadata.

Incremental text, reasoning and command output now publish the latest workspace
snapshot after **32 ms or 128 updates**, whichever comes first. Every reducer
still runs immediately, in arrival order. History revision checks, cancellation
and queue decisions therefore see current state. There is one timer and counter,
with no retained event queue or duplicate snapshots. New output does not extend
the timer. Browser scheduling can delay timer delivery, particularly while hidden;
the update-count bound still applies.

Other events and local mutations publish immediately, including approvals,
questions, failures, completion, interruption, navigation and history responses.
Visibility changes flush; unmount cancels the timer. This targets UI rendering
cost. Callback counts and payload sizes remain unchanged, so it does not claim
to optimize native IPC.

## Streaming

Each stream lasts five seconds. The concurrent case sends three sessions' deltas
on the same scheduler tick, as in the baseline; it does not model three independent
native transports. Render time is accumulated React rendering, not wall time.

| Engine / workload | Baseline commits / render ms | Feature stack before | After |
| --- | ---: | ---: | ---: |
| Chromium / 500 deltas | 1,003 / 1,610 | 1,004 / 1,530 | **117 / 481** |
| Chromium / 1,500 deltas | 1,004 / 1,611 | 1,003 / 1,505 | **114 / 455** |
| WebKit / 500 deltas | 982 / 1,514 | 988 / 1,419 | **142 / 636** |
| WebKit / 1,500 deltas | 983 / 1,480 | 991 / 1,360 | **140 / 657** |

After single-stream render ranges are 480–505 ms / 625–655 ms. Chromium's
single-stream main-thread task time also falls from 2.210 s to 0.801 s (64%);
this is the CDP `TaskDuration` difference between idle and single-stream captures,
not whole-app CPU. Neither Chromium streaming phase records a long task.

| Engine / workload | Before output p95 ms | After p95 ms | Largest after observation ms |
| --- | ---: | ---: | ---: |
| Chromium / single | 5.7 | 47.1 | 52.3 |
| Chromium / concurrent | 5.8 | 47.2 | 53.2 |
| WebKit / single | 7 | 45 | 47 |
| WebKit / concurrent | 7 | 45 | 50 |

Latency ends at DOM mutation; it excludes provider/native transport and compositor
presentation. This is the explicit cost of reducing render frequency. Terminal
and input-request publication does not wait for that cadence.

## Startup, navigation, history and recovery

| Phase | Before render ms, Chromium / WebKit | After render ms, Chromium / WebKit |
| --- | ---: | ---: |
| Idle, 5 s | 0 / 0 | 0 / 0 |
| 2,000 history entries | 65 / 61 | 73 / 67 |
| 20 session switches | 404 / 352 | 412 / 395 |
| 20 project switches | 302 / 265 | 310 / 295 |
| 100-file PR | 13 / 14 | 13 / 16 |

Non-streaming cases show no improvement; several are modestly slower in these
runs. The changes do not defer those operations, and these measurements do not
establish the cause. Navigation still makes 60 session-switch invokes and about
118–120 project-switch invokes. Request coalescing remains a separate candidate.

- Fresh-context restoration is 117.6 / 141 ms; warm restoration 43.7 / 79 ms,
  including driver observation. Baseline: 98.6 / 126 and 36.7 / 56 ms.
- Idle remains at zero commits, invokes and callbacks in both engines.
- History retains 2,000 entries and mounts 100. The PR mounts 10,802 nodes;
  closing views returns to 424 nodes and zero mounted transcript entries.
- Chromium median heap after session navigation is 77.4 MiB, after closing views
  27.7 MiB, and after forced GC **11.75 MiB** (baseline 11.7 MiB).
  The pre-publication stack recovered to 12.02 MiB. These short runs do not prove
  absence of longer-lived retention; WebKit heap metrics remain unavailable.

## Native release app

The prior profiling identity had acquired a saved real project. A fresh
`dev.caelum.profile.stack` / `Caelum Stack Profile` bundle isolated this run
without altering that workspace. Build and bundle commands from the README were
given an additional override:

```sh
--config '{"productName":"Caelum Stack Profile","identifier":"dev.caelum.profile.stack"}'
```

Six 30-second samples use the same disposable repository as the baseline. RSS
values are per-run medians; CPU is mean interval CPU, one core = 100%. WebKit
helpers are matched by resource coalition. RSS includes shared pages.

| Scenario | Native RSS MiB | WebKit RSS MiB | Native CPU % | WebKit CPU % |
| --- | ---: | ---: | ---: | ---: |
| Empty launch | 99.81 | 107.27 | 1.17 | 0.41 |
| Warm empty 1 | 95.45 | 103.41 | 0.99 | 0.45 |
| Warm empty 2 | 103.69 | 105.98 | 0.86 | 0.38 |
| Open disposable project | 102.04 | 133.06 | 4.33 | 5.23 |
| Restore project 1 | 97.37 | 98.64 | 1.00 | 0.58 |
| Restore project 2 | 98.21 | 100.31 | 1.06 | 0.72 |

The first project-open sample is busier than baseline, while warm restoration is
quieter. Native startup variability prevents attributing these differences to
stream publication. No owned provider process appeared in the one-second samples;
external services and short-lived discovery costs are not included in app totals.

A separate instrumented idle restoration used five-second `sample` traces on
the app and its three WebKit helpers. Physical footprints as reported were:
native **25.9M**, GPU **23.6M**, networking **6737K**, WebContent **48.7M**.
They are sequential snapshots, not simultaneous totals. Native/WebContent main
threads were predominantly waiting in their event loops; the audio render thread
was waiting on a semaphore. These traces are excluded from the CPU table.

| Artifact | Baseline bytes | Feature stack bytes |
| --- | ---: | ---: |
| Release app | 17,940,072 | 19,288,330 |
| DMG | 6,444,813 | 7,165,479 |
| Frontend, 123 files | 2,495,541 | 2,544,884 |
| Frontend HTML/JS/CSS individually gzipped | 846,070 | 860,269 |

Normal production assets contain no profiling global/module. The native live
streaming/concurrent workload, populated-session restoration and native IPC wire
volume remain unmeasured. The frontend reductions are established by the mocked
workloads; native startup/idle samples do not establish equivalent end-to-end
savings. Native IPC batching remains unjustified by current evidence.
