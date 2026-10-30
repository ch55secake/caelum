const tokenCount = (value) =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : null;

export function contextWindow(value) {
  return tokenCount(value) > 0 ? value : null;
}

export function openCodeModel(model) {
  return typeof model?.providerID === "string" &&
    typeof model?.id === "string"
    ? `${model.providerID}/${model.id}`
    : null;
}

export function codexContextUsage(usage) {
  const usedTokens = tokenCount(usage?.last?.totalTokens);
  if (usedTokens === null) return null;
  // `total` is cumulative billing usage, not the current context. Cached
  // input and reasoning are already included in Codex's last-request total.
  return {
    usedTokens,
    contextWindow: contextWindow(usage.modelContextWindow),
    model: null,
    timestamp: null,
  };
}

export function openCodeContextUsage(tokens, model, timestamp = null) {
  const counts = [
    tokens?.input,
    tokens?.output,
    tokens?.cache?.read,
    tokens?.cache?.write,
  ];
  if (counts.some((value) => tokenCount(value) === null)) return null;
  // OpenCode separates cached input from uncached input; reasoning is a
  // breakdown of output and must not be added again.
  const usedTokens = tokenCount(
    counts.reduce((sum, value) => sum + value, 0),
  );
  return usedTokens === null
    ? null
    : {
        usedTokens,
        contextWindow: null,
        model,
        timestamp: tokenCount(timestamp),
      };
}

export function openCodeHistoryUsage(messages, since = 0) {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    // A projected snapshot may lag the live stream even if it was requested
    // later. Never restore usage from before a newer step or compaction.
    if (since > 0 && (tokenCount(message.time?.created) ?? -1) < since)
      continue;
    // Compaction's own token bill describes its old input, not the new context.
    if (message.type === "compaction" && message.status === "completed")
      return null;
    if (message.type !== "assistant") continue;
    const usage = openCodeContextUsage(
      message.tokens,
      openCodeModel(message.model),
      message.time?.created,
    );
    if (usage) return usage;
  }
  // A partial history page may not contain the last measured request.
  return undefined;
}

export function contextUsageSince(usage, conversation) {
  return conversation.reduce(
    (since, entry) =>
      entry.kind === "compaction" && entry.status === "completed"
        ? Math.max(since, entry.startedAt ?? 0)
        : since,
    usage?.timestamp ?? 0,
  );
}
