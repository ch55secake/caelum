# Desktop guide

## Projects and sessions

Open a project with **⌘O**, the sidebar's **+**, or `caelum /path/to/project`.
The sidebar gives Projects, Sessions and Pull requests independently scrollable
space that adapts to their contents and window height. Use a project's **×** or
right-click → **Close project** to close its checkout group. Sessions, drafts,
attachments, queues and running work are retained; reopening the folder returns to
the remembered session. Closing the final project returns to the welcome screen.
Arrow keys navigate each section; Tab enters a project's actions and Left/Right
moves between Create worktree and Close. **⌘K** searches sessions in open projects.
**⇧⌘N** opens the project/provider launcher; **⌘N** reuses that project's
draft for the default provider chosen in Settings. Each session retains an
independent draft and reading position. Hover a
session to close it with **×**. A temporary notice names the archived session and
offers **Undo**; it expires after eight unengaged seconds and pauses while hovered,
keyboard-focused or restoring. Undo preserves newer navigation. Session actions and
custom context menus also offer archive/reconnect. Hover any session row for its
**…** popover; actions belong to that row, including background sessions. Escape
returns focus to its button. Use the archive icon beside
**Sessions** to switch between active and archived sessions. **New session** uses
the default provider chosen in Settings → Providers. **Refresh sessions** reloads
active session history across all open projects.

Right-click projects, sessions, messages, files, tabs and pull requests for themed
context menus. Actions apply to the item you clicked, including background sessions
and files. Text fields offer Undo, Redo, Cut, Copy, Paste and Select all, preserving
their selection and editing history. **⇧F10** or the Menu key opens the focused
item's menu; arrows navigate, Enter chooses, and Escape or Tab dismisses it.

Choose the model, reasoning effort and available agent in the composer. Effort
and agent controls have distinct icons and labelled popouts with readable names,
selected checkmarks and options to return to the model/provider default.
The composer is docked below the transcript, with a softly tinted, top-lit surface
in light and dark themes. Conversation text ends above it. The model is the primary
setting; borderless labels with small chevrons wrap into rows in narrow panes,
with a separate action row below. Shortcut hints and full setting values appear
in tooltips. The primary **Send** / **Queue** button keeps equal 12-point right and
bottom insets. **Stop** is a separate labelled control beside it while working.
Sending creates a provider session when needed. Later selection changes apply
to the next message. Search models and star favourites in the model picker.
The anchored popover keeps its original session or Settings target while open. Unavailable
current models and saved favourites remain visible; arrow keys navigate the
windowed catalogue, including after Tab moves focus. Escape or an outside click
closes it; its footer can save the current model, effort and agent as provider defaults.
Responses, thinking, typed Read/Run/Patch rows and shell activity
appear in the transcript with tool-specific icons, status and timing. Active labels
use a quiet left-to-right shimmer. Click a disclosure row or focus it and press
Space to reveal output and errors. Raw input arguments are omitted; the command,
filename or query stays in the heading. Successful commands show a tick, while
non-zero exit codes remain visible. Expanded results use a flat, lightly ruled
surface with selectable monospace text; file reads use syntax highlighting when
the language is recognised. The open icon beside the heading opens the file.
Each result has a copy action for its complete output, including content beyond
the preview. Long results scroll independently and offer **Show full output** /
**Show less** when truncated. Errors appear above logs with a labelled, coloured rule.
Patch previews open automatically and fit short changes to their content. Longer patches
use a compact scrollable preview with **Expand preview / Compact preview** controls.
File headers, unchanged lines and selections follow the selected theme; additions
and removals retain their status colours. Use the file chevron to collapse a file,
the open icon to view its source, the copy icon to copy the patch, or **…** beside
the tool heading for tool details.
Explicitly collapsed patches stay collapsed as output arrives. Internal reminders
are hidden from rows, timeline results and copied responses, while literal code stays intact. **Load earlier messages**
pages provider history while the visible transcript stays bounded.
Context compaction has its own transcript divider, including when the provider
supplies no summary. It shows running, completed, failed and interrupted states;
click a summary-bearing divider to read and copy its formatted context summary.
Running OpenCode background commands appear above the composer with three truncated
output lines, elapsed time, refresh and **Stop**. **Hide** reduces them to a command-count
row; **Show** restores the preview. Each session remembers this choice during the app
run, including after navigation and output updates. Hidden commands keep running.
**Output** opens a scrollable reader; **View all** lists multiple commands. The reader
keeps its original session and retains output after completion. Completed commands
also appear in the transcript.
The compact composer activity label shows working, thinking, responding, tools,
retry, input, background-waiting and stopping states, including between output events.
The same shimmer appears on running background commands, PR checks and update
checks. These indicators follow reduced-motion preferences.

## Drafts, queues and requests

Enter and the primary button send when idle or queue behind earlier work;
Shift+Enter inserts a newline. The editor grows naturally from 56 points as the
draft gets longer, up to a window-aware limit of 420 points, then scrolls
independently above the settings and actions. Shortening the draft reclaims space
for the conversation. Caret, selection and undo history stay in the native editor;
each session remembers its editor position during the app run. The transcript
stays above the dock and the edge remains neutral while editing.
Click **2 queued** (or focus it and
press Space) to expand queued messages; each session remembers its disclosure state.
Paused, saving and failed queues remain identifiable when collapsed. Local queue
entries capture their own settings and context; edit, reorder, remove, pause or
resume them. Failed entries return to the front with their original options.
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

