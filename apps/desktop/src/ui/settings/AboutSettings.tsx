import { Orbit, RefreshCw } from "lucide-react";
import type { useAbout } from "../../application/useAbout.js";
import { CopyButton } from "../conversation/MessageContent.js";
import { ExternalLink } from "../primitives/ExternalLink.js";

const platformName = (value: string) =>
  ({ macos: "macOS", windows: "Windows", linux: "Linux" })[value] ?? value;
const architectureName = (value: string) =>
  ({ aarch64: "Apple silicon (arm64)", x86_64: "Intel (x86_64)" })[value] ??
  value;

export function AboutSettings({
  about,
}: {
  about: ReturnType<typeof useAbout>;
}) {
  const { info, infoError, checking, update, updateError } = about;
  return (
    <>
      <div className="about-heading">
        <Orbit size={32} />
        <div>
          <h2>Caelum</h2>
          <p>A native home for AI coding agents</p>
        </div>
      </div>
      {!info && !infoError && <p role="status">Loading application details…</p>}
      {infoError && (
        <div className="settings-provider-error" role="alert">
          <p>Could not load application details: {infoError}</p>
          <button onClick={about.retryInfo}>Retry</button>
        </div>
      )}
      {info && (
        <>
          <dl className="about-details">
            <dt>Version</dt>
            <dd>{info.version}</dd>
            <dt>Platform</dt>
            <dd>{platformName(info.platform)}</dd>
            <dt>Architecture</dt>
            <dd>
              {info.platform === "macos"
                ? architectureName(info.architecture)
                : info.architecture}
            </dd>
            <dt>License</dt>
            <dd>{info.license}</dd>
          </dl>
          <div className="about-copy">
            <CopyButton
              label="Copy application details"
              text={`${info.name} ${info.version}\nPlatform: ${info.platform}\nArchitecture: ${info.architecture}\n${info.repositoryUrl}`}
            />
            <span>Copy application details</span>
          </div>
          <nav className="about-links" aria-label="Application links">
            <ExternalLink href={info.repositoryUrl}>Repository</ExternalLink>
            <ExternalLink href={info.releasesUrl}>Releases</ExternalLink>
            <ExternalLink href={info.licenseUrl}>License</ExternalLink>
          </nav>
          <section className="about-updates" aria-labelledby="updates-title">
            <h3 id="updates-title">Updates</h3>
            <p className="settings-note">
              Check GitHub for the latest stable release.
            </p>
            <button
              className="settings-refresh"
              disabled={checking}
              onClick={() => void about.check()}
            >
              <RefreshCw size={14} className={checking ? "spinning" : ""} />
              {checking
                ? "Checking…"
                : updateError
                  ? "Retry update check"
                  : "Check for updates"}
            </button>
            {checking && (
              <p role="status" className="settings-note">
                Checking for updates…
              </p>
            )}
            {updateError && (
              <p className="settings-provider-error" role="alert">
                {updateError}
              </p>
            )}
            {update && (
              <div className="update-result">
                <p role="status">
                  {update.status === "available"
                    ? `Caelum ${update.latestVersion} is available.`
                    : update.status === "ahead"
                      ? `You’re running ${update.currentVersion}, ahead of the latest stable release (${update.latestVersion}).`
                      : `You’re up to date — Caelum ${update.currentVersion}.`}
                </p>
                <div className="about-links">
                  <ExternalLink href={update.releaseUrl}>
                    Release notes
                  </ExternalLink>
                  {update.status === "available" && (
                    <ExternalLink
                      href={update.downloadUrl ?? `${update.releaseUrl}#assets`}
                    >
                      {update.downloadUrl
                        ? "Download update"
                        : "View downloads"}
                    </ExternalLink>
                  )}
                </div>
                <p className="settings-note">
                  Last checked{" "}
                  {update.checkedAt.toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </p>
              </div>
            )}
          </section>
        </>
      )}
    </>
  );
}
