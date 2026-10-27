#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Stable, serializable application metadata shared across Caelum interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

/// A project path approved and normalized by the Rust core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
}
