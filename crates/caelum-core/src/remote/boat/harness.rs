use super::{api, Boat};
use crate::application::Provider;
use crate::remote::{harness::Harness, Outcome, Page, Prompt, Run, RunStatus, Target, TurnEnd};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use boat_sdk::agents::{
    models::{EventsResponse, PromptRun, SandboxEvent},
    EventOrder, EventQuery, PromptRequest,
};

// In particular disallow commas: the events API interprets these as multiple conversations.
pub(super) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}

fn owned(target: &Target, run: &PromptRun, id: &str, conv: Option<&str>) -> bool {
    run.sandbox_id == target.machine.id
        && run.prompt_id == id
        && valid_id(&run.id)
        && valid_id(id)
        && run
            .conversation_id
            .as_deref()
            .is_none_or(|c| valid_id(c) && conv.is_none_or(|v| c == v))
}

pub(super) fn position(event: &SandboxEvent) -> Option<String> {
    Some(URL_SAFE_NO_PAD.encode(format!("{}:{}", event.timestamp?, event.id.as_ref()?)))
}

fn checkpoint(
    page: &EventsResponse,
    run: &Run,
    cursor: Option<&str>,
    more: bool,
    done: bool,
) -> Result<Option<String>, String> {
    let Some((last, earlier)) = page.events.split_last() else {
        return if more {
            Err("Boat returned an empty intermediate event page".into())
        } else {
            Ok(cursor.map(str::to_owned))
        };
    };
    // The latest response is a snapshot. Content or tools can change without
    // a new (timestamp, id). At the live tail, overlap just this keyed event.
    if !done
        && !more
        && last.id.as_deref().is_some_and(super::events::event_id)
        && super::events::response(last, &run.conversation, &run.prompt).is_some()
    {
        match earlier.last() {
            None => return Ok(cursor.map(str::to_owned)),
            Some(prev) => {
                if let Some(before) = position(prev) {
                    return Ok(Some(before));
                }
            }
        }
    }
    page.page_info
        .as_ref()
        .and_then(|p| p.next_cursor.clone())
        .or_else(|| position(last))
        .filter(|next| !next.is_empty() && Some(next.as_str()) != cursor)
        .map(Some)
        .ok_or_else(|| "Invalid Boat event cursor".into())
}

impl Boat {
    pub(in crate::remote) async fn recover(
        &self,
        target: &Target,
        provider: Provider,
        conversation: Option<&str>,
        prompt: &str,
    ) -> Result<Run, String> {
        self.owns(&target.machine)?;
        if !valid_id(prompt) {
            return Err("Enter the Boat prompt ID for this turn".into());
        }
        let status = self
            .client
            .prompt_status(&target.machine.id, prompt)
            .await
            .map_err(api)?;
        let run = status.prompt_run;
        if status.id != target.machine.id || !owned(target, &run, prompt, conversation) {
            return Err("That turn belongs to a different checkout or conversation".into());
        }
        let conv = run
            .conversation_id
            .ok_or("Boat did not return a conversation for this prompt")?;
        let list = self
            .client
            .conversations(&target.machine.id)
            .await
            .map_err(api)?;
        let Some(chat) = list.conversations.iter().find(|c| c.id == conv) else {
            return Err("Boat did not list this prompt’s conversation.".into());
        };
        if list.id != target.machine.id || chat.last_harness.as_deref() != Some(provider.as_str()) {
            return Err("That conversation does not match the provider used for this turn.".into());
        }
        // lastHarness describes only the latest prompt, not every turn in a mixed-provider chat.
        if chat.prompts != 1 {
            if conversation.is_none() || chat.prompts < 1 {
                return Err("Choose the first prompt of the new Boat conversation.".into());
            }
            let latest = self
                .client
                .events(
                    &target.machine.id,
                    &EventQuery {
                        conversation: Some(conv.clone()),
                        types: Some("prompt".into()),
                        sort: Some(EventOrder::Desc),
                        limit: Some(1),
                        ..Default::default()
                    },
                )
                .await
                .map_err(api)?;
            if latest.id != target.machine.id
                || !latest.events.first().is_some_and(|e| {
                    e.kind == "prompt"
                        && e.conversation_id.as_deref().is_none_or(|c| c == conv)
                        && e.task_id
                            .as_ref()
                            .or(e.id.as_ref())
                            .is_some_and(|id| id == prompt)
                })
            {
                return Err("That is not the latest prompt in this Boat conversation.".into());
            }
        }
        Ok(Run {
            provider,
            conversation: conv,
            prompt: prompt.into(),
            id: run.id,
        })
    }
}

