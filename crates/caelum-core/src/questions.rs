use std::{
    collections::{BTreeMap, HashSet},
    sync::{mpsc, Arc, Mutex, MutexGuard},
};

use caelum_protocol::QuestionPrompt;
use serde_json::{Map, Value};

pub type Answer = Map<String, Value>;
pub type Reply = Arc<dyn Fn(Option<Answer>) -> Result<(), String> + Send + Sync>;
type Key = (String, String, String, String);

struct Pending {
    prompt: QuestionPrompt,
    reply: Reply,
    submitting: bool,
}

#[derive(Default)]
struct State {
    disabled: bool,
    revision: u64,
    pending: BTreeMap<Key, Pending>,
    subscribers: Vec<mpsc::Sender<Vec<QuestionPrompt>>>,
}

/// Keeps live requests until the provider accepts a reply. Failed submissions
/// remain retryable; cancellation by a provider never sends a second reply.
#[derive(Default, Clone)]
pub struct QuestionBroker(Arc<Mutex<State>>);

impl QuestionBroker {
    /// Terminal clients without a question UI keep their existing behavior.
    pub fn disabled() -> Self {
        let broker = Self::default();
        broker.lock().disabled = true;
        broker
    }

    pub fn pending(&self) -> Vec<QuestionPrompt> {
        Self::prompts(&self.lock())
    }

    pub fn subscribe(&self) -> mpsc::Receiver<Vec<QuestionPrompt>> {
        let (sender, receiver) = mpsc::channel();
        let mut state = self.lock();
        let _ = sender.send(Self::prompts(&state));
        state.subscribers.push(sender);
        receiver
    }

    pub fn insert(&self, prompt: QuestionPrompt, reply: Reply) -> Result<(), String> {
        validate(&prompt)?;
        let mut state = self.lock();
        if state.disabled {
            return Err("This client cannot answer agent questions".into());
        }
        let key = key(&prompt);
        if state.pending.contains_key(&key) {
            // A duplicate live event can still invalidate a stale snapshot.
            Self::publish(&mut state);
            return Ok(());
        }
        state.pending.insert(
            key,
            Pending {
                prompt,
                reply,
                submitting: false,
            },
        );
        Self::publish(&mut state);
        Ok(())
    }

    pub fn resolve(
        &self,
        provider: &str,
        project: &str,
        session: &str,
        request: &str,
        answer: Option<Answer>,
    ) -> Result<(), String> {
        let key = (
            provider.into(),
            project.into(),
            session.into(),
            request.into(),
        );
        let reply = {
            let mut state = self.lock();
            let pending = state
                .pending
                .get_mut(&key)
                .ok_or("Question is no longer pending")?;
            if pending.submitting {
                return Err("An answer is already being submitted".into());
            }
            pending.submitting = true;
            Arc::clone(&pending.reply)
        };
        // Never hold the broker lock across network I/O or a provider callback.
        let result = reply(answer);
        let mut state = self.lock();
        if let Some(pending) = state.pending.get_mut(&key) {
            if Arc::ptr_eq(&pending.reply, &reply) {
                if result.is_ok() {
                    state.pending.remove(&key);
                    Self::publish(&mut state);
                } else {
                    pending.submitting = false;
                }
            }
        }
        result
    }

    pub fn dismiss(&self, provider: &str, project: &str, session: &str, request: &str) {
        let mut state = self.lock();
        state.pending.remove(&(
            provider.into(),
            project.into(),
            session.into(),
            request.into(),
        ));
        // Even an unseen resolution invalidates an in-flight pending snapshot.
        Self::publish(&mut state);
    }

    pub fn clear(&self, provider: &str, project: &str, session: Option<&str>) {
        let mut state = self.lock();
        state.pending.retain(|(p, path, id, _), _| {
            p != provider || path != project || session.is_some_and(|session| id != session)
        });
        Self::publish(&mut state);
    }

    pub fn revision(&self) -> u64 {
        self.lock().revision
    }

    /// The event stream is subscribed before fetching. Discard a snapshot if a
    /// live event arrived during the fetch, so answered forms cannot reappear.
    pub fn sync(
        &self,
        provider: &str,
        project: &str,
        revision: u64,
        requests: Vec<(QuestionPrompt, Reply)>,
    ) -> Result<bool, String> {
        for (prompt, _) in &requests {
            validate(prompt)?;
            if prompt.provider != provider || prompt.project_path != project {
                return Err("Question snapshot does not belong to this project".into());
            }
        }
        let mut state = self.lock();
        if state.disabled || state.revision != revision {
            return Ok(false);
        }
        let keys: HashSet<_> = requests.iter().map(|(prompt, _)| key(prompt)).collect();
        let before = Self::prompts(&state);
        state
            .pending
            .retain(|key, _| key.0 != provider || key.1 != project || keys.contains(key));
        for (prompt, reply) in requests {
            state.pending.entry(key(&prompt)).or_insert(Pending {
                prompt,
                reply,
                submitting: false,
            });
        }
        if before != Self::prompts(&state) {
            Self::publish(&mut state);
        }
        Ok(true)
    }

    fn prompts(state: &State) -> Vec<QuestionPrompt> {
        state
            .pending
            .values()
            .map(|pending| pending.prompt.clone())
            .collect()
    }

