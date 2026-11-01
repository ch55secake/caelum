# Caelum

Caelum is an AI coding workspace for macOS, with a native Rust core shared by its Tauri desktop app and standalone CLI. The desktop supports project-scoped Codex and OpenCode sessions, workspace restoration, file browsing, and Git review.

## Project status

- **Desktop:** Tauri 2 + React + TypeScript + Vite + Tailwind CSS v4; native folder picker and Rust-validated project paths.
- **CLI:** Standalone Rust/clap executable sharing `caelum-core`; can launch the desktop app or run Codex and OpenCode workflows in a terminal.
- **Core:** independent Rust library for shared application behavior, project validation, and provider adapters.
- **Providers:** Codex and OpenCode integrations use their local server protocols and share the desktop session and approval experience.

## Repository layout

```text
crates/caelum-core/       Shared, GUI-independent application logic
crates/caelum-protocol/   Serializable types shared by interfaces
apps/desktop/             Tauri 2 and React desktop application
apps/cli/                 Standalone Rust CLI
docs/                     Architecture and development notes
.github/workflows/        Continuous integration
```

## Development prerequisites

- Rust stable with `rustfmt` and `clippy`
- Node.js 22 and pnpm 10
- Tauri 2 platform prerequisites ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

Alternatively, use the Nix development shell for the Rust, Node.js, pnpm, and Make toolchain:

```bash
nix develop
make help
```

On Linux, the shell also provides Tauri's native build libraries. On macOS, install the Tauri platform prerequisites on the host.

## Run locally

```bash
make help
make dev
```

`make dev` installs the frontend dependencies if needed and starts the Tauri desktop app. The machine must also have the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/) installed.

Build the macOS app bundle, or just the frontend:

```bash
make app
make frontend
```

Build and use the standalone CLI:

```bash
make cli
make cli-run PROJECT=.
caelum chat [PROJECT] [--provider codex|opencode]
caelum run "Fix the failing tests" --project .
caelum sessions [PROJECT] --provider opencode
```

With no subcommand, `caelum [PROJECT]` opens the project in the installed Caelum desktop app (macOS). Terminal workflows work without the desktop app. Use `--session ID` with `chat` or `run` to resume a provider-owned session; `/quit` exits interactive chat. Provider permissions are denied unless explicitly approved once in the terminal.

## Verification

```bash
make check
```

Session state and browser interaction checks:

```bash
pnpm --filter @caelum/desktop test
pnpm --filter @caelum/desktop exec playwright install chromium
pnpm --filter @caelum/desktop test:ui
```

Browser checks use a mocked desktop/provider boundary. Rust integration tests exercise the actual provider adapters against local fake servers.

## Sessions

Open a project and send a message to create a session. Choose the provider and any available model/agent before sending. Model and effort pickers also work in existing sessions: selections apply to the next message and are kept independently for each session. Effort options come from the selected model’s supported reasoning levels or OpenCode variants; changing models resets effort to the new model’s default. Each session keeps its own draft, reading position, and expanded tools while the app is open. Provider-owned history is restored in pages; use **Load earlier messages** to read further back.

OpenCode’s **Build** and **Plan** controls sit below the message box in new and existing sessions. Click either to choose the agent for your next message, or press **Shift+Tab** in the message box to cycle through available agents without moving the caret. **More** opens the full agent picker, including custom agents. A subtle animated underline marks the selected quick control, an animated ring identifies the running agent, an idle dot marks the current agent, and **Next** marks a pending change. Reduced-motion settings keep these indicators static, and screen readers announce the current and next agents. Pending selections stay with their session, and a failed agent switch keeps your draft ready to retry.

Right-click for Caelum's themed, context-aware menus: edit text fields; copy messages, code, selections, links, tool input/output, and diffs; add messages or selected code to your draft; open or archive sessions; and copy paths, reveal files in Finder, or manage projects and worktrees. **Shift+F10** opens the menu for the focused item; use arrow keys to navigate, **Enter** to choose, and **Esc** to dismiss.

Responses and thinking update live. Expand a thinking row to read the provider's summary or trace. A subtle composer outline pulses throughout work, including long-running commands and tool calls, which show elapsed time. OpenCode activity follows its runtime active-session state, so quiet gaps and history refreshes keep the running indicator steady until execution ends. Reduced-motion settings keep the outline static. Sending a message returns the transcript to the latest response. OpenCode connects to its existing local background service when available.

