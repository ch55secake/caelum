# Performance measurements

Current native evidence: [Rune evaluation and design follow-up](../design/rune-evaluation.md).
Historical web-client results: [release baseline](baseline-0.1.12.md) and
[bounded render publication](stack-0.1.12.md). Their source/workloads are available
at the recorded revisions; the browser application is no longer part of this tree.

Measure on the same machine, power mode, viewport and workload without simultaneous
builds/tests. Keep raw JSON with reports under ignored `apps/desktop/profile-results/`.
Repeat scenarios and compare medians/ranges rather than enforcing machine-specific
CI timing thresholds.

## Synthetic native frame measurement

```sh
nix develop -c cargo build --locked --release -p caelum-desktop
/usr/bin/time -l target/release/caelum-desktop \
  --evaluation-workload --native \
  --fps 120 --frames 1200 --trace /path/to/output/native.jsonl
```

This creates six sessions with 2,000 entries each and concurrent synthetic deltas,
plus a 300-file PR. The native window stays in the initial chat view while deltas
continue. Pinned Rune's `--native` path does not dispatch `--script` events.
It starts no providers and writes no saved workspace.
Report native presentation intervals separately from CPU/headless frame time and
end-to-end input latency. Synthetic work omits provider/network/storage cost.

## Headless interaction replay

```sh
target/release/caelum-desktop \
  --evaluation-workload --script apps/desktop/preview/evaluation.json \
  --fps 120 --frames 1200 --trace /path/to/output/headless.jsonl
```

The headless replay opens a patch, returns to chat, types, navigates and pastes
independent drafts on a virtual clock. Its timing describes headless CPU work.
Use the running application for native input, OS-menu and end-to-end latency checks.

### Session hover and focus observation

Rune's headless replay omits `App::on_frame_drawn`. To include Caelum's presented-focus
observation in CPU measurements, run the opt-in profile alone:

```sh
nix develop -c cargo test --locked -p caelum-desktop session_hover_switch_profile \
  -- --ignored --nocapture --test-threads=1
```

It replays idle, repeated session hover and 60 session switches against six
2,000-entry histories, both with and without synthetic streaming. The reported
frame time includes pointer dispatch, `Runtime::frame`, focus inspection and the
state update it requests. It excludes native rendering, OS event delivery and
provider/storage work; it is not an end-to-end latency measurement.

On 9 November 2026, a 15-second sample of the running development app during native
session hover attributed 3,431 of 11,965 main-thread samples to `live::frame_drawn`,
almost entirely `Runtime::elements`. This diagnostic API copies the whole tree and
walks ancestors for presentation metadata. It was being called after every drawn
frame, even when focus and the view were unchanged. The observer now caches the
commit/focus-presence stamp, and skips tree inspection entirely when nothing is
focused. Pinned Rune commits pointer, keyboard and accessibility focus changes;
regressions compare the observed key against the presented tree after navigation.

Initial same-machine development-profile results (1280×1000, 360 frames per phase):

| Workload | Frame + focus p50 before → after | p95 before → after |
| --- | --- | --- |
| Idle | 0.92 → 0.16 ms | 0.95 → 0.16 ms |
| Hover | 0.94 → 0.17 ms | 1.94 → 1.05 ms |
| Switching | 3.50 → 0.66 ms | 5.75 → 5.67 ms |
| Streaming + hover | 1.04 → 0.24 ms | 3.98 → 3.08 ms |
| Streaming + switching | 4.43 → 4.35 ms | 7.06 → 7.04 ms |

The reported multi-second beachball was not reproduced in these captures. This
removes measured redundant UI-thread work; it does not establish that the focus
scan was the only cause of that stall. Switching frames that rebuild the view
still inspect focus, which is reflected in their similar p95 times.

### Preview and persistence polling

Run the isolated UI-thread polling workload in release mode, separately from
builds and other tests:

```sh
nix develop -c cargo test --locked --release -p caelum-desktop idle_polling_profile \
  -- --ignored --nocapture --test-threads=1
```

It reports JSON p50/p95/max for `Live::poll` and `Workspace::commands` separately,
after 20 warmup polls and over 360 measured polls. Run it five times and compare
the medians/ranges. It uses the six 2,000-entry histories from the evaluation
fixture, with these variations:

- Idle: six settled sessions with persistence enabled.
- Tools: eight mounted entries, four 100-line patch inputs and four shell calls;
  each tool has 64 KiB of output.
