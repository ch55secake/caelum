# Native desktop evaluation record

## Historical parity repair — 7 November 2026

The audit uses the final TypeScript tree at `82ca39d`, its interaction tests and
historical screenshots. The last follow-intent unread repair is `388d09e` in #94;
the native implementation remains in the second stacked PR, #95.

- **191 Rust tests passed**: 155 core, 22 native, 6 protocol and 8 CLI. The two
  external-service tests were then run explicitly and passed. Formatting and
  workspace Clippy with warnings denied pass; ShellCheck and Actionlint pass.
  The release/profile Python suites pass **17 + 2 checks**.
- Coverage includes bounded multiline composition, pinned Settings/Undo, local
  and provider-owned close/restore, caret-aware Unicode mentions and dismissal,
  model search/favourites/focus, side-question ownership, command failure, launcher
  handoff, local queue recovery, OpenCode inbox admission/removal retries and
  reconciliation, notifications/unread and image payloads for both adapters.
- The migration-lock regression now deliberately retains a duplicate file
  description during release. Explicit unlock-on-drop fixes the reproduced race
  with concurrently forked provider processes; subsequent locks remain exclusive.
- Files, Changes and PR review share virtualized native code views. Native tests
  verify source search/refresh and exact continuous-review position across refresh
  and tab navigation, alongside the 2,000-entry/six-stream and 300-file workloads.
  Outdated/unavailable-line comments scroll separately so a long feedback list
  leaves the patch viewport usable.
- **20 light/dark captures** cover workspace, inspector, compact, five-page Settings,
  typed tools, review, checks, mentions, images and crowded layouts. Ten representative
  scenes were inspected and checked in. Recognized tools omit redundant raw inputs;
  unavailable PR patches replace copy actions with an explicit GitHub link. Rune's
  pinned renderer uses its binary no-content note for GitHub-omitted patches too;
  the header help explains both cases.
- Real OpenCode discovery/history passed with **3 sessions and 158 rendered entries**
  using isolated state. Read-only workspace/GitHub services passed against this
  checkout and #95: directory/file/search, Git/diff, worktrees, PR discovery/details,
  **30 changed files** and all feedback categories.
- Apple Silicon app/DMG/CLI packaging passed architecture, ad-hoc signature,
  version, bundle identity, URL registration, image verification and mounted-bundle
  equality checks. The packaged CLI smoke passed cold/warm URL delivery, hidden-app
  reopening, preserved drafts, restart and graceful quit using isolated fake providers.

Real Codex remains unavailable locally because its executable is missing. These
read-only service checks do not establish end-to-end prompt/approval/inbox delivery,
remote mutation or accessibility acceptance; the remaining scope is listed below.

## Session-close and visual follow-up — 6 November 2026

The native pointer regression reproduced an out-of-bounds panic when closing the
selected local session: core selected a replacement before the next poll had
created its editor and reading-position state. Archive/restore callbacks now
synchronize immediately; provider completions use the same native reconciliation
path. Replacement and restored composers receive focus, and background archives
preserve the user's current selection and independent drafts.

- **173 Rust tests passed** (143 core, 16 native, 6 protocol, 8 CLI), plus **19
  Python checks**. Formatting, workspace Clippy with warnings denied and ShellCheck
  passed. The two opt-in external-service smoke tests were not rerun for this fix.
- New native interaction coverage exercises sidebar close, Session actions,
  local and both provider-owned sessions, failed remote archive, selection changes
  while archiving, undo, archived-list restore and typing after replacement/restore.
- Eight light/dark workspace, inspector, compact and settings captures were
  regenerated; the four checked-in comparison images were inspected and updated.
  The original Orbit mark, brand-row toggle and lighter text rendering are restored,
  with quieter session rows, a compact composer and aligned inspector controls.
- The Apple Silicon `.app` was rebuilt and passed signature/plist checks, including
  the bundled icon attribution. Its native close/undo replay ran for 780 native
  frames without an acquisition failure. The equivalent headless replay reached
  `live-draft-4` containing “Closing another session still works.”

## Production cutover — 6 November 2026

The native client now lives in `apps/desktop` and shares the root Rust workspace,
toolchain and lockfile. The Tauri/web build and application sources are removed.

- Unified Rust suite: **171 passed** (143 core, 14 native, 6 protocol, 8 CLI).
  Formatting and workspace Clippy pass with warnings denied.
- Migration coverage includes candidate precedence, drafts/attachments/queues,
  pending sends, legacy worktrees/session selections, branch history and committed
  SQLite WAL data. Corrupt/future sources and existing destinations are preserved.
  A parallel-fixture timestamp collision was reproduced and fixed with unique
  directory allocation; 30 repeated parallel migration runs then passed.
- Native lifecycle coverage checks cold URL ordering, running-app draft ownership,
  shutdown's latest-editor flush and lifetime-lock release.
- **19 Python checks passed** for version/release automation and profiling.
  Actionlint, ShellCheck and native preview capture passed; dark/light, inspector,
  compact and settings scenes were inspected.
- Real OpenCode discovery/history: **passed**, three sessions discovered and
  **197 history entries rendered** using isolated state. Read-only workspace/GitHub
  checks passed against this checkout and PR #94, including 19 changed files and
  discussion/review/thread feedback.
- Apple Silicon app/DMG/CLI packaging checks signatures, architectures, versions,
  bundle identity, URL registration and mounted-image equality. The packaged CLI
  launch check exercises cold/warm delivery, hidden-app reopening, preserved drafts
  and graceful quit in an isolated workspace with fake provider executables.

Intel packaging and Linux checks are configured in CI; they were not run locally.
Real Codex remains unavailable because its executable is not installed. The
real-provider fault, accessibility and extended workflow limits below still apply.

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
automated checks and a synthetic native workload. Cutover was subsequently
approved; [the migration record](rune-migration.md) tracks remaining acceptance.

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

Run from the repository root. These commands use the promoted desktop paths;
the observations above were recorded before promotion. Choose a fresh output directory:

```sh
nix develop -c cargo build --locked --release -p caelum-desktop

/usr/bin/time -l target/release/caelum-desktop \
  --evaluation-workload --native \
  --script apps/desktop/preview/evaluation.json \
  --fps 120 --frames 1200 --trace /path/to/output/native.jsonl

target/release/caelum-desktop \
  --evaluation-workload --script apps/desktop/preview/evaluation.json \
  --theme light --scale 1 --size 1280x1000 --at 1.3 \
  --snap /path/to/output/review.png --tree /path/to/output/review.json

nix develop -c bash scripts/release/package_macos_app.sh /path/to/output
```

The production package is an ad-hoc-signed `.app`; release tooling creates DMGs
and CLI archives. `codesign --verify --deep --strict` and `plutil -lint` run during
packaging. See the [desktop README](../../apps/desktop/README.md) for deterministic
and real-provider/service smoke commands.

## What this does not establish

There is no matched native Tauri performance run, measured end-to-end input
latency, extended real concurrent-provider workload, real merge/update exercise,
complete restart/fault-injection campaign, or full accessibility acceptance yet.
Synthetic fixtures omit provider/network/storage costs. They establish a
reproducible rendering baseline and bounded mounting, not 90% daily-use parity.
Use the remaining-workflow list in the migration record for further evaluation.
