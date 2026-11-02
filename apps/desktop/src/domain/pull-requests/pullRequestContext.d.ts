import type { PullRequestDetails } from "./pullRequestState.js";

export type PullRequestContext = {
  id: string;
  number: number;
  title: string;
  url: string;
  state: string;
  head: string;
  base: string;
  commit: string;
  capturedAt: number;
  checks: { name: string; status: string; url: string | null }[] | null;
  checksTruncated: boolean;
};
export const MAX_PULL_REQUEST_CONTEXT: number;
export function createPullRequestContext(
  pr: PullRequestDetails,
): PullRequestContext;
export function promptWithPullRequests(
  text: string,
  pullRequests: PullRequestContext[],
): string;
export function splitPullRequestContext(prompt: string): {
  text: string;
  pullRequests: PullRequestContext[];
};
