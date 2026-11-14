use super::*;

impl Services {
    pub(in crate::application) fn complete(
        &mut self,
        id: u64,
        task: &Task,
        reply: Result<TaskReply, String>,
    ) -> Vec<Task> {
        let current = self.current(id, task);
        self.generations.remove(&id);
        if self.latest.get(&task.slot()) == Some(&id) {
            self.loading.remove(&task.slot());
        }
        if !current {
            return Vec::new();
        }
        let reply = match reply {
            Ok(reply) => reply,
            Err(error) => {
                self.errors.insert(task.slot(), error.clone());
                if let Some((url, _)) = task.review() {
                    self.reviews
                        .entry((task.project().into(), url.into()))
                        .or_default()
                        .error = Some(error);
                } else if !matches!(task, Task::Shell { .. }) {
                    self.checkouts
                        .entry(task.project().into())
                        .or_default()
                        .error = Some(error);
                }
                return Vec::new();
            }
        };
        let mut next = Vec::new();
        let mut merged_url = None;
        self.revision += 1;
        let checkout = self.checkouts.entry(task.project().into()).or_default();
        match (task, reply) {
            (
                Task::Shell {
                    session,
                    id,
                    stop: false,
                    ..
                },
                TaskReply::Shell(output),
            ) => {
                // Long-running workspaces can observe many short-lived shells.
                if checkout.shell_output.len() >= 64
                    && !checkout
                        .shell_output
                        .contains_key(&(session.clone(), id.clone()))
                {
                    checkout.shell_output.pop_first();
                }
                checkout
                    .shell_output
                    .insert((session.clone(), id.clone()), output);
            }
            (
                Task::Shell {
                    session,
                    id,
                    stop: true,
                    ..
                },
                TaskReply::Shell(_),
            ) => {
                checkout.shell_output.remove(&(session.clone(), id.clone()));
            }
            (Task::Image { project, source }, TaskReply::Image(data)) => {
                while self.images.len() >= 24
                    || self.images.values().map(String::len).sum::<usize>() + data.len()
                        > 40 * 1024 * 1024
                {
                    if self.images.pop_first().is_none() {
                        break;
                    }
                }
                self.images.insert((project.clone(), source.clone()), data);
            }
            (Task::Updates, TaskReply::Updates(update)) => self.update = Some(update),
            (Task::Usage { .. }, TaskReply::Usage(usage)) => checkout.usage = Some(usage),
            (Task::Stats { .. }, TaskReply::Stats(stats, projects)) => {
                checkout.stats = Some(stats);
                checkout.stats_projects = projects;
            }
            (Task::Completions { query, .. }, TaskReply::Search(results)) => {
                checkout.completions = Some(results);
                checkout.completion_query = query.clone();
            }
            (Task::Directory { path, offset, .. }, TaskReply::Directory(mut page)) => {
                if *offset > 0 {
                    if let Some(previous) = checkout.directories.get(path) {
                        let mut entries = previous.entries.clone();
                        entries.append(&mut page.entries);
                        page.entries = entries;
                    }
                }
                checkout.directories.insert(path.clone(), page);
            }
            (Task::File { path, .. }, TaskReply::File(file)) => {
                checkout.files.insert(path.clone(), file);
            }
            (Task::Search { query, .. }, TaskReply::Search(results)) => {
                checkout.search = Some(results);
                checkout.search_query = query.clone();
            }
            (Task::Git { project }, TaskReply::Git(status, _)) => {
                checkout
                    .diffs
                    .retain(|path, _| status.changes.iter().any(|c| &c.path == path));
                if let Some(branch) = status
                    .branch
                    .as_ref()
                    .filter(|b| checkout.git.as_ref().and_then(|g| g.branch.as_ref()) != Some(*b))
                {
                    next.push(Task::Discover {
                        project: project.clone(),
                        branch: branch.clone(),
                    });
                }
                checkout.git = Some(status);
            }
            (Task::Diff { path, .. }, TaskReply::Diff(diff))
                if checkout
                    .git
                    .as_ref()
                    .is_none_or(|g| g.changes.iter().any(|c| &c.path == path)) =>
            {
                checkout.diffs.insert(path.clone(), diff);
            }
            (_, TaskReply::Worktrees(listing)) => {
                checkout.worktrees = listing;
                checkout.worktrees_revision = id;
            }
            (
                Task::CreateWorktree {
                    directory, project, ..
                },
                TaskReply::Worktree(_),
            ) => next.push(Task::Worktrees {
                project: project.clone(),
                directory: directory.clone(),
            }),
            (Task::RemoveWorktree { project, .. }, TaskReply::Worktree(_)) => {
                next.push(Task::Worktrees {
                    project: project.clone(),
                    directory: String::new(),
                })
            }
            (
                Task::RenameWorktree {
                    project, directory, ..
                },
                TaskReply::Worktree(_),
            ) => next.push(Task::Worktrees {
                project: project.clone(),
                directory: directory.clone(),
            }),
            (Task::Discover { project, branch }, TaskReply::Discovered(pr)) => {
                let pr = pr.filter(|pr| !self.merged_pull_requests.contains(&pr.url));
                let url = pr.as_ref().map(|pr| pr.url.clone());
                if let Some(Some(previous)) = self
                    .discovered
                    .insert((project.clone(), branch.clone()), url.clone())
                {
                    if Some(&previous) != url.as_ref()
                        && !self.discovered.iter().any(|((path, _), url)| {
                            path == project && url.as_ref() == Some(&previous)
                        })
                    {
                        checkout.pull_requests.retain(|pr| pr.url != previous);
                    }
                }
                if let Some(pr) = pr {
                    if let Some(previous) =
                        checkout.pull_requests.iter_mut().find(|p| p.url == pr.url)
                    {
                        *previous = pr.clone();
                    } else {
                        checkout.pull_requests.push(pr.clone());
                    }
                    if !pr.base_ref_name.is_empty()
                        && !self
                            .discovered
                            .contains_key(&(project.clone(), pr.base_ref_name.clone()))
                    {
                        self.discovered
                            .insert((project.clone(), pr.base_ref_name.clone()), None);
                        next.push(Task::Discover {
                            project: project.clone(),
                            branch: pr.base_ref_name,
                        });
                    }
                }
            }
            (Task::ReviewCache { project, url }, TaskReply::ReviewCache(Some(cached))) => {
                let review = self
                    .reviews
                    .entry((project.clone(), url.clone()))
                    .or_default();
                // Disk replies never replace live data, including a newer head.
                if review.details.is_none() && cached.details.pull_request.url == *url {
                    if let Some(files) = cached
                        .files
                        .filter(|f| f.base_oid == cached.details.base_ref_oid)
                    {
                        review.files = files.files;
                        review.base = Some(files.base_oid);
                        review.next_page = files.next_page;
                        review.files_loaded = true;
                    }
                    review.details = Some(cached.details);
                    review.source = ReviewSource::Cache;
                }
            }
            (Task::Details { project, url }, TaskReply::Details(details)) => {
                if details.state == "MERGED" {
                    merged_url = Some(url.clone());
                }
                if self
                    .reviews
                    .get(&(project.clone(), url.clone()))
                    .is_none_or(|review| {
                        review.head() != Some(&details.head_ref_oid)
                            || review.expected_base() != Some(&details.base_ref_oid)
                    })
                {
                    self.refresh_review(project, url);
                }
                let review = self
                    .reviews
                    .entry((project.clone(), url.clone()))
                    .or_default();
                if review.head() != Some(&details.head_ref_oid)
                    || review.expected_base() != Some(&details.base_ref_oid)
                {
                    let generation = review.generation;
                    *review = Review::default();
                    review.generation = generation;
                }
                let head = details.head_ref_oid.clone();
                let base = details.base_ref_oid.clone();
                review.details = Some(*details);
                review.source = ReviewSource::GitHub;
                if !review.files_loaded {
                    next.push(Task::ReviewFiles {
                        project: project.clone(),
                        url: url.clone(),
                        head: head.clone(),
                        base,
                        page: 1,
                    });
                }
                for kind in [
                    GitHubFeedbackKind::Discussion,
                    GitHubFeedbackKind::Reviews,
                    GitHubFeedbackKind::Threads,
                ] {
                    if !review.cursors.contains_key(&format!("{kind:?}")) {
                        next.push(Task::Feedback {
                            project: project.clone(),
                            url: url.clone(),
                            head: head.clone(),
                            kind,
                            cursor: None,
                            thread: None,
                        });
                    }
                }
            }
            (
                Task::ReviewFiles {
                    project,
                    url,
                    base,
                    page,
                    ..
                },
                TaskReply::ReviewFiles(files),
            ) => {
                let review = self
                    .reviews
                    .entry((project.clone(), url.clone()))
                    .or_default();
                if files.base_oid != *base || (*page > 1 && review.base.as_ref() != Some(base)) {
                    review.error = Some(
                        "PR base changed; refresh the review before loading more files".into(),
                    );
                    return next;
                }
                if *page == 1 {
                    review.files.clear();
                }
                for file in files.files {
                    if !review.files.iter().any(|f| f.path == file.path) {
                        review.files.push(file);
                    }
                }
                review.base = Some(files.base_oid);
                review.next_page = files.next_page;
                review.files_loaded = true;
            }
            (
                Task::Feedback {
                    project,
                    url,
                    kind,
                    thread,
                    ..
                },
                TaskReply::Feedback(page),
            ) => {
                let review = self
                    .reviews
                    .entry((project.clone(), url.clone()))
                    .or_default();
                if *kind == GitHubFeedbackKind::Replies {
                    if let Some(target) = review
                        .threads
                        .iter_mut()
                        .find(|t| Some(&t.id) == thread.as_ref())
                    {
                        append_comments(&mut target.comments, page.comments);
                        target.next_cursor = page.next_cursor;
                    }
                } else {
                    review.cursors.insert(format!("{kind:?}"), page.next_cursor);
                    match kind {
                        GitHubFeedbackKind::Discussion => {
                            append_comments(&mut review.discussion, page.comments)
                        }
                        GitHubFeedbackKind::Reviews => {
                            append_comments(&mut review.reviews, page.comments)
                        }
                        GitHubFeedbackKind::Threads => {
                            for thread in page.threads {
                                if !review.threads.iter().any(|t| t.id == thread.id) {
                                    review.threads.push(thread);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            (Task::SetAutoMerge { project, url, .. }, TaskReply::AutoMerge(result)) => {
                // Refreshes started before or during the mutation cannot restore
                // the previous setting after GitHub has confirmed the change.
                self.invalidate_details(project, url);
                if result.state == "MERGED" {
                    merged_url = Some(url.clone());
                }
                let review = self
                    .reviews
                    .entry((project.clone(), url.clone()))
                    .or_default();
                review.notice = Some(
                    if result.state == "MERGED" {
                        "Pull request merged."
                    } else if result.auto_merge_request.is_some() {
                        "Auto-merge enabled. GitHub will merge when requirements are met."
                    } else {
                        "Auto-merge disabled."
                    }
                    .into(),
                );
                if let Some(details) = review
                    .details
                    .as_mut()
                    .filter(|details| details.head_ref_oid == result.head_ref_oid)
                {
                    details.state = result.state;
                    details.auto_merge_request = result.auto_merge_request;
                }
                next.push(Task::Details {
                    project: project.clone(),
                    url: url.clone(),
                });
            }
            (
                Task::Merge { project, url, .. } | Task::UpdateBranch { project, url, .. },
                TaskReply::Done,
            ) => {
                if matches!(task, Task::Merge { .. }) {
                    merged_url = Some(url.clone());
                }
                self.reviews
                    .entry((project.clone(), url.clone()))
                    .or_default()
                    .notice = Some(
                    if matches!(task, Task::Merge { .. }) {
                        "Pull request merged."
                    } else {
                        "Branch update accepted. Refresh to observe the new head."
                    }
                    .into(),
                );
                next.push(Task::Details {
                    project: project.clone(),
                    url: url.clone(),
                });
            }
            _ => {}
        }
        if let Some(url) = merged_url {
            self.merged_pull_requests.insert(url.clone());
            for checkout in self.checkouts.values_mut() {
                checkout.pull_requests.retain(|pr| pr.url != url);
            }
        }
        next
    }
}

fn append_comments(target: &mut Vec<GitHubReviewComment>, comments: Vec<GitHubReviewComment>) {
    for comment in comments {
        if !target.iter().any(|c| c.id == comment.id) {
            target.push(comment);
        }
    }
}
