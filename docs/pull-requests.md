# Pull requests

Caelum uses the installed, authenticated GitHub CLI (`gh`).

## Discovery and review

The sidebar groups open PRs associated with checkout/session branches, including
stack ancestors. Shared URLs appear once. Caelum records branch history during
execution and preserves it across restarts and archive/undo. Concurrent sessions
sharing a checkout do not claim ambiguous branch changes; linked worktrees are
independent.

Select a PR to open the main review screen. **Back to session** opens the session
associated with the PR's source branch and restores its conversation and draft,
including sessions in linked worktrees and earlier recorded branches. Among matching
sessions, active sessions take priority, followed by the selected session and recent
activity. If no association is known, Back returns to the selected session.
A compact header identifies the reviewed repository and PR,
with its state, title, author, source → destination branches and change counts.
Long titles use at most two lines; hover truncated titles or branches for the full
text. The header keeps Back, GitHub and Refresh available while loading or retrying.
**Overview** puts review/check/branch readiness and merge actions before the
description, so long descriptions cannot bury blockers. Markdown and discussion
use a centred reading column. Checks start as a summary; **Details / Hide** expands their aligned
names, icons, statuses, timing and links. Skipped or unknown results are kept
distinct from successful checks. **Refresh review** reloads metadata;
open reviews also refresh every 30 seconds.

## Files and feedback

**Files changed** shows one continuous native unified/split diff. The independently
scrollable, searchable file list and previous/next-thread controls navigate within
it; collapse the list when more code space is needed. Further pages load near the end or with
**Load more files**. Large patches remain windowed. Reading position survives
refresh and review-tab navigation. Local and remote diffs keep separate state.
Patch headers offer copy/selection context actions. Binary or GitHub-omitted
patches are marked unavailable and link to GitHub.

Discussions, review summaries, threads and replies retain author, file/line,
resolution/outdated status and available diff context. Current threads appear
inline, resolved threads expand on demand, and unavailable-line/outdated comments
remain accessible. **More** loads additional
pages/replies. Selecting comments reveals the pinned **Address selected comments**
action to submit the source-linked feedback as a new turn to its displayed session.
The destination must be connected and idle with no pending requests or queued messages.
Its independent composer draft and attachments are preserved.

File/feedback reads are pinned to the displayed head and request generation;
late responses cannot replace a different review or mix commits. Periodic metadata
refresh failures retain loaded data and expose a retryable error.

## Branch updates and merging

**Update branch** appears when the branch is behind and requests GitHub's
base-into-head update. A joined green merge button and dropdown offer merge commit,
squash or rebase. Pending, blocked and conflicting states disable merging; draft,
merged and closed PRs omit the merge action. Both mutations pin the displayed head; GitHub checks
permissions and repository rules. In-flight mutations remain locked across
navigation and refresh, and errors remain available for retry.

Type **#** at the composer caret to attach session-related PR context without
leaving the draft. The [migration record](design/rune-migration.md) tracks the
remaining real-workflow acceptance.
