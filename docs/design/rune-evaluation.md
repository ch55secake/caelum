# Rune candidate evaluation record

## Design-parity follow-up — 6 November 2026

The live layout now shares Caelum's message/composer surfaces and theme vocabulary,
with composer pickers, a files/changes side inspector and focus-scoped dialogs.
`--design-fixture` renders the reference data through the real live views; the
light/dark, compact and settings captures are in `previews/`.

The 153 core/protocol/CLI tests and 12 native interaction tests pass; Clippy passes
with warnings denied. Compact coverage now checks inspector controls and modal
background focus/draft isolation. Long histories and review files remain windowed.

The same 1,200-frame native workload after the design changes reported presentation
interval p50/p95 **8.33/16.67 ms**, maximum **33.33 ms**, with ten intervals over
25 ms. Maximum RSS was **145,342,464 bytes** and peak footprint **279,954,296 bytes**.
This run is a responsiveness check, not an end-to-end input-latency measurement.
The earlier baseline remains below for comparison.

## Scope — 6 November 2026

Candidate branch: `feat/rune-evaluation-client`, based on the real-session
foundation in PR #92. These observations cover the consolidated candidate's
automated checks and a synthetic native workload. The real-use/cutover decision
in [the migration plan](rune-migration.md) is still open.

Environment: Apple M5 Pro, 24 GiB RAM, Mac17,9, macOS 26.5.1 (25F80), arm64.
Rune is pinned to `ddf14140253bd1b963993895e3fa025d1ec58537`; the candidate uses
Rust 1.95 and a release build with thin LTO.

## Functional evidence

- Root Rust suite: **153 passed** (139 core, 6 protocol, 8 CLI), with four test
  threads. Root and candidate Clippy pass with warnings denied; formatting passes.
- Rune deterministic suite: **12 passed**; two environment-dependent smoke tests
  are ignored by default and were run explicitly below.
- The native interaction suite covers editing/navigation, independent drafts,
  cancellation, scoped approvals/questions, next-message model changes during
  creation, file attachments, reading-position restoration, compact panels,
  transcript virtualization and a usable selected-PR-diff viewport.
- Core regressions cover disk-error admission, control replies during save failure,
  restart recovery of pending/failed/queued/newer prompts, queued settings,
  bounded reconnect, stale review generations and active-worktree removal.
- Real OpenCode discovery/history: **passed**, three workspace sessions discovered
  and 176 history entries rendered, using an isolated state file.
- Real read-only workspace services: **passed** against this checkout and PR #92:
  directory/file/search, Git/diff, worktree inventory, PR discovery/overview,
  18 changed files and discussion/review/thread feedback reads.
- Real Codex smoke: **blocked by the missing Codex executable**. Fake-process
  adapter tests pass, but are not a substitute for an authenticated real run.
- Dark transcript, light selected-diff and 800 × 620 compact synthetic captures
  were inspected. This is not a full accessibility or manual workflow acceptance.

An earlier unrestricted concurrent root run hit the existing GitHub timeout
fixture's missing `pid` race. That test passed alone; the final complete run with
four test threads passed. No production timeout changes were made for it.

## Synthetic native measurement

`--evaluation-workload` creates six sessions with 2,000 initial history entries
each and continuing small text deltas, plus a 300-file PR. The replay opens a
100-addition/100-deletion patch, returns to chat, types, switches sessions, pastes
an independent draft and returns. Providers and workspace persistence are disabled
in this mode.

The final native run was performed after builds/tests finished, with no other
agent-launched checks running. Actual viewport: **1280 × 949 pt at 2×** (macOS
constrained the requested 1000 pt height). Display-link pacing reported 120
redraws/presents per second. Rune's trace header still labels the display 60 Hz;
the observed 8.33 ms intervals and the 25 ms missed-frame threshold are reported
as emitted, without reinterpreting them as an input-latency measurement.

| Measurement | Result |
| --- | --- |
| Native submissions | 1,200 |
| Presentation trace | 1,198 presented, 2 skipped |
| Presentation interval p50 / p95 | 8.33 / 8.33 ms |
| Maximum presentation interval | 83.33 ms |
| Intervals over 25 ms | 1 (around 1.9 seconds) |
| Native CPU frame p50 / p95 / max | 0.45 / 2.42 / 16.86 ms |
| Timed process wall / user / system | 11.05 / 3.39 / 0.47 seconds |
| Maximum resident set | 149,340,160 bytes (142.4 MiB) |
| Peak memory footprint | 290,309,008 bytes (276.9 MiB) |
| Mounted transcript entries in deterministic check | 62, from 2,001 entries in the active session |

The one long presentation interval remains an observation to investigate, not
evidence of hitch-free rendering. The headless test's CPU p95 ranged from 1.21 to
2.19 ms across checks, including runs concurrent with compilation; those values
are not native GPU presentation times. The native table is the useful baseline.

## Reproduce

Run from the repository root. Choose a fresh directory for artifacts:

```sh
nix develop -c cargo build --locked --release --manifest-path apps/rune-prototype/Cargo.toml

/usr/bin/time -l apps/rune-prototype/target/release/caelum-rune-prototype \
  --evaluation-workload --native \
  --script apps/rune-prototype/preview/evaluation.json \
  --fps 120 --frames 1200 --trace /path/to/output/native.jsonl

apps/rune-prototype/target/release/caelum-rune-prototype \
  --evaluation-workload --script apps/rune-prototype/preview/evaluation.json \
  --theme light --scale 1 --size 1280x1000 --at 1.3 \
  --snap /path/to/output/review.png --tree /path/to/output/review.json

nix develop -c bash scripts/release/package_rune_candidate.sh /path/to/new/output
```

The package contains an ad-hoc-signed `.app` and architecture-specific ZIP;
`codesign --verify --deep --strict` and `plutil -lint` run during packaging.
The candidate CI uploads the ZIP. See the [candidate README](../../apps/rune-prototype/README.md)
for deterministic and real-provider/service smoke commands.

## What this does not establish

There is no matched native Tauri performance run, measured end-to-end input
latency, extended real concurrent-provider workload, real merge/update exercise,
complete restart/fault-injection campaign, or full accessibility acceptance yet.
Synthetic fixtures omit provider/network/storage costs. They establish a
reproducible rendering baseline and bounded mounting, not 90% daily-use parity.
Use the remaining-workflow list in the migration plan for the cutover evaluation.