The composer’s **Context** meter shows the latest reported context usage. Hover or focus it for token counts and the model’s context-window size. Codex supplies usage and its effective limit directly; OpenCode uses the latest assistant step and that model’s reported limit, resetting after compaction. Missing usage or limits are shown as unavailable rather than guessed.

Type **/** in either provider’s composer for local commands; OpenCode also discovers native and custom project commands. Filter by name, use **↑/↓** to navigate, **Tab** or click to complete a command for arguments, **Enter** to run it, and **Esc** to close the picker. Native commands execute through OpenCode with your selected model/agent and attached file snapshots; their responses appear in the conversation.

- **/new** opens a new-session draft in the current project and provider, like **⌘N**. It works during an active turn, keeps attachments and pending settings with their original session, and creates the provider session on your first message.
- **/goal objective** uses OpenCode’s native goal workflow. **/goal pause**, **/goal resume**, and **/goal clear** also work during active execution; **Stop** remains available.
- **/usage** opens an **OpenCode Go usage** window with rolling, weekly, and monthly allowance cards, remaining percentages, and reset countdowns. OpenCode Console budgets and the current session’s context are labelled separately; Codex account usage is not included. Refresh keeps the last reported values visible. It works before creating a session and retains attached context.
- Other commands, such as **/review**, **/init**, and project-defined commands, appear when the connected OpenCode installation reports them.

Patch tool previews show compact, syntax-highlighted snippets of added and removed code beneath the affected filenames, including renames. Previews show up to two files and six changed lines per file, with counts for omitted changes; execute previews show the JavaScript being run. Expand a tool row to inspect the complete original input and output. **Turn cancelled** describes the current turn and clears when work resumes, including after a history refresh or reconnect. A resumable OpenCode shutdown appears as **Execution paused**.

The **Changes** pane follows and accents the latest edited file while the agent works and reveals newly changed diff hunks. Diffs use language-aware syntax colours with green/red backgrounds and `+`/`−` markers for additions and removals, across staged, unstaged, and untracked changes. Repeated edits to already-dirty files are detected; unchanged refreshes preserve your selection and reading position. Toggle it from the sidebar or header. Model selection is searchable by name or ID, with arrow-key navigation and Enter to select. Shell notifications display their command, execution ID, status, and output as expandable tool activity.

Your newest message has an accent rail. Press **↑** in an empty composer to recall the last message sent in that session, ready to edit and resend.

Context compaction appears as a checkpoint in live and restored Codex and OpenCode conversations, including running, completed, failed, and interrupted states. Expand an OpenCode checkpoint to read its summary or error when supplied. Earlier messages stay available, and the agent can keep working after compaction finishes.

Project rows show the folder path, current branch, and changed-file count or clean status. Git details refresh while projects are visible, with running-session and approval indicators for background work. Detached HEAD, non-Git folders, and unavailable Git status are identified explicitly.

Session branches show a green pull-request symbol when they have an open GitHub PR, or a muted symbol for drafts. Hover or focus the symbol for a compact preview with the title, description, branches, added/deleted lines, changed-file count, checks, and review status. Move into the card to keep it open; **Tab** or **↓** enters its actions, and **Esc** dismisses it. **View pull request** opens a dedicated screen in the main workspace with the full description and individual check results; **GitHub** opens the PR in your browser.

The **Pull requests** section below **Sessions** lists PRs attached to open sessions in the current checkout, including sessions you have not selected or scrolled to. Shared PRs appear once with their linked session name or count. Click a PR to open its screen directly; the sidebar stays available for switching between PRs and sessions. **Back to session** returns to your conversation, draft, and reading position. The list stays independent of session search and updates as sessions are archived or restored.

The PR screen's **Overview** shows compact check rows with green ticks for passing checks, animated pending indicators, and distinct failed or skipped results. Completed checks show their duration, and running checks show elapsed time ticking every second from GitHub's start timestamp. Timing is shown when GitHub provides it. Check names link to their details when available; pending checks refresh automatically while the screen is visible. Animations respect reduced-motion preferences. **Update branch** merges the latest base branch into the PR branch on GitHub, with progress and refreshed checks once the new head appears. A green **Ready to merge** badge appears in the preview and screen header when merging is available, with a highlighted merge panel and checkmark on the screen. The merge button turns green when ready and has a dropdown for **Create a merge commit**, **Squash and merge**, or **Rebase and merge**; its main action follows your selection. GitHub's merge status explains blocked actions, update and merge errors appear inline, and successfully merged PRs leave the open list.

