use std::thread;
use tokio::sync::{mpsc, oneshot};

type Job<State> = Box<dyn FnOnce(&mut State) + Send>;

/// A single owner for state used by synchronous background workers. The actor
/// runs on a dedicated thread so Git, SQLite, and other blocking I/O never run
/// on a Tokio executor. Dropping the last handle closes the mailbox.
pub(crate) struct BlockingActor<State>(mpsc::Sender<Job<State>>);

impl<State> Clone for BlockingActor<State> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<State: Send + 'static> BlockingActor<State> {
    pub(crate) fn spawn(name: &str, mut state: State) -> Self {
        let (sender, mut mailbox) = mpsc::channel::<Job<State>>(32);
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                while let Some(job) = mailbox.blocking_recv() {
                    job(&mut state);
                }
            })
            .expect("start blocking actor");
        Self(sender)
    }

    /// Only call from synchronous code, not from the UI or a Tokio task.
    pub(crate) fn call<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut State) -> T + Send + 'static,
    ) -> Option<T> {
        let (reply, result) = oneshot::channel();
        self.0
            .blocking_send(Box::new(move |state| {
                let _ = reply.send(job(state));
            }))
            .ok()?;
        result.blocking_recv().ok()
    }
}
