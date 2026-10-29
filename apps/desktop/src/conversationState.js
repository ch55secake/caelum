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

const isText = (entry) => ["assistant", "reasoning"].includes(entry.kind);
const openCodePartId = (messageId, kind, ordinal = 0) =>
  kind === "text" && ordinal === 0
    ? `opencode-${messageId}`
    : `opencode-${messageId}-${kind}-${ordinal}`;
const reasoningParts = (parts = []) =>
  Object.fromEntries(
    parts.map((part, index) => [
      index,
      { text: typeof part === "string" ? part : (part.text ?? "") },
    ]),
  );
const reasoningText = (reasoning) => {
  const join = (parts) =>
    Object.values(parts ?? {})
      .map((part) => part.text)
      .join("\n\n");
  return join(reasoning.summary) || join(reasoning.content);
};

function mergeText(snapshot, current = {}) {
  const streamText =
    current.streamText ??
    (current.historyText === undefined ? (current.text ?? "") : "");
  return {
    historyText: snapshot.text,
    streamText,
    text: snapshot.complete
      ? snapshot.text
      : current.complete && current.text !== undefined
        ? current.text
        : reconcileText(snapshot.text, streamText),
  };
}

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
  if (item.type === "reasoning") {
    const reasoning = {
      summary: reasoningParts(item.summary),
      content: reasoningParts(item.content),
    };
    return {
      ...base,
      kind: "reasoning",
      reasoning,
      text: reasoningText(reasoning),
      complete: completed,
    };
  }
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
    input:
      item.command ?? item.arguments ?? item.changes ?? item.query ?? item,
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
    // V2 ordinals count fragments of each kind within an assistant message.
    const ordinals = { text: 0, reasoning: 0 };
    for (const part of message.content ?? []) {
      if (part.type === "text" || part.type === "reasoning") {
        entries.push({
          id: openCodePartId(message.id, part.type, ordinals[part.type]++),
          messageId: message.id,
          kind: part.type === "text" ? "assistant" : "reasoning",
          text: part.text ?? "",
          complete: Boolean(
            part.time?.completed || message.time?.completed || message.finish,
          ),
        });
      }
      if (part.type === "tool") {
        const state = object(part.state);
        entries.push({
          id: `opencode-tool-${part.id}`,
          messageId: message.id,
          kind: "tool",
          text: "",
          name: part.name,
          status: status(state.status),
          input: state.input,
          output: textContent(state.content),
          error: state.error
            ? displayValue(state.error.message ?? state.error)
            : undefined,
          startedAt: part.time?.ran,
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

export function mergeHistory(
  live,
  history,
  mode = "prepend",
  beforeId = null,
) {
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
    if (!current) current = { id: entry.id };
    adopted.add(current.id);
    if (current.optimistic) return entry;
    const result = {
      ...entry,
      ...Object.fromEntries(
        Object.entries(current).filter(([, value]) => value !== undefined),
      ),
    };
    if (isText(entry)) {
      Object.assign(result, mergeText(entry, current));
      result.complete = Boolean(current.complete || entry.complete);
      if (entry.reasoning) {
        result.reasoning = Object.fromEntries(
          ["summary", "content"].map((kind) => {
            const parts = { ...current.reasoning?.[kind] };
            for (const [index, part] of Object.entries(entry.reasoning[kind]))
              parts[index] = mergeText(
                { ...part, complete: entry.complete },
                { ...parts[index], complete: current.complete },
              );
            return [kind, parts];
          }),
        );
        result.text = reasoningText(result.reasoning);
      }
      if (entry.complete) result.status = entry.status;
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
  const working = (label = "Working…", detail) => {
    next = {
      ...next,
      isWorking: true,
      activity: { label, detail },
      lastOutcome: null,
    };
  };
  const settle = (outcome) => {
    next = {
      ...next,
      isWorking: false,
      isSubmitting: false,
      activity: null,
      lastOutcome: outcome,
      conversation: next.conversation.map((entry) =>
        entry.kind === "tool" && entry.status === "running"
          ? { ...entry, status: status(outcome) }
          : isText(entry) && !entry.complete
            ? { ...entry, complete: true, status: status(outcome) }
            : entry,
      ),
    };
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
    if (kind === "assistant" || kind === "reasoning") {
      const streamText =
        (existing?.streamText ??
          (existing?.historyText === undefined
            ? (existing?.text ?? "")
            : "")) + text;
      put({
        ...existing,
        id,
        kind,
        ...(existing?.status === "interrupted" ? { status: undefined } : {}),
        streamText,
        text: reconcileText(existing?.historyText ?? "", streamText),
      });
      working(kind === "reasoning" ? "Thinking…" : "Responding…");
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
      working("Running tool…");
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
    if (["item/started", "item/completed"].includes(method)) {
      const entry = codexItem(object(data.item), method === "item/completed");
      if (entry?.kind === "tool") {
        const previous = next.conversation.find(
          (item) => item.id === entry.id,
        );
        entry.startedAt = previous?.startedAt ?? Date.now();
        if (method === "item/completed")
          entry.durationMs ??= Date.now() - entry.startedAt;
      }
      if (entry && method === "item/started")
        next = {
          ...next,
          conversation: mergeHistory(next.conversation, [entry], "refresh"),
        };
      else put(entry);
      if (method === "item/started")
        working(data.item?.type === "reasoning" ? "Thinking…" : "Working…");
      else if (next.isWorking) working();
    }
    if (
      [
        "item/reasoning/summaryTextDelta",
        "item/reasoning/summaryPartAdded",
        "item/reasoning/textDelta",
      ].includes(method)
    ) {
      const id = `codex-${data.itemId}`;
      const previous = next.conversation.find((entry) => entry.id === id);
      if (previous?.complete) return next;
      const kind =
        method === "item/reasoning/textDelta" ? "content" : "summary";
      const index =
        (kind === "summary" ? data.summaryIndex : data.contentIndex) ?? 0;
      if (!Number.isInteger(index) || index < 0) return next;
      const reasoning = { summary: {}, content: {}, ...previous?.reasoning };
      const part = reasoning[kind][index];
      const streamText =
        (part?.streamText ??
          (part?.historyText === undefined ? (part?.text ?? "") : "")) +
        (data.delta ?? "");
      reasoning[kind] = {
        ...reasoning[kind],
        [index]: {
          ...part,
          streamText,
          text: reconcileText(part?.historyText ?? "", streamText),
        },
      };
      put({
        ...previous,
        id,
        kind: "reasoning",
        ...(previous?.status === "interrupted" ? { status: undefined } : {}),
        reasoning,
        text: reasoningText(reasoning),
        complete: false,
      });
      working("Thinking…");
    }
    if (method === "item/agentMessage/delta")
      delta(`codex-${data.itemId}`, "assistant", data.delta);
    if (method === "item/commandExecution/outputDelta")
      delta(`codex-${data.itemId}`, "tool", data.delta, "output");
    if (method === "turn/started") {
      working();
      next = { ...next, error: null };
    }
    if (method === "thread/status/changed") {
      if (data.status?.type === "active") working();
      else next = { ...next, isWorking: false, activity: null };
    }
    if (method === "turn/completed") {
      const turnStatus = data.turn?.status?.type ?? data.turn?.status;
      settle(turnStatus);
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
        error: displayValue(
          data.error?.message ?? data.message ?? data.error,
        ),
      };
  } else {
    if (
      method === "session.model.selected" &&
      data.model?.providerID &&
      data.model?.id
    )
      next = {
        ...next,
        session: {
          ...next.session,
          model: `${data.model.providerID}/${data.model.id}`,
          effort: data.model.variant ?? "",
        },
      };
    if (/^session\.(text|reasoning)\.(started|delta|ended)$/.test(method)) {
      const [, type, phase] = method.split(".");
      const messageId = data.assistantMessageID ?? data.messageID;
      if (!messageId) return next;
      const id = openCodePartId(messageId, type, data.ordinal ?? 0);
      const kind = type === "text" ? "assistant" : "reasoning";
      const previous = next.conversation.find((entry) => entry.id === id);
      if (phase === "ended") {
        put({
          ...previous,
          id,
          messageId,
          kind,
          text: data.text ?? previous?.text ?? "",
          complete: true,
        });
        if (next.isWorking) working();
      } else if (!previous?.complete) {
        if (!previous)
          put({ id, messageId, kind, text: "", complete: false });
        if (phase === "delta") delta(id, kind, data.delta ?? data.text);
        working(kind === "reasoning" ? "Thinking…" : "Responding…");
      }
    }
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
        messageId: data.assistantMessageID ?? previous?.messageId,
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
      if (!["session.tool.success", "session.tool.failed"].includes(method))
        working("Running tool…", data.name ?? previous?.name);
      else if (next.isWorking) working();
    }
    if (data.message?.type)
      next = {
        ...next,
        conversation: mergeHistory(
          next.conversation,
          openCodeMessage(data.message),
          "refresh",
        ),
      };
    if (
      ["session.execution.started", "session.step.started"].includes(method)
    ) {
      working();
      next = { ...next, error: null };
    }
    if (["session.step.ended", "session.step.failed"].includes(method))
      next = {
        ...next,
        conversation: next.conversation.map((entry) =>
          entry.messageId === data.assistantMessageID && isText(entry)
            ? { ...entry, complete: true }
            : entry,
        ),
      };
    if (
      method === "session.retry.scheduled" ||
      (method === "session.status" && data.status?.type === "retry")
    ) {
      const retry = method === "session.status" ? data.status : data;
      working(
        `Retrying · attempt ${retry.attempt ?? 1}`,
        displayValue(retry.message ?? retry.error?.message),
      );
    }
    if (method === "session.status") {
      if (data.status?.type === "busy" && !next.isWorking) working();
      if (data.status?.type === "idle") settle("completed");
    }
    if (
      [
        "session.execution.succeeded",
        "session.execution.failed",
        "session.execution.interrupted",
        "session.idle",
      ].includes(method)
    ) {
      const outcome = method.split(".").at(-1);
      settle(
        outcome === "succeeded" || outcome === "idle" ? "completed" : outcome,
      );
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
