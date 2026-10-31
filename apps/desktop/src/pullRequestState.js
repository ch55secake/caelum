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
  const name = run ? check.name : check.context;
  if (state === "SUCCESS") return { name, tone: "success", text: "Passed" };
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
      name,
      tone: "failure",
      text: state === "CANCELLED" ? "Cancelled" : "Failed",
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
    return { name, tone: "pending", text: "Pending" };
  if (["NEUTRAL", "SKIPPED"].includes(state))
    return {
      name,
      tone: "neutral",
      text: state === "SKIPPED" ? "Skipped" : "Neutral",
    };
  return { name, tone: "unknown", text: "Unknown" };
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
