use super::harness::Harness;
use super::{
    boat::{api, Boat},
    credentials, Credential, Machines, Operation, Outcome,
};
use std::{sync::OnceLock, time::Duration};
use tokio::{
    runtime::{Builder, Runtime},
    sync::Mutex,
};

#[derive(Default)]
pub(crate) struct Backend {
    runtime: OnceLock<Result<Runtime, String>>,
    connection: Mutex<Option<Boat>>,
    closed: tokio::sync::watch::Sender<bool>,
}

impl Backend {
    pub(crate) fn shutdown(&self) {
        self.closed.send_replace(true);
    }
    pub(crate) fn execute(&self, op: &Operation) -> Result<Outcome, String> {
        let rt = self.runtime.get_or_init(|| {
            Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
        });
        rt.as_ref().map_err(Clone::clone)?.block_on(async {
            let mut closed = self.closed.subscribe();
            tokio::select! {
                biased;
                _ = closed.wait_for(|v| *v) => Err("Remote connection closed".into()),
                result = tokio::time::timeout(Duration::from_secs(90), self.perform(op)) =>
                    result.map_err(|_| "Boat did not respond in time".to_owned())?,
            }
        })
    }

    async fn authenticate(key: &Credential) -> Result<Boat, String> {
        Boat::authenticate(boat_sdk::Client::new(key.0.trim()).map_err(api)?).await
    }

    async fn connected(&self) -> Result<Boat, String> {
        let mut slot = self.connection.lock().await;
        if slot.is_none() {
            *slot = Some(Self::authenticate(&credentials::read().await?).await?);
        }
        Ok(slot.as_ref().expect("authenticated").clone())
    }

    async fn perform(&self, op: &Operation) -> Result<Outcome, String> {
        match op {
            Operation::Models => boat_sdk::agents::provider_models()
                .await
                .map(super::models::model_lists)
                .map(Outcome::Models)
                .map_err(api),
            Operation::Delete(key) => {
                let confirmed = self.connected().await?.delete(key).await?;
                if let Outcome::Deleted(id) = &confirmed {
                    if let Err(e) =
                        super::native::retire(&crate::application::Store::default_path(), id)
                    {
                        eprintln!("Could not retire the deleted VM's native workspace: {e}");
                    }
                }
                Ok(confirmed)
            }
            Operation::Stats(key) => self.connected().await?.stats(key).await.map(Outcome::Stats),
            Operation::Git(target, action) => self
                .connected()
                .await?
                .git(target, action)
                .await
                .map(Outcome::Review),
            Operation::Prompt(target, prompt) => match self.connected().await {
                Ok(boat) => boat.send(target, prompt).await,
                Err(error) => Ok(Outcome::Rejected(error)),
            },
            Operation::Follow(target, run, cursor) => self
                .connected()
                .await?
                .follow(target, run, cursor.as_deref())
                .await
                .map(Outcome::Page),
            Operation::InspectTool(key, source) => self
                .connected()
                .await?
                .tool_output(key, source)
                .await
                .map(Outcome::ToolOutput),
            Operation::Interrupt(target, run) => self
                .connected()
                .await?
                .interrupt(target, run)
                .await
                .map(|_| Outcome::Interrupted),
            Operation::Recover(target, provider, conv, prompt) => self
                .connected()
                .await?
                .recover(target, *provider, conv.as_deref(), prompt)
                .await
                .map(Outcome::Accepted),
            Operation::Discover(project) | Operation::DiscoverHandoff(project) => self
                .connected()
                .await?
                .discover(project, matches!(op, Operation::DiscoverHandoff(_)))
                .await
                .map(Outcome::Source),
            Operation::Lifetime(key, lifetime) => self
                .connected()
                .await?
                .lifetime(key, *lifetime)
                .await
                .map(Outcome::Lifetime),
            Operation::Create(source, plan) => self
                .connected()
                .await?
                .create(source, plan)
                .await
                .map(Outcome::Machine),
            Operation::Prepare(target, base, branch) => {
                self.connected()
                    .await?
                    .prepare(target, base, branch.as_deref())
                    .await
            }
            Operation::Power(key, resume) => self
                .connected()
                .await?
                .power(key, *resume)
                .await
                .map(Outcome::Machine),
            Operation::Connect(key) => {
                let mut slot = self.connection.lock().await;
                let boat = Self::authenticate(key).await?;
                let inventory = boat.inventory().await?;
                credentials::store(Some(key)).await?;
                *slot = Some(boat);
                Ok(Outcome::Inventory(inventory))
            }
            Operation::Inventory => self
                .connected()
                .await?
                .inventory()
                .await
                .map(Outcome::Inventory),
            Operation::Disconnect => {
                let mut slot = self.connection.lock().await;
                credentials::store(None).await?;
                *slot = None;
                Ok(Outcome::Disconnected)
            }
        }
    }
}
