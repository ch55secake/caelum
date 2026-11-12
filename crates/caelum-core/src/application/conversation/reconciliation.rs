//! Merge provider history snapshots with the conversation's live stream.
use std::collections::{BTreeMap, HashSet};

use super::super::AttachmentKind;
use super::{Conversation, Entry, EntryKind};

pub(super) fn merge(conversation: &mut Conversation, history: Vec<Entry>, older: bool) {
    let mut live = std::mem::take(&mut conversation.entries);
    let ids: HashSet<_> = history.iter().map(|entry| entry.id.as_str()).collect();
    // A repeated prompt may occur in old history. Only its newest occurrence
    // in a refresh can adopt this client's pending optimistic submission.
    let optimistic = if older {
        None
    } else {
        live.iter()
            .position(|entry| entry.optimistic)
            .and_then(|index| {
                history
                    .iter()
                    .rposition(|entry| optimistic_matches(entry, &live[index]))
                    .map(|history_index| (index, history_index))
            })
    };
    let optimistic_context =
        optimistic.map(|(live_index, history_index)| (history_index, live[live_index].clone()));
    let insertion = live
        .iter()
        .position(|entry| ids.contains(entry.id.as_str()))
        .or_else(|| optimistic.map(|(index, _)| index))
        .unwrap_or(if older { 0 } else { live.len() });
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut existing = BTreeMap::new();
    for (index, entry) in live.drain(..).enumerate() {
        if ids.contains(entry.id.as_str()) {
            existing.insert(entry.id.clone(), entry);
        } else if optimistic.is_none_or(|(adopted, _)| adopted != index) {
            if index < insertion {
                before.push(entry);
            } else {
                after.push(entry);
            }
        }
    }
    for (index, mut snapshot) in history.into_iter().enumerate() {
        if let Some((history_index, optimistic)) = &optimistic_context {
            if *history_index == index {
                preserve_optimistic_context(&mut snapshot, optimistic);
            }
        }
        let entry = match existing.remove(&snapshot.id) {
            Some(current) => merge_entry(snapshot, current),
            None => snapshot_entry(snapshot),
        };
        before.push(entry);
    }
    before.extend(after);
    conversation.entries = before;
}

fn optimistic_matches(history: &Entry, optimistic: &Entry) -> bool {
    if history.kind != EntryKind::User || optimistic.kind != EntryKind::User {
        return false;
    }
    if history.text == optimistic.text {
        return true;
    }
    let (history_text, _) = super::super::split_context(&history.text);
    let (optimistic_text, attachments) = super::super::split_context(&optimistic.text);
    if history_text != optimistic_text
        || !attachments
            .iter()
            .any(|attachment| attachment.kind == AttachmentKind::Image)
    {
        return false;
    }
    let recent = history
        .started_at
        .zip(optimistic.started_at)
        .is_some_and(|(history, submitted)| history >= submitted);
    let image_matches = attachments
        .iter()
        .filter(|attachment| attachment.kind == AttachmentKind::Image)
        .all(|attachment| {
            history
                .media
                .iter()
                .any(|media| media.source == attachment.content || media.name == attachment.path)
        });
    recent || (history.started_at.is_none() && image_matches)
}

fn preserve_optimistic_context(history: &mut Entry, optimistic: &Entry) {
    let (history_text, mut attachments) = super::super::split_context(&history.text);
    let (_, optimistic_attachments) = super::super::split_context(&optimistic.text);
    for attachment in optimistic_attachments {
        if attachments
            .iter()
            .any(|existing| existing.id == attachment.id)
        {
            continue;
        }
        if attachment.kind == AttachmentKind::Image
            && history
                .media
                .iter()
                .any(|media| media.source == attachment.content || media.name == attachment.path)
        {
            continue;
        }
        attachments.push(attachment);
    }
    history.text = super::super::composer::with_context(history_text, &attachments);
}

fn snapshot_entry(mut entry: Entry) -> Entry {
    entry.history = entry.text.clone();
    entry.history_output = entry.output.clone();
    entry
}

fn overlay_metadata(target: &mut serde_json::Value, source: &serde_json::Value) {
    let Some(source) = source.as_object() else {
        return;
    };
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    let target = target.as_object_mut().expect("metadata was made an object");
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn merge_missing_metadata(target: &mut serde_json::Value, fallback: &serde_json::Value) {
    let Some(fallback) = fallback.as_object() else {
        return;
    };
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    let target = target.as_object_mut().expect("metadata was made an object");
    for (key, value) in fallback {
        target.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

fn merge_entry(mut snapshot: Entry, mut current: Entry) -> Entry {
    snapshot.started_at = snapshot.started_at.or(current.started_at);
    overlay_metadata(&mut snapshot.metadata, &current.metadata);
    if snapshot.complete {
        if snapshot.kind == EntryKind::Tool && snapshot.output.is_empty() {
            snapshot.output = current.output;
        }
        if snapshot.kind == EntryKind::Compaction {
            if snapshot.text.is_empty() {
                snapshot.text = current.text;
            }
            if snapshot.error.is_empty() && snapshot.status == current.status {
                snapshot.error = current.error;
            }
        }
        return snapshot_entry(snapshot);
    }
    if current.complete {
        current.started_at = current.started_at.or(snapshot.started_at);
        merge_missing_metadata(&mut current.metadata, &snapshot.metadata);
        return current;
    }
    current.history = snapshot.text;
    current.text = reconcile(&current.history, &current.stream);
    // OpenCode publishes whole output snapshots, while Codex also publishes
    // output deltas. Retain either kind when a slower history page arrives.
    let live_output = if current.stream_output.is_empty() {
        &current.output
    } else {
        &current.stream_output
    };
    current.history_output = snapshot.output;
    current.output = reconcile(&current.history_output, live_output);
    if current.name.is_empty() {
        current.name = snapshot.name;
    }
    if current.input.is_empty() {
        current.input = snapshot.input;
    }
    current
}

/// Linear-time snapshot/live overlap, including a stream that lags the snapshot.
pub(super) fn reconcile(snapshot: &str, stream: &str) -> String {
    if stream.is_empty() || snapshot.contains(stream) {
        return snapshot.into();
    }
    if snapshot.is_empty() || stream.contains(snapshot) {
        return stream.into();
    }
    let pattern: Vec<_> = stream.chars().collect();
    let mut failure = vec![0; pattern.len()];
    let mut matched = 0;
    for index in 1..pattern.len() {
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = failure[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
        }
        failure[index] = matched;
    }
    matched = 0;
    for character in snapshot.chars() {
        while matched > 0 && (matched == pattern.len() || character != pattern[matched]) {
            matched = failure[matched - 1];
        }
        if character == pattern[matched] {
            matched += 1;
        }
    }
    let mut result = snapshot.to_owned();
    result.extend(pattern[matched..].iter());
    result
}
