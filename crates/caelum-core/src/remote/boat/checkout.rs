use super::{setup_error::Step, worktree::Worktree, Boat};
use crate::remote::{
    checkout::repo_name, valid_branch, Machine, MachineId, Outcome, Plan, Size, Source, Target,
};
use boat_sdk::{
    environments::models::*,
    sandbox::{requests::CreateSandboxRequest, resources::Sandbox, MachineSize},
};

fn selected_repo<'a>(e: &'a SandboxEnvironment, repo: &str) -> Option<&'a SelectedRepository> {
    let [selected] = e.selected_repositories.as_deref()? else {
        return None;
    };
    selected
        .full_name
        .as_deref()
        .filter(|name| {
            !e.safe_for_third_parties
                && e.pass_github
                && e.pass_agents_credentials
                && name.eq_ignore_ascii_case(repo)
        })
        .map(|_| selected)
}

fn clones(e: &SandboxEnvironment, repo: &str, branch: &str) -> bool {
    selected_repo(e, repo).and_then(|r| r.base_branch.as_deref()) == Some(branch)
}
pub(in crate::remote) fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// Boat may have already cloned this branch, even at a newer published commit.
// Only reposition a published ref on a newly created VM. Record completion on
// its disk so a lost response can be retried without moving the branch again.
const OPEN_SESSION_BRANCH: &str = r#"set -e
ref="refs/heads/$branch"
upstream="refs/remotes/origin/$branch"
receipt=refs/caelum/handoff-start
if git show-ref --verify --quiet "$receipt"; then
    test "$(git rev-parse "$receipt")" = "$tip"
    test "$(git branch --show-current)" = "$branch"
else
    git fetch --no-tags origin "+$ref:$upstream"
    git merge-base --is-ancestor "$tip" FETCH_HEAD
    if git show-ref --verify --quiet "$ref"; then
        git merge-base --is-ancestor "$ref" FETCH_HEAD
    fi
    git switch --no-overwrite-ignore -C "$branch" "$tip"
    if ! git branch --set-upstream-to="$upstream" "$branch"; then
        # A single-branch clone may not have a fetch mapping for this branch.
        git remote set-branches --add origin "$branch"
        git branch --set-upstream-to="$upstream" "$branch"
    fi
    git update-ref "$receipt" "$tip" ""
fi"#;

impl Boat {
    pub(in crate::remote) async fn discover(&self, project: &str) -> Result<Source, String> {
        let git = crate::git::GitClient::new();
        let repository = git.github_origin(project)?;
        let published = git.handoff_base(project).await?;
        let environment = self
            .client
            .environments(None)
            .await
            .map_err(|e| Step::ListEnvironments.error(e))?
            .environments
            .into_iter()
            .find(|e| clones(e, &repository, &published.branch))
            .map(|e| e.name);
        Ok(Source {
            project: project.to_owned(),
            account: self.account.clone(),
            repository,
            base: published.branch,
            published_tip: Some(published.tip),
            environment,
        })
    }

