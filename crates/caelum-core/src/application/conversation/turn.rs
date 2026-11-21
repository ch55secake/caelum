use serde_json::Value;

use super::{provider_started_at, set_metadata, set_output_duration, Conversation};

#[derive(Debug)]
pub(super) enum Turn {
    AwaitingProvider(Timing),
    ProviderStarted(Timing),
}

#[derive(Debug)]
pub(super) struct Timing {
    started_at: u64,
    agent: Option<String>,
    output_ids: Vec<String>,
}

impl Timing {
    fn new(started_at: u64, agent: Option<String>) -> Self {
        Self {
            started_at,
            agent,
            output_ids: Vec::new(),
        }
    }
}

impl Conversation {
    pub fn begin_turn(&mut self, agent: Option<String>, started_at: u64) {
        self.failure.begin(started_at);
        if !self.working || self.turn.is_none() {
            self.turn = Some(Turn::AwaitingProvider(Timing::new(started_at, agent)));
        }
    }

    pub fn provider_started_with_agent(&mut self, agent: Option<String>, started_at: u64) {
        let timing = match self.turn.take() {
            Some(started @ Turn::ProviderStarted(_)) => {
                self.turn = Some(started);
                return;
            }
            Some(Turn::AwaitingProvider(mut timing)) if self.working => {
                timing.started_at = timing.started_at.min(started_at);
                timing
            }
            None | Some(Turn::AwaitingProvider(_)) => Timing::new(started_at, agent),
        };
        self.turn = Some(Turn::ProviderStarted(timing));
    }

    pub(super) fn provider_started(&mut self, data: &Value) {
        let agent = self.turn.as_ref().and_then(|turn| match turn {
            Turn::AwaitingProvider(timing) | Turn::ProviderStarted(timing) => timing.agent.clone(),
        });
        self.provider_started_with_agent(agent, provider_started_at(data));
    }

    pub(super) fn note_turn_output(&mut self, id: &str) {
        if let Some(Turn::AwaitingProvider(timing) | Turn::ProviderStarted(timing)) = &mut self.turn
        {
            if !timing.output_ids.iter().any(|existing| existing == id) {
                timing.output_ids.push(id.into());
            }
        }
    }

    pub(super) fn finish_turn(&mut self, ended_at: u64) {
        let Some(Turn::AwaitingProvider(timing) | Turn::ProviderStarted(timing)) = self.turn.take()
        else {
            return;
        };
        let Some(entry) = timing
            .output_ids
            .last()
            .and_then(|id| self.entries.iter_mut().find(|entry| entry.id == *id))
        else {
            return;
        };
        if entry.metadata["agent"].as_str().is_none() {
            if let Some(agent) = timing.agent {
                set_metadata(entry, "agent", agent);
            }
        }
        set_metadata(entry, "ended", ended_at);
        set_output_duration(entry, timing.started_at, ended_at);
    }
}
