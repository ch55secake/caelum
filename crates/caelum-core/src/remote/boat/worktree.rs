use super::{checkout::quote, setup_error::Step, Boat};
use crate::remote::{
    checkout::{repo_name, valid_root},
    valid_branch, Target,
};
use boat_sdk::commands::{models::CommandResponse, CommandRequest};

pub(super) struct Worktree<'a> {
    boat: &'a Boat,
    target: &'a Target,
    pub(super) root: String,
}

impl Boat {
    async fn exec(
        &self,
        target: &Target,
        request: CommandRequest,
    ) -> Result<CommandResponse, String> {
        self.owns(&target.machine)?;
        self.client
            .run_command(&target.machine.id, &request)
            .await
            .map_err(|e| Step::RunCommand.error(e))
    }

    pub(super) async fn worktree<'a>(
        &'a self,
        target: &'a Target,
        new_clone: Option<(&str, &str)>,
    ) -> Result<Worktree<'a>, String> {
        let (repo, base) = new_clone.unwrap_or((&target.repository, ""));
        repo_name(repo)?;
        if !repo.eq_ignore_ascii_case(&target.repository)
            || new_clone.is_some() && !valid_branch(base)
            || target.root.as_deref().is_some_and(|r| !valid_root(r))
        {
            return Err("Invalid VM checkout target.".into());
        }
        let script = format!(
            "python3 -I - {} {} {} <<'CAELUM_CHECKOUT'\n{}CAELUM_CHECKOUT",
            quote(repo),
            quote(target.root.as_deref().unwrap_or("")),
            quote(base),
            include_str!("locate_checkout.py"),
        );
        let mut request = CommandRequest::new(script);
        if new_clone.is_some() {
            request.timeout_seconds = Some(300);
        }
        let reply = self.exec(target, request).await?;
        let root = reply.stdout.strip_suffix('\n').unwrap_or(&reply.stdout);
        if reply.success
            && !reply.timed_out
            && reply.exit_code == Some(0)
            && reply.stdout_truncated != Some(true)
            && valid_root(root)
            && target.root.as_ref().is_none_or(|r| r == root)
        {
            return Ok(Worktree {
                boat: self,
                target,
                root: root.into(),
            });
        }
        if reply.timed_out {
            return Err(format!("Timed out preparing {repo} on the VM. Check its GitHub access, then choose Refresh checkout."));
        }
        Err(match (reply.exit_code, target.root.as_deref()) {
            (Some(40), Some(path)) => format!("The saved checkout of {repo} is missing at {path} on this VM. Restore it there, then choose Refresh checkout."),
            (Some(40), None) => format!("No checkout of {repo} was found on this VM. Check Boat’s GitHub repository access and restore the checkout, then choose Refresh checkout."),
            (Some(41), _) => format!("The VM checkout directory is occupied or its Git origin does not match {repo}. Check the repository on the VM, then choose Refresh checkout."),
            (Some(42), _) => format!("More than one VM checkout has {repo} as its origin. Leave one checkout in the VM work directory, then choose Refresh checkout."),
            (Some(43), _) => format!("Could not clone {repo} at branch {base} on the new VM. Check Boat’s GitHub access and the published branch, then choose Refresh checkout."),
            _ => "Could not locate the Git checkout on the VM. Check that Git and Python 3 are available, then choose Refresh checkout.".into(),
        })
    }
}

impl Worktree<'_> {
    pub(super) async fn command(&self, command: &str) -> Result<String, String> {
        let path = quote(&self.root);
        let script = format!(
            "cd -P {path} && test \"$PWD\" = {path} && test \"$(git rev-parse --show-toplevel)\" = \"$PWD\" && {{ {command}; }}"
        );
        let result = self
            .boat
            .exec(self.target, CommandRequest::new(script))
            .await?;
        if !result.success || result.timed_out || result.exit_code != Some(0) {
            return Err(format!("Remote Git command failed (exit {}). Check the checkout, branch and GitHub access on the machine.",
                result.exit_code.map_or_else(|| "unknown".to_owned(), |n| n.to_string())));
        }
        if result.stdout_truncated == Some(true) {
            return Err("The remote command output was truncated".into());
        }
        Ok(result.stdout)
    }

    pub(super) async fn verify_base(&self) -> Result<(), String> {
        if let Some(base) = &self.target.required_base {
            if !base.valid() {
                return Err("Invalid session base branch.".into());
            }
            let answer = self
                .command(&format!(
                    "if git merge-base --is-ancestor {} HEAD; then echo yes; else echo no; fi",
                    quote(&base.tip)
                ))
                .await?;
            if answer.trim() != "yes" {
                return Err(format!("This VM checkout does not include the published tip of {}. Update its branch or choose a new VM.", base.branch));
            }
        }
        Ok(())
    }
}
