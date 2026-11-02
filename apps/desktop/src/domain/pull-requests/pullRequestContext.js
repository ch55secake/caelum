import { checkStatus } from "./pullRequestState.js";

const OPEN = "\n\n<caelum-pull-request-context>\n";
const CLOSE = "\n</caelum-pull-request-context>";
export const MAX_PULL_REQUEST_CONTEXT = 16;
const MAX_CHECKS = 100;

export function createPullRequestContext(pr) {
  const checks = pr.statusCheckRollup?.map(checkStatus) ?? null;
  return {
    id: crypto.randomUUID(),
    number: pr.number,
    title: pr.title,
    url: pr.url,
    state: pr.state,
    head: pr.headRefName,
    base: pr.baseRefName,
    commit: pr.headRefOid,
    capturedAt: Date.now(),
    checks:
      checks
        ?.sort(
          (a, b) => Number(b.tone === "failure") - Number(a.tone === "failure"),
        )
        .slice(0, MAX_CHECKS)
        .map(({ name, text, url }) => ({ name, status: text, url })) ?? null,
    checksTruncated: Boolean(checks && checks.length > MAX_CHECKS),
  };
}

export function promptWithPullRequests(text, pullRequests) {
  return pullRequests.length
    ? `${text}${OPEN}${JSON.stringify({ version: 1, pullRequests })}${CLOSE}`
    : text;
}

const httpUrl = (value) => {
  try {
    return ["https:", "http:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
};
export function splitPullRequestContext(prompt) {
  const start = prompt.lastIndexOf(OPEN);
  if (start >= 0 && prompt.endsWith(CLOSE)) {
    try {
      const value = JSON.parse(
        prompt.slice(start + OPEN.length, -CLOSE.length),
      );
      const prs = value?.pullRequests;
      if (
        value?.version === 1 &&
        Array.isArray(prs) &&
        prs.length > 0 &&
        prs.length <= MAX_PULL_REQUEST_CONTEXT &&
        new Set(prs.map((pr) => pr?.url)).size === prs.length &&
        new Set(prs.map((pr) => pr?.id)).size === prs.length &&
        prs.every(
          (pr) =>
            pr &&
            ["id", "title", "url", "state", "head", "base", "commit"].every(
              (key) => typeof pr[key] === "string",
            ) &&
            pr.id &&
            httpUrl(pr.url) &&
            Number.isSafeInteger(pr.number) &&
            pr.number > 0 &&
            Number.isFinite(pr.capturedAt) &&
            typeof pr.checksTruncated === "boolean" &&
            (pr.checks === null ||
              (Array.isArray(pr.checks) &&
                pr.checks.length <= MAX_CHECKS &&
                pr.checks.every(
                  (check) =>
                    check &&
                    typeof check.name === "string" &&
                    typeof check.status === "string" &&
                    (check.url === null ||
                      (typeof check.url === "string" && httpUrl(check.url))),
                ))),
        )
      )
        return { text: prompt.slice(0, start), pullRequests: prs };
    } catch {
      /* Incomplete and ordinary message text stays visible. */
    }
  }
  return { text: prompt, pullRequests: [] };
}
