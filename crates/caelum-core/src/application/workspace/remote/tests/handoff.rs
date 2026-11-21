use super::*;
use crate::application::{Attachment, HandoffPhase, HandoffSource, Provider, Scope, VmDestination};
use caelum_protocol::ProjectInfo;

fn source() -> Source {
    Source {
        base: "feature/local".into(),
        published_tip: Some("a".repeat(40)),
        ..super::source()
    }
}

fn local(app: &mut Workspace) -> usize {
    let project = source().project;
    app.projects.push(ProjectInfo {
        path: project.clone(),
        name: "Project".into(),
    });
    app.remote.inventory = Some(Inventory {
        account: "owner".into(),
        machines: vec![machine("vm")],
    });
    let i = app.new_session(Scope {
        project,
        provider: Provider::OpenCode,
    });
    let s = &mut app.sessions[i];
    s.remote_id = Some("local-session".into());
    s.draft = "Next step".into();
    s.attachments.push(Attachment::new(
        "local.rs".into(),
        "local attachment".into(),
        None,
        false,
    ));
    i
}

fn reading(app: &mut Workspace, i: usize) -> u64 {
    assert!(app.begin_vm_handoff(HandoffSource::Composer(i)));
    app.commands()
        .into_iter()
        .find_map(|(req, c)| {
            matches!(c, Command::HandoffChat(key) if key.id == "local-session").then_some(req)
        })
        .unwrap()
}

fn ready(app: &mut Workspace, req: u64) {
    app.apply(Event::Completed(
        req,
        Ok(Reply::HandoffChat(Chat {
            messages: vec![
                Message::User("Earlier question".into()),
                Message::Assistant("Earlier answer".into()),
            ],
            clipped: false,
        })),
    ));
    app.remote.picker = Some(Picker::Ready(source()));
}

fn new_vm() -> VmDestination {
    VmDestination::Create {
        size: Size::Small,
        lifetime: Lifetime::EightHours,
    }
}

#[test]
fn deletion_waits_for_the_confirmed_boat_target_and_retires_late_results() {
    let mut app = Workspace::default();
    let index = local(&mut app);
    let req = reading(&mut app, index);
    ready(&mut app, req);
    let id = app
        .finish_vm_handoff(VmDestination::Existing(machine("vm")))
        .unwrap();
    let req = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(prepared_vm(machine("vm"), "feature/local"))),
    ));
    app.remote
        .saved
        .expanded
        .insert(source().project.clone(), true);
    assert!(app.linked_vm(index).is_some());
    assert!(app.send_remote(&id, "Keep working"));
    let key = machine("vm").key;
    assert!(!app.remove_remote(&id, Some(&key)));
    let req = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(Outcome::Accepted(Run {
            provider: Provider::OpenCode,
            conversation: "c".into(),
            prompt: "p".into(),
            id: "r".into(),
        }))),
    ));
    let (feed, Command::Remote(Operation::Follow(..))) = app.commands().pop().unwrap() else {
        panic!()
    };
    app.monitor_remote(Some(&id), std::time::Instant::now());
    let (stats, Command::Remote(Operation::Stats(_))) = app.commands().pop().unwrap() else {
        panic!()
    };
    assert!(!app.remove_remote(&id, None));
    assert!(!app.remove_remote(&id, Some(&machine("other").key)));
    assert!(app.remove_remote(&id, Some(&key)));
    assert!(!app.remove_remote(&id, Some(&key)));
    let (r, Command::Remote(Operation::Delete(sent))) = app.commands().pop().unwrap() else {
        panic!()
    };
    assert_eq!(sent, key);
    app.apply(Event::Completed(r, Err("Access denied".into())));
    assert!(app.linked_vm(index).is_some() && app.remote.delete_errors.contains_key(&id));
    assert!(app.remove_remote(&id, Some(&key)));
    let (r, _) = app.commands().pop().unwrap();
    let Connection::Loading(list) = app.remote.connection else {
        panic!("handoff inventory must still be in flight")
    };
    let prior = app.remote.inventory.clone().unwrap();
    app.apply(Event::Completed(
        r,
        Ok(Reply::Remote(Outcome::Deleted(key))),
    ));
    app.apply(Event::Completed(
        list,
        Ok(Reply::Remote(Outcome::Inventory(prior))),
    ));
    assert!(app.remote.inventory.as_ref().unwrap().machines.is_empty());
    app.apply(Event::Completed(
        feed,
        Ok(Reply::Remote(Outcome::Page(Page {
            cursor: None,
            more: false,
            lines: vec![],
            status: RunStatus::Running,
        }))),
    ));
    app.apply(Event::Completed(stats, Err("obsolete sample".into())));
    assert!(app
        .commands()
        .iter()
        .all(|(_, c)| !matches!(c, Command::Remote(_))));
    assert!(matches!(app.remote.monitor, Monitor::Off));
    assert!(
        app.remote.feeds.is_empty()
            && app.remote.jobs.is_empty()
            && app.remote.saved.expanded.is_empty()
    );
    let saved = app.snapshot();
    assert!(saved.remote.checkouts.is_empty() && saved.remote.selected.is_none());
    assert!(saved.sessions[index].vm_link.is_none());
    assert_eq!(saved.sessions[index].draft, "Next step");
}

