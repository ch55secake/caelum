use super::{Command, Event, Execute};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    thread::{self, JoinHandle},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

pub(super) type Request = (u64, Command);

const NORMAL_LIMIT: usize = 8;
const HISTORY_LIMIT: usize = 2;
const CONTROL_LIMIT: usize = 2;

pub(super) struct Workers {
    thread: JoinHandle<()>,
    stop: oneshot::Sender<()>,
}

impl Workers {
    pub(super) fn start(
        normal: mpsc::Receiver<Request>,
        history: mpsc::Receiver<Request>,
        control: mpsc::Receiver<Request>,
        execute: Execute,
        sender: SyncSender<Event>,
        wake: Arc<dyn Fn() + Send + Sync>,
        closed: Arc<AtomicBool>,
    ) -> Self {
        let (stop, stopped) = oneshot::channel();
        let thread = thread::Builder::new()
            .name("caelum-commands".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .max_blocking_threads(NORMAL_LIMIT + HISTORY_LIMIT + CONTROL_LIMIT)
                    .build()
                    .expect("start command runtime");
                let run = move |(id, command): Request| {
                    if closed.load(Ordering::Acquire) {
                        return;
                    }
                    let _context = crate::diagnostics::context(id);
                    crate::diagnostics::command("started", id, &command);
                    let result = execute(&command);
                    crate::diagnostics::command("finished", id, &command);
                    if sender.send(Event::Completed(id, result)).is_ok() {
                        wake();
                    }
                };
                runtime.block_on(async {
                    tokio::select! {
                        biased;
                        _ = stopped => {}
                        _ = async {
                            tokio::join!(
                                dispatch(normal, NORMAL_LIMIT, run.clone()),
                                dispatch(history, HISTORY_LIMIT, run.clone()),
                                dispatch(control, CONTROL_LIMIT, run),
                            );
                        } => {}
                    }
                });
            })
            .expect("start command dispatcher");
        Self { thread, stop }
    }

    pub(super) fn stop(self) -> JoinHandle<()> {
        let _ = self.stop.send(());
        self.thread
    }
}

async fn dispatch(
    mut mailbox: mpsc::Receiver<Request>,
    limit: usize,
    run: impl Fn(Request) + Clone + Send + 'static,
) {
    let mut running = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = running.join_next(), if !running.is_empty() => {}
            request = mailbox.recv(), if running.len() < limit => {
                let Some(request) = request else { break };
                let run = run.clone();
                running.spawn_blocking(move || run(request));
            }
        }
    }
}
