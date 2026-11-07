# Native desktop evaluation record

## Sidebar space and project closing — 7 November 2026

This follow-up continues in #95 above #94, following `c62c0b8`.

- Projects and Pull requests now share flexible, content-aware height with Sessions,
  replacing their 110/100-point caps. The four-project, six-PR fixture gives each
  section about **268 points at 1280×1000** and **134 points at 800×620**; Sessions
  retains about 223/111 points. Each section scrolls independently. Project cards
  use two lines with visible worktree/close actions; PR rows have full-title help.
- Project **×** and native context-menu **Close project** remove the entire checkout
  group from open navigation and session search. Stable session indices retain drafts,
  attachments, queues, live output and reading positions. Reopening restores the
  remembered session, including after restart; closed groups defer detail discovery
  until reopened. Closing the final project returns to the welcome screen.
- Request cutoffs prevent late Open, worktree discovery/creation and archive Undo
  callbacks from reopening or selecting a closed group. Newly created checkouts record
  their group before the asynchronous Open handoff.
- `nix develop -c make check` passed: **226 Rust tests** (168 core, 44 native,
  6 protocol, 8 CLI), formatting, warnings-denied workspace Clippy and **19 Python
  checks**. New regressions cover grouped close/reopen/restart, retained live output,
  context and queues, stale callbacks, compact section bounds, offscreen keyboard
  navigation, search exclusion and close-action focus recovery.
- Generated **360 captures**: 70 scenes plus 20 interaction states in both themes
  and sizes. All trees/logs passed diagnostic checks; populated, compact and ordinary
  sidebar images were visually reviewed. Refreshed **44 reference screens** and added
  **two populated-sidebar references**.
- **24 targeted headless replays** passed on the final build: background close with
  a retained editor, keyboard close, close-all/focus recovery, context-menu close,
  independent scrolling and closing from PR review, each in both themes and sizes.

The existing manual/native-menu visual and provider acceptance gaps remain below.
Artifacts: `/private/var/folders/sd/8zxnlg513mlfz8nkx928l6s00000gn/T/opencode/caelum-sidebar/`.

## Lighter composer, hideable commands and black appearance — 7 November 2026

This follow-up continues in #95 above #94, following `9281793`.

- Composer settings now use borderless inline labels, small chevrons, neutral hover
  treatment and full-value help; the model stays prominent and Queue is a quiet action.
- Background commands collapse to a 24-point count/Show row. Hide/Show retains keyboard
  focus, remembers each session independently across navigation/output updates and
  leaves commands running. Reopened Output/View all and Stop keep their captured owner.
- Settings → Appearance adds a saved **Black backgrounds** switch. Dark workspace and
  sidebar surfaces become `#000000`, with `#080808` panels; light appearance and
  theme-family text/accent choices are retained. Old snapshots default the option to off.
- `nix develop -c make check` passed: **222 Rust tests** (166 core, 42 native, 6 protocol,
  8 CLI), formatting, warnings-denied workspace Clippy and **19 Python checks**.
  Native regressions cover collapse/reopen bounds and keyboard focus, background output
  after navigation, draft retention and the appearance control across light/dark/system
  modes and snapshot restoration. Core store coverage includes the new preference and
  older snapshots lacking it. Palette assertions cover all four theme families.
- Generated **356 captures**: 69 scenes plus 20 interaction states in light/dark and
  normal/compact layouts. All trees/logs passed diagnostic checks; collapse/reopen state
  and bounds and dark/light surface values were checked. Representative composer,
  command and Settings images were visually reviewed. **44 native reference screens**
  are checked in, alongside the earlier historical before/after comparison.
- Archive, interaction and six-session evaluation replays passed with expected retained
  or submitted text and no tree warnings or missing-target/zero-height diagnostics.
- The final Apple Silicon app/DMG/CLI passed architecture, signing, system-library,
  disk-image and mounted-bundle checks. The packaged lifecycle smoke passed cold/warm
  project URLs, hidden-app reopening, draft restoration and graceful quit.

