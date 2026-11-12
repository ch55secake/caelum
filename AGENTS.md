# Caelum

A native Rust desktop and CLI for Codex and OpenCode sessions. See
`docs/architecture.md` for ownership and `docs/contributing.md` for checks.

## Layout and ownership

- `crates/caelum-core`: renderer-independent application state, providers,
  persistence, filesystem, Git and GitHub operations.
- `crates/caelum-protocol`: serializable interface and provider contracts.
- `apps/desktop`: Rune views, native editors, focus, dialogs and app lifecycle.
- `apps/cli`: terminal workflows using the shared core.
- `scripts/`: structure checks, performance tooling and release packaging.
- A feature owns its state, events, keys and registrations. Keep shared workspace
  structs focused on coordination; put new subsystem state with its owner.
- Keep `caelum-core` independent of Rune and desktop rendering.

## Rust modules

- Aim for **800 non-test lines per Rust file**; **1,500 is the hard cap**.
  Inline `#[cfg(test)]` items are excluded from production counts.
- Put tests in sibling `tests.rs`, `*_tests.rs` or capability-specific `tests/`
  modules. Keep each test file or inline test module under **1,000 lines**.
- Existing violations are recorded in `scripts/structure/exceptions.txt`.
  Ceilings only decrease. After shrinking a file, run
  `python3 scripts/structure/check.py --update-exceptions`.
- Before adding behaviour to an over-cap file, extract the area being changed
  into a focused module in a separate structural PR.
- Split by capability and responsibility. A base `x.rs` or `x/` with two or more
  `x_*.rs` siblings, or three such siblings alone, belongs in `x/`.
  A folder over 40 source files warns; over 60 fails. Tests and standalone
  programs under `bin/`, `examples/` and `benches/` are excluded.
- Keep module declarations and re-exports sorted, one item per line.
- Keep visibility narrow. Prefer private or `pub(super)` implementation details
  over broadening the public API to accommodate a move.

## State and asynchronous work

- Model mutually exclusive lifecycle phases as enums carrying the data valid in
  each phase. Prefer explicit state/event transitions returning effects over
  combinations of booleans, optional values and timestamps.
- Route effects through the existing core command/event runtime. Desktop-only
  effects belong in Rune handlers; core state transitions remain testable without UI.
- Preserve project/provider/session ownership and request generations. Late
  results and timer events for an obsolete attempt must not change newer state.
- Treat timers as events with a recorded deadline and attempt identity. Use the
  subsystem clock and schedule a wake rather than repeatedly polling an idle loop.

## UI and performance

- Keep blocking filesystem, network, process and persistence work off the UI thread.
  Start nonessential launch work after the first frame.
- Return changed from polling only when visible state changes. Stop requesting
  frames when work and animation finish.
- Build mounted rows through virtualization and use memoization where appropriate.
  Use the layout/live-value APIs available in the pinned Rune revision.
- Bound caches by entries and bytes, including decoded image memory.
- Preserve native editor identity, selection, undo history and session-owned scroll.
- Use `docs/performance/README.md` for measurements and report the workload and
  what was measured. Keep diagnostics idle unless there is work to record.

## Commands and verification

- `nix develop -c make check`: structure, formatting, Clippy, Rust and Python tests.
- `make structure`: file/folder limits and structure-checker regression tests.
- `make dev PROJECT=.`: run the desktop. `make cli`: build the standalone CLI.
- `make previews`: native light/dark references for visual changes.
- Use the most relevant existing checks while iterating and complete `make check`
  before reporting a change as finished. Native desktop checks require macOS;
  portable core, protocol and CLI checks also run on Linux.
- Keep structural changes behaviour-preserving and verify existing test coverage
  is still registered. Inspect native previews when rendering/layout changes.

## Contributions

- Keep changes focused and preserve unrelated work. Use Conventional Commits.
- Create a PR for completed work and follow `.github/PULL_REQUEST_TEMPLATE.md`.
  Keep validation results in the user-facing summary rather than the PR body.
- Public examples and committed artifacts use repo-relative paths, synthetic
  identifiers and example hosts. Keep credentials, private session content and
  machine-specific diagnostics out of commits; commit references that docs use.

The size, capability ownership and state-transition guidelines are adapted from
[Rune's agent guidelines](https://github.com/anomalyco/rune/blob/main/AGENTS.md).
