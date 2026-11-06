#![forbid(unsafe_code)]

pub mod application;
pub mod approvals;
pub mod codex;
pub mod database;
pub mod desktop;
pub mod files;
pub mod git;
pub mod github;
mod images;
pub mod opencode;
pub mod processes;
pub mod projects;
pub mod questions;
pub mod session_branches;
pub mod updates;
pub mod worktrees;

#[cfg(all(test, unix))]
mod test_support;
