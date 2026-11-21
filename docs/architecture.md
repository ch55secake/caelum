# Architecture

## Ownership

- `crates/caelum-core`: renderer-independent application state, provider adapters,
  persistence, scoped filesystem access, Git/worktrees and GitHub operations.
- `crates/caelum-protocol`: serializable values shared by interfaces and adapters.
- `apps/desktop`: Rune rendering, native editors/focus/dialogs, pane state and
  platform lifecycle integration. This is the shipping desktop application.
- `apps/cli`: clap-based terminal workflows and desktop project launching, using
  the same Rust providers and project validation.

The root Cargo workspace uses Rust 1.95.0 and one lockfile. Rune/rune-kit are pinned
to `e5e9ea9abacd20ddfa878cf5e5988430e121aba4`.

## Application runtime

`caelum-core::application::Workspace` owns sessions, options, drafts, queues,
approvals/questions, project selection and service requests. Commands/results
carry their originating project/provider/session and stable local session slot.
Request generations reject stale history, project, file and review responses.
`Conversation` reconciles provider history with live text, reasoning and tool
events without duplicating or overwriting newer activity.

`application/workspace.rs` defines the coordinator and stable session state.
Its `workspace/` modules own project navigation, session discovery/history,
archival, composer context, queues/submission, persistence, service tasks and
provider events. Completion routing preserves each request's original owner and
generation. The application and parity suites are grouped by capability under
`application/tests/` and `application/parity_tests/`.

`workspace/home.rs` owns the saved Home draft, target/options and asynchronous
attachment/project requests. First send creates a fresh stable session only after
admission, transferring its captured context through the normal durable submission
path. Home request generations reject results for an older target or draft.

`Runtime` uses eight normal workers, two conversation workers, two control workers,
bounded command queues, a 1,024-event inbox and 256-event UI drains. Arrival wakes
Rune; provider I/O does not run on the UI thread. Cancellation and permission
replies bypass the normal persistence gate. Resume, history pagination and inbox
recovery have reserved workers so background catalogues, diffs and discovery
cannot occupy their capacity.
The shared `ProviderRuntime` owns providers, brokers, process registration and
session-branch attribution.

Remote checkouts live in `caelum-core::remote`, outside the local project and
filesystem registries. Settings owns the Boat connection; a checkout captures
its account, machine ID, GitHub repository and branch. The local project path
only groups the sidebar row. A saved creation attempt carries its idempotency
key through provisioning; once Boat returns a machine, retries prepare that
same machine. In a single-repository environment Caelum resolves the physical
Git worktree on the VM, verifies its GitHub origin and saves the directory for
managed Git and native SSH. A new VM can fill a missing clone from its published
base; attaching to an existing VM only verifies it. Caelum can create a dedicated environment and never
adds a repository to an existing shared environment.

On explicit Send, the remote composer captures the thread’s provider (`codex`
or `opencode`), optional Boat model ID and optional `reasoningEffort`.
The pickers share the public SDK catalogue; only a concrete model’s advertised
levels are selectable. Each default omits its request field. The public harness
default does not identify the account’s configured model. Changing models
clears effort; switching providers clears both overrides but keeps the conversation ID and event cursor;
the accepted run records which provider authored its replies. This managed
composer uses Boat’s integrated-agent API.
Each send explicitly targets a new conversation or its saved Boat conversation ID.
The core follows ascending event
pages and uses `promptRun.done` to finish only after the last page. The account,
machine, conversation, prompt/run IDs, event cursor and bounded transcript
survive a restart. A lost prompt acknowledgement requires reconciliation;
reopening never resends it. Turn interruption always supplies the conversation
ID, whereas stopping the machine is a distinct lifecycle operation.

At the live edge the feed rereads the latest identified response snapshot;
terminal and intermediate pages advance their cursor. VM tool rows save a
bounded reference to the Boat conversation, prompt, event and call. Expansion
rereads its event page and keeps only the selected call’s bounded text in
memory. Commands and result bodies aren’t written to the workspace. Structured
reasoning items render their public `summary_text` blocks when supplied;
generic running state is shown as working, not inferred reasoning.

Remote **Changes** runs Git through the sandbox commands API. A temporary VM
index captures tracked edits and non-ignored new files as one reviewable Git
tree. Commit pins that tree and checks the branch, HEAD and original index;
edits after review stay in the working directory. Push sends the reviewed
commit to the verified GitHub origin without force. To bring it back to the
laptop, fetch the branch in the local checkout.

VM sessions use the same workspace, session slots, editors and sidebar as local
sessions. The desktop resolves a saved checkout ID to its Boat account, machine,
repository and verified Git root. It translates project ownership at the SSH
boundary, leaving provider IDs and transcript content untouched. OpenSSH pins
the host key returned by Boat each time it authorizes the workspace’s public key.
One master per checkout forwards the OpenCode HTTP service and Codex WebSocket
app-server on demand. SFTP reads files; SSH runs Git and `gh` on the VM.

The provider servers survive detaching the desktop. A new connection verifies the
checkout, subscribes again, restores pending provider requests and
refreshes native histories. Only completions for that attachment are delivered;
no prompt is replayed. Native IDs and drafts live in the main snapshot under
their checkout identities. New handoffs populate a regular VM session, including
the bounded chat on its first explicit send after rechecking the published base.
Existing managed Boat chats retain their own conversation IDs.

