use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex, MutexGuard, Weak,
    },
    time::Duration,
};

use caelum_protocol::{ApprovalDecision, ApprovalPrompt, PermissionRequest};
use thiserror::Error;

type RequestKey = (String, String, String);

enum BrokerState {
    Open(OpenBroker),
    Closed,
}

impl Default for BrokerState {
    fn default() -> Self {
        Self::Open(OpenBroker::default())
    }
}

#[derive(Default)]
struct OpenBroker {
    pending: BTreeMap<RequestKey, PendingRequest>,
    subscribers: Vec<Sender<Vec<ApprovalPrompt>>>,
}

struct PendingRequest {
    attempt: Arc<()>,
    request: PermissionRequest,
    response: Sender<ApprovalDecision>,
}

/// Coordinates one-shot approvals between provider adapters and the desktop UI.
/// Unknown, timed-out, or cancelled requests always resolve as denied.
#[derive(Default, Clone)]
pub struct ApprovalBroker {
    state: Arc<Mutex<BrokerState>>,
}

impl ApprovalBroker {
    pub fn request(&self, request: PermissionRequest) -> Result<ApprovalWaiter, ApprovalError> {
        if request.provider.trim().is_empty()
            || request.project_path.trim().is_empty()
            || request.request_id.trim().is_empty()
            || request.summary.trim().is_empty()
        {
            return Err(ApprovalError::InvalidRequest);
        }

        let key = (
            request.provider.clone(),
            request.project_path.clone(),
            request.request_id.clone(),
        );
        let (response, receiver) = mpsc::channel();
        let mut state = self.lock();
        let BrokerState::Open(open) = &mut *state else {
            return Err(ApprovalError::Closed);
        };
        if open.pending.contains_key(&key) {
            return Err(ApprovalError::DuplicateRequest);
        }

        let attempt = Arc::new(());
        open.pending.insert(
            key.clone(),
            PendingRequest {
                attempt: attempt.clone(),
                request,
                response,
            },
        );
        Self::publish(open);

        Ok(ApprovalWaiter {
            key,
            attempt,
            receiver,
            state: Arc::downgrade(&self.state),
        })
    }

    pub fn pending(&self) -> Vec<ApprovalPrompt> {
        match &*self.lock() {
            BrokerState::Open(open) => Self::prompts(open),
            BrokerState::Closed => Vec::new(),
        }
    }

    pub fn subscribe(&self) -> Receiver<Vec<ApprovalPrompt>> {
        let (sender, receiver) = mpsc::channel();
        let mut state = self.lock();
        match &mut *state {
            BrokerState::Open(open) => {
                let _ = sender.send(Self::prompts(open));
                open.subscribers.push(sender);
            }
            BrokerState::Closed => {
                let _ = sender.send(Vec::new());
            }
        }
        receiver
    }

    pub fn resolve(
        &self,
        provider: &str,
        project_path: &str,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), ApprovalError> {
        let key = (
            provider.to_owned(),
            project_path.to_owned(),
            request_id.to_owned(),
        );
        let mut state = self.lock();
        let BrokerState::Open(open) = &mut *state else {
            return Err(ApprovalError::Closed);
        };
        let pending = open
            .pending
            .remove(&key)
            .ok_or(ApprovalError::UnknownRequest)?;
        let _ = pending.response.send(decision);
        Self::publish(open);
        Ok(())
    }

    /// Reject pending approvals during app shutdown and notify the UI.
    pub fn shutdown(&self) {
        let mut state = self.lock();
        let BrokerState::Open(mut open) = std::mem::replace(&mut *state, BrokerState::Closed)
        else {
            return;
        };
        for (_, pending) in std::mem::take(&mut open.pending) {
            let _ = pending.response.send(ApprovalDecision::Deny);
        }
        Self::publish(&mut open);
    }

    fn prompts(state: &OpenBroker) -> Vec<ApprovalPrompt> {
        state
            .pending
            .values()
            .map(|pending| ApprovalPrompt {
                provider: pending.request.provider.clone(),
                project_path: pending.request.project_path.clone(),
                session_id: pending.request.session_id.clone(),
                request_id: pending.request.request_id.clone(),
                summary: pending.request.summary.clone(),
                description: pending.request.description.clone(),
            })
            .collect()
    }

    fn publish(state: &mut OpenBroker) {
        let prompts = Self::prompts(state);
        state
            .subscribers
            .retain(|subscriber| subscriber.send(prompts.clone()).is_ok());
    }

    fn lock(&self) -> MutexGuard<'_, BrokerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub struct ApprovalWaiter {
    key: RequestKey,
    attempt: Arc<()>,
    receiver: Receiver<ApprovalDecision>,
    state: Weak<Mutex<BrokerState>>,
}

impl ApprovalWaiter {
    /// Wait for an explicit choice. Timeout or channel closure means deny.
    pub fn wait(self, timeout: Duration) -> ApprovalDecision {
        match self.receiver.recv_timeout(timeout) {
            Ok(decision) => decision,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                ApprovalDecision::Deny
            }
        }
    }
}

