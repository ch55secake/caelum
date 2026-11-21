use super::{Command, Desktop, Reply, SessionKey};
use crate::application::{Task, TaskReply};
use caelum_protocol::ProjectSearchMode;

pub(super) fn checkout(desktop: &mut Desktop, key: &SessionKey) {
    let project = &key.scope.project;
    let task = |desktop: &mut Desktop, t| match desktop.request(Command::Task(t)) {
        Reply::Task(reply) => reply,
        _ => panic!("expected a checkout reply"),
    };
    let listing = task(
        desktop,
        Task::Directory {
            project: project.clone(),
            path: String::new(),
            offset: 0,
        },
    );
    assert!(
        matches!(listing,TaskReply::Directory(d) if d.entries.iter().any(|f|f.name=="note.txt"))
    );
    let note = task(
        desktop,
        Task::File {
            project: project.clone(),
            path: "note.txt".into(),
        },
    );
    assert!(matches!(note,TaskReply::File(f) if f.content=="VM edit\n"));
    assert!(desktop
        .try_request(Command::Task(Task::File {
            project: project.clone(),
            path: "outside/endpoint.json".into()
        }))
        .unwrap_err()
        .contains("outside the VM checkout"));
    let results = task(
        desktop,
        Task::Search {
            project: project.clone(),
            query: "VM edit".into(),
            mode: ProjectSearchMode::Content,
        },
    );
    assert!(matches!(results,TaskReply::Search(r) if r.matches.iter().any(|m|m.path=="note.txt")));
    let git = task(
        desktop,
        Task::Git {
            project: project.clone(),
        },
    );
    assert!(
        matches!(git,TaskReply::Git(g,_) if g.branch.as_deref()==Some("feat/native") && g.changes.iter().any(|f|f.path=="note.txt"))
    );
    let diff = task(
        desktop,
        Task::Diff {
            project: project.clone(),
            path: "note.txt".into(),
        },
    );
    assert!(matches!(diff,TaskReply::Diff(d) if d.unstaged.contains("+VM edit")));
    assert!(matches!(
        task(
            desktop,
            Task::Discover {
                project: project.clone(),
                branch: "feat/native".into()
            }
        ),
        TaskReply::Discovered(None)
    ));
    assert!(
        matches!(desktop.request(Command::Shells(key.clone())),Reply::Shells(s) if s[0].id=="sh_example")
    );
    let output = task(
        desktop,
        Task::Shell {
            project: project.clone(),
            session: key.id.clone(),
            id: "sh_example".into(),
            stop: true,
        },
    );
    assert!(matches!(output,TaskReply::Shell(s) if s.ends_with("latest café output")));
    assert!(
        matches!(desktop.request(Command::Shells(key.clone())),Reply::Shells(s) if s.is_empty())
    );
}