Both PRs remain open and unmerged. The existing Codex, Intel, native-menu and manual
provider/accessibility acceptance gaps remain recorded below. Artifacts for this pass:
`/private/var/folders/sd/8zxnlg513mlfz8nkx928l6s00000gn/T/opencode/caelum-refinement-v4/`.

## Composer, merge and Files refinement — 7 November 2026

This approved follow-up continues in #95 above #94. The previous pass at `6e56970`
supplies the matching before captures in the [comparison](previews/rune-refinement-comparison.png).

- **220 Rust tests passed**: 166 core, 40 native, 6 protocol and 8 CLI.
  `nix develop -c make check` passed formatting, warnings-denied workspace Clippy
  and **17 release + 2 profile Python checks**.
- The composer uses a 46–180-point editor, clearer contrast, model-first settings
  that wrap in narrow panes, aligned Send/Stop/Queue and shortcut tooltips. A native
  regression keeps all four settings and actions reachable with the inspector open,
  including while work runs; nested Escape returns focus without cancelling it.
- The Files inspector uses filename tabs with underlines, integrated hover/focus
  close controls and **Add to prompt / Add selection** actions. Native coverage
  roves horizontally to offscreen tabs, closes by keyboard, recovers focus after the
  final tab and keeps attachments with the originating draft.
- GitHub review has separate approval, check-summary and branch-readiness rows,
  expandable aligned checks and a joined green merge action/dropdown. Shared core
  presentation distinguishes skipped/unknown results, pending readiness, conflicts,
  blocked, behind, draft and terminal states. Native regressions verify compact
  action bounds, disabled/absent merge actions, check disclosure, method navigation,
  duplicate-mutation locking and captured project/URL/head after review/session changes.
- Default surfaces are neutral. Colour appears in selection markers, tab underlines,
  focus borders, links, semantic status icons and solid primary actions. Tool rows
  replace leading disclosure arrows with tool-specific icons and trailing Details/Hide.
- Generated and checked **332 captures**: 66 scenes plus 17 interaction states in
  light/dark and normal/compact layouts. Representative images were visually reviewed;
  all trees have no warnings, and logs have no missing targets or zero-height viewport
  diagnostics. Merge menus, expanded checks and narrow control bounds were inspected.
  **40 native reference screens plus one before/after comparison** are checked in.
- Archive, interaction and six-session evaluation headless replays completed with
  no tree warnings and retained the expected replacement draft, submitted prompt and
  concurrent-session draft text. Runtime regressions retain the existing ownership,
  cancellation, queue, virtualized history/diff and exact reading-position coverage.
- The final Apple Silicon app/DMG/CLI passed architecture, signing, system-library,
  disk-image and mounted-bundle checks. The packaged smoke passed cold/warm project
  URLs, hidden-app reopening, draft restoration and graceful quit.

Both PRs remain open and unmerged. Real Codex remains unavailable because its
executable is absent. Intel packaging, native OS-context-menu visual acceptance,
complete authenticated provider workflows and manual accessibility acceptance
remain open as recorded below. Current artifacts are under
`/private/var/folders/sd/8zxnlg513mlfz8nkx928l6s00000gn/T/opencode/caelum-refinement-v3/`.

## Overview, controls and message polish — 7 November 2026

This pass updates #95 above #94 against the historical `82ca39d` reference.

- **215 Rust tests passed**: 165 core, 36 native, 6 protocol and 8 CLI.
  `nix develop -c make check` passed formatting, warnings-denied Clippy and
  **17 release + 2 profile Python checks**. The grouped-file/image fixture was
  subsequently checked with its native rendering/copy regression and workspace Clippy.
- PR Overview now uses a centred 960-point reading column, Markdown description,
  compact branch metadata, aligned status/timing columns and joined readiness/merge
  controls. Selected-feedback actions appear only when needed and retain their
  captured destination and independent draft/attachments.
