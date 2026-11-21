use super::{github_command, pull_request_url, Gh, GitHubError};
use serde_json::Value;
use std::{path::Path, time::Duration};

/// Accept a repository move only when GitHub resolves the requested repository
/// to the returned one. A matching PR number alone cannot establish identity.
pub(super) fn check<'a>(
    executable: &(impl Gh + ?Sized),
    project: &Path,
    requested: &str,
    returned: Option<&'a str>,
    timeout: Duration,
) -> Result<&'a str, GitHubError> {
    let different = || GitHubError::Command("GitHub returned a different pull request".into());
    let returned = returned.ok_or_else(different)?;
    let original = pull_request_url(requested)?;
    let mut canonical = pull_request_url(returned).map_err(|_| different())?;
    if original == canonical {
        return Ok(returned);
    }
    let original_path: Vec<_> = original.path_segments().unwrap().collect();
    let canonical_path: Vec<_> = canonical.path_segments().unwrap().collect();
    if original.origin() != canonical.origin() || original_path[3] != canonical_path[3] {
        return Err(different());
    }
    let repository_path = format!("/{}/{}", canonical_path[0], canonical_path[1]);
    canonical.set_path(&repository_path);
    let mut command = github_command(executable, project);
    command.args([
        "api",
        "--hostname",
        original.host_str().unwrap(),
        "--method",
        "GET",
        &format!("repos/{}/{}", original_path[0], original_path[1]),
    ]);
    let output = executable.output(command, timeout)?;
    let repository: Value = serde_json::from_slice(&output)?;
    let confirmed = repository["html_url"]
        .as_str()
        .and_then(|url| reqwest::Url::parse(url).ok());
    if confirmed.as_ref() != Some(&canonical) {
        return Err(different());
    }
    Ok(returned)
}
