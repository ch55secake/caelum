export type PullRequest = {
  number: number;
  title: string;
  url: string;
  isDraft: boolean;
};
export type StatusCheck =
  | {
      __typename: "CheckRun";
      name: string;
      status: string;
      conclusion: string | null;
    }
  | { __typename: "StatusContext"; context: string; state: string };
export type PullRequestDetails = PullRequest & {
  body: string;
  state: string;
  author: { login: string } | null;
  baseRefName: string;
  headRefName: string;
  additions: number;
  deletions: number;
  changedFiles: number;
  reviewDecision: string | null;
  statusCheckRollup: StatusCheck[] | null;
};
export function pullRequestState(pr: PullRequest & { state?: string }): string;
export function reviewStatus(decision: string | null): string;
export function checkStatus(check: StatusCheck): {
  name: string;
  tone: string;
  text: string;
};
export function checkSummary(checks: StatusCheck[] | null): {
  tone: string;
  text: string;
};
