use super::{default_prompt, environment, fixture, sandbox};
use crate::application::Provider;
use crate::remote::{
    harness::Harness, GitAction, Host, MachineId, Machines, Outcome, Plan, PublishedBase, Size,
    Source, Target,
};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};
use wiremock::{
    matchers::{body_json, header, method, path},
    Mock, MockServer, Request, ResponseTemplate,
};

fn git(dir: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8_lossy(&result.stdout)
        .trim_end()
        .to_owned()
}

fn clone_vm(vm: &Path, published: &Path) {
    fs::create_dir_all(vm.parent().unwrap()).unwrap();
    git(
        vm.parent().unwrap(),
        &[
            "clone",
            "--single-branch",
            published.to_str().unwrap(),
            vm.to_str().unwrap(),
        ],
    );
    git(
        vm,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/example/project.git",
        ],
    );
}

async fn serve_checkout(server: &MockServer, workdir: &Path, published: &Path) {
    environment(server).await;
    Mock::given(method("GET"))
        .and(path("/sandboxes/vm-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"ok":true,"type":"info","sandbox":sandbox("idle")})),
        )
        .mount(server)
        .await;
    let wd = workdir.to_owned();
    let home = wd.join(".fixture-home");
    fs::create_dir(&home).unwrap();
    let upstream = published.to_owned();
    let bin = wd.join(".fixture-bin");
    fs::create_dir(&bin).unwrap();
    crate::test_support::write_executable(
        &bin.join("git"),
        r#"#!/bin/sh
export PATH="$FIXTURE_PATH"
case "$1" in clone|fetch|push) set -- -c "url.$UPSTREAM.insteadOf=https://github.com/example/project.git" "$@";; esac
exec git "$@"
"#,
    );
    let original = std::env::var_os("PATH").unwrap();
    let path_with_bin =
        std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&original))).unwrap();
    Mock::given(method("POST")).and(path("/sandboxes/vm-1/commands")).and(header("authorization","Bearer fixture-key"))
        .respond_with(move |req:&Request| {
            let body:Value=req.body_json().unwrap();
            let script=body["command"].as_str().unwrap();
            let result=Command::new("/bin/sh").arg("-c").arg(script).current_dir(&wd)
                .env("HOME", &home).env("UPSTREAM", &upstream).env("GIT_CONFIG_GLOBAL","/dev/null").env("GIT_CONFIG_NOSYSTEM","1")
                .env("PATH", &path_with_bin).env("FIXTURE_PATH", &original).env("GIT_ALLOW_PROTOCOL","file")
                .env("GIT_AUTHOR_NAME","Fixture").env("GIT_AUTHOR_EMAIL","fixture@example.com")
                .env("GIT_COMMITTER_NAME","Fixture").env("GIT_COMMITTER_EMAIL","fixture@example.com").output().unwrap();
            ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"exec","success":result.status.success(),"exitCode":result.status.code(),
                "timedOut":false,"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)}))
        }).mount(server).await;
}

