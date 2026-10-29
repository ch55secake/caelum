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

#[derive(Default)]
struct BrokerState {
    closed: bool,
    pending: BTreeMap<RequestKey, PendingRequest>,
    subscribers: Vec<Sender<Vec<ApprovalPrompt>>>,
}

struct PendingRequest {
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
        if state.closed {
            return Err(ApprovalError::Closed);
        }
        if state.pending.contains_key(&key) {
            return Err(ApprovalError::DuplicateRequest);
        }

        state
            .pending
            .insert(key.clone(), PendingRequest { request, response });
        Self::publish(&mut state);

        Ok(ApprovalWaiter {
            key,
            receiver,
            state: Arc::downgrade(&self.state),
        })
    }

    pub fn pending(&self) -> Vec<ApprovalPrompt> {
        Self::prompts(&self.lock())
    }

    pub fn subscribe(&self) -> Receiver<Vec<ApprovalPrompt>> {
        let (sender, receiver) = mpsc::channel();
        let mut state = self.lock();
        let _ = sender.send(Self::prompts(&state));
        if !state.closed {
            state.subscribers.push(sender);
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
        if state.closed {
            return Err(ApprovalError::Closed);
        }
        let pending = state
            .pending
            .remove(&key)
            .ok_or(ApprovalError::UnknownRequest)?;
        let _ = pending.response.send(decision);
        Self::publish(&mut state);
        Ok(())
    }

    /// Reject pending approvals during app shutdown and notify the UI.
    pub fn shutdown(&self) {
        let mut state = self.lock();
        if state.closed {
            return;
        }
        state.closed = true;
        for (_, pending) in std::mem::take(&mut state.pending) {
            let _ = pending.response.send(ApprovalDecision::Deny);
        }
        Self::publish(&mut state);
        state.subscribers.clear();
    }

    fn prompts(state: &BrokerState) -> Vec<ApprovalPrompt> {
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

    fn publish(state: &mut BrokerState) {
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
        if let Some(pending) = state.pending.remove(&self.key) {
            let _ = pending.response.send(ApprovalDecision::Deny);
            ApprovalBroker::publish(&mut state);
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