    fn publish(state: &mut State) {
        state.revision += 1;
        let prompts = Self::prompts(state);
        state
            .subscribers
            .retain(|sender| sender.send(prompts.clone()).is_ok());
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn key(prompt: &QuestionPrompt) -> Key {
    (
        prompt.provider.clone(),
        prompt.project_path.clone(),
        prompt.session_id.clone(),
        prompt.request_id.clone(),
    )
}

fn validate(prompt: &QuestionPrompt) -> Result<(), String> {
    let mut keys = HashSet::new();
    if [
        &prompt.provider,
        &prompt.project_path,
        &prompt.session_id,
        &prompt.request_id,
    ]
    .iter()
    .any(|value| value.is_empty())
        || prompt.fields.is_empty()
        || prompt.fields.iter().any(|field| {
            field
                .get("key")
                .and_then(Value::as_str)
                .map_or(true, |key| key.is_empty() || !keys.insert(key))
        })
    {
        return Err("Invalid agent question".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
        time::Duration,
    };

    fn prompt(provider: &str, project: &str, session: &str) -> QuestionPrompt {
        QuestionPrompt {
            provider: provider.into(),
            project_path: project.into(),
            session_id: session.into(),
            request_id: "same-id".into(),
            title: "Choose".into(),
            fields: vec![json!({ "key": "q0", "type": "string" })],
        }
    }

    #[test]
    fn failed_answers_are_retryable_and_resolutions_are_scoped() {
        let broker = QuestionBroker::default();
        let attempts = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&attempts);
        let reply: Reply = Arc::new(move |answer| {
            assert_eq!(answer.unwrap()["q0"], "selected");
            if counted.fetch_add(1, Ordering::SeqCst) == 0 {
                Err("offline".into())
            } else {
                Ok(())
            }
        });
        broker.insert(prompt("codex", "/a", "s1"), reply).unwrap();
        for request in [
            prompt("opencode", "/a", "s1"),
            prompt("codex", "/b", "s1"),
            prompt("codex", "/a", "s2"),
        ] {
            broker
                .insert(request, Arc::new(|_| panic!("unrelated reply")))
                .unwrap();
        }
        let answer = json!({ "q0": "selected" }).as_object().unwrap().clone();
        assert!(broker
            .resolve("codex", "/a", "s1", "same-id", Some(answer.clone()))
            .is_err());
        assert_eq!(broker.pending().len(), 4);
        broker
            .resolve("codex", "/a", "s1", "same-id", Some(answer))
            .unwrap();
        assert!(broker
            .resolve("codex", "/a", "s1", "same-id", None)
            .is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        broker.clear("codex", "/a", None);
        assert_eq!(broker.pending().len(), 2);
        broker.dismiss("codex", "/b", "s1", "same-id");
        assert_eq!(broker.pending(), vec![prompt("opencode", "/a", "s1")]);
    }

    #[test]
    fn duplicate_submissions_are_locked_and_provider_cancellation_wins() {
        let broker = QuestionBroker::default();
        let (started, waiting) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let blocked = Mutex::new(blocked);
        broker
            .insert(
                prompt("codex", "/a", "s1"),
                Arc::new(move |answer| {
                    assert!(answer.is_none());
                    started.send(()).unwrap();
                    blocked
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(2))
                        .unwrap();
                    Ok(())
                }),
            )
            .unwrap();
        let worker = broker.clone();
        let task = thread::spawn(move || worker.resolve("codex", "/a", "s1", "same-id", None));
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(broker
            .resolve("codex", "/a", "s1", "same-id", None)
            .unwrap_err()
            .contains("already"));
        broker.dismiss("codex", "/a", "s1", "same-id");
        // A completion from the old callback must not remove a replacement.
        broker
            .insert(prompt("codex", "/a", "s1"), Arc::new(|_| Ok(())))
            .unwrap();
        release.send(()).unwrap();
        task.join().unwrap().unwrap();
        assert_eq!(broker.pending().len(), 1);
    }

    #[test]
    fn live_events_invalidate_snapshots_and_sync_preserves_other_scopes() {
        let broker = QuestionBroker::default();
        let request = prompt("opencode", "/a", "s1");
        let reply: Reply = Arc::new(|_| Ok(()));
        let version = broker.revision();
        broker.dismiss("opencode", "/a", "s1", "same-id");
        assert!(!broker
            .sync(
                "opencode",
                "/a",
                version,
                vec![(request.clone(), reply.clone())]
            )
            .unwrap());
        assert!(broker.pending().is_empty());
        broker
            .insert(prompt("codex", "/b", "s1"), reply.clone())
            .unwrap();
        assert!(broker
            .sync(
                "opencode",
                "/a",
                broker.revision(),
                vec![(request.clone(), reply.clone())]
            )
            .unwrap());
        let version = broker.revision();
        broker.insert(request, reply).unwrap();
        assert!(!broker.sync("opencode", "/a", version, vec![]).unwrap());
        assert_eq!(broker.pending().len(), 2);
        assert!(broker
            .sync("opencode", "/a", broker.revision(), vec![])
            .unwrap());
        assert_eq!(broker.pending(), vec![prompt("codex", "/b", "s1")]);
    }
}