impl Drop for ApprovalWaiter {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let BrokerState::Open(open) = &mut *state else {
            return;
        };
        if let std::collections::btree_map::Entry::Occupied(entry) =
            open.pending.entry(self.key.clone())
        {
            if !Arc::ptr_eq(&self.attempt, &entry.get().attempt) {
                return;
            }
            let pending = entry.remove();
            let _ = pending.response.send(ApprovalDecision::Deny);
            ApprovalBroker::publish(open);
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("approval request is missing a provider, project path, request ID, or summary")]
    InvalidRequest,
    #[error("an approval request with this provider, project, and ID is already pending")]
    DuplicateRequest,
    #[error("approval request is no longer pending")]
    UnknownRequest,
    #[error("approval broker is shutting down")]
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(request_id: &str) -> PermissionRequest {
        PermissionRequest {
            provider: "test-provider".to_owned(),
            project_path: "/workspace/test".to_owned(),
            session_id: None,
            request_id: request_id.to_owned(),
            summary: "Run a tool".to_owned(),
            description: "The provider requested a tool operation.".to_owned(),
            provider_details: json!({ "providerMode": "confirm-once" }),
        }
    }

    #[test]
    fn allows_only_after_explicit_one_shot_approval() {
        let broker = ApprovalBroker::default();
        let waiter = broker.request(request("request-1")).unwrap();

        assert_eq!(broker.pending().len(), 1);
        broker
            .resolve(
                "test-provider",
                "/workspace/test",
                "request-1",
                ApprovalDecision::AllowOnce,
            )
            .unwrap();

        assert_eq!(
            waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::AllowOnce
        );
        assert!(broker.pending().is_empty());
    }

    #[test]
    fn times_out_as_deny_and_removes_the_pending_request() {
        let broker = ApprovalBroker::default();
        let waiter = broker.request(request("request-2")).unwrap();

        assert_eq!(
            waiter.wait(Duration::from_millis(1)),
            ApprovalDecision::Deny
        );
        assert!(broker.pending().is_empty());
        assert_eq!(
            broker.resolve(
                "test-provider",
                "/workspace/test",
                "request-2",
                ApprovalDecision::AllowOnce,
            ),
            Err(ApprovalError::UnknownRequest)
        );
    }

    #[test]
    fn app_shutdown_denies_all_pending_requests() {
        let broker = ApprovalBroker::default();
        let waiter = broker.request(request("request-3")).unwrap();
        let updates = broker.subscribe();
        assert_eq!(updates.recv().unwrap().len(), 1);

        broker.shutdown();

        assert_eq!(
            waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::Deny
        );
        assert!(updates.recv().unwrap().is_empty());
        assert_eq!(
            broker.request(request("later")).err(),
            Some(ApprovalError::Closed)
        );
    }

    #[test]
    fn duplicate_and_unknown_decisions_are_rejected() {
        let broker = ApprovalBroker::default();
        let _waiter = broker.request(request("request-4")).unwrap();

        assert_eq!(
            broker.request(request("request-4")).err(),
            Some(ApprovalError::DuplicateRequest)
        );
        assert_eq!(
            broker.resolve(
                "test-provider",
                "/workspace/test",
                "missing",
                ApprovalDecision::AllowOnce,
            ),
            Err(ApprovalError::UnknownRequest)
        );
    }

    #[test]
    fn dropping_a_resolved_waiter_cannot_deny_a_reused_request_id() {
        let broker = ApprovalBroker::default();
        let old = broker.request(request("reused")).unwrap();
        let resolve =
            |decision| broker.resolve("test-provider", "/workspace/test", "reused", decision);
        resolve(ApprovalDecision::Deny).unwrap();
        let replacement = broker.request(request("reused")).unwrap();

        drop(old);

        assert_eq!(broker.pending().len(), 1);
        resolve(ApprovalDecision::AllowOnce).unwrap();
        assert_eq!(
            replacement.wait(Duration::ZERO),
            ApprovalDecision::AllowOnce
        );
    }

    #[test]
    fn provider_details_are_not_exposed_in_ui_prompts() {
        let broker = ApprovalBroker::default();
        let mut request = request("request-5");
        request.provider_details = json!({ "token": "sensitive provider data" });
        let _waiter = broker.request(request).unwrap();

        let prompt = broker.pending().pop().unwrap();
        let encoded = serde_json::to_string(&prompt).unwrap();
        assert!(!encoded.contains("sensitive provider data"));
        assert_eq!(prompt.provider, "test-provider");
        assert_eq!(prompt.project_path, "/workspace/test");
        assert_eq!(prompt.session_id, None);
    }

    #[test]
    fn approvals_with_same_provider_and_request_id_are_scoped_by_project() {
        let broker = ApprovalBroker::default();
        let mut first = request("same-id");
        first.project_path = "/workspace/one".to_owned();
        let mut second = request("same-id");
        second.project_path = "/workspace/two".to_owned();
        let first_waiter = broker.request(first).unwrap();
        let second_waiter = broker.request(second).unwrap();

        broker
            .resolve(
                "test-provider",
                "/workspace/two",
                "same-id",
                ApprovalDecision::AllowOnce,
            )
            .unwrap();

        assert_eq!(
            second_waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::AllowOnce
        );
        assert_eq!(broker.pending()[0].project_path, "/workspace/one");
        drop(first_waiter);
    }
}
