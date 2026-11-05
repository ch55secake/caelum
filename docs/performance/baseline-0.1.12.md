# 0.1.12 baseline — 2026-11-03

Source: `a265e4f` before the workspace/settings/activity refactors. Only the opt-in
profiling harness was added. Reproduction commands and metric definitions are in
[README.md](README.md); repeat-level results are in
[baseline-0.1.12.json](baseline-0.1.12.json).

## Environment and scope

- Apple M5 Pro (`Mac17,9`), arm64, 24 GiB RAM, macOS 26.5.1.
- Battery power. This is a developer-machine baseline, with the development app
  and external provider processes still running. No builds or other test suites
  ran during the recorded samples.
- Native: normal release frontend/Rust (`lto=thin`, one codegen unit, stripped),
  isolated `dev.caelum.profile` identity; 1440 × 960 window.
- Frontend: optimized React profiling build, 1440 × 960 viewport, serial Chromium
  and Playwright WebKit workloads, three repeats each. Native/provider boundaries
  are mocked; this is not end-to-end provider or native IPC latency.

## Distribution size

| Artifact | Bytes | MiB |
| --- | ---: | ---: |
| Isolated release `.app` | 17,940,072 | 17.11 |
| DMG | 6,444,813 | 6.15 |
| All production frontend assets (123 files) | 2,495,541 | 2.38 |
| Frontend HTML/JS/CSS, individually gzipped | 846,070 | 0.81 |

The main production JS chunk is 752 kB uncompressed (225 kB gzipped); syntax and
file-preview chunks are lazy-loaded. The profiling renderer increases the main
chunk to 773 kB and adds a 2.3 kB metrics chunk. The normal production output has
no profiling module or metrics global. DMG size is from the isolated local bundle,
not a downloaded release artifact.

## Native process samples

Each row is a 30-second run, sampled every second, including launch/restoration.
RSS columns are the median across samples; CPU is mean interval CPU excluding the
first sample. CPU percentages use one core as 100%.

| Scenario | Native RSS MiB | WebKit RSS MiB | Native CPU % | WebKit CPU % |
| --- | ---: | ---: | ---: | ---: |
| First empty launch | 102.52 | 103.63 | 1.06 | 0.79 |
| Warm empty launch 1 | 107.27 | 104.38 | 3.64 | 5.02 |
| Warm empty launch 2 | 109.83 | 230.33 | 2.30 | 4.54 |
| Open disposable project | 113.11 | 124.88 | 1.72 | 1.51 |
| Restore disposable project 1 | 112.15 | 128.15 | 1.65 | 2.51 |
| Restore disposable project 2 | 112.19 | 127.11 | 1.65 | 2.54 |

The empty-run WebKit variation is substantial; do not treat a small RSS difference
as a regression. Each run had a distinct resource coalition and three matched
WebKit helpers. The running development app's helpers were excluded. RSS sums
include shared pages and cannot be equated with physical footprint.

No owned provider process was present in the one-second samples. External/shared
provider processes were excluded, and short-lived discovery processes may fall
between samples. This does **not** establish zero provider cost. Native live
streaming/concurrency, populated-session restoration, physical-footprint traces,
and native IPC wire volume still need interactive follow-up using the documented
scenario checklist. The deterministic frontend workloads cover the UI side of
those cases.

## Frontend workloads

Medians of three runs per browser (Chromium 156.0.8078.4; Playwright WebKit 27.2).
Render time is accumulated React render time within the phase, not wall time.
Streaming phases last approximately five seconds.

| Workload | Chromium commits | Chromium render ms | WebKit commits | WebKit render ms |
| --- | ---: | ---: | ---: | ---: |
| Idle, 5 s | 0 | 0 | 0 | 0 |
| Single stream, 500 deltas | 1,003 | 1,610 | 982 | 1,514 |
| Three streams, 1,500 deltas | 1,004 | 1,611 | 983 | 1,480 |
| Load 2,000 history entries | 19 | 68 | 20 | 62 |
| 20 session switches | 80 | 401 | 80 | 362 |
| 20 project switches | 218 | 306 | 181 | 263 |
| 100-file PR | 8 | 12 | 8 | 14 |

- Fresh-context restoration was observed at **98.6 ms / 126 ms** from navigation
  in Chromium/WebKit; warm restoration at **36.7 ms / 56 ms**. These include
  readiness observation overhead and are not native launch times.
- Single-stream event-to-DOM p95 latency was **5.7 ms / 7 ms**; three-stream p95
  was **5.7 ms / 6 ms**. All 500 visible tokens were observed in every run.
- The single-stream phase delivered 502 callback events and 80,255 JSON payload
  bytes; concurrency delivered 1,506 events. Chromium generally committed twice
  per visible delta. Concurrent events intentionally share each scheduler tick,
  so React already coalesces much of that workload; this is not a claim about
  three independently timed native IPC streams.
- Idle generated no React commits, invokes or provider callbacks over five seconds.
- The long-history case retained 2,000 entries while rendering **100 transcript
  entries**. The large PR mounted 10,794 DOM nodes including the hidden transcript;
  only viewport-adjacent diff bodies are fully rendered.
- Median Chromium JS heap reached **88.6 MiB** after repeated session navigation,
  was **29.0 MiB** five seconds after dismissing views, and **11.7 MiB** after an
  explicitly forced collection. Final DOM count was **402**, with no transcript
  entries mounted. Cached session state remains intentionally retained. This short
  run does not prove the absence of long-lived leaks. WebKit does not expose the
  corresponding heap/long-task metrics through this harness.

## Decisions for the implementation stack

1. **Address redundant visible-session state transitions with the unread work.**
   The current reducer sets unread on every changed event; a subsequent effect
   clears it for the selected session. The roughly two commits per visible delta
   make this the first concrete candidate to investigate. The foreground/latest-
   content read rule should avoid unnecessary toggles as well as fixing correctness.
2. **Keep streaming batching as a measured follow-up.** At 100 deltas/s the root
   spends roughly 1.5–1.6 seconds rendering during a five-second workload, although
   output latency remains low. After the refactors/activity changes, compare a
   bounded per-session flush cadence with this baseline. Preserve urgent terminal
   events and report any latency tradeoff.
3. **Do not add native IPC batching yet.** Logical callback/JSON counts are now
   available, but there is no measured native transport bottleneck in these runs.
4. **Retain bounded transcript/diff rendering.** The history limit holds. Repeat
   navigation and post-close measurements after the new launcher and activity UI;
   inspect retained objects if post-GC heap grows across multiple workload cycles.
5. **Track repeated navigation requests separately.** Twenty session switches made
   60 invokes, and project switches approximately 118–120. Profile history/status
   refreshes before changing their freshness or introducing request coalescing.
