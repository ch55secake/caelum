use std::{cmp::Ordering, io::Read, time::Duration};

use caelum_protocol::{UpdateCheck, UpdateStatus};
use reqwest::{blocking::Client, StatusCode};
use semver::Version;
use serde_json::Value;

pub const REPOSITORY_URL: &str = "https://github.com/ch55secake/caelum";
pub const RELEASES_URL: &str = "https://github.com/ch55secake/caelum/releases";
pub const LICENSE_URL: &str = "https://github.com/ch55secake/caelum/blob/main/LICENSE";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/ch55secake/caelum/releases/latest";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

/// Manual, unauthenticated release discovery. Downloads open in the user's
/// browser; this command neither installs updates nor starts a background poll.
pub fn check_for_updates(current_version: &str) -> Result<UpdateCheck, String> {
    let client = Client::builder()
        .user_agent(concat!("Caelum/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())?;
    let release = fetch_release(&client, LATEST_RELEASE_API)?;
    compare_release(
        &release,
        current_version,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

fn fetch_release(client: &Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                "The update check timed out. Try again.".to_owned()
            } else {
                "Could not reach GitHub. Check your connection and try again.".to_owned()
            }
        })?;
    match response.status() {
        StatusCode::NOT_FOUND => return Err("No stable release is published yet.".to_owned()),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
            return Err("GitHub is limiting update checks. Try again later.".to_owned())
        }
        status if !status.is_success() => {
            return Err(format!(
                "GitHub returned HTTP {}. Try again.",
                status.as_u16()
            ))
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read GitHub’s response. Try again.".to_owned())?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("GitHub’s release response was too large.".to_owned());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "GitHub returned an invalid release response. Try again.".to_owned())
}

fn compare_release(
    release: &Value,
    current_version: &str,
    platform: &str,
    architecture: &str,
) -> Result<UpdateCheck, String> {
    let invalid = || "GitHub did not return a valid stable release.".to_owned();
    let tag = release["tag_name"].as_str().ok_or_else(invalid)?;
    let latest = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| invalid())?;
    if release["draft"] != false || release["prerelease"] != false || !latest.pre.is_empty() {
        return Err(invalid());
    }
    let current = Version::parse(current_version)
        .map_err(|_| "The installed application version is not a semantic version.".to_owned())?;
    let status = match current.cmp_precedence(&latest) {
        Ordering::Less => UpdateStatus::Available,
        Ordering::Equal => UpdateStatus::UpToDate,
        Ordering::Greater => UpdateStatus::Ahead,
    };
    let download_url = (platform == "macos" && matches!(architecture, "aarch64" | "x86_64"))
        .then(|| {
            let name = format!("Caelum-v{latest}-{architecture}.dmg");
            let url = format!("{RELEASES_URL}/download/{tag}/{name}");
            release["assets"].as_array()?.iter().find(|asset| {
                asset["name"] == name
                    && asset["state"] == "uploaded"
                    && asset["browser_download_url"] == url
            })?;
            Some(url)
        })
        .flatten();
    Ok(UpdateCheck {
        current_version: current_version.to_owned(),
        latest_version: latest.to_string(),
        status,
        release_url: format!("{RELEASES_URL}/tag/{tag}"),
        download_url,
    })
}

#[cfg(test)]
mod tests;
