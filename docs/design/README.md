# Native workspace design

Caelum uses flush conversation surfaces, quiet sidebar selection, system-font
typography and a rounded composer. Rune supplies native text editing, animated
split panes, Markdown and windowed diffs. Provider/model/effort/agent controls live
in the composer; files and changes share the side inspector. Settings, session
actions and worktrees use focus-scoped dialogs. PR review occupies the main screen.

## Visual reference

The polished web-client captures remain the historical design reference. Native
captures use the actual live views with offline data from [`workspace.json`](workspace.json).
Screens are 1280 × 1000 at one pixel per point, except the 800 × 620 compact/crowded scenes.
The follow-up captures restore the original 18-point Lucide Orbit mark, keep the
sidebar toggle beside the brand, and use browser-equivalent unsmoothed system text.
The inspector sits below the shared toolbar; pane widths adapt to compact windows
while retaining the user's preferred sizes. The bounded multiline editor and
composer controls occupy separate rows; Settings and Undo remain in a pinned footer.
The final TypeScript tree at `82ca39d`, its regression tests and historical UI
captures are the parity reference. `388d09e` adds the follow-intent unread repair.

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

Four theme families and six accent colours (plus the theme default) apply across
the workspace. Rune follows system reduced-motion preferences. Use native replay
and the running application to judge focus, typing, scrolling and pane motion.

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

# Close, type into the replacement, undo, switch back, and close again.
cargo run --locked -p caelum-desktop -- \
  --design-fixture --script apps/desktop/preview/archive.json \
  --native --fps 120 --frames 780
```

The capture script writes 20 dark/light workspace, inspector, compact, settings,
tools, review, checks, mentions, images and crowded PNGs and element-tree JSON to
ignored `apps/desktop/preview-results/`. An optional output
directory argument is relative to the current directory. With FFmpeg installed,
replace the replay's `--at 9 --snap …` with `--video /path/to/replay.mp4 --duration auto`.
Rune's `--help` lists tooling options.

`--fixtures` supports editing/sending simulated replies, background streaming,
pane resizing, stopping and light/dark switching. `--evaluation-workload` adds
large histories, concurrent streams and a large PR. All fixture modes are isolated
from providers and saved data.

## Checks and measurements

`cargo test --locked -p caelum-desktop` drives native keyboard/pointer interactions,
including drafts, focus, approvals/questions, compact layout and bounded mounting.
`make check` also checks the shared Rust core and release scripts.

See [native measurements](rune-evaluation.md), [performance instructions](../performance/README.md),
[desktop development](../../apps/desktop/README.md) and [migration status](rune-migration.md).
