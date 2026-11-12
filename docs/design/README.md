# Native workspace design

Caelum uses flush conversation surfaces, quiet sidebar selection, system-font
typography and a rounded composer. Rune supplies native text editing, animated
split panes, Markdown and windowed diffs. Provider/model/effort/agent controls live
in the composer; files and changes share the side inspector. Settings and worktree
creation use focus-scoped dialogs; session actions use row-anchored popovers.
PR review occupies the main screen.

## Visual reference

The polished web-client captures remain the historical design reference. Native
captures use the actual live views with offline data from [`workspace.json`](workspace.json).
Screens are 1280 × 1000 at one pixel per point, except the 800 × 620 compact/crowded scenes.
The follow-up captures restore the original 18-point Lucide Orbit mark, keep the
sidebar toggle beside the brand, and use browser-equivalent unsmoothed system text.
The inspector sits below the shared toolbar; pane widths adapt to compact windows
while retaining the user's preferred sizes. The bounded multiline editor and
composer controls occupy separate rows; borderless model-first labels and icon-led
effort/agent choices wrap in narrow panes. Their popouts show titles, readable
choices and default options; the provider is chosen in Settings. The controls sit
beside a labelled Send/Queue action and a separate Stop control; narrow panes put
actions on their own row. Settings lives in the top toolbar, archive
filtering sits beside Sessions, and session actions belong to the clicked row.
Archive Undo is a temporary bottom-centred notice above the composer, pausing on
hover or keyboard focus. Files attach through mentions, the inspector or drops;
images also support paste.
The final TypeScript tree at `82ca39d`, its regression tests and historical UI
captures are the parity reference. `388d09e` adds the follow-intent unread repair.
Shared controls now cover command panels, questions, dialogs and dropdowns, with
tool-specific icons, compact disclosure chevrons, green success indicators and
explicit loading, empty and error surfaces. Default surfaces stay neutral;
colour is carried by markers, underlines, focus borders, links and solid actions.
Active tool, reasoning, compaction, background-command, PR-check and update labels
share a restrained 2.2-second text sheen; icon-only progress uses the same effect.
Thinking uses a brain glyph, while compact generic activity markers use a clock.
Compaction and general composer activity rely on their labels to keep them quiet.
Reduced motion holds the highlight still. Tool details focus on output and errors,
with raw arguments and successful exit codes omitted. Routine metadata belongs in
expanded details; non-zero exits and failures remain visible. Compaction is a quiet
conversation divider with an optional expandable, copyable Markdown summary. Empty
summaries still produce a milestone, and failures survive history refreshes.
Expanded tool output sits directly on the conversation surface beside a thin rule.
Twelve-point monospace text uses twenty-point leading, wraps long tokens and stays
selectable; supported file extensions receive the shared syntax palette. A compact
copy action sits alongside the output, and single-file navigation lives in the tool
heading. Errors lead the logs with a labelled danger-coloured rule. Long output
has an independent 260-point scroll viewport and line-count footer; previews stop
at a complete line within 12,000 characters where possible. Copy retains the full
result, and full-output expansion preserves reading position.
Model selection is an anchored searchable popover. Inline patch previews open by
default and size to their visible file headers, code and unchanged-line gaps. Short
patches have no expansion footer; longer patches start at a 204-point viewport and
can expand to half the window height, up to 480 points. Collapsing a file reclaims
its space. The file count and tool-details action share the tool heading; filenames
stay in the diff headers. Headers use the conversation background, and selections
and hunk gaps follow the theme accent. Settings uses a vertical section list,
aligned label/control rows and page-specific heights with scrollable overflow.
Switching sections resets scrolling and dismisses the previous page's dropdown.
Provider model fields show their friendly names.
Settings sections have quiet icons. About uses a theme-aware Orbit mark, a compact
version/platform line, grouped project links and a dedicated update-status panel.
Checking, current, available, newer-build and error states share the same panel;
downloads appear only for an available update, with a release-page fallback when
no matching download exists. Legacy preferences import has its own labelled row.
The docked composer uses smoky/milky tint, a subtle gradient, a top-lit hairline and
soft shadow. The edge stays neutral when editing; text and controls stay crisp.
The transcript viewport ends above the composer. The flat background needs
no backdrop-blur pass. A compact activity label follows session state between output
events, and the queue count expands into bounded controls with per-session disclosure.
The docked editor grows naturally with its draft, up to a window-aware 420-point
limit, and shrinks as text is removed. It preserves native selection and undo;
each session keeps its scroll position. The height budget reserves conversation
space above the dock. Attachment chips
wrap within the available width, with image thumbnails, a distinct remove control
and a visible Partial label. File previews fit short snapshots, with a compact
filename/path header, the Files syntax palette, original line numbers and a quiet
snapshot/Copy footer. Longer captures scroll in the code surface. Image previews
share the zoom/pan reader. Reads and failures
are named per file in the originating draft.
PR review restores Overview /
Files changed, searchable file/thread navigation, inline feedback and a
selected-feedback bar naming its destination session.
The overview has a centred 960-point reading column, separate review/check/branch
rows, expandable checks and a joined GitHub-green merge button and method dropdown.
Filename tabs in the conversation inspector have an active underline, integrated
hover/focus close controls and a compact Add to prompt / Add selection toolbar.
Shared dropdowns keep field controls distinct from quiet
inline composer choices. Worktree creation provides a selected suggested branch,
searchable base and automatic location preview. Background commands show three
visual lines and open a captured-session output reader. Hide/Show collapses them to
a command-count row, remembered per session during the app run. Sessions waiting on
background commands show an amber waiting label in the sidebar until foreground
work resumes or the last command finishes. User messages use
content-sized neutral Markdown bubbles with grouped attachments and copy actions.
Statistics uses summary/chart/token/model sections; timeline has date groups,
excerpts, highlighted search and stable prompt jumps.

