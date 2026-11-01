export function pullRequestState(pr) {
  if (pr.state === "MERGED") return "Merged";
  if (pr.state === "CLOSED") return "Closed";
  return pr.isDraft ? "Draft" : "Open";
}

export function reviewStatus(decision) {
  return (
    {
      APPROVED: "Approved",
      CHANGES_REQUESTED: "Changes requested",
      REVIEW_REQUIRED: "Review required",
    }[decision] ?? "No review decision"
  );
}

export function checkStatus(check) {
  const run = check.__typename === "CheckRun";
  const state =
    run && check.status === "COMPLETED"
      ? check.conclusion
      : run
        ? check.status
        : check.state;
  const link = run ? check.detailsUrl : check.targetUrl;
  const base = {
    name: run ? check.name : check.context,
    url: typeof link === "string" && /^https?:\/\//i.test(link) ? link : null,
  };
  if (state === "SUCCESS") return { ...base, tone: "success", text: "Passed" };
  if (
    [
      "FAILURE",
      "ERROR",
      "CANCELLED",
      "TIMED_OUT",
      "ACTION_REQUIRED",
      "STARTUP_FAILURE",
      "STALE",
    ].includes(state)
  )
    return {
      ...base,
      tone: "failure",
      text:
        {
          CANCELLED: "Cancelled",
          TIMED_OUT: "Timed out",
          ACTION_REQUIRED: "Action required",
          STALE: "Stale",
        }[state] ?? "Failed",
    };
  if (
    [
      "QUEUED",
      "IN_PROGRESS",
      "PENDING",
      "EXPECTED",
      "WAITING",
      "REQUESTED",
    ].includes(state)
  )
    return {
      ...base,
      tone: "pending",
      text:
        {
          IN_PROGRESS: "In progress",
          QUEUED: "Queued",
          WAITING: "Waiting",
          REQUESTED: "Requested",
        }[state] ?? "Pending",
    };
  if (["NEUTRAL", "SKIPPED"].includes(state))
    return {
      ...base,
      tone: "neutral",
      text: state === "SKIPPED" ? "Skipped" : "Neutral",
    };
  return { ...base, tone: "unknown", text: "Unknown" };
}

export function checkSummary(checks) {
  const values = (checks ?? []).map(checkStatus);
  if (!values.length) return { tone: "neutral", text: "No checks reported" };
  const counts = (tone) => values.filter((value) => value.tone === tone).length;
  const text = [
    ["success", "passed"],
    ["failure", "failed"],
    ["pending", "pending"],
    ["neutral", "neutral / skipped"],
    ["unknown", "unknown"],
  ]
    .filter(([tone]) => counts(tone))
    .map(([tone, label]) => `${counts(tone)} ${label}`)
    .join(" · ");
  const tone = ["failure", "pending", "unknown", "success", "neutral"].find(
    (tone) => counts(tone),
  );
  return { tone, text };
}

export function mergeStatus(pr) {
  if (pr.state === "MERGED")
    return { enabled: false, text: `Merged into ${pr.baseRefName}` };
  if (pr.state === "CLOSED")
    return { enabled: false, text: "This pull request is closed." };
  if (pr.isDraft)
    return {
      enabled: false,
      text: "Mark this draft ready for review on GitHub before merging.",
    };
  if (pr.mergeable === "CONFLICTING" || pr.mergeStateStatus === "DIRTY")
    return { enabled: false, text: "Resolve merge conflicts before merging." };
  if (pr.mergeStateStatus === "BEHIND")
    return {
      enabled: false,
      text: "Update this branch with the base branch before merging.",
    };
  if (
    pr.mergeStateStatus === "BLOCKED" ||
    pr.reviewDecision === "CHANGES_REQUESTED"
  )
    return {
      enabled: false,
      text: "Waiting for required checks, reviews, or repository rules.",
    };
  if (
    pr.state !== "OPEN" ||
    pr.mergeable !== "MERGEABLE" ||
    !["CLEAN", "HAS_HOOKS", "UNSTABLE"].includes(pr.mergeStateStatus) ||
    !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(pr.headRefOid ?? "")
  )
    return {
      enabled: false,
      text: "Waiting for GitHub to determine mergeability…",
    };
  return { enabled: true, text: `Ready to merge into ${pr.baseRefName}` };
}

export function canUpdateBranch(pr) {
  return (
    pr.state === "OPEN" &&
    pr.mergeable === "MERGEABLE" &&
    pr.mergeStateStatus !== "DIRTY" &&
    /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(pr.headRefOid ?? "")
  );
}
