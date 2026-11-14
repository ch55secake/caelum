use super::*;
use github::cache::{timestamp, Cache};

pub(super) fn cache(runtime: &ProviderRuntime) -> Option<Cache> {
    runtime
        .review_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub(super) fn details(
    runtime: &ProviderRuntime,
    project: &str,
    url: &str,
) -> Result<GitHubPullRequestDetails, String> {
    let cache = cache(runtime);
    let observed = timestamp();
    let details = github::pull_request_details(project, url).map_err(|e| e.to_string())?;
    if let Some(cache) = cache {
        cache.save_details(&details, observed);
    }
    Ok(details)
}

pub(super) fn files(
    runtime: &ProviderRuntime,
    project: &str,
    url: &str,
    head: &str,
    base: &str,
    page: u32,
) -> Result<GitHubPullRequestFiles, String> {
    let cache = cache(runtime);
    if let Some(files) = cache
        .as_ref()
        .and_then(|cache| cache.page(url, head, base, page))
    {
        return Ok(files);
    }
    let files =
        github::pull_request_files(project, url, head, base, page).map_err(|e| e.to_string())?;
    if let Some(cache) = cache {
        cache.save_page(url, head, page, &files);
    }
    Ok(files)
}

#[cfg(test)]
mod tests;
