# Pull-request loading

## Workload and method

Measured on 14 November 2026 on the same macOS host, with builds and tests stopped.
Three repetitions each loaded the same three-file PR using:

1. The exact `gh` command chain from `2370251`: details, then concurrent files and
   three feedback requests. Files required PR validation → files → PR validation.
2. A fresh release-mode core runtime with an empty, isolated disk cache.
3. Another fresh process using the cache populated by step 2.

The new runtime measures from `Workspace::open_review` until data is admitted to
application state, including worker dispatch, JSON parsing and cache I/O. Runtime
construction and workspace-store initialization happen before the timer. The old
chain measures subprocess/network time only. Native rendering, whole-application
startup and queueing behind session restoration are excluded. OS filesystem caches
were not flushed; the restart case is **process-cold**, not disk-cold.

## Results

Milliseconds, median (range):

| Data available | Previous request chain | New, uncached | New, process restarted |
| --- | ---: | ---: | ---: |
| Overview | 647.05 (548.83–697.40) | 707.01 (692.09–909.49) | 0.73 (0.67–2.03) |
| Files | 1703.07 (1593.75–2028.47) | 1525.54 (1511.52–1741.08) | 0.73 (0.67–2.03) |

Fresh GitHub metadata replaced the cached overview after 697.99 ms
(601.97–704.93 ms). All restarted samples restored all three files. Checks and
merge readiness remained explicitly cached until fresh metadata arrived.

The uncached files critical path now has three GitHub requests instead of four:
details (including base/head SHAs) → files → final URL/base/head validation.
The removed pre-files request took 311–370 ms in this comparison. Network variance
affects total latency: an earlier three-run baseline on the same day measured
2354.95 ms to files, so the paired comparison above is preferable to claiming that
entire difference as an improvement.

Raw timings remain under ignored
`apps/desktop/profile-results/pr-loading-followup-2026-11-14/results.json` and
`apps/desktop/profile-results/pr-cold-load-2026-11-14.json`.

## Reproduce

Build the core-only measurement executable, then use an accessible PR and a new
isolated state path. Run the executable again with the same state path to measure
cache restoration in a new process:

```sh
nix develop -c cargo build --locked --release -p caelum-core --example review_load
target/release/examples/review_load . \
  https://github.com/example/repository/pull/42 \
  apps/desktop/profile-results/pr-load/workspace.json
```

The executable emits timings and file counts, without PR contents. Use a new state
filename for another uncached sample. Each state filename owns its own cache;
changing only the project/worktree reuses the canonical-URL cache entries.

The native `overview-cached` scene in `make previews` covers the loading display.
Native regressions cover light/dark compact layouts, an open inspector, failed
background refreshes, and exact diff reading-position retention when commits match.
The cache policy and ownership are documented in [architecture](../architecture.md).
