# Desktop guide

## Projects and sessions

Open a project with **⌘O**, the sidebar's **+**, or `caelum /path/to/project`.
The sidebar groups projects, sessions and related pull requests. **⌘K** searches
sessions. **⇧⌘N** opens the project/provider launcher; **⌘N** reuses that project's
draft. Each session retains an independent draft and reading position. Hover a
session to close it with **×**. A temporary notice names the archived session and
offers **Undo**; it expires after eight unengaged seconds and pauses while hovered,
keyboard-focused or restoring. Undo preserves newer navigation. Session actions and
native context menus also offer archive/reconnect. Hover any session row for its
**…** popover; actions belong to that row, including background sessions. Escape
returns focus to its button. Use the archive icon beside
**Sessions** to switch between active and archived sessions. **New session** opens
the searchable project/provider launcher.

Choose the provider, model, reasoning effort and available agent in the composer.
Sending creates a provider session when needed. Later selection changes apply
to the next message. Search models and star favourites in the model picker.
The anchored popover keeps its original session or Settings target while open. Unavailable
current models and saved favourites remain visible; arrow keys navigate the
windowed catalogue, including after Tab moves focus. Escape or an outside click
closes it; its footer can save the current model, effort and agent as provider defaults.
Responses, thinking, typed Read/Run/Patch rows, compaction and shell activity
appear in the transcript with status and timing; expand rows for details. Patch
previews open automatically, with size and tool-detail controls. Explicitly collapsed
patches stay collapsed as output arrives. Internal reminders are hidden from rows,
timeline results and copied responses, while literal code stays intact. **Load earlier messages**
pages provider history while the visible transcript stays bounded.
Running OpenCode background commands remain above the composer with three truncated
output lines, elapsed time, refresh and **Stop**. **View output** opens a scrollable
reader; **View all** lists multiple commands. The reader keeps its original session
and retains output after completion. Completed commands also appear in the transcript.
The composer pulse and agent indicator show working, thinking, responding, tools,
retry, input, background-waiting and stopping states, including between output events.
These indicators follow reduced-motion preferences.

## Drafts, queues and requests

Enter sends or queues; Shift+Enter inserts a newline. The editor grows from 56 to
180 points, then scrolls independently above the model and send/stop controls.
Local queue entries capture their own settings and context; edit, reorder, remove,
pause or resume them. Failed entries return to the front with their original options.
OpenCode uses its provider-owned inbox during execution when settings are unchanged.
Those entries restore by message ID and support removal and admission/removal retry.
**Stop** or Escape interrupts the active turn and pauses its queue. Dropdowns,
dialogs, mentions, focused code selections and search surfaces consume Escape
before it reaches session cancellation.

Failed submissions stay separate from newer typing. Recovery retries once;
**Refresh session** explicitly retries again. Approvals and question
forms belong to the requesting session, including when another session is selected.
Permission choices are **Allow once** and **Deny**.

Drafts and pending sends save atomically before dispatch. Restored local queues pause.
An unacknowledged submission returns to the front of the paused queue; inspect
provider history before resuming because it may have been accepted before a crash.

## Composer context and commands

Use project-file drag/drop, caret-aware `@query` suggestions, or **Attach** in the file
inspector to attach immutable text snapshots. Each message supports 16 snapshots,
each capped at 64 KiB; partial snapshots are marked. Selection context retains
its file and line range. Navigation during a file read keeps the attachment with
its originating draft. **#** finds session-related pull requests and captures their
context. Tab/Enter accepts a mention; Escape cancels it, including an in-flight read.
Surrounding text and Unicode caret positions are preserved.

Paste or drop images for a thumbnail; open it to zoom and drag-pan the preview.
Images remain image payloads when sent or queued. User messages use right-aligned
Markdown bubbles with grouped file/image context and a hover/keyboard copy action.
Message and file context menus
provide copy, reuse and attachment actions. Up in an empty composer recalls the
last prompt; Shift+Tab cycles available OpenCode agents. The context meter shows
reported token usage and model capacity when available.

