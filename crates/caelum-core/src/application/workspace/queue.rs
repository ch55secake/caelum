use super::{Pending, Workspace};
use crate::application::{Command, Provider};

impl Workspace {
    pub fn resume_queue(&mut self, index: usize) {
        self.sessions[index].queue_paused = false;
        self.sessions[index].error = None;
        if !self.sessions[index].loaded || self.sessions[index].disconnected {
            self.refresh(index);
        }
        self.advance_queue(index);
    }

    pub fn refresh_inbox(&mut self, index: usize) {
        let session = &self.sessions[index];
        if session.scope.provider != Provider::OpenCode
            || self
                .pending
                .values()
                .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index))
        {
            return;
        }
        if let Some(key) = session.key() {
            self.request(
                Command::Inbox(key),
                Pending::Inbox(index, session.inbox.revision),
            );
        }
    }

    pub fn retry_queued(&mut self, index: usize, id: &str) {
        if self.sessions[index]
            .inbox
            .items
            .iter()
            .any(|item| item.id == id && item.removing)
        {
            self.remove_queued(index, id);
            return;
        }
        let session = &mut self.sessions[index];
        let Some(key) = session.key() else {
            return;
        };
        let Some(item) = session
            .inbox
            .items
            .iter_mut()
            .find(|v| v.id == id && !v.pending)
        else {
            return;
        };
        item.pending = true;
        item.error = None;
        let text = item.text.clone();
        session.inbox.revision += 1;
        self.request(
            Command::Enqueue(key, id.into(), text),
            Pending::Enqueue(index, id.into()),
        );
    }

    pub fn remove_queued(&mut self, index: usize, id: &str) {
        let session = &mut self.sessions[index];
        let Some(key) = session.key() else {
            return;
        };
        let Some(item) = session
            .inbox
            .items
            .iter_mut()
            .find(|v| v.id == id && !v.pending)
        else {
            return;
        };
        item.pending = true;
        item.removing = true;
        item.error = None;
        session.inbox.revision += 1;
        self.request(
            Command::RemoveQueued(key, id.into()),
            Pending::RemoveQueued(index, id.into()),
        );
    }

    pub(super) fn advance_queue(&mut self, index: usize) {
        if self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Inbox(i, _) if *i == index))
        {
            return;
        }
        let session = &mut self.sessions[index];
        if !session.queue_paused
            && session.inbox.items.is_empty()
            && session.can_send()
            && !session.queue.is_empty()
            && session.error.is_none()
        {
            let queued = session.queue.remove(0);
            let options = std::mem::replace(&mut session.options, queued.options);
            let dirty = session.options_dirty;
            self.send_raw(index, queued.text);
            self.sessions[index].inflight_queued = true;
            self.sessions[index].options = options;
            self.sessions[index].options_dirty =
                dirty || self.sessions[index].options != self.sessions[index].inflight_options;
        }
    }
}
