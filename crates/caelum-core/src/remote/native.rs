//! Native workspace protocol. Agent/provider processes and project operations live on the VM.
//! The desktop owns editors, its workspace snapshot, and Boat lifecycle credentials.
mod client;
mod profile;
mod protocol;
mod server;
mod ssh;

pub use client::Client;
pub use client::Notice;
pub use client::Status;
pub(crate) use profile::retire;
pub use server::run_peer;

use super::{checkout::repo_name, MachineId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable across SSH reauthorization and VM resume. Never use an IP as workspace identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeTarget {
    pub machine: MachineId,
    pub repository: String,
}

impl NativeTarget {
    pub fn root(&self) -> Result<String, String> {
        if self.machine.account.trim().is_empty()
            || self.machine.id.is_empty()
            || self.machine.id.len() > 256
            || !self
                .machine
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err("A native workspace needs an identified Boat machine.".into());
        }
        Ok(format!("/home/user/{}", repo_name(&self.repository)?))
    }
}

fn digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
