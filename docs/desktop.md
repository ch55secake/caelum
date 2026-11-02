# Desktop guide

## Sessions and projects

Open a project and send a message to create a session. Choose a provider, model and available agent before sending. Model and reasoning-effort selections in an existing session apply to its next message; changing models resets effort to that model's default. Each session retains its draft, reading position, attachments and expanded tools while the app is open.

The sidebar shows each project's remembered session title, current branch and change count. Clicking the project returns to its remembered checkout and session. Project activity indicators include background work and pending approvals. Hover or focus a project to see its full title and path. Drag the sidebar divider to resize it, or focus it and use arrow keys, Home and End.

**Sessions** combines both providers across the project's available checkouts, including unopened linked worktrees. Selecting a session switches to its checkout. Search by title, provider, branch or checkout path with **⌘K**; Escape clears and closes search. Session and pull-request lists scroll independently and retain their per-project positions.

Hover or focus a session row to **Archive** it. Archiving keeps provider-owned conversation history; **Undo** restores the most recently archived session and its in-app draft/settings. Stop an active turn before archiving. Refresh failures preserve known sessions, and an uninstalled optional provider does not produce a discovery warning. Genuine failures identify the provider and checkout in the warning's details.

## Conversation and composer

Responses, thinking and tool activity stream live. Expand a row for the complete trace, input or output. Patch previews show compact changed-code snippets; execute previews show their JavaScript. Long-running tools show elapsed time, and quiet gaps do not imply that execution has finished. Activity animations respect reduced motion.

Provider-owned history loads in pages; choose **Load earlier messages** to read further back. Compaction appears as a checkpoint, including running, completed, failed and interrupted states. Earlier messages remain available. OpenCode checkpoints can include a summary. **Turn cancelled** clears when work resumes; a resumable shutdown appears as **Execution paused**.

Draft or queue another message while a response runs. **Stop** interrupts the active turn. Failed submissions preserve text and attachments for retry; disconnected sessions offer **Reconnect**. Approvals appear in the requesting conversation, with indicators for background sessions.

OpenCode's **Build** and **Plan** controls choose the agent for your next message. **More** includes custom agents. **Shift+Tab** cycles available agents without moving the caret. The current, running and pending-next agent states are distinct, and a failed switch preserves the draft.

The **Context** meter shows the latest reported context usage and model limit, not cumulative usage. Codex reports its effective limit; OpenCode uses the latest assistant step and its model's limit, resetting after compaction. Missing information is shown as unavailable.

Click an image to open the centred viewer. Use **Fit**, **100%**, or the zoom controls; drag or scroll enlarged images to pan. Escape closes the viewer and restores thumbnail focus.

### Commands

Type **/** for local commands and OpenCode's discovered native/custom project commands. Use arrows to navigate, Tab or click to complete, Enter to run and Escape to dismiss.

Type **@** at the caret to find files in the current checkout with fuzzy matching. Use arrows and Tab/Enter, or click a result, to insert its path and attach a text snapshot for either provider. Escape dismisses completion. Read failures preserve the draft and existing attachments; the same 16-snapshot and 64 KiB limits apply.

- **/new** opens a new-session draft in the current checkout/provider. It also works during execution and keeps the original session's attachments/settings.
- **/goal objective**, **/goal pause**, **/goal resume**, **/goal clear** use OpenCode's native goal workflow, including during execution.
- **/usage** opens OpenCode Go allowances, Console budgets and current-session context as separately labelled values. It works before creating a session and keeps attached context. Codex account usage is not included.
- **/stats** shows all recorded OpenCode usage on the connected server, including projects outside Caelum. Filter by OpenCode project (including linked worktrees) and inclusive local dates, or choose **All time**. Totals include input, output, reasoning, cache read/write, recorded cost, model breakdowns and daily activity. Missing pricing/subscription costs may be recorded as zero. Codex usage is not included.
- Other native/custom commands appear when OpenCode reports them and execute with the selected settings and attached snapshots.

## Files and changes

**Files** and **Changes** share a resizable inspector beside the conversation. Expand it for more space without discarding the conversation. The file tree has its own keyboard-accessible divider and All, Changes and Recent views.

- **⌘P** finds filenames with fuzzy matching; **⌘⇧F** searches file contents. Search respects ignore rules unless **Include ignored** is selected. Results are bounded and identify when a narrower query is needed.
- Read-only tabs, expanded folders, recent files and reading positions restore per project. Previews are capped at 1 MiB and refresh as files change.
- **⌘F** searches the active preview. Comparisons display HEAD beside the working tree with shared syntax colours.
- **Add to prompt** captures a file snapshot. Select code to **Add selection to prompt** or **Ask about selection**; these actions also appear in context menus.
- Each message supports 16 snapshots, each capped at 64 KiB of UTF-8 text. Partial snapshots are marked, and selections retain their path and line range. Both providers receive the captured text.
- Sent context chips and local references open the current file. Right-click a chip to copy the original attached snapshot.

**Changes** follows the latest edited file during agent work and highlights newly changed hunks, including repeated edits to already-dirty files. Unchanged refreshes preserve manual review positions. Staged, unstaged and untracked changes use syntax-aware diffs with addition/deletion markers.

## Worktrees

Right-click a project to **Create worktree**. Choose an editable generated branch name and a base branch. Existing checkouts are discovered automatically; **Refresh worktrees** updates the inventory. Expand a project's chevron to see its checkouts, or use the branch picker beside a new-session composer.

Each checkout supports multiple provider sessions with independent drafts and file tabs. Files and Changes follow the selected checkout, and the preferred checkout restores after restarting. Projects opened below a repository root retain that relative scope in other checkouts.

New checkouts default to `~/.caelum/worktrees/<project>-<repo-id>/<worktree>/`; choose a parent folder in **Settings → Worktrees**. Existing checkouts keep their locations, and separate clones get independent storage.

Removing a checkout retains its branch and commits. Stop sessions and resolve pending requests first; Git refuses dirty or locked checkout removal. Closing a project only closes its sidebar group.

## Settings and shortcuts

Open Settings from the sidebar, collapsed header or **⌘,**. Preferences save automatically on this device.

- **Appearance:** Dark, Light or System, with six theme-aware accent colours.
- **Providers:** default provider and independent model/effort defaults, plus OpenCode agent defaults. Existing sessions keep their own settings. Refresh provider choices after setup changes or reset defaults to use native settings.
- **Worktrees:** parent folder for new checkouts.
- Star models to pin per-provider favourites. Search models by name, ID or supplying provider, including custom/routed services.

| Shortcut | Action |
| --- | --- |
| Enter / ⌘Enter | Send or queue |
| Shift+Enter | Newline |
| ↑ in an empty composer | Recall the session's last message |
| Shift+Tab in the composer | Cycle OpenCode agents |
| ⌘N | New-session draft |
| ⌘K | Search sessions |
| ⌘\ | Toggle sidebar |
| ⌘P / ⌘⇧F | Search filenames / content |
| Shift+F10 | Context menu for the focused item |

Ctrl works in place of ⌘. Context menus support text editing, copying messages/code/tool output, attaching selections, file/path actions and session/project management. Navigate with arrows and Enter; Escape dismisses.

See [pull requests](pull-requests.md) for GitHub discovery, reviews, checks and merging.
