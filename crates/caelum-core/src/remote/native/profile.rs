//! A confirmed machine deletion serializes with native snapshot writes, including writes
//! from a still-open child window. An old writer may not recreate a retired workspace.
#[cfg(test)]
mod tests;
use super::{digest, NativeTarget};
use crate::remote::MachineId;
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

fn owner(parent: &Path, machine: &MachineId) -> PathBuf {
    parent.with_added_extension("native").join(digest(
        &serde_json::to_vec(machine).expect("machine identity"),
    ))
}
fn guard(owner: &Path) -> Result<fs::File, String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(owner.parent().expect("profile parent"))
        .map_err(|e| e.to_string())?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(owner.with_extension("guard"))
        .map_err(|e| e.to_string())
}
impl NativeTarget {
    pub fn store_path(&self, parent: &Path) -> PathBuf {
        owner(parent, &self.machine)
            .join(digest(self.repository.to_ascii_lowercase().as_bytes()))
            .join("workspace.json")
    }
    pub fn with_live_profile<T>(
        &self,
        parent: &Path,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let lock = guard(&owner(parent, &self.machine))?;
        lock.lock_shared().map_err(|e| e.to_string())?;
        if lock.metadata().map_err(|e| e.to_string())?.len() != 0 {
            return Err("This VM has been deleted on Boat.".into());
        }
        action()
    }
}
pub(crate) fn retire(parent: &Path, machine: &MachineId) -> Result<(), String> {
    let owner = owner(parent, machine);
    let lock = guard(&owner)?;
    lock.lock().map_err(|e| e.to_string())?;
    lock.set_len(1)
        .and_then(|()| lock.sync_all())
        .map_err(|e| e.to_string())?;
    match fs::remove_dir_all(owner) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
