//! Provider lookup for Finder launches, whose PATH omits user package managers.
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

fn directories() -> Vec<PathBuf> {
    search_paths(env::var_os("PATH"), env::var_os("HOME"))
}

fn search_paths(path: Option<OsString>, home: Option<OsString>) -> Vec<PathBuf> {
    let mut paths: Vec<_> = path
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = home {
        let home = PathBuf::from(home);
        paths.extend(
            [
                ".local/bin",
                ".nix-profile/bin",
                ".opencode/bin",
                ".npm-global/bin",
                ".volta/bin",
                ".bun/bin",
                ".cargo/bin",
                ".local/share/mise/shims",
                ".asdf/shims",
            ]
            .map(|p| home.join(p)),
        );
        let mut nodes: Vec<_> = std::fs::read_dir(home.join(".nvm/versions/node"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        nodes.sort_by_key(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| semver::Version::parse(n.trim_start_matches('v')).ok())
        });
        paths.extend(nodes.into_iter().rev().map(|p| p.join("bin")));
    }
    paths.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/run/current-system/sw/bin",
            "/usr/bin",
            "/bin",
        ]
        .map(PathBuf::from),
    );
    paths
}

pub fn find(name: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    directories()
        .into_iter()
        .map(|p| p.join(&name))
        .find(|p| executable(p))
}

pub fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Include the selected installation's node/runtime alongside the CLI shim.
pub fn path(executable: &Path) -> OsString {
    let mut paths: Vec<_> = executable
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .collect();
    paths.extend(directories());
    env::join_paths(paths).unwrap_or_else(|_| env::var_os("PATH").unwrap_or_default())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn finder_path_finds_user_installations_and_their_runtime() {
        let home = env::temp_dir().join(format!("caelum-cli-discovery-{}", std::process::id()));
        for version in ["v9.0.0", "v22.0.0"] {
            let bin = home.join(".nvm/versions/node").join(version).join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            crate::test_support::write_executable(&bin.join("codex"), "#!/usr/bin/env node\n");
            crate::test_support::write_executable(&bin.join("node"), "#!/bin/sh\nexit 0\n");
        }
        let paths = search_paths(
            Some("/usr/bin:/bin".into()),
            Some(home.clone().into_os_string()),
        );
        let binary = paths
            .iter()
            .map(|p| p.join("codex"))
            .find(|p| executable(p))
            .unwrap();
        assert!(binary.starts_with(home.join(".nvm/versions/node/v22.0.0")));
        assert!(std::process::Command::new(binary)
            .env("PATH", env::join_paths(&paths).unwrap())
            .status()
            .unwrap()
            .success());
        let nix = home.join(".nix-profile/bin");
        std::fs::create_dir_all(&nix).unwrap();
        crate::test_support::write_executable(&nix.join("codex"), "#!/bin/sh\nexit 0\n");
        assert_eq!(
            paths
                .iter()
                .map(|p| p.join("codex"))
                .find(|p| executable(p)),
            Some(nix.join("codex"))
        );
        std::fs::remove_dir_all(home).unwrap();
    }
}
