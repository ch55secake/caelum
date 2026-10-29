const PROVIDERS = ["codex", "opencode"];

function emptyProviderWorkspace() {
  return {
    sessions: [],
    sessionsLoaded: false,
    isLoadingSessions: false,
    selectedSessionId: null,
    sessionsById: {},
    error: null,
  };
}

export function createProjectWorkspace() {
  return {
    provider: "codex",
    prompt: "",
    providers: {
      codex: emptyProviderWorkspace(),
      opencode: emptyProviderWorkspace(),
    },
  };
}

export function updateProjectWorkspace(workspaces, projectPath, update) {
  const current = workspaces[projectPath] ?? createProjectWorkspace();
  const next = update(current);
  if (next === current) return workspaces;
  return { ...workspaces, [projectPath]: next };
}

export function updateProviderWorkspace(workspaces, projectPath, provider, update) {
  return updateProjectWorkspace(workspaces, projectPath, (workspace) => {
    const current = workspace.providers[provider];
    const next = update(current);
    if (next === current) return workspace;
    return {
      ...workspace,
      providers: { ...workspace.providers, [provider]: next },
    };
  });
}

export function updateSessionWorkspace(workspaces, projectPath, provider, sessionId, update) {
  return updateProviderWorkspace(workspaces, projectPath, provider, (providerWorkspace) => {
    const current = providerWorkspace.sessionsById[sessionId];
    if (!current) return providerWorkspace;
    const next = update(current);
    if (next === current) return providerWorkspace;
    return {
      ...providerWorkspace,
      sessionsById: { ...providerWorkspace.sessionsById, [sessionId]: next },
    };
  });
}

export function projectHasActiveTurn(workspace) {
  return PROVIDERS.some((provider) =>
    Object.values(workspace.providers[provider].sessionsById).some((session) => session.isWorking),
  );
}

function record(value) {
  return value && typeof value === "object" ? value : {};
}

function string(value) {
  return typeof value === "string" ? value : undefined;
}

function addEntry(session, entry) {
  return { ...session, conversation: [...session.conversation, entry].slice(-300) };
}

function appendDelta(session, id, kind, delta) {
  const existing = session.conversation.findIndex((entry) => entry.id === id);
  if (existing < 0) return addEntry(session, { id, kind, text: delta });
  const conversation = [...session.conversation];
  conversation[existing] = { ...conversation[existing], text: conversation[existing].text + delta };
  return { ...session, conversation };
}

function codexActivity(item) {
  const value = record(item);
  const type = string(value.type) ?? "tool";
  if (type === "commandExecution") {
    const command = Array.isArray(value.command) ? value.command.join(" ") : string(value.command);
    return command ? `Running command: ${command}` : "Running a command";
  }
  if (type === "fileChange") return "Preparing file changes";
  if (type === "mcpToolCall") return `Using tool: ${string(value.tool) ?? "MCP tool"}`;
  if (type === "webSearch") return `Searching the web: ${string(value.query) ?? ""}`.trim();
  if (type === "reasoning") return undefined;
  return `Codex activity: ${type.replace(/([A-Z])/g, " $1").toLowerCase()}`;
}

export function applyCodexEvent(workspaces, event) {
  const projectPath = event.projectPath;
  if (event.method === "codex/processExited") {
    return updateProviderWorkspace(workspaces, projectPath, "codex", (workspace) => ({
      ...workspace,
      error: "The Codex app-server stopped. Start or resume a session to reconnect.",
      sessionsById: Object.fromEntries(Object.entries(workspace.sessionsById).map(([id, session]) => [
        id,
        { ...session, isWorking: false, isSubmitting: false },
      ])),
    }));
  }

  const params = record(event.params);
  const thread = record(params.thread);
  const threadId = string(params.threadId) ?? string(thread.id);
  if (!threadId) return workspaces;

  return updateSessionWorkspace(workspaces, projectPath, "codex", threadId, (session) => {
    if (event.method === "turn/started") return { ...session, isWorking: true };
    if (event.method === "turn/completed") {
      const turn = record(params.turn);
      const statusValue = turn.status;
      const status = statusValue && typeof statusValue === "object"
        ? string(record(statusValue).type)
        : string(statusValue);
      return addEntry({ ...session, isWorking: false, isSubmitting: false }, {
        id: `turn-${String(turn.id ?? Date.now())}`,
        kind: "status",
        text: status === "interrupted" ? "Turn cancelled" : status === "failed" ? "Codex could not complete this turn" : "Turn complete",
      });
    }
    if (event.method === "item/agentMessage/delta") {
      const delta = string(params.delta);
      if (!delta) return session;
      const itemId = string(params.itemId) ?? `stream-${threadId}`;
      return appendDelta(session, `assistant-${itemId}`, "assistant", delta);
    }
    if (event.method === "item/started") {
      const activity = codexActivity(params.item);
      if (!activity) return session;
      const id = string(record(params.item).id) ?? `${Date.now()}`;
      return addEntry(session, { id: `activity-${id}`, kind: "activity", text: activity });
    }
    if (event.method === "item/commandExecution/outputDelta") {
      const id = string(params.itemId);
      const delta = string(params.delta);
      if (!id || !delta) return session;
      const entryId = `activity-${id}`;
      const existing = session.conversation.findIndex((entry) => entry.id === entryId);
      if (existing < 0) {
        return addEntry(session, { id: entryId, kind: "activity", text: `Command output:\n${delta}`.slice(-12000) });
      }
      const conversation = [...session.conversation];
      conversation[existing] = { ...conversation[existing], text: `${conversation[existing].text}\n${delta}`.slice(-12000) };
      return { ...session, conversation };
    }
    if (event.method === "item/completed") {
      const item = record(params.item);
      const id = string(item.id);
      const status = string(item.status);
      if (!id || !status) return session;
      return {
        ...session,
        conversation: session.conversation.map((entry) => entry.id !== `activity-${id}`
          ? entry
          : { ...entry, text: `${entry.text} (${status === "completed" ? "complete" : status})` }),
      };
    }
    return session;
  });
}

