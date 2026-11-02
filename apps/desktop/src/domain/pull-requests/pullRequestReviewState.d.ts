import type { PullRequestDetails } from "./pullRequestState.js";

export type PullRequestFile = {
  path: string;
  previousPath: string | null;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
};
export type ReviewComment = {
  id: string;
  body: string;
  url: string;
  author: { login: string } | null;
  createdAt: string;
  state: string | null;
  diffHunk: string | null;
};
export type ReviewThread = {
  id: string;
  path: string;
  line: number | null;
  startLine: number | null;
  originalLine: number | null;
  diffSide: string;
  isResolved: boolean;
  isOutdated: boolean;
  comments: ReviewComment[];
  nextCursor: string | null;
};
export type ReviewPage = {
  files?: PullRequestFile[];
  baseOid?: string;
  nextPage?: number | null;
  comments?: ReviewComment[];
  threads?: ReviewThread[];
  nextCursor?: string | null;
};
export type ReviewFeedback = {
  id: string;
  kind: "discussion" | "review" | "thread";
  comments: ReviewComment[];
  thread?: ReviewThread;
};
export function feedbackSelectedByDefault(item: ReviewFeedback): boolean;
export function reviewPrompt(
  pr: PullRequestDetails,
  feedback: ReviewFeedback[],
): string;
