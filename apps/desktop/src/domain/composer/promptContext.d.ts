import type { FileContext } from "../files/fileContext.js";
import type { PullRequestContext } from "../pull-requests/pullRequestContext.js";

export function promptWithContext(
  text: string,
  files: FileContext[],
  pullRequests: PullRequestContext[],
): string;
export function splitPromptContext(prompt: string): {
  text: string;
  files: FileContext[];
  pullRequests: PullRequestContext[];
};
