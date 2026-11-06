# Performance measurements

Current native evidence: [Rune evaluation and design follow-up](../design/rune-evaluation.md).
Historical web-client results: [release baseline](baseline-0.1.12.md) and
[bounded render publication](stack-0.1.12.md). Their source/workloads are available
at the recorded revisions; the browser application is no longer part of this tree.

Measure on the same machine, power mode, viewport and workload without simultaneous
builds/tests. Keep raw JSON with reports under ignored `apps/desktop/profile-results/`.
Repeat scenarios and compare medians/ranges rather than enforcing machine-specific
CI timing thresholds.

## Deterministic native workload

```sh
nix develop -c cargo build --locked --release -p caelum-desktop
/usr/bin/time -l target/release/caelum-desktop \
  --evaluation-workload --native \
  --script apps/desktop/preview/evaluation.json \
  --fps 120 --frames 1200 --trace /path/to/output/native.jsonl
```

This creates six sessions with 2,000 entries each and concurrent synthetic deltas,
plus a 300-file PR. The replay opens a patch, returns to chat, types, navigates and
pastes independent drafts. It starts no providers and writes no saved workspace.
Report native presentation intervals separately from CPU/headless frame time and
end-to-end input latency. Synthetic work omits provider/network/storage cost.

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
