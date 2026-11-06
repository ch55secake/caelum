# Workspace polish and Rune comparison

The Tauri workspace uses a flush main surface, softer sidebar selection, roomier message typography and a rounded composer. Shared radius and motion tokens keep the controls consistent; colour transitions respect reduced motion.

The [Rune prototype](../../apps/rune-prototype/) explores the same workspace with native text editing, animated split panes and Rune's Markdown and diff components. Both previews load the conversation, command output, draft and patch from [`workspace.json`](workspace.json).

## Visual comparison

Captured on macOS at 1280 × 1000, with one output pixel per point. Rune uses bundled Inter and its own text/code rendering; the React preview uses WebKit and the desktop's font stack.

| | Polished Tauri / React | Rune prototype |
| --- | --- | --- |
| Dark workspace | ![React dark workspace](previews/react-dark.png) | ![Rune dark workspace](previews/rune-dark.png) |
| Light changes inspector | ![React light changes inspector](previews/react-light-inspector.png) | ![Rune light changes inspector](previews/rune-light-inspector.png) |

Both renderers use a flush conversation surface and quieter controls. Rune's toolbar is more compact, its inspector sits beside the conversation, and its diff view has interactive line selection and unified/split modes. Use the running apps to judge typing, scrolling and pane motion.

## Motion in Tauri

The production workspace now coordinates sidebar and inspector opening, closing and expansion with a 220 ms, non-overshooting settle and shorter content fades. The browser's Web Animations API drives the pane slots; interrupted toggles continue from the current width, and pointer/keyboard resizing remains immediate. No animation dependency is needed.

Closing panes become inert immediately and stay mounted through their exit. The conversation remains mounted during inspector expansion, preserving drafts, reading positions and live-output following. Reduced motion snaps the layout, including when the preference changes during an animation. The macOS titlebar spacing follows the same timing.

## Run the prototype

From the repository root, with Rust 1.95+ and the macOS development tools available:

```sh
cargo run --locked --manifest-path apps/rune-prototype/Cargo.toml
```

With the repository's development shell:

```sh
nix develop -c cargo run --locked --manifest-path apps/rune-prototype/Cargo.toml
```

The first run builds Rune's rendering dependencies. The experiment has its own Cargo workspace and lockfile, with `rune` and `rune-kit` pinned to `ddf14140253bd1b963993895e3fa025d1ec58537`. It is opt-in; normal Caelum builds use the root workspace.

### Try it

- Edit the draft, paste text, select/copy a response, and use Shift+Enter for a newline. Enter sends a simulated reply; the stop button or Escape cancels it.
- Switch sessions during a reply. Drafts and streaming belong to their session. New session creates another editable conversation.
- Scroll upward during streaming to keep an older reading position; scroll back to the bottom to follow again.
- Drag a pane's inner edge to resize it. Double-click the edge to reset its width, or drag it inward to collapse. The toolbar buttons restore panes.
- Expand the sample command, open Changes, select diff lines and switch between unified and split views.
- Toggle light/dark appearance with the half-filled circle in the toolbar. Pane motion and text reveal follow the system's reduced-motion preference.

| macOS shortcut | Action |
| --- | --- |
| Cmd+N | New session |
| Cmd+L | Focus composer |
| Shift+Cmd+B | Toggle sidebar |
| Option+Cmd+I | Toggle changes inspector |

The prototype uses local fixtures and simulated replies. The patch is read-only, and sessions, drafts, appearance and pane sizes live only for the current run. Provider integration, saved history and the rest of Caelum's production features belong to the Tauri app. Native launch and rendering have been exercised on macOS; other platforms are unverified.

## Reproduce the previews

Install the desktop dependencies and Playwright WebKit as described in [contributing](../contributing.md). Run these from the repository root inside `nix develop`, or with Node.js, pnpm and Cargo on PATH:

```sh
pnpm --filter @caelum/desktop preview:workspace
node apps/rune-prototype/preview/capture.mjs
```

Both commands write into the ignored `apps/desktop/preview-results/` directory: light/dark workspaces, inspectors and 800 × 620 compact layouts. The React command also records `react-pane-motion.webm`, a WebKit replay of pane toggles and expansion in both themes with macOS titlebar spacing. Rune also writes element-tree JSON. Its compact scene starts with the sidebar collapsed; the React compact capture keeps all three panes visible. Set `CAELUM_PREVIEW_DIR` for the React command (relative to `apps/desktop`), or pass an output directory to the Rune capture script (relative to the current directory).

Replay editing, session switching, background streaming, resizing and theme changes:

```sh
cargo run --locked --manifest-path apps/rune-prototype/Cargo.toml -- \
  --script apps/rune-prototype/preview/interaction.json \
  --theme dark --scale 1 --at 9 \
  --snap apps/desktop/preview-results/rune-stream.png
```

With FFmpeg on PATH, replace `--at 9 --snap ...` with `--video apps/desktop/preview-results/rune-interaction.mp4 --duration auto` to record the full interaction. Rune's `--help` lists the headless controls.

## Prototype checks

The independent workspace needs explicit commands:

```sh
cargo fmt --manifest-path apps/rune-prototype/Cargo.toml -- --check
cargo clippy --locked --manifest-path apps/rune-prototype/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path apps/rune-prototype/Cargo.toml
```

The interaction tests drive Rune's runtime through keyboard and pointer input, including draft isolation, background streaming, stopping, clipboard copy, pane resizing, compact controls and scroll-follow behaviour.