#[test]
fn a_second_handoff_cannot_repurpose_an_unfinished_new_vm_checkout() {
    let mut app = Workspace::default();
    let i = local(&mut app);
    let req = reading(&mut app, i);
    ready(&mut app, req);
    let id = app.finish_vm_handoff(new_vm()).unwrap();
    let (create, _) = app.commands().pop().unwrap();
    let m = machine("vm");
    app.apply(Event::Completed(
        create,
        Ok(Reply::Remote(Outcome::Machine(m.clone()))),
    ));
    let (prepare, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(prepare, Err("Lost setup response".into())));

    app.home_select_project(source().project);
    assert!(app.begin_vm_handoff(HandoffSource::HomeDraft));
    app.remote.picker = Some(Picker::Ready(Source {
        base: "other-branch".into(),
        ..source()
    }));
    assert!(app.finish_vm_handoff(VmDestination::Existing(m)).is_none());
    app.refresh_remote(&id);
    let commands = app.commands();
    assert!(commands.iter().any(|(_, c)| matches!(c, Command::Remote(Operation::Prepare(t, base, Some(branch)))
        if base == "feature/local" && branch == base && t.required_base.as_ref().is_some_and(|p| p.branch == *base))));
}

#[test]
fn confirming_checks_the_exact_worktree_again_and_a_late_or_changed_branch_cannot_start_a_vm() {
    let mut app = Workspace::default();
    let i = local(&mut app);
    let path = app.sessions[i].scope.project.clone();
    app.project_roots
        .insert(path.clone(), "/other/main-worktree".into());
    let request = reading(&mut app, i);
    ready(&mut app, request);
    let confirm = |app: &mut Workspace| {
        app.confirm_vm_handoff(new_vm());
        let cmds = app.commands();
        let [(req, Command::Remote(Operation::DiscoverHandoff(p)))] = cmds.as_slice() else {
            panic!("{cmds:?}")
        };
        assert_eq!(p, &path);
        *req
    };
    let old = confirm(&mut app);
    app.dismiss_vm_handoff();
    let req = reading(&mut app, i);
    ready(&mut app, req);
    let current = confirm(&mut app);
    app.apply(Event::Completed(
        old,
        Ok(Reply::Remote(Outcome::Source(source()))),
    ));
    assert!(!app.enter_confirmed_vm());
    app.apply(Event::Completed(
        current,
        Ok(Reply::Remote(Outcome::Source(Source {
            base: "switched".into(),
            ..source()
        }))),
    ));
    assert!(!app.enter_confirmed_vm());
    assert!(matches!(
        app.vm_handoff().unwrap().phase,
        HandoffPhase::Failed(_)
    ));
    assert!(app.remote.saved.checkouts.is_empty() && app.commands().is_empty());
    assert_eq!(app.selected, Some(i));
}

