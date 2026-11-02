import {
  promptWithFileContext,
  splitFileContext,
} from "../files/fileContext.js";
import {
  promptWithPullRequests,
  splitPullRequestContext,
} from "../pull-requests/pullRequestContext.js";

export function promptWithContext(text, files, pullRequests) {
  return promptWithFileContext(
    promptWithPullRequests(text, pullRequests),
    files,
  );
}

export function splitPromptContext(prompt) {
  const { text, files } = splitFileContext(prompt);
  return { ...splitPullRequestContext(text), files };
}
