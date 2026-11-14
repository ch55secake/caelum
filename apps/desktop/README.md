# Caelum native desktop

The shipping macOS application, built with Rune and backed by
`caelum-core::application`. See the [desktop guide](../../docs/desktop.md) for workflows.

## Run and build

From the repository root with Rust 1.95.0 and Xcode Command Line Tools:

```sh
make dev PROJECT=/path/to/project
cargo test --locked --workspace -- --test-threads=4
cargo clippy --locked --workspace --all-targets -- -D warnings
bash scripts/release/package_macos_app.sh
```

Prefix commands with `nix develop -c` to use the development shell. All crates
share the root lockfile. Packaging produces an ad-hoc-signed `Caelum.app` at
`target/<triple>/release/bundle/macos/`, preserving `dev.caelum.desktop` and the
`caelum` project URL scheme. [Release tooling](../../docs/releasing.md) creates
architecture-specific DMGs and CLI archives.

`make dev` builds and launches `target/debug/bundle/macos/Caelum Dev.app`, with
terminal output and environment variables preserved. The signed debug bundle has
a stable notification identity for each checkout, distinct from the release app.
It does not register the release app's `caelum` URL scheme. macOS asks to allow
notifications when the first one is sent; clicking one opens the originating
session in that same app. Bare `cargo run` remains useful for offline fixtures;
native notifications require the app bundle.

Install/authenticate Codex or OpenCode for live sessions and `gh` for GitHub.
⌘O opens projects; `caelum /path` also reaches a running or hidden app. The sidebar's
project **×** closes its checkout group and retains sessions, drafts and running work.
Compact Projects and rich Sessions sections stay visible and scroll independently.
The session feed spans all open projects, with a project filter and a saved
Appearance → Sidebar scope setting. Closing
the window keeps work running; ⌘Q shuts down owned providers and saves editors.

Fresh launches open **Home**, also available from the sidebar or Caelum logo.
Home shows the black-hole artwork in dark mode and pale sunlit clouds in light mode.
Its draft and attachments are saved independently of conversations. Choose a
project/worktree, provider and model, then send to start a fresh session. Explicit
project links still open their destination. Use `--design-fixture --scene home`
(or `home-empty`, `home-draft`, `home-projects`) to preview the Home layouts.

## Offline development

Append one of these modes to `cargo run --locked -p caelum-desktop --`:

- `--fixtures`: original interactive sample with simulated replies.
- `--design-fixture`: reference content through the real live views.
- `--evaluation-workload`: six streaming sessions with 2,000 history entries each
  and a 300-file synthetic PR.

These modes do not start providers or write workspace state. Capture previews
with `python3 apps/desktop/preview/capture.py`; see [design](../../docs/design/README.md)
and [performance](../../docs/performance/README.md) for headless interaction replay
and native frame-measurement commands. Pinned Rune's `--native` mode does not
dispatch `--script` events; check OS interaction in the running application.
The 360 light/dark, normal/compact captures include populated, empty, loading,
error, hover, focus and disabled states. Use `--scene models` (or `worktrees`,
`shells`, `review-loading`, `composer-options`, `files-tabs`, `tools-collapsed`,
`merge-ready`, `merge-blocked`, `shells-collapsed`, `vercel`, `vercel-settings`, `sidebar-projects`, `crowded`) with `--design-fixture`
to inspect one.

## State and migration

The production snapshot is `~/.caelum/workspace-v1.json`, with `branches.sqlite3`
beside it. `CAELUM_STATE` overrides the path (`CAELUM_RUNE_STATE` is also accepted).
Explicit paths skip migration. Use a separate directory for isolated live runs.

First launch prefers the previous native candidate's snapshot and branch DB under
`~/.caelum/rune/`; otherwise it imports the legacy workspace SQLite database.
Sources are preserved. Invalid/future data blocks loading/saving. Exported
`caelum.settings.v1` JSON remains importable through Settings. Full paths and
recovery semantics are in the [migration record](../../docs/design/rune-migration.md).

## Opt-in service checks

The discovery checks require existing provider history and send no prompts:

```sh
CAELUM_STATE=/path/to/isolated/workspace.json \
CAELUM_RUNE_SMOKE_PROJECT=/path/to/project \
CAELUM_RUNE_SMOKE_PROVIDER=opencode \
cargo test --locked -p caelum-desktop real_provider_discovery_and_history_smoke -- --ignored --nocapture

CAELUM_RUNE_SMOKE_PROJECT=/path/to/caelum \
CAELUM_RUNE_SMOKE_PR=https://github.com/ch55secake/caelum/pull/95 \
cargo test --locked -p caelum-desktop real_workspace_services_smoke -- --ignored --nocapture

# Starts, reads, stops and cleans up an isolated background command, without inference.
cargo test --locked -p caelum-core real_background_shell_lifecycle_smoke -- --ignored --nocapture
```

Use `codex` for the other provider. These explicit test environment variables
retain their original names for compatibility. See the [evaluation evidence](../../docs/design/rune-evaluation.md)
and [remaining workflow gaps](../../docs/design/rune-migration.md).