#[test]
fn restored_local_link_opens_its_own_parked_thread_and_timer_changes_do_not_touch_the_active_prompt(
) {
    use crate::application::HandoffSource;
    use std::time::Instant;
    let mut app = Workspace::default();
    let i = local(&mut app);
    let req = reading(&mut app, i);
    ready(&mut app, req);
    let id = app
        .finish_vm_handoff(VmDestination::Existing(machine("vm")))
        .unwrap();
    let link = app.linked_vm(i).unwrap().clone();
    let (req, _) = app.commands().pop().unwrap();
    let prepared = || prepared_vm(machine("vm"), "feature/local");
    app.apply(Event::Completed(req, Ok(Reply::Remote(prepared()))));
    app.home_select_project(source().project);
    assert!(app.begin_vm_handoff(HandoffSource::HomeDraft));
    app.remote.picker = Some(Picker::Ready(source()));
    app.finish_vm_handoff(VmDestination::Existing(machine("vm")))
        .unwrap();
    let saved = app.snapshot();
    let mut app = Workspace::default();
    app.enable_store("/unused-vm-links".into());
    let load = app.commands()[0].0;
    app.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(saved)))),
    ));
    assert_eq!(app.linked_vm(i), Some(&link));
    assert!(app.open_linked_vm(i));
    assert_eq!(
        app.remote.saved.checkouts[&id].thread.editor_key(&id),
        link.thread
    );
    // A stats panel can inspect this machine while the local session stays selected.
    app.remote.saved.selected = None;
    app.select(i);
    app.remote.jobs.clear();
    let run = Run {
        provider: Provider::OpenCode,
        conversation: "conv".into(),
        prompt: "prompt".into(),
        id: "run".into(),
    };
    app.remote.saved.checkouts.get_mut(&id).unwrap().thread.turn = Turn::Following(run.clone());
    app.monitor_remote(Some(&id), Instant::now());
    let Monitor::Loading { request: stale, .. } = app.remote.monitor else {
        panic!()
    };
    app.set_vm_lifetime(&id, Lifetime::Day);
    let Some(Job::Busy(req)) = app.remote.lifetimes.get(&id) else {
        panic!()
    };
    let req = *req;
    let mut m = machine("vm");
    m.auto_stop_unix = Some(86_400);
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(Outcome::Lifetime(m))),
    ));
    app.apply(Event::Completed(
        stale,
        Ok(Reply::Remote(Outcome::Stats(Sample {
            machine: machine("vm"),
            vcpu: None,
            memory_gb: None,
            activity: Activity::Unavailable,
        }))),
    ));
    let c = &app.remote.saved.checkouts[&id];
    assert_eq!(c.machine().unwrap().auto_stop_unix, Some(86_400));
    assert_eq!(c.thread.turn, Turn::Following(run));
    assert_eq!(app.selected, Some(i));
    assert!(app.remote.saved.selected.is_none());
}

#[test]
fn late_history_and_source_changes_cannot_provision_or_replace_a_newer_attempt() {
    let mut app = Workspace::default();
    let i = local(&mut app);
    let old = reading(&mut app, i);
    app.dismiss_vm_handoff();
    let current = reading(&mut app, i);
    ready(&mut app, old);
    assert!(matches!(app.vm_handoff().unwrap().phase, HandoffPhase::Reading(r) if r == current));
    assert!(app.finish_vm_handoff(new_vm()).is_none());
    app.leave_conversation();
    ready(&mut app, current);
    assert!(app.finish_vm_handoff(new_vm()).is_none());
    app.select(i);
    let req = reading(&mut app, i);
    app.apply(Event::Completed(req, Err("unavailable".into())));
    assert!(matches!(
        app.vm_handoff().unwrap().phase,
        HandoffPhase::Failed(_)
    ));
    assert!(app.finish_vm_handoff(new_vm()).is_none());
    let req = reading(&mut app, i);
    ready(&mut app, req);
    app.remote.picker = Some(Picker::Ready(Source {
        project: "/different".into(),
        ..source()
    }));
    assert!(app.finish_vm_handoff(new_vm()).is_none());
    app.sessions[i].conversation.working = true;
    assert!(!app.begin_vm_handoff(HandoffSource::Composer(i)));
    assert!(app.remote.saved.checkouts.is_empty() && app.commands().is_empty());
    assert_eq!(app.sessions[i].draft, "Next step");
}