    async fn environment(&self, source: &Source, plan: &Plan) -> Result<String, String> {
        let dedicated = format!("caelum-{}", plan.token);
        let name = source.environment.as_deref().unwrap_or(&dedicated);
        let envs = self
            .client
            .environments(None)
            .await
            .map_err(|e| Step::ListEnvironments.error(e))?
            .environments;
        let env = envs.into_iter().find(|e| e.name == name);
        if env
            .as_ref()
            .is_some_and(|e| clones(e, &source.repository, &source.base))
        {
            return Ok(name.into());
        }
        // Only the private name minted for this create attempt may be configured.
        if source.environment.is_some() {
            return Err(format!(
                "The selected Boat environment no longer clones {} on {}. Reopen VM setup.",
                source.repository, source.base
            ));
        }
        let repo = self
            .client
            .repos(&ReposQuery {
                q: Some(source.repository.clone()),
                sync: Some(true),
                ..Default::default()
            })
            .await
            .map_err(|e| Step::FindRepository.error(e))?
            .installations
            .into_iter()
            .flat_map(|i| i.repositories.unwrap_or_default())
            .find(|r| {
                r.full_name
                    .as_ref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&source.repository))
            })
            .and_then(|r| r.database_id)
            .ok_or("Connect this GitHub repository to Boat before creating a machine")?;
        let env = match env {
            Some(env) => env,
            None => self
                .client
                .create_environment(None, &CreateSandboxEnvironmentRequest { name: name.into() })
                .await
                .map_err(|e| Step::CreateEnvironment.error(e))?
                .environments
                .into_iter()
                .find(|e| e.name == name)
                .ok_or("Boat did not return the new environment")?,
        };
        if env.is_default
            || env
                .selected_repositories
                .as_ref()
                .is_some_and(|r| !r.is_empty())
        {
            return Err("The checkout environment is already in use".into());
        }
        let response = self
            .client
            .update_environment(
                &env.id,
                &UpdateSandboxEnvironmentRequest {
                    pass_github: Some(true),
                    pass_agents_credentials: Some(true),
                    pass_secrets: Some(false),
                    pass_sandbox_credentials: Some(false),
                    safe_for_third_parties: Some(false),
                    repositories: Some(vec![EnvironmentRepository {
                        repository_id: repo,
                        base_branch: Some(source.base.clone()),
                        setup_script: None,
                        setup_blocking: None,
                    }]),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| Step::ConfigureEnvironment.error(e))?;
        if !response.success {
            return Err("Could not configure the new checkout environment".into());
        }
        self.client
            .environments(None)
            .await
            .map_err(|e| Step::ListEnvironments.error(e))?
            .environments
            .iter()
            .any(|e| {
                e.id == env.id && e.name == name && clones(e, &source.repository, &source.base)
            })
            .then(|| name.to_owned())
            .ok_or_else(|| {
                format!(
                    "Boat has not confirmed the clone branch {} for {}. Refresh checkout to retry.",
                    source.base, source.repository
                )
            })
    }

    pub(super) async fn create_checkout(
        &self,
        source: &Source,
        plan: &Plan,
    ) -> Result<Machine, String> {
        if source.account != self.account {
            return Err("Connect the Boat account selected for this checkout".into());
        }
        let name = repo_name(&source.repository)?;
        let token =
            uuid::Uuid::parse_str(&plan.token).map_err(|_| "Invalid machine create request")?;
        if !source.valid_plan_branch(&plan.branch) {
            return Err("Invalid branch in the VM checkout plan.".into());
        }
        let environment = self.environment(source, plan).await?;
        let result = self
            .client
            .create_sandbox(
                &CreateSandboxRequest {
                    name: Some(format!(
                        "{}-{}",
                        &name[..name.len().min(32)],
                        &token.simple().to_string()[..8]
                    )),
                    machine_type: Some(match plan.size {
                        Size::Small => MachineSize::Small,
                        Size::Standard => MachineSize::Default,
                        Size::Large => MachineSize::Large,
                        Size::Xlarge => MachineSize::Xlarge,
                    }),
                    ttl_seconds: Some(super::auto_stop(plan.lifetime)),
                    environment: Some(environment),
                    ..Default::default()
                },
                Some(&plan.token),
            )
            .await
            .map_err(|e| Step::CreateMachine.error(e))?;
        Ok(self.machine(result.sandbox))
    }

    pub(super) async fn inspect(&self, key: &MachineId) -> Result<Sandbox, String> {
        self.owns(key)?;
        let sandbox = self
            .client
            .get_sandbox(&key.id)
            .await
            .map_err(|e| Step::InspectMachine.error(e))?
            .sandbox;
        if sandbox.id != key.id {
            return Err("Boat returned a different machine".into());
        }
        Ok(sandbox)
    }

    pub(super) async fn prepare_checkout(
        &self,
        target: &Target,
        base: &str,
        branch: Option<&str>,
    ) -> Result<Outcome, String> {
        let sandbox = self.inspect(&target.machine).await?;
        if matches!(
            sandbox.state.as_str(),
            "archived" | "stopped" | "error" | "failed" | "cancelled"
        ) {
            return Ok(Outcome::Machine(self.machine(sandbox)));
        }
        if matches!(
            sandbox.state.as_str(),
            "init" | "provisioning" | "provisioned" | "cloning" | "archiving"
        ) || sandbox.hydrated == Some(false)
            || matches!(sandbox.setup_status.as_deref(), Some("pending" | "running"))
        {
            return Ok(Outcome::Waiting(self.machine(sandbox)));
        }
        if !matches!(sandbox.state.as_str(), "idle" | "ready" | "running") {
            return Err(format!("Unsupported Boat machine state: {}", sandbox.state));
        }
        if sandbox.setup_status.as_deref() == Some("failed") {
            return Err("Boat repository setup failed".into());
        }
        let envs = self
            .client
            .environments(None)
            .await
            .map_err(|e| Step::ListEnvironments.error(e))?;
        if envs
            .environments
            .iter()
            .find(|e| {
                Some(&e.name) == sandbox.environment.as_ref()
                    && e.latest_version_number == sandbox.environment_version
            })
            .is_none_or(|e| selected_repo(e, &target.repository).is_none())
        {
            return Err("This machine needs a current single-repository environment for the selected GitHub repository".into());
        };
        if branch.is_some_and(|b| !valid_branch(b)) {
            return Err("Invalid checkout branch".into());
        }
        let worktree = self
            .worktree(target, branch.map(|_| (target.repository.as_str(), base)))
            .await?;
        if let Some(branch) = branch {
            worktree.open_planned_branch(target, base, branch).await?;
        }
        let current = worktree.command("git branch --show-current").await?;
        let current = current.trim();
        if let Some(base) = target
            .required_base
            .as_ref()
            .filter(|b| branch.is_none() && current != b.branch)
        {
            return Err(format!("The session uses {}, but the VM checkout is on {}. Choose a matching VM or create one.", base.branch, if current.is_empty() {"detached HEAD"} else {current}));
        }
        worktree.verify_base().await?;
        if !valid_branch(current) {
            return Err("Select a named branch on the remote checkout".into());
        }
        if branch.is_some_and(|expected| expected != current) {
            return Err("The VM changed branches during setup. Refresh the checkout.".into());
        }
        Ok(Outcome::Prepared {
            machine: self.machine(sandbox),
            branch: current.into(),
            root: worktree.root,
        })
    }
}

impl Worktree<'_> {
    async fn open_planned_branch(
        &self,
        target: &Target,
        base: &str,
        branch: &str,
    ) -> Result<(), String> {
        if !valid_branch(branch) || !valid_branch(base) {
            return Err("Invalid checkout branch".into());
        }
        let start = match &target.required_base {
            Some(p) if p.branch == base && p.valid() => quote(&p.tip),
            None if branch != base => "FETCH_HEAD".into(),
            _ => return Err("The checkout plan does not match the session branch.".into()),
        };
        let command = if branch == base {
            format!(
                "branch={}; tip={start}; {OPEN_SESSION_BRANCH}",
                quote(branch)
            )
        } else {
            // Standalone creates and older saved handoffs have an explicit new branch.
            format!("if test \"$(git branch --show-current)\" != {branch}; then git fetch --no-tags origin {base} && git merge-base --is-ancestor {start} FETCH_HEAD && git switch --no-track -c {branch} {start}; fi",
                branch = quote(branch), base = quote(&format!("refs/heads/{base}")))
        };
        self.command(&command).await?;
        Ok(())
    }
}
