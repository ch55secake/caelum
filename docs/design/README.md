# Native workspace design

Caelum uses flush conversation surfaces, quiet sidebar selection, system-font
typography and a rounded composer. Rune supplies native text editing, animated
split panes, Markdown and windowed diffs. Provider/model/effort/agent controls live
in the composer; files and changes share the side inspector. Settings, session
actions and worktrees use focus-scoped dialogs. PR review occupies the main screen.

## Visual reference

The polished web-client captures remain the historical design reference. Native
captures use the actual live views with offline data from [`workspace.json`](workspace.json).
Screens are 1280 × 1000 at one pixel per point, except the 800 × 620 compact scene.

| | Historical polished reference | Native Caelum |
| --- | --- | --- |
| Dark workspace | ![Reference dark](previews/react-dark.png) | ![Native dark](previews/rune-dark.png) |
| Light inspector | ![Reference inspector](previews/react-light-inspector.png) | ![Native inspector](previews/rune-light-inspector.png) |

| Compact workspace | Settings |
| --- | --- |
| ![Native compact](previews/rune-dark-compact.png) | ![Native settings](previews/rune-dark-settings.png) |

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
```

The capture script writes dark/light, inspector, compact and settings PNGs and
element-tree JSON to ignored `apps/desktop/preview-results/`. An optional output
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