- Archive Undo is an eight-second, session-named notice above the composer. Core
  owns expiry, hover/focus/restoration pauses, retry lifetime and revision ordering
  so late archive/restore responses cannot replace a newer notice.
- Session actions use a row-anchored popover. A regression opens a background row,
  roves its menu, dismisses with Escape and refreshes its captured session without
  selecting it or cancelling foreground work. Row action focus is isolated from
  the sidebar's selection-following group.
- Worktree creation has a selected suggested branch, searchable base and automatic
  location preview. Its regression covers nested-menu Escape, Enter submission,
  duplicate Enter during the successful handoff, captured provider/project and
  focus in the new checkout. Default-base discovery honours `origin/HEAD`.
- Shared dropdowns cover composer controls, Settings, statistics, worktree bases
  and merge methods. The composer paperclip is removed; mentions, Files actions,
  project-file drops and image paste remain available.
- Background previews occupy three truncated visual lines. The scrollable reader
  retains its session and output snapshot, and **View all** exposes multiple commands.
  Native tests check compact bounds and Stop after session navigation. User messages
  use content-sized neutral Markdown bubbles, grouped file/image context and
  hover/keyboard copy actions that preserve the original Markdown.
- Generated **280 captures** (55 scenes and 15 interaction states × two themes × two
  sizes), refreshed affected captures after the focus repair, and reviewed
  representative light/dark normal/compact surfaces. **32 checked-in native images**
  document this pass. Capture trees/logs report no warnings or missing targets.
- Archive, interaction and six-session evaluation headless replays completed with
  no tree warnings; expected replacement drafts, submitted prompt and concurrent
  session draft text remained present. These replays exercise scripted input.
- Native rendering was remeasured with the presented-focus bridge enabled:

  | Scene | Presents / timed samples | Interval p50 / p95 / max | Failed acquisitions |
  | --- | --- | --- | --- |
  | Compact working state | 360 / 357 | 8.33 / 8.33 / 16.67 ms | 0 |
  | Six concurrent sessions, 2,000 entries each | 600 / 598 | 8.33 / 16.67 / 16.67 ms | 0 |

  Both used display-link pacing and reported zero bunched intervals or intervals
  over the report's 25 ms missed-frame threshold. The timing reports excluded 3 and
  2 samples respectively. Pinned Rune's native path does not dispatch scripts;
  these measurements cover rendering, not native input latency.
- Real OpenCode discovery/history passed with **3 sessions / 178 entries**; real
  directory, file, search, Git/diff, worktree and GitHub reads loaded **30 changed
  files** and all feedback categories. The isolated background-shell lifecycle
  smoke passed. Application state was isolated; model turns and PR mutations were
  not part of these service checks.
- Apple Silicon app/DMG/CLI packaging passed architecture, signing, system-library,
  disk-image and mounted-bundle checks, cold/warm project URLs, hidden-app reopening,
  draft restoration and graceful quit.

The stack remains unmerged. Codex's executable is unavailable locally; Intel
packaging, native OS-context-menu visual acceptance (screen-capture permission is
unavailable), complete authenticated provider workflows and manual accessibility
acceptance remain open. Artifacts for this pass are under
`/private/var/folders/sd/8zxnlg513mlfz8nkx928l6s00000gn/T/opencode/caelum-followup-v2/`.

## PR workspace and in-session follow-up — 7 November 2026

This follow-up continues in #95 above #94, using the final TypeScript tree at
`82ca39d` and its historical captures as the workflow reference.

- **207 Rust tests passed**: 163 core, 30 native, 6 protocol and 8 CLI.
  `nix develop -c make check` also passed formatting, warnings-denied workspace
  Clippy, **17 release + 2 profile Python checks**.
- PR review restores **Overview / Files changed**, description/discussion,
  grouped checks and merge readiness, searchable/collapsible file navigation,
  inline feedback, thread navigation and collapsed resolved threads. The pinned
  selected-comment action names its captured destination and submits a new turn
  while preserving that session’s independent draft and attachments.
