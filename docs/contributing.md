# Contributing

Keep changes focused and follow the existing Rust modules. Application state,
provider behavior and filesystem/Git/GitHub operations belong in `caelum-core`;
native editors, focus and rendering belong in `apps/desktop`.

## Rust structure

The repository's [agent guidelines](../AGENTS.md) define module ownership,
state-machine and UI performance conventions. Aim for 800 non-test lines per
Rust file; 1,500 is the hard cap. Test files and inline test modules have a
1,000-line cap. Split by capability, with tests alongside the feature they cover.

`make structure` runs the shared
[rust-structure action](https://github.com/caelurium/rust-structure) against
`apps/` and `crates/`, including new non-ignored Rust files. It excludes inline
`#[cfg(test)]` items from production counts and measures tests separately. Folder
checks encourage capability modules: related prefixed siblings belong together,
and folders warn above 40 source files and fail above 60. Test folders and
standalone program directories are excluded.

Existing oversized files have explicit ceilings in
`scripts/structure/exceptions.txt`. New exceptions and increases fail against
the PR's base revision. Extract the area being changed before adding behaviour
to an over-cap file, in a separate structural PR. After reducing a violation,
run `make structure STRUCTURE_ARGS=--update-exceptions` to lower its ceiling or
remove it. The checker verifies these reductions too.

## Checks

On macOS, from the repository root:

```sh
make structure
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=4
python3 -m unittest discover -s scripts/release -p 'test_*.py'
python3 -m unittest discover -s scripts/profile -p 'test_*.py'
python3 -m unittest discover -s scripts/dev -p 'test_*.py'
python3 -m unittest discover -s apps/desktop/preview -p 'test_*.py'
```

On Linux, install the [desktop build prerequisites](linux.md) and run desktop
tests under `dbus-run-session`. `make check` does this automatically. Portable-only
checks can select `-p caelum-core -p caelum-protocol -p caelum-cli`.

The desktop test modules drive Rune's real runtime with keyboard/pointer
input and injected application events. It covers draft ownership, focus,
streaming, approvals/questions, bounded long histories and large reviews.
Core tests use fake provider processes and local HTTP servers for deterministic
protocol, recovery and lifecycle checks. Real-service smoke tests are opt-in;
see the [desktop development guide](../apps/desktop/README.md).

`nix develop -c python3 scripts/dev/smoke_native_workspace.py` exercises the
desktop transport and headless CLI over a pinned loopback SSH connection, using
a temporary Git checkout and provider fixtures. To stream a real provider turn
and recover the history after a worker restart and host-key change, pass
`--real-opencode /path/to/opencode` or `--real-codex /path/to/codex`. A loopback
Responses fixture supplies the model output and checks that reconnecting does
not ask for another answer. All modes use a temporary home.

To run Caelum Dev on fresh VMs before a release, stage the headless peers from
a compatible CI build of this checkout. Once both Linux AppImage jobs have
published their artifacts, run:

```sh
nix shell nixpkgs#squashfsTools -c python3 scripts/dev/stage_vm_peers.py RUN_ID FULL_COMMIT
nix develop -c python3 scripts/dev/macos_app.py --build-only
```

The signed Dev app carries both Linux architectures. On first connection it
uploads the matching binary over pinned SSH and checks its digest before use.
Subsequent connections reuse the verified copy. Release builds without bundled
peers use the matching checksum-verified release AppImage.

For visual changes, run `make previews` and inspect light/dark, inspector,
compact and settings scenes. Native frame measurements are described in
[performance](performance/README.md).

## CI layout

[`ci.yml`](../.github/workflows/ci.yml) is the single CI entry point for pull
requests. The release workflow calls it against the exact versioned release
commit and publishes its verified package artifacts. CodeQL analysis and PR
labeling have their own workflows.

| Area | Runner | Checks |
| --- | --- | --- |
| Shared source and tooling | `ubuntu-24.04` | `rust-structure`, `rust-format`, core/protocol/CLI `rust-lint` and `rust-tests`, Python tests and packaging shell syntax in `release-package` |
| macOS desktop | Apple Silicon `macos-15` | Desktop Clippy/tests, development app build and selected previews in `native-desktop-tests`; app/DMG/CLI packaging and lifecycle/update smoke checks in `desktop-bundle-build` |
| Linux desktop | Native `ubuntu-22.04` and `ubuntu-22.04-arm` | Desktop Clippy/tests in `linux-desktop-tests-*`; AppImage audits, X11/Wayland and update smoke checks in `linux-appimage-*` |

Every PR runs shared checks and desktop tests on all three native platforms.
Linux packaging also runs on every PR. macOS packaging runs when Rust, app,
packaging or CI/release workflow inputs change; native macOS previews run for
desktop UI, assets, fixture or CI workflow changes. Release calls always run
macOS packaging and both Linux architectures, except when recovering a historical
macOS-only tag. Native macOS previews remain PR-only artifacts.

[`linux-desktop.yml`](../.github/workflows/linux-desktop.yml) is called by CI,
rather than independently reacting to PRs. It also supports manual dispatch.
Its tests and packaging run concurrently on both architectures, sharing the
architecture matrix and dependency installation. Ubuntu 22.04 preserves the
AppImage's glibc 2.35 compatibility baseline.

Rust CI jobs upload `cargo-timings-*` artifacts for 14 days, including reports
available after a failed build. Open the timestamped HTML files to inspect each
Cargo invocation's compilation time and concurrency. Packaging and the macOS
development helper also leave these reports in `target/cargo-timings/` locally.
Compare matching revisions, runner architectures, profiles and cache-hit states;
job queue/setup time, test execution, preview capture and packaging smoke checks
are separate from Cargo's build timings.

Each Linux architecture has separate development/test and release dependency
caches, reused by PRs and main through the same workflow. Explicit shared keys
preserve the cache namespaces and give each cache one writer per run. See
[CI build caches](performance/ci-caches.md) for measurement guidance.

## Merging

The `main` ruleset requires an up-to-date PR branch and these seven checks:
`rust-tests`, `rust-lint`, `rust-format`, `desktop-bundle-build`,
`native-desktop-tests`, `rust-structure` and `release-package`. Their names remain
stable because the ruleset refers to them directly. Path-filtered macOS packaging
is reported as skipped when it is not needed.

Update your branch and rerun CI if another PR lands first. Linux checks run on
every PR but are not required by the current merge ruleset. Release publication
requires the entire reusable CI run to succeed, including Linux tests and
packaging for tags that ship Linux assets.

Use Conventional Commits and the repository's pull-request template.