#[test]
fn handoff_first_send_survives_rejection_and_restore_without_replaying_chat_after_ack() {
    let mut app = Workspace::default();
    let i = local(&mut app);
    app.sessions[i].scope.provider = Provider::Codex;
    app.sessions[i].options.model = Some("local-model".into());
    app.sessions[i].options.effort = Some("local-effort".into());
    let req = reading(&mut app, i);
    ready(&mut app, req);
    app.sessions[i].draft = "New laptop draft".into();
    let id = app.finish_vm_handoff(new_vm()).unwrap();
    assert_eq!(app.sessions[i].draft, "New laptop draft");
    assert_eq!(app.sessions[i].attachments.len(), 1);
    let creates = app.commands();
    let [(req, Command::Remote(Operation::Create(source, original)))] = creates.as_slice() else {
        panic!()
    };
    assert_eq!(original.branch, "feature/local");
    assert_eq!(original.branch, source.base);
    app.apply(Event::Completed(*req, Err("network offline".into())));
    assert!(app.commands().is_empty());
    let mut reopened = Workspace::default();
    reopened.remote.saved =
        serde_json::from_value(serde_json::to_value(app.snapshot().remote).unwrap()).unwrap();
    reopened.select_remote(&id);
    let (retry, Command::Remote(Operation::Create(_, plan))) = reopened.commands().pop().unwrap()
    else {
        panic!()
    };
    assert_eq!(&plan, original);
    let complete = |app: &mut Workspace, req, result| {
        app.apply(Event::Completed(req, Ok(Reply::Remote(result))))
    };
    complete(&mut reopened, retry, Outcome::Machine(machine("vm")));
    let (prepare, Command::Remote(Operation::Prepare(target, base, Some(branch)))) =
        reopened.commands().pop().unwrap()
    else {
        panic!()
    };
    assert_eq!(branch, plan.branch);
    assert_eq!(base, branch);
    assert_eq!(
        target.required_base.unwrap().tip,
        source.published_tip.as_deref().unwrap()
    );
    complete(&mut reopened, prepare, prepared_vm(machine("vm"), branch));
    assert!(reopened.commands().is_empty());
    assert_eq!(
        reopened.remote.saved.checkouts[&id].thread.draft,
        "Next step"
    );
    let thread = &reopened.remote.saved.checkouts[&id].thread;
    assert!(thread.model.is_none() && thread.effort.is_none());
    let editor = thread.editor_key(&id).to_owned();
    reopened.remote.models = ModelCatalogue::Ready(
        [(
            Provider::Codex,
            [caelum_protocol::ProviderModelChoice {
                id: "remote-model".into(),
                name: "Remote model".into(),
                provider: None,
                context_window: None,
                efforts: vec![caelum_protocol::ProviderChoice {
                    id: "high".into(),
                    name: "High".into(),
                }],
                default_effort: None,
            }]
            .into(),
        )]
        .into(),
    );
    assert!(reopened.set_vm_model(&id, &editor, Provider::Codex, Some("remote-model")));
    assert!(reopened.set_vm_effort(&id, &editor, Provider::Codex, "remote-model", Some("high")));
    assert!(reopened.commands().is_empty());
    for reject in [true, false] {
        assert!(reopened.send_remote(&id, "Edited next step"));
        let (req, Command::Remote(Operation::Prompt(target, turn))) =
            reopened.commands().pop().unwrap()
        else {
            panic!()
        };
        assert_eq!(target.machine.id, "vm");
        assert_eq!(turn.provider, Provider::Codex);
        assert_eq!(turn.model.as_deref(), Some("remote-model"));
        assert_eq!(turn.effort.as_deref(), Some("high"));
        assert!(turn.conversation.is_none());
        let prompt = turn.text;
        assert!(
            prompt.contains("Earlier question")
                && prompt.contains("Earlier answer")
                && prompt.ends_with("Edited next step")
        );
        assert!(!prompt.contains("local attachment") && !prompt.contains("local-session"));
        assert_eq!(
            reopened.remote.saved.checkouts[&id]
                .thread
                .lines
                .last()
                .unwrap()
                .text,
            "Edited next step"
        );
        if reject {
            complete(&mut reopened, req, Outcome::Rejected("try again".into()));
            assert_eq!(
                reopened.remote.saved.checkouts[&id].thread.draft,
                "Edited next step"
            );
        } else {
            let saved = reopened.snapshot().remote;
            reopened = Workspace::default();
            reopened.remote.saved =
                serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap();
            assert_eq!(
                reopened.remote.saved.checkouts[&id].thread.model.as_deref(),
                Some("remote-model")
            );
            assert_eq!(
                reopened.remote.saved.checkouts[&id]
                    .thread
                    .effort
                    .as_deref(),
                Some("high")
            );
            assert!(!reopened.send_remote(&id, "duplicate"));
            reopened.recover_remote(&id, "p");
            let (req, Command::Remote(Operation::Recover(_, Provider::Codex, None, _))) =
                reopened.commands().pop().unwrap()
            else {
                panic!()
            };
            complete(
                &mut reopened,
                req,
                Outcome::Accepted(Run {
                    provider: Provider::Codex,
                    conversation: "boat-thread".into(),
                    id: "r".into(),
                    prompt: "p".into(),
                }),
            );
            let (req, _) = reopened.commands().pop().unwrap();
            complete(
                &mut reopened,
                req,
                Outcome::Page(Page {
                    lines: vec![],
                    cursor: None,
                    more: false,
                    status: RunStatus::Ended(TurnEnd::Finished),
                }),
            );
            assert!(reopened.set_vm_model(&id, &editor, Provider::Codex, None));
            assert!(reopened.send_remote(&id, "Again"));
            assert!(
                matches!(reopened.commands().as_slice(), [(_, Command::Remote(Operation::Prompt(_, p)))] if p.provider == Provider::Codex && p.model.is_none() && p.effort.is_none() && p.conversation.as_deref() == Some("boat-thread") && p.text == "Again")
            );
        }
    }
}

