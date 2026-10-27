# Releasing Caelum

Releases are created when a version bump is merged to `main`. The workflow creates the matching version tag and builds Apple Silicon and Intel macOS app DMGs and standalone CLI archives, publishes a GitHub release with SHA-256 checksums, and can open a pull request updating the Homebrew tap. Pushing a matching version tag directly is also supported.

The app builds are currently unsigned and not notarized. macOS may warn users or block opening the downloaded app. No Apple Developer account or signing secrets are needed for these builds.

## Optional Homebrew setup

To have releases automatically open a pull request in `ch55secake/homebrew-tap`:

1. Merge the [tap-side Caelum packaging PR](https://github.com/ch55secake/homebrew-tap/pull/1). The release workflow checks out the tap's default branch and runs its renderer.
2. Create a fine-grained GitHub token with **Contents: read and write** and **Pull requests: read and write** access to `ch55secake/homebrew-tap`.
3. Store it in this repository as `HOMEBREW_TAP_TOKEN`.
4. Set the Actions repository variable `HOMEBREW_TAP_UPDATES` to `enabled`.

The workflow opens a tap PR; it does not merge it. The formula installs the CLI as `caelum`, while the cask installs `Caelum.app` without linking a second executable.

## Publish a release

1. Update the version in the Cargo workspace, `apps/desktop/src-tauri/tauri.conf.json`, and `apps/desktop/package.json` to the same `MAJOR.MINOR.PATCH` value in a pull request, and refresh `Cargo.lock`.
2. Merge the version bump to `main`. The release workflow detects the change, creates an annotated `vMAJOR.MINOR.PATCH` tag on the merge commit, and runs the release build and publish jobs.

Only version changes trigger automatic releases; unrelated merges do not create duplicate tags. Directly pushing a matching version tag remains available for manual releases. The resulting release contains an unsigned DMG for each architecture, a standalone CLI archive for each architecture, and `SHA256SUMS`. If Homebrew updates are enabled, the workflow opens a separate pull request with the updated formula, cask, and tap README.

## Install

After the tap update PR is merged, install the desktop app and CLI independently:

```sh
brew install --cask ch55secake/tap/caelum
brew install ch55secake/tap/caelum-cli
```

Alternatively, download the matching DMG from the GitHub release to install the app, or download and extract the CLI archive for the machine's architecture.
