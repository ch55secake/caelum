use super::{checkout::quote, Boat};
use crate::remote::{github_slug, valid_branch, GitAction, Review, Revision, Target};

// A temporary index produces one reviewable tree including non-ignored new files
// without staging anything in the user's index. The blobs stay on the VM.
const REVIEW: &str = r#"set -e
head=$(git rev-parse --verify HEAD)
branch=$(git branch --show-current)
index=$(git write-tree)
status=$(git status --short --branch --untracked-files=all)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export GIT_INDEX_FILE="$tmp/index"
git read-tree "$index"
git add -A -- .
tree=$(git write-tree)
printf '%s\0' "$branch" "$head" "$index" "$tree" "$status"
git --no-pager diff --no-color --no-ext-diff --no-textconv "$head" "$tree" -- ."#;

fn oid(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Revision {
    fn valid(&self) -> bool {
        valid_branch(&self.branch)
            && [&self.head, &self.index, &self.tree]
                .into_iter()
                .all(|s| oid(s))
    }
    fn guard(&self) -> String {
        format!(
            "test \"$(git branch --show-current)\" = {} && test \"$(git rev-parse HEAD)\" = {}",
            quote(&self.branch),
            quote(&self.head)
        )
    }
}

impl Boat {
    pub(in crate::remote) async fn git(
        &self,
        target: &Target,
        action: &GitAction,
    ) -> Result<Review, String> {
        let worktree = self.worktree(target, None).await?;
        match action {
            GitAction::Read => {}
            GitAction::Commit(r, msg) => {
                if !r.valid() || msg.trim().is_empty() || msg.len() > 4096 || msg.contains('\0') {
                    return Err("Invalid remote commit".into());
                }
                // Only commit the reviewed tree. Later edits to the worktree remain unstaged;
                // refuse to replace an index or HEAD changed since review.
                worktree.command(&format!("{} && test \"$(git write-tree)\" = {} && git read-tree {} && git commit -m {}",
                    r.guard(),quote(&r.index),quote(&r.tree),quote(msg.trim()))).await?;
            }
            GitAction::Push(r) => {
                if !r.valid() {
                    return Err("Invalid remote branch".into());
                }
                let urls = worktree
                    .command("git remote get-url --push --all origin")
                    .await?;
                if urls.trim().is_empty()
                    || !urls.lines().all(|url| {
                        github_slug(url).is_some_and(|s| s.eq_ignore_ascii_case(&target.repository))
                    })
                {
                    return Err("The push destination is not the reviewed GitHub repository".into());
                }
                // Pin the source object even if another tool moves the branch during the push.
                worktree
                    .command(&format!(
                        "{} && git push origin {}",
                        r.guard(),
                        quote(&format!("{}:refs/heads/{}", r.head, r.branch))
                    ))
                    .await?;
            }
        }
        let data = worktree.command(REVIEW).await?;
        if data.len() > 512 * 1024 {
            return Err("Remote diff exceeds the 512 KiB review limit".into());
        }
        let mut parts = data.splitn(6, '\0');
        let mut field = || {
            parts
                .next()
                .map(str::to_owned)
                .ok_or("Incomplete remote diff")
        };
        let revision = Revision {
            branch: field()?,
            head: field()?,
            index: field()?,
            tree: field()?,
        };
        if !revision.valid() {
            return Err("The checkout has no valid branch and commit to review".into());
        }
        Ok(Review {
            revision,
            status: field()?,
            patch: field()?,
        })
    }
}