| | Historical polished reference | Native Caelum |
| --- | --- | --- |
| Dark workspace | ![Reference dark](previews/react-dark.png) | ![Native dark](previews/rune-dark.png) |
| Light inspector | ![Reference inspector](previews/react-light-inspector.png) | ![Native inspector](previews/rune-light-inspector.png) |

| Frosted composer · working and queued | Frosted composer · idle |
| --- | --- |
| ![Smoky docked composer](previews/rune-dark-composer-queued.png) | ![Milky docked composer](previews/rune-light-composer-empty.png) |

| Expanded queue · compact | Attachments · compact |
| --- | --- |
| ![Expanded queue controls](previews/rune-dark-composer-queue-expanded-small.png) | ![Wrapped attachment chips](previews/rune-light-composer-attachments-small.png) |

| Natural prompt growth | Long prompt · compact |
| --- | --- |
| ![Naturally growing docked editor](previews/rune-dark-composer-long.png) | ![Compact long-prompt editor](previews/rune-light-composer-long-small.png) |

| Attachment loading and failure | Captured text snapshot |
| --- | --- |
| ![Named attachment status](previews/rune-light-composer-attachment-status-small.png) | ![Partial snapshot preview](previews/rune-dark-composer-attachment-preview-small.png) |

| Captured text snapshot · light | Captured text snapshot · dark |
| --- | --- |
| ![Light source snapshot reader](previews/rune-light-composer-attachment-preview-small.png) | ![Dark source snapshot reader](previews/rune-dark-composer-attachment-preview-small.png) |

| Captured image snapshot | Preview and removal controls |
| --- | --- |
| ![Attached image viewer](previews/rune-light-composer-image-preview-small.png) | ![Separate attachment controls](previews/rune-dark-composer-attachments-small.png) |

| Compact workspace | Settings |
| --- | --- |
| ![Native compact](previews/rune-dark-compact.png) | ![Native settings](previews/rune-dark-settings.png) |

| Typed tools | Continuous review |
| --- | --- |
| ![Native tools](previews/rune-dark-tools.png) | ![Native review](previews/rune-dark-review.png) |

| Compact Vercel tool output | Expanded failure output |
| --- | --- |
| ![Flat output and highlighted source](previews/rune-light-vercel-tools-small.png) | ![Error summary above command logs](previews/rune-dark-tool-error-expanded-small.png) |

| Long command output | Wrapped search results |
| --- | --- |
| ![Bounded Vercel output preview](previews/rune-dark-vercel-tool-output-long-small.png) | ![Compact search results](previews/rune-light-tool-search-small.png) |

| Active tool call | Expanded context summary |
| --- | --- |
| ![Running command with text sheen](previews/rune-dark-tool-running-small.png) | ![Compaction summary](previews/rune-light-compaction-summary.png) |

| Crowded sidebar and multiline composer | Images |
| --- | --- |
| ![Native crowded](previews/rune-dark-crowded.png) | ![Native images](previews/rune-light-images.png) |

| Caret-aware PR mentions | Check status and timing |
| --- | --- |
| ![Native mentions](previews/rune-dark-mentions.png) | ![Native checks](previews/rune-dark-checks.png) |

| Searchable models | Compact session actions |
| --- | --- |
| ![Native models](previews/rune-light-models-small.png) | ![Native session actions](previews/rune-dark-session-small.png) |

| Project/provider launcher | Background commands |
| --- | --- |
| ![Native launcher](previews/rune-light-launcher-small.png) | ![Native shells](previews/rune-dark-shells-small.png) |

