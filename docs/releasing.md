# Releasing Caelum

Merges to `main` trigger automatic patch releases for Apple Silicon and Intel Macs. A deliberate version change uses that version instead. The release contains two DMGs, two standalone CLI archives, and `SHA256SUMS`.

## Signing without an Apple Developer account

Release builds use **ad-hoc signing** (`codesign --sign -`) for the desktop app and CLI. No Apple account, certificate, signing secret, or notarization credential is required. Ad-hoc signatures verify bundle integrity but do not identify an Apple-approved developer; the app is **not notarized**.

After attempting to open the installed app, macOS may require **System Settings → Privacy & Security → Open Anyway**. See [Apple's first-launch instructions](https://support.apple.com/en-us/102445). Release notes include this information and links for each architecture.

## Release flow

1. Prepare an immutable version tag. Ordinary merges create a `chore(release)` version commit updating the Cargo workspace and local lockfile packages. Every crate inherits the workspace version; native bundle metadata is generated from it.
2. Run shared CI against that exact release commit: Rust formatting, Clippy, core/CLI/native interaction tests, native previews and Python release/profile checks. The architecture-specific release builds run alongside these checks.
3. Build ad-hoc-signed app bundles/DMGs and CLI archives on native Apple Silicon and Intel runners with locked dependencies.
4. Verify signatures, architectures, app metadata, extracted CLI startup and mounted DMG contents. Exercise packaged CLI cold/warm URLs, hidden-window reopening, draft restoration and graceful quit in an isolated workspace.
5. Generate checksums, upload into a **draft** GitHub release, and verify the complete uploaded asset set before publication. Publication requires both CI and both architecture builds to succeed.
6. When configured, open a separate Homebrew tap update PR.

Main release runs are serialized. GitHub retains only the newest pending run when merges arrive faster than releases finish, so overlapping untagged merges can share a release. A superseded preparation run yields to the newer main workflow rather than silently switching its source tree. Once a version commit/tag exists, retries reuse it. Tags are never moved.

## Automatic version commit setup

The `main` ruleset requires pull requests and status checks. A dedicated GitHub App installation token permits the automated version commit:

1. Create a private GitHub App with **Contents: read and write** permission and install it on `ch55secake/caelum`. Webhooks are unnecessary.
2. Store its app ID and private key in Actions secrets `RELEASE_APP_ID` and `RELEASE_APP_PRIVATE_KEY`.
3. Add the app to the `main` ruleset bypass list with **Always allow**.

The workflow limits the installation token to this repository. It uses the normal Actions token for tags to avoid a second tag-triggered release. The bot's main push runs the usual CI and its duplicate Release run exits without another bump. Manual tag recovery uses the Actions token and does not require the app's credentials.

## Publish and recover

Ordinary releases only need a merge to `main`. For a minor/major release, update the root `Cargo.toml` workspace version and refresh `Cargo.lock` in a PR. Direct pushes of matching `vMAJOR.MINOR.PATCH` or prerelease tags are also supported.

For a failed build, partial upload, or missed Homebrew update, run **Actions → Release → Run workflow**, choose the default branch, and supply the **existing** version tag. From the CLI:

```sh
gh workflow run release.yml --ref main -f tag=v0.1.5
```

Recovery validates the remote tag and workspace/lockfile versions before using it. Unpublished native releases rebuild with the tagged packaging scripts and current CI; publication uses current recovery tooling. A partial draft can have its uploads replaced and verified before publication. Rebuilding an unpublished pre-cutover web-client tag requires that historical release workflow; already published legacy assets remain recoverable without rebuilding.

Already published releases are verified and reused without rebuilding or replacing assets or checksums; recovery can then retry the Homebrew update. An incomplete or checksum-mismatched published release fails visibly. Publish a new version to correct released binaries. Stable releases use GitHub's semantic-version-aware latest selection; prereleases are marked separately. Intermediate build artifacts are retained for 14 days.

## Homebrew setup

The renderer and templates are already merged in [`ch55secake/homebrew-tap`](https://github.com/ch55secake/homebrew-tap). To enable automated update PRs:

1. Create a fine-grained GitHub token limited to `ch55secake/homebrew-tap` with **Contents: read and write** and **Pull requests: read and write** permissions.
2. Add it directly to this repository's Actions secrets as `HOMEBREW_TAP_TOKEN`.
3. Set the Actions repository variable `HOMEBREW_TAP_UPDATES` to `enabled`.
4. Recover the latest published tag to generate its tap update, or wait for the next release.

Merge the generated tap PR to make the version installable. The formula installs the CLI as `caelum`; the cask installs `Caelum.app` without a second executable link. Homebrew installation has the same first-launch behavior as the DMG. Prereleases do not update the stable tap.

## Local native packaging

```sh
nix develop -c bash scripts/release/package_macos_app.sh
TARGET=aarch64-apple-darwin ARCH=aarch64 RELEASE_TAG=v0.1.13 \
RUNNER_TEMP=/absolute/path/to/output \
nix develop -c bash scripts/release/package_macos_assets.sh
```

Use the current Cargo version and a pre-created output directory. Intel builds
use `TARGET=x86_64-apple-darwin ARCH=x86_64` on an Intel host. The app target is
macOS 13; native packaging retains `Caelum.app`, `dev.caelum.desktop`, the icon,
version/revision metadata and `caelum://open?path=…` URL registration. It rejects
non-system dynamic libraries after normalizing the known Nix libiconv dependency.

The asset script builds both binaries, signs them, creates a compressed DMG,
mounts/verifies it and checks extracted CLI startup. Release publication produces:

- `Caelum-v<version>-aarch64.dmg` and `Caelum-v<version>-x86_64.dmg`
- `caelum-cli-v<version>-<target>.tar.gz` for both macOS targets
- `SHA256SUMS`

## Install

Once a Caelum package PR has been merged into the tap:

```sh
brew install --cask ch55secake/tap/caelum
brew install ch55secake/tap/caelum-cli
```

For direct installation, download the matching DMG from the GitHub release and drag **Caelum.app** into **Applications**. Extract the matching CLI archive and place `caelum` on your `PATH`. Use `aarch64` for Apple Silicon (M-series) and `x86_64` for Intel.

Download `SHA256SUMS` beside the selected asset and verify it with:

```sh
shasum -a 256 --ignore-missing -c SHA256SUMS
```
