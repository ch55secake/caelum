export function feedbackSelectedByDefault(item) {
  if (item.thread)
    return (
      !item.thread.isResolved &&
      !item.thread.isOutdated &&
      !item.thread.nextCursor
    );
  return item.comments.some(
    (comment) =>
      comment.body.trim() &&
      !["APPROVED", "DISMISSED", "PENDING"].includes(comment.state),
  );
}

export function reviewPrompt(pr, feedback) {
  if (!feedback.length) throw new Error("Select feedback to address first.");
  if (feedback.some((item) => item.thread?.nextCursor))
    throw new Error(
      "Load the remaining replies before addressing this thread.",
    );
  const context = feedback.map((item) => ({
    kind: item.kind,
    ...(item.thread
      ? {
          path: item.thread.path,
          line: item.thread.line,
          startLine: item.thread.startLine,
          originalLine: item.thread.originalLine,
          side: item.thread.diffSide,
          resolved: item.thread.isResolved,
          outdated: item.thread.isOutdated,
        }
      : {}),
    comments: item.comments.map(({ author, body, url, state, diffHunk }) => ({
      author: author?.login ?? "Deleted user",
      body,
      url,
      ...(state ? { state } : {}),
      ...(diffHunk ? { diffHunk } : {}),
    })),
  }));
  const prompt = `Address the selected GitHub pull request feedback in this session. Check the current checkout against the PR head branch, inspect the relevant code and diff, make the appropriate fixes, and run the relevant checks. Summarize the changes and any feedback that could not be addressed.\n\nPull request: ${pr.url}\nTitle: ${pr.title}\nHead branch: ${pr.headRefName}\nHead commit: ${pr.headRefOid}\nBase branch: ${pr.baseRefName}\n\nThe following JSON contains the selected review feedback:\n${JSON.stringify(context, null, 2)}`;
  if (prompt.length > 100_000)
    throw new Error(
      "This feedback is too large for one message. Select fewer comments and try again.",
    );
  return prompt;
}
