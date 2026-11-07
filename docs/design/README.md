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
composer controls occupy separate rows; borderless model-first labels with small
chevrons wrap in narrow panes
beside an aligned Send/Stop action. Settings lives in the top toolbar, archive
filtering sits beside Sessions, and session actions belong to the clicked row.
Archive Undo is a temporary bottom-centred notice above the composer, pausing on
hover or keyboard focus. Files attach through mentions, the inspector or drops;
images also support paste.
The final TypeScript tree at `82ca39d`, its regression tests and historical UI
captures are the parity reference. `388d09e` adds the follow-intent unread repair.
Shared controls now cover command panels, questions, dialogs and dropdowns, with
tool-specific icons, quiet Details/Hide controls, green success indicators and
explicit loading, empty and error surfaces. Default surfaces stay neutral;
colour is carried by markers, underlines, focus borders, links and solid actions.
Model selection is an anchored searchable popover. Patch previews open by default;
the composer pulse and active-agent indicator follow session state between output
events. PR review restores Overview / Files changed, searchable file/thread
navigation, inline feedback and a selected-feedback bar naming its destination session.
The overview has a centred 960-point reading column, separate review/check/branch
rows, expandable checks and a joined GitHub-green merge button and method dropdown.
Filename tabs in the conversation inspector have an active underline, integrated
hover/focus close controls and a compact Add to prompt / Add selection toolbar.
Shared dropdowns keep field controls distinct from quiet
inline composer choices. Worktree creation provides a selected suggested branch,
searchable base and automatic location preview. Background commands show three
visual lines and open a captured-session output reader. Hide/Show collapses them to
a command-count row, remembered per session during the app run. User messages use
content-sized neutral Markdown bubbles with grouped attachments and copy actions.
Statistics uses summary/chart/token/model sections; timeline has date groups,
excerpts, highlighted search and stable prompt jumps.

| | Historical polished reference | Native Caelum |
| --- | --- | --- |
| Dark workspace | ![Reference dark](previews/react-dark.png) | ![Native dark](previews/rune-dark.png) |
| Light inspector | ![Reference inspector](previews/react-light-inspector.png) | ![Native inspector](previews/rune-light-inspector.png) |

| Compact workspace | Settings |
| --- | --- |
| ![Native compact](previews/rune-dark-compact.png) | ![Native settings](previews/rune-dark-settings.png) |

| Typed tools | Continuous review |
| --- | --- |
| ![Native tools](previews/rune-dark-tools.png) | ![Native review](previews/rune-dark-review.png) |

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

| Compact file tabs | Tool icons and details |
| --- | --- |
| ![Native compact files](previews/rune-dark-files-tabs-small.png) | ![Native collapsed tools](previews/rune-dark-tools-collapsed.png) |

| Ready to merge | Review required |
| --- | --- |
| ![Native ready merge](previews/rune-light-merge-ready-small.png) | ![Native blocked merge](previews/rune-dark-merge-blocked-small.png) |

| Expanded checks | Branch update |
| --- | --- |
| ![Native check details](previews/rune-light-check-details.png) | ![Native branch behind](previews/rune-dark-merge-behind-small.png) |

| Black workspace | Black appearance setting |
| --- | --- |
| ![Native black workspace](previews/rune-dark-black.png) | ![Native black settings](previews/rune-dark-black-settings-small.png) |

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

Four theme families and six accent colours (plus the theme default) apply across
the workspace. The saved **Black backgrounds** option uses black workspace/sidebar
surfaces and near-black panels in dark appearance, keeping theme text and accents.
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

The capture script writes **360 states**: 70 scenes and twenty headless
pointer/keyboard replays, each in light/dark and normal/compact layouts. Coverage
includes all five Settings pages, models, launcher, session popovers/worktree forms, commands, shells,
Files/Changes, populated PR discussion/diffs, statistics/timeline, activity states,
loading/error/empty views, questions, images, hover and keyboard focus. The latest
fixtures include long composer options, multiple file tabs, collapsed tools,
ready/blocked/pending/conflicting/behind/draft/merged/closed PR states, black
backgrounds, command collapse/reopen interactions and a populated four-project,
six-PR sidebar.
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
