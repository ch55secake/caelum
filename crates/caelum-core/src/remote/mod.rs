//! Machine identities and operations are independent of local project paths and coding providers.
mod boat;
mod chat;
mod checkout;
mod credentials;
mod git;
mod harness;
mod lifetime;
mod models;
pub mod native;
mod runtime;
mod stats;
mod tools;

pub use chat::Chat;
pub use chat::Continuation;
pub use chat::Message;
pub(crate) use checkout::github_slug;
pub use checkout::PublishedBase;
pub use checkout::VmLink;
pub use checkout::{valid_branch, Checkout, Job, Phase, Picker, Plan, Saved, Size, Source, Target};
pub use git::{Changes, GitAction, Review, Revision};
pub use harness::{Line, Page, Prompt, Run, RunStatus, Speaker, Thread, ToolState, Turn, TurnEnd};
pub use lifetime::Lifetime;
pub use models::ModelCatalogue;
pub use models::ModelLists;
pub(crate) use runtime::Backend;
pub use stats::{Activity, Monitor, Process, Sample};
pub use tools::ToolCall;
pub use tools::ToolInspection;
pub use tools::ToolLoad;
pub use tools::ToolOutput;
pub use tools::ToolSource;

use crate::application::Provider;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Credential(Arc<Zeroizing<String>>);

impl Credential {
    pub fn new(key: String) -> Self {
        Self(Arc::new(Zeroizing::new(key)))
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Host {
    Boat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineId {
    pub host: Host,
    pub account: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub key: MachineId,
    pub name: String,
    pub state: String,
    #[serde(default)]
    pub auto_stop_unix: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub account: String,
    pub machines: Vec<Machine>,
}

trait Machines {
    async fn inventory(&self) -> Result<Inventory, String>;
    async fn create(&self, source: &Source, plan: &Plan) -> Result<Machine, String>;
    async fn power(&self, machine: &MachineId, resume: bool) -> Result<Machine, String>;
    async fn prepare(
        &self,
        target: &Target,
        base: &str,
        branch: Option<&str>,
    ) -> Result<Outcome, String>;
}

#[derive(Clone, Debug)]
pub enum Operation {
    Connect(Credential),
    Inventory,
    Models,
    Disconnect,
    Discover(String),
    DiscoverHandoff(String),
    Create(Source, Plan),
    Prepare(Target, String, Option<String>),
    Power(MachineId, bool),
    Delete(MachineId),
    Lifetime(MachineId, Lifetime),
    Prompt(Target, Prompt),
    Follow(Target, Run, Option<String>),
    InspectTool(MachineId, ToolSource),
    Interrupt(Target, Run),
    Recover(Target, Provider, Option<String>, String),
    Git(Target, GitAction),
    Stats(MachineId),
}

#[derive(Debug)]
pub enum Outcome {
    Inventory(Inventory),
    Models(ModelLists),
    Disconnected,
    Deleted(MachineId),
    Source(Source),
    Machine(Machine),
    Waiting(Machine),
    Prepared {
        machine: Machine,
        branch: String,
        root: String,
    },
    Lifetime(Machine),
    Accepted(Run),
    Rejected(String),
    Page(Page),
    ToolOutput(ToolOutput),
    Interrupted,
    Review(Review),
    Stats(Sample),
}

#[derive(Debug, Default)]
pub struct State {
    pub inventory: Option<Inventory>,
    pub models: ModelCatalogue,
    pub connection: Connection,
    pub saved: Saved,
    pub picker: Option<Picker>,
    pub jobs: std::collections::HashMap<String, Job>,
    pub feeds: std::collections::HashMap<String, Job>,
    pub interrupts: std::collections::HashMap<String, u64>,
    pub changes: std::collections::HashMap<String, Changes>,
    pub monitor: Monitor,
    pub tool: Option<ToolInspection>,
    pub lifetimes: std::collections::HashMap<String, Job>,
    pub delete_errors: std::collections::HashMap<String, String>,
    pub removed: Vec<String>,
    pub(crate) deleted_during_inventory: Vec<MachineId>,
}

#[derive(Debug, Default)]
pub enum Connection {
    #[default]
    Idle,
    Loading(u64),
    Failed(String),
}