#[tokio::test]
async fn only_new_vm_setup_fills_a_missing_checkout_and_retry_uses_the_verified_directory() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("vm");
    let upstream = temp.path().join("published");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&upstream).unwrap();
    git(&upstream, &["init", "-b", "main"]);
    git(&upstream, &["commit", "--allow-empty", "-m", "published"]);
    let (server, boat) = fixture().await;
    serve_checkout(&server, &home, &upstream).await;
    let target = Target {
        root: None,
        ..super::agent_target()
    };
    let dest = home.join("project");
    assert!(boat
        .prepare(&target, "main", None)
        .await
        .unwrap_err()
        .contains("No checkout"));
    assert!(
        !dest.exists(),
        "attaching must not populate an existing machine"
    );
    assert!(boat
        .prepare(&target, "unpublished", Some("topic"))
        .await
        .unwrap_err()
        .contains("Could not clone"));
    assert!(
        !dest.exists(),
        "a failed clone must leave a retryable destination"
    );
    fs::create_dir(&dest).unwrap();
    assert!(boat.prepare(&target, "main", Some("topic")).await.is_err());
    assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
    fs::remove_dir(&dest).unwrap();

    let Outcome::Prepared { root, branch, .. } =
        boat.prepare(&target, "main", Some("topic")).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(Path::new(&root), dest.canonicalize().unwrap());
    assert_eq!(branch, "topic");
    assert_eq!(
        git(&dest, &["rev-parse", "HEAD"]),
        git(&upstream, &["rev-parse", "main"])
    );
    fs::write(dest.join("draft"), "uncommitted").unwrap();
    let bound = Target {
        root: Some(root),
        ..target.clone()
    };
    let second = home.join("other clone");
    clone_vm(&second, &upstream);
    assert!(boat
        .prepare(&target, "main", None)
        .await
        .unwrap_err()
        .contains("More than one"));
    assert!(
        matches!(boat.prepare(&bound, "main", None).await.unwrap(), Outcome::Prepared { branch, .. } if branch=="topic")
    );

    fs::rename(&dest, home.join(".parked")).unwrap();
    assert!(boat
        .prepare(&bound, "main", Some("topic"))
        .await
        .unwrap_err()
        .contains("missing at"));
    assert!(
        !dest.exists(),
        "a pinned checkout must not be replaced or rebound"
    );
    fs::rename(&second, &dest).unwrap();
    git(
        &dest,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/another/project.git",
        ],
    );
    fs::rename(home.join(".parked"), &second).unwrap();
    assert!(boat
        .prepare(&target, "main", Some("topic"))
        .await
        .unwrap_err()
        .contains("does not match"));
    assert_eq!(git(&dest, &["branch", "--show-current"]), "main");
    assert_eq!(
        fs::read_to_string(second.join("draft")).unwrap(),
        "uncommitted"
    );
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|r| r.method.as_str() == "GET" || r.url.path() == "/sandboxes/vm-1/commands"));
}

#[tokio::test]
async fn an_environment_write_denial_can_be_corrected_and_the_same_create_retried() {
    let (server, boat) = fixture().await;
    let plan = Plan {
        token: uuid::Uuid::new_v4().to_string(),
        branch: "feat/new".into(),
        size: Size::Small,
        lifetime: crate::remote::Lifetime::UntilStopped,
    };
    let name = format!("caelum-{}", plan.token);
    let environment = |id: &str, name: &str, is_default: bool| {
        json!({"id":id,"name":name,"isDefault":is_default,
        "safeForThirdParties":false,"passGithub":true,"passSecrets":false,"passSandboxCredentials":false,
        "passAgentsCredentials":true,"envContents":"","secretFiles":[],"versions":[],"selectedRepositories":[]})
    };
    Mock::given(method("GET"))
        .and(path("/environments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "environments":[environment("shared","base",true)]
        })))
        .mount(&server)
        .await;
    Mock::given(path("/repos")).and(wiremock::matchers::query_param("q","example/project"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"repos","environmentId":"shared",
            "installations":[{"repositories":[{"fullName":"other/project","databaseId":"wrong"},{"fullName":"example/project","databaseId":"repo-1"}]}],"selectedRepositories":[]}))).mount(&server).await;
    Mock::given(method("PUT")).and(path("/environments/dedicated")).and(body_json(json!({
        "safeForThirdParties":false,"passGithub":true,"passSecrets":false,"passSandboxCredentials":false,
        "passAgentsCredentials":true,"repositories":[{"repositoryId":"repo-1","baseBranch":"main"}]
    }))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true}))).expect(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/sandboxes")).and(header("idempotency-key",plan.token.as_str()))
        .and(wiremock::matchers::body_partial_json(json!({"environment":name})))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({"ok":true,"type":"create","status":"provisioning","sandbox":sandbox("provisioning")}))).expect(1).mount(&server).await;
    let source = Source {
        published_tip: None,
        project: "/local/project".into(),
        account: boat.account.clone(),
        repository: "example/project".into(),
        base: "main".into(),
        environment: None,
    };
    let denied = Mock::given(method("POST"))
        .and(path("/environments"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error":{"code":"api_key_action_forbidden","message":"private error detail"},
            "requestId":"req_env01"
        })))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let error = boat.create(&source, &plan).await.unwrap_err();
    for expected in [
        "Create checkout environment",
        "403",
        "api_key_action_forbidden",
        "environment.write",
        "req_env01",
    ] {
        assert!(error.contains(expected), "{error}");
    }
    assert!(!error.contains("private error detail"));
    assert!(!server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|r| r.url.path() == "/sandboxes"));
    drop(denied);
    Mock::given(method("POST"))
        .and(path("/environments"))
        .and(body_json(json!({"name":name})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"environments":[environment("dedicated",&name,false)]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert!(boat
        .create(&source, &plan)
        .await
        .unwrap_err()
        .contains("not confirmed"));
    let mut configured = environment("dedicated", &name, false);
    configured["selectedRepositories"] =
        json!([{"fullName":"example/project","baseBranch":"main"}]);
    Mock::given(method("GET"))
        .and(path("/environments"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"environments":[configured]})),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    assert_eq!(boat.create(&source, &plan).await.unwrap().key.id, "vm-1");
    let requests = server.received_requests().await.unwrap();
    let create = requests
        .iter()
        .find(|r| r.url.path() == "/sandboxes" && r.method.as_str() == "POST")
        .unwrap();
    assert_eq!(
        create.body_json::<Value>().unwrap().get("ttlSeconds"),
        Some(&Value::Null)
    );
}

