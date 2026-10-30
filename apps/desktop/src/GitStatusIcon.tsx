import {
  ArrowRight,
  Copy,
  FileDiff,
  FilePlus2,
  Minus,
  Pencil,
  Plus,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";
import type { GitFileStatus } from "./gitReviewState.js";

const statusIcons: Record<string, { icon: LucideIcon; kind: string }> = {
  M: { icon: Pencil, kind: "modified" },
  A: { icon: Plus, kind: "added" },
  D: { icon: Minus, kind: "deleted" },
  R: { icon: ArrowRight, kind: "renamed" },
  C: { icon: Copy, kind: "copied" },
  U: { icon: TriangleAlert, kind: "conflicted" },
  "?": { icon: FilePlus2, kind: "untracked" },
};

export function GitStatusIcon({
  change,
}: {
  change: Pick<GitFileStatus, "indexStatus" | "worktreeStatus">;
}) {
  const code = `${change.indexStatus}${change.worktreeStatus}`;
  // Conflicts take priority over either side's added/deleted status.
  const status = ["DD", "AU", "UD", "UA", "DU", "AA", "UU"].includes(code)
    ? "U"
    : change.worktreeStatus.trim() || change.indexStatus.trim();
  const { icon: Icon, kind } = statusIcons[status] ?? {
    icon: FileDiff,
    kind: "changed",
  };

  return (
    <span className={`git-status-icon ${kind}`} aria-hidden="true">
      <Icon size={14} />
    </span>
  );
}