impl Harness for Boat {
    async fn send(&self, target: &Target, prompt: &Prompt) -> Result<Outcome, String> {
        let provider = prompt.provider;
        let conversation = prompt.conversation.as_deref();
        if conversation.is_some_and(|c| !valid_id(c)) {
            return Ok(Outcome::Rejected("Invalid remote conversation".into()));
        }
        let worktree = match self.worktree(target, None).await {
            Ok(w) => w,
            Err(e) => return Ok(Outcome::Rejected(e)),
        };
        if let Err(error) = worktree.verify_base().await {
            return Ok(Outcome::Rejected(error));
        }
        let request = PromptRequest {
            provider: Some(provider.as_str().into()),
            model: prompt.model.clone(),
            reasoning_effort: prompt.effort.clone(),
            new_conversation: conversation.is_none().then_some(true),
            conversation_id: conversation.map(str::to_owned),
            ..PromptRequest::new(&prompt.text)
        };
        let response = match self.client.prompt(&target.machine.id, &request).await {
            Err(boat_sdk::Error::Api { status, code, .. })
                if status.is_client_error() && status.as_u16() != 408 =>
            {
                return Ok(Outcome::Rejected(
                    if code.as_deref() == Some("provider_not_configured") {
                        format!("Configure {} in Boat → Agents & Proxying.", provider.name())
                    } else {
                        format!("Boat declined the prompt: HTTP {status}")
                    },
                ));
            }
            other => other.map_err(api)?,
        };
        let conv = response
            .conversation_id
            .or_else(|| response.prompt_run.conversation_id.clone())
            .ok_or("Missing Boat conversation ID")?;
        if response.id != target.machine.id
            || response.provider != provider.as_str()
            || !valid_id(&conv)
            || conversation.is_some_and(|c| c != conv)
            || !owned(
                target,
                &response.prompt_run,
                &response.prompt_id,
                Some(&conv),
            )
        {
            return Err("Boat returned a different machine, conversation or harness".into());
        }
        Ok(Outcome::Accepted(Run {
            provider,
            conversation: conv,
            prompt: response.prompt_id,
            id: response.prompt_run.id,
        }))
    }

    async fn follow(
        &self,
        target: &Target,
        run: &Run,
        cursor: Option<&str>,
    ) -> Result<Page, String> {
        self.owns(&target.machine)?;
        if !valid_id(&run.conversation) || !valid_id(&run.prompt) {
            return Err("Invalid saved Boat turn".into());
        }
        // Read status first so completion cannot race the final page of events.
        let status = self
            .client
            .prompt_status(&target.machine.id, &run.prompt)
            .await
            .map_err(api)?;
        if status.id != target.machine.id
            || status.prompt_run.id != run.id
            || !owned(
                target,
                &status.prompt_run,
                &run.prompt,
                Some(&run.conversation),
            )
        {
            return Err("Boat returned a different prompt run".into());
        }
        let events = self
            .client
            .events(
                &target.machine.id,
                &EventQuery {
                    conversation: Some(run.conversation.clone()),
                    cursor: cursor.map(str::to_owned),
                    sort: Some(EventOrder::Asc),
                    limit: Some(100),
                    ..Default::default()
                },
            )
            .await
            .map_err(api)?;
        if events.id != target.machine.id {
            return Err("Boat returned events for a different machine".into());
        }
        let more = events.page_info.as_ref().is_some_and(|p| p.has_more);
        let next = checkpoint(&events, run, cursor, more, status.prompt_run.done)?;
        let status = match (
            status.prompt_run.done,
            more,
            status.prompt_run.status.as_str(),
        ) {
            (true, true, _) => RunStatus::Finishing,
            (true, false, s) => RunStatus::Ended(match s {
                "finished" => TurnEnd::Finished,
                "interrupted" => TurnEnd::Interrupted,
                "failed" => TurnEnd::Failed,
                _ => TurnEnd::Other,
            }),
            (false, _, "queued" | "sending") => RunStatus::Queued,
            _ => RunStatus::Running,
        };
        Ok(Page {
            lines: super::events::lines(events.events, run, target.root.as_deref(), cursor),
            cursor: next,
            more,
            status,
        })
    }

    async fn interrupt(&self, target: &Target, run: &Run) -> Result<(), String> {
        self.owns(&target.machine)?;
        if !valid_id(&run.conversation) {
            return Err("Cannot interrupt without the exact conversation ID".into());
        }
        let response = self
            .client
            .interrupt(&target.machine.id, Some(&run.conversation))
            .await
            .map_err(api)?;
        if response.id != target.machine.id {
            return Err("Boat returned a different machine".into());
        }
        Ok(())
    }
}