**Files changed** is a single scrolling review of the remote PR: scroll through stacked, syntax-highlighted patches with old/new line numbers, rename paths, and added/deleted counts. Review threads and replies appear directly beneath their matching old/new diff lines, with selectable feedback and resolved/outdated labels. Resolved threads start collapsed into a compact **Resolved** row; expand one to read or select its feedback. File-level comments and threads outside the current patch sit beside their file with the original diff context; comments on unloaded or missing files, discussion comments, and review summaries follow the diffs. File headers and feedback actions stay visible as you scroll. Click a file in the navigator to jump to its diff, collapse individual files or all files, or hide the navigator for more room. Long file lists and expanded comment context share the page's vertical scroll. More files load automatically as you approach the end of the patches, with **Load more files** available too. Binary files and patches GitHub omits are identified. Files and feedback load on demand, preserve selections when switching to **Overview** and back, retain results on refresh failures, and have separate refresh controls.

The file navigator shows review-thread counts and highlights files with unresolved feedback. Under each commented file, line-level links show the old/new side, a short preview, and unresolved/resolved/outdated status; resolved threads show their location and status without a preview. Click a thread to expand its file and jump directly to the comment. Comments on unloaded or missing files and links to discussion/review summaries are included too. Patches render as you approach them, and visited diffs are reused when switching tabs.

Select feedback and click **Address selected comments** to send it to the currently selected Codex or OpenCode session and return to the conversation. The prompt includes the PR URL, branch, commit, exact comments, and code context. Current unresolved threads and non-approval feedback are selected by default; resolved or outdated threads can be included explicitly. Load additional pages and remaining replies to include them. The action uses an existing connected, idle session and preserves its draft and attached files. Failed sends keep the feedback available to retry.

This uses the installed, authenticated **GitHub CLI (`gh`)** and its repository selection. Branch discovery is shared by the list and indicators, with up to four concurrent lookups, and refreshes about once a minute while the sidebar is visible. **Refresh sessions** bypasses the discovery cache for both loaded providers, and execution events refresh the session's association immediately. Temporary lookup failures keep known PRs visible with an inline **Retry** action and retry about every 15 seconds. A successful lookup with no open PR removes the entry; ambiguous branch matches show an error. Richer details load on demand and are shared by PR URL across projects, sessions, and worktrees. Failed detail refreshes also retain the last loaded details and offer **Retry**.

Drag the left sidebar's right edge to resize it, or focus its divider and use **←/→**, **Home**, and **End**. Its preferred width is saved on this device and restored after collapsing or reopening the app. Narrow windows temporarily constrain the sidebar to leave room for the conversation and inspector.

Session rows show a branch and added/deleted lines from the latest turn when available. Caelum records each session's branch when execution starts and finishes, including branches created during a task. These associations survive navigation, app restarts, and archive/undo. Overlapping sessions in the same checkout keep their previous associations rather than claiming ambiguous branch changes; linked worktrees are tracked independently. Existing Codex sessions can use their native recorded branch until Caelum observes execution. Older OpenCode sessions gain an association the next time they run while connected to Caelum. Search also matches branch names. Hover or focus a row to **Archive** an old session; **Undo** restores the most recently archived session, including its in-app draft and pending settings. Archiving uses the provider’s archive and keeps its conversation history. Stop an active turn before archiving.

- **Enter** or **⌘Enter** sends; **Shift+Enter** inserts a newline.
- **Shift+Tab** in the message box cycles OpenCode agents for the next message.
- **⌘N** opens a new draft; **⌘K** searches sessions; **⌘\\** toggles the sidebar. Ctrl works in place of ⌘.
- Draft the next message while a response runs. Stop interrupts the active turn.
- Approvals appear in the requesting conversation; the sidebar and header identify pending background requests.
- A disconnected session offers **Reconnect**. A failed submission restores its text and attached context in the composer.

## Project worktrees