Local Codex uses one `codex app-server --listen stdio://` JSONL connection per project.
OpenCode uses its verified local registered service or an owned loopback server.
Connection attempts are shared within a checkout; startup and question recovery
for one checkout do not lock other checkouts. Release/shutdown retires in-flight
attempts before they can publish a connection or recover stale forms.
Only owned processes are terminated on shutdown. See [Codex](codex.md),
[OpenCode](opencode.md) and [security boundaries](security.md).

Provider roots own connection/server lifetimes. `codex/` separates session
operations, inbound approval/question requests and JSONL transport; `opencode/`
separates sessions, events, permissions, questions, server startup and transport
alongside its shell, statistics and generation capabilities. Provider tests live
alongside these modules, with embedded fake servers in `tests/fixtures/`.

## Desktop modules

- `main.rs`: entry point, shell, keyboard routing and offline render modes.
- `lifecycle.rs` / `lifecycle/`: launch parsing, macOS menus/URL delivery, Linux
  per-workspace D-Bus activation, close/quit behaviour and explicit AppImage installation.
- `notifications.rs`: native macOS and Linux notification delivery and app/window activation;
  core notification effects carry stable project/provider/session targets.
- `live.rs`: editor/application synchronization and runtime wake/drain, with
  sidebar, windowed transcript, composer and session menus in `live/`.
- `state.rs`: pane/view state and the original simulated interaction fixture.
- `home.rs` / `home/`: Home navigation, native editor, project/provider controls,
  cached background and deterministic preview scenes. Fresh launches open Home;
  explicit project launches and notification targets open their conversations.
  `home/art` owns the dark-mode ASCII compositor: one colour/glyph grid and atlas,
  compiled off the UI thread, with render-clock motion independent of view commits.
  Only a visible animated slot keeps drawing; Reduce Motion uses a static frame.
- `panels.rs`: panel state, synchronization and routing. `panels/` owns the
  file/change inspector, review/feedback, appearance, launcher, queue and overlay
  views. Local and review diffs have independent state.
- `questions.rs`: form editors backed by shared Rust validation.
- `theme.rs` / `ui.rs`: semantic palettes, typography and shared surfaces.
- `evaluation.rs`: offline design and demanding-workload fixtures.
- `tests.rs`: shared native-runtime helpers, with capability suites in `tests/`.

Each session has an independent native editor and reading position. Settings,
worktree and session dialogs scope focus. Long transcripts, file previews and
review patches are windowed/bounded. Provider/model/effort/agent choices are
captured when a message is queued. Rendering state never owns provider processes.

## Persistence and migration

`~/.caelum/workspace-v1.json` stores versioned projects, sessions, drafts,
attachments, options, queues and preferences. `CAELUM_STATE` overrides the path;
`CAELUM_RUNE_STATE` remains a compatibility override. A lifetime file lock
prevents concurrent desktop writers. Writes use a private temporary file,
fsync and atomic replacement; submissions wait for their own durable snapshot.
Invalid/future state blocks saving instead of becoming an empty workspace.

The optional `home` field stores its independent text, attachment snapshots and
project/provider/model choices. Older snapshots restore an empty Home draft.

On the first production-default launch, migration prefers the evaluation
snapshot at `~/.caelum/rune/workspace-v1.json`. Otherwise it imports workspace
metadata from `~/Library/Application Support/dev.caelum.desktop/workspace.sqlite3`.
SQLite backup includes committed WAL data; migration runs only on the copied
database. Sources remain available. Explicit state overrides skip migration.
Legacy appearance/provider preferences can be imported from exported v1 JSON.

`branches.sqlite3` beside the snapshot retains session-specific branch history.
Conversation history remains provider-owned. On macOS, closing the window keeps
the runtime and state alive; Dock activation or a project URL reopens it. Linux
closes the process with its last window. Quitting
stops workers/owned providers before flushing the latest editors and releasing
the lock. Restored queues and unacknowledged submissions pause for explicit resume.

## Projects and services

Core validates project roots and scoped paths. Checkouts group under a project
while retaining independent provider sessions and drafts. Worktree creation uses
Git's native APIs and stable per-repository storage. Removal checks active work,
pending requests and Git dirty/locked state; it preserves branches and commits.

`SessionBranches` records branches during unambiguous execution, retaining earlier
branches across restart/archive. Concurrent sessions in the same checkout do not
claim each other's changes. Linked worktrees remain independent. Provider-native
metadata can seed Codex history; idle sessions keep their historical associations.

GitHub operations use authenticated `gh` through bounded subprocesses. Discovery
follows recorded branches and stacked bases, deduplicating canonical PR URLs.
Files and feedback paginate against the expected head/base. Review generations
reject late pages even after a same-head refresh. Mutations remain locked across
navigation/refresh and pin the displayed head. GitHub enforces merge permissions
and repository rules. Selected feedback becomes editable composer context.

Review opening reads the last overview/files from disk alongside a fresh details
request. `github/cache.rs` owns the best-effort SQLite cache at the workspace
snapshot path plus `.pr-cache-v1.sqlite3`, preserving isolated state overrides.
Canonical PR URLs share entries across worktrees; file pages additionally pin both
base and head commits. The cache retains at most 128 overview/page records and
32 MiB of JSON, with a 4 MiB per-record limit and seven-day expiry. Eviction reclaims
SQLite pages. All cache I/O runs on service workers; cache failures fall back to
GitHub. Read-only review tasks bypass draft saves after workspace restoration.

Cached overviews remain labelled until live details arrive, and do not provide
current checks or merge actions. Matching commits retain cached files; either
commit changing invalidates files and old in-flight pages. Uncached file pages
use the details snapshot followed by the files request and a final canonical
URL/base/head validation, avoiding a redundant pre-files request.
