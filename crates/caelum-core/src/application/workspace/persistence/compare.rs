//! Compare the persisted projection by reference. Public editor/preferences fields
//! can change without a reducer event, so an event-only dirty flag is insufficient.
use super::super::{Session, Workspace};
use crate::application::{persistence::FileWorkspace, Preferences, SavedSession, Snapshot};
use std::collections::{BTreeMap, HashMap};

#[cfg(test)]
mod tests;

impl Workspace {
    pub(super) fn matches_snapshot(&self, snapshot: &Snapshot) -> bool {
        // Exhaustive patterns keep additions to the storage contract visible here.
        let Snapshot {
            version,
            projects,
            sessions,
            selected,
            preferences,
            remembered,
            remembered_checkouts,
            managed_worktrees,
            project_roots,
        } = snapshot;
        let retained = || self.sessions.iter().filter(|s| !s.forgotten);
        let saved_index = |index: usize| {
            self.sessions.get(index).filter(|s| !s.forgotten).map(|_| {
                if self.sessions.len() == sessions.len() {
                    index
                } else {
                    self.sessions[..index]
                        .iter()
                        .filter(|s| !s.forgotten)
                        .count()
                }
            })
        };
        let remembered_matches =
            |current: &HashMap<String, usize>, saved: &BTreeMap<String, usize>| {
                let mut projected = current
                    .iter()
                    .filter_map(|(key, index)| saved_index(*index).map(|index| (key, index)));
                projected.clone().count() == saved.len()
                    && projected.all(|(key, index)| saved.get(key) == Some(&index))
            };
        *version == 1
            && self.projects == *projects
            && retained().count() == sessions.len()
            && retained()
                .zip(sessions)
                .all(|(s, saved)| s.matches_saved(saved))
            && self.selected.and_then(saved_index) == *selected
            && self.managed_worktrees == *managed_worktrees
            && self.project_roots == *project_roots
            && remembered_matches(&self.remembered, remembered)
            && remembered_matches(&self.remembered_checkouts, remembered_checkouts)
            && self.matches_preferences(preferences)
    }

    fn matches_preferences(&self, saved: &Preferences) -> bool {
        let Preferences {
            files,
            theme,
            theme_family,
            accent_color,
            session_scope,
            default_provider,
            worktree_directory,
            worktree_expanded,
            providers,
            favorite_models,
            notifications,
            notification_mode,
            sound_volume,
            background_shells,
            input_needed,
        } = &self.preferences;
        if *theme != saved.theme
            || *theme_family != saved.theme_family
            || *accent_color != saved.accent_color
            || *session_scope != saved.session_scope
            || *default_provider != saved.default_provider
            || *worktree_directory != saved.worktree_directory
            || *worktree_expanded != saved.worktree_expanded
            || *providers != saved.providers
            || *favorite_models != saved.favorite_models
            || *notifications != saved.notifications
            || *notification_mode != saved.notification_mode
            || *sound_volume != saved.sound_volume
            || *background_shells != saved.background_shells
            || *input_needed != saved.input_needed
        {
            return false;
        }
        // snapshot() overlays live tabs/selection on remembered file state, and
        // creates a default file workspace for each nonempty checkout path.
        if files.keys().any(|path| !saved.files.contains_key(path))
            || self
                .services
                .checkouts
                .keys()
                .any(|path| !path.is_empty() && !saved.files.contains_key(path))
        {
            return false;
        }
        let empty = FileWorkspace::default();
        saved.files.iter().all(|(path, saved)| {
            let checkout = self
                .services
                .checkouts
                .get(path)
                .filter(|_| !path.is_empty());
            let current = files.get(path);
            if current.is_none() && checkout.is_none() {
                return false;
            }
            let FileWorkspace {
                scroll,
                tabs,
                selected,
                expanded,
                line,
            } = current.unwrap_or(&empty);
            *scroll == saved.scroll
                && *expanded == saved.expanded
                && *line == saved.line
                && checkout.map_or(tabs, |c| &c.tabs) == &saved.tabs
                && checkout.map_or(selected, |c| &c.selected_file) == &saved.selected
        })
    }
}

impl Session {
    fn matches_saved(&self, saved: &SavedSession) -> bool {
        let SavedSession {
            last_activity,
            scope,
            remote_id,
            title,
            title_refresh_turns,
            options,
            draft,
            attachments,
            failed_prompt,
            pending_submission,
            queue,
            inbox,
            archived,
            reading,
            unread,
        } = saved;
        self.last_activity == *last_activity
            && self.scope == *scope
            && self.remote_id == *remote_id
            && self.title == *title
            && self.title_state.turns == *title_refresh_turns
            && self.options == *options
            && self.draft == *draft
            && self.attachments == *attachments
            && self.failed_prompt == *failed_prompt
            && match (&self.inflight_prompt, pending_submission) {
                (Some(text), Some(prompt)) => {
                    *text == prompt.text && self.inflight_options == prompt.options
                }
                (None, None) => true,
                _ => false,
            }
            && self.queue == *queue
            && self.inbox.items == *inbox
            && self.archived == *archived
            && self.reading == *reading
            && self.unread == *unread
    }
}
