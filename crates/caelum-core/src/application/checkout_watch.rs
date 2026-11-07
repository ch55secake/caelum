//! Coalesced native checkout notifications, including worktree Git metadata.
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(super) struct CheckoutWatch {
    watcher: RecommendedWatcher,
    roots: Arc<Mutex<BTreeMap<PathBuf, BTreeSet<String>>>>,
    pub dirty: Arc<Mutex<BTreeSet<String>>>,
}

impl CheckoutWatch {
    pub fn new() -> notify::Result<Self> {
        let roots = Arc::new(Mutex::new(BTreeMap::<PathBuf, BTreeSet<String>>::new()));
        let dirty = Arc::new(Mutex::new(BTreeSet::new()));
        let watched = roots.clone();
        let changed = dirty.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let roots = watched.lock().unwrap_or_else(|e| e.into_inner());
            let mut dirty = changed.lock().unwrap_or_else(|e| e.into_inner());
            match event {
                Ok(event)
                    if matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Modify(_)
                            | EventKind::Remove(_)
                            | EventKind::Any
                    ) =>
                {
                    for path in event.paths {
                        // Git's own reads create index locks; refreshing those would loop.
                        if path.components().any(|c| c.as_os_str() == ".git")
                            && path
                                .file_name()
                                .is_some_and(|n| n.to_string_lossy().ends_with(".lock"))
                        {
                            continue;
                        }
                        for (root, owners) in
                            roots.iter().filter(|(root, _)| path.starts_with(root))
                        {
                            let relative = path.strip_prefix(root).unwrap_or(&path);
                            if relative.components().any(|c| {
                                matches!(
                                    c.as_os_str().to_str(),
                                    Some("target" | "node_modules" | ".DS_Store")
                                )
                            }) {
                                continue;
                            }
                            dirty.extend(owners.iter().cloned());
                        }
                    }
                }
                // An overflow/error invalidates the snapshot as well.
                Err(_) => dirty.extend(roots.values().flatten().cloned()),
                _ => {}
            }
        })?;
        Ok(Self {
            watcher,
            roots,
            dirty,
        })
    }

    pub fn sync(&mut self, projects: impl IntoIterator<Item = String>) {
        let mut next = BTreeMap::<PathBuf, BTreeSet<String>>::new();
        for project in projects {
            let root = PathBuf::from(&project);
            next.entry(root.clone())
                .or_default()
                .insert(project.clone());
            let Some(marker) = root
                .ancestors()
                .map(|p| p.join(".git"))
                .find(|p| p.exists())
            else {
                continue;
            };
            if marker.is_dir() {
                next.entry(marker).or_default().insert(project);
                continue;
            }
            let parent = marker.parent().unwrap().to_owned();
            if let Some(dir) = std::fs::read_to_string(marker)
                .ok()
                .and_then(|v| v.trim().strip_prefix("gitdir: ").map(str::to_owned))
            {
                let git = parent.join(dir);
                let git = git.canonicalize().unwrap_or(git);
                next.entry(git.clone()).or_default().insert(project.clone());
                if let Ok(common) = std::fs::read_to_string(git.join("commondir")) {
                    let common = git.join(common.trim());
                    next.entry(common.canonicalize().unwrap_or(common))
                        .or_default()
                        .insert(project);
                }
            }
        }
        let previous = std::mem::replace(
            &mut *self.roots.lock().unwrap_or_else(|e| e.into_inner()),
            next.clone(),
        );
        for path in previous.keys().filter(|p| !next.contains_key(*p)) {
            let _ = self.watcher.unwatch(Path::new(path));
        }
        for path in next.keys().filter(|p| !previous.contains_key(*p)) {
            let _ = self.watcher.watch(path, RecursiveMode::Recursive);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_edits_and_worktree_metadata_notify_the_correct_checkout() {
        let temp =
            std::env::temp_dir().join(format!("caelum-checkout-watch-{}", std::process::id()));
        let root = temp.join("checkout");
        let git = temp.join("metadata");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(root.join(".git"), format!("gitdir: {}", git.display())).unwrap();
        let root = root.canonicalize().unwrap().to_string_lossy().into_owned();
        let mut watch = CheckoutWatch::new().unwrap();
        watch.sync([root.clone()]);
        for path in [
            Path::new(&root).join("file.rs"),
            Path::new(&root).join("Cargo.lock"),
            git.join("HEAD"),
        ] {
            watch.dirty.lock().unwrap().clear();
            std::fs::write(path, "changed externally").unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
            while watch.dirty.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert_eq!(*watch.dirty.lock().unwrap(), BTreeSet::from([root.clone()]));
        }
        drop(watch);
        std::fs::remove_dir_all(temp).unwrap();
    }
}
