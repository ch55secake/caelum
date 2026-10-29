# Releasing Caelum

Every merge to `main` creates a patch release: the workflow commits the next patch version, tags it, builds Apple Silicon and Intel macOS app DMGs and standalone CLI archives, and publishes a GitHub release with SHA-256 checksums. A merge that already contains an explicit version change uses that version instead. Pushing a matching version tag directly is also supported.

The app builds are currently unsigned and not notarized. macOS may warn users or block opening the downloaded app. No Apple Developer account or signing secrets are needed for these builds.

## Automatic version commit setup

The `main` ruleset requires pull requests and status checks. The release workflow uses a dedicated GitHub App installation token to bypass those rules only for its automated version commit.

1. Create a private GitHub App under the `ch55secake` account with **Contents: read and write** repository permissions. Disable webhooks; no events are needed.
2. Install the app on `ch55secake/caelum` only, then generate a private key.
3. Add the app ID and private key as repository Actions secrets named `RELEASE_APP_ID` and `RELEASE_APP_PRIVATE_KEY`.
4. Add the installed app to the `main` ruleset's bypass list with **Always allow**.

The release workflow uses the app token for the version commit and the normal Actions token for the release tag, avoiding a duplicate release run.

## Optional Homebrew setup

To have releases automatically open a pull request in `ch55secake/homebrew-tap`:

1. Merge the [tap-side Caelum packaging PR](https://github.com/ch55secake/homebrew-tap/pull/1). The release workflow checks out the tap's default branch and runs its renderer.
2. Create a fine-grained GitHub token with **Contents: read and write** and **Pull requests: read and write** access to `ch55secake/homebrew-tap`.
3. Store it in this repository as `HOMEBREW_TAP_TOKEN`.
4. Set the Actions repository variable `HOMEBREW_TAP_UPDATES` to `enabled`.

The workflow opens a tap PR; it does not merge it. The formula installs the CLI as `caelum`, while the cask installs `Caelum.app` without linking a second executable.

## Publish a release

1. For a deliberate minor or major release, update the version in the Cargo workspace, `apps/desktop/src-tauri/tauri.conf.json`, and `apps/desktop/package.json` to the same `MAJOR.MINOR.PATCH` value in a pull request, and refresh `Cargo.lock`. Ordinary merges do not need a version edit.
2. Merge to `main`. If the merge has no version change, the workflow commits the next patch version to `main`. If it contains an explicit version bump, that version is used. The workflow creates an annotated `vMAJOR.MINOR.PATCH` tag and runs the release build and publish jobs.

Each main merge triggers an automatic patch release. Directly pushing a matching version tag remains available for manual releases. The resulting release contains an unsigned DMG for each architecture, a standalone CLI archive for each architecture, and `SHA256SUMS`. If Homebrew updates are enabled, the workflow opens a separate pull request with the updated formula, cask, and tap README.

## Install

After the tap update PR is merged, install the desktop app and CLI independently:

```sh
brew install --cask ch55secake/tap/caelum
brew install ch55secake/tap/caelum-cli
```

Alternatively, download the matching DMG from the GitHub release to install the app, or download and extract the CLI archive for the machine's architecture.
