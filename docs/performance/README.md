# Performance measurements

Results: [release baseline](baseline-0.1.12.md) and
[feature stack with bounded render publication](stack-0.1.12.md).

Run measurements on the same machine, power mode, viewport, and commit stack,
without simultaneous builds or test suites. Keep the raw JSON alongside the
report; `apps/desktop/profile-results/` is ignored. Repeat each scenario at least
three times and compare medians and ranges, rather than enforcing machine-specific
CI timing thresholds.

## Release app (macOS)

Build an isolated application with a separate bundle identifier and workspace
database. Its initial state is empty; it does not restore the development app's
workspace. These commands use the normal optimized Rust and frontend builds:

```sh
nix develop -c pnpm --filter @caelum/desktop tauri build --ci \
  --config profile/tauri.conf.json --bundles app -- --locked
nix develop -c env DEVELOPER_DIR=/Library/Developer/CommandLineTools \
  pnpm --filter @caelum/desktop tauri bundle --ci \
  --config profile/tauri.conf.json --bundles app,dmg

python3 scripts/profile/macos.py \
  --app 'target/release/bundle/macos/Caelum Profile.app' \
  --label empty-start --duration 30 --quit \
  --artifact 'target/release/bundle/macos/Caelum Profile.app' \
  --artifact 'target/release/bundle/dmg/Caelum Profile_0.1.12_aarch64.dmg' \
  --artifact apps/desktop/dist \
  --output apps/desktop/profile-results/native/empty-start.json
```

Adjust the DMG name for the current version/architecture. Under Nix, only the
**bundle** command needs the host Command Line Tools path for `SetFile`; overriding
`DEVELOPER_DIR` during compilation conflicts with Nix's SDK selection.

Use `--project /path/to/disposable/repository` to open a project, then repeat without
that argument to measure restoration. To sample an interactive workload, launch
the profile app with `open -n` and use `--pid PID --label LABEL --duration 60`.

Native scenario checklist:

| Scenario | Action during the sample |
| --- | --- |
| Startup/restoration | First launch, warm relaunch, restore a populated workspace |
| Idle | Leave a loaded session in the foreground for 60 seconds |
| Streaming | Run the same prompt/model; record provider/model and output size |
| Concurrent | Three active sessions, including both providers and another checkout |
| History | Load 1,000 turns, reveal an old message, return to latest |
| PR | Open 100 changed files, inspect several diffs, then dismiss |
| Navigation | Switch sessions/projects 20 times with a background turn running |
| Recovery | Close heavy views and projects; sample immediately and after 60 seconds |

The sampler measures interval CPU from cumulative process CPU time (100% = one
core), and RSS in MiB. Summed RSS includes shared pages and is **not physical
footprint**. For physical footprint/stack attribution, run macOS `sample PID 10
-file PATH` separately, or use Instruments Allocations/Time Profiler. Do not mix
those instrumented runs into the uninstrumented CPU sample.

WebKit helpers are associated by the app's LaunchServices resource coalition,
including helpers parented by PID 1. Provider subprocesses must be descendants of
this app to count as owned. Shared/external servers appear as unattributed PIDs and
are excluded from app totals. A provider's descendants appear in the child group.
Launch separate .app instances through LaunchServices rather than a terminal to
avoid inheriting the terminal's coalition. The command never terminates existing
apps or external provider processes; `--quit` applies to its own launched app.

## Deterministic frontend workloads

```sh
nix develop -c pnpm --filter @caelum/desktop profile:build
nix develop -c pnpm --filter @caelum/desktop profile:ui
python3 scripts/profile/summarize.py apps/desktop/profile-results/ui
```

The dedicated server uses port 1431 and refuses to reuse an existing server. It
serves an optimized profiling build, with React's production profiling renderer,
and runs Chromium and Playwright WebKit serially, three times each. It covers
fresh-context and warm restoration, five-second idle, one stream at 100 deltas/s,
three streams at 300 deltas/s, 2,000 loaded history entries, 20 session and project
switches, a 100-file/40,000-line PR, and post-dismissal memory recovery.

Native commands/providers are mocked with the same fixtures as the browser suite.
This isolates UI cost; these results do not measure native IPC transport, provider
latency/CPU, or WKWebView's process footprint. Playwright WebKit is a useful engine
comparison, not the system WKWebView. Fresh-context startup does not flush the OS
disk cache. Startup durations include test-driver readiness observation.

Each run writes `metrics.json` with:

- React commit count, total render duration and p50/p95 duration.
- Callback event counts/JSON bytes by method, plus invoke counts/argument/result
  JSON bytes/duration by command. These are logical payload sizes, not wire bytes;
  callbacks count listener deliveries rather than unique native emissions.
- Frame gaps and long tasks where the browser exposes them.
- Stream event-to-DOM-mutation p50/p95/max latency and unobserved-token count.
  This excludes compositor presentation and native/provider transport latency.
- DOM/transcript node counts; Chromium heap/task metrics where available.
- Natural recovery and a separately labelled forced-GC snapshot in Chromium.

Instrumentation adds JSON serialization, a root Profiler and a shared animation
frame callback. Metric samples are bounded to the latest 4,096 observations and
128 method/command keys plus an overflow bucket. Compare like-for-like instrumented
runs, and confirm improvements in the normal native release app. Production builds
omit the profiling module, profiling renderer and instrumentation.

## Batching decisions

Use the same workload and event schedule before and after a candidate change.
Consider bounded session-ordered batching only if event/render frequency dominates
cost. Report output latency alongside CPU/render savings. Cancellation, approvals,
errors and completion must flush promptly; include background sessions, navigation,
history reconciliation and buffer bounds in regressions. Do not infer a native IPC
bottleneck from a mocked frontend benchmark alone.
