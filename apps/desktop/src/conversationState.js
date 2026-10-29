const object = (value) => (value && typeof value === "object" ? value : {});
export const displayValue = (value) =>
  typeof value === "string"
    ? value
    : value == null
      ? ""
      : JSON.stringify(value, null, 2);
const textContent = (content) =>
  Array.isArray(content)
    ? content.map((part) => part.text ?? displayValue(part)).join("\n")
    : displayValue(content);

// Reconcile a snapshot prefix with an independently arriving stream in linear
// time, including when SSE delivery lags behind the history response.
function reconcileText(snapshot, stream) {
  if (!stream || snapshot.includes(stream)) return snapshot;
  if (!snapshot || stream.includes(snapshot)) return stream;
  const failure = new Uint32Array(stream.length);
  for (let i = 1, matched = 0; i < stream.length; i++) {
    while (matched && stream[i] !== stream[matched])
      matched = failure[matched - 1];
    if (stream[i] === stream[matched]) matched++;
    failure[i] = matched;
  }
  let matched = 0;
  for (let index = 0; index < snapshot.length; index++) {
    const character = snapshot[index];
    while (
      matched &&
      (matched === stream.length || character !== stream[matched])
    )
      matched = failure[matched - 1];
    if (character === stream[matched]) matched++;
  }
  return snapshot + stream.slice(matched);
}
const status = (value) =>
  ["failed", "error", "declined"].includes(value)
    ? "failed"
    : ["completed", "success", "exited"].includes(value)
      ? "completed"
      : ["interrupted", "cancelled", "killed"].includes(value)
        ? "interrupted"
        : "running";

export function codexItem(item, completed = false) {
  if (!item.id) return null;
  const base = { id: `codex-${item.id}` };
  if (item.type === "userMessage")
    return {
      ...base,
      kind: "user",
      text: (item.content ?? [])
        .map((part) => part.text ?? (part.type === "image" ? "[Image]" : ""))
        .join("\n"),
    };
  if (item.type === "agentMessage")
    return {
      ...base,
      kind: "assistant",
      text: item.text ?? "",
      complete: completed,
    };
  if (item.type === "reasoning")
    return {
      ...base,
      kind: "reasoning",
      text: [...(item.summary ?? []), ...(item.content ?? [])]
        .map((part) => (typeof part === "string" ? part : (part.text ?? "")))
        .join("\n"),
    };
  const names = {
    commandExecution: "Run",
    fileChange: "Edit",
    mcpToolCall: item.tool ?? "MCP",
    webSearch: "Search web",
    imageView: "View image",
    dynamicToolCall: item.tool ?? "Tool",
  };
  return {
    ...base,
    kind: "tool",
    text: "",
    name: names[item.type] ?? item.type,
    status: item.exitCode
      ? "failed"
      : status(item.status ?? (completed ? "completed" : "running")),
    input: item.command ?? item.arguments ?? item.changes ?? item.query ?? item,
    output: item.aggregatedOutput ?? item.result ?? item.content,
    error: item.error
      ? displayValue(item.error)
      : item.exitCode
        ? `Exit code ${item.exitCode}`
        : undefined,
    durationMs: item.durationMs,
  };
}

export function openCodeMessage(message) {
  const id = `opencode-${message.id}`;
  if (message.type === "user")
    return [{ id, kind: "user", text: message.text ?? "" }];
  if (message.type === "assistant") {
    const entries = [];
    // Text deltas are message-scoped in V2; keep the same stable identity on restore.
    const text = (message.content ?? [])
      .filter((part) => part.type === "text")
      .map((part) => part.text)
      .join("");
    if (text)
      entries.push({
        id,
        kind: "assistant",
        text,
        complete: Boolean(message.time?.completed || message.finish),
      });
    for (const part of message.content ?? []) {
      if (part.type === "tool") {
        const state = object(part.state);
        entries.push({
          id: `opencode-tool-${part.id}`,
          kind: "tool",
          text: "",
          name: part.name,
          status: status(state.status),
          input: state.input,
          output: textContent(state.content),
          error: state.error
            ? displayValue(state.error.message ?? state.error)
            : undefined,
          durationMs:
            part.time?.completed && part.time?.ran
              ? part.time.completed - part.time.ran
              : undefined,
        });
      }
    }
    return entries;
  }
  if (message.type === "shell")
    return [
      {
        id,
        kind: "tool",
        text: "",
        name: "Run",
        status: status(message.status),
        input: message.command,
        output: message.output?.output,
      },
    ];
  if (message.type === "idle" && message.outcome !== "succeeded")
    return [
      {
        id,
        kind: "status",
        text:
          message.outcome === "interrupted"
            ? "Turn cancelled"
            : displayValue(message.error?.message ?? "Turn failed"),
      },
    ];
  if (["system", "synthetic"].includes(message.type))
    return [{ id, kind: "status", text: message.text ?? "" }];
  return [];
}

