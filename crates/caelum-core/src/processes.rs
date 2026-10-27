use std::{
    collections::BTreeMap,
    process::{Child, ExitStatus},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(u64);

impl ProcessId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Default)]
struct RegistryState {
    closed: bool,
    processes: BTreeMap<ProcessId, ManagedChild>,
}

/// Owns child processes so cancellation and application shutdown can reap them.
/// Providers should build commands from an explicit executable and argument
/// vector, never by interpolating a request into a shell command.
#[derive(Default)]
pub struct ProcessRegistry {
    next_id: AtomicU64,
    state: Mutex<RegistryState>,
}

impl ProcessRegistry {
    pub fn register(&self, child: Child) -> Result<ProcessId, ProcessError> {
        let id = ProcessId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let child = ManagedChild::new(child);
        let mut state = self.lock();
        if state.closed {
            let _ = child.terminate();
            return Err(ProcessError::Closed);
        }
        state.processes.insert(id, child);
        Ok(id)
    }

    pub fn terminate(&self, id: ProcessId) -> Result<ExitStatus, ProcessError> {
        let child = self
            .lock()
            .processes
            .remove(&id)
            .ok_or(ProcessError::UnknownProcess)?;
        child.terminate().map_err(ProcessError::from)
    }

    /// Reap completed children and return their exit statuses for provider-level
    /// recovery decisions.
    pub fn poll_exited(&self) -> Result<Vec<(ProcessId, ExitStatus)>, ProcessError> {
        let mut state = self.lock();
        let mut completed = Vec::new();
        for (id, child) in &mut state.processes {
            if let Some(status) = child.try_wait().map_err(ProcessError::from)? {
                completed.push((*id, status));
            }
        }
        for (id, _) in &completed {
            state.processes.remove(id);
        }
        Ok(completed)
    }

    /// Stop accepting children and terminate/reap every child that remains.
    pub fn shutdown(&self) {
        let children = {
            let mut state = self.lock();
            state.closed = true;
            std::mem::take(&mut state.processes)
        };
        for (_, child) in children {
            let _ = child.terminate();
        }
    }

    fn lock(&self) -> MutexGuard<'_, RegistryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for ProcessRegistry {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ManagedChild {
    child: Option<Child>,
}

impl ManagedChild {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        match self.child.as_mut() {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }

    fn terminate(mut self) -> std::io::Result<ExitStatus> {
        let Some(mut child) = self.child.take() else {
            return Err(std::io::Error::other("child process was already reaped"));
        };
        match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => {
                let kill_result = child.kill();
                match child.wait() {
                    Ok(status) => Ok(status),
                    Err(wait_error) => match kill_result {
                        Ok(()) => Err(wait_error),
                        Err(kill_error) => Err(kill_error),
                    },
                }
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        }
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProcessError {
    #[error("process registry is shutting down")]
    Closed,
    #[error("child process is not registered")]
    UnknownProcess,
    #[error("child process operation failed: {0}")]
    Io(String),
}

impl From<std::io::Error> for ProcessError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Command, thread, time::Duration};

    #[cfg(unix)]
    fn sleeper() -> Child {
        Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("start test child")
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_terminates_and_reaps_the_child() {
        let registry = ProcessRegistry::default();
        let id = registry.register(sleeper()).unwrap();

        let status = registry.terminate(id).unwrap();

        assert!(!status.success());
        assert_eq!(registry.poll_exited().unwrap().len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_terminates_all_running_children() {
        let registry = ProcessRegistry::default();
        registry.register(sleeper()).unwrap();
        registry.register(sleeper()).unwrap();

        registry.shutdown();

        assert_eq!(registry.poll_exited().unwrap().len(), 0);
        assert_eq!(
            registry.register(sleeper()).err(),
            Some(ProcessError::Closed)
        );
    }

    #[cfg(unix)]
    #[test]
    fn exited_children_are_reported_for_provider_recovery() {
        let registry = ProcessRegistry::default();
        let id = registry
            .register(Command::new("/usr/bin/true").spawn().unwrap())
            .unwrap();
        thread::sleep(Duration::from_millis(20));

        let exited = registry.poll_exited().unwrap();

        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].0, id);
        assert!(exited[0].1.success());
    }

    #[cfg(unix)]
    #[test]
    fn a_provider_can_register_a_replacement_after_a_child_exits() {
        let registry = ProcessRegistry::default();
        registry
            .register(Command::new("/usr/bin/false").spawn().unwrap())
            .unwrap();
        thread::sleep(Duration::from_millis(20));

        let failed = registry.poll_exited().unwrap();
        assert_eq!(failed.len(), 1);
        assert!(!failed[0].1.success());

        let replacement = registry.register(sleeper()).unwrap();
        assert!(!registry.terminate(replacement).unwrap().success());
    }
}