- Exact-position coverage now includes an explicit refresh that clears/reloads
  files, a multi-page refresh beyond file 100, and tab navigation. Position capture
  includes the virtualized prefix; restoration survives the remounted viewport’s
  first measurements.
- Models open in an anchored, viewport-aware popover with search, grouped
  favourites/providers, unavailable models, keyboard navigation, modal focus,
  outside-click/Escape dismissal, focus return and saved-default controls.
- Settings is in the top toolbar; archive filtering is beside Sessions and actions
  are on the selected row. Shared buttons use consistent 32-point styling.
- Statistics has populated summary cards, an activity chart, token bars and model
  costs. Timeline has date groups, excerpts, Unicode search highlighting and
  stable-ID keyboard jumps, including after older history is inserted.
- Patches open by default with lazy visible-document preparation, bounded cache,
  retained collapse, preview-size and tool-detail controls. Reminder-only rows
  are removed before virtualization; displayed/copied responses, reasoning,
  statuses, timeline and side answers share filtering while literal code remains.
- Composer pulse and active-agent animation use session state through output gaps,
  tools, retry, input, background waiting and stopping. Tests cover settlement,
  hidden sidebar and reduced motion, alongside existing independent editors,
  captured approvals, cancellation, queues and six concurrent streams.
- Generated **236 captures** (47 scenes and 12 pointer/keyboard states × two themes
  × two sizes), inspected representative surfaces and updated **26 checked-in
  native images**. Populated fixtures include 48 prompts, activity/model statistics
  and a 12-file PR with inline and overview discussion. Capture logs reported no
  missing targets or zero-height scroll viewports. Statistics errors show a retryable
  notice rather than a misleading empty-history message.
- Headless archive, interaction and evaluation replays completed with no element-tree
  warnings. The archive replacement and concurrent-session editor retained their
  expected drafts; the submitted fixture prompt remained visible. These are scripted
  runtime results, not native OS-input evidence.
- A compact native working-scene run reported **360 display-link presents** and
  zero acquisition failures. The timing report used **357 presented samples / 3
  skipped**, with interval p50/p95 **8.33/8.33 ms**, maximum **16.67 ms**, and zero
  bunched or missed intervals. This records rendering and animation pacing; the
  replay-mode boundary described below still applies.
- Real OpenCode discovery/history passed with **3 sessions / 159 entries**; real
  workspace/GitHub reads loaded **30 changed files** and all feedback categories.
  The isolated background-shell lifecycle smoke passed. These checks use isolated
  application state and do not submit authenticated model turns or mutate PRs.
- Apple Silicon app/DMG/CLI packaging passed signing, architecture, dependency,
  disk-image verification and mounted-bundle checks, plus cold/warm project URLs,
  hidden-app reopening, draft restoration and graceful quit.

Visual and real-workflow acceptance remains distinct from these automated checks.
The two PRs remain unmerged while the documented provider, native-menu and manual
accessibility acceptance gaps remain open.

## Whole-workspace polish — 7 November 2026

The follow-up remains in #95, stacked on #94. It uses the same historical
`82ca39d` reference and pinned Rune/toolchain as the earlier parity pass.

- **199 Rust tests passed**: 160 core, 25 native, 6 protocol and 8 CLI.
  `nix develop -c make check` passes formatting, workspace Clippy with warnings
  denied, the complete Rust suite and **17 release + 2 profile Python checks**.
- New regressions exercise Escape before cancellation, Shift+Tab agent switching,
  nested Settings dropdowns, a 160-model virtual catalogue, arrow navigation after
  Tab, captured model ownership, file-row hit bounds, line selection/copy and
  immutable attachment ownership. Existing queue, approval, six-stream/history,
  300-file review and exact reading-position checks continue to pass.
- Real filesystem tests cover external edits, repeated modifications, staging,
  `Cargo.lock`, linked-worktree metadata and deferred in-flight refreshes. Watch
  registration now runs off the UI thread; the live smoke rerun no longer reported
  the earlier 820 ms blocked-poll warning.