export function historyEntries(provider, data) {
  if (provider === "opencode") return data.flatMap(openCodeMessage);
  return data.flatMap((turn) =>
    (turn.items ?? [])
      .map((item) => codexItem(item, turn.status !== "inProgress"))
      .filter(Boolean),
  );
}

export function upsertEntry(entries, entry) {
  let index = entries.findIndex((current) => current.id === entry.id);
  // Adopt the provider's identity for a local optimistic message exactly once.
  if (index < 0 && entry.kind === "user")
    index = entries.findIndex(
      (current) => current.optimistic && current.text === entry.text,
    );
  if (index < 0) return [...entries, entry];
  const next = [...entries];
  next[index] = {
    ...next[index],
    ...entry,
    optimistic: entry.optimistic ?? false,
  };
  return next;
}

export function mergeHistory(live, history, mode = "prepend", beforeId = null) {
  const liveById = new Map(live.map((entry) => [entry.id, entry]));
  const adopted = new Set();
  const merged = history.map((entry) => {
    let current = liveById.get(entry.id);
    // Only the newest matching persisted message may adopt a local submission.
    if (
      !current &&
      entry.kind === "user" &&
      history.findLast(
        (candidate) =>
          candidate.kind === "user" && candidate.text === entry.text,
      ) === entry
    )
      current = live.find(
        (candidate) =>
          candidate.id.startsWith("local-") &&
          candidate.optimistic &&
          candidate.text === entry.text &&
          !adopted.has(candidate.id),
      );
    if (!current)
      return entry.kind === "assistant"
        ? { ...entry, historyText: entry.text, streamText: "" }
        : entry;
    adopted.add(current.id);
    if (current.optimistic) return entry;
    const result = {
      ...entry,
      ...Object.fromEntries(
        Object.entries(current).filter(([, value]) => value !== undefined),
      ),
    };
    if (entry.kind === "assistant") {
      result.historyText = entry.text;
      result.streamText =
        current.streamText ??
        (current.historyText === undefined ? current.text : "");
      result.complete = Boolean(current.complete || entry.complete);
      result.text = current.complete
        ? current.text
        : entry.complete
          ? entry.text
          : reconcileText(entry.text, result.streamText);
    }
    if (
      entry.kind === "tool" &&
      ["running", "interrupted"].includes(current.status) &&
      entry.status !== "running"
    )
      return { ...result, ...entry };
    if (entry.output && !current.output) result.output = entry.output;
    if (
      entry.kind === "tool" &&
      entry.status === "running" &&
      current.status === "running" &&
      typeof entry.output === "string"
    ) {
      result.historyOutput = entry.output;
      result.streamOutput =
        current.streamOutput ??
        (current.historyOutput === undefined
          ? displayValue(current.output)
          : "");
      result.output = reconcileText(entry.output, result.streamOutput);
    }
    return result;
  });
  const historyIds = new Set(history.map((entry) => entry.id));
  const first = live.findIndex(
    (entry) => historyIds.has(entry.id) || adopted.has(entry.id),
  );
  const before = beforeId
    ? live.findIndex((entry) => entry.id === beforeId)
    : -1;
  const insertion =
    first >= 0
      ? first
      : before >= 0
        ? before
        : mode === "refresh" &&
            !live.some(
              (entry) => historyIds.has(entry.id) || adopted.has(entry.id),
            )
          ? live.length
          : 0;
  const keep = (entry) => !historyIds.has(entry.id) && !adopted.has(entry.id);
  return [
    ...live.slice(0, insertion).filter(keep),
    ...merged,
    ...live.slice(insertion).filter(keep),
  ];
}