- Background stream: the same tool rows while text grows in five other sessions.
- Populated: 120 sessions, each with a 7 KiB draft and a 64 KiB attachment,
  alongside the same histories and mounted tools.

The mounted-entry set is supplied directly; this measures polling, not viewport
layout or native input latency. Load/save replies are completed in memory, with
no provider process, filesystem writes or native rendering. Background text
changes happen outside the timed section. The session-navigation profile above
remains the separate frame/focus workload.

Preview polling caches the exact entry kind, tool name and input, including
negative results. Output/status/timing changes need no patch update. Cache source
strings are limited to 48 entries and 4 MiB and released when entries leave the
mounted set; oversized inputs use the uncached path. Cached history positions
are checked against entry IDs and rediscovered after history insertion/removal.
Persistence compares the effective saved projection by reference, including live
file-tab overrides and forgotten-session index remapping, and allocates a
snapshot only when it differs.

On 12 November 2026, five alternating before/after runs on the same macOS host
in the optimized release profile produced the following results. The baseline
was `facc3be` with the identical polling harness added; the after build includes
the input/position cache and borrowed persistence comparison. No builds or other
tests ran alongside the measurements. Values below are medians of each run's
percentiles, in **microseconds**:

| Workload / measured work | p50 before → after | p95 before → after |
| --- | --- | --- |
| Idle / persistence check | 0.88 → 0.13 | 1.00 → 0.13 |
| Tools / live polling | 34.67 → 0.58 | 38.08 → 0.58 |
| Background stream / live polling | 34.71 → 0.58 | 36.29 → 0.63 |
| Populated / live polling | 52.67 → 16.46 | 59.50 → 19.17 |
| Populated / persistence check | 321.17 → 171.92 | 352.00 → 189.46 |

Tool-polling p50 ranged from 34.08–36.58 µs before and 0.54–0.67 µs after;
populated persistence p50 ranged from 317.08–328.42 µs before and
171.50–172.38 µs after. These are about 98% and 46% reductions respectively in
the isolated work. The remaining populated persistence cost includes comparing
the attachment contents; unchanged polling no longer copies them into a new
snapshot. Sub-microsecond results are close to clock granularity.

The separate 1280×1000 frame/focus profile still measured streaming-switch p95
at 4.82 → 4.87 ms, with focus inspection at 1.77 → 1.82 ms: no material change.
This pass removes redundant polling work; native drawing and committed-frame
focus inspection remain follow-up targets for general responsiveness. Raw local
JSON is retained under `apps/desktop/profile-results/idle-polling-{before,after}.json`
and `apps/desktop/profile-results/navigation-{before,after}.json` (ignored).

## Packaged application

Build with `make app`, then launch with an isolated state directory:

```sh
open -n target/aarch64-apple-darwin/release/bundle/macos/Caelum.app \
  --env CAELUM_STATE=/absolute/path/to/profile/workspace.json
python3 scripts/profile/macos.py --pid PID \
  --label idle --duration 60 \
  --artifact target/aarch64-apple-darwin/release/bundle/macos/Caelum.app \
  --output apps/desktop/profile-results/idle.json
```

Use the launched app's PID and the host target triple. The state override skips
migration; keep its directory separate from daily-use data. Open a disposable
project for real-provider measurements, then relaunch against the same snapshot
to measure restoration. Record provider/model/history and output sizes.

| Scenario | Action |
| --- | --- |
| Startup/restoration | Fresh launch, warm relaunch, populated workspace |
| Idle | Loaded session in the foreground for 60 seconds |
| Streaming/concurrent | Same prompt/model; several sessions across checkouts/providers |
| History | At least 1,000 turns, read an old message, return to latest |
| Review | At least 100 changed files, inspect diffs, return to chat |
| Navigation | Switch projects/sessions repeatedly during background work |
| Recovery | Dismiss heavy views, sample immediately and after 60 seconds |
| Hidden window | Close, continue background work, reopen through the CLI |

The sampler measures interval CPU from cumulative process CPU time (one core =
100%) and RSS. Summed RSS includes shared pages and is not physical footprint.
Use macOS `sample` or Instruments separately for footprint/stack attribution.
Owned provider descendants are identified; external/shared services remain
unattributed and excluded from application totals. Its legacy WebKit grouping
is retained for interpreting the historical reports.

When considering event batching, measure output latency alongside CPU savings.
Cancellation, approvals, errors and completion must remain prompt; include
background sessions, history reconciliation and bounded queues in regressions.
