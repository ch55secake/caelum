export function inboxMessages(items) {
  return (items ?? []).flatMap((item) =>
    item.type === "user" &&
    typeof item.id === "string" &&
    typeof item.payload?.text === "string"
      ? [
          {
            id: item.id,
            prompt: item.payload.text,
            remote: true,
            pending: false,
            model: null,
            effort: null,
            agent: null,
          },
        ]
      : [],
  );
}

export function reconcileInbox(current, snapshot) {
  const ids = new Set(snapshot.map((item) => item.id));
  return [
    ...snapshot,
    ...current.filter(
      (item) =>
        !ids.has(item.id) && (!item.remote || item.pending || item.error),
    ),
  ];
}

export function queueEvent(session, method, data) {
  const messages = session.queuedMessages ?? [];
  if (!method.startsWith("session.inbox.")) return session;
  let next = messages;
  if (method === "session.inbox.enqueued") {
    const [item] = inboxMessages([{ ...data.item, id: data.inboxID }]);
    if (item && !(session.settledInboxIds ?? []).includes(item.id)) {
      const previous = messages.find((entry) => entry.id === item.id);
      next = previous
        ? messages.map((entry) => (entry.id === item.id ? item : entry))
        : [...messages, item];
    }
  }
  const settled = [
    "session.inbox.delivered",
    "session.inbox.cancelled",
  ].includes(method);
  if (settled) next = messages.filter((item) => item.id !== data.inboxID);
  return {
    ...session,
    queuedMessages: next,
    inboxRevision: (session.inboxRevision ?? 0) + 1,
    settledInboxIds: settled
      ? [
          ...(session.settledInboxIds ?? []).filter(
            (id) => id !== data.inboxID,
          ),
          data.inboxID,
        ].slice(-256)
      : (session.settledInboxIds ?? []),
  };
}
