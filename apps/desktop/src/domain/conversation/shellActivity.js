const text = (value) => (typeof value === "string" ? value : undefined);
const number = (value) => (Number.isFinite(value) ? value : undefined);

export function shellDetails(name, input, metadata = {}) {
  if (!/^(?:run|shell|bash|commandExecution)$/i.test(name ?? ""))
    return undefined;
  let fields = input;
  if (typeof input === "string") {
    try {
      fields = JSON.parse(input);
    } catch {
      fields = {};
    }
  }
  const command = text(fields?.command) ?? text(input);
  if (!command) return undefined;
  return {
    command,
    cwd: text(fields?.workdir) ?? text(fields?.cwd) ?? text(metadata.cwd),
    exit: number(metadata.exitCode) ?? number(metadata.exit),
    signal: text(metadata.signal),
    background: fields?.background === true || metadata.background === true,
    outputTruncated: metadata.truncated === true,
  };
}

export function shellEntry(shell, output) {
  if (typeof shell?.id !== "string") return null;
  const exit = number(shell.exit);
  const status =
    shell.status === "running"
      ? "running"
      : shell.status === "killed"
        ? "interrupted"
        : shell.status === "timeout" || (exit != null && exit !== 0)
          ? "failed"
          : "completed";
  return {
    id: `opencode-shell-${shell.id}`,
    kind: "tool",
    text: "",
    name: "Run",
    executionId: shell.id,
    input: shell.command,
    output: output?.output,
    status,
    startedAt: shell.time?.started,
    durationMs:
      Number.isFinite(shell.time?.completed) &&
      Number.isFinite(shell.time?.started)
        ? Math.max(0, shell.time.completed - shell.time.started)
        : undefined,
    shell: {
      command: shell.command,
      cwd: shell.cwd,
      exit,
      signal: shell.signal,
      independent: true,
      background: shell.metadata?.background === true,
      outputTruncated: output?.truncated === true,
      timedOut: shell.status === "timeout",
    },
  };
}

export function recentShellOutput(value) {
  if (typeof value !== "string") return "";
  return value
    .slice(-4000)
    .split(/\r?\n|\r/)
    .slice(-6)
    .join("\n");
}