Use project-file drag/drop, caret-aware `@query` suggestions, or **Add to prompt** in the file
inspector to attach immutable text snapshots. Each message supports 16 snapshots,
each capped at 64 KiB; partial snapshots are marked. Selection context retains
its file and line range. Navigation during a file read keeps the attachment with
its originating draft. **#** finds session-related pull requests and captures their
context. Tab/Enter accepts a mention; Escape cancels it, including an in-flight read.
Surrounding text and Unicode caret positions are preserved.

Click an attachment chip to preview the captured snapshot; its separate **×**
removes it. File previews have a filename/path header, syntax-highlighted source
and line numbers matching the captured range. Short snapshots fit their content;
longer ones scroll. Text stays selectable, and **Copy** copies the exact snapshot.
A **Partial** label identifies truncated captures. Previews show the attached content
even if the source changes. Closing a preview returns to the draft and its editor
position. Pending reads name each file and disable Send/Queue until they settle;
failed reads have a per-file error that can be dismissed. Attach again through the
same entry point to retry.

Paste or drop images for a thumbnail; open it to zoom and drag-pan the preview.
Images remain image payloads when sent or queued. User messages use right-aligned
Markdown bubbles with grouped file/image context and a hover/keyboard copy action.
Message and file context menus
provide copy, reuse and attachment actions. Up in an empty composer recalls the
last prompt; Shift+Tab cycles available OpenCode agents. The context meter shows
reported token usage and model capacity when available.

Type **/** to discover local and provider commands. `/new`, `/sessions`, `/models`, `/timeline`,
`/files`, `/changes` and `/settings` open native workflows. `/models` opens the
searchable model picker for the current session. `/variants` or bare `/effort`
opens the selected model’s effort choices; `/effort xhigh` selects an effort
directly, and `/effort default` restores the model default. OpenCode also supports
`/usage`, `/stats`, provider-discovered commands and `/btw question` (`/ask` is an alias).
`/sessions` searches the current project's Codex or OpenCode history, including archived
sessions; choose a result to load that conversation.
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
source search. Filename tabs use an active underline and reveal their close button
on hover or keyboard focus. Arrow keys switch tabs; Tab reaches the focused tab's
close button, and closing it returns focus to a remaining tab. Tabs, folders and
reading positions persist. Native checkout watches,
provider/tool events, **Refresh** and foreground activation reload files and Changes
while preserving position. Repeated edits and staging coalesce; an edit during an
in-flight refresh schedules another read. Linked-worktree Git metadata is watched too.
Select lines and choose **Add selection** to attach them to the prompt. Files, local staged/unstaged/untracked
changes and continuous PR review share the same native code/diff surfaces.

Expand **Worktrees** beneath a project to see the worktrees created
through Caelum. The section is hidden when that project has no Caelum worktrees.
Indented folder names and a connecting line show which project
owns each worktree. Rows show the current worktree and working, queued or unread
activity; hover for the branch and full path. Click the project card to return to
its original folder, or a worktree to restore its session, draft and reading position.
Arrow keys and Enter also navigate the list. Disclosure choices persist.
Right-click a worktree in the sidebar, manager or new-session launcher and choose
**Delete worktree** to remove it.
Caelum remembers created worktrees across restarts and storage-directory changes,
and recognises older worktrees in that project's Caelum storage folder.
Use **Refresh worktrees** to update the list, or **… → Manage worktrees** for a
compact list with the parent project's name and path, creation and removal actions.

Use a project's branch icon or **New worktree** in its manager
to open the creation form. Replace the selected suggested branch, search the starting
branch and review the destination, then press Enter or **Create worktree**. The form
keeps its original project and provider. The new-session launcher groups Caelum
worktrees beneath their parent project, including while searching, and offers
creation and management beside each project. Change the parent directory in Settings;
the default storage is under
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

The top-toolbar gear or **⌘,** opens Settings. Choose System/Light/Dark, Caelum/Vercel/Catppuccin/Nord/Tokyo Night,
and six accents or the theme default. Its five pages are Appearance, Providers,
Worktrees, Notifications and About, with keyboard-navigable sections and compact,
page-sized panels. Switching sections resets the page's scroll position. Provider
defaults/favourites, notification visibility and sounds, worktree storage, legacy settings import and persistence
retry live here. Unread state persists and clears only when the loaded conversation
is visible and following the latest messages.
About brings together the app version and platform, project links, and a manual
update check. Available updates open in your browser; the page also offers a
copyable app-details summary and legacy preferences import. Update failures can
be retried directly from the status panel.
The default Caelum palette keeps surfaces neutral, with colour in selection markers,
tab underlines, focus borders, status icons, links and primary actions.
Choose **Vercel** for a monochrome theme with a pure-black workspace and sidebar,
subtly raised panels, and a soft hairline sidebar divider with fading ends. Its light
variant follows Light/System appearance. Selecting Vercel starts with the theme's
monochrome accent; other accents remain available. The former **Black backgrounds**
setting migrates to Vercel.

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