function openCodeToolInput(value) {
  if (typeof value === "string") {
    try {
      return openCodeToolInput(JSON.parse(value)) ?? value;
    } catch {
      return value;
    }
  }
  const input = record(value);
  const command = string(input.command);
  if (command) return command;
  return Object.keys(input).length > 0 ? JSON.stringify(input) : undefined;
}

export function applyOpenCodeEvent(workspaces, event) {
  const projectPath = event.projectPath;
  if (event.eventType === "server.disconnected") {
    return updateProviderWorkspace(workspaces, projectPath, "opencode", (workspace) => ({
      ...workspace,
      error: "The OpenCode server disconnected. Start or resume a session to reconnect.",
      sessionsById: Object.fromEntries(Object.entries(workspace.sessionsById).map(([id, session]) => [
        id,
        { ...session, isWorking: false, isSubmitting: false },
      ])),
    }));
  }

  const data = record(event.data);
  const sessionId = string(data.sessionID);
  if (!sessionId) return workspaces;
  if (event.eventType === "permission.reply.failed") {
    const message = `OpenCode could not apply the permission decision: ${string(data.message) ?? "server request failed"}`;
    const current = workspaces[projectPath]?.providers.opencode.sessionsById[sessionId];
    if (current) {
      return updateSessionWorkspace(workspaces, projectPath, "opencode", sessionId, (session) => ({ ...session, error: message }));
    }
    return updateProviderWorkspace(workspaces, projectPath, "opencode", (workspace) => ({ ...workspace, error: message }));
  }

  return updateSessionWorkspace(workspaces, projectPath, "opencode", sessionId, (session) => {
    if (event.eventType === "session.execution.started") return { ...session, isWorking: true };
    if (["session.execution.succeeded", "session.execution.failed", "session.execution.interrupted"].includes(event.eventType)) {
      const error = record(data.error);
      const errorMessage = string(error.message) ?? string(data.error);
      const outcome = event.eventType === "session.execution.interrupted"
        ? "Turn cancelled"
        : event.eventType === "session.execution.failed"
          ? `OpenCode could not complete this turn${errorMessage ? `: ${errorMessage}` : ""}`
          : "Turn complete";
      return addEntry({ ...session, isWorking: false, isSubmitting: false }, {
        id: `opencode-turn-${string(data.messageID) ?? Date.now()}`,
        kind: "status",
        text: outcome,
      });
    }
    if (event.eventType === "session.idle") return { ...session, isWorking: false, isSubmitting: false };
    if (event.eventType === "session.text.delta") {
      const delta = string(data.delta) ?? string(data.text);
      if (!delta) return session;
      const messageId = string(data.assistantMessageID) ?? string(data.messageID) ?? `stream-${sessionId}`;
      return appendDelta(session, `opencode-assistant-${messageId}`, "assistant", delta);
    }
    if ([
      "session.tool.input.started",
      "session.tool.input.delta",
      "session.tool.input.ended",
      "session.tool.called",
      "session.tool.progress",
      "session.tool.success",
      "session.tool.failed",
    ].includes(event.eventType)) {
      const toolId = string(data.id) ?? string(data.partID);
      if (!toolId) return session;
      const entryId = `opencode-tool-${toolId}`;
      const existing = session.conversation.findIndex((entry) => entry.id === entryId);
      const existingText = existing < 0 ? "" : session.conversation[existing].text;
      const existingName = existingText.match(/^Using tool: (.+?) \(/)?.[1];
      const toolName = string(data.name) ?? string(data.tool) ?? existingName ?? "tool";
      const existingCommand = existingText.split("\n").slice(1).join("\n");
      const delta = string(data.delta);
      const accumulated = delta ? `${existingCommand}${delta}` : existingCommand;
      const eventInput = event.eventType === "session.tool.input.ended"
        ? openCodeToolInput(data.text)
        : openCodeToolInput(data.input);
      const command = eventInput ?? (accumulated || openCodeToolInput(data.input) || "");
      const status = event.eventType === "session.tool.success"
        ? "complete"
        : event.eventType === "session.tool.failed"
          ? "failed"
          : event.eventType === "session.tool.progress" || event.eventType === "session.tool.called"
            ? "running"
            : "preparing";
      const toolError = string(record(data.error).message);
      const detail = command || (event.eventType === "session.tool.failed" ? toolError : undefined);
      const entry = {
        id: entryId,
        kind: "activity",
        text: `Using tool: ${toolName} (${status})${detail ? `\n${detail}` : ""}`,
      };
      if (existing < 0) return addEntry(session, entry);
      const conversation = [...session.conversation];
      conversation[existing] = entry;
      return { ...session, conversation };
    }
    return session;
  });
}
