use super::{is_image, Pending, Workspace};
use crate::application::{Attachment, Command, Provider, Task};

impl Workspace {
    pub fn options_changed(&mut self, index: usize) {
        self.sessions[index].options_dirty = true;
    }

    pub fn set_effort(&mut self, index: usize, effort: Option<String>) {
        let session = &mut self.sessions[index];
        // OpenCode treats None as keeping the current variant, and "" as a reset.
        session.options.effort =
            effort.or_else(|| (session.scope.provider == Provider::OpenCode).then(String::new));
        self.options_changed(index);
    }

    pub fn attach(&mut self, index: usize, attachment: Attachment) -> bool {
        self.sessions[index].attachments.retain(|f| {
            !(f.kind == attachment.kind
                && if f.kind == crate::application::AttachmentKind::Image {
                    f.content == attachment.content
                } else {
                    f.path == attachment.path
                        && f.start_line == attachment.start_line
                        && f.end_line == attachment.end_line
                })
        });
        if self.sessions[index].attachments.len() >= 16 {
            self.sessions[index].attachment_error(
                attachment.path,
                "Attach at most 16 items per message".into(),
            );
            return false;
        }
        self.sessions[index]
            .attachment_errors
            .retain(|(path, _)| *path != attachment.path);
        self.sessions[index].attachments.push(attachment);
        true
    }

    /// Pending reads for this draft, in request order. Side-question completions
    /// have their own presentation and must not appear in the main composer.
    pub fn attachment_reads(&self, index: usize) -> Vec<(u64, &str)> {
        let mut reads: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(request, pending)| match pending {
                Pending::Attachment(owner, name)
                | Pending::Mention {
                    index: owner,
                    name,
                    side: false,
                    ..
                } if *owner == index => Some((*request, name.as_str())),
                _ => None,
            })
            .collect();
        reads.sort_unstable_by_key(|(request, _)| *request);
        reads
    }

    pub fn attach_file(&mut self, index: usize, path: String) {
        if self.sessions[index].attaching + self.sessions[index].attachments.len() >= 16 {
            self.sessions[index]
                .attachment_error(path, "Attach at most 16 items per message".into());
            return;
        }
        self.sessions[index]
            .attachment_errors
            .retain(|(name, _)| *name != path);
        self.sessions[index].attaching += 1;
        let project = self.sessions[index].scope.project.clone();
        let name = path.clone();
        let task = if is_image(&path) {
            Task::Image {
                project,
                source: path,
            }
        } else {
            Task::File { project, path }
        };
        self.request(Command::Task(task), Pending::Attachment(index, name));
    }

    pub fn attach_image(&mut self, index: usize, name: String, bytes: Vec<u8>) {
        if self.sessions[index].attaching + self.sessions[index].attachments.len() >= 16 {
            self.sessions[index]
                .attachment_error(name, "Attach at most 16 items per message".into());
            return;
        }
        self.sessions[index]
            .attachment_errors
            .retain(|(path, _)| *path != name);
        self.sessions[index].attaching += 1;
        let task = Task::EncodeImage {
            project: self.sessions[index].scope.project.clone(),
            bytes,
        };
        self.request(Command::Task(task), Pending::Attachment(index, name));
    }

    pub fn complete_mention(
        &mut self,
        index: usize,
        token: crate::application::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        self.mention(index, false, token, path, project, replacement);
    }

    pub fn complete_side_mention(
        &mut self,
        index: usize,
        token: crate::application::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        self.mention(index, true, token, path, project, replacement);
    }

    fn mention(
        &mut self,
        index: usize,
        side: bool,
        token: crate::application::Completion,
        path: String,
        project: String,
        replacement: String,
    ) {
        let session = &mut self.sessions[index];
        if session.attaching > 0 {
            return;
        }
        let draft = if side {
            &session.side_draft
        } else {
            &session.draft
        };
        if token.start > token.end || draft.get(token.start..token.end).is_none() {
            return;
        }
        session.attaching += 1;
        if side {
            session.side_error = None;
        } else {
            session.attachment_errors.retain(|(name, _)| *name != path);
        }
        session.completion_generation += 1;
        let pending = Pending::Mention {
            index,
            side,
            name: path.clone(),
            generation: session.completion_generation,
            draft: draft.clone(),
            token: token.clone(),
            replacement,
        };
        let task = if token.kind == crate::application::CompletionKind::PullRequest {
            Task::AttachPullRequest { project, url: path }
        } else {
            Task::File { project, path }
        };
        self.request(Command::Task(task), pending);
    }

    pub fn dismiss_side_question(&mut self, index: usize) {
        self.cancel_mentions(index);
        let session = &mut self.sessions[index];
        session.side_generation += 1;
        session.completion_generation += 1;
        session.generating = false;
        session.side_question = None;
        session.side_error = None;
        session.side_draft.clear();
        session.side_attachments.clear();
    }

    pub fn cancel_mentions(&mut self, index: usize) {
        let requests: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(id, p)| {
                matches!(p,Pending::Mention {index:i,..} if *i==index).then_some(*id)
            })
            .collect();
        for id in &requests {
            self.pending.remove(id);
        }
        self.outgoing.retain(|(id, _)| !requests.contains(id));
        self.sessions[index].attaching = self.sessions[index]
            .attaching
            .saturating_sub(requests.len());
        self.sessions[index].completion_generation += 1;
    }

    pub fn ask(&mut self, index: usize, prompt: String) -> bool {
        let session = &mut self.sessions[index];
        if session.scope.provider != Provider::OpenCode
            || session.generating
            || prompt.trim().is_empty()
        {
            return false;
        }
        let Some(key) = session.key() else {
            session.error = Some("Start an OpenCode session before asking a side question".into());
            return false;
        };
        session.generating = true;
        session.side_error = None;
        session.side_generation += 1;
        let generation = session.side_generation;
        session.side_draft = prompt.clone();
        session.side_question = Some((prompt.clone(), String::new()));
        let context =
            crate::application::composer::with_context(&prompt, &session.side_attachments);
        self.request(
            Command::Generate(key, context),
            Pending::Generate(index, prompt, generation),
        );
        true
    }
}