- Shell regressions cover stale-list rejection, captured Stop ownership, bounded
  tails, output-read failure during Stop and late reads after completion. The
  opt-in real OpenCode lifecycle smoke passed isolated start/list/output/Stop,
  cleanup and archival without model inference.
- **188 captures** rendered successfully: 39 scenes plus eight interaction replays,
  each in both themes and both window sizes. Contact sheets and individual renders
  were inspected; **20 representative images** are checked in. The pass covers
  shared controls, command panels, five Settings pages, dropdowns, models, launcher,
  session/worktree dialogs, shells, code/patches, PRs, questions and images, including
  populated/empty/loading/error, hover/focus and disabled states. Populated fixtures
  release pending reads so refresh/retry actions have the correct enabled state.
- Real OpenCode discovery/history passed with **3 sessions / 158 rendered entries**.
  The isolated real workspace smoke loaded directories, files/search, Git/diffs,
  worktrees, PR discovery/overview, **30 changed files** and all feedback categories.
- Apple Silicon app/DMG/CLI packaging passed architecture, ad-hoc signatures,
  versions, bundle identity, URL registration, image verification and mounted-bundle
  equality. The isolated packaged launch smoke passed cold/warm URLs, hidden-app
  reopening, draft preservation, restart and graceful quit.
- The packaged native rendering run completed **780 presents**, with no
  acquisition failures. Its presentation trace reported p50/p95 **8.33/8.33 ms**,
  maximum **16.67 ms**, and no intervals above 25 ms.

**Replay-mode correction:** pinned Rune's `--native` window path does not dispatch
`--script` events. The latest trace contained only `live-draft-0`, with no replacement
editors from the close/undo script. The native measurements here, including earlier
runs originally described as replays, establish rendering/frame pacing of the
initial view and any ongoing synthetic deltas. Headless replays and runtime tests
dispatch the inputs; close/undo/typing remains covered by those interaction checks.
All three headless scripts were rerun successfully after correcting the commands;
the archive replay reached focused `live-draft-4` with the expected final text.

**Acceptance still open:** Codex is absent from PATH and the supported GUI discovery
locations, so its real authenticated workflow remains unverified. Intel packaging
was not run locally. macOS screen capture permission is unavailable. External
native-menu input attempts stalled, and a foreground-guarded attempt could not
obtain focus; actual OS context-menu appearance/selection is not signed off.
Runtime interaction tests and fixture captures do not close that gap.
Both PRs remain unmerged pending the
remaining visual and real-provider acceptance in [the migration record](rune-migration.md).

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
  the bundled icon attribution. Its native rendering run recorded 780 frames
  without an acquisition failure; the supplied close/undo script was not dispatched.
  The separate headless replay reached `live-draft-4` containing
  “Closing another session still works.”

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
This run measures rendering under synthetic deltas; scripted navigation and typing
were not dispatched, and end-to-end input latency was not measured.
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
each and continuing small text deltas, plus a 300-file PR. The native measurement
stays in the initial chat view while deltas continue. The separate headless replay
opens a 100-addition/100-deletion patch, returns to chat, types, switches sessions,
pastes an independent draft and returns. Providers and workspace persistence are
disabled in this mode.

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

Run from the repository root. These commands use the promoted desktop paths for
the historical workload. Choose a fresh output directory:

```sh
nix develop -c cargo build --locked --release -p caelum-desktop

# Native frame measurement: initial chat view with synthetic deltas.
/usr/bin/time -l target/release/caelum-desktop \
  --evaluation-workload --native \
  --fps 120 --frames 1200 --trace /path/to/output/native.jsonl

# Headless interaction replay on a virtual clock.
target/release/caelum-desktop \
  --evaluation-workload --script apps/desktop/preview/evaluation.json \
  --fps 120 --frames 1200 --trace /path/to/output/headless.jsonl

# Headless capture during the review step.
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
