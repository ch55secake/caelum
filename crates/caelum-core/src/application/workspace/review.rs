use super::Workspace;

impl Workspace {
    /// Restore the last overview/diff while GitHub refreshes current metadata.
    pub fn open_review(&mut self, project: String, url: String) {
        if self
            .services
            .reviews
            .get(&(project.clone(), url.clone()))
            .is_none_or(|review| review.details.is_none())
        {
            self.task(crate::application::Task::ReviewCache {
                project: project.clone(),
                url: url.clone(),
            });
        }
        self.task(crate::application::Task::Details { project, url });
    }

    /// Find the conversation associated with the reviewed source branch, including
    /// its recorded history in linked checkouts. The checkout's current Git branch
    /// alone is not evidence that a session worked on the pull request.
    pub fn pull_request_session(&self, project: &str, url: &str) -> Option<usize> {
        let head = &self
            .services
            .reviews
            .get(&(project.into(), url.into()))?
            .details
            .as_ref()?
            .head_ref_name;
        if head.is_empty() {
            return None;
        }
        let group = self.group_path(project);
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                !session.forgotten
                    && !session.deleting
                    && !session.archiving
                    && self.group_path(&session.scope.project) == group
                    && self
                        .projects
                        .iter()
                        .any(|p| p.path == session.scope.project)
                    && session.details.as_ref().is_some_and(|details| {
                        details.branch.as_ref() == Some(head) || details.branches.contains(head)
                    })
            })
            .max_by_key(|(index, session)| {
                (
                    !session.archived,
                    self.selected == Some(*index),
                    session.last_activity,
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests;
