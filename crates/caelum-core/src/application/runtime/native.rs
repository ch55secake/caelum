use super::{Command, ProviderRuntime, Reply, Runtime};
use crate::{
    application::{Store, Task},
    remote::native::NativeTarget,
};
use std::{path::PathBuf, sync::Arc};

impl Runtime {
    /// Desktop-side workers for a native VM window. No provider event subscriptions or laptop
    /// checkout watchers are started. Reject project operations even if misrouted by a caller.
    pub fn native_desktop(
        target: NativeTarget,
        parent_store: PathBuf,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let backend = ProviderRuntime::default();
        let path = target.store_path(&parent_store);
        Self::with_executor(
            Arc::new(move |cmd| match cmd {
                Command::Load(store @ Store(p)) if p == &path => target
                    .with_live_profile(&parent_store, || {
                        store.load().map(|s| Reply::Snapshot(s.map(Box::new)))
                    }),
                Command::Save(store @ Store(p), snapshot) if p == &path => target
                    .with_live_profile(&parent_store, || {
                        store.save(snapshot).map(|()| Reply::Done)
                    }),
                Command::Sound(_) | Command::Task(Task::Updates) => backend.execute(cmd),
                _ => Err("A VM workspace operation cannot run on the laptop.".into()),
            }),
            Arc::new(wake),
        )
    }
}