Right-click a project and choose **Create worktree** to get started. The **Worktrees** section appears once linked worktrees exist, with a **+** button for creating more. Only created, linked worktrees appear in that section; click the project itself to return to its original directory. New worktrees use a generated, editable branch name and `main` as the default base when available; you can choose another local or remote-tracking base branch. Existing Git worktrees are discovered automatically; **Refresh worktrees** in the project menu or **Refresh** beside the section updates the list immediately.

Choose a checkout in the sidebar or the **Worktree** selector on a new session. Each checkout can have multiple Codex and OpenCode sessions, with independent drafts and file tabs. **Files** and **Changes** follow the selected checkout, and the selected worktree restores when you restart. Projects opened at a repository subdirectory keep that scope in each checkout.

New worktrees default to `~/.caelum/worktrees/<project>-<repo-id>/<worktree>/`. Change the parent folder in **Settings → Worktrees**; existing checkouts keep their locations. Separate clones get separate storage directories.

Hover a linked worktree to remove its checkout. Its branch and commits are retained. Stop running sessions and resolve pending agent requests first; Git refuses removal of dirty or locked worktrees. Closing a project only closes its sidebar group.

## Files and prompt context

**Files** and **Changes** share a resizable inspector beside the conversation. Drag its divider or use the arrow keys while the divider is focused; expand it for a larger preview and restore the conversation layout when finished. The file tree has its own draggable, keyboard-accessible divider, with its preferred width saved per project and fitted to the available space. **Sessions** stays highlighted whenever the conversation is visible alongside either pane.

- **⌘P** finds filenames with fuzzy matching; **⌘⇧F** searches file contents. Use the arrow keys and Enter to open a result at its matching line. Search respects ignore rules unless **Include ignored** is checked. Ctrl works in place of ⌘.
- Open multiple read-only file or diff tabs. Tabs, the active file, expanded folders, recent files, and reading positions restore per project on this device. The tree shows Git status and offers **All**, **Changes**, and **Recent** views. Visible files refresh automatically as the agent works.
- A lightweight CodeMirror viewer shares syntax colours with Changes, including Rust highlighting, and follows the app’s theme. **⌘F** searches the current preview; file comparisons show HEAD and the working tree side by side.
- **Add to prompt** attaches a file snapshot. Select code in a preview to **Add selection to prompt** or **Ask about selection**; these actions also appear in the editor’s right-click menu. Remove attachments using their chips in the composer.
- Each message supports up to 16 snapshots, each capped at 64 KiB of UTF-8 text, with partial snapshots marked. Selections retain their path and line range. Attachments stay with their project, provider, and session draft while the app is open, and both Codex and OpenCode receive the captured text.
- Sent messages show file-context chips. Click a chip, a local file reference in a response, or a file link in expanded tool details to open the current file. Right-click a chip to copy its original attached snapshot.

File previews are capped at 1 MiB. Search results are bounded and identify when a narrower query is needed.

## Settings

Open **Settings** from the sidebar, the header when the sidebar is collapsed, or **⌘,** (**Ctrl+,** on other platforms). Changes save automatically on this device.

- **Appearance:** choose Dark, Light, or System, and a Violet, Blue, Teal, Green, Amber, or Rose accent. Accent shades adapt to the theme across highlights, links, controls, activity indicators, and code selections. System follows your device’s appearance; themes also cover conversations, tools, Git review, and native window chrome.
- **Providers:** choose a default provider for new projects and save independent Codex/OpenCode model and reasoning-effort defaults, plus an OpenCode agent. Defaults apply to new sessions; the composer can override them for an individual draft. Each existing session keeps its own configured model and effort.
- **Worktrees:** choose the parent folder for new checkouts, or leave it empty to use `~/.caelum/worktrees`.
- Star models in the composer or Settings to pin favourites above the provider-grouped model list. Favourites save per device, separately for Codex and OpenCode. OpenCode models are grouped by their supplying provider (including custom and routed services); search by model name, model ID, or provider name.
- Provider status and available choices are checked against the selected project. Use **Refresh** after updating your local provider setup, or **Reset defaults** to use the provider’s own settings.

## Releases

See [docs/releasing.md](docs/releasing.md) for macOS release and Homebrew setup instructions.

Caelum is licensed under the [MIT License](LICENSE).

## Roadmap

See [docs/roadmap.md](docs/roadmap.md) for milestones and [docs/architecture.md](docs/architecture.md) for current boundaries and planned integrations.
