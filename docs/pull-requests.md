# Pull requests

Caelum uses the installed, authenticated GitHub CLI (`gh`).

## Discovery and review

The sidebar groups open PRs associated with checkout/session branches, including
stack ancestors. Shared URLs appear once. Caelum records branch history during
execution and preserves it across restarts and archive/undo. Concurrent sessions
sharing a checkout do not claim ambiguous branch changes; linked worktrees are
independent.

Select a PR to open the main review screen. **Back to session** restores the
conversation and draft. The review shows its description, branches, head commit,
checks, merge state and a link to GitHub. Check icons, timing and links distinguish
running, successful, failed and unknown results. **Refresh review** reloads metadata;
open reviews also refresh every 30 seconds.

## Files and feedback

Changed files form one continuous native unified/split diff. The independently
scrollable file list navigates within it; further pages load near the end or with
**Load more files**. Large patches remain windowed. Reading position survives
refresh and review-tab navigation. Local and remote diffs keep separate state.
Patch headers offer copy/selection context actions. Binary or GitHub-omitted
patches are marked unavailable and link to GitHub.

Discussions, review summaries, threads and replies retain author, file/line,
resolution/outdated status and available diff context. Current threads appear
inline, resolved threads expand on demand, and unavailable-line/outdated comments
remain accessible. **More** loads additional
pages/replies. Select comments and choose **Address selected comments** to append
the selected feedback and PR context to the current draft. Review/edit the prompt
before sending it with either provider.

File/feedback reads are pinned to the displayed head and request generation;
late responses cannot replace a different review or mix commits. Refresh failures
retain loaded data and expose the error.

## Branch updates and merging

**Update branch** requests GitHub's base-into-head update. Choose merge commit,
squash or rebase to merge. Both actions pin the displayed head; GitHub checks
permissions and repository rules. In-flight mutations remain locked across
navigation and refresh, and errors remain available for retry.

Type **#** at the composer caret to attach session-related PR context without
leaving the draft. The [migration record](design/rune-migration.md) tracks the
remaining real-workflow acceptance.
