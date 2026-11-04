// Reduce provider events eagerly; only publishing their latest snapshot is
// deferred. One timer and one counter retain no events or obsolete snapshots.
// Urgent updates flush everything preceding them in the original arrival order.
export function createPublication(
  publish,
  schedule = setTimeout,
  cancel = clearTimeout,
) {
  let timer;
  let updates = 0;
  const clear = () => {
    if (timer !== undefined) cancel(timer);
    timer = undefined;
    updates = 0;
  };
  const flush = () => {
    clear();
    publish();
  };
  return {
    defer() {
      if (++updates >= 128) flush();
      else if (timer === undefined) timer = schedule(flush, 32);
    },
    flush,
    cancel: clear,
  };
}

// Only incremental output can wait. Unknown methods, requests, tool boundaries,
// failures, interrupts and execution terminals all publish synchronously.
export function deferredOutput(provider, method) {
  return provider === "codex"
    ? [
        "item/agentMessage/delta",
        "item/reasoning/summaryTextDelta",
        "item/reasoning/textDelta",
        "item/commandExecution/outputDelta",
      ].includes(method)
    : [
        "session.text.delta",
        "session.reasoning.delta",
        "session.tool.progress",
      ].includes(method);
}
