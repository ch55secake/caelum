# Releasing Caelum

Releases are created by pushing a version tag that matches both the Cargo workspace and Tauri app versions. The workflow builds Apple Silicon and Intel macOS assets, signs and notarizes the desktop app, publishes a GitHub release with SHA-256 checksums, and can open a pull request updating the Homebrew tap.

## One-time GitHub setup

Add these repository Actions secrets before the first release:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Apple **Developer ID Application** `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` certificate |
| `APPLE_SIGNING_IDENTITY` | Full Developer ID Application identity from `security find-identity -v -p codesigning` |
| `APPLE_ID` | Apple ID used for notarization |
| `APPLE_PASSWORD` | App-specific password for that Apple ID |
| `APPLE_TEAM_ID` | Apple Developer team ID |

Export the Developer ID Application certificate from Keychain Access as a password-protected `.p12`, then encode it without line breaks:

```sh
openssl base64 -A -in certificate.p12 -out certificate-base64.txt
gh secret set APPLE_CERTIFICATE < certificate-base64.txt
```

Set the remaining secrets with `gh secret set SECRET_NAME`; enter secret values locally and never commit them. A paid Apple Developer account is required for notarization.

To have releases automatically open a pull request in `ch55secake/homebrew-tap`, also:

1. Merge the [tap-side Caelum packaging PR](https://github.com/ch55secake/homebrew-tap/pull/1). The release workflow checks out the tap's default branch and runs its renderer.
2. Create a fine-grained GitHub token with **Contents: read and write** and **Pull requests: read and write** access to `ch55secake/homebrew-tap`.
3. Store it in this repository as `HOMEBREW_TAP_TOKEN`.
4. Set the Actions repository variable `HOMEBREW_TAP_UPDATES` to `enabled`.

The workflow opens a tap PR; it does not merge it. The formula installs the CLI as `caelum`, while the cask installs `Caelum.app` without linking a second executable.

## Publish a release

1. Update the version in the Cargo workspace, `apps/desktop/src-tauri/tauri.conf.json`, and `apps/desktop/package.json` to the same `MAJOR.MINOR.PATCH` value.
2. Merge the changes to `main` and push the matching tag:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The tag triggers native arm64 and Intel builds. The resulting release contains a signed/notarized DMG for each architecture, a standalone CLI archive for each architecture, and `SHA256SUMS`. If Homebrew updates are enabled, the workflow opens a separate pull request with the updated formula, cask, and tap README.

## Install

After the tap update PR is merged, install the desktop app and CLI independently:

```sh
brew install --cask ch55secake/tap/caelum
brew install ch55secake/tap/caelum-cli
```

Alternatively, download the matching DMG from the GitHub release to install the app, or download and extract the CLI archive for the machine's architecture.
