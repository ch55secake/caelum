//! Native workspace protocol. Agent/provider processes and project operations live on the VM.
//! The desktop owns editors, its workspace snapshot, and Boat lifecycle credentials.
mod protocol;
mod server;

pub use server::run_peer;

use super::{checkout::repo_name, MachineId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Stable across SSH reauthorization and VM resume. Never use an IP as workspace identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeTarget {
    pub machine: MachineId,
    pub repository: String,
}

impl NativeTarget {
    pub fn root(&self) -> Result<String, String> {
        if self.machine.account.is_empty() || self.machine.id.is_empty() {
            return Err("A native workspace needs an identified Boat machine.".into());
        }
        Ok(format!("/home/user/{}", repo_name(&self.repository)?))
    }

    pub fn store_path(&self, parent_store: &Path) -> PathBuf {
        let key = digest(&serde_json::to_vec(self).expect("serializable target"));
        parent_store
            .with_extension("native")
            .join(key)
            .join("workspace.json")
    }
}

fn digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
