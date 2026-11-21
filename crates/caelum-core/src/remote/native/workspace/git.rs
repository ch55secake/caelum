use super::Access;
use crate::{files, git, remote::boat::checkout_quote as quote};
use caelum_protocol::{
    GitFileDiff, GitRepositoryStatus, ProjectSearchMatch, ProjectSearchMode, ProjectSearchResults,
};

impl Access {
    async fn git(&self, args: &[&str], codes: &[i32]) -> Result<Vec<u8>, String> {
        let root = quote(&self.root);
        let args = args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
        let script=format!("test \"$(git -C {root} rev-parse --show-toplevel)\" = {root} && GIT_OPTIONAL_LOCKS=0 LC_ALL=C git -C {root} --literal-pathspecs {args}");
        self.run(&script, 8 * 1024 * 1024, codes).await
    }
    pub(crate) async fn branch_context(&self) -> Result<git::GitBranchContext, String> {
        let branch = self.git(&["branch", "--show-current"], &[0]).await?;
        let branch = String::from_utf8(branch).map_err(|_| "Invalid VM branch")?;
        Ok(git::GitBranchContext {
            root: self.root.clone().into(),
            branch: (!branch.trim().is_empty()).then(|| branch.trim().into()),
        })
    }
    pub(super) async fn status(&self) -> Result<GitRepositoryStatus, String> {
        let branch = self.branch_context().await?.branch;
        let mut changes = git::parse_status(&self.git(git::PORCELAIN_STATUS_ARGS, &[0]).await?)
            .map_err(|e| e.to_string())?;
        self.stamps(&mut changes).await;
        Ok(GitRepositoryStatus {
            project_path: self.root.clone(),
            repository_root: Some(self.root.clone()),
            branch,
            changes,
        })
    }
    pub(super) async fn diff(&self, path: &str) -> Result<GitFileDiff, String> {
        files::validate_relative_path(path).map_err(|e| e.to_string())?;
        let mut args = git::PORCELAIN_STATUS_ARGS.to_vec();
        args.extend_from_slice(&["--", path]);
        let changes =
            git::parse_status(&self.git(&args, &[0]).await?).map_err(|e| e.to_string())?;
        let change = changes
            .iter()
            .find(|f| f.path == path)
            .ok_or("Not a changed file on the VM.")?;
        let read = async |extra: &[&str], paths: &[&str], codes: &[i32]| {
            let mut args = vec!["diff", "--no-ext-diff", "--no-textconv", "--color=never"];
            args.extend_from_slice(extra);
            args.push("--");
            args.extend_from_slice(paths);
            self.git(&args, codes)
                .await
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        Ok(GitFileDiff {
            path: path.into(),
            staged: read(&["--cached"], &[path], &[0]).await?,
            unstaged: read(&[], &[path], &[0]).await?,
            untracked: if change.index_status == "?" {
                read(&["--no-index"], &["/dev/null", path], &[0, 1]).await?
            } else {
                String::new()
            },
        })
    }
    pub(super) async fn search(
        &self,
        query: &str,
        mode: ProjectSearchMode,
    ) -> Result<ProjectSearchResults, String> {
        if query.chars().count() > 256 || query.contains('\0') {
            return Err("Search query is too long or invalid.".into());
        }
        let mut result = ProjectSearchResults {
            matches: vec![],
            truncated: false,
            skipped: 0,
        };
        match mode {
            ProjectSearchMode::Files => {
                let data = self
                    .git(
                        &[
                            "ls-files",
                            "-z",
                            "--cached",
                            "--others",
                            "--exclude-standard",
                            "--deduplicate",
                        ],
                        &[0],
                    )
                    .await?;
                let mut ranked: Vec<_> = data
                    .split(|b| *b == 0)
                    .filter(|p| !p.is_empty())
                    .filter_map(|p| std::str::from_utf8(p).ok())
                    .filter_map(|p| files::path_score(p, query.trim()).map(|s| (s, p)))
                    .collect();
                ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
                result.truncated = ranked.len() > 200;
                result.matches = ranked
                    .into_iter()
                    .take(200)
                    .map(|(_, p)| ProjectSearchMatch {
                        path: p.into(),
                        line: None,
                        column: None,
                        preview: None,
                    })
                    .collect();
            }
            ProjectSearchMode::Content if !query.trim().is_empty() => {
                let data = self
                    .git(
                        &[
                            "grep",
                            "--untracked",
                            "--exclude-standard",
                            "-n",
                            "-z",
                            "-I",
                            "-i",
                            "-F",
                            "-e",
                            query,
                            "--",
                            ".",
                        ],
                        &[0, 1],
                    )
                    .await?;
                let pattern = regex::RegexBuilder::new(&regex::escape(query))
                    .case_insensitive(true)
                    .build()
                    .map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&data);
                let mut rest = text.as_ref();
                while let Some((path, after)) = rest.split_once('\0') {
                    let Some((n, after)) = after.split_once('\0') else {
                        break;
                    };
                    let (line, after) = after.split_once('\n').unwrap_or((after, ""));
                    if result.matches.len() == 200 {
                        result.truncated = true;
                        break;
                    }
                    if let Some(m) = pattern.find(line) {
                        let prefix = &line[..m.start()];
                        result.matches.push(ProjectSearchMatch {
                            path: path.into(),
                            line: n.parse().ok(),
                            column: Some(prefix.encode_utf16().count() + 1),
                            preview: Some(
                                line.chars()
                                    .skip(prefix.chars().count().saturating_sub(60))
                                    .take(240)
                                    .collect(),
                            ),
                        });
                    }
                    rest = after;
                }
            }
            _ => {}
        }
        Ok(result)
    }
}