#[test]
fn home_can_continue_on_an_existing_idle_machine_without_overwriting_its_thread() {
    let mut app = Workspace::default();
    let i = local(&mut app);
    let m = machine("vm");
    app.remote.picker = Some(Picker::Ready(source()));
    let id = app.attach_remote(m.clone(), None).unwrap();
    let (req, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(prepared_vm(m.clone(), "existing"))),
    ));
    let original = Thread {
        conversation: Some("original".into()),
        draft: "VM draft".into(),
        ..Default::default()
    };
    app.remote.saved.checkouts.get_mut(&id).unwrap().thread = original.clone();
    app.home_select_project(source().project);
    app.home.draft.text = "From Home".into();
    app.home_set_provider(Provider::Codex);
    assert!(app.begin_vm_handoff(HandoffSource::HomeDraft));
    app.home_select_project(source().project);
    app.remote.picker = Some(Picker::Ready(source()));
    assert!(app
        .finish_vm_handoff(VmDestination::Existing(m.clone()))
        .is_none());
    assert!(app.begin_vm_handoff(HandoffSource::HomeDraft));
    app.remote.picker = Some(Picker::Ready(source()));
    let destination = || VmDestination::Existing(m.clone());
    app.remote.saved.checkouts.get_mut(&id).unwrap().thread.turn = Turn::Submitting;
    assert!(app.finish_vm_handoff(destination()).is_none());
    app.remote.saved.checkouts.get_mut(&id).unwrap().thread.turn = Turn::Idle;
    assert_eq!(app.finish_vm_handoff(destination()), Some(id.clone()));
    assert_eq!(app.home.draft.text, "From Home");
    assert_eq!(app.sessions[i].draft, "Next step");
    let checkout = &app.remote.saved.checkouts[&id];
    assert_eq!(checkout.other_threads, std::slice::from_ref(&original));
    assert_eq!(checkout.thread.draft, "From Home");
    assert!(checkout.thread.conversation.is_none());
    let new_key = checkout.thread.editor_key(&id).to_owned();
    let (req, _) = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Remote(Operation::Prepare(..))))
        .unwrap();
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(prepared_vm(m, "existing"))),
    ));
    assert!(app.switch_vm_thread(&id, &id));
    assert_eq!(app.remote.saved.checkouts[&id].thread, original);
    assert!(app.switch_vm_thread(&id, &new_key));
    assert!(app.send_remote(&id, "From Home"));
    let commands = app.commands();
    let [(
        req,
        Command::Remote(Operation::Prompt(
            _,
            Prompt {
                provider: Provider::Codex,
                conversation: None,
                model: None,
                effort: None,
                text: prompt,
            },
        )),
    )] = commands.as_slice()
    else {
        panic!()
    };
    assert_eq!(prompt, "From Home");
    assert!(!app.switch_vm_thread(&id, &id));
    app.apply(Event::Completed(*req, Err("lost acknowledgement".into())));
    app.recover_remote(&id, "old-prompt");
    let (req, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        req,
        Ok(Reply::Remote(Outcome::Accepted(Run {
            provider: Provider::Codex,
            conversation: "original".into(),
            prompt: "old-prompt".into(),
            id: "old-run".into(),
        }))),
    ));
    assert!(app.commands().is_empty());
    assert!(app.remote.saved.checkouts[&id]
        .thread
        .conversation
        .is_none());
    assert_eq!(
        app.remote.saved.checkouts[&id].thread.turn,
        Turn::Submitting
    );
}
