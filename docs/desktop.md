# Desktop guide

## Projects and sessions

Open a project with **⌘O**, the sidebar's **+**, or `caelum /path/to/project`.
The sidebar groups projects, sessions and related pull requests. **⌘K** searches
sessions. Each session retains an independent draft and reading position.
**Session actions** offers archive/undo, reconnect, model favourites and saved
provider defaults.

Choose the provider, model, reasoning effort and available agent in the composer.
Sending creates a provider session when needed. Later selection changes apply
to the next message. Responses, reasoning, tools, compaction and shell activity
appear in the transcript; expand rows for details. **Load earlier messages**
pages provider history while the visible transcript stays bounded.

## Drafts, queues and requests

Enter sends or queues; Shift+Enter inserts a newline. Queue entries capture their
own settings and attached text. You can edit, remove, pause or resume the queue.
**Stop** or Escape interrupts the active turn and pauses its queue; open dialogs,
session search and review navigation consume Escape first.

Failed submissions stay separate from newer typing. Recovery retries once;
**Refresh / reconnect session** explicitly retries again. Approvals and question
forms belong to the requesting session, including when another session is selected.
Permission choices are **Allow once** and **Deny**.

Drafts and pending sends save atomically before dispatch. Restored queues pause.
An unacknowledged submission returns to the front of the paused queue; inspect
provider history before resuming because it may have been accepted before a crash.

## Composer context and commands

Use **Attach**, project-file drag/drop, trailing `@query` suggestions, or the file
inspector to attach immutable text snapshots. Each message supports 16 snapshots,
each capped at 64 KiB; partial snapshots are marked. Selection context retains
its file and line range. Navigation during a file read keeps the attachment with
its originating draft.

Local commands are `/new`, `/files`, `/changes` and `/settings`. OpenCode's
discovered project commands also appear. `/ask question` generates a transient
answer from an existing OpenCode session without adding a conversation turn.

## Files, changes and worktrees

**Files** and **Changes** share the resizable side inspector. **⌘P** opens file
search; choose filename or content search. Directories and results are paged,
previews are read-only with syntax highlighting and 200-line pages, and **Refresh**
reloads the current file. Select lines to attach them to the prompt. Changes
supports local staged/unstaged/untracked patches in unified or split mode.

The sidebar's **Worktrees** button opens checkout discovery, creation and removal.
Choose a branch/base and parent directory; the default storage is under
`~/.caelum/worktrees`. Each checkout keeps its own sessions and drafts. Removing
a checkout preserves its branch and commits; active work and dirty/locked Git
checkouts prevent removal.

See [pull requests](pull-requests.md) for remote reviews, feedback and merging.

## Appearance and window lifecycle

**⌘,** opens Settings. Choose System/Light/Dark, Caelum/Catppuccin/Nord/Tokyo Night,
and six accents or the theme default. Settings also includes notifications,
worktree storage, legacy settings import and persistence-error retry.

Closing the window keeps sessions running. Clicking the Dock icon or opening
a project through the CLI brings it back. **⌘Q** quits and saves the latest drafts.

| Shortcut | Action |
| --- | --- |
| Enter / ⌘Enter | Send or queue |
| Shift+Enter | Newline |
| Escape | Dismiss the current surface, otherwise stop active work |
| ⌘O / ⌘N | Open project / new session |
| ⌘K | Search sessions |
| ⌘L | Focus composer |
| ⇧⌘B | Toggle sidebar |
| ⌥⌘I | Toggle inspector |
| ⌘P | Files/search |
| ⌘, | Settings |

## Saved data

State lives in `~/.caelum/workspace-v1.json`, with branch attribution in the
adjacent `branches.sqlite3`. First launch imports the native evaluation snapshot
if present, otherwise the legacy desktop workspace database. Sources are retained;
provider conversations remain with the provider. Exported `caelum.settings.v1`
JSON can be imported from Settings. See [migration details](design/rune-migration.md)
for source paths, overrides and the remaining workflow gaps.
