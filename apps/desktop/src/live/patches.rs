//! Patch inputs alone determine preview content. Output, timing and status events
//! should not reformat tool details or disturb a native diff's selection/scroll.
use super::Live;
use caelum_core::application::{Entry, EntryKind, presentation::Tool};
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

type Key = (usize, String);
const MAX_ENTRIES: usize = 48;
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Inputs {
    values: HashMap<Key, Source>,
    bytes: usize,
}

// These are the inputs to Tool::from_entry's patch and paths projection. Cache
// negative results as well; shell output and ordinary text need no preview work.
struct Source {
    position: usize,
    kind: EntryKind,
    name: String,
    input: String,
}

impl Source {
    fn bytes(&self) -> usize {
        self.name.len() + self.input.len()
    }
}

impl Inputs {
    fn unchanged(&mut self, key: &Key, position: usize, entry: &Entry) -> bool {
        let Some(cached) = self.values.get_mut(key) else {
            return false;
        };
        cached.position = position;
        cached.kind == entry.kind && cached.name == entry.name && cached.input == entry.input
    }

    fn remove(&mut self, key: &Key) {
        if let Some(source) = self.values.remove(key) {
            self.bytes -= key.1.len() + source.bytes();
        }
    }

    fn remember(&mut self, key: &Key, position: usize, entry: &Entry) {
        self.remove(key);
        let bytes = key
            .1
            .len()
            .saturating_add(entry.name.len())
            .saturating_add(entry.input.len());
        if self.values.len() < MAX_ENTRIES && bytes <= MAX_BYTES - self.bytes {
            self.values.insert(
                key.clone(),
                Source {
                    position,
                    kind: entry.kind,
                    name: entry.name.clone(),
                    input: entry.input.clone(),
                },
            );
            self.bytes += bytes;
        }
    }

    fn retain(&mut self, visible: &HashSet<Key>) {
        self.values.retain(|key, source| {
            if visible.contains(key) {
                true
            } else {
                self.bytes -= key.1.len() + source.bytes();
                false
            }
        });
    }
}

impl Live {
    pub(super) fn sync_patches(&mut self) -> bool {
        self.patch_inputs.retain(&self.visible_entries);
        let mut changed = false;
        for key @ (index, id) in &self.visible_entries {
            if self.collapsed_patches.contains(key) {
                continue;
            }
            let position = self
                .patch_inputs
                .values
                .get(key)
                .map(|source| source.position);
            let entry = self.app.sessions.get(*index).and_then(|session| {
                let entries = &session.conversation.entries;
                // Older history can prepend entries; live/history reconciliation
                // can also replace or remove them. Never trust a cached slot's ID.
                position
                    .and_then(|i| entries.get(i).filter(|e| &e.id == id).map(|e| (i, e)))
                    .or_else(|| entries.iter().enumerate().find(|(_, e)| &e.id == id))
            });
            let Some((position, entry)) = entry else {
                self.patch_inputs.remove(key);
                changed |= self.patches.remove(key).is_some();
                continue;
            };
            if self.patch_inputs.unchanged(key, position, entry) {
                continue;
            }
            let tool = Tool::from_entry(entry, 0);
            if tool.patch.is_empty() {
                changed |= self.patches.remove(key).is_some();
            } else {
                changed |= self.patches.entry(key.clone()).or_default().patch(
                    tool.paths.first().map(String::as_str).unwrap_or("Patch"),
                    &tool.patch,
                );
            }
            // Oversized inputs remain correct through the uncached path. Keep
            // empty patch results too, so incomplete arguments aren't reparsed.
            self.patch_inputs.remember(key, position, entry);
        }
        if self.patches.len() > MAX_ENTRIES {
            self.patches
                .retain(|key, _| self.visible_entries.contains(key));
        }
        changed
    }
}
