//! The CLI/desktop launch contract. URLs deliver project opens to cold and running apps.
use reqwest::Url;
use std::path::{Path, PathBuf};

pub fn project_url(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .filter(|_| path.is_absolute())
        .ok_or("Expected an absolute UTF-8 project path")?;
    let mut url = Url::parse("caelum://open").expect("static URL");
    url.query_pairs_mut().append_pair("path", path);
    Ok(url.into())
}

pub fn project_from_url(value: &str) -> Result<PathBuf, String> {
    let url = Url::parse(value).map_err(|e| e.to_string())?;
    if url.scheme() != "caelum"
        || url.host_str() != Some("open")
        || !url.path().is_empty()
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Expected a caelum://open project link".into());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.len() != 1 || pairs[0].0 != "path" || pairs[0].1.contains('\0') {
        return Err("Expected one project path".into());
    }
    let path = PathBuf::from(pairs[0].1.as_ref());
    if !path.is_absolute() {
        return Err("Project links require an absolute path".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_links_roundtrip_spaces_unicode_and_url_delimiters() {
        let path = Path::new("/projects/café #1 & 2/%?+branch");
        assert_eq!(project_from_url(&project_url(path).unwrap()).unwrap(), path);
        for invalid in [
            "https://open?path=/project",
            "caelum://open?path=relative",
            "caelum://open?path=/a&path=/b",
            "caelum://open?path=/a&run=command",
            "caelum://user@open?path=/a",
            "caelum://open?path=/a%00",
        ] {
            assert!(project_from_url(invalid).is_err(), "{invalid}");
        }
    }
}
