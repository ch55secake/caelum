import { sessionWaiting } from "./workspaceState.js";

export const activityKey = (path, provider, id = "") =>
  `${path}\0${provider}\0${id}`;
export const READ_MARKERS_KEY = "caelum.session-read.v1";
export const MAX_READ_MARKERS = 2000;

export function emptyActivity() {
  return {
    unread: false,
    updatedAt: 0,
    readAt: 0,
    observedAt: 0,
    run: null,
    settled: [],
  };
}

// Only session-execution terminals complete a prompt. Model steps, tool
// completions, idle snapshots and history reconciliation never emit completion.
export function activityEvent(provider, method, data, envelope = {}) {
  const id =
    data.threadId ??
    data.thread?.id ??
    data.sessionID ??
    data.session?.id ??
    data.info?.id;
  if (typeof id !== "string") return null;
  const common = { sessionId: id, eventId: envelope.id, at: envelope.created };
  if (provider === "codex") {
    const executionId = data.turn?.id ?? data.turnId;
    if (method === "turn/started")
      return { ...common, type: "started", executionId };
    if (method === "turn/completed") {
      const outcome = data.turn?.status?.type ?? data.turn?.status;
      if (["completed", "failed", "interrupted"].includes(outcome))
        return { ...common, type: "completed", executionId, outcome };
    }
    if (
      /^item\/(?:agentMessage|reasoning|commandExecution)\//.test(method) ||
      ["item/started", "item/completed", "error"].includes(method)
    )
      return { ...common, type: "content" };
  } else {
    if (method === "session.execution.started")
      return { ...common, type: "started" };
    if (/^session\.execution\.(succeeded|failed|interrupted)$/.test(method))
      return {
        ...common,
        type: "completed",
        outcome: method.endsWith("succeeded")
          ? "completed"
          : method.split(".").at(-1),
        quiet: data.reason === "shutdown",
      };
    if (
      /^session\.(?:text|reasoning|tool|compaction|shell)\./.test(method) ||
      method === "session.synthetic" ||
      data.message?.type
    )
      return { ...common, type: "content" };
  }
  return null;
}

export function applyActivity(record, event, viewed, now) {
  record ??= emptyActivity();
  const at = Number.isFinite(event.at) ? event.at : now;
  const identity = event.executionId ?? event.eventId;
  if (identity && record.settled.includes(identity)) return { record };
  if (event.type !== "content" && at < record.observedAt) return { record };
  let run = record.run;
  let completion;
  let settled = record.settled;
  if (event.type === "started") {
    if (run && run.finishedAt === null && (!identity || run.id === identity))
      return { record };
    run = {
      ...(run?.awaitingStart && run.finishedAt === null ? run : {}),
      id: identity ?? `live-${now}`,
      startedAt:
        run?.awaitingStart && run.finishedAt === null
          ? Math.min(run.startedAt, at)
          : at,
      awaitingStart: false,
      finishedAt: null,
      outcome: null,
    };
  } else if (event.type === "completed") {
    if (
      event.executionId &&
      run &&
      !run.awaitingStart &&
      event.executionId !== run.id
    )
      return { record };
    // A repeated unidentifiable terminal is quiet until a new start arrives.
    if (!identity && run?.finishedAt != null) return { record };
    if (run && run.finishedAt === null) {
      run = {
        ...run,
        id: event.executionId ?? run.id,
        awaitingStart: false,
        finishedAt: Math.max(run.startedAt, at),
        outcome: event.outcome,
      };
      if (!event.quiet) completion = run;
    }
    const keys = [identity, run?.id].filter(Boolean);
    settled = [...new Set([...settled, ...keys])].slice(-32);
  }
  // Provider timestamps stop reconnect replays from making viewed content unread.
  const freshContent =
    event.type !== "started" && (event.at == null || at > record.readAt);
  const unread = freshContent ? !viewed : record.unread;
  return {
    record: {
      ...record,
      run,
      settled,
      unread,
      updatedAt: freshContent
        ? Math.max(record.updatedAt, at)
        : record.updatedAt,
      readAt:
        viewed && freshContent ? Math.max(record.readAt, now) : record.readAt,
      observedAt: Math.max(record.observedAt, at),
    },
    completion,
  };
}

export function readMarkers(value) {
  if (!Array.isArray(value)) return new Map();
  return new Map(
    value.slice(-MAX_READ_MARKERS).flatMap((entry) => {
      if (!Array.isArray(entry) || entry.length !== 2) return [];
      const [key, marker] = entry;
      if (
        typeof key !== "string" ||
        key.length > 8192 ||
        key.split("\0").length !== 3 ||
        !["codex", "opencode"].includes(key.split("\0")[1]) ||
        !marker ||
        typeof marker.unread !== "boolean" ||
        ![marker.updatedAt, marker.readAt].every(
          (time) => Number.isFinite(time) && time >= 0,
        )
      )
        return [];
      return [
        [
          key,
          {
            ...emptyActivity(),
            unread: marker.unread,
            updatedAt: marker.updatedAt,
            readAt: marker.readAt,
          },
        ],
      ];
    }),
  );
}

export function savedMarkers(records) {
  return [...records]
    .sort(
      ([, a], [, b]) =>
        Math.max(a.updatedAt, a.readAt) - Math.max(b.updatedAt, b.readAt),
    )
    .slice(-MAX_READ_MARKERS)
    .map(([key, { unread, updatedAt, readAt }]) => [
      key,
      { unread, updatedAt, readAt },
    ]);
}

export function projectActivity(workspaces, member, lookup, requests) {
  const working = [],
    waiting = [],
    unread = [];
  for (const [path, workspace] of Object.entries(workspaces)) {
    if (!member(path)) continue;
    for (const provider of ["codex", "opencode"]) {
      const value = workspace.providers[provider];
      const sessions = new Map(
        value.sessions.map((session) => [session.id, session]),
      );
      for (const [id, session] of Object.entries(value.sessionsById))
        if (!sessions.has(id)) sessions.set(id, session.session.thread);
      for (const [id, session] of sessions) {
        if (
          value.archivedSessionIds.includes(id) ||
          value.archivingSessionIds.includes(id)
        )
          continue;
        const key = activityKey(path, provider, id);
        const target = { key, path, provider, session };
        const live = value.sessionsById[id];
        if (live?.isSubmitting || (live?.isWorking ?? session.isWorking))
          working.push(target);
        else if (
          sessionWaiting(live) &&
          !requests.some((request) =>
            request.projectPath === path && request.provider === provider &&
            request.sessionId === id,
          )
        ) waiting.push(target);
        if (lookup(key)?.unread) unread.push(target);
      }
    }
  }
  unread.sort(
    (a, b) => (lookup(a.key)?.updatedAt ?? 0) - (lookup(b.key)?.updatedAt ?? 0),
  );
  const attention = [
    ...new Map(
      requests
        .filter((request) => member(request.projectPath))
        .map((request) => [
          activityKey(
            request.projectPath,
            request.provider,
            request.sessionId ?? "",
          ),
          request,
        ]),
    ).values(),
  ];
  return { working, waiting, unread, attention };
}