Type **/** to discover local and provider commands. `/new`, `/timeline`, `/files`,
`/changes` and `/settings` open native workflows. OpenCode also supports `/usage`,
`/stats`, provider-discovered commands and `/btw question` (`/ask` is an alias).
Side questions have a separate editor, context, retry and answer-copy actions;
they preserve the main draft and do not add a conversation turn. Timeline groups
prompts by date, shows excerpts and highlights search matches. Use ↑/↓ and Enter to
jump back to a prompt; loading older history preserves its identity. Statistics
presents summary cards, an activity chart, token breakdown and model costs, with
date-range and project filters.

## Files, changes and worktrees

**Files** and **Changes** share the resizable side inspector. **⌘P** opens file
search; choose filename or content search. Directories and results are paged,
previews are read-only with syntax highlighting, virtualized lines and **⌘F**
source search. Tabs, folders and reading positions persist. Native checkout watches,
provider/tool events, **Refresh** and foreground activation reload files and Changes
while preserving position. Repeated edits and staging coalesce; an edit during an
in-flight refresh schedules another read. Linked-worktree Git metadata is watched too.
Select lines to attach them to the prompt. Files, local staged/unstaged/untracked
changes and continuous PR review share the same native code/diff surfaces.

Use a project's branch icon or **Create worktree** in the project/provider launcher
to open the creation form. Replace the selected suggested branch, search the starting
branch and review the destination, then press Enter or **Create worktree**. The form
keeps its original project and provider. **Manage checkouts** opens discovery and
removal. Change the parent directory in Settings; the default storage is under
`~/.caelum/worktrees`. Each checkout keeps its own sessions and drafts. Removing
a checkout preserves its branch and commits; active work and dirty/locked Git
checkouts prevent removal.

PR review has **Overview** and **Files changed** tabs. Overview groups the description,
checks, merge readiness and discussion. Files changed keeps searchable file/thread
navigation beside the continuous diff; the file list can be collapsed in compact windows.
Inline feedback, collapsed resolved threads, outdated context and missing/binary
patches remain available. Open reviews refresh every 30 seconds and preserve their
reading position across refreshes, pagination and tab changes. The pinned
**Address selected comments** action starts a new turn in the displayed destination
session, preserving its independent draft and attachments. This bar appears only
while comments are selected. See
[pull requests](pull-requests.md) for feedback and merging.

## Appearance and window lifecycle

The top-toolbar gear or **⌘,** opens Settings. Choose System/Light/Dark, Caelum/Catppuccin/Nord/Tokyo Night,
and six accents or the theme default. Its five pages are Appearance, Providers,
Worktrees, Notifications and About. Provider defaults/favourites, notification
visibility and sounds, worktree storage, legacy settings import and persistence
retry live here. Unread state persists and clears only when the loaded conversation
is visible and following the latest messages.

Closing the window keeps sessions running. Clicking the Dock icon or opening
a project through the CLI brings it back. **⌘Q** quits and saves the latest drafts.

| Shortcut | Action |
| --- | --- |
| Enter / ⌘Enter | Send or queue |
| Shift+Enter | Newline |
| Escape | Dismiss the current surface, otherwise stop active work |
| ⌘O / ⌘N | Open project / new session |
| ⇧⌘N | Project/provider launcher |
| ⌘K | Search sessions |
| ⌘L | Focus composer |
| ⌘\\ / ⇧⌘B | Toggle sidebar |
| ⌥⌘I | Toggle inspector |
| ⌘P | Files/search |
| ⇧⌘F | Content search |
| ⌘F | Search the open source file |
| Shift+Tab | Cycle OpenCode agent in the composer |
| ⌘, | Settings |

## Saved data

State lives in `~/.caelum/workspace-v1.json`, with branch attribution in the
adjacent `branches.sqlite3`. First launch imports the native evaluation snapshot
if present, otherwise the legacy desktop workspace database. Sources are retained;
provider conversations remain with the provider. Exported `caelum.settings.v1`
JSON can be imported from Settings. See [migration details](design/rune-migration.md)
for source paths, overrides and the remaining workflow gaps.