export function applyConversationEvent(session, provider, method, data) {
  let next = session;
  const put = (entry) => {
    if (entry)
      next = { ...next, conversation: upsertEntry(next.conversation, entry) };
  };
  const delta = (id, kind, text, field = "text") => {
    if (!text) return;
    const existing = next.conversation.find((entry) => entry.id === id);
    if (
      existing?.complete ||
      (existing?.kind === "tool" &&
        ["completed", "failed"].includes(existing.status))
    )
      return;
    if (kind === "assistant") {
      const streamText =
        (existing?.streamText ??
          (existing?.historyText === undefined ? (existing?.text ?? "") : "")) +
        text;
      put({
        ...existing,
        id,
        kind,
        streamText,
        text: reconcileText(existing?.historyText ?? "", streamText),
      });
      return;
    }
    if (field === "output") {
      const streamOutput =
        (existing?.streamOutput ??
          (existing?.historyOutput === undefined
            ? displayValue(existing?.output)
            : "")) + text;
      put({
        ...existing,
        id,
        kind,
        text: "",
        streamOutput,
        output: reconcileText(existing?.historyOutput ?? "", streamOutput),
      });
      return;
    }
    put({
      ...existing,
      id,
      kind,
      text: existing?.text ?? "",
      [field]: (existing?.[field] ?? "") + text,
    });
  };
  if (provider === "codex") {
    if (
      method === "item/started" &&
      next.conversation.some(
        (entry) =>
          entry.id === `codex-${data.item?.id}` &&
          (entry.complete || ["completed", "failed"].includes(entry.status)),
      )
    )
      return next;
    if (["item/started", "item/completed"].includes(method))
      put(codexItem(object(data.item), method === "item/completed"));
    if (method === "item/agentMessage/delta")
      delta(`codex-${data.itemId}`, "assistant", data.delta);
    if (method === "item/commandExecution/outputDelta")
      delta(`codex-${data.itemId}`, "tool", data.delta, "output");
    if (method === "turn/started")
      next = { ...next, isWorking: true, error: null };
    if (method === "thread/status/changed")
      next = { ...next, isWorking: data.status?.type === "active" };
    if (method === "turn/completed") {
      const turnStatus = data.turn?.status?.type ?? data.turn?.status;
      next = {
        ...next,
        isWorking: false,
        isSubmitting: false,
        lastOutcome: turnStatus,
        conversation: next.conversation.map((entry) =>
          entry.kind === "tool" && entry.status === "running"
            ? {
                ...entry,
                status:
                  turnStatus === "completed"
                    ? "completed"
                    : turnStatus === "failed"
                      ? "failed"
                      : "interrupted",
              }
            : entry,
        ),
      };
      if (["interrupted", "failed"].includes(turnStatus))
        put({
          id: `codex-turn-${data.turn.id}`,
          kind: "status",
          text:
            turnStatus === "interrupted"
              ? "Turn cancelled"
              : displayValue(data.turn.error?.message ?? "Turn failed"),
        });
    }
    if (method === "error")
      next = {
        ...next,
        error: displayValue(data.error?.message ?? data.message ?? data.error),
      };
  } else {
    if (method === "session.text.delta")
      delta(
        `opencode-${data.assistantMessageID ?? data.messageID}`,
        "assistant",
        data.delta ?? data.text,
      );
    if (method.startsWith("session.tool.")) {
      const id = `opencode-tool-${data.id ?? data.partID}`;
      const previous = next.conversation.find((entry) => entry.id === id);
      if (
        ["completed", "failed"].includes(previous?.status) &&
        !["session.tool.success", "session.tool.failed"].includes(method)
      )
        return next;
      const input =
        method === "session.tool.input.delta"
          ? displayValue(previous?.input) + (data.delta ?? "")
          : (data.input ??
            (method === "session.tool.input.ended" ? data.text : undefined) ??
            previous?.input);
      put({
        ...previous,
        id,
        kind: "tool",
        text: "",
        name: data.name ?? data.tool ?? previous?.name ?? "Tool",
        input,
        status: method.endsWith("success")
          ? "completed"
          : method.endsWith("failed")
            ? "failed"
            : "running",
        output: data.content
          ? textContent(data.content)
          : (data.output ?? previous?.output),
        error: data.error
          ? displayValue(data.error.message ?? data.error)
          : previous?.error,
        startedAt: previous?.startedAt ?? Date.now(),
        durationMs:
          ["session.tool.success", "session.tool.failed"].includes(method) &&
          previous?.startedAt
            ? Date.now() - previous.startedAt
            : previous?.durationMs,
      });
    }
    if (data.message?.type)
      for (const entry of openCodeMessage(data.message)) put(entry);
    if (method === "session.execution.started")
      next = { ...next, isWorking: true, error: null };
    if (
      [
        "session.execution.succeeded",
        "session.execution.failed",
        "session.execution.interrupted",
        "session.idle",
      ].includes(method)
    ) {
      next = {
        ...next,
        isWorking: false,
        isSubmitting: false,
        lastOutcome: method.split(".").at(-1),
        conversation: next.conversation.map((entry) =>
          entry.kind === "tool" && entry.status === "running"
            ? {
                ...entry,
                status: method.endsWith("interrupted")
                  ? "interrupted"
                  : method.endsWith("failed")
                    ? "failed"
                    : "completed",
              }
            : entry,
        ),
      };
      if (method.endsWith("failed") || method.endsWith("interrupted"))
        put({
          id: `opencode-outcome-${data.messageID ?? Date.now()}`,
          kind: "status",
          text: method.endsWith("interrupted")
            ? "Turn cancelled"
            : displayValue(data.error?.message ?? "Turn failed"),
        });
    }
    if (method === "permission.reply.failed")
      next = {
        ...next,
        error: data.message ?? "Could not apply the permission decision",
      };
  }
  return next;
}
