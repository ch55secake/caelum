export const diffSections = ["unstaged", "staged", "untracked"];

export function latestChangedFile(previous, changes) {
  const before = new Map(previous?.map((change) => [change.path, change]));
  const changed = changes.filter((change) => {
    const old = before.get(change.path);
    return (
      !old ||
      change.revision !== old.revision ||
      change.modifiedAt !== old.modifiedAt ||
      change.indexStatus !== old.indexStatus ||
      change.worktreeStatus !== old.worktreeStatus
    );
  });
  return changed.reduce((latest, change) => {
    if (!latest) return change;
    // A newly observed deletion has no remaining file timestamp. On first
    // opening the pane, prefer files whose edit order is actually known.
    if (previous && !change.modifiedAt) return change;
    if (previous && !latest.modifiedAt) return latest;
    return BigInt(change.modifiedAt ?? 0) > BigInt(latest.modifiedAt ?? 0)
      ? change
      : latest;
  }, null);
}

function hunks(content) {
  const result = [];
  let hunk;
  content.split("\n").forEach((line, index) => {
    if (line.startsWith("@@")) {
      hunk = { start: index, lines: [], signature: "" };
      result.push(hunk);
    } else if (hunk && /^[+-]/.test(line)) {
      hunk.lines.push(index);
      hunk.signature += `${line}\n`;
    }
  });
  return result;
}

export function changedDiffLines(previous, next) {
  return Object.fromEntries(
    diffSections.map((section) => {
      if (previous?.[section] === next[section]) return [section, []];
      // Ignore offset/context shifts when an earlier edit moves an existing
      // hunk. Counts distinguish repeated identical edits in the same file.
      const before = new Map();
      for (const hunk of hunks(previous?.[section] ?? ""))
        before.set(hunk.signature, (before.get(hunk.signature) ?? 0) + 1);
      const lines = hunks(next[section]).flatMap((hunk) => {
        const count = before.get(hunk.signature) ?? 0;
        if (count) {
          before.set(hunk.signature, count - 1);
          return [];
        }
        return hunk.lines.length ? [hunk.start, ...hunk.lines] : [];
      });
      return [section, lines];
    }),
  );
}