| Worktree creation | Provider dropdown |
| --- | --- |
| ![Native worktree form](previews/rune-dark-worktrees-form-small.png) | ![Native dropdown](previews/rune-light-provider-dropdown-small.png) |

| Files loading | Pull request loading |
| --- | --- |
| ![Native file loading](previews/rune-light-files-loading-small.png) | ![Native PR loading](previews/rune-dark-review-loading-small.png) |

| Provider connection error | Question choices |
| --- | --- |
| ![Native provider error](previews/rune-light-provider-error.png) | ![Native questions](previews/rune-dark-questions-small.png) |

| Populated statistics | Prompt timeline |
| --- | --- |
| ![Native statistics](previews/rune-light-stats.png) | ![Native timeline](previews/rune-dark-timeline-small.png) |

| Compact PR workspace | In-session working indicator |
| --- | --- |
| ![Native compact review](previews/rune-light-review-small.png) | ![Native working](previews/rune-dark-working-small.png) |

| Compact token/model breakdown | Timeline search |
| --- | --- |
| ![Native statistics details](previews/rune-dark-stats-details-small.png) | ![Native prompt search](previews/rune-light-timeline-search-small.png) |

| Readable PR overview | Temporary archive Undo |
| --- | --- |
| ![Native PR overview](previews/rune-light-overview.png) | ![Native archive notice](previews/rune-dark-archive-small.png) |

| User messages and attachments | Command output reader |
| --- | --- |
| ![Native user messages](previews/rune-light-messages.png) | ![Native command output](previews/rune-dark-shell-output-small.png) |

| Searchable worktree base | Merge method menu |
| --- | --- |
| ![Native worktree bases](previews/rune-light-worktree-base-small.png) | ![Native merge methods](previews/rune-dark-merge-method-small.png) |

| Wrapping composer settings | Filename tabs and attachment actions |
| --- | --- |
| ![Native narrow composer](previews/rune-dark-composer-options-small.png) | ![Native file tabs](previews/rune-light-files-tabs.png) |

| Reasoning effort menu | Agent menu |
| --- | --- |
| ![Native effort choices](previews/rune-dark-composer-effort.png) | ![Native agent choices](previews/rune-light-composer-agent-small.png) |

| Session context menu | Composer editing menu |
| --- | --- |
| ![Custom session actions](previews/rune-dark-context-session.png) | ![Custom editing actions](previews/rune-light-context-editor-small.png) |

| Caelum worktrees | Compact project navigation |
| --- | --- |
| ![Caelum worktree folders and activity](previews/rune-dark-sidebar-worktrees.png) | ![Worktrees inside their parent project](previews/rune-light-sidebar-worktrees-small.png) |

| Multiple project groups | Project-scoped worktree manager |
| --- | --- |
| ![Worktree ownership across projects](previews/rune-dark-sidebar-worktrees-projects.png) | ![Compact Caelum worktree manager](previews/rune-light-worktree-manager-small.png) |

| Vercel worktrees | Compact Vercel worktrees |
| --- | --- |
| ![Flat Vercel worktree hierarchy](previews/rune-dark-sidebar-worktrees-vercel.png) | ![Light Vercel worktrees](previews/rune-light-sidebar-worktrees-vercel-small.png) |

| Compact file tabs | Tool icons and details |
| --- | --- |
| ![Native compact files](previews/rune-dark-files-tabs-small.png) | ![Native collapsed tools](previews/rune-dark-tools-collapsed.png) |

| Ready to merge | Review required |
| --- | --- |
| ![Native ready merge](previews/rune-light-merge-ready-small.png) | ![Native blocked merge](previews/rune-dark-merge-blocked-small.png) |

| Expanded checks | Branch update |
| --- | --- |
| ![Native check details](previews/rune-light-check-details.png) | ![Native branch behind](previews/rune-dark-merge-behind-small.png) |

| Vercel workspace | Vercel appearance setting |
| --- | --- |
| ![Native Vercel workspace](previews/rune-dark-vercel.png) | ![Native Vercel settings](previews/rune-dark-vercel-settings-small.png) |

| About Caelum | Available update |
| --- | --- |
| ![Vercel About page](previews/rune-dark-vercel-about-small.png) | ![About with an update available](previews/rune-light-about-update-small.png) |

| Short inline patch | Multiple-file inline patch |
| --- | --- |
| ![Compact Vercel patch](previews/rune-dark-vercel-patch-short-small.png) | ![Light Vercel patch](previews/rune-light-vercel-patch-multi-small.png) |

