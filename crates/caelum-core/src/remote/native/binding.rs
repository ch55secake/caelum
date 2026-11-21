use super::{commands, NativeTarget};
use crate::application::{Command, Event, Reply, Task, TaskReply};

/// The saved checkout ID names a Boat account/machine/repository. A VM path alone
/// cannot distinguish two machines (or a local checkout with the same path).
pub fn project_key(checkout: &str) -> String {
    format!("boat://checkout/{checkout}")
}
pub fn checkout_id(project: &str) -> Option<&str> {
    project.strip_prefix("boat://checkout/")
}

pub struct Binding {
    pub project: String,
    root: String,
}

impl Binding {
    pub fn new(checkout: &str, target: &NativeTarget) -> Result<Self, String> {
        Ok(Self {
            project: project_key(checkout),
            root: target.root()?,
        })
    }

    /// Route before consulting the VM allowlist, so unsupported VM commands
    /// can never fall back to the laptop. Global desktop commands have no owner.
    pub fn owner(command: &mut Command) -> Option<&mut String> {
        match command {
            Command::Open(p) => Some(p),
            Command::List(s)
            | Command::ListArchived(s)
            | Command::Choices(s)
            | Command::Start(s, _)
            | Command::Commands(s) => Some(&mut s.project),
            Command::Inbox(k)
            | Command::Enqueue(k, ..)
            | Command::RemoveQueued(k, _)
            | Command::Generate(k, _)
            | Command::RegenerateTitle(k)
            | Command::Rename(k, _)
            | Command::CheckpointFiles(k, ..)
            | Command::Export(k, ..)
            | Command::Rewind { key: k, .. }
            | Command::Shells(k)
            | Command::Archive(k, _)
            | Command::DeleteSession(k)
            | Command::Details(k)
            | Command::Fork(k, _)
            | Command::EditBranch(k, ..)
            | Command::Resume(k)
            | Command::History(k, _)
            | Command::HandoffChat(k)
            | Command::CodexControlHistory(k, _)
            | Command::Send(k, ..)
            | Command::Cancel(k)
            | Command::VerifyOpenCodeMove { from: k, .. } => Some(&mut k.scope.project),
            Command::Approve(p, _) => Some(&mut p.project_path),
            Command::Answer(p, _) => Some(&mut p.project_path),
            Command::Task(t) => t.project_mut(),
            Command::Remote(
                crate::remote::Operation::Discover(p)
                | crate::remote::Operation::DiscoverHandoff(p)
                | crate::remote::Operation::MatchCheckout(p, _),
            ) => Some(p),
            Command::Remote(_)
            | Command::Notify(_)
            | Command::Sound(_)
            | Command::Load(_)
            | Command::Save(..) => None,
        }
    }

    pub fn outgoing(&self, mut command: Command) -> Result<Command, String> {
        let owner = Self::owner(&mut command)
            .filter(|p| **p == self.project)
            .ok_or("This command belongs to another checkout.")?;
        owner.clone_from(&self.root);
        if !commands::admitted(&command, &self.root) {
            return Err("This action is not available on the VM checkout.".into());
        }
        if let Command::Task(Task::Worktrees { directory, .. }) = &mut command {
            directory.clear();
        }
        Ok(command)
    }

    fn bind(&self, path: &mut String) -> Result<(), String> {
        if path != &self.root {
            return Err("The VM returned a different checkout.".into());
        }
        path.clone_from(&self.project);
        Ok(())
    }

    /// Translate only typed owner fields. Prompt text, paths in tool output and
    /// provider IDs are data and must survive the trip literally.
    pub fn incoming(&self, mut event: Event) -> Option<Event> {
        match &mut event {
            Event::Completed(_, Ok(reply)) => {
                if let Err(e) = self.reply(reply) {
                    if let Event::Completed(_, result) = &mut event {
                        *result = Err(e);
                    }
                }
            }
            Event::Completed(_, Err(_)) => {}
            Event::Checkout(path) => self.bind(path).ok()?,
            Event::Branch(b) => self.bind(&mut b.project_path).ok()?,
            Event::Codex(e) => self.bind(&mut e.project_path).ok()?,
            Event::OpenCode(e) => {
                self.bind(&mut e.project_path).ok()?;
                if e.event_type == "session.moved" {
                    return None;
                }
            }
            Event::Approvals(items) => items.retain_mut(|p| self.bind(&mut p.project_path).is_ok()),
            Event::Questions(items) => items.retain_mut(|p| self.bind(&mut p.project_path).is_ok()),
            Event::UpdateProgress(..) => return None,
        }
        Some(event)
    }

    fn reply(&self, reply: &mut Reply) -> Result<(), String> {
        match reply {
            Reply::Project(p) => self.bind(&mut p.path)?,
            Reply::Sessions(list) => {
                for s in list {
                    self.bind(&mut s.key.scope.project)?;
                }
            }
            Reply::Session(s) | Reply::Resumed(s, _) => self.bind(&mut s.key.scope.project)?,
            Reply::Forked { source, child } | Reply::EditableBranch { source, child, .. } => {
                self.bind(&mut source.scope.project)?;
                self.bind(&mut child.key.scope.project)?;
            }
            Reply::SessionMoved(_) => {
                return Err("This session moved outside its VM checkout.".into())
            }
            Reply::Task(TaskReply::Git(status, updates)) => {
                self.bind(&mut status.project_path)?;
                if let Some(root) = &mut status.repository_root {
                    self.bind(root)?;
                }
                updates.retain_mut(|u| self.bind(&mut u.project_path).is_ok());
            }
            // Sibling paths reported by Git on a VM are not laptop projects.
            Reply::Task(TaskReply::Worktrees(list)) => *list = None,
            Reply::Task(TaskReply::Worktree(_)) | Reply::Remote(_) | Reply::Snapshot(_) => {
                return Err("Unexpected VM response.".into())
            }
            _ => {}
        }
        Ok(())
    }
}