#[tokio::test]
async fn runs_opencode_on_the_session_worktree_branch_and_publishes_only_the_reviewed_vm_tree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let laptop = root.join("laptop/project");
    let vm = root.join("vm/Boat's checkout");
    fs::create_dir_all(&laptop).unwrap();
    git(root, &["init", "--bare", "-b", "main", "published.git"]);
    git(&laptop, &["init", "-b", "main"]);
    fs::write(laptop.join("README"), "Published\n").unwrap();
    git(&laptop, &["add", "README"]);
    git(&laptop, &["commit", "-m", "initial"]);
    let published = root.join("published.git");
    git(&laptop, &["push", published.to_str().unwrap(), "main"]);
    git(
        &laptop,
        &["remote", "add", "origin", published.to_str().unwrap()],
    );
    let worktree = root.join("session-worktree");
    git(
        &laptop,
        &[
            "worktree",
            "add",
            "-b",
            "feat/source",
            worktree.to_str().unwrap(),
            "main",
        ],
    );
    git(
        &worktree,
        &["commit", "--allow-empty", "-m", "Published session commit"],
    );
    git(&worktree, &["push", "origin", "feat/source"]);
    let published_head = git(&worktree, &["rev-parse", "HEAD"]);
    git(
        &worktree,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "Unpublished session commit",
        ],
    );
    let git_client = crate::git::GitClient::new();
    let base = git_client
        .handoff_base(worktree.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(
        (&base.branch[..], &base.tip),
        ("feat/source", &published_head)
    );
    clone_vm(&vm, &published);
    git(&laptop, &["switch", "-c", "laptop-only"]);
    assert!(git_client
        .handoff_base(laptop.to_str().unwrap())
        .await
        .unwrap_err()
        .contains("Publish laptop-only"));
    fs::write(laptop.join("README"), "Unpublished laptop work\n").unwrap();
    let laptop_before = git(&laptop, &["diff"]);
    let (server, boat) = fixture().await;
    serve_checkout(&server, vm.parent().unwrap(), &published).await;
    let plan = Plan {
        token: uuid::Uuid::new_v4().to_string(),
        branch: base.branch.clone(),
        size: Size::Small,
        lifetime: crate::remote::Lifetime::Day,
    };
    Mock::given(method("POST"))
        .and(path("/sandboxes"))
        .and(header("idempotency-key", plan.token.as_str()))
        .and(wiremock::matchers::body_partial_json(
            json!({"ttlSeconds":86400}),
        ))
        .respond_with(ResponseTemplate::new(202).set_body_json(
            json!({"ok":true,"type":"create","status":"provisioning","sandbox":sandbox("cloning")}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let agent_dir = vm.clone();
    Mock::given(method("POST")).and(path("/sandboxes/vm-1/prompt")).and(body_json(json!({"provider":"opencode","new":true,"prompt":"Update the README"})))
        .respond_with(move |_:&Request| {
            assert_eq!(git(&agent_dir,&["branch","--show-current"]),"feat/source");
            fs::write(agent_dir.join("README"),"Reviewed remote edit\n").unwrap();
            fs::write(agent_dir.join("notes.txt"),"New remote file\n").unwrap();
            ResponseTemplate::new(202).set_body_json(json!({"ok":true,"type":"queued","id":"vm-1","promptId":"p","conversationId":"c",
                "promptRun":{"id":"r","promptId":"p","sandboxId":"vm-1","status":"queued","done":false},"status":"queued","provider":"opencode"}))
        }).expect(1).mount(&server).await;
    let source = Source {
        published_tip: Some(base.tip.clone()),
        project: worktree.to_string_lossy().into(),
        account: "user-1".into(),
        repository: "example/project".into(),
        base: base.branch.clone(),
        environment: Some("project-env".into()),
    };
    let mismatch = boat.create(&source, &plan).await.unwrap_err();
    assert!(mismatch.contains("feat/source"));
    assert!(!server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|r| r.url.path() == "/sandboxes"));
    super::environment_on(&server, "feat/source", 1).await;
    let machine = boat.create(&source, &plan).await.unwrap();
    let newer = git(
        &published,
        &[
            "commit-tree",
            &format!("{published_head}^{{tree}}"),
            "-p",
            &published_head,
            "-m",
            "Another contributor advances origin",
        ],
    );
    git(
        &published,
        &[
            "update-ref",
            "refs/heads/feat/source",
            &newer,
            &published_head,
        ],
    );
    let target = Target {
        root: None,
        required_base: Some(base),
        machine: machine.key,
        repository: source.repository,
    };
    let err = boat.prepare(&target, &source.base, None).await.unwrap_err();
    assert!(
        err.contains("feat/source")
            && err.contains("VM checkout is on main")
            && git(&vm, &["branch", "--show-current"]) == "main"
    );
    assert!(matches!(
        boat.send(
            &target,
            &default_prompt(Provider::OpenCode, None, "Wrong branch")
        )
        .await
        .unwrap(),
        Outcome::Rejected(_)
    ));
    let Outcome::Prepared { branch, root, .. } = boat
        .prepare(&target, &source.base, Some(&plan.branch))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(branch, "feat/source");
    assert_eq!(Path::new(&root), vm.canonicalize().unwrap());
    let target = Target {
        root: Some(root),
        ..target
    };
    assert_eq!(git(&vm, &["rev-parse", "HEAD"]), published_head);
    assert_eq!(
        git(&vm, &["rev-parse", "--abbrev-ref", "@{upstream}"]),
        "origin/feat/source"
    );
    assert!(matches!(
        boat.send(
            &target,
            &default_prompt(Provider::OpenCode, None, "Update the README")
        )
        .await
        .unwrap(),
        Outcome::Accepted(_)
    ));
    let review = boat.git(&target, &GitAction::Read).await.unwrap();
    assert!(
        review.patch.contains("+Reviewed remote edit") && review.patch.contains("+New remote file")
    );
    assert!(git(&vm, &["diff", "--cached"]).is_empty());
    fs::write(vm.join("README"), "Later edit\n").unwrap();
    let committed = boat
        .git(
            &target,
            &GitAction::Commit(review.revision.clone(), "feat: developer's edit".into()),
        )
        .await
        .unwrap();
    assert!(committed.patch.contains("+Later edit"));
    assert!(boat
        .git(&target, &GitAction::Commit(review.revision, "stale".into()))
        .await
        .is_err());
    assert!(boat
        .git(&target, &GitAction::Push(committed.revision))
        .await
        .is_err());
    assert_eq!(git(&published, &["rev-parse", "feat/source"]), newer);
    git(&vm, &["merge", "--no-edit", &newer]);
    let merged = boat.git(&target, &GitAction::Read).await.unwrap();
    boat.git(&target, &GitAction::Push(merged.revision))
        .await
        .unwrap();
    assert_eq!(
        git(&published, &["show", "feat/source:README"]),
        "Reviewed remote edit"
    );
    assert_eq!(
        git(&published, &["show", "feat/source:notes.txt"]),
        "New remote file"
    );
    assert_eq!(git(&laptop, &["branch", "--show-current"]), "laptop-only");
    assert_eq!(git(&laptop, &["diff"]), laptop_before);
    assert!(!laptop.join("notes.txt").exists());
    git(&worktree, &["switch", "--detach"]);
    assert!(git_client
        .handoff_base(worktree.to_str().unwrap())
        .await
        .unwrap_err()
        .contains("named branch"));
}

#[tokio::test]
async fn a_precloned_session_branch_opens_at_the_pin_and_a_prepare_retry_preserves_vm_work() {
    let temp = tempfile::tempdir().unwrap();
    let published = temp.path().join("upstream");
    fs::create_dir(&published).unwrap();
    git(&published, &["init", "-b", "main"]);
    git(&published, &["commit", "--allow-empty", "-m", "Confirmed"]);
    let tip = git(&published, &["rev-parse", "HEAD"]);
    git(
        &published,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "Published during provisioning",
        ],
    );
    let newer = git(&published, &["rev-parse", "HEAD"]);
    let vm = temp.path().join("vm/project");
    clone_vm(&vm, &published);
    let (server, boat) = fixture().await;
    serve_checkout(&server, vm.parent().unwrap(), &published).await;
    let target = Target {
        root: None,
        machine: MachineId {
            host: Host::Boat,
            account: boat.account.clone(),
            id: "vm-1".into(),
        },
        repository: "example/project".into(),
        required_base: Some(PublishedBase {
            branch: "main".into(),
            tip: tip.clone(),
        }),
    };
    let open = || boat.prepare(&target, "main", Some("main"));
    git(
        &vm,
        &["commit", "--allow-empty", "-m", "VM-only setup work"],
    );
    let vm_only = git(&vm, &["rev-parse", "HEAD"]);
    assert!(open().await.is_err());
    assert_eq!(git(&vm, &["rev-parse", "HEAD"]), vm_only);
    // A saved plan from before same-branch handoffs still opens its named branch.
    assert!(
        matches!(boat.prepare(&target, "main", Some("legacy/topic")).await.unwrap(), Outcome::Prepared { branch, .. } if branch == "legacy/topic")
    );
    assert_eq!(git(&vm, &["rev-parse", "HEAD"]), tip);
    assert_eq!(git(&vm, &["rev-parse", "main"]), vm_only);

    git(&vm, &["branch", "-m", "main", "setup-work"]);
    git(&vm, &["switch", "--track", "origin/main"]);
    assert!(matches!(open().await.unwrap(), Outcome::Prepared { branch, .. } if branch == "main"));
    assert_eq!(git(&vm, &["rev-parse", "HEAD"]), tip);

    git(&vm, &["merge", "--ff-only", &newer]);
    fs::write(vm.join("draft"), "Staged on the VM").unwrap();
    git(&vm, &["add", "draft"]);
    assert!(matches!(open().await.unwrap(), Outcome::Prepared { branch, .. } if branch == "main"));
    assert_eq!(git(&vm, &["rev-parse", "HEAD"]), newer);
    assert_eq!(git(&vm, &["show", ":draft"]), "Staged on the VM");
}