| Expanded inline patch | Compact notification settings |
| --- | --- |
| ![Expanded Vercel patch](previews/rune-dark-patch-expanded.png) | ![Vercel notifications](previews/rune-light-vercel-notifications-small.png) |

| Hidden background commands | Reopened command previews |
| --- | --- |
| ![Native hidden commands](previews/rune-dark-shells-hide-small.png) | ![Native reopened commands](previews/rune-light-shells-reopen-small.png) |

| Projects and pull requests | Compact sidebar |
| --- | --- |
| ![Native populated sidebar](previews/rune-light-sidebar-projects.png) | ![Native compact sidebar](previews/rune-dark-sidebar-projects-small.png) |

Projects and Pull requests receive content-aware shares of sidebar height and
independent scrollbars. Two-line project cards keep branch/status information and
visible worktree/close actions. Closing a project retains its sessions, drafts and
running work for reopening; arrow keys navigate the lists and project actions.

[Earlier before/after comparison (`6e56970` → `9281793`)](previews/rune-refinement-comparison.png)
records the previous composer, Files inspector, merge panel and tool-header pass
using matching fixture content.

Five theme families and six accent colours (plus the theme default) apply across
the workspace. **Vercel** uses a pure-black workspace and sidebar, subtly raised panels,
a soft hairline sidebar divider with fading ends and monochrome controls, with a
corresponding light palette.
Existing Black backgrounds preferences migrate to Vercel. Theme cards wrap in
compact Settings and preview the current light/dark appearance.
Rune follows system reduced-motion preferences. Use the running
application to judge focus, typing, scrolling, pane motion and OS context menus;
headless replays provide deterministic interaction coverage.

## Capture and replay

Run from the root with Rust 1.95.0, Python 3.11+ and macOS developer tools, or
inside `nix develop`:

```sh
python3 apps/desktop/preview/capture.py
cargo run --locked -p caelum-desktop -- --design-fixture

cargo run --locked -p caelum-desktop -- \
  --fixtures --script apps/desktop/preview/interaction.json \
  --theme dark --scale 1 --at 9 \
  --snap apps/desktop/preview-results/rune-stream.png

# Headless: close, type into the replacement, undo, switch back, and close again.
cargo run --locked -p caelum-desktop -- \
  --design-fixture --script apps/desktop/preview/archive.json \
  --fps 120 --at 6 \
  --snap apps/desktop/preview-results/rune-archive.png \
  --tree apps/desktop/preview-results/rune-archive.json

# Native rendering/frame measurement of the initial fixture view.
cargo run --locked -p caelum-desktop -- \
  --design-fixture --native --fps 120 --frames 780 \
  --trace apps/desktop/preview-results/native.jsonl
```

Pinned Rune's `--native` mode does not dispatch `--script` events. Use the
headless commands above for scripted input and the running app for OS interaction.

The capture script writes **524 states**: 101 scenes and 30 headless
pointer/keyboard replays, each in light/dark and normal/compact layouts. Coverage
includes all five Settings pages, models, launcher, session popovers/worktree forms, commands, shells,
Files/Changes, populated PR discussion/diffs, statistics/timeline, activity states,
loading/error/empty views, questions, images, hover and keyboard focus. The latest
fixtures include long composer options, multiple file tabs, collapsed tools,
ready/blocked/pending/conflicting/behind/draft/merged/closed PR states, Vercel
appearance, command collapse/reopen interactions and a populated four-project,
six-PR sidebar. Inline patches cover short, long and multiple-file changes in
Caelum and Vercel, plus explicit collapse and expansion replays. About includes
checking, current, available, missing-download, newer-build and error states.
Tool activity includes running and failed commands, expanded errors, wrapped search
results, long-output expansion and Vercel output surfaces. Compaction covers running,
completed, empty-summary, failed and interrupted events, plus summary expansion.
PNGs, element-tree JSON and logs go to ignored `apps/desktop/preview-results/`. An optional output
directory argument is relative to the current directory. With FFmpeg installed,
replace the replay's `--at 9 --snap …` with `--video /path/to/replay.mp4 --duration auto`.
Rune's `--help` lists tooling options.

`--fixtures` supports editing/sending simulated replies, background streaming,
pane resizing, stopping and light/dark switching. `--evaluation-workload` adds
large histories, concurrent streams and a large PR. All fixture modes are isolated
from providers and saved data.

## Checks and measurements

`cargo test --locked -p caelum-desktop` drives Rune runtime keyboard/pointer interactions,
including drafts, focus, approvals/questions, compact layout and bounded mounting.
`make check` also checks the shared Rust core and release scripts.

See [native measurements](rune-evaluation.md), [performance instructions](../performance/README.md),
[desktop development](../../apps/desktop/README.md) and [migration status](rune-migration.md).
