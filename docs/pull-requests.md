# Pull requests

Caelum uses the installed, authenticated GitHub CLI (`gh`) and its repository selection.

## Discovery and navigation

Session branches show a green icon for open PRs and a muted icon for drafts. Hover or focus an icon for a preview; move into it to keep it open, use Tab or ↓ to reach its actions, and Escape to dismiss. **View pull request** opens the PR screen; **GitHub** opens the browser.

The sidebar's **Pull requests** section includes PRs linked to sessions across the project's checkouts, including off-screen/unselected sessions. Shared PRs appear once. The sidebar remains available while reviewing, and **Back to session** restores the conversation, draft and reading position.

Caelum records session branches during execution and retains their history across restarts and archive/undo. Discovery also follows each open PR’s base branch to recover the rest of a stack when branch switches happened between Git samples. The entire stack remains available in the independently scrolling PR list. Git refreshes update running-session associations without waiting for completion. Concurrent sessions sharing a checkout do not claim ambiguous branch changes; linked checkouts are independent. Existing Codex sessions can seed native branch metadata, while older OpenCode sessions gain an association when they next execute in Caelum.

Lookups are shared and bounded, refresh about once a minute while visible, and refresh immediately after execution or **Refresh sessions**. Temporary failures retain known PRs with Retry and a shorter refresh interval. Successful misses remove entries; ambiguous matches are reported. Details load on demand and are shared by canonical PR URL.

## Checks, branch updates and merging

**Overview** shows the description, review state and linked check results, including pending, failed and skipped statuses. When GitHub supplies timestamps, completed checks show their duration and running checks update elapsed time locally. Pending checks refresh automatically; animations respect reduced motion.

**Update branch** asks GitHub to merge the latest base into the PR branch and refreshes until the new head appears. **Ready to merge** highlights eligible PRs. Choose merge commit, squash or rebase from the merge button. GitHub enforces permissions and repository rules; updates and merges are pinned to the displayed head. Errors stay inline, progress survives navigation, and confirmed merges remove the PR from the open list.

## Files and feedback

**Files changed** is a continuous, syntax-highlighted review with line numbers, rename paths and change counts. File headers stay visible while scrolling. Collapse files individually or together, hide the navigator, or select a file to jump to it. The navigator scrolls independently and includes comment counts and line-level links.

Review threads appear beside their matching old/new diff lines. Resolved threads start collapsed; outdated, file-level and out-of-patch comments retain their original context. Comments on unloaded files, discussion comments and review summaries remain accessible. Thread links expand and reveal their destination.

More files load near the end of the review, with an explicit **Load more files** option. Binary files and omitted patches are identified. Initial loading uses themed skeletons; later refresh failures retain loaded content. Visited patches and feedback selections survive tab changes, and files/feedback have separate refresh controls.

Select feedback and choose **Address selected comments** to send it to the current connected, idle Codex or OpenCode session. The prompt includes canonical PR identity, commit, exact selected comments and code context while preserving the session's draft and attachments. Current unresolved feedback is selected initially; resolved/outdated feedback can be included explicitly. Load remaining pages/replies before including them. Failed sends preserve selections for retry.
